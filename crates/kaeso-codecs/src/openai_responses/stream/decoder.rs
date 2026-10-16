use std::collections::HashMap;

use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Citation, Delta, Extras, KppError, Opaque,
    OpaqueKind, Origin, RawItem, RawJson, ReasoningHeader, ResponseDelta, ResponseStart,
    ResponseStop, StreamEvent, SummaryDelta, ToolCallHeader,
};
use serde_json::Value;
use serde_json::value::RawValue;

use super::{EVENT_KEY, Engine, NEW_ITEM, NEW_PART, PART, SKIP, event_extras};
use crate::codec::{EgressCtx, StreamDecoder};
use crate::openai_responses::DIALECT;
use crate::openai_responses::node::{Node, entries_of};
use crate::openai_responses::overrides::{Overrides, form_set_insert};
use crate::openai_responses::response::{NATIVE, decode_usage, failed_error, stop_for};
use crate::openai_responses::wire;
use crate::sse::{SseItem, SseParser};
use crate::wire::type_tag;

#[derive(Clone, Debug)]
struct Frame {
    event: Option<String>,
    ty: String,
    data: Box<RawValue>,
    value: Value,
}

impl Frame {
    fn int(&self, key: &str) -> Option<i64> {
        self.value.get(key).and_then(Value::as_i64)
    }

    fn str(&self, key: &str) -> Option<&str> {
        self.value.get(key).and_then(Value::as_str)
    }

    fn raw(&self, key: &str) -> Option<&RawValue> {
        entries_of(&self.data)?
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemKind {
    Call,
    Reasoning,
    Unknown,
}

enum Action {
    Hold,
    Pre,
    Main(Vec<StreamEvent>),
    HoldError(KppError),
    Broken(String),
    Other,
}

pub(crate) struct Decoder {
    parser: SseParser,
    origin: Origin,
    ctx: EgressCtx,
    engine: Engine,
    pending: Vec<Frame>,
    held: Option<Frame>,
    next_block: u32,
    current_message: Option<i64>,
    items: HashMap<i64, (u32, ItemKind)>,
    parts: HashMap<(i64, i64), u32>,
    held_error: Option<(KppError, Frame)>,
    inert: bool,
}

impl Decoder {
    pub(crate) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            parser: SseParser::new(),
            origin: ctx.origin(DIALECT),
            ctx: ctx.clone(),
            engine: Engine::new(Vec::new(), 0),
            pending: Vec::new(),
            held: None,
            next_block: 0,
            current_message: None,
            items: HashMap::new(),
            parts: HashMap::new(),
            held_error: None,
            inert: false,
        }
    }

    fn fail(&mut self, out: &mut Vec<StreamEvent>, err: KppError) {
        out.push(StreamEvent::Error(err.with_origin(self.origin.clone())));
        self.inert = true;
    }

    fn extra(&mut self, out: &mut Vec<StreamEvent>, raw: Node, seq: Option<i64>, is_event: bool) {
        if let Ok(raw) = raw.to_raw() {
            out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
        }
        if is_event {
            self.engine.saw_sequence(seq);
        }
    }

    fn frame_extra(&mut self, out: &mut Vec<StreamEvent>, f: &Frame) {
        let raw = Node::obj()
            .with(
                "event",
                f.event.as_deref().map_or_else(Node::null, Node::str),
            )
            .with("data", Node::raw_value(&f.data));
        let seq = f.int("sequence_number");
        self.extra(out, raw, seq, true);
    }

    fn flush_pending(&mut self, out: &mut Vec<StreamEvent>) {
        for f in std::mem::take(&mut self.pending) {
            self.frame_extra(out, &f);
        }
    }

    fn release_held(&mut self, out: &mut Vec<StreamEvent>) {
        if let Some(created) = self.held.take() {
            self.finish_start(created, None, out);
        }
    }

