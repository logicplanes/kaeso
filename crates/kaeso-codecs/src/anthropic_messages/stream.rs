use std::collections::HashMap;

use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, BlockStart, BlockStop, Citation, Delta, Extras, KppError,
    Opaque, OpaqueKind, Origin, RawItem, RawJson, ReasoningHeader, ResponseDelta, ResponseStart,
    ResponseStop, StreamEvent, ToolCallHeader, Usage,
};
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};

use super::blocks::{Dec, decode_block};
use super::common::{D, is_empty_object, no_position};
use super::error::decode_stream_error;
use super::response::{decode_stop, decode_usage, decode_usage_with};
use super::wire::{
    BlockDeltaW, BlockStartW, BlockStopW, CitationsDeltaW, InputJsonDeltaW, MessageDeltaW,
    MessageStartW, SignatureDeltaW, TextDeltaW, ThinkingDeltaW, TypeOnlyW,
};
use crate::codec::{EgressCtx, StreamDecoder};
use crate::sse::{SseEvent, SseItem, SseParser};
use crate::wire::{Ext, add_nested_extra, extras_from, type_tag, verbatim_into_form};

pub(crate) const IN_START_FORM: &str = "in_start";
pub(crate) const CONTENT_BLOCK_FORM: &str = "content_block";
pub(crate) const INDEX_FORM: &str = "index";
pub(crate) const EVENT_FORM: &str = "event";
pub(crate) const SSE_FORM: &str = "sse";

const KNOWN_EVENTS: [&str; 8] = [
    "message_start",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
    "message_delta",
    "message_stop",
    "ping",
    "error",
];

pub(crate) fn default_start_block(header: &BlockHeader, visible_redacted: bool) -> Option<Value> {
    Some(match header {
        BlockHeader::Text | BlockHeader::Refusal => json!({"type": "text", "text": ""}),
        BlockHeader::ToolCall(h) => {
            json!({"type": "tool_use", "id": h.id, "name": h.name, "input": {}})
        }
        BlockHeader::Reasoning(h) => {
            match h
                .opaque
                .iter()
                .find(|o| o.kind == OpaqueKind::AnthropicRedactedThinking)
            {
                Some(o) if visible_redacted => json!({"type": "redacted_thinking", "data": o.data}),
                _ => json!({"type": "thinking", "thinking": "", "signature": ""}),
            }
        }
        _ => return None,
    })
}

fn framing(ev: &SseEvent, ty: &str, extras: &mut Extras) {
    if ev.event.as_deref() != Some(ty) {
        extras.form.insert(
            EVENT_FORM.into(),
            ev.event
                .as_ref()
                .map_or(Value::Null, |e| Value::from(e.as_str())),
        );
    }
    if ev.id.is_some() || ev.retry.is_some() || !ev.other.is_empty() {
        extras.form.insert(SSE_FORM.into(), sse_fields(ev));
    }
}

fn sse_fields(ev: &SseEvent) -> Value {
    let mut m = Map::new();
    if let Some(id) = &ev.id {
        m.insert("id".into(), id.as_str().into());
    }
    if let Some(retry) = ev.retry {
        m.insert("retry".into(), retry.into());
    }
    if !ev.other.is_empty() {
        m.insert("other".into(), json!(ev.other));
    }
    Value::Object(m)
}

#[derive(Serialize)]
struct FrameRaw<'a> {
    event: Option<&'a str>,
    data: &'a RawValue,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    data_text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry: Option<u64>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    other: &'a [(String, String)],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Tool,
    Reasoning,
    Unknown,
}

struct Open {
    kpp: u32,
    kind: Kind,
    closed: bool,
}

pub(crate) struct Decoder {
    parser: SseParser,
    ctx: EgressCtx,
    origin: Origin,
    blocks: HashMap<u32, Open>,
    next: u32,
    start_usage: Option<Usage>,
    started: bool,
    stopped: bool,
    inert: bool,
}

impl Decoder {
    pub(crate) fn new(ctx: &EgressCtx) -> Self {
        Decoder {
            parser: SseParser::new(),
            origin: ctx.origin(D),
            ctx: ctx.clone(),
            blocks: HashMap::new(),
            next: 0,
            start_usage: None,
            started: false,
            stopped: false,
            inert: false,
        }
    }

    fn items(&mut self, items: Vec<SseItem>, out: &mut Vec<StreamEvent>) {
        for item in items {
            if self.inert {
                return;
            }
            if let Err(e) = self.item(item, out) {
                self.fail(e, out);
            }
        }
    }

    fn fail(&mut self, e: KppError, out: &mut Vec<StreamEvent>) {
        let e = if e.origin.is_none() {
            e.with_origin(self.origin.clone())
        } else {
            e
        };
        out.push(StreamEvent::Error(e));
        self.inert = true;
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        match item {
            SseItem::Comment(text) => {
                let raw = RawJson::from_serialize(&json!({ "comment": text }))
                    .map_err(|e| bad(&no_position(&e)))?;
                out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
                Ok(())
            }
            SseItem::Event(ev) => self.event(ev, out),
        }
    }

