use std::borrow::Cow;

use indexmap::IndexMap;
use kaeso_kpp::{DialectId, KppError, Origin, RawJson, RequestKind, StopReason, StreamMode, Usage};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::codec::{EgressCtx, EncodeCtx, sealed};
use crate::keepalive;
use crate::openai_chat::profile::ReplyReasoning;
use crate::openai_chat::response::{ReasoningOutput, chat_reasoning_output};
use crate::sse::{DEFAULT_MAX_FRAME_BYTES, SseError};
use crate::wire::{Short, type_tag, without_position};

/// Output delta counts gathered while passing a same-dialect SSE stream through, matching
/// `kaeso_core::estimate::Tally` so usage estimation on unreported or cancelled streams is unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassthroughTally {
    pub deltas: u64,
    pub bytes: u64,
    pub reasoning_deltas: u64,
    pub reasoning_bytes: u64,
    pub final_usage: bool,
}

impl PassthroughTally {
    fn add(&mut self, len: usize, reasoning: bool) {
        let len = u64::try_from(len).unwrap_or(u64::MAX);
        self.deltas = self.deltas.saturating_add(1);
        self.bytes = self.bytes.saturating_add(len);
        if reasoning {
            self.reasoning_deltas = self.reasoning_deltas.saturating_add(1);
            self.reasoning_bytes = self.reasoning_bytes.saturating_add(len);
        }
    }
}

#[derive(Debug, Default)]
pub struct PassthroughStep {
    pub events: u32,
    pub saw_output: bool,
    pub ended: bool,
    pub error: Option<KppError>,
}

#[derive(Clone, Debug)]
pub struct PassthroughSummary {
    pub id: String,
    pub model: String,
    pub usage: Usage,
    pub stop: StopReason,
    pub complete: bool,
    pub started: bool,
    pub tally: PassthroughTally,
}

/// Same-dialect SSE stream passthrough: forwards upstream SSE frames without decoding into KPP
/// events and re-encoding, while still extracting usage, model/id, completion state, output delta
/// tallies, and provider errors.
pub trait StreamPassthrough: Send + sealed::Sealed {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep;
    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep;
    fn flush_eof_error(&mut self, out: &mut Vec<u8>) -> PassthroughStep;
    fn encode_error(&mut self, err: &KppError) -> Vec<u8>;
    fn keepalive(&mut self) -> Vec<u8>;
    fn summary(&self) -> PassthroughSummary;
}

/// Returns a same-dialect passthrough handler when the stream can be forwarded directly without
/// changing what the app receives.
pub fn for_stream(
    dialect: DialectId,
    egress: &EgressCtx,
    encode: &EncodeCtx,
) -> Option<Box<dyn StreamPassthrough>> {
    if egress.kind != RequestKind::Generate || encode.kind != RequestKind::Generate {
        return None;
    }
    if !egress.tool_names.is_empty() || !egress.tool_ids.is_empty() {
        return None;
    }
    match dialect {
        DialectId::OpenAiChat => {
            if egress.chat_profile.reply.reasoning == ReplyReasoning::ThinkingParts {
                return None;
            }
            if chat_reasoning_output(encode) == ReasoningOutput::Exclude {
                return None;
            }
            let include_usage = matches!(
                encode.stream,
                StreamMode::On {
                    include_usage: true,
                    ..
                }
            );
            Some(Box::new(ChatPassthrough::new(egress, include_usage)))
        }
        DialectId::OpenAiResponses => {
            if crate::openai_responses::response::excludes_reasoning(encode.request.as_deref()) {
                return None;
            }
            if !crate::openai_responses::namespaces::Namespaces::of(encode.request.as_deref())
                .is_empty()
            {
                return None;
            }
            Some(Box::new(ResponsesPassthrough::new(egress)))
        }
        DialectId::AnthropicMessages => Some(Box::new(AnthropicPassthrough::new(egress))),
        _ => None,
    }
}

// Splits raw SSE bytes into complete frame slices separated by a blank line while keeping the
// exact wire bytes of each frame so valid frames are forwarded with a single copy.
struct RawSseFramer {
    buf: Vec<u8>,
    started: bool,
    bom: Vec<u8>,
    max_bytes: usize,
}

impl RawSseFramer {
    fn new() -> Self {
        RawSseFramer {
            buf: Vec::new(),
            started: false,
            bom: Vec::new(),
            max_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }

    fn feed<F>(&mut self, chunk: &[u8], mut on_frame: F) -> Result<(), SseError>
    where
        F: FnMut(&[u8]),
    {
        let mut rest = chunk;
        if !self.started && !rest.is_empty() {
            const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
            let need = BOM.len().saturating_sub(self.bom.len());
            let take = need.min(rest.len());
            let (head, tail) = rest.split_at(take);
            self.bom.extend_from_slice(head);
            rest = tail;
            if self.bom.len() < BOM.len() && BOM.starts_with(&self.bom) {
                return Ok(());
            }
            self.started = true;
            if self.bom == BOM {
                self.bom.clear();
            } else {
                let prefix = std::mem::take(&mut self.bom);
                self.buf.extend_from_slice(&prefix);
            }
        }
        if self.buf.is_empty() {
            // Fast path: scan directly over `rest` without copying complete frames into `self.buf`.
            let mut start = 0usize;
            while start < rest.len() {
                if let Some(end) = find_frame_end(rest, start) {
                    let frame = rest.get(start..end).unwrap_or_default();
                    if frame.len() > self.max_bytes {
                        return Err(SseError::TooLarge {
                            limit: self.max_bytes,
                        });
                    }
                    if std::str::from_utf8(frame).is_err() {
                        return Err(SseError::InvalidUtf8);
                    }
                    on_frame(frame);
                    start = end;
                } else {
                    let tail = rest.get(start..).unwrap_or_default();
                    if tail.len() > self.max_bytes {
                        return Err(SseError::TooLarge {
                            limit: self.max_bytes,
                        });
                    }
                    self.buf.extend_from_slice(tail);
                    return Ok(());
                }
            }
            return Ok(());
        }
        self.buf.extend_from_slice(rest);
        if self.buf.len() > self.max_bytes && find_frame_end(&self.buf, 0).is_none() {
            return Err(SseError::TooLarge {
                limit: self.max_bytes,
            });
        }
        let mut start = 0usize;
        while start < self.buf.len() {
            let Some(end) = find_frame_end(&self.buf, start) else {
                break;
            };
            let frame = self.buf.get(start..end).unwrap_or_default();
            if frame.len() > self.max_bytes {
                return Err(SseError::TooLarge {
                    limit: self.max_bytes,
                });
            }
            if std::str::from_utf8(frame).is_err() {
                return Err(SseError::InvalidUtf8);
            }
            on_frame(frame);
            start = end;
        }
        if start > 0 {
            self.buf.drain(..start);
        }
        if self.buf.len() > self.max_bytes {
            return Err(SseError::TooLarge {
                limit: self.max_bytes,
            });
        }
        Ok(())
    }

    fn finish<F>(&mut self, mut on_frame: F) -> Result<(), SseError>
    where
        F: FnMut(&[u8]),
    {
        self.started = true;
        if !self.bom.is_empty() {
            let prefix = std::mem::take(&mut self.bom);
            self.buf.extend_from_slice(&prefix);
        }
        if self.buf.is_empty() {
            return Ok(());
        }
        let trailing = std::mem::take(&mut self.buf);
        if std::str::from_utf8(&trailing).is_err() {
            return Err(SseError::InvalidUtf8);
        }
        // A line without a line terminator at EOF is a truncated event; a terminated frame without
        // its trailing blank line is still dispatched by `SseParser::finish`.
        if !matches!(trailing.last(), Some(b'\n' | b'\r')) {
            return Err(SseError::Truncated);
        }
        if !trailing.iter().all(|&b| b == b'\n' || b == b'\r') {
            on_frame(&trailing);
        }
        Ok(())
    }

    // Dispatches a trailing line-terminated frame (e.g. a CR-only stream whose final `\r` waited
    // for a possible `\n`) without consuming unterminated bytes that `finish` must reject.
    fn finish_terminated<F>(&mut self, mut on_frame: F) -> Result<(), SseError>
    where
        F: FnMut(&[u8]),
    {
        if self.buf.is_empty() || !matches!(self.buf.last(), Some(b'\n' | b'\r')) {
            return Ok(());
        }
        let trailing = std::mem::take(&mut self.buf);
        if std::str::from_utf8(&trailing).is_err() {
            return Err(SseError::InvalidUtf8);
        }
        if !trailing.iter().all(|&b| b == b'\n' || b == b'\r') {
            on_frame(&trailing);
        }
        Ok(())
    }
}

// Finds the byte offset right after the blank line terminating the frame starting at `from`, or
// right after a standalone comment line terminator.
fn find_frame_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let mut saw_content = false;
    while let Some(rest) = bytes.get(i..) {
        if rest.is_empty() {
            return None;
        }
        let rel = rest.iter().position(|&b| b == b'\n' || b == b'\r')?;
        let line_len = rel;
        let is_comment = !saw_content && line_len > 0 && rest.starts_with(b":");
        let term = *rest.get(rel)?;
        i += rel + 1;
        if term == b'\r' {
            match bytes.get(i) {
                Some(b'\n') => i += 1,
                Some(_) => {}
                // Wait for the next chunk in case `\r` is the first half of `\r\n`.
                None => return None,
            }
        }
        if is_comment {
            return Some(i);
        }
        if line_len == 0 {
            if saw_content {
                return Some(i);
            }
            // Skip stray leading blank lines so they stay attached to the next frame.
            continue;
        }
        saw_content = true;
    }
    None
}

fn split_sse_lines(mut text: &str) -> impl Iterator<Item = (&str, &str)> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        match text.find(['\r', '\n']) {
            Some(pos) => {
                let (line, rest) = text.split_at(pos);
                let eol_len = if rest.starts_with("\r\n") { 2 } else { 1 };
                let (eol, tail) = rest.split_at(eol_len);
                text = tail;
                Some((line, eol))
            }
            None => {
                let line = text;
                text = "";
                Some((line, ""))
            }
        }
    })
}

struct ParsedFrame<'a> {
    event: Option<&'a str>,
    data: Cow<'a, str>,
    has_data: bool,
    has_non_plain_framing: bool,
    comment_only: bool,
}

fn parse_frame(raw: &[u8]) -> ParsedFrame<'_> {
    let text = std::str::from_utf8(raw).unwrap_or_default();
    let mut event: Option<&str> = None;
    let mut data: Option<Cow<'_, str>> = None;
    let mut has_data = false;
    let mut has_non_plain_framing = false;
    let mut has_field = false;
    let mut has_comment = false;

    for (line, _) in split_sse_lines(text) {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(':') {
            has_comment = true;
            continue;
        }
        has_field = true;
        let (field, val) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => event = Some(val),
            "data" => {
                has_data = true;
                match &mut data {
                    None => data = Some(Cow::Borrowed(val)),
                    Some(existing) => {
                        let owned = existing.to_mut();
                        owned.push('\n');
                        owned.push_str(val);
                    }
                }
            }
            _ => has_non_plain_framing = true,
        }
    }

    ParsedFrame {
        event,
        data: data.unwrap_or(Cow::Borrowed("")),
        has_data,
        has_non_plain_framing,
        comment_only: !has_field && has_comment,
    }
}