    fn sse_item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        match item {
            SseItem::Comment(c) => {
                self.release_held(out);
                self.flush_pending(out);
                self.extra(out, Node::obj().with("comment", Node::str(c)), None, false);
            }
            SseItem::Event(e) => {
                let data: Option<Box<RawValue>> = serde_json::from_str(&e.data).ok();
                let plain = e.id.is_none() && e.retry.is_none() && e.other.is_empty();
                let value: Option<Value> = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str(d.get()).ok());
                let ty = value
                    .as_ref()
                    .and_then(|v| v.get("type"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                match (data, value, ty) {
                    (Some(data), Some(value), Some(ty)) if plain => {
                        self.frame(
                            Frame {
                                event: e.event,
                                ty,
                                data,
                                value,
                            },
                            out,
                        );
                    }
                    (data, _, _) => {
                        if data.is_none() && !e.data.trim().is_empty() {
                            let err = KppError::provider(format!(
                                "The provider sent a stream event that is not valid JSON: {}",
                                crate::wire::Short(&e.data, 200)
                            ));
                            self.fail(out, err);
                            return;
                        }
                        self.release_held(out);
                        self.flush_pending(out);
                        let mut raw = Node::obj().with(
                            "event",
                            e.event.as_deref().map_or_else(Node::null, Node::str),
                        );
                        let seq = match &data {
                            Some(d) => {
                                raw.insert("data", Node::raw_value(d));
                                serde_json::from_str::<Value>(d.get())
                                    .ok()
                                    .and_then(|v| v.get("sequence_number").and_then(Value::as_i64))
                            }
                            None => {
                                raw.insert("data", Node::str(&e.data));
                                raw.insert("text", Node::bool(true));
                                None
                            }
                        };
                        if let Some(id) = &e.id {
                            raw.insert("id", Node::str(id));
                        }
                        if let Some(retry) = e.retry {
                            raw.insert("retry", Node::int(retry));
                        }
                        if !e.other.is_empty() {
                            raw.insert(
                                "fields",
                                Node::Arr(
                                    e.other
                                        .iter()
                                        .map(|(k, v)| Node::Arr(vec![Node::str(k), Node::str(v)]))
                                        .collect(),
                                ),
                            );
                        }
                        self.extra(out, raw, seq, true);
                    }
                }
            }
        }
    }

    fn frame(&mut self, f: Frame, out: &mut Vec<StreamEvent>) {
        if let Some((err, first)) = self.held_error.take() {
            let mut frames = vec![first];
            let mut err = err;
            if f.ty == "response.failed" {
                if let Some(Value::Object(_)) = &err.raw {
                    let first_raw = err.raw.take().unwrap_or(Value::Null);
                    err.raw = Some(Value::Array(vec![first_raw, f.value.clone()]));
                }
                frames.push(f);
            }
            self.run(StreamEvent::Error(err), frames, out);
            self.inert = true;
            return;
        }
        if let Some(created) = self.held.take() {
            if f.ty == "response.in_progress" && self.pending.is_empty() {
                self.finish_start(created, Some(f), out);
                return;
            }
            self.finish_start(created, None, out);
        }
        match self.classify(&f) {
            Action::Hold => {
                self.flush_pending(out);
                self.held = Some(f);
            }
            Action::HoldError(err) => {
                self.flush_pending(out);
                self.held_error = Some((err, f));
            }
            Action::Pre => self.pending.push(f),
            Action::Main(mut events) => {
                let Some(last) = events.pop() else {
                    self.flush_pending(out);
                    self.frame_extra(out, &f);
                    return;
                };
                if !self.engine.started()
                    && !matches!(last, StreamEvent::Error(_))
                    && let Err(e) = self.synthesize_start(out)
                {
                    self.flush_pending(out);
                    self.fail(out, e);
                    return;
                }
                for ev in events {
                    if let Err(e) = self.engine.step(&ev, &mut |_, _, _| Ok(None)) {
                        self.flush_pending(out);
                        self.fail(out, e);
                        return;
                    }
                    out.push(ev);
                }
                self.lifecycle(last, f, out);
                if self.engine.terminated() {
                    self.inert = true;
                }
            }
            Action::Broken(message) => {
                self.flush_pending(out);
                self.fail(out, KppError::provider(message));
            }
            Action::Other => {
                self.flush_pending(out);
                self.frame_extra(out, &f);
            }
        }
    }