    fn event(&mut self, ev: SseEvent, out: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        let data: Box<RawValue> = match serde_json::from_str(&ev.data) {
            Ok(d) => d,
            Err(e) => {
                if ev
                    .event
                    .as_deref()
                    .is_some_and(|n| KNOWN_EVENTS.contains(&n))
                {
                    return Err(bad(&format!(
                        "the data of a `{}` event is not JSON ({})",
                        ev.event.as_deref().unwrap_or_default(),
                        no_position(&e)
                    )));
                }
                let text = RawJson::from_serialize(&ev.data).map_err(|e| bad(&no_position(&e)))?;
                return self.extra(&ev, text.as_raw(), true, out);
            }
        };
        let ty = type_tag(&data).ok().flatten();
        let Some(ty) = ty else {
            if let Some(name) = ev.event.as_deref().filter(|n| KNOWN_EVENTS.contains(n)) {
                return Err(bad(&format!("the data of a `{name}` event has no `type`")));
            }
            return self.extra(&ev, &data, false, out);
        };
        self.check_order(&ty)?;
        let origin = self.origin.clone();
        let parse_err =
            |e: serde_json::Error| bad(&format!("invalid `{ty}` event: {}", no_position(&e)));
        match ty.as_str() {
            "message_start" => {
                let Ext {
                    known,
                    extra,
                    verbatim,
                } = Ext::<MessageStartW>::from_raw(&data).map_err(parse_err)?;
                let msg = known.message;
                let usage = decode_usage(&msg.known.usage, &origin).map_err(|e| bad(&e))?;
                self.start_usage = Some(usage.clone());
                let mut start = ResponseStart::new(msg.known.id, msg.known.model);
                start.usage = Some(usage);
                start.extras = extras_from(&origin, extra, verbatim);
                add_nested_extra(&mut start.extras, "message", msg.extra);
                verbatim_into_form(&mut start.extras, "message", msg.verbatim);
                framing(&ev, &ty, &mut start.extras);
                out.push(StreamEvent::ResponseStart(start));
            }
            "content_block_start" => {
                let w = Ext::<BlockStartW>::from_raw(&data).map_err(parse_err)?;
                self.block_start(&ev, &ty, w, out)?;
            }
            "content_block_delta" => {
                let w = Ext::<BlockDeltaW>::from_raw(&data).map_err(parse_err)?;
                self.block_delta(&ev, &ty, w, out)?;
            }
            "content_block_stop" => {
                let Ext {
                    known,
                    extra,
                    verbatim,
                } = Ext::<BlockStopW>::from_raw(&data).map_err(parse_err)?;
                let open = self.open(known.index)?;
                open.closed = true;
                let mut stop = BlockStop::new(open.kpp);
                stop.extras = extras_from(&origin, extra, verbatim);
                framing(&ev, &ty, &mut stop.extras);
                out.push(StreamEvent::BlockStop(stop));
            }
            "message_delta" => {
                let Ext {
                    known,
                    extra,
                    verbatim,
                } = Ext::<MessageDeltaW>::from_raw(&data).map_err(parse_err)?;
                let mut extras = extras_from(&origin, extra, verbatim);
                let d = known.delta;
                add_nested_extra(&mut extras, "delta", d.extra);
                verbatim_into_form(&mut extras, "delta", d.verbatim);
                let stop = decode_stop(
                    &mut extras,
                    "delta",
                    d.known.stop_reason,
                    d.known.stop_sequence,
                    true,
                );
                let usage = decode_usage_with(&known.usage, &origin, self.start_usage.as_ref())
                    .map_err(|e| bad(&e))?;
                let mut delta = ResponseDelta::new(stop, Some(usage));
                framing(&ev, &ty, &mut extras);
                delta.extras = extras;
                out.push(StreamEvent::ResponseDelta(delta));
            }
            "message_stop" => {
                if let Some(index) = self.open_block() {
                    return Err(bad(&format!(
                        "`message_stop` arrived while content block {index} was still open"
                    )));
                }
                let Ext {
                    extra, verbatim, ..
                } = Ext::<TypeOnlyW>::from_raw(&data).map_err(parse_err)?;
                let mut stop = ResponseStop::new();
                stop.extras = extras_from(&origin, extra, verbatim);
                framing(&ev, &ty, &mut stop.extras);
                out.push(StreamEvent::ResponseStop(stop));
                self.stopped = true;
            }
            "error" => {
                let mut err = decode_stream_error(&ev.data);
                err.origin = Some(origin);
                out.push(StreamEvent::Error(err));
                self.inert = true;
            }
            _ => return self.extra(&ev, &data, false, out),
        }
        Ok(())
    }