fn some_raw<'de, D>(deserializer: D) -> Result<Option<&'de RawValue>, D::Error>
where
    D: Deserializer<'de>,
{
    <&'de RawValue>::deserialize(deserializer).map(Some)
}

trait FrameState {
    fn origin(&self) -> &Origin;
    fn is_inert(&self) -> bool;
    fn set_inert(&mut self);
    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep);
}

fn feed_framer<S: FrameState>(
    framer: &mut RawSseFramer,
    state: &mut S,
    chunk: &[u8],
    out: &mut Vec<u8>,
) -> PassthroughStep {
    let mut step = PassthroughStep::default();
    if state.is_inert() {
        return step;
    }
    let mut framed_err = None;
    let res = framer.feed(chunk, |frame| {
        if framed_err.is_none() && !state.is_inert() {
            state.process_frame(frame, out, &mut step);
            if step.error.is_some() {
                framed_err = step.error.take();
            }
        }
    });
    if let Some(err) = framed_err {
        step.error = Some(err);
    } else if let Err(e) = res
        && !state.is_inert()
    {
        state.set_inert();
        step.error = Some(KppError::from(e).with_origin(state.origin().clone()));
    }
    step
}

fn finish_framer<S: FrameState>(
    framer: &mut RawSseFramer,
    state: &mut S,
    out: &mut Vec<u8>,
) -> (PassthroughStep, bool) {
    let mut step = PassthroughStep::default();
    if state.is_inert() {
        return (step, true);
    }
    let mut framed_err = None;
    let res = framer.finish(|frame| {
        if framed_err.is_none() && !state.is_inert() {
            state.process_frame(frame, out, &mut step);
            if step.error.is_some() {
                framed_err = step.error.take();
            }
        }
    });
    if let Some(err) = framed_err {
        step.error = Some(err);
        return (step, true);
    }
    if let Err(e) = res {
        state.set_inert();
        step.error = Some(KppError::from(e).with_origin(state.origin().clone()));
        return (step, true);
    }
    (step, false)
}

fn flush_eof_framer<S: FrameState>(
    framer: &mut RawSseFramer,
    state: &mut S,
    out: &mut Vec<u8>,
) -> PassthroughStep {
    let mut step = PassthroughStep::default();
    if state.is_inert() {
        return step;
    }
    let mut framed_err = None;
    let res = framer.finish_terminated(|frame| {
        if framed_err.is_none() && !state.is_inert() {
            state.process_frame(frame, out, &mut step);
            if step.error.is_some() {
                framed_err = step.error.take();
            }
        }
    });
    if let Some(err) = framed_err {
        step.error = Some(err);
    } else if let Err(e) = res {
        state.set_inert();
        step.error = Some(KppError::from(e).with_origin(state.origin().clone()));
    }
    step
}

// ---------- OpenAI Chat Completions passthrough ----------

struct ChatPassthrough {
    framer: RawSseFramer,
    state: ChatState,
}

struct ChatState {
    origin: Origin,
    include_usage: bool,
    reasons_first: bool,
    error_detail: &'static [&'static str],
    id: String,
    model: String,
    usage: Usage,
    running_usage: Option<Usage>,
    held: Option<Held>,
    usage_chunk: Option<Vec<u8>>,
    stop: StopReason,
    started: bool,
    has_text: bool,
    has_reasoning: bool,
    finished: bool,
    done: bool,
    complete: bool,
    inert: bool,
    tally: PassthroughTally,
}

impl sealed::Sealed for ChatPassthrough {}

impl ChatPassthrough {
    fn new(ctx: &EgressCtx, include_usage: bool) -> Self {
        ChatPassthrough {
            framer: RawSseFramer::new(),
            state: ChatState {
                origin: ctx.origin(DialectId::OpenAiChat),
                include_usage,
                reasons_first: !ctx.chat_profile.reply.reasoning_fields.is_empty(),
                error_detail: ctx.chat_profile.reply.error_detail,
                id: String::new(),
                model: String::new(),
                usage: Usage::default(),
                running_usage: None,
                held: None,
                usage_chunk: None,
                stop: StopReason::EndTurn,
                started: false,
                has_text: false,
                has_reasoning: false,
                finished: false,
                done: false,
                complete: false,
                inert: false,
                tally: PassthroughTally::default(),
            },
        }
    }
}

impl ChatState {
    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        if self.inert {
            return;
        }
        let frame = parse_frame(raw);
        if frame.comment_only {
            self.release_held(false, out);
            out.extend_from_slice(raw);
            step.events = step.events.saturating_add(1);
            return;
        }
        if frame.has_data && frame.event == Some("error") {
            self.release_before_error(out);
            let err = crate::openai_chat::error::with_detail(
                crate::openai_chat::error::decode_body(None, frame.data.as_bytes()),
                self.error_detail,
            )
            .with_origin(self.origin.clone());
            self.inert = true;
            step.error = Some(err);
            return;
        }
        let plain =
            frame.event.is_none() && !(frame.has_non_plain_framing && frame.data == "[DONE]");
        if !plain || self.done || frame.data.is_empty() {
            self.release_held(false, out);
            out.extend_from_slice(raw);
            step.events = step.events.saturating_add(1);
            return;
        }
        if frame.data == "[DONE]" {
            if !self.finished {
                self.inert = true;
                step.error = Some(
                    KppError::provider(
                        "The provider's OpenAI Chat Completions stream ended before the response was complete.",
                    )
                    .with_origin(self.origin.clone()),
                );
                return;
            }
            if let Some(u) = self.running_usage.take() {
                self.usage = u;
                self.tally.final_usage = true;
            }
            self.done = true;
            self.complete = true;
            step.ended = true;
            step.events = step.events.saturating_add(1);
            self.release_held(false, out);
            if let Some(chunk) = self.usage_chunk.take() {
                out.extend_from_slice(&chunk);
            }
            out.extend_from_slice(raw);
            return;
        }

        #[derive(Deserialize)]
        struct ChatTap<'a> {
            #[serde(default, borrow)]
            id: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            model: Option<Cow<'a, str>>,
            #[serde(default)]
            object: Option<&'a str>,
            #[serde(default, borrow)]
            choices: Option<Vec<ChoiceTap<'a>>>,
            #[serde(default, borrow, deserialize_with = "some_raw")]
            usage: Option<&'a RawValue>,
            #[serde(default, borrow)]
            error: Option<&'a RawValue>,
        }

        #[derive(Deserialize)]
        struct ChoiceTap<'a> {
            #[serde(default)]
            index: u64,
            #[serde(default, borrow)]
            delta: Option<DeltaTap<'a>>,
            #[serde(default, borrow)]
            finish_reason: Option<Cow<'a, str>>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TextOrOther<'a> {
            Text(Cow<'a, str>),
            Other(serde::de::IgnoredAny),
        }

        #[derive(Deserialize)]
        struct DeltaTap<'a> {
            #[serde(default, borrow)]
            content: Option<TextOrOther<'a>>,
            #[serde(default, borrow)]
            refusal: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            reasoning: Option<TextOrOther<'a>>,
            #[serde(default, borrow)]
            reasoning_content: Option<TextOrOther<'a>>,
            #[serde(default, borrow)]
            tool_calls: Option<Vec<ToolCallTap<'a>>>,
            #[serde(default, borrow)]
            audio: Option<&'a RawValue>,
        }

        #[derive(Deserialize)]
        struct ToolCallTap<'a> {
            #[serde(default, borrow)]
            function: Option<FunctionTap<'a>>,
        }

        #[derive(Deserialize)]
        struct FunctionTap<'a> {
            #[serde(default, borrow)]
            arguments: Option<Cow<'a, str>>,
        }

        let tap: ChatTap<'_> = match serde_json::from_str(&frame.data) {
            Ok(t) => t,
            Err(e) => {
                self.release_before_error(out);
                self.inert = true;
                step.error = Some(
                    KppError::provider(format!(
                        "The provider sent a streamed chunk that is not JSON: {}.",
                        without_position(&e)
                    ))
                    .with_origin(self.origin.clone()),
                );
                return;
            }
        };

        if tap.error.is_some() || (tap.object == Some("error") && tap.choices.is_none()) {
            let err = crate::openai_chat::error::with_detail(
                crate::openai_chat::error::decode_body(None, frame.data.as_bytes()),
                self.error_detail,
            )
            .with_origin(self.origin.clone());
            self.release_before_error(out);
            self.inert = true;
            step.error = Some(err);
            return;
        }

        self.started = true;
        if self.id.is_empty()
            && let Some(id) = tap.id
            && !id.is_empty()
        {
            self.id = id.into_owned();
        }
        if self.model.is_empty()
            && let Some(model) = tap.model
            && !model.is_empty()
        {
            self.model = model.into_owned();
        }

        let choices_empty = match &tap.choices {
            Some(choices) if choices.len() > 1 => {
                self.release_before_error(out);
                self.inert = true;
                step.error = Some(
                    KppError::unsupported(
                        "n",
                        "The provider streamed several choices, but Kaeso carries one; leave `n` out or set it to 1.",
                    )
                    .with_origin(self.origin.clone()),
                );
                return;
            }
            Some(choices) if choices.is_empty() => true,
            Some(choices) => {
                if let Some(ch) = choices.first() {
                    if ch.index != 0 {
                        self.release_before_error(out);
                        self.inert = true;
                        step.error = Some(
                            KppError::unsupported(
                                "n",
                                "The provider streamed a choice other than the first; leave `n` out or set it to 1.",
                            )
                            .with_origin(self.origin.clone()),
                        );
                        return;
                    }
                    if ch.finish_reason.as_deref() == Some("error") {
                        let raw_val =
                            serde_json::from_str::<Value>(&frame.data).unwrap_or(Value::Null);
                        self.release_before_error(out);
                        self.inert = true;
                        step.error = Some(
                            KppError::provider(
                                "The provider ended the stream with `finish_reason: \"error\"` and no details.",
                            )
                            .with_origin(self.origin.clone())
                            .with_raw(raw_val),
                        );
                        return;
                    }
                }
                false
            }
            None => false,
        };

        let mut decoded_usage = None;
        if let Some(usage_raw) = tap.usage
            && usage_raw.get() != "null"
        {
            let raw_json = RawJson::from(usage_raw);
            match crate::openai_chat::response::decode_usage(&raw_json, &self.origin) {
                Ok(decoded) => decoded_usage = Some(decoded),
                Err(err) => {
                    self.release_before_error(out);
                    self.inert = true;
                    step.error = Some(err.with_origin(self.origin.clone()));
                    return;
                }
            }
        }

        if let Some(ch) = tap.choices.as_deref().and_then(|c| c.first()) {
            if let Some(delta) = &ch.delta {
                for r in [&delta.reasoning, &delta.reasoning_content]
                    .into_iter()
                    .flatten()
                {
                    match r {
                        TextOrOther::Text(text) if !text.is_empty() => {
                            self.has_reasoning = true;
                            step.saw_output = true;
                            self.tally.add(text.len(), true);
                        }
                        TextOrOther::Text(_) if self.has_reasoning => {
                            step.saw_output = true;
                        }
                        TextOrOther::Text(_) => {}
                        TextOrOther::Other(_) => {
                            self.has_reasoning = true;
                            step.saw_output = true;
                        }
                    }
                }
                if let Some(content) = &delta.content {
                    match content {
                        TextOrOther::Text(text)
                            if text.is_empty()
                                && !self.has_text
                                && (self.reasons_first || self.finished) => {}
                        TextOrOther::Text(text) => {
                            self.has_text = true;
                            step.saw_output = true;
                            if !text.is_empty() {
                                self.tally.add(text.len(), false);
                            }
                        }
                        TextOrOther::Other(_) => {
                            step.saw_output = true;
                        }
                    }
                }
                if let Some(refusal) = &delta.refusal {
                    step.saw_output = true;
                    if !refusal.is_empty() {
                        self.tally.add(refusal.len(), false);
                    }
                }
                if let Some(tcs) = &delta.tool_calls
                    && !tcs.is_empty()
                {
                    step.saw_output = true;
                    for tc in tcs {
                        if let Some(args) = tc.function.as_ref().and_then(|f| f.arguments.as_ref())
                            && !args.is_empty()
                        {
                            self.tally.add(args.len(), false);
                        }
                    }
                }
                if delta.audio.is_some() {
                    step.saw_output = true;
                }
            }
            if let Some(finish) = ch.finish_reason.as_deref()
                && !finish.is_empty()
            {
                self.finished = true;
                self.stop = crate::openai_chat::response::decode_finish(finish);
                step.saw_output = true;
            }
        }

        let has_non_null_usage = decoded_usage.is_some();
        if let Some(decoded) = decoded_usage {
            if self.finished {
                self.running_usage = None;
                self.usage = decoded;
                self.tally.final_usage = true;
            } else {
                self.running_usage = Some(decoded);
            }
        }

        step.events = step.events.saturating_add(1);
        if !self.include_usage && tap.usage.is_some() {
            // Kaeso asked the provider for usage so it can bill the stream; hide the injected
            // usage chunk or field from an app that did not set `include_usage`.
            if has_non_null_usage && choices_empty {
                return;
            }
            if write_usage(&frame.data, raw, frame.has_non_plain_framing, false, out) {
                return;
            }
        }
        let provider_usage_chunk = has_non_null_usage && choices_empty;
        self.release_held(provider_usage_chunk, out);
        // OpenAI sends the usage in a chunk of its own with empty `choices`
        // (`stream_options.include_usage`); DeepSeek and Mistral put it on the chunk that
        // finishes. That chunk waits for the next one: if it is the provider's own usage chunk,
        // both go as sent, otherwise the usage moves into a chunk of its own before `[DONE]`.
        if self.include_usage && has_non_null_usage && !choices_empty && self.finished {
            self.held = Some(Held {
                data: frame.data.to_string(),
                raw: raw.to_vec(),
                has_non_plain_framing: frame.has_non_plain_framing,
            });
            return;
        }
        if provider_usage_chunk {
            self.usage_chunk = None;
        }
        out.extend_from_slice(raw);
    }

    // An error ends the stream: what the app would have got before it, the held chunk as sent or
    // the usage a comment already moved out of it, goes first.
    fn release_before_error(&mut self, out: &mut Vec<u8>) {
        self.release_held(true, out);
        if let Some(chunk) = self.usage_chunk.take() {
            out.extend_from_slice(&chunk);
        }
    }

    fn release_held(&mut self, as_sent: bool, out: &mut Vec<u8>) {
        let Some(held) = self.held.take() else {
            return;
        };
        if !as_sent
            && let Some(chunk) = usage_chunk(&held.data)
            && write_usage(&held.data, &held.raw, held.has_non_plain_framing, true, out)
        {
            self.usage_chunk = Some(chunk);
            return;
        }
        // The chunk goes with its own usage, newer than any an earlier chunk left for the end.
        self.usage_chunk = None;
        out.extend_from_slice(&held.raw);
    }
}

