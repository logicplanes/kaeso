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
    fn flush_error(&mut self) -> Option<KppError> {
        None
    }
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
        on_frame(&trailing);
        Ok(())
    }
}

// Finds the byte offset right after the blank line terminating the frame starting at `from`, or
// right after a standalone comment line followed by a blank line.
fn find_frame_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let mut saw_content = false;
    while let Some(rest) = bytes.get(i..) {
        if rest.is_empty() {
            return None;
        }
        let rel = rest.iter().position(|&b| b == b'\n' || b == b'\r')?;
        let line_len = rel;
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

    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
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

// ---------- OpenAI Chat Completions passthrough ----------

struct ChatPassthrough {
    framer: RawSseFramer,
    state: ChatState,
}

struct ChatState {
    origin: Origin,
    include_usage: bool,
    error_detail: &'static [&'static str],
    id: String,
    model: String,
    usage: Usage,
    running_usage: Option<Usage>,
    stop: StopReason,
    started: bool,
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
                error_detail: ctx.chat_profile.reply.error_detail,
                id: String::new(),
                model: String::new(),
                usage: Usage::default(),
                running_usage: None,
                stop: StopReason::EndTurn,
                started: false,
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
            out.extend_from_slice(raw);
            step.events = step.events.saturating_add(1);
            return;
        }
        if frame.has_data && frame.event == Some("error") {
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
                self.inert = true;
                step.error = Some(
                    KppError::unsupported_by(
                        "n",
                        DialectId::OpenAiChat,
                        "The provider streamed several choices, but Kaeso carries exactly one; do not request `n` > 1.",
                    )
                    .with_origin(self.origin.clone()),
                );
                return;
            }
            Some(choices) if choices.is_empty() => true,
            Some(choices) => {
                if let Some(ch) = choices.first() {
                    if ch.index != 0 {
                        self.inert = true;
                        step.error = Some(
                            KppError::unsupported_by(
                                "n",
                                DialectId::OpenAiChat,
                                "The provider streamed a choice other than the first; do not request `n` > 1.",
                            )
                            .with_origin(self.origin.clone()),
                        );
                        return;
                    }
                    if let Some(delta) = &ch.delta {
                        if let Some(content) = &delta.content {
                            step.saw_output = true;
                            if let TextOrOther::Text(text) = content
                                && !text.is_empty()
                            {
                                self.tally.add(text.len(), false);
                            }
                        }
                        if let Some(refusal) = &delta.refusal {
                            step.saw_output = true;
                            if !refusal.is_empty() {
                                self.tally.add(refusal.len(), false);
                            }
                        }
                        for r in [&delta.reasoning, &delta.reasoning_content]
                            .into_iter()
                            .flatten()
                        {
                            step.saw_output = true;
                            if let TextOrOther::Text(text) = r
                                && !text.is_empty()
                            {
                                self.tally.add(text.len(), true);
                            }
                        }
                        if let Some(tcs) = &delta.tool_calls
                            && !tcs.is_empty()
                        {
                            step.saw_output = true;
                            for tc in tcs {
                                if let Some(args) =
                                    tc.function.as_ref().and_then(|f| f.arguments.as_ref())
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
                        if finish == "error" {
                            let raw_val =
                                serde_json::from_str::<Value>(&frame.data).unwrap_or(Value::Null);
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
                        self.finished = true;
                        self.stop = crate::openai_chat::response::decode_finish(finish);
                        step.saw_output = true;
                    }
                }
                false
            }
            None => false,
        };

        let mut has_non_null_usage = false;
        if let Some(usage_raw) = tap.usage
            && usage_raw.get() != "null"
        {
            has_non_null_usage = true;
            let raw_json = RawJson::from(usage_raw);
            match crate::openai_chat::response::decode_usage(&raw_json, &self.origin) {
                Ok(decoded) => {
                    if self.finished {
                        self.running_usage = None;
                        self.usage = decoded;
                        self.tally.final_usage = true;
                    } else {
                        self.running_usage = Some(decoded);
                    }
                }
                Err(err) => {
                    self.inert = true;
                    step.error = Some(err.with_origin(self.origin.clone()));
                    return;
                }
            }
        }

        step.events = step.events.saturating_add(1);
        if !self.include_usage && tap.usage.is_some() {
            // Kaeso asked the provider for usage so it can bill the stream; hide the injected
            // usage chunk or field from an app that did not set `include_usage`.
            if has_non_null_usage && choices_empty {
                return;
            }
            if write_without_usage(&frame.data, raw, frame.has_non_plain_framing, out) {
                return;
            }
        }
        out.extend_from_slice(raw);
    }
}

