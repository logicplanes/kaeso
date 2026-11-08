use std::collections::HashMap;

use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, Extras, KppError, OpaqueKind,
    Origin, RawItem, RawJson, ReasoningHeader, ResponseDelta, ResponseStart, ResponseStop,
    StreamEvent, ToolCallHeader,
};
use serde_json::{Map, Value, json};

use super::D;
use super::error::{decode_error, error_body};
use super::parts::{Calls, PartDecoder, PartEncoder, carried_signature};
use super::response::{encode_usage, finish_reason, id_seed, provider_error, stop_from_wire};
use super::wire::{Raw, ResponseW};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList, StreamDecoder, StreamEncoder};
use crate::cross::{ReplyPolicy, reasoning_reply_policy, reply_policy};
use crate::keepalive::Pacer;
use crate::sse::{SseEvent, SseItem, SseParser, comment_extra, frame_extra, framing, set_framing};
use crate::wire::Ext;

/// The form marker of the first event decoded from a chunk: the chunk without its parts,
/// its finish reason and its final usage, which the events carry.
pub(crate) const CHUNK: &str = "chunk";
const SAME_PART: &str = "same_part";

// The google-genai SDK reads every line that doesn't start with `data: ` as part of a JSON
// error body (`_api_client.py`, `_iter_response_stream`), so an SSE comment would break it; it
// skips blank lines, and a blank line with no data before it dispatches no event (WHATWG).
pub(crate) const KEEPALIVE_FRAME: &[u8] = b"\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    Text,
    Reasoning,
}

pub(crate) struct Decoder {
    origin: Origin,
    parser: SseParser,
    started: bool,
    finished: bool,
    failed: bool,
    open: Option<(u32, Open)>,
    next: u32,
    calls: Calls,
    saw_calls: bool,
    seed: String,
    generated: usize,
}

impl Decoder {
    pub(crate) fn new(ctx: &EgressCtx) -> Self {
        Decoder {
            origin: ctx.origin(D),
            parser: SseParser::new(),
            started: false,
            finished: false,
            failed: false,
            open: None,
            next: 0,
            calls: Calls::default(),
            saw_calls: false,
            seed: String::new(),
            generated: 0,
        }
    }

    fn extras(&self) -> Extras {
        Extras::new(self.origin.clone())
    }

    fn close(&mut self, out: &mut Vec<StreamEvent>) {
        if let Some((index, _)) = self.open.take() {
            out.push(StreamEvent::BlockStop(BlockStop::new(index)));
        }
    }

    fn start(&mut self, header: BlockHeader, extras: Extras, out: &mut Vec<StreamEvent>) -> u32 {
        let index = self.next;
        self.next += 1;
        let mut s = BlockStart::new(index, header);
        s.extras = extras;
        out.push(StreamEvent::BlockStart(s));
        index
    }

    fn delta(index: u32, delta: Delta, extras: Extras, out: &mut Vec<StreamEvent>) {
        let mut d = BlockDelta::new(index, delta);
        d.extras = extras;
        out.push(StreamEvent::BlockDelta(d));
    }