struct Held {
    data: String,
    raw: Vec<u8>,
    has_non_plain_framing: bool,
}

// The chunk's `usage` removed, or set to `null` with `null_usage`.
fn write_usage(
    data: &str,
    raw: &[u8],
    has_non_plain_framing: bool,
    null_usage: bool,
    out: &mut Vec<u8>,
) -> bool {
    let Ok(mut map) = serde_json::from_str::<IndexMap<String, &RawValue>>(data) else {
        return false;
    };
    let found = if null_usage {
        match (
            map.get_mut("usage"),
            serde_json::from_str::<&RawValue>("null"),
        ) {
            (Some(usage), Ok(null)) => {
                *usage = null;
                true
            }
            _ => false,
        }
    } else {
        map.shift_remove("usage").is_some()
    };
    if !found {
        return false;
    }
    let start = out.len();
    if has_non_plain_framing {
        // Preserve non-data SSE fields (id, retry, etc.) by rewriting only the `data:` line.
        let Ok(text) = std::str::from_utf8(raw) else {
            return false;
        };
        let mut wrote_data = false;
        for (line, eol) in split_sse_lines(text) {
            if line == "data" || line.starts_with("data:") {
                if !wrote_data {
                    let Ok(json) = serde_json::to_string(&map) else {
                        out.truncate(start);
                        return false;
                    };
                    write_data_lines(&json, if eol.is_empty() { "\n" } else { eol }, out);
                    wrote_data = true;
                }
            } else {
                out.extend_from_slice(line.as_bytes());
                out.extend_from_slice(eol.as_bytes());
            }
        }
        if !wrote_data {
            out.truncate(start);
        }
        return wrote_data;
    }
    let Ok(json) = serde_json::to_string(&map) else {
        out.truncate(start);
        return false;
    };
    write_data_lines(&json, "\n", out);
    out.extend_from_slice(b"\n");
    true
}

// A raw value the provider split across `data:` lines keeps its line breaks; each line needs a
// `data:` of its own again, or the client reads the rest as other fields.
fn write_data_lines(json: &str, eol: &str, out: &mut Vec<u8>) {
    for line in json.split('\n') {
        out.extend_from_slice(b"data: ");
        out.extend_from_slice(line.strip_suffix('\r').unwrap_or(line).as_bytes());
        out.extend_from_slice(eol.as_bytes());
    }
}

// The usage chunk OpenAI sends last: the chunk's other keys, `choices: []` and its `usage`.
fn usage_chunk(data: &str) -> Option<Vec<u8>> {
    let mut map = serde_json::from_str::<IndexMap<String, &RawValue>>(data).ok()?;
    let empty: &RawValue = serde_json::from_str("[]").ok()?;
    *map.get_mut("choices")? = empty;
    let mut out = Vec::new();
    write_data_lines(&serde_json::to_string(&map).ok()?, "\n", &mut out);
    out.extend_from_slice(b"\n");
    Some(out)
}

impl FrameState for ChatState {
    fn origin(&self) -> &Origin {
        &self.origin
    }

    fn is_inert(&self) -> bool {
        self.inert
    }

    fn set_inert(&mut self) {
        self.inert = true;
    }

    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        ChatState::process_frame(self, raw, out, step);
    }
}

impl StreamPassthrough for ChatPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        feed_framer(&mut self.framer, &mut self.state, chunk, out)
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let (mut step, early) = finish_framer(&mut self.framer, &mut self.state, out);
        if early {
            return step;
        }
        if self.state.inert || self.state.done {
            self.state.inert = true;
            return step;
        }
        if self.state.finished {
            if let Some(u) = self.state.running_usage.take() {
                self.state.usage = u;
                self.state.tally.final_usage = true;
            }
            self.state.release_held(false, out);
            if let Some(chunk) = self.state.usage_chunk.take() {
                out.extend_from_slice(&chunk);
            }
            self.state.complete = true;
            step.ended = true;
            self.state.inert = true;
        } else {
            self.state.inert = true;
            step.error = Some(
                KppError::provider(
                    "The provider's OpenAI Chat Completions stream ended before the response was complete.",
                )
                .with_origin(self.state.origin.clone()),
            );
        }
        step
    }

    fn flush_eof_error(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        self.state.release_before_error(out);
        flush_eof_framer(&mut self.framer, &mut self.state, out)
    }

    fn encode_error(&mut self, err: &KppError) -> Vec<u8> {
        let body = crate::openai_chat::error::error_body(err);
        let mut out = Vec::new();
        self.state.release_before_error(&mut out);
        out.extend_from_slice(b"data: ");
        let _ = serde_json::to_writer(&mut out, &body);
        out.extend_from_slice(b"\n\n");
        out
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.state.done || self.state.inert {
            Vec::new()
        } else {
            keepalive::COMMENT_FRAME.to_vec()
        }
    }

    fn summary(&self) -> PassthroughSummary {
        let mut usage = self.state.usage.clone();
        let mut tally = self.state.tally;
        if !tally.final_usage
            && let Some(u) = &self.state.running_usage
        {
            usage = u.clone();
            tally.final_usage = true;
        }
        PassthroughSummary {
            id: self.state.id.clone(),
            model: self.state.model.clone(),
            usage,
            stop: self.state.stop.clone(),
            complete: self.state.complete,
            started: self.state.started,
            tally,
        }
    }
}

// ---------- OpenAI Responses passthrough ----------

struct ResponsesPassthrough {
    framer: RawSseFramer,
    state: ResponsesState,
}

struct ResponsesState {
    origin: Origin,
    id: String,
    model: String,
    usage: Usage,
    stop: StopReason,
    called: bool,
    started: bool,
    done: bool,
    complete: bool,
    inert: bool,
    held_error: Option<KppError>,
    tally: PassthroughTally,
}

impl sealed::Sealed for ResponsesPassthrough {}

impl ResponsesPassthrough {
    fn new(ctx: &EgressCtx) -> Self {
        ResponsesPassthrough {
            framer: RawSseFramer::new(),
            state: ResponsesState {
                origin: ctx.origin(DialectId::OpenAiResponses),
                id: String::new(),
                model: String::new(),
                usage: Usage::default(),
                stop: StopReason::EndTurn,
                called: false,
                started: false,
                done: false,
                complete: false,
                inert: false,
                held_error: None,
                tally: PassthroughTally::default(),
            },
        }
    }
}

impl ResponsesState {
    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        if self.inert {
            return;
        }
        let frame = parse_frame(raw);
        if frame.comment_only || !frame.has_data || frame.data.trim().is_empty() {
            out.extend_from_slice(raw);
            step.events = step.events.saturating_add(1);
            return;
        }