fn write_without_usage(
    data: &str,
    raw: &[u8],
    has_non_plain_framing: bool,
    out: &mut Vec<u8>,
) -> bool {
    let Ok(mut map) = serde_json::from_str::<IndexMap<String, &RawValue>>(data) else {
        return false;
    };
    if map.shift_remove("usage").is_none() {
        return false;
    }
    if has_non_plain_framing {
        // Preserve non-data SSE fields (id, retry, etc.) by rewriting only the `data:` line.
        let Ok(text) = std::str::from_utf8(raw) else {
            return false;
        };
        let mut wrote_data = false;
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.starts_with("data:") {
                if !wrote_data {
                    out.extend_from_slice(b"data: ");
                    if serde_json::to_writer(&mut *out, &map).is_err() {
                        return false;
                    }
                    out.extend_from_slice(b"\n");
                    wrote_data = true;
                }
            } else {
                out.extend_from_slice(line.as_bytes());
            }
        }
        return wrote_data;
    }
    let start = out.len();
    out.extend_from_slice(b"data: ");
    if serde_json::to_writer(&mut *out, &map).is_err() {
        out.truncate(start);
        return false;
    }
    out.extend_from_slice(b"\n\n");
    true
}

impl StreamPassthrough for ChatPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.feed(chunk, |frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err {
            step.error = Some(err);
        } else if let Err(e) = res
            && !self.state.inert
        {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
        }
        step
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.finish(|frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err {
            step.error = Some(err);
            return step;
        }
        if let Err(e) = res {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
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

    fn encode_error(&mut self, err: &KppError) -> Vec<u8> {
        let body = crate::openai_chat::error::error_body(err);
        let mut out = Vec::new();
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
            Some("response.reasoning_text.delta" | "response.reasoning_summary_text.delta") => {
                if let Some(delta) = tap.delta {
                    self.tally.add(delta.len(), true);
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some("response.output_item.added") => {
                if tap
                    .item
                    .is_some_and(|i| type_tag(i).ok().flatten().as_deref() == Some("function_call"))
                {
                    self.called = true;
                }
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(_) => {
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            None => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
        }
    }
}

impl StreamPassthrough for ResponsesPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.feed(chunk, |frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err {
            step.error = Some(err);
        } else if let Err(e) = res
            && !self.state.inert
        {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
        }
        step
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.finish(|frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err.or_else(|| self.state.held_error.take()) {
            self.state.inert = true;
            step.error = Some(err);
            return step;
        }
        if let Err(e) = res {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
            return step;
        }
        self.state.inert = true;
        step
    }

    fn flush_error(&mut self) -> Option<KppError> {
        self.state.held_error.take()
    }

    fn encode_error(&mut self, err: &KppError) -> Vec<u8> {
        let body = crate::openai_responses::error::stream_error_event(err);
        let mut out = Vec::new();
        out.extend_from_slice(b"event: error\ndata: ");
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
                step.saw_output = true;
                step.ended = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            Some(_) => {
                step.saw_output = true;
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
            None => {
                step.events = step.events.saturating_add(1);
                out.extend_from_slice(raw);
            }
        }
    }
}

impl StreamPassthrough for AnthropicPassthrough {
    fn feed(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.feed(chunk, |frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err {
            step.error = Some(err);
        } else if let Err(e) = res
            && !self.state.inert
        {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
        }
        step
    }

    fn finish(&mut self, out: &mut Vec<u8>) -> PassthroughStep {
        let mut step = PassthroughStep::default();
        if self.state.inert {
            return step;
        }
        let state = &mut self.state;
        let mut framed_err = None;
        let res = self.framer.finish(|frame| {
            if framed_err.is_none() && !state.inert {
                state.process_frame(frame, out, &mut step);
                if step.error.is_some() {
                    framed_err = step.error.take();
                }
            }
        });
        if let Some(err) = framed_err {
            step.error = Some(err);
            return step;
        }
        if let Err(e) = res {
            self.state.inert = true;
            step.error = Some(KppError::from(e).with_origin(self.state.origin.clone()));
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
        assert_eq!(
            out_with,
            raw.as_bytes(),
            "include_usage=true is byte-identical"
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
            .or_else(|| pass.flush_error())
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
    #[ignore = "microbenchmark: cargo test -p kaeso-codecs --release bench_streaming_codecs -- --ignored --nocapture"]
    fn bench_streaming_codecs() {
        let resp_bytes = fixture("openai_responses/streams/recorded-text-basic-stream.sse");
        let resp_chunks: Vec<&[u8]> = resp_bytes.split_inclusive(|b| *b == b'\n').collect();
        let iters = 2_000u32;

        let t0 = std::time::Instant::now();
        for _ in 0..iters {
            let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
            let mut dec = dispatch::stream_decoder(DialectId::OpenAiResponses, &ectx).unwrap();
            let mut enc = dispatch::stream_encoder(DialectId::OpenAiResponses, &enc_ctx).unwrap();
            let mut out = Vec::new();
            for c in &resp_chunks {
                for ev in dec.feed(c) {
                    out.extend_from_slice(&enc.encode(&ev).unwrap());
                }
            }
            for ev in dec.finish() {
                out.extend_from_slice(&enc.encode(&ev).unwrap());
            }
            out.extend_from_slice(&enc.finish().unwrap());
            std::hint::black_box(&out);
        }
        let full_same = t0.elapsed().as_secs_f64() * 1e6 / f64::from(iters);

        let t1 = std::time::Instant::now();
        for _ in 0..iters {
            let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
            let mut pass = for_stream(DialectId::OpenAiResponses, &ectx, &enc_ctx).unwrap();
            let mut out = Vec::new();
            for c in &resp_chunks {
                let _ = pass.feed(c, &mut out);
            }
            let _ = pass.finish(&mut out);
            std::hint::black_box(&out);
        }
        let pass_same = t1.elapsed().as_secs_f64() * 1e6 / f64::from(iters);

        let t2 = std::time::Instant::now();
        for _ in 0..iters {
            let mut ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
            ectx.preserve_wire = true;
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
            let mut dec = dispatch::stream_decoder(DialectId::OpenAiResponses, &ectx).unwrap();
            let mut enc = dispatch::stream_encoder(DialectId::OpenAiChat, &enc_ctx).unwrap();
            let mut out = Vec::new();
            for c in &resp_chunks {
                for ev in dec.feed(c) {
                    out.extend_from_slice(&enc.encode(&ev).unwrap());
                }
            }
            for ev in dec.finish() {
                out.extend_from_slice(&enc.encode(&ev).unwrap());
            }
            out.extend_from_slice(&enc.finish().unwrap());
            std::hint::black_box(&out);
        }
        let cross_full = t2.elapsed().as_secs_f64() * 1e6 / f64::from(iters);

        let t3 = std::time::Instant::now();
        for _ in 0..iters {
            let mut ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(false));
            ectx.preserve_wire = false;
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
            let mut dec = dispatch::stream_decoder(DialectId::OpenAiResponses, &ectx).unwrap();
            let mut enc = dispatch::stream_encoder(DialectId::OpenAiChat, &enc_ctx).unwrap();
            let mut out = Vec::new();
            for c in &resp_chunks {
                for ev in dec.feed(c) {
                    out.extend_from_slice(&enc.encode(&ev).unwrap());
                }
            }
            for ev in dec.finish() {
                out.extend_from_slice(&enc.encode(&ev).unwrap());
            }
            out.extend_from_slice(&enc.finish().unwrap());
            std::hint::black_box(&out);
        }
        let cross_fast = t3.elapsed().as_secs_f64() * 1e6 / f64::from(iters);

        eprintln!(
            "responses->responses: full={full_same:.1}us passthrough={pass_same:.1}us ({:.1}x)\nresponses->chat:      preserve_wire=true={cross_full:.1}us preserve_wire=false={cross_fast:.1}us ({:.1}x)",
            full_same / pass_same,
            cross_full / cross_fast,
        );
    }
}