    fn synthesize_start(&mut self, out: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        let mut start = ResponseStart::new("", "");
        start.extras = Extras::new(self.origin.clone())
            .with_form(NATIVE, true.into())
            .with_form(SKIP, serde_json::json!(["", "@in_progress"]));
        let event = StreamEvent::ResponseStart(start);
        self.engine.step(&event, &mut |_, _, _| Ok(None))?;
        out.push(event);
        Ok(())
    }

    fn finish_start(
        &mut self,
        created: Frame,
        in_progress: Option<Frame>,
        out: &mut Vec<StreamEvent>,
    ) {
        let Some(resp) = created.raw("response") else {
            self.frame_extra(out, &created);
            if let Some(f) = in_progress {
                self.frame_extra(out, &f);
            }
            return;
        };
        let Ok(w) = serde_json::from_str::<wire::ResponseObject>(resp.get()) else {
            self.frame_extra(out, &created);
            if let Some(f) = in_progress {
                self.frame_extra(out, &f);
            }
            return;
        };
        let mut start = ResponseStart::new(w.id, w.model);
        start.usage = w
            .usage
            .as_deref()
            .and_then(|u| decode_usage(u, &self.origin));
        start.extras = Extras::new(self.origin.clone()).with_form(NATIVE, true.into());
        let mut frames = vec![created];
        frames.extend(in_progress);
        self.run(StreamEvent::ResponseStart(start), frames, out);
    }

    fn next_index(&mut self) -> u32 {
        let i = self.next_block;
        self.next_block = self.next_block.saturating_add(1);
        i
    }