    fn check_order(&mut self, ty: &str) -> Result<(), KppError> {
        let lifecycle = matches!(
            ty,
            "message_start"
                | "content_block_start"
                | "content_block_delta"
                | "content_block_stop"
                | "message_delta"
                | "message_stop"
        );
        if !lifecycle {
            return Ok(());
        }
        if self.stopped {
            return Err(bad(&format!("`{ty}` arrived after `message_stop`")));
        }
        if ty == "message_start" {
            if self.started {
                return Err(bad("a second `message_start` arrived"));
            }
            self.started = true;
        } else if !self.started {
            return Err(bad(&format!("`{ty}` arrived before `message_start`")));
        }
        Ok(())
    }

    fn open_block(&self) -> Option<u32> {
        self.blocks
            .iter()
            .filter(|(_, o)| !o.closed)
            .map(|(i, _)| *i)
            .min()
    }

    fn extra(
        &self,
        ev: &SseEvent,
        data: &RawValue,
        data_text: bool,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), KppError> {
        let raw = RawJson::from_serialize(&FrameRaw {
            event: ev.event.as_deref(),
            data,
            data_text,
            id: ev.id.as_deref(),
            retry: ev.retry,
            other: &ev.other,
        })
        .map_err(|e| bad(&no_position(&e)))?;
        out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
        Ok(())
    }

    fn open(&mut self, index: u32) -> Result<&mut Open, KppError> {
        match self.blocks.get_mut(&index) {
            Some(open) if !open.closed => Ok(open),
            Some(_) => Err(bad(&format!("content block {index} was already stopped"))),
            None => Err(bad(&format!("content block {index} was never started"))),
        }
    }

    fn block_start(
        &mut self,
        ev: &SseEvent,
        ty: &str,
        w: Ext<BlockStartW>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), KppError> {
        let origin = self.origin.clone();
        let Ext {
            known,
            extra,
            verbatim,
        } = w;
        if self.blocks.contains_key(&known.index) {
            return Err(bad(&format!(
                "content block {} was started twice",
                known.index
            )));
        }
        let kpp = self.next;
        self.next = self.next.saturating_add(1);
        let cb = &known.content_block;
        let block = decode_block(
            cb,
            Dec {
                origin: &origin,
                ctx: None,
            },
        )
        .map_err(|e| bad(&e))?;
        let mut deltas = Vec::new();
        let (wire_header, kind) = match block {
            Block::Text(t) => {
                if !t.text.is_empty() {
                    deltas.push(Delta::Text(t.text));
                }
                deltas.extend(t.citations.into_iter().map(Delta::Citation));
                (BlockHeader::Text, Kind::Text)
            }
            Block::ToolCall(c) => {
                if !is_empty_object(c.arguments.as_raw()) {
                    deltas.push(Delta::ToolArguments(c.arguments.get().to_owned()));
                }
                (
                    BlockHeader::ToolCall(ToolCallHeader::new(c.id, c.name)),
                    Kind::Tool,
                )
            }
            Block::Reasoning(r) => {
                let mut header = ReasoningHeader::new(origin.clone());
                for o in r.opaque {
                    match o.kind {
                        OpaqueKind::AnthropicRedactedThinking => header.opaque.push(o),
                        _ if o.data.is_empty() => {}
                        _ => deltas.push(Delta::Opaque(o)),
                    }
                }
                let text: Vec<Delta> = r
                    .text
                    .into_iter()
                    .filter(|t| !t.is_empty())
                    .map(Delta::ReasoningText)
                    .collect();
                deltas.splice(0..0, text);
                (BlockHeader::Reasoning(header), Kind::Reasoning)
            }
            _ => (
                BlockHeader::Unknown(RawItem::new(origin.clone(), RawJson::from(cb.as_ref()))),
                Kind::Unknown,
            ),
        };
        let mut extras = extras_from(&origin, extra, verbatim);
        if known.index != kpp {
            extras.form.insert(INDEX_FORM.into(), known.index.into());
        }
        if let Some(default) = default_start_block(&wire_header, true) {
            let actual: Value =
                serde_json::from_str(cb.get()).map_err(|e| bad(&no_position(&e)))?;
            if actual != default {
                extras.form.insert(CONTENT_BLOCK_FORM.into(), actual);
            }
        }
        framing(ev, ty, &mut extras);
        let header = match wire_header {
            BlockHeader::ToolCall(h) => BlockHeader::ToolCall(ToolCallHeader::new(
                self.ctx.original_tool_id(&h.id),
                self.ctx.original_tool_name(&h.name),
            )),
            other => other,
        };
        let mut start = BlockStart::new(kpp, header);
        start.extras = extras;
        out.push(StreamEvent::BlockStart(start));
        for delta in deltas {
            let mut d = BlockDelta::new(kpp, delta);
            d.extras = Extras::new(origin.clone()).with_form(IN_START_FORM, Value::Bool(true));
            out.push(StreamEvent::BlockDelta(d));
        }
        self.blocks.insert(
            known.index,
            Open {
                kpp,
                kind,
                closed: false,
            },
        );
        Ok(())
    }