        #[derive(Deserialize)]
        struct ResponsesTap<'a> {
            #[serde(default, rename = "type")]
            ty: Option<&'a str>,
            #[serde(default, borrow)]
            delta: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            response: Option<&'a RawValue>,
            #[serde(default, borrow)]
            item: Option<&'a RawValue>,
        }

        let tap: ResponsesTap<'_> = match serde_json::from_str(&frame.data) {
            Ok(t) => t,
            Err(_) => {
                self.inert = true;
                step.error = Some(
                    KppError::provider(format!(
                        "The provider sent a stream event that is not valid JSON: {}",
                        Short(&frame.data, 200)
                    ))
                    .with_origin(self.origin.clone()),
                );
                return;
            }
        };

        if let Some(mut held) = self.held_error.take() {
            if tap.ty == Some("response.failed")
                && let Ok(val) = serde_json::from_str::<Value>(&frame.data)
                && let Some(Value::Object(_)) = &held.raw
            {
                let first_raw = held.raw.take().unwrap_or(Value::Null);
                held.raw = Some(Value::Array(vec![first_raw, val]));
            }
            self.inert = true;
            step.error = Some(held);
            return;
        }

        match tap.ty {
            Some("response.created") => {
                if let Some(resp_raw) = tap.response
                    && let Ok(w) = serde_json::from_str::<
                        crate::openai_responses::wire::ResponseObject<'_>,
                    >(resp_raw.get())
                {
                    self.started = true;
                    if self.id.is_empty() && !w.id.is_empty() {
                        self.id = w.id;
                    }
                    if self.model.is_empty() && !w.model.is_empty() {
                        self.model = w.model;
                    }
                    if let Some(u_raw) = w.usage
                        && let Ok(u) =
                            crate::openai_responses::response::decode_usage(u_raw, &self.origin)
                    {
                        self.usage = u;
                    }
                }
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("response.in_progress") => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("error") => {
                let value: Value = serde_json::from_str(&frame.data).unwrap_or(Value::Null);
                let fields = match value.get("error") {
                    Some(nested @ Value::Object(_)) => nested,
                    _ => &value,
                };
                let field = |key: &str| fields.get(key).and_then(Value::as_str);
                let kind = crate::openai_responses::error::kind_for(
                    None,
                    field("type").filter(|t| *t != "error"),
                    field("code"),
                );
                let message = field("message")
                    .filter(|m| !m.is_empty())
                    .unwrap_or("The provider reported an error in the middle of the stream.");
                let mut err = KppError::new(kind, message)
                    .with_origin(self.origin.clone())
                    .with_raw(value.clone());
                if let Some(p) = field("param") {
                    err = err.with_param(p);
                }
                self.held_error = Some(err);
            }
            Some("response.failed") => {
                let value: Value = serde_json::from_str(&frame.data).unwrap_or(Value::Null);
                let err = match tap.response.and_then(|r| {
                    serde_json::from_str::<crate::openai_responses::wire::ResponseObject<'_>>(
                        r.get(),
                    )
                    .ok()
                }) {
                    Some(w) => {
                        crate::openai_responses::response::failed_error(&w, value, &self.origin)
                    }
                    None => KppError::provider("The provider reported that the response failed.")
                        .with_origin(self.origin.clone())
                        .with_raw(value),
                };
                self.inert = true;
                step.error = Some(err);
            }
            Some("response.completed" | "response.incomplete") => {
                let Some(w) = tap.response.and_then(|r| {
                    serde_json::from_str::<crate::openai_responses::wire::ResponseObject<'_>>(
                        r.get(),
                    )
                    .ok()
                }) else {
                    self.inert = true;
                    step.error = Some(
                        KppError::provider(
                            "The provider sent a response terminal event without a valid response.",
                        )
                        .with_origin(self.origin.clone()),
                    );
                    return;
                };
                self.started = true;
                if self.id.is_empty() && !w.id.is_empty() {
                    self.id = w.id;
                }
                if self.model.is_empty() && !w.model.is_empty() {
                    self.model = w.model;
                }
                let has_calls = self.called
                    || w.output
                        .iter()
                        .flatten()
                        .any(|i| type_tag(i).ok().flatten().as_deref() == Some("function_call"));
                self.stop = crate::openai_responses::response::stop_for(
                    w.status.as_deref(),
                    w.incomplete_details
                        .as_ref()
                        .and_then(|d| d.reason.as_deref()),
                    has_calls,
                );
                if let Some(u_raw) = w.usage {
                    match crate::openai_responses::response::decode_usage(u_raw, &self.origin) {
                        Ok(mut u) => {
                            let searches = crate::web_search::billed_searches(
                                w.output.iter().flatten().copied(),
                                &self.origin,
                            );
                            if searches > 0 {
                                u.web_searches = Some(searches);
                            }
                            self.usage = u;
                            self.tally.final_usage = true;
                        }
                        Err(err) => {
                            self.inert = true;
                            step.error = Some(err.with_origin(self.origin.clone()));
                            return;
                        }
                    }
                }
                self.done = true;
                self.complete = true;
                self.inert = true;
                step.saw_output = true;
                step.ended = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(
                "response.output_text.delta"
                | "response.refusal.delta"
                | "response.function_call_arguments.delta",
            ) => {
                if let Some(delta) = tap.delta {
                    self.tally.add(delta.len(), false);
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(
                "response.reasoning.delta"
                | "response.reasoning_text.delta"
                | "response.reasoning_summary_text.delta",
            ) => {
                if let Some(delta) = tap.delta {
                    self.tally.add(delta.len(), true);
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("response.output_item.added") => {
                let item_ty = tap.item.and_then(|i| type_tag(i).ok().flatten());
                if item_ty.as_deref() == Some("function_call") {
                    self.called = true;
                }
                if item_ty.as_deref() != Some("message") {
                    step.saw_output = true;
                }
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(ty) if is_responses_output_event(ty) => {
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(_) | None => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
        }
    }
}

fn is_responses_output_event(ty: &str) -> bool {
    matches!(
        ty,
        "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.done"
            | "response.output_text.annotation.added"
            | "response.refusal.done"
            | "response.function_call_arguments.done"
            | "response.reasoning.done"
            | "response.reasoning_text.done"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_part.done"
            | "response.reasoning_summary_text.done"
            | "response.output_item.done"
    ) || ty.starts_with("response.image_generation_call.")
        || ty.starts_with("response.web_search_call.")
        || ty.starts_with("response.file_search_call.")
        || ty.starts_with("response.code_interpreter_call.")
        || ty.starts_with("response.mcp_call.")
        || ty.starts_with("response.custom_tool_call.")
}

impl FrameState for ResponsesState {
    fn origin(&self) -> &Origin {
        &self.origin
    }

    fn is_inert(&self) -> bool {
        self.inert
    }

    fn set_inert(&mut self) {
        self.inert = true;
    }

    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        ResponsesState::process_frame(self, raw, out, step);
    }
}

impl StreamPassthrough for ResponsesPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        feed_framer(&mut self.framer, &mut self.state, chunk, out)
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let (mut step, early) = finish_framer(&mut self.framer, &mut self.state, out);
        if early {
            if step.error.is_none()
                && let Some(err) = self.state.held_error.take()
            {
                step.error = Some(err);
            }
            return step;
        }
        if let Some(err) = self.state.held_error.take() {
            self.state.inert = true;
            step.error = Some(err);
            return step;
        }
        self.state.inert = true;
        step
    }

    fn flush_eof_error(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = flush_eof_framer(&mut self.framer, &mut self.state, out);
        if step.error.is_none()
            && let Some(err) = self.state.held_error.take()
        {
            self.state.inert = true;
            step.error = Some(err);
        }
        step
    }

    fn encode_error(&mut self, err: &KppError) -> Vec<u8> {
        let is_event = |v: &Value| v.get("type").and_then(Value::as_str).is_some();
        let mine = err
            .origin
            .as_ref()
            .is_some_and(|o| o.is(DialectId::OpenAiResponses));
        let mut out = Vec::new();
        let write_ev = |out: &mut Vec<u8>, v: &Value| {
            let ty = v.get("type").and_then(Value::as_str).unwrap_or("error");
            out.extend_from_slice(b"event: ");
            out.extend_from_slice(ty.as_bytes());
            out.extend_from_slice(b"\ndata: ");
            let _ = serde_json::to_writer(&mut *out, v);
            out.extend_from_slice(b"\n\n");
        };
        match &err.raw {
            Some(v) if mine && is_event(v) => write_ev(&mut out, v),
            Some(Value::Array(list)) if mine && !list.is_empty() && list.iter().all(is_event) => {
                for v in list {
                    write_ev(&mut out, v);
                }
            }
            _ => {
                let body = crate::openai_responses::error::stream_error_event(err);
                write_ev(&mut out, &body);
            }
        }
        out
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.state.done || self.state.inert {
            Vec::new()
        } else {
            keepalive::COMMENT_FRAME.to_vec()
        }
    }

    fn summary(&self) -> PassthroughSummary {
        PassthroughSummary {
            id: self.state.id.clone(),
            model: self.state.model.clone(),
            usage: self.state.usage.clone(),
            stop: self.state.stop.clone(),
            complete: self.state.complete,
            started: self.state.started,
            tally: self.state.tally,
        }
    }
}

// ---------- Anthropic Messages passthrough ----------

struct AnthropicPassthrough {
    framer: RawSseFramer,
    state: AnthropicState,
}

struct AnthropicState {
    origin: Origin,
    id: String,
    model: String,
    start_usage: Option<Usage>,
    usage: Usage,
    stop: StopReason,
    started: bool,
    done: bool,
    complete: bool,
    inert: bool,
    tally: PassthroughTally,
}

impl sealed::Sealed for AnthropicPassthrough {}

impl AnthropicPassthrough {
    fn new(ctx: &EgressCtx) -> Self {
        AnthropicPassthrough {
            framer: RawSseFramer::new(),
            state: AnthropicState {
                origin: ctx.origin(DialectId::AnthropicMessages),
                id: String::new(),
                model: String::new(),
                start_usage: None,
                usage: Usage::default(),
                stop: StopReason::EndTurn,
                started: false,
                done: false,
                complete: false,
                inert: false,
                tally: PassthroughTally::default(),
            },
        }
    }
}

impl AnthropicState {
    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        if self.inert {
            return;
        }
        let frame = parse_frame(raw);
        if frame.comment_only || !frame.has_data || frame.data.trim().is_empty() {
            out.extend_from_slice(raw);
            step.events = step.events.saturating_add(1);
            return;
        }
        if frame.event == Some("error") {
            let err = crate::anthropic_messages::error::decode_stream_error(&frame.data)
                .with_origin(self.origin.clone());
            self.inert = true;
            step.error = Some(err);
            return;
        }

        #[derive(Deserialize)]
        struct AnthropicTap<'a> {
            #[serde(default, rename = "type")]
            ty: Option<&'a str>,
            #[serde(default, borrow)]
            message: Option<MessageStartTap<'a>>,
            #[serde(default, borrow)]
            delta: Option<AnthropicDeltaTap<'a>>,
            #[serde(default, borrow)]
            usage: Option<&'a RawValue>,
        }

        #[derive(Deserialize)]
        struct MessageStartTap<'a> {
            #[serde(default, borrow)]
            id: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            model: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            usage: Option<&'a RawValue>,
        }

        #[derive(Deserialize)]
        struct AnthropicDeltaTap<'a> {
            #[serde(default, rename = "type")]
            ty: Option<&'a str>,
            #[serde(default, borrow)]
            text: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            partial_json: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            thinking: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            stop_reason: Option<Cow<'a, str>>,
            #[serde(default, borrow)]
            stop_sequence: Option<Cow<'a, str>>,
        }

        let tap: AnthropicTap<'_> = match serde_json::from_str(&frame.data) {
            Ok(t) => t,
            Err(e) => {
                self.inert = true;
                step.error = Some(
                    KppError::provider(format!(
                        "The provider sent an Anthropic Messages stream Kaeso cannot read: {}.",
                        without_position(&e)
                    ))
                    .with_origin(self.origin.clone()),
                );
                return;
            }
        };

        match tap.ty {
            Some("error") => {
                let err = crate::anthropic_messages::error::decode_stream_error(&frame.data)
                    .with_origin(self.origin.clone());
                self.inert = true;
                step.error = Some(err);
            }
            Some("ping") => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("message_start") => {
                self.started = true;
                if let Some(msg) = tap.message {
                    if let Some(id) = msg.id
                        && !id.is_empty()
                    {
                        self.id = id.into_owned();
                    }
                    if let Some(model) = msg.model
                        && !model.is_empty()
                    {
                        self.model = model.into_owned();
                    }
                    if let Some(u_raw) = msg.usage
                        && let Ok(u) =
                            crate::anthropic_messages::response::decode_usage(u_raw, &self.origin)
                    {
                        self.start_usage = Some(u.clone());
                        self.usage = u;
                    }
                }
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("content_block_delta") => {
                if let Some(d) = tap.delta {
                    match d.ty {
                        Some("text_delta") => {
                            if let Some(t) = d.text {
                                self.tally.add(t.len(), false);
                            }
                        }
                        Some("input_json_delta") => {
                            if let Some(j) = d.partial_json {
                                self.tally.add(j.len(), false);
                            }
                        }
                        Some("thinking_delta") => {
                            if let Some(t) = d.thinking {
                                self.tally.add(t.len(), true);
                            }
                        }
                        _ => {}
                    }
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("message_delta") => {
                if let Some(d) = tap.delta
                    && let Some(reason) = d.stop_reason.as_deref()
                {
                    self.stop = crate::anthropic_messages::common::stop_from_wire(
                        reason,
                        d.stop_sequence.as_deref(),
                    );
                }
                if let Some(u_raw) = tap.usage
                    && let Ok(u) = crate::anthropic_messages::response::decode_usage_with(
                        u_raw,
                        &self.origin,
                        self.start_usage.as_ref(),
                    )
                {
                    self.usage = u;
                    self.tally.final_usage = true;
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("message_stop") => {
                self.done = true;
                self.complete = true;
                self.inert = true;
                step.saw_output = true;
                step.ended = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("content_block_start" | "content_block_stop") => {
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(_) | None => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
        }
    }
}

impl FrameState for AnthropicState {
    fn origin(&self) -> &Origin {
        &self.origin
    }

    fn is_inert(&self) -> bool {
        self.inert
    }

    fn set_inert(&mut self) {
        self.inert = true;
    }

    fn process_frame(&mut self, raw: &[u8], out: &mut Vec<u8>, step: &mut PassthroughStep) {
        AnthropicState::process_frame(self, raw, out, step);
    }
}

impl StreamPassthrough for AnthropicPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        feed_framer(&mut self.framer, &mut self.state, chunk, out)
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let (mut step, early) = finish_framer(&mut self.framer, &mut self.state, out);
        if early {
            return step;
        }
        if !self.state.done {
            self.state.inert = true;
            step.error = Some(
                KppError::provider(
                    "The provider sent an Anthropic Messages stream Kaeso cannot read: the stream ended before `message_stop`.",
                )
                .with_origin(self.state.origin.clone()),
            );
        }
        self.state.inert = true;
        step
    }

    fn flush_eof_error(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        flush_eof_framer(&mut self.framer, &mut self.state, out)
    }

    fn encode_error(&mut self, err: &KppError) -> Vec<u8> {
        let data = crate::anthropic_messages::error::stream_error_data(err);
        let mut out = Vec::with_capacity(data.len() + 24);
        out.extend_from_slice(b"event: error\ndata: ");
        out.extend_from_slice(data.as_bytes());
        out.extend_from_slice(b"\n\n");
        out
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.state.done || self.state.inert {
            Vec::new()
        } else {
            keepalive::PING_FRAME.to_vec()
        }
    }

    fn summary(&self) -> PassthroughSummary {
        PassthroughSummary {
            id: self.state.id.clone(),
            model: self.state.model.clone(),
            usage: self.state.usage.clone(),
            stop: self.state.stop.clone(),
            complete: self.state.complete,
            started: self.state.started,
            tally: self.state.tally,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch;

    fn fixture(rel: &str) -> Vec<u8> {
        std::fs::read(format!("{}/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn feed_chunked(
        pass: &mut dyn StreamPassthrough,
        bytes: &[u8],
        step: usize,
    ) -> (Vec<u8>, Vec<KppError>, bool) {
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let mut ended = false;
        for chunk in bytes.chunks(step) {
            let s = pass.feed(chunk, &mut out);
            ended |= s.ended;
            if let Some(err) = s.error {
                errors.push(err);
                return (out, errors, ended);
            }
        }
        let s = pass.finish(&mut out);
        ended |= s.ended;
        if let Some(err) = s.error {
            errors.push(err);
        }
        (out, errors, ended)
    }

    fn usage_chunks(out: &[u8]) -> (Vec<Value>, Vec<Value>) {
        let chunks: Vec<Value> = crate::sse::parse_all(out)
            .unwrap()
            .into_iter()
            .filter_map(|i| match i {
                crate::sse::SseItem::Event(e) if e.data != "[DONE]" => {
                    serde_json::from_str(&e.data).ok()
                }
                _ => None,
            })
            .collect();
        // Running usage on chunks before the finish stays as the provider sent it.
        let (own, inline): (Vec<Value>, Vec<Value>) = chunks
            .into_iter()
            .filter(|c| !c["usage"].is_null())
            .partition(|c| c["choices"] == serde_json::json!([]));
        let finishing = inline
            .into_iter()
            .filter(|c| c["choices"][0]["finish_reason"].is_string())
            .collect();
        (own, finishing)
    }

    #[test]
    fn chat_streams_with_include_usage_end_with_a_usage_chunk_of_their_own() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
        for (name, usage) in [
            ("deepseek", 25),
            ("mistral", 35),
            ("standardcompute", -1),
            ("cloudflare-ai-gateway", 24),
        ] {
            let bytes = fixture(&format!(
                "openai_chat/providers/{name}/streams/recorded-stream-include-usage.sse"
            ));
            let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
            let (out, errs, ended) = feed_chunked(pass.as_mut(), &bytes, 37);
            assert!(errs.is_empty() && ended, "{name}: {errs:?}");
            let (own, inline) = usage_chunks(&out);
            assert!(inline.is_empty(), "{name}: {inline:?}");
            assert_eq!(own.len(), 1, "{name}");
            if usage >= 0 {
                assert_eq!(own[0]["usage"]["total_tokens"], usage, "{name}");
            }
            let text = String::from_utf8(out).unwrap();
            assert!(text.ends_with("data: [DONE]\n\n"), "{name}: {text}");
            let (text_in, _) = usage_chunks(&bytes);
            assert!(text_in.is_empty(), "{name} has no usage chunk of its own");
            let said = |b: &[u8]| -> String {
                crate::sse::parse_all(b)
                    .unwrap()
                    .into_iter()
                    .filter_map(|i| match i {
                        crate::sse::SseItem::Event(e) => {
                            serde_json::from_str::<Value>(&e.data).ok()
                        }
                        _ => None,
                    })
                    .filter_map(|c| {
                        c["choices"][0]["delta"]["content"]
                            .as_str()
                            .map(str::to_owned)
                    })
                    .collect()
            };
            assert_eq!(said(text.as_bytes()), said(&bytes), "{name}");
        }
        for rel in [
            "openai_chat/providers/groq/streams/recorded-stream-include-usage.sse",
            "openai_chat/providers/xai/streams/recorded-stream-include-usage.sse",
            "openai_chat/streams/usage-every-chunk.sse",
            "openai_chat/streams/stream-include-usage.sse",
        ] {
            let bytes = fixture(rel);
            let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
            let (out, errs, ended) = feed_chunked(pass.as_mut(), &bytes, 37);
            assert!(errs.is_empty() && ended, "{rel}: {errs:?}");
            assert_eq!(out, bytes, "{rel} goes as sent");
        }
    }

    #[test]
    fn a_held_finish_chunk_survives_split_data_errors_and_comments() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
        let head = "data: {\"id\":\"c\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n";
        let finish = "data: {\"id\":\"c\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n";
        let run = |bytes: &str| {
            let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
            let (out, errors, _) = feed_chunked(pass.as_mut(), bytes.as_bytes(), 9);
            (pass, String::from_utf8(out).unwrap(), errors)
        };
        let data = |out: &str| -> Vec<Value> {
            crate::sse::parse_all(out.as_bytes())
                .unwrap()
                .into_iter()
                .filter_map(|i| match i {
                    crate::sse::SseItem::Event(e) if e.data != "[DONE]" => {
                        Some(serde_json::from_str(&e.data).unwrap())
                    }
                    _ => None,
                })
                .collect()
        };

        // A usage object the provider split across `data:` lines.
        let split = finish.replace("\"prompt_tokens\":5,", "\"prompt_tokens\":5,\ndata: ");
        let (_, out, errors) = run(&format!("{head}{split}data: [DONE]\n\n"));
        assert!(errors.is_empty(), "{errors:?}");
        let chunks = data(&out);
        assert_eq!(chunks.len(), 3, "{out}");
        assert_eq!(chunks[1]["usage"], Value::Null, "{out}");
        assert_eq!(chunks[1]["choices"][0]["delta"]["content"], "!", "{out}");
        assert_eq!(chunks[2]["choices"], serde_json::json!([]), "{out}");
        assert_eq!(chunks[2]["usage"]["total_tokens"], 7, "{out}");

        // A named error right after it: the chunk goes as the provider sent it.
        let (_, out, errors) = run(&format!(
            "{head}{finish}event: error\ndata: {{\"error\":{{\"message\":\"boom\"}}}}\n\n"
        ));
        assert_eq!(errors.len(), 1);
        assert!(out.ends_with(finish), "{out}");

        // A transport error: the gateway writes the error after the chunk.
        let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
        let mut out = Vec::new();
        assert!(
            pass.feed(format!("{head}{finish}").as_bytes(), &mut out)
                .error
                .is_none()
        );
        let error = pass.encode_error(&KppError::provider("The connection to the provider broke."));
        assert!(
            String::from_utf8(error).unwrap().starts_with(finish),
            "the held chunk comes first"
        );

        // A comment between it and the provider's own usage chunk.
        let own = "data: {\"id\":\"c\",\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n";
        let (_, out, errors) = run(&format!(
            "{head}{finish}: keepalive\n\n{own}data: [DONE]\n\n"
        ));
        assert!(errors.is_empty(), "{errors:?}");
        let chunks = data(&out);
        assert_eq!(chunks.len(), 3, "{out}");
        assert_eq!(chunks[1]["usage"], Value::Null, "{out}");
        assert_eq!(chunks[2]["usage"]["total_tokens"], 7, "{out}");
        assert!(out.contains(": keepalive\n\n"), "{out}");

        // A comment, then an error: the usage the comment moved out of the chunk comes first.
        let (_, out, errors) = run(&format!(
            "{head}{finish}: keepalive\n\nevent: error\ndata: {{\"error\":{{\"message\":\"boom\"}}}}\n\n"
        ));
        assert_eq!(errors.len(), 1);
        let chunks = data(&out);
        assert_eq!(chunks.len(), 3, "{out}");
        assert_eq!(chunks[2]["choices"], serde_json::json!([]), "{out}");
        assert_eq!(chunks[2]["usage"]["total_tokens"], 7, "{out}");
        let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
        let mut out = Vec::new();
        let fed = format!("{head}{finish}: keepalive\n\n");
        assert!(pass.feed(fed.as_bytes(), &mut out).error.is_none());
        let error =
            String::from_utf8(pass.encode_error(&KppError::provider("The connection broke.")))
                .unwrap();
        assert!(error.contains("\"choices\":[]"), "{error}");
        // Two finishing chunks with usage, then an error: only the newer counts reach the app.
        let older = finish.replace("\"total_tokens\":7", "\"total_tokens\":0");
        let (_, out, errors) = run(&format!(
            "{head}{older}{finish}event: error\ndata: {{\"error\":{{\"message\":\"boom\"}}}}\n\n"
        ));
        assert_eq!(errors.len(), 1);
        let totals: Vec<Value> = data(&out)
            .iter()
            .filter(|c| !c["usage"].is_null())
            .map(|c| c["usage"]["total_tokens"].clone())
            .collect();
        assert_eq!(totals, [serde_json::json!(7)], "{out}");
    }

    #[test]
    fn a_recorded_stream_cut_by_an_error_after_its_usage_keeps_the_newest_counts() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
        let bytes = fixture(
            "openai_chat/providers/cloudflare-ai-gateway/streams/recorded-stream-include-usage.sse",
        );
        let text = String::from_utf8(bytes).unwrap();
        let cut = text.strip_suffix("data: [DONE]\n\n").unwrap();
        let failed = format!("{cut}event: error\ndata: {{\"error\":{{\"message\":\"boom\"}}}}\n\n");
        let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
        let (out, errors, _) = feed_chunked(pass.as_mut(), failed.as_bytes(), 41);
        assert_eq!(errors.len(), 1);
        let chunks: Vec<Value> = crate::sse::parse_all(&out)
            .unwrap()
            .into_iter()
            .filter_map(|i| match i {
                crate::sse::SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
                _ => None,
            })
            .collect();
        let finishing: Vec<&Value> = chunks
            .iter()
            .filter(|c| {
                c["choices"][0]["finish_reason"].is_string()
                    || c["choices"] == serde_json::json!([])
            })
            .collect();
        let last = finishing.last().unwrap();
        assert_eq!(last["usage"]["total_tokens"], 24, "{last}");
        let with_usage = finishing.iter().filter(|c| !c["usage"].is_null()).count();
        assert_eq!(with_usage, 1, "{finishing:?}");
    }

    #[test]
    fn chat_passthrough_refuses_several_choices_as_kaesos_limit() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        for (chunk, message) in [
            (
                "data: {\"id\":\"c\",\"choices\":[{\"index\":0,\"delta\":{}},{\"index\":1,\"delta\":{}}]}\n\n",
                "The provider streamed several choices, but Kaeso carries one; leave `n` out or set it to 1.",
            ),
            (
                "data: {\"id\":\"c\",\"choices\":[{\"index\":1,\"delta\":{}}]}\n\n",
                "The provider streamed a choice other than the first; leave `n` out or set it to 1.",
            ),
        ] {
            let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc).unwrap();
            let (out, errors, _) = feed_chunked(pass.as_mut(), chunk.as_bytes(), 64);
            assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
            assert_eq!(errors.len(), 1, "{chunk}");
            assert_eq!(errors[0].unsupported_feature(), Some("n"));
            assert_eq!(errors[0].message, message);
        }
    }

    #[test]
    fn chat_passthrough_preserves_wire_and_hides_unasked_usage() {
        let raw = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello \"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"world\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n",
            "data: [DONE]\n\n"
        );
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));

        let enc_with = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
        let mut pass_with = for_stream(DialectId::OpenAiChat, &ectx, &enc_with).unwrap();
        let (out_with, errs, ended) = feed_chunked(pass_with.as_mut(), raw.as_bytes(), 11);
        assert!(errs.is_empty() && ended);
        let finish = "{\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}";
        let (before, _) = raw.split_once(finish).unwrap();
        let normalised = format!(
            "{before}{}\n\ndata: {}\n\ndata: [DONE]\n\n",
            finish.replace(
                "\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}",
                "\"usage\":null"
            ),
            "{\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}",
        );
        assert_eq!(
            String::from_utf8(out_with).unwrap(),
            normalised,
            "include_usage=true moves the finish chunk's usage into a chunk of its own"
        );
        let summary = pass_with.summary();
        assert_eq!(summary.id, "chatcmpl-1");
        assert_eq!(summary.model, "gpt-4o-mini");
        assert_eq!(summary.usage.input_tokens, 10);
        assert_eq!(summary.usage.output_tokens, 2);
        assert!(summary.complete && summary.tally.final_usage);
        assert_eq!(summary.tally.deltas, 2);
        assert_eq!(summary.tally.bytes, 11);

        let enc_without = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        let mut pass_without = for_stream(DialectId::OpenAiChat, &ectx, &enc_without).unwrap();
        let (out_without, errs, ended) = feed_chunked(pass_without.as_mut(), raw.as_bytes(), 13);
        assert!(errs.is_empty() && ended);
        let text = String::from_utf8(out_without).unwrap();
        assert!(
            !text.contains("\"usage\""),
            "usage hidden when unasked: {text}"
        );
        assert!(text.contains("\"finish_reason\":\"stop\"") && text.contains("data: [DONE]\n\n"));
        assert_eq!(pass_without.summary().usage.output_tokens, 2);

        // A separate usage-only `choices: []` chunk is dropped completely when unasked.
        let usage_only = concat!(
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}],\"usage\":null}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o-mini\",\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":1,\"total_tokens\":6}}\n\n",
            "data: [DONE]\n\n"
        );
        let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc_without).unwrap();
        let (out, errs, ended) = feed_chunked(pass.as_mut(), usage_only.as_bytes(), 19);
        assert!(errs.is_empty() && ended);
        let text = String::from_utf8(out).unwrap();
        assert!(
            !text.contains("\"usage\"") && !text.contains("\"choices\":[]"),
            "{text}"
        );
        assert_eq!(pass.summary().usage.input_tokens, 5);
        assert_eq!(pass.summary().usage.output_tokens, 1);
    }

    #[test]
    fn responses_passthrough_matches_fixtures_and_held_errors() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));

        for name in [
            "text-basic-stream",
            "recorded-text-basic-stream",
            "function-call-stream",
            "reasoning-summary-stream",
            "max-output-tokens-incomplete-stream",
        ] {
            let bytes = fixture(&format!("openai_responses/streams/{name}.sse"));
            let mut pass = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
            let (out, errs, ended) = feed_chunked(pass.as_mut(), &bytes, 23);
            assert!(errs.is_empty(), "{name}: {errs:?}");
            assert!(ended, "{name}");
            assert_eq!(out, bytes, "{name} must be byte-identical");
            let summary = pass.summary();
            assert!(summary.complete && summary.started, "{name}");
            assert!(
                !summary.id.is_empty() && !summary.model.is_empty(),
                "{name}"
            );
        }

        // Error before any output is reported without emitting raw error bytes.
        let err_start = fixture("openai_responses/streams/error-at-start.sse");
        let mut pass = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let mut out = Vec::new();
        let step = pass.feed(&err_start, &mut out);
        assert!(!step.saw_output);
        let err = step
            .error
            .or_else(|| pass.flush_eof_error(&mut out).error)
            .expect("error at start");
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::ProviderError);
        assert!(!String::from_utf8_lossy(&out).contains("event: error"));

        // Error mid-stream reports output first, then surfaces the error.
        let err_mid = fixture("openai_responses/streams/error-mid-stream.sse");
        let mut pass = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let (out, errs, _) = feed_chunked(pass.as_mut(), &err_mid, 17);
        assert!(!out.is_empty());
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn anthropic_passthrough_preserves_wire_and_merges_usage() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        for name in [
            "text-simple",
            "max-tokens",
            "tools-single",
            "thinking-enabled-budget",
        ] {
            let bytes = fixture(&format!("anthropic_messages/streams/{name}.sse"));
            let mut pass = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
            let (out, errs, ended) = feed_chunked(pass.as_mut(), &bytes, 19);
            assert!(errs.is_empty(), "{name}: {errs:?}");
            assert!(ended, "{name}");
            assert_eq!(out, bytes, "{name} must be byte-identical");
            let summary = pass.summary();
            assert!(
                summary.complete && summary.started && summary.tally.final_usage,
                "{name}"
            );
            assert!(
                summary.usage.input_tokens > 0 && summary.usage.output_tokens > 0,
                "{name}"
            );
        }
    }

    #[test]
    fn responses_cross_dialect_fast_path_matches_preserve_wire_byte_for_byte() {
        let dir = format!(
            "{}/fixtures/openai_responses/streams",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension().and_then(|s| s.to_str()) == Some("sse")).then_some(p)
            })
            .collect();
        paths.sort();
        assert!(!paths.is_empty());
        for path in paths {
            let bytes = std::fs::read(&path).unwrap();
            for target in [
                DialectId::OpenAiChat,
                DialectId::AnthropicMessages,
                DialectId::Gemini,
            ] {
                let run = |preserve_wire: bool| {
                    let mut ectx =
                        EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
                    ectx.preserve_wire = preserve_wire;
                    let mut dec =
                        dispatch::stream_decoder(DialectId::OpenAiResponses, &ectx).unwrap();
                    let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false))
                        .with_created_at(1_700_000_000);
                    let mut enc = dispatch::stream_encoder(target, &enc_ctx).unwrap();
                    let mut out = Vec::new();
                    for chunk in bytes.chunks(29) {
                        for ev in dec.feed(chunk) {
                            out.extend_from_slice(&enc.encode(&ev).unwrap());
                        }
                    }
                    for ev in dec.finish() {
                        out.extend_from_slice(&enc.encode(&ev).unwrap());
                    }
                    out.extend_from_slice(&enc.finish().unwrap());
                    out
                };
                let full = run(true);
                let fast = run(false);
                // When `preserve_wire = true`, a non-canonical Responses frame like
                // `output_text.done` without prior deltas can leave a trailing foreign `Extra`
                // that a cross-dialect encoder turns into a keepalive comment/ping; all data
                // events must still be byte-for-byte identical.
                let data_frames = |raw: &[u8]| -> Vec<String> {
                    String::from_utf8_lossy(raw)
                        .split("\n\n")
                        .filter(|f| {
                            !f.is_empty()
                                && *f != ": keepalive"
                                && *f != "event: ping\ndata: {\"type\": \"ping\"}"
                        })
                        .map(str::to_owned)
                        .collect()
                };
                assert_eq!(
                    data_frames(&fast),
                    data_frames(&full),
                    "fixture {} -> {target} diverged when preserve_wire=false",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn for_stream_rejects_unsupported_modes_and_namespaces() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        assert!(for_stream(DialectId::Gemini, &ectx, &enc).is_none());

        let embed_e = EgressCtx::for_kind(RequestKind::Embed, StreamMode::on(false));
        let embed_c = EncodeCtx::new(RequestKind::Embed, StreamMode::on(false));
        assert!(for_stream(DialectId::OpenAiChat, &embed_e, &enc).is_none());
        assert!(for_stream(DialectId::OpenAiChat, &ectx, &embed_c).is_none());

        let mut with_tools = ectx.clone();
        with_tools
            .tool_names
            .insert("short".to_owned(), "very_long_tool_name".to_owned());
        assert!(for_stream(DialectId::OpenAiChat, &with_tools, &enc).is_none());

        let mut with_ids = ectx.clone();
        with_ids
            .tool_ids
            .insert("id1".to_owned(), "orig1".to_owned());
        assert!(for_stream(DialectId::AnthropicMessages, &with_ids, &enc).is_none());

        let mut thinking_parts = ectx.clone();
        thinking_parts.chat_profile.reply.reasoning = ReplyReasoning::ThinkingParts;
        assert!(for_stream(DialectId::OpenAiChat, &thinking_parts, &enc).is_none());

        let mut exclude_chat = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        let mut chat_req = kaeso_kpp::GenerateRequest::new("gpt-4o-mini");
        let mut r = kaeso_kpp::ReasoningConfig::default();
        r.exclude = true;
        chat_req.reasoning = Some(r.clone());
        exclude_chat.request = Some(std::sync::Arc::new(kaeso_kpp::Request::Generate(chat_req)));
        assert!(for_stream(DialectId::OpenAiChat, &ectx, &exclude_chat).is_none());

        let mut exclude_resp = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        let mut resp_req = kaeso_kpp::GenerateRequest::new("gpt-4o-mini");
        resp_req.reasoning = Some(r);
        exclude_resp.request = Some(std::sync::Arc::new(kaeso_kpp::Request::Generate(resp_req)));
        assert!(for_stream(DialectId::OpenAiResponses, &ectx, &exclude_resp).is_none());

        // Responses requests declaring tool namespaces must stay on the full codec path so the
        // encoder can re-nest tool names on the way out.
        let mut ns_resp = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
        let mut ns_req = kaeso_kpp::GenerateRequest::new("gpt-4o-mini");
        ns_req
            .tools
            .push(kaeso_kpp::ToolDef::Unknown(kaeso_kpp::RawItem::new(
                Origin::new(DialectId::OpenAiResponses),
                RawJson::from_serialize(&serde_json::json!({
                    "type": "namespace",
                    "name": "math",
                    "description": "math tools",
                    "tools": [{"type": "function", "name": "add"}]
                }))
                .unwrap(),
            )));
        ns_resp.request = Some(std::sync::Arc::new(kaeso_kpp::Request::Generate(ns_req)));
        assert!(for_stream(DialectId::OpenAiResponses, &ectx, &ns_resp).is_none());
    }

    #[test]
    fn raw_sse_framer_edge_cases() {
        let mut frames = Vec::new();
        let mut framer = RawSseFramer::new();
        // Split UTF-8 BOM across chunks + standalone comment + CRLF + CR-only frame.
        framer
            .feed(&[0xEF, 0xBB], |f| frames.push(f.to_vec()))
            .unwrap();
        framer
            .feed(
                b"\xBF: keepalive\n\r\ndata: one\r\n\r\ndata: two\r\r",
                |f| frames.push(f.to_vec()),
            )
            .unwrap();
        framer
            .finish_terminated(|f| frames.push(f.to_vec()))
            .unwrap();
        framer.finish(|f| frames.push(f.to_vec())).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], b": keepalive\n");
        assert!(String::from_utf8_lossy(&frames[1]).contains("data: one"));
        assert_eq!(frames[2], b"data: two\r\r");

        // Non-BOM leading bytes are preserved.
        let mut f2 = RawSseFramer::new();
        let mut got = Vec::new();
        f2.feed(b"data: a\n\n", |f| got.push(f.to_vec())).unwrap();
        assert_eq!(got, vec![b"data: a\n\n".to_vec()]);

        // Invalid UTF-8 in fast path and buffered path and finish.
        let mut f3 = RawSseFramer::new();
        assert_eq!(
            f3.feed(b"data: \xFF\n\n", |_| {}),
            Err(SseError::InvalidUtf8)
        );
        let mut f4 = RawSseFramer::new();
        f4.feed(b"data: ", |_| {}).unwrap();
        assert_eq!(f4.feed(b"\xFF\n\n", |_| {}), Err(SseError::InvalidUtf8));
        let mut f5 = RawSseFramer::new();
        f5.feed(b"data: \xFF\n", |_| {}).unwrap();
        assert_eq!(f5.finish(|_| {}), Err(SseError::InvalidUtf8));

        // Truncated unterminated line at EOF.
        let mut f6 = RawSseFramer::new();
        f6.feed(b"data: partial", |_| {}).unwrap();
        // `finish_terminated` leaves unterminated bytes for `finish`.
        let mut flushed = Vec::new();
        f6.finish_terminated(|f| flushed.push(f.to_vec())).unwrap();
        assert!(flushed.is_empty());
        assert_eq!(f6.finish(|_| {}), Err(SseError::Truncated));

        // Partial BOM at EOF also triggers Truncated.
        let mut f7 = RawSseFramer::new();
        f7.feed(&[0xEF], |_| {}).unwrap();
        assert_eq!(f7.finish(|_| {}), Err(SseError::InvalidUtf8));

        // TooLarge in fast path, tail, and buffered path.
        let mut f8 = RawSseFramer {
            max_bytes: 8,
            ..RawSseFramer::new()
        };
        assert!(matches!(
            f8.feed(b"data: 123456789\n\n", |_| {}),
            Err(SseError::TooLarge { limit: 8 })
        ));
        let mut f9 = RawSseFramer {
            max_bytes: 8,
            ..RawSseFramer::new()
        };
        assert!(matches!(
            f9.feed(b"data: 123456789", |_| {}),
            Err(SseError::TooLarge { limit: 8 })
        ));
        let mut f10 = RawSseFramer {
            max_bytes: 8,
            ..RawSseFramer::new()
        };
        f10.feed(b"data: 1", |_| {}).unwrap();
        assert!(matches!(
            f10.feed(b"23456789", |_| {}),
            Err(SseError::TooLarge { limit: 8 })
        ));
        let mut f11 = RawSseFramer {
            max_bytes: 8,
            ..RawSseFramer::new()
        };
        f11.feed(b"data: 1", |_| {}).unwrap();
        assert!(matches!(
            f11.feed(b"23\n\n", |_| {}),
            Err(SseError::TooLarge { limit: 8 })
        ));
        let mut f12 = RawSseFramer {
            max_bytes: 10,
            ..RawSseFramer::new()
        };
        f12.feed(b"a:1\n", |_| {}).unwrap();
        assert!(matches!(
            f12.feed(b"\n12345678901", |_| {}),
            Err(SseError::TooLarge { limit: 10 })
        ));

        // Trailing blank line only at EOF is ignored rather than dispatched.
        let mut f13 = RawSseFramer::new();
        let mut blank_frames = Vec::new();
        f13.feed(b"\r", |f| blank_frames.push(f.to_vec())).unwrap();
        f13.finish(|f| blank_frames.push(f.to_vec())).unwrap();
        assert!(blank_frames.is_empty());
    }

    #[test]
    fn chat_passthrough_errors_validation_and_edge_cases() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc_no = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));

        // Non-plain framing with CR-only line endings + usage stripping + bare `data` line.
        let non_plain = concat!(
            ": comment\n",
            "id: 1\rretry: 1000\rdata: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":1,\"total_tokens\":4}}\r\r",
            "event: custom\ndata: {\"note\":\"keep\"}\n\n",
            "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"refusal\":\"no\",\"reasoning\":\"r1\",\"reasoning_content\":\"r2\",\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{}\"}}],\"audio\":{\"data\":\"AA==\"}},\"finish_reason\":\"stop\"}]}\n\n"
        );
        let mut pass = for_stream(DialectId::OpenAiChat, &ectx, &enc_no).unwrap();
        assert_eq!(pass.keepalive(), keepalive::COMMENT_FRAME);
        let (out, errs, ended) = feed_chunked(pass.as_mut(), non_plain.as_bytes(), 17);
        assert!(errs.is_empty() && ended);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("id: 1\rretry: 1000\rdata: ") && !text.contains("\"usage\""));
        assert!(text.contains(": comment\n"));
        let summary = pass.summary();
        assert_eq!(summary.usage.input_tokens, 3);
        assert_eq!(summary.usage.output_tokens, 1);
        assert!(summary.complete && summary.tally.final_usage);
        assert_eq!(summary.tally.reasoning_deltas, 2);
        assert_eq!(summary.tally.reasoning_bytes, 4);
        assert!(pass.keepalive().is_empty());
        // Feeding after inert is a no-op.
        let mut extra = Vec::new();
        assert_eq!(pass.feed(b"data: [DONE]\n\n", &mut extra).events, 0);
        assert!(extra.is_empty());

        // Validation errors must not mark `saw_output = true` on the same chunk.
        let cases: &[(&str, &str)] = &[
            (
                "finish_reason error",
                "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"x\"},\"finish_reason\":\"error\"}]}\n\n",
            ),
            (
                "n > 1",
                "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}},{\"index\":1,\"delta\":{\"content\":\"b\"}}]}\n\n",
            ),
            (
                "index != 0",
                "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":1,\"delta\":{\"content\":\"a\"}}]}\n\n",
            ),
            (
                "invalid usage",
                "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}],\"usage\":\"bad\"}\n\n",
            ),
            (
                "event error",
                "event: error\ndata: {\"error\":{\"message\":\"boom\",\"type\":\"server_error\"}}\n\n",
            ),
            (
                "inline error",
                "data: {\"error\":{\"message\":\"boom\",\"type\":\"server_error\"}}\n\n",
            ),
            (
                "object error",
                "data: {\"object\":\"error\",\"message\":\"boom\"}\n\n",
            ),
            ("invalid json", "data: {not json}\n\n"),
            ("done before finish", "data: [DONE]\n\n"),
        ];
        for (label, raw) in cases {
            let mut p = for_stream(DialectId::OpenAiChat, &ectx, &enc_no).unwrap();
            let mut o = Vec::new();
            let step = p.feed(raw.as_bytes(), &mut o);
            assert!(step.error.is_some(), "{label} should error");
            assert!(
                !step.saw_output,
                "{label} must not set saw_output before error"
            );
            let encoded = p.encode_error(step.error.as_ref().unwrap());
            assert!(String::from_utf8_lossy(&encoded).starts_with("data: {"));
        }

        // CR-only error at EOF is caught by `flush_eof_error` during peek.
        let mut p = for_stream(DialectId::OpenAiChat, &ectx, &enc_no).unwrap();
        let mut o = Vec::new();
        let s1 = p.feed(
            b"event: error\rdata: {\"error\":{\"message\":\"cr boom\"}}\r\r",
            &mut o,
        );
        assert!(s1.error.is_none());
        let s2 = p.flush_eof_error(&mut o);
        assert!(s2.error.is_some() && !s2.saw_output);

        // `reasons_first` profile ignores opening empty `content: ""` for `saw_output`.
        let mut ds_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        ds_ctx.chat_profile.reply.reasoning_fields = &["reasoning_content"];
        let mut p = for_stream(DialectId::OpenAiChat, &ds_ctx, &enc_no).unwrap();
        let mut o = Vec::new();
        let s1 = p.feed(
            b"data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\",\"reasoning_content\":\"\"},\"finish_reason\":null}]}\n\n",
            &mut o,
        );
        assert!(!s1.saw_output);
        let s2 = p.feed(
            b"data: {\"error\":{\"message\":\"fail after open\"}}\n\n",
            &mut o,
        );
        assert!(s2.error.is_some() && !s2.saw_output);

        // Incomplete stream EOF error & summary fallback to running usage.
        let mut p = for_stream(DialectId::OpenAiChat, &ectx, &enc_no).unwrap();
        let mut o = Vec::new();
        let _ = p.feed(
            b"data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\n",
            &mut o,
        );
        let fin = p.finish(&mut o);
        assert!(fin.error.is_some());
        let sum = p.summary();
        assert_eq!(sum.usage.input_tokens, 4);
        assert!(sum.tally.final_usage);

        // Truncated SSE error on finish.
        let mut p = for_stream(DialectId::OpenAiChat, &ectx, &enc_no).unwrap();
        let mut o = Vec::new();
        let _ = p.feed(b"data: {\"id\":\"c1\"", &mut o);
        assert!(p.finish(&mut o).error.is_some());

        // `write_usage` truncates on failure and handles bare `data` line.
        let mut buf = b"prefix".to_vec();
        assert!(!write_usage(
            "not json",
            b"not json",
            false,
            false,
            &mut buf
        ));
        assert!(!write_usage(
            "{\"a\":1}",
            b"data: {\"a\":1}\n\n",
            false,
            false,
            &mut buf
        ));
        assert!(!write_usage(
            "{\"a\":1,\"usage\":null}",
            b"id: 1\n\n",
            true,
            false,
            &mut buf,
        ));
        assert_eq!(buf, b"prefix");
        assert!(write_usage(
            "{\"a\":1,\"usage\":null}",
            b"id: 1\ndata\n\n",
            true,
            false,
            &mut buf,
        ));
        assert!(String::from_utf8_lossy(&buf).contains("id: 1\ndata: {\"a\":1}\n"));
    }

    #[test]
    fn responses_passthrough_errors_and_edge_cases() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));

        // `response.output_item.added` for `message` and unknown events keep `saw_output = false`;
        // `response.reasoning.delta` tallies reasoning bytes; `response.completed` sets inert.
        let mut pass = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        assert_eq!(pass.keepalive(), keepalive::COMMENT_FRAME);
        let mut out = Vec::new();
        let s0 = pass.feed(
            concat!(
                ": ping\n",
                "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"model\":\"gpt-4o-mini\",\"usage\":{\"input_tokens\":3,\"output_tokens\":0,\"total_tokens\":3}}}\n\n",
                "event: response.in_progress\ndata: {\"type\":\"response.in_progress\"}\n\n",
                "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\"}}\n\n",
                "event: response.custom_unknown\ndata: {\"type\":\"response.custom_unknown\"}\n\n",
                "data: {}\n\n"
            )
            .as_bytes(),
            &mut out,
        );
        assert!(!s0.saw_output && s0.error.is_none());

        let s1 = pass.feed(
            concat!(
                "event: response.reasoning.delta\ndata: {\"type\":\"response.reasoning.delta\",\"delta\":\"think\"}\n\n",
                "event: response.content_part.added\ndata: {\"type\":\"response.content_part.added\"}\n\n",
                "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"model\":\"gpt-4o-mini\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":3,\"output_tokens\":2,\"total_tokens\":5}}}\n\n"
            )
            .as_bytes(),
            &mut out,
        );
        assert!(s1.saw_output && s1.ended && s1.error.is_none());
        assert_eq!(pass.summary().tally.reasoning_deltas, 1);
        assert_eq!(pass.summary().tally.reasoning_bytes, 5);
        assert!(pass.keepalive().is_empty());
        // Inert after `response.completed`: subsequent frames are ignored.
        let len_before = out.len();
        assert_eq!(
            pass.feed(
                b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"late\"}\n\n",
                &mut out
            )
            .events,
            0
        );
        assert_eq!(out.len(), len_before);

        // Native `encode_error` replays single and paired native raw events, falls back for foreign.
        let mut p_err = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let mut err_out = Vec::new();
        let pair = concat!(
            "event: error\ndata: {\"type\":\"error\",\"code\":\"rate_limit\",\"message\":\"slow down\",\"param\":\"model\"}\n\n",
            "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"id\":\"r1\",\"status\":\"failed\",\"error\":{\"code\":\"rate_limit\",\"message\":\"slow down\"}}}\n\n"
        );
        let step = p_err.feed(pair.as_bytes(), &mut err_out);
        let err = step.error.expect("paired error");
        let encoded = String::from_utf8(p_err.encode_error(&err)).unwrap();
        assert!(encoded.contains("event: error\ndata: "));
        assert!(encoded.contains("event: response.failed\ndata: "));

        let single_native = KppError::provider("oops")
            .with_origin(Origin::new(DialectId::OpenAiResponses))
            .with_raw(serde_json::json!({"type": "response.failed", "message": "oops"}));
        let single_enc = String::from_utf8(p_err.encode_error(&single_native)).unwrap();
        assert!(single_enc.starts_with("event: response.failed\ndata: "));

        let foreign = KppError::provider("foreign").with_origin(Origin::new(DialectId::OpenAiChat));
        let foreign_enc = String::from_utf8(p_err.encode_error(&foreign)).unwrap();
        assert!(foreign_enc.starts_with("event: error\ndata: "));

        // Error cases: invalid JSON, standalone `response.failed` (with and without response),
        // missing response on `response.completed`, invalid usage on `response.completed`, truncated.
        for raw in [
            "data: {bad\n\n",
            "event: response.failed\ndata: {\"type\":\"response.failed\"}\n\n",
            "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"id\":\"r1\",\"status\":\"failed\",\"error\":{\"code\":\"server_error\",\"message\":\"bad\"}}}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"model\":\"m\",\"status\":\"completed\",\"usage\":\"bad\"}}\n\n",
        ] {
            let mut p = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
            let mut o = Vec::new();
            assert!(p.feed(raw.as_bytes(), &mut o).error.is_some(), "{raw}");
        }

        // Held error flushed by `finish` and by `flush_eof_error` on CR-only input.
        let mut p1 = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let mut o1 = Vec::new();
        assert!(
            p1.feed(
                b"event: error\ndata: {\"type\":\"error\",\"message\":\"solo\"}\n\n",
                &mut o1
            )
            .error
            .is_none()
        );
        assert!(p1.finish(&mut o1).error.is_some());

        let mut p2 = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let mut o2 = Vec::new();
        let _ = p2.feed(
            b"event: error\rdata: {\"type\":\"error\",\"message\":\"cr\"}\r\r",
            &mut o2,
        );
        assert!(p2.flush_eof_error(&mut o2).error.is_some());

        // Truncated SSE error on finish.
        let mut p3 = for_stream(DialectId::OpenAiResponses, &ectx, &enc).unwrap();
        let mut o3 = Vec::new();
        let _ = p3.feed(b"data: {\"type\":\"response.created\"", &mut o3);
        assert!(p3.finish(&mut o3).error.is_some());

        // Cross-dialect `preserve_wire = false` decoder reports truncated stream on `finish`.
        let mut cross_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        cross_ctx.preserve_wire = false;
        let mut dec = dispatch::stream_decoder(DialectId::OpenAiResponses, &cross_ctx).unwrap();
        let _ = dec.feed(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"model\":\"gpt-4o-mini\",\"status\":\"in_progress\"}}\n\n",
        );
        let fin = dec.finish();
        assert!(
            fin.iter()
                .any(|e| matches!(e, kaeso_kpp::StreamEvent::Error(_))),
            "preserve_wire=false must emit an error on truncated Responses stream: {fin:?}"
        );
    }

    #[test]
    fn anthropic_passthrough_errors_and_edge_cases() {
        let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
        let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));

        let mut pass = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
        assert_eq!(pass.keepalive(), keepalive::PING_FRAME);
        let mut out = Vec::new();
        let s0 = pass.feed(
            concat!(
                ": comment\n",
                "event: ping\ndata: {\"type\":\"ping\"}\n\n",
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"claude\",\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
                "event: unknown_ev\ndata: {\"type\":\"unknown_ev\"}\n\n",
                "data: {}\n\n"
            )
            .as_bytes(),
            &mut out,
        );
        assert!(!s0.saw_output && s0.error.is_none());

        let s1 = pass.feed(
            concat!(
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\n",
                "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"stop_sequence\",\"stop_sequence\":\"END\"},\"usage\":{\"output_tokens\":5}}\n\n",
                "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
            )
            .as_bytes(),
            &mut out,
        );
        assert!(s1.saw_output && s1.ended && s1.error.is_none());
        let sum = pass.summary();
        assert_eq!(sum.stop, StopReason::StopSequence("END".to_owned()));
        assert_eq!(sum.usage.input_tokens, 10);
        assert_eq!(sum.usage.output_tokens, 5);
        assert_eq!(sum.tally.reasoning_deltas, 1);
        assert_eq!(sum.tally.reasoning_bytes, 3);
        assert!(pass.keepalive().is_empty());
        // Inert after `message_stop`.
        assert_eq!(
            pass.feed(b"event: ping\ndata: {\"type\":\"ping\"}\n\n", &mut out)
                .events,
            0
        );

        // Error events: `event: error`, JSON `{"type":"error"}`, invalid JSON, incomplete EOF,
        // truncated EOF, and CR-only EOF error.
        for raw in [
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}\n\n",
            "data: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"bad\"}}\n\n",
            "data: {not json\n\n",
        ] {
            let mut p = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
            let mut o = Vec::new();
            let step = p.feed(raw.as_bytes(), &mut o);
            assert!(step.error.is_some() && !step.saw_output, "{raw}");
            let encoded = String::from_utf8(p.encode_error(step.error.as_ref().unwrap())).unwrap();
            assert!(encoded.starts_with("event: error\ndata: "));
        }

        let mut p_inc = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
        let mut o_inc = Vec::new();
        let _ = p_inc.feed(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"c\"}}\n\n",
            &mut o_inc,
        );
        assert!(p_inc.finish(&mut o_inc).error.is_some());

        let mut p_trunc = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
        let mut o_trunc = Vec::new();
        let _ = p_trunc.feed(b"data: {\"type\":\"ping\"", &mut o_trunc);
        assert!(p_trunc.finish(&mut o_trunc).error.is_some());

        let mut p_cr = for_stream(DialectId::AnthropicMessages, &ectx, &enc).unwrap();
        let mut o_cr = Vec::new();
        let _ = p_cr.feed(
            b"event: error\rdata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"cr\"}}\r\r",
            &mut o_cr,
        );
        assert!(p_cr.flush_eof_error(&mut o_cr).error.is_some());
    }
}