    fn classify(&mut self, f: &Frame) -> Action {
        let oi = f.int("output_index");
        let ci = f.int("content_index");
        let delta = || f.str("delta").map(str::to_owned);
        let block_delta = |index: u32, d: Delta| StreamEvent::BlockDelta(BlockDelta::new(index, d));
        match f.ty.as_str() {
            "response.created" if !self.engine.started() => Action::Hold,
            "response.output_item.added" => {
                let (Some(oi), Some(item)) = (oi, f.raw("item")) else {
                    return self.broken(f);
                };
                let header = match type_tag(item).ok().flatten().as_deref() {
                    Some("message") => return Action::Pre,
                    Some("function_call") => {
                        let Ok(c) = serde_json::from_str::<wire::FunctionCallItem>(item.get())
                        else {
                            return self.broken(f);
                        };
                        (
                            BlockHeader::ToolCall(ToolCallHeader::new(c.call_id, c.name)),
                            ItemKind::Call,
                        )
                    }
                    Some("reasoning") => {
                        let mut h = ReasoningHeader::new(self.origin.clone());
                        if let Some(id) = serde_json::from_str::<Value>(item.get())
                            .ok()
                            .and_then(|v| v.get("id").and_then(Value::as_str).map(str::to_owned))
                        {
                            h.opaque.push(Opaque::new(
                                self.origin.clone(),
                                OpaqueKind::OpenAiReasoningItemId,
                                id,
                            ));
                        }
                        (BlockHeader::Reasoning(h), ItemKind::Reasoning)
                    }
                    _ => (
                        BlockHeader::Unknown(RawItem::new(
                            self.origin.clone(),
                            RawJson::from(item),
                        )),
                        ItemKind::Unknown,
                    ),
                };
                let index = self.next_index();
                self.items.insert(oi, (index, header.1));
                let mut start = BlockStart::new(index, header.0);
                start.extras = Extras::new(self.origin.clone());
                Action::Main(vec![StreamEvent::BlockStart(start)])
            }
            "response.content_part.added" => {
                let (Some(oi), Some(ci), Some(part)) = (oi, ci, f.raw("part")) else {
                    return self.broken(f);
                };
                if let Some((index, ItemKind::Reasoning)) = self.items.get(&oi).copied() {
                    let mut d = BlockDelta::new(index, Delta::ReasoningText(String::new()));
                    d.extras = Extras::new(self.origin.clone()).with_form(NEW_PART, true.into());
                    return Action::Main(vec![StreamEvent::BlockDelta(d)]);
                }
                if self.items.contains_key(&oi) {
                    return self.broken(f);
                }
                let mut extras = Extras::new(self.origin.clone());
                let header = match type_tag(part).ok().flatten().as_deref() {
                    Some("output_text") => BlockHeader::Text,
                    Some("refusal") => BlockHeader::Refusal,
                    _ => {
                        extras.form.insert(PART.to_owned(), true.into());
                        BlockHeader::Unknown(RawItem::new(self.origin.clone(), RawJson::from(part)))
                    }
                };
                if self.engine.message_open() && self.current_message != Some(oi) {
                    extras.form.insert(NEW_ITEM.to_owned(), true.into());
                }
                self.current_message = Some(oi);
                let index = self.next_index();
                self.parts.insert((oi, ci), index);
                let mut start = BlockStart::new(index, header);
                start.extras = extras;
                Action::Main(vec![StreamEvent::BlockStart(start)])
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                let (Some(index), Some(text)) = (self.part(oi, ci), delta()) else {
                    return self.broken(f);
                };
                let d = if f.ty == "response.output_text.delta" {
                    Delta::Text(text)
                } else {
                    Delta::Refusal(text)
                };
                Action::Main(vec![block_delta(index, d)])
            }
            "response.output_text.annotation.added" => {
                let (Some(index), Some(a)) = (self.part(oi, ci), f.raw("annotation")) else {
                    return self.broken(f);
                };
                let c = Citation::new(self.origin.clone(), RawJson::from(a));
                Action::Main(vec![block_delta(index, Delta::Citation(c))])
            }
            "response.output_text.done"
            | "response.refusal.done"
            | "response.function_call_arguments.done"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_summary_part.done"
            | "response.reasoning_text.done" => Action::Pre,
            "response.content_part.done" => {
                if let (Some(oi), Some(ci)) = (oi, ci)
                    && let Some(index) = self.parts.remove(&(oi, ci))
                {
                    return Action::Main(vec![StreamEvent::BlockStop(BlockStop::new(index))]);
                }
                match oi.and_then(|oi| self.items.get(&oi)) {
                    Some((_, ItemKind::Reasoning)) => Action::Pre,
                    _ => self.broken(f),
                }
            }
            "response.function_call_arguments.delta" => {
                match (oi.and_then(|oi| self.items.get(&oi)), delta()) {
                    (Some((index, ItemKind::Call)), Some(t)) => {
                        Action::Main(vec![block_delta(*index, Delta::ToolArguments(t))])
                    }
                    _ => self.broken(f),
                }
            }
            "response.reasoning_summary_part.added" | "response.reasoning_summary_text.delta" => {
                let si = f.int("summary_index").and_then(|s| u32::try_from(s).ok());
                let text = if f.ty.ends_with("part.added") {
                    Some(String::new())
                } else {
                    delta()
                };
                match (oi.and_then(|oi| self.items.get(&oi)), si, text) {
                    (Some((index, ItemKind::Reasoning)), Some(si), Some(t)) => {
                        Action::Main(vec![block_delta(
                            *index,
                            Delta::ReasoningSummary(SummaryDelta::new(si, t)),
                        )])
                    }
                    _ => self.broken(f),
                }
            }
            "response.reasoning_text.delta" => {
                match (oi.and_then(|oi| self.items.get(&oi)), delta()) {
                    (Some((index, ItemKind::Reasoning)), Some(t)) => {
                        Action::Main(vec![block_delta(*index, Delta::ReasoningText(t))])
                    }
                    _ => self.broken(f),
                }
            }
            "response.output_item.done" => {
                let (Some(oi), Some(item)) = (oi, f.raw("item")) else {
                    return self.broken(f);
                };
                if type_tag(item).ok().flatten().as_deref() == Some("message") {
                    if self.current_message == Some(oi) {
                        self.current_message = None;
                    }
                    return Action::Pre;
                }
                let Some((index, kind)) = self.items.remove(&oi) else {
                    return self.broken(f);
                };
                let mut events = Vec::new();
                match kind {
                    ItemKind::Reasoning => {
                        if let Some(enc) =
                            serde_json::from_str::<Value>(item.get())
                                .ok()
                                .and_then(|v| {
                                    v.get("encrypted_content")
                                        .and_then(Value::as_str)
                                        .map(str::to_owned)
                                })
                        {
                            let o = Opaque::new(
                                self.origin.clone(),
                                OpaqueKind::OpenAiEncryptedReasoning,
                                enc,
                            );
                            events.push(block_delta(index, Delta::Opaque(o)));
                        }
                    }
                    ItemKind::Unknown => {
                        let raw = RawItem::new(self.origin.clone(), RawJson::from(item));
                        events.push(block_delta(index, Delta::Unknown(raw)));
                    }
                    ItemKind::Call => {}
                }
                events.push(StreamEvent::BlockStop(BlockStop::new(index)));
                Action::Main(events)
            }
            "response.completed" | "response.incomplete" => {
                let Some(w) = f
                    .raw("response")
                    .and_then(|r| serde_json::from_str::<wire::ResponseObject>(r.get()).ok())
                else {
                    return self.broken(f);
                };
                let has_calls = w
                    .output
                    .iter()
                    .flatten()
                    .any(|i| type_tag(i).ok().flatten().as_deref() == Some("function_call"));
                let stop = stop_for(
                    w.status.as_deref(),
                    w.incomplete_details
                        .as_ref()
                        .and_then(|d| d.reason.as_deref()),
                    has_calls,
                );
                let usage = w
                    .usage
                    .as_deref()
                    .and_then(|u| decode_usage(u, &self.origin));
                let mut d = ResponseDelta::new(Some(stop), usage);
                d.extras = Extras::new(self.origin.clone());
                let mut s = ResponseStop::new();
                s.extras = Extras::new(self.origin.clone());
                Action::Main(vec![
                    StreamEvent::ResponseDelta(d),
                    StreamEvent::ResponseStop(s),
                ])
            }
            "response.failed" => {
                let Some(w) = f
                    .raw("response")
                    .and_then(|r| serde_json::from_str::<wire::ResponseObject>(r.get()).ok())
                else {
                    return self.broken(f);
                };
                Action::Main(vec![StreamEvent::Error(failed_error(
                    &w,
                    f.value.clone(),
                    &self.origin,
                ))])
            }
            "error" => {
                let fields = match f.value.get("error") {
                    Some(nested @ Value::Object(_)) => nested,
                    _ => &f.value,
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
                    .with_raw(f.value.clone());
                if let Some(p) = field("param") {
                    err = err.with_param(p);
                }
                Action::HoldError(err)
            }
            _ => Action::Other,
        }
    }

    fn broken(&self, f: &Frame) -> Action {
        Action::Broken(format!(
            "The provider sent a `{}` event that refers to an output item or part it never announced, or lacks required fields.",
            f.ty
        ))
    }

    fn part(&self, oi: Option<i64>, ci: Option<i64>) -> Option<u32> {
        self.parts.get(&(oi?, ci?)).copied()
    }

    fn lifecycle(&mut self, event: StreamEvent, main: Frame, out: &mut Vec<StreamEvent>) {
        if !self.pending.is_empty() {
            let queue: Vec<&Frame> = self.pending.iter().chain(std::iter::once(&main)).collect();
            let mut shadow = self.engine.clone();
            let mut qi = 0;
            let ok = shadow
                .step(&event, &mut |_, canon, _| {
                    if queue.get(qi).is_some_and(|f| matches(&canon, f)) {
                        qi += 1;
                        Ok(queue.get(qi - 1).map(|f| Node::raw_value(&f.data)))
                    } else {
                        Ok(None)
                    }
                })
                .is_ok()
                && qi == queue.len();
            if !ok {
                self.flush_pending(out);
            }
        }
        let mut frames = std::mem::take(&mut self.pending);
        frames.push(main);
        self.run(event, frames, out);
    }

    fn run(&mut self, mut event: StreamEvent, frames: Vec<Frame>, out: &mut Vec<StreamEvent>) {
        let mut rec = Extras::new(self.origin.clone());
        let mut skips: Vec<String> = Vec::new();
        let mut qi = 0;
        let result = self
            .engine
            .step(&event, &mut |scope, mut canon, seq| match frames.get(qi) {
                Some(f) if matches(&canon, f) => {
                    qi += 1;
                    if !canon.has("sequence_number") {
                        canon.insert("sequence_number", Node::Val(Value::from(seq)));
                    }
                    Overrides::diff(&canon, &f.data, &[]).record(&mut rec, scope);
                    if f.event.as_deref() != Some(f.ty.as_str()) {
                        let key = if scope.is_empty() {
                            EVENT_KEY.to_owned()
                        } else {
                            format!("{scope}.{EVENT_KEY}")
                        };
                        let name = f.event.clone().map_or(Value::Null, Value::String);
                        form_set_insert(&mut rec, key, name);
                    }
                    Ok(Some(Node::raw_value(&f.data)))
                }
                _ => {
                    skips.push(scope.to_owned());
                    Ok(None)
                }
            });
        if result.is_err() {
            for f in &frames {
                self.frame_extra(out, f);
            }
            return;
        }
        if let Some(extras) = event_extras_mut(&mut event) {
            for (k, v) in rec.fields {
                extras.fields.insert(k, v);
            }
            for (k, v) in rec.form {
                extras.form.insert(k, v);
            }
            if !skips.is_empty() {
                extras.form.insert(
                    SKIP.to_owned(),
                    Value::Array(skips.into_iter().map(Value::String).collect()),
                );
            }
            if extras.origin.is_none() {
                extras.origin = Some(self.origin.clone());
            }
        }
        if let StreamEvent::BlockStart(b) = &mut event
            && let BlockHeader::ToolCall(h) = &mut b.block
        {
            h.id = self.ctx.original_tool_id(&h.id).to_owned();
            h.name = self.ctx.original_tool_name(&h.name).to_owned();
        }
        debug_assert!(event_extras(&event).is_some() || matches!(event, StreamEvent::Error(_)));
        out.push(event);
        for f in frames.iter().skip(qi) {
            self.frame_extra(out, f);
        }
    }
}

fn event_extras_mut(event: &mut StreamEvent) -> Option<&mut Extras> {
    match event {
        StreamEvent::ResponseStart(e) => Some(&mut e.extras),
        StreamEvent::BlockStart(e) => Some(&mut e.extras),
        StreamEvent::BlockDelta(e) => Some(&mut e.extras),
        StreamEvent::BlockStop(e) => Some(&mut e.extras),
        StreamEvent::ResponseDelta(e) => Some(&mut e.extras),
        StreamEvent::ResponseStop(e) => Some(&mut e.extras),
        _ => None,
    }
}

fn matches(canon: &Node, f: &Frame) -> bool {
    if canon.get_str("type").as_deref() != Some(f.ty.as_str()) {
        return false;
    }
    ["output_index", "content_index", "summary_index"]
        .iter()
        .all(|k| match (canon.get_i64(k), f.int(k)) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        })
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        let mut items = Vec::new();
        let result = self.parser.feed_into(bytes, &mut items);
        for item in items {
            if self.inert {
                break;
            }
            self.sse_item(item, &mut out);
        }
        if let Err(e) = result
            && !self.inert
        {
            self.fail(&mut out, e.into());
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        match self.parser.finish() {
            Ok(items) => {
                for item in items {
                    if self.inert {
                        break;
                    }
                    self.sse_item(item, &mut out);
                }
            }
            Err(e) => {
                self.release_held(&mut out);
                self.flush_pending(&mut out);
                self.fail(&mut out, e.into());
                return out;
            }
        }
        if !self.inert
            && let Some((err, frame)) = self.held_error.take()
        {
            self.run(StreamEvent::Error(err), vec![frame], &mut out);
            self.inert = true;
        }
        if !self.inert {
            self.release_held(&mut out);
            self.flush_pending(&mut out);
            if self.engine.open() {
                self.fail(
                    &mut out,
                    KppError::provider(
                        "The provider's stream ended before the response was complete.",
                    ),
                );
            }
        }
        self.inert = true;
        out
    }
}