    fn block_delta(
        &mut self,
        ev: &SseEvent,
        ty: &str,
        w: Ext<BlockDeltaW>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), KppError> {
        let origin = self.origin.clone();
        let Ext {
            known,
            extra,
            verbatim,
        } = w;
        let (kpp, kind) = {
            let open = self.open(known.index)?;
            (open.kpp, open.kind)
        };
        let mut extras = extras_from(&origin, extra, verbatim);
        let raw = &known.delta;
        let dty = type_tag(raw).ok().flatten();
        let parse_err = |e: serde_json::Error| {
            bad(&format!("invalid content block delta: {}", no_position(&e)))
        };
        let unknown = || Delta::Unknown(RawItem::new(origin.clone(), RawJson::from(raw.as_ref())));
        let expected = match dty.as_deref() {
            Some("text_delta" | "citations_delta") => Some(Kind::Text),
            Some("input_json_delta") => Some(Kind::Tool),
            Some("thinking_delta" | "signature_delta") => Some(Kind::Reasoning),
            _ => None,
        };
        if kind != Kind::Unknown
            && let Some(expected) = expected
            && expected != kind
        {
            return Err(bad(&format!(
                "a `{}` arrived for content block {}, which is a {} block",
                dty.as_deref().unwrap_or_default(),
                known.index,
                match kind {
                    Kind::Text => "text",
                    Kind::Tool => "tool_use",
                    Kind::Reasoning => "thinking",
                    Kind::Unknown => "server",
                }
            )));
        }
        let delta = if kind == Kind::Unknown {
            unknown()
        } else {
            match dty.as_deref() {
                Some("text_delta") => {
                    let d = Ext::<TextDeltaW>::from_raw(raw).map_err(parse_err)?;
                    nest(&mut extras, d.extra, d.verbatim);
                    Delta::Text(d.known.text)
                }
                Some("input_json_delta") => {
                    let d = Ext::<InputJsonDeltaW>::from_raw(raw).map_err(parse_err)?;
                    nest(&mut extras, d.extra, d.verbatim);
                    Delta::ToolArguments(d.known.partial_json)
                }
                Some("thinking_delta") => {
                    let d = Ext::<ThinkingDeltaW>::from_raw(raw).map_err(parse_err)?;
                    nest(&mut extras, d.extra, d.verbatim);
                    Delta::ReasoningText(d.known.thinking)
                }
                Some("signature_delta") => {
                    let d = Ext::<SignatureDeltaW>::from_raw(raw).map_err(parse_err)?;
                    nest(&mut extras, d.extra, d.verbatim);
                    Delta::Opaque(Opaque::new(
                        origin.clone(),
                        OpaqueKind::AnthropicThinkingSignature,
                        d.known.signature,
                    ))
                }
                Some("citations_delta") => {
                    let d = Ext::<CitationsDeltaW>::from_raw(raw).map_err(parse_err)?;
                    nest(&mut extras, d.extra, d.verbatim);
                    Delta::Citation(Citation::new(
                        origin.clone(),
                        RawJson::from(d.known.citation),
                    ))
                }
                _ => unknown(),
            }
        };
        framing(ev, ty, &mut extras);
        let mut d = BlockDelta::new(kpp, delta);
        d.extras = extras;
        out.push(StreamEvent::BlockDelta(d));
        Ok(())
    }
}

fn nest(
    extras: &mut Extras,
    extra: indexmap::IndexMap<String, RawJson>,
    verbatim: indexmap::IndexMap<String, RawJson>,
) {
    add_nested_extra(extras, "delta", extra);
    verbatim_into_form(extras, "delta", verbatim);
}

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent an Anthropic Messages stream Kaeso cannot read: {detail}."
    ))
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        let mut items = Vec::new();
        let parsed = self.parser.feed_into(bytes, &mut items);
        self.items(items, &mut out);
        if let Err(e) = parsed
            && !self.inert
        {
            self.fail(e.into(), &mut out);
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        match self.parser.finish() {
            Ok(items) => self.items(items, &mut out),
            Err(e) => self.fail(e.into(), &mut out),
        }
        if !self.inert {
            if let Some(index) = self.open_block() {
                self.fail(
                    bad(&format!(
                        "the stream ended while content block {index} was still open"
                    )),
                    &mut out,
                );
            } else if !self.stopped {
                self.fail(bad("the stream ended before `message_stop`"), &mut out);
            }
        }
        self.inert = true;
        out
    }
}