    fn part(&mut self, raw: &Raw, out: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        let mut blocks = Vec::new();
        let mut dec = PartDecoder {
            origin: &self.origin,
            calls: &mut self.calls,
            id_seed: self.seed.clone(),
            generated: self.generated,
        };
        dec.part(raw, &mut blocks).map_err(provider_error)?;
        self.generated = dec.generated;
        let typed = matches!(
            blocks.last(),
            Some(Block::Text(_) | Block::Reasoning(_) | Block::ToolCall(_))
        );
        if !typed {
            self.close(out);
            let index = self.start(
                BlockHeader::Unknown(RawItem::new(self.origin.clone(), RawJson::from(raw.as_ref()))),
                self.extras(),
                out,
            );
            out.push(StreamEvent::BlockStop(BlockStop::new(index)));
            return Ok(());
        }
        for block in blocks {
            match block {
                Block::Text(t) => {
                    let index = match self.open {
                        Some((i, Open::Text)) => i,
                        _ => {
                            self.close(out);
                            let i = self.start(BlockHeader::Text, self.extras(), out);
                            self.open = Some((i, Open::Text));
                            i
                        }
                    };
                    Self::delta(index, Delta::Text(t.text), t.extras, out);
                }
                Block::Reasoning(r) if carried_signature(&r, &self.origin).is_some() => {
                    self.close(out);
                    let mut header = ReasoningHeader::new(self.origin.clone());
                    header.opaque = r.opaque;
                    let index = self.start(BlockHeader::Reasoning(header), self.extras(), out);
                    out.push(StreamEvent::BlockStop(BlockStop::new(index)));
                }
                Block::Reasoning(r) => {
                    let index = match self.open {
                        Some((i, Open::Reasoning)) => i,
                        _ => {
                            self.close(out);
                            let header = ReasoningHeader::new(self.origin.clone());
                            let i = self.start(BlockHeader::Reasoning(header), self.extras(), out);
                            self.open = Some((i, Open::Reasoning));
                            i
                        }
                    };
                    let sig = r.opaque_of(OpaqueKind::GeminiThoughtSignature).cloned();
                    Self::delta(index, Delta::ReasoningText(r.text.concat()), r.extras, out);
                    if let Some(o) = sig {
                        Self::delta(
                            index,
                            Delta::Opaque(o),
                            self.extras().with_form(SAME_PART, true.into()),
                            out,
                        );
                    }
                }
                Block::ToolCall(c) => {
                    self.close(out);
                    let index = self.start(
                        BlockHeader::ToolCall(ToolCallHeader::new(c.id, c.name)),
                        c.extras,
                        out,
                    );
                    Self::delta(
                        index,
                        Delta::ToolArguments(c.arguments.get().to_owned()),
                        self.extras(),
                        out,
                    );
                    out.push(StreamEvent::BlockStop(BlockStop::new(index)));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn chunk(&mut self, ev: &SseEvent, out: &mut Vec<StreamEvent>) {
        let first = out.len();
        let value: Value = match serde_json::from_str(&ev.data) {
            Ok(v @ Value::Object(_)) => v,
            _ => {
                self.extra_frame(ev, out);
                return;
            }
        };
        if value.get("error").is_some() {
            let mut err = decode_error(0, &HeaderList::new(), ev.data.as_bytes());
            err.status = value["error"]["code"].as_u64().and_then(|c| u16::try_from(c).ok());
            self.close(out);
            out.push(StreamEvent::Error(err));
            self.failed = true;
            return;
        }
        let w: Ext<ResponseW> = match Ext::from_text(&ev.data) {
            Ok(w) => w,
            Err(e) => {
                out.push(StreamEvent::Error(
                    provider_error(crate::wire::without_position(&e)).with_origin(self.origin.clone()),
                ));
                self.failed = true;
                return;
            }
        };
        if let Some(n) = w.candidates.as_ref().map(Vec::len)
            && n > 1
        {
            out.push(StreamEvent::Error(
                KppError::unsupported_by(
                    "more than one candidate",
                    D,
                    "Kaeso reads one candidate per reply; leave `candidateCount` out or set it to 1.",
                )
                .with_origin(self.origin.clone()),
            ));
            self.failed = true;
            return;
        }
        let mut env = value;
        let candidate = env
            .get_mut("candidates")
            .and_then(|c| c.get_mut(0))
            .and_then(Value::as_object_mut);
        let mut had_parts = false;
        let mut finish = None;
        if let Some(c) = candidate {
            finish = c.remove("finishReason");
            if let Some(content) = c.get_mut("content").and_then(Value::as_object_mut) {
                had_parts = content.remove("parts").is_some();
            }
        }
        let finish = finish.and_then(|f| f.as_str().map(str::to_owned));
        let usage = if finish.is_some() {
            env.as_object_mut().and_then(|e| e.remove("usageMetadata"))
        } else {
            None
        };
        if !self.started {
            self.started = true;
            let id = w.response_id.clone().unwrap_or_default();
            self.seed = id_seed(&id);
            let mut s = ResponseStart::new(id, w.model_version.clone().unwrap_or_default());
            s.extras = self.extras();
            out.push(StreamEvent::ResponseStart(s));
        }
        let parts: Vec<Raw> = w
            .known
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .map(|c| c.parts.clone())
            .unwrap_or_default();
        for raw in &parts {
            let before = out.len();
            if let Err(e) = self.part(raw, out) {
                out.push(StreamEvent::Error(e.with_origin(self.origin.clone())));
                self.failed = true;
                return;
            }
            self.saw_calls |= out.iter().skip(before).any(|e| {
                matches!(e, StreamEvent::BlockStart(s) if matches!(s.block, BlockHeader::ToolCall(_)))
            });
        }
        if let Some(f) = &finish {
            self.close(out);
            let mut extras = self.extras();
            let stop = stop_from_wire(f, self.saw_calls, &mut extras);
            let usage = match usage.as_ref().map(serde_json::value::to_raw_value) {
                Some(Ok(raw)) => match super::response::decode_usage(&raw, &self.origin) {
                    Ok(u) => Some(u),
                    Err(e) => {
                        out.push(StreamEvent::Error(e.with_origin(self.origin.clone())));
                        self.failed = true;
                        return;
                    }
                },
                _ => None,
            };
            let mut d = ResponseDelta::new(Some(stop), usage);
            d.extras = extras;
            out.push(StreamEvent::ResponseDelta(d));
            self.finished = true;
        }
        if out.len() == first {
            self.extra_frame(ev, out);
            return;
        }
        let mut marker = Map::new();
        marker.insert("env".into(), env);
        marker.insert("parts".into(), had_parts.into());
        if let Some(f) = framing(ev) {
            marker.insert("sse".into(), f);
        }
        if let Some(e) = out.get_mut(first).and_then(event_extras) {
            if e.origin.is_none() {
                *e = self.extras();
            }
            e.form.insert(CHUNK.into(), Value::Object(marker));
        }
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        match item {
            SseItem::Comment(text) => {
                if let Ok(raw) = comment_extra(&text) {
                    out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
                }
            }
            SseItem::Event(ev) if !ev.has_data || ev.event.is_some() => self.extra_frame(&ev, out),
            SseItem::Event(ev) => self.chunk(&ev, out),
        }
    }

    fn extra_frame(&self, ev: &SseEvent, out: &mut Vec<StreamEvent>) {
        if let Ok(raw) = frame_extra(ev) {
            out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
        }
    }
}

fn event_extras(event: &mut StreamEvent) -> Option<&mut Extras> {
    Some(match event {
        StreamEvent::ResponseStart(e) => &mut e.extras,
        StreamEvent::BlockStart(e) => &mut e.extras,
        StreamEvent::BlockDelta(e) => &mut e.extras,
        StreamEvent::BlockStop(e) => &mut e.extras,
        StreamEvent::ResponseDelta(e) => &mut e.extras,
        StreamEvent::ResponseStop(e) => &mut e.extras,
        _ => return None,
    })
}

fn extras_of(event: &StreamEvent) -> Option<&Extras> {
    Some(match event {
        StreamEvent::ResponseStart(e) => &e.extras,
        StreamEvent::BlockStart(e) => &e.extras,
        StreamEvent::BlockDelta(e) => &e.extras,
        StreamEvent::BlockStop(e) => &e.extras,
        StreamEvent::ResponseDelta(e) => &e.extras,
        StreamEvent::ResponseStop(e) => &e.extras,
        _ => return None,
    })
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.failed {
            return out;
        }
        let items = match self.parser.feed(bytes) {
            Ok(items) => items,
            Err(e) => {
                out.push(StreamEvent::Error(KppError::from(e).with_origin(self.origin.clone())));
                self.failed = true;
                return out;
            }
        };
        for item in items {
            if self.failed {
                break;
            }
            self.item(item, &mut out);
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.failed {
            return out;
        }
        match self.parser.finish() {
            Ok(items) => {
                for item in items {
                    self.item(item, &mut out);
                }
            }
            Err(e) => {
                self.close(&mut out);
                out.push(StreamEvent::Error(KppError::from(e).with_origin(self.origin.clone())));
                self.failed = true;
                return out;
            }
        }
        if self.failed {
            return out;
        }
        self.close(&mut out);
        if self.started && !self.finished {
            out.push(StreamEvent::Error(
                KppError::provider(
                    "The provider's stream ended before the reply had a finish reason.",
                )
                .with_origin(self.origin.clone()),
            ));
            self.failed = true;
            return out;
        }
        let mut stop = ResponseStop::new();
        stop.extras = self.extras();
        out.push(StreamEvent::ResponseStop(stop));
        self.failed = true;
        out
    }

    fn flush(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        self.close(&mut out);
        self.failed = true;
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Thought,
    Hidden,
    Tool,
    Other,
}

struct Chunk {
    env: Option<Value>,
    had_parts: bool,
    sse: Option<Value>,
    parts: Vec<Value>,
    finish: Option<String>,
    usage: Option<Value>,
}

pub(crate) struct Encoder {
    id: String,
    model: String,
    chunk: Option<Chunk>,
    pending: Option<String>,
    kinds: HashMap<u32, Kind>,
    tools: HashMap<u32, (String, String, String, Extras)>,
    pacer: Pacer,
    finished: bool,
    done: bool,
}

impl Encoder {
    pub(crate) fn new(_ctx: &EncodeCtx) -> Self {
        Encoder {
            id: String::new(),
            model: String::new(),
            chunk: None,
            pending: None,
            kinds: HashMap::new(),
            tools: HashMap::new(),
            pacer: Pacer::default(),
            finished: false,
            done: false,
        }
    }

    fn write(&mut self, out: &mut Vec<u8>) -> Result<(), KppError> {
        let Some(chunk) = self.chunk.take() else {
            return Ok(());
        };
        let value = match chunk.env {
            Some(mut env) => {
                if let Some(c) = env
                    .get_mut("candidates")
                    .and_then(|c| c.get_mut(0))
                    .and_then(Value::as_object_mut)
                {
                    if chunk.had_parts
                        && let Some(content) = c.get_mut("content").and_then(Value::as_object_mut)
                    {
                        content.insert("parts".into(), Value::Array(chunk.parts));
                    }
                    if let Some(f) = chunk.finish {
                        self.finished = true;
                        c.insert("finishReason".into(), Value::String(f));
                    }
                }
                if let (Some(u), Some(e)) = (chunk.usage, env.as_object_mut())
                    && !e.contains_key("usageMetadata")
                {
                    e.insert("usageMetadata".into(), u);
                }
                env
            }
            None => {
                let mut parts = chunk.parts;
                if parts.is_empty() {
                    parts.push(json!({"text": ""}));
                }
                let mut candidate = json!({
                    "content": {"parts": parts, "role": "model"},
                    "index": 0,
                });
                if let Some(f) = chunk.finish {
                    self.finished = true;
                    candidate["finishReason"] = Value::String(f);
                }
                let mut v = json!({"candidates": [candidate]});
                if let Some(u) = chunk.usage {
                    v["usageMetadata"] = u;
                }
                v["modelVersion"] = Value::String(self.model.clone());
                v["responseId"] = Value::String(self.id.clone());
                v
            }
        };
        let data = serde_json::to_string(&value).map_err(provider_error)?;
        let mut ev = SseEvent::new(None, data);
        if let Some(f) = &chunk.sse {
            set_framing(&mut ev, f);
        }
        crate::sse::write_item(out, &SseItem::Event(ev));
        Ok(())
    }

    fn chunk(&mut self) -> &mut Chunk {
        self.chunk.get_or_insert(Chunk {
            env: None,
            had_parts: true,
            sse: None,
            parts: Vec::new(),
            finish: None,
            usage: None,
        })
    }

    fn push(&mut self, mut part: Value, out: &mut Vec<u8>) -> Result<(), KppError> {
        if let (Some(sig), Some(p)) = (self.pending.take(), part.as_object_mut())
            && !p.contains_key("thoughtSignature")
        {
            p.insert("thoughtSignature".into(), Value::String(sig));
        }
        let foreign = self.chunk.as_ref().is_none_or(|c| c.env.is_none());
        self.chunk().parts.push(part);
        if foreign {
            self.write(out)?;
        }
        Ok(())
    }

    fn part_with(&self, block: &Block) -> Result<Value, KppError> {
        let target = Origin::new(D);
        let mut enc = PartEncoder::new(&target);
        enc.reply = true;
        enc.block(block, "")?;
        Ok(enc.parts.into_iter().next().unwrap_or(Value::Null))
    }

    fn event(&mut self, event: &StreamEvent, out: &mut Vec<u8>) -> Result<(), KppError> {
        if let Some(marker) = extras_of(event)
            .and_then(|e| e.form_value(D, CHUNK))
            .and_then(Value::as_object)
        {
            self.write(out)?;
            self.chunk = Some(Chunk {
                env: marker.get("env").cloned(),
                had_parts: marker.get("parts").and_then(Value::as_bool).unwrap_or(false),
                sse: marker.get("sse").cloned(),
                parts: Vec::new(),
                finish: None,
                usage: None,
            });
        }
        match event {
            StreamEvent::ResponseStart(s) => {
                self.id.clone_from(&s.id);
                self.model.clone_from(&s.model);
            }
            StreamEvent::BlockStart(s) => {
                let kind = match &s.block {
                    BlockHeader::Text | BlockHeader::Refusal => Kind::Text,
                    BlockHeader::Reasoning(h) => {
                        if reasoning_reply_policy(h.origin.as_ref(), D) != ReplyPolicy::Keep {
                            Kind::Hidden
                        } else if let Some(sig) = h
                            .opaque
                            .iter()
                            .find(|o| o.kind == OpaqueKind::GeminiThoughtSignature && o.origin.is(D))
                        {
                            self.pending = Some(sig.data.clone());
                            Kind::Other
                        } else {
                            Kind::Thought
                        }
                    }
                    BlockHeader::ToolCall(h) => {
                        self.tools.insert(
                            s.index,
                            (h.id.clone(), h.name.clone(), String::new(), s.extras.clone()),
                        );
                        Kind::Tool
                    }
                    BlockHeader::Unknown(item) => {
                        if reply_policy(Some(&item.origin), D) == ReplyPolicy::Keep {
                            let part: Value = item.raw.to_value().map_err(provider_error)?;
                            self.push(part, out)?;
                        }
                        Kind::Other
                    }
                    _ => Kind::Hidden,
                };
                self.kinds.insert(s.index, kind);
            }
            StreamEvent::BlockDelta(d) => {
                let kind = self.kinds.get(&d.index).cloned().unwrap_or(Kind::Hidden);
                match (&kind, &d.delta) {
                    (Kind::Text, Delta::Text(t) | Delta::Refusal(t)) => {
                        let mut block = kaeso_kpp::TextBlock::new(t.clone());
                        block.extras = d.extras.clone();
                        let part = self.part_with(&Block::Text(block))?;
                        self.push(part, out)?;
                    }
                    (Kind::Thought, Delta::ReasoningText(t)) => {
                        let mut part = json!({"text": t, "thought": true});
                        if d.extras.is_from(D)
                            && let Some(p) = part.as_object_mut()
                        {
                            for (k, v) in crate::wire::extra_for(&d.extras, D) {
                                p.insert(k, v.to_value().map_err(provider_error)?);
                            }
                        }
                        self.push(part, out)?;
                    }
                    (Kind::Thought, Delta::Opaque(o))
                        if o.kind == OpaqueKind::GeminiThoughtSignature =>
                    {
                        let same = d.extras.form_flag(D, SAME_PART);
                        let last = self
                            .chunk
                            .as_mut()
                            .and_then(|c| c.parts.last_mut())
                            .and_then(Value::as_object_mut);
                        match last {
                            Some(p) if same => {
                                p.insert("thoughtSignature".into(), Value::String(o.data.clone()));
                            }
                            _ => {
                                let part = json!({"text": "", "thought": true, "thoughtSignature": o.data});
                                self.push(part, out)?;
                            }
                        }
                    }
                    (Kind::Tool, Delta::ToolArguments(a)) => {
                        if let Some(t) = self.tools.get_mut(&d.index) {
                            t.2.push_str(a);
                        }
                    }
                    _ => {}
                }
            }
            StreamEvent::BlockStop(s) => {
                if let Some((id, name, args, extras)) = self.tools.remove(&s.index) {
                    let arguments = if args.trim().is_empty() {
                        RawJson::empty_object()
                    } else {
                        RawJson::from_string(args.clone()).unwrap_or_else(|_| {
                            RawJson::from_serialize(&json!({ "input": args }))
                                .unwrap_or_else(|_| RawJson::empty_object())
                        })
                    };
                    let mut call = kaeso_kpp::ToolCallBlock::new(id, name, arguments);
                    call.extras = extras;
                    let part = self.part_with(&Block::ToolCall(call))?;
                    self.push(part, out)?;
                }
            }
            StreamEvent::ResponseDelta(d) => {
                if let Some(stop) = &d.stop {
                    self.chunk().finish = Some(finish_reason(stop, &d.extras));
                }
                if let Some(u) = &d.usage {
                    self.chunk().usage = Some(encode_usage(u));
                }
            }
            StreamEvent::ResponseStop(_) => {
                self.finish_chunk(out)?;
                self.done = true;
            }
            StreamEvent::Error(e) => {
                self.write(out)?;
                let body = error_body(e, D);
                let data = serde_json::to_string(&body).map_err(provider_error)?;
                crate::sse::write_event(out, None, &data);
                self.done = true;
            }
            StreamEvent::Extra(item) => {
                if item.origin.is(D) {
                    self.write(out)?;
                    crate::sse::write_frame_extra(out, item.raw.as_raw()).map_err(provider_error)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn finish_chunk(&mut self, out: &mut Vec<u8>) -> Result<(), KppError> {
        if let Some(sig) = self.pending.take() {
            self.chunk()
                .parts
                .push(json!({"text": "", "thoughtSignature": sig}));
        }
        if !self.finished && self.chunk.as_ref().is_none_or(|c| c.finish.is_none()) {
            self.chunk().finish = Some("STOP".into());
        }
        self.write(out)
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if self.done {
            return Ok(out);
        }
        self.event(event, &mut out)?;
        self.pacer.pace(event, D, &mut out, KEEPALIVE_FRAME);
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if !self.done {
            self.finish_chunk(&mut out)?;
            self.done = true;
        }
        Ok(out)
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.done {
            return Vec::new();
        }
        self.pacer.reset();
        KEEPALIVE_FRAME.to_vec()
    }
}
