use indexmap::IndexMap;
use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, Extras, KppError, Origin, RawItem,
    RawJson, ReasoningHeader, ResponseDelta, ResponseStart, ResponseStop, StreamEvent,
    ToolCallHeader, Usage,
};
use serde_json::{Map, Value};

use super::DIALECT;
use super::convert::{nulls_to_verbatim, parse_ext, to_raw};
use super::error::{decode_body, with_detail};
use super::profile::ReplyReasoning;
use super::response::{
    decode_finish, decode_usage, plain_text_part, take_reasoning_field, thinking_part,
};
use super::wire::{ChunkChoice, ChunkWire, ToolCallChunk};
use crate::codec::{EgressCtx, StreamDecoder};
use crate::sse::{SseEvent, SseItem, SseParser};
use crate::wire::Ext;
use crate::wire::StringOr;

pub(crate) const FORM_CHUNK: &str = "chunk";
pub(crate) const FORM_CONTENT_SLOTS: &str = "cs";
pub(crate) const FORM_REASONING_SLOT: &str = "r";
pub(crate) const FORM_REASONING_FIELD: &str = "rf";
pub(crate) const FORM_EMPTY_TEXT: &str = "et";
pub(crate) const FORM_IN_CHUNK: &str = "in_chunk";
pub(crate) const FORM_ENV: &str = "env";
pub(crate) const FORM_ENV_ABSENT: &str = "env_absent";
pub(crate) const FORM_TOOL_CALL: &str = "tc";
pub(crate) const FORM_NO_DONE: &str = "no_done";
pub(crate) const FORM_OBJECT: &str = "object";
pub(crate) const FORM_SSE: &str = "sse";
pub(crate) const FORM_RUNNING_USAGE: &str = "u";
const FINISH_ERROR: &str = "error";

pub(crate) const ENVELOPE_KEYS: &[&str] = &[
    "id",
    "object",
    "created",
    "model",
    "service_tier",
    "system_fingerprint",
];
pub(crate) const ENVELOPE_FIELDS: &[&str] = &["created", "service_tier", "system_fingerprint"];
pub(crate) const CHUNK_OBJECT: &str = "chat.completion.chunk";

pub(crate) type Obj = IndexMap<String, RawJson>;

pub(crate) fn start_envelope(start: &ResponseStart) -> Obj {
    let mut env = Obj::new();
    if let Ok(id) = RawJson::from_serialize(&start.id) {
        env.insert("id".to_owned(), id);
    }
    match start.extras.form_value(DIALECT, FORM_OBJECT) {
        None => {
            if let Ok(object) = RawJson::from_serialize(CHUNK_OBJECT) {
                env.insert("object".to_owned(), object);
            }
        }
        Some(Value::Null) => {}
        Some(other) => {
            if let Ok(object) = RawJson::from_serialize(other) {
                env.insert("object".to_owned(), object);
            }
        }
    }
    if let Some(created) = start.extras.fields.get("created") {
        env.insert("created".to_owned(), created.clone());
    }
    if let Ok(model) = RawJson::from_serialize(&start.model) {
        env.insert("model".to_owned(), model);
    }
    for key in ["service_tier", "system_fingerprint"] {
        if let Some(value) = start.extras.fields.get(key) {
            env.insert(key.to_owned(), value.clone());
        }
    }
    env
}

fn same_raw(a: &RawJson, b: &RawJson) -> bool {
    a.get() == b.get() || a.to_value().ok() == b.to_value().ok()
}

pub(crate) fn obj_value(obj: &Obj) -> Value {
    Value::Object(
        obj.iter()
            .filter_map(|(k, v)| Some((k.clone(), v.to_value().ok()?)))
            .collect(),
    )
}

pub(crate) fn value_obj(value: Option<&Value>) -> Obj {
    match value {
        Some(Value::Object(map)) => map
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), RawJson::from_serialize(v).ok()?)))
            .collect(),
        _ => Obj::new(),
    }
}

struct ToolState {
    block: u32,
    id: String,
    unknown: bool,
    complete: bool,
}

struct Held {
    env: Option<Obj>,
    events: Vec<StreamEvent>,
}

fn env_form(env: &Obj, base: &Obj, form: &mut Map<String, Value>) {
    let mut set = Map::new();
    let mut absent = Vec::new();
    for key in ENVELOPE_KEYS {
        match (env.get(*key), base.get(*key)) {
            (Some(now), Some(then)) if same_raw(now, then) => {}
            (None, None) => {}
            (Some(now), _) => {
                set.insert((*key).to_owned(), now.to_value().unwrap_or(Value::Null));
            }
            (None, Some(_)) => absent.push(Value::String((*key).to_owned())),
        }
    }
    if !set.is_empty() {
        form.insert(FORM_ENV.to_owned(), Value::Object(set));
    }
    if !absent.is_empty() {
        form.insert(FORM_ENV_ABSENT.to_owned(), Value::Array(absent));
    }
}

fn env_text(env: &Obj, key: &str) -> String {
    env.get(key)
        .and_then(|v| v.parse::<String>().ok())
        .unwrap_or_default()
}

pub(crate) struct ChatStreamDecoder {
    parser: SseParser,
    origin: Origin,
    tool_names: IndexMap<String, String>,
    started: bool,
    start_open: bool,
    base: Option<Obj>,
    chunk_env: Option<Obj>,
    queue: Vec<Held>,
    next_block: u32,
    text: Option<u32>,
    refusal: Option<u32>,
    reasoning: Option<u32>,
    thinking_parts: bool,
    reasoning_fields: &'static [&'static str],
    error_detail: &'static [&'static str],
    tools: IndexMap<u32, ToolState>,
    running_usage: Option<Usage>,
    finished: bool,
    done: bool,
    inert: bool,
}

impl ChatStreamDecoder {
    pub(crate) fn new(ctx: &EgressCtx) -> Self {
        ChatStreamDecoder {
            parser: SseParser::new(),
            origin: ctx.origin(DIALECT),
            tool_names: ctx.tool_names.clone(),
            started: false,
            start_open: false,
            base: None,
            chunk_env: None,
            queue: Vec::new(),
            next_block: 0,
            text: None,
            refusal: None,
            reasoning: None,
            thinking_parts: ctx.chat_profile.reply.reasoning == ReplyReasoning::ThinkingParts,
            reasoning_fields: ctx.chat_profile.reply.reasoning_fields,
            error_detail: ctx.chat_profile.reply.error_detail,
            tools: IndexMap::new(),
            running_usage: None,
            finished: false,
            done: false,
            inert: false,
        }
    }

    fn extras(&self) -> Extras {
        Extras::new(self.origin.clone())
    }

    fn fail(&mut self, out: &mut Vec<StreamEvent>, err: KppError) {
        out.push(StreamEvent::Error(err.with_origin(self.origin.clone())));
        self.inert = true;
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        if self.inert {
            return;
        }
        let mut events = Vec::new();
        match item {
            SseItem::Comment(text) => {
                let raw = crate::sse::comment_extra(&text);
                self.extra(raw, &mut events);
            }
            SseItem::Event(event) => self.event(event, &mut events),
        }
        let env = self.chunk_env.take();
        self.queue.push(Held { env, events });
        if self.inert || !self.holding() {
            self.release(out);
        }
    }

    fn holding(&self) -> bool {
        self.start_open || self.tools.values().any(|t| !t.complete)
    }

    fn release_all(&mut self, out: &mut Vec<StreamEvent>) {
        self.start_open = false;
        for tool in self.tools.values_mut() {
            tool.complete = true;
        }
        self.release(out);
    }

    fn release(&mut self, out: &mut Vec<StreamEvent>) {
        for held in std::mem::take(&mut self.queue) {
            let mut events = held.events;
            if self.base.is_none()
                && let Some(StreamEvent::ResponseStart(start)) = events
                    .iter()
                    .find(|e| matches!(e, StreamEvent::ResponseStart(_)))
            {
                self.base = Some(start_envelope(start));
            }
            let response_id = self
                .base
                .as_ref()
                .map(|b| env_text(b, "id"))
                .unwrap_or_default();
            for event in &mut events {
                give_call_an_id(event, &response_id);
            }
            if let (Some(env), Some(base)) = (&held.env, &self.base)
                && let Some(Value::Object(form)) = events
                    .iter_mut()
                    .filter_map(event_extras_mut)
                    .find_map(|e| e.form.get_mut(FORM_CHUNK))
            {
                env_form(env, base, form);
            }
            out.extend(events);
        }
    }

    fn queued_mut(&mut self) -> impl Iterator<Item = &mut StreamEvent> {
        self.queue.iter_mut().flat_map(|h| h.events.iter_mut())
    }

    fn resolve_start(&mut self, env: &Obj, content: bool) {
        let id = env_text(env, "id");
        let model = env_text(env, "model");
        let mut complete = false;
        for event in self.queued_mut() {
            if let StreamEvent::ResponseStart(start) = event {
                if start.id.is_empty() {
                    start.id = id.clone();
                }
                if start.model.is_empty() {
                    start.model = model.clone();
                }
                complete = !start.id.is_empty() && !start.model.is_empty();
            }
        }
        if complete || content {
            self.start_open = false;
        }
    }

    fn extra(&mut self, raw: Result<RawJson, serde_json::Error>, out: &mut Vec<StreamEvent>) {
        match raw {
            Ok(raw) => out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw))),
            Err(e) => self.fail(out, KppError::provider(e.to_string())),
        }
    }

    fn event(&mut self, event: SseEvent, out: &mut Vec<StreamEvent>) {
        if event.has_data && event.event.as_deref() == Some("error") {
            let err = with_detail(decode_body(None, event.data.as_bytes()), self.error_detail);
            self.fail(out, err);
            return;
        }
        let framing = crate::sse::framing(&event);
        let plain = event.event.is_none() && !(framing.is_some() && event.data == "[DONE]");
        if !plain || self.done || event.data.is_empty() {
            self.extra(crate::sse::frame_extra(&event), out);
            return;
        }
        if event.data == "[DONE]" && !self.finished {
            self.fail(
                out,
                KppError::provider(
                    "The provider's OpenAI Chat Completions stream ended before the response was complete.",
                ),
            );
            return;
        }
        if event.data == "[DONE]" {
            self.close_blocks(out);
            self.report_running_usage(out);
            let mut stop = ResponseStop::new();
            stop.extras = self.extras();
            out.push(StreamEvent::ResponseStop(stop));
            self.done = true;
            return;
        }
        let raw: Box<serde_json::value::RawValue> = match serde_json::from_str(&event.data) {
            Ok(raw) => raw,
            Err(e) => {
                self.fail(
                    out,
                    KppError::provider(format!(
                        "The provider sent a streamed chunk that is not JSON: {}.",
                        crate::wire::without_position(&e)
                    )),
                );
                return;
            }
        };
        if is_error_frame(&raw) {
            let err = with_detail(decode_body(None, event.data.as_bytes()), self.error_detail);
            self.fail(out, err);
            return;
        }
        match self.chunk(&raw, framing) {
            Ok(events) if events.is_empty() => self.extra(crate::sse::frame_extra(&event), out),
            Ok(events) => out.extend(events),
            Err(err) => self.fail(out, err),
        }
    }

    fn close_blocks(&mut self, out: &mut Vec<StreamEvent>) {
        let mut open: Vec<u32> = self
            .tools
            .values()
            .map(|t| t.block)
            .chain(self.text)
            .chain(self.refusal)
            .chain(self.reasoning)
            .collect();
        open.sort_unstable();
        for index in open {
            let mut stop = BlockStop::new(index);
            stop.extras = self.extras();
            out.push(StreamEvent::BlockStop(stop));
        }
        self.text = None;
        self.refusal = None;
        self.reasoning = None;
        self.tools.clear();
    }

    fn report_running_usage(&mut self, out: &mut Vec<StreamEvent>) {
        if let Some(usage) = self.running_usage.take() {
            let mut delta = ResponseDelta::new(None, Some(usage));
            delta.extras = self.extras();
            out.push(StreamEvent::ResponseDelta(delta));
        }
    }

    fn stop_reasoning(&mut self, out: &mut Vec<StreamEvent>) {
        if let Some(index) = self.reasoning.take() {
            let mut stop = BlockStop::new(index);
            stop.extras = self.extras();
            out.push(StreamEvent::BlockStop(stop));
        }
    }

    fn reasons_first(&self) -> bool {
        self.thinking_parts || !self.reasoning_fields.is_empty()
    }

    fn new_block(&mut self) -> u32 {
        let index = self.next_block;
        self.next_block = self.next_block.saturating_add(1);
        index
    }

    fn chunk(
        &mut self,
        raw: &serde_json::value::RawValue,
        framing: Option<Value>,
    ) -> Result<Vec<StreamEvent>, KppError> {
        let chunk: Ext<ChunkWire> = parse_ext(raw, "stream chunk", true)?;
        let Ext {
            known,
            mut extra,
            mut verbatim,
            ..
        } = chunk;
        let mut env = Obj::new();
        for key in ENVELOPE_KEYS {
            if let Some(value) = extra.shift_remove(*key) {
                env.insert((*key).to_owned(), value);
            }
        }
        let obfuscation = extra.shift_remove("obfuscation");
        nulls_to_verbatim(&mut extra, &mut verbatim);

        let mut form = Map::new();
        let mut events = Vec::new();
        let mut fields = extra;
        let first = !self.started;
        if first {
            self.started = true;
            let mut start = ResponseStart::new(env_text(&env, "id"), env_text(&env, "model"));
            self.start_open = start.id.is_empty() || start.model.is_empty();
            start.extras = self.extras();
            for key in ENVELOPE_FIELDS {
                if let Some(value) = env.get(*key) {
                    start.extras.fields.insert((*key).to_owned(), value.clone());
                }
            }
            match env.get("object") {
                Some(o) if o.parse::<String>().ok().as_deref() == Some(CHUNK_OBJECT) => {}
                Some(o) => {
                    start
                        .extras
                        .form
                        .insert(FORM_OBJECT.to_owned(), o.to_value().unwrap_or(Value::Null));
                }
                None => {
                    start
                        .extras
                        .form
                        .insert(FORM_OBJECT.to_owned(), Value::Null);
                }
            }
            events.push(StreamEvent::ResponseStart(start));
        }
        if let Some(obf) = obfuscation {
            form.insert("obf".to_owned(), obf.to_value().unwrap_or(Value::Null));
        }
        if let Some(framing) = framing {
            form.insert(FORM_SSE.to_owned(), framing);
        }
        if !verbatim.is_empty() {
            form.insert("kv".to_owned(), obj_value(&verbatim));
        }

        let mut stop_usage = None;
        match known.choices.as_deref() {
            None => {
                form.insert("ch".to_owned(), Value::String("absent".to_owned()));
            }
            Some([]) => {
                form.insert("ch".to_owned(), Value::String("empty".to_owned()));
            }
            Some([choice]) => {
                let finish = self.choice(choice, &mut form, &mut fields, &mut events)?;
                if finish.as_deref() == Some(FINISH_ERROR) {
                    let raw = serde_json::from_str::<Value>(raw.get()).unwrap_or(Value::Null);
                    return Err(KppError::provider(
                        "The provider ended the stream with `finish_reason: \"error\"` and no details.",
                    )
                    .with_raw(raw));
                }
                if let Some(finish) = finish {
                    self.close_blocks(&mut events);
                    self.finished = true;
                    stop_usage = Some(Some(decode_finish(&finish)));
                }
            }
            Some(_) => {
                return Err(KppError::unsupported_by(
                    "n",
                    DIALECT,
                    "The provider streamed several choices, but Kaeso carries exactly one; do not request `n` > 1.",
                ));
            }
        }
        let mut usage = None;
        if let Some(raw) = known.usage {
            let decoded = decode_usage(&raw, &self.origin)?;
            if self.finished {
                self.running_usage = None;
                usage = Some(decoded);
            } else {
                form.insert(
                    FORM_RUNNING_USAGE.to_owned(),
                    raw.to_value().unwrap_or(Value::Null),
                );
                self.running_usage = Some(decoded);
            }
        }
        if stop_usage.is_some() || usage.is_some() {
            let mut delta = ResponseDelta::new(stop_usage.flatten(), usage);
            delta.extras = self.extras();
            events.push(StreamEvent::ResponseDelta(delta));
        }

        if first {
            self.start_open &= events.len() == 1;
        } else if self.start_open {
            self.resolve_start(&env, !events.is_empty());
        }
        self.chunk_env = Some(env);
        if events.len() > 1 {
            form.insert("n".to_owned(), Value::from(events.len()));
        }
        for event in events.iter_mut().skip(1) {
            if let Some(extras) = event_extras_mut(event) {
                extras
                    .form
                    .insert(FORM_IN_CHUNK.to_owned(), Value::Bool(true));
            }
        }
        if let Some(first) = events.first_mut().and_then(event_extras_mut) {
            for (key, value) in fields {
                first.fields.insert(key, value);
            }
            first
                .form
                .insert(FORM_CHUNK.to_owned(), Value::Object(form));
        }
        Ok(events)
    }

    fn choice(
        &mut self,
        raw: &RawJson,
        form: &mut Map<String, Value>,
        fields: &mut Obj,
        events: &mut Vec<StreamEvent>,
    ) -> Result<Option<String>, KppError> {
        let choice: Ext<ChunkChoice> = parse_ext(raw.as_raw(), "stream choice", true)?;
        let Ext {
            known,
            mut extra,
            mut verbatim,
            ..
        } = choice;
        if known.index != 0 {
            return Err(KppError::unsupported_by(
                "n",
                DIALECT,
                "The provider streamed a choice other than the first; do not request `n` > 1.",
            ));
        }
        nulls_to_verbatim(&mut extra, &mut verbatim);
        let mut finish_reason = known.finish_reason;
        if finish_reason.as_deref() == Some("") {
            finish_reason = None;
            if let Ok(empty) = RawJson::from_string("\"\"".to_owned()) {
                verbatim.insert("finish_reason".to_owned(), empty);
            }
        }
        if !verbatim.is_empty() {
            form.insert("cv".to_owned(), obj_value(&verbatim));
        }
        for (key, value) in extra {
            fields.insert(format!("choice.{key}"), value);
        }
        let Ext {
            known: delta,
            extra: mut d_extra,
            verbatim: mut d_verbatim,
            ..
        } = known.delta;
        nulls_to_verbatim(&mut d_extra, &mut d_verbatim);
        if !d_verbatim.is_empty() {
            form.insert("dv".to_owned(), obj_value(&d_verbatim));
        }
        if let Some(role) = delta.role {
            form.insert("role".to_owned(), Value::String(role));
        }
        let mut thoughts: Vec<String> = Vec::new();
        let text = match delta.content {
            Some(StringOr::String(text)) => Some(text),
            Some(StringOr::List(parts)) => {
                let mut text: Option<String> = None;
                let mut slots = Vec::new();
                let mut others = Vec::new();
                for part in parts {
                    if let Some(t) = plain_text_part(&part) {
                        slots.push(Value::from(t.len()));
                        text.get_or_insert_with(String::new).push_str(&t);
                    } else if let Some(thought) =
                        self.thinking_parts.then(|| thinking_part(&part)).flatten()
                    {
                        let mut slot = thought.rest;
                        slot.insert(
                            FORM_REASONING_SLOT.to_owned(),
                            thought.text.iter().map(|t| Value::from(t.len())).collect(),
                        );
                        slots.push(Value::Object(slot));
                        thoughts.extend(thought.text);
                    } else {
                        slots.push(Value::Null);
                        others.push(part);
                    }
                }
                form.insert(FORM_CONTENT_SLOTS.to_owned(), Value::Array(slots));
                if !others.is_empty() {
                    fields.insert("delta.content".to_owned(), to_raw(&others)?);
                }
                text
            }
            None => None,
        };
        if thoughts.is_empty()
            && let Some((field, thought)) =
                take_reasoning_field(self.reasoning_fields, &mut d_extra)
        {
            form.insert(FORM_REASONING_FIELD.to_owned(), Value::from(field));
            thoughts.push(thought);
        }
        for (key, value) in d_extra {
            fields.insert(format!("delta.{key}"), value);
        }
        for thought in thoughts.into_iter().filter(|t| !t.is_empty()) {
            let index = match self.reasoning {
                Some(index) => index,
                None => {
                    let index = self.new_block();
                    self.reasoning = Some(index);
                    let mut start = BlockStart::new(
                        index,
                        BlockHeader::Reasoning(ReasoningHeader::new(self.origin.clone())),
                    );
                    start.extras = self.extras();
                    events.push(StreamEvent::BlockStart(start));
                    index
                }
            };
            let mut d = BlockDelta::new(index, Delta::ReasoningText(thought));
            d.extras = self.extras();
            events.push(StreamEvent::BlockDelta(d));
        }
        let text = match text {
            Some(t) if t.is_empty() && self.text.is_none() && self.reasons_first() => {
                form.insert(FORM_EMPTY_TEXT.to_owned(), Value::Bool(true));
                None
            }
            other => other,
        };
        if text.is_some() || delta.refusal.is_some() || !delta.tool_calls.is_empty() {
            self.stop_reasoning(events);
        }
        if let Some(text) = text {
            let index = match self.text {
                Some(index) => index,
                None => {
                    let index = self.new_block();
                    self.text = Some(index);
                    let mut start = BlockStart::new(index, BlockHeader::Text);
                    start.extras = self.extras();
                    events.push(StreamEvent::BlockStart(start));
                    index
                }
            };
            let mut d = BlockDelta::new(index, Delta::Text(text));
            d.extras = self.extras();
            events.push(StreamEvent::BlockDelta(d));
        }
        if let Some(text) = delta.refusal {
            let index = match self.refusal {
                Some(index) => index,
                None => {
                    let index = self.new_block();
                    self.refusal = Some(index);
                    let mut start = BlockStart::new(index, BlockHeader::Refusal);
                    start.extras = self.extras();
                    events.push(StreamEvent::BlockStart(start));
                    index
                }
            };
            let mut d = BlockDelta::new(index, Delta::Refusal(text));
            d.extras = self.extras();
            events.push(StreamEvent::BlockDelta(d));
        }
        for entry in &delta.tool_calls {
            self.tool_entry(entry, events)?;
        }
        Ok(finish_reason)
    }

    fn tool_entry(&mut self, raw: &RawJson, events: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        let entry: Ext<ToolCallChunk> = parse_ext(raw.as_raw(), "tool call chunk", true)?;
        let Ext {
            known,
            mut extra,
            mut verbatim,
            ..
        } = entry;
        nulls_to_verbatim(&mut extra, &mut verbatim);
        let mut extras = self.extras();
        for (key, value) in extra {
            extras.fields.insert(format!("tool_call.{key}"), value);
        }
        let mut tc = Map::new();
        if !verbatim.is_empty() {
            tc.insert("v".to_owned(), obj_value(&verbatim));
        }
        let (name, arguments) = match known.function {
            Some(function) => {
                let Ext {
                    known: f,
                    extra: mut f_extra,
                    verbatim: mut f_verbatim,
                    ..
                } = function;
                nulls_to_verbatim(&mut f_extra, &mut f_verbatim);
                for (key, value) in f_extra {
                    extras
                        .fields
                        .insert(format!("tool_call.function.{key}"), value);
                }
                if !f_verbatim.is_empty() {
                    tc.insert("fv".to_owned(), obj_value(&f_verbatim));
                }
                (f.name, f.arguments)
            }
            None => {
                tc.insert("no_fn".to_owned(), Value::Bool(true));
                (None, None)
            }
        };
        let flag = |tc: &mut Map<String, Value>, key: &str| {
            tc.insert(key.to_owned(), Value::Bool(true));
        };

        let new_id = known.id.as_deref().filter(|id| !id.is_empty());
        let replaced = self
            .tools
            .get(&known.index)
            .is_some_and(|t| !t.unknown && !t.id.is_empty() && new_id.is_some_and(|id| id != t.id));
        if replaced && let Some(old) = self.tools.shift_remove(&known.index) {
            let mut stop = BlockStop::new(old.block);
            stop.extras = self.extras();
            events.push(StreamEvent::BlockStop(stop));
        }
        let known_call = self.tools.get(&known.index).map(|t| (t.block, t.unknown));
        match known_call {
            None => {
                let block = self.new_block();
                let expected = u32::try_from(self.tools.len()).unwrap_or(u32::MAX);
                let unknown = known.ty.as_deref().is_some_and(|t| t != "function");
                let complete = unknown
                    || (known.id.as_deref().is_some_and(|id| !id.is_empty())
                        && name.as_deref().is_some_and(|n| !n.is_empty()));
                self.tools.insert(
                    known.index,
                    ToolState {
                        block,
                        id: known.id.clone().unwrap_or_default(),
                        unknown,
                        complete,
                    },
                );
                if unknown {
                    let header =
                        BlockHeader::Unknown(RawItem::new(self.origin.clone(), raw.clone()));
                    let mut start = BlockStart::new(block, header);
                    start.extras = self.extras();
                    events.push(StreamEvent::BlockStart(start));
                    return Ok(());
                }
                if known.index != expected || replaced {
                    tc.insert("index".to_owned(), Value::from(known.index));
                }
                if known.id.is_none() {
                    flag(&mut tc, "no_id");
                }
                if known.ty.is_none() {
                    flag(&mut tc, "no_type");
                }
                if name.is_none() && !tc.contains_key("no_fn") {
                    flag(&mut tc, "no_name");
                }
                if arguments.is_none() {
                    flag(&mut tc, "no_args");
                }
                let name = name.unwrap_or_default();
                let name = self.tool_names.get(&name).cloned().unwrap_or(name);
                let header = ToolCallHeader::new(known.id.unwrap_or_default(), name);
                let mut start = BlockStart::new(block, BlockHeader::ToolCall(header));
                if !tc.is_empty() {
                    extras
                        .form
                        .insert(FORM_TOOL_CALL.to_owned(), Value::Object(tc));
                }
                start.extras = extras;
                events.push(StreamEvent::BlockStart(start));
                if let Some(arguments) = arguments {
                    let mut d = BlockDelta::new(block, Delta::ToolArguments(arguments));
                    d.extras = self.extras();
                    events.push(StreamEvent::BlockDelta(d));
                }
            }
            Some((block, true)) => {
                let delta = Delta::Unknown(RawItem::new(self.origin.clone(), raw.clone()));
                let mut d = BlockDelta::new(block, delta);
                d.extras = self.extras();
                events.push(StreamEvent::BlockDelta(d));
            }
            Some((block, false)) => {
                self.complete_tool(
                    known.index,
                    block,
                    known.id.as_deref(),
                    name.as_deref(),
                    events,
                );
                if let Some(id) = known.id {
                    tc.insert("id".to_owned(), Value::String(id));
                }
                if let Some(ty) = known.ty {
                    tc.insert("type".to_owned(), Value::String(ty));
                }
                if let Some(name) = name {
                    tc.insert("name".to_owned(), Value::String(name));
                }
                if arguments.is_none() && !tc.contains_key("no_fn") {
                    flag(&mut tc, "no_args");
                }
                let mut d =
                    BlockDelta::new(block, Delta::ToolArguments(arguments.unwrap_or_default()));
                if !tc.is_empty() {
                    extras
                        .form
                        .insert(FORM_TOOL_CALL.to_owned(), Value::Object(tc));
                }
                d.extras = extras;
                events.push(StreamEvent::BlockDelta(d));
            }
        }
        Ok(())
    }
}

impl ChatStreamDecoder {
    fn complete_tool(
        &mut self,
        index: u32,
        block: u32,
        id: Option<&str>,
        name: Option<&str>,
        events: &mut [StreamEvent],
    ) {
        if self.tools.get(&index).is_none_or(|t| t.complete) {
            return;
        }
        let id = id.filter(|id| !id.is_empty());
        let name = name
            .filter(|n| !n.is_empty())
            .map(|n| self.tool_names.get(n).map_or(n, String::as_str).to_owned());
        let mut complete = false;
        let queued = self.queue.iter_mut().flat_map(|h| h.events.iter_mut());
        for event in queued.chain(events.iter_mut()) {
            let StreamEvent::BlockStart(start) = event else {
                continue;
            };
            let BlockHeader::ToolCall(header) = &mut start.block else {
                continue;
            };
            if start.index != block {
                continue;
            }
            let mut tc = match start.extras.form.shift_remove(FORM_TOOL_CALL) {
                Some(Value::Object(tc)) => tc,
                _ => Map::new(),
            };
            if let Some(id) = id
                && header.id.is_empty()
            {
                if tc.get("no_id") != Some(&Value::Bool(true)) {
                    tc.insert("id".to_owned(), Value::String(header.id.clone()));
                }
                header.id = id.to_owned();
            }
            if let Some(name) = &name
                && header.name.is_empty()
            {
                if tc.get("no_name") != Some(&Value::Bool(true)) && !tc.contains_key("no_fn") {
                    tc.insert("name".to_owned(), Value::String(header.name.clone()));
                }
                header.name.clone_from(name);
            }
            if !tc.is_empty() {
                start
                    .extras
                    .form
                    .insert(FORM_TOOL_CALL.to_owned(), Value::Object(tc));
            }
            complete = !header.id.is_empty() && !header.name.is_empty();
        }
        if let Some(tool) = self.tools.get_mut(&index) {
            if let Some(id) = id
                && tool.id.is_empty()
            {
                id.clone_into(&mut tool.id);
            }
            tool.complete |= complete;
        }
    }
}

fn give_call_an_id(event: &mut StreamEvent, response_id: &str) {
    let StreamEvent::BlockStart(start) = event else {
        return;
    };
    let BlockHeader::ToolCall(header) = &mut start.block else {
        return;
    };
    if !header.id.is_empty() {
        return;
    }
    header.id = generated_call_id(response_id, start.index);
    let mut tc = match start.extras.form.shift_remove(FORM_TOOL_CALL) {
        Some(Value::Object(tc)) => tc,
        _ => Map::new(),
    };
    if tc.get("no_id") != Some(&Value::Bool(true)) {
        tc.entry("id").or_insert(Value::String(String::new()));
    }
    start
        .extras
        .form
        .insert(FORM_TOOL_CALL.to_owned(), Value::Object(tc));
}

pub(crate) fn generated_call_id(scope: &str, index: impl std::fmt::Display) -> String {
    let scope: String = scope.chars().filter(char::is_ascii_alphanumeric).collect();
    if scope.is_empty() {
        format!("call_kaeso_{index}")
    } else {
        format!("call_kaeso_{scope}_{index}")
    }
}

fn is_error_frame(raw: &serde_json::value::RawValue) -> bool {
    #[derive(serde::Deserialize)]
    struct Probe {
        #[serde(default)]
        error: Option<serde::de::IgnoredAny>,
    }
    serde_json::from_str::<Probe>(raw.get()).is_ok_and(|p| p.error.is_some())
}

pub(crate) fn event_extras_mut(event: &mut StreamEvent) -> Option<&mut Extras> {
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

pub(crate) fn event_extras(event: &StreamEvent) -> Option<&Extras> {
    match event {
        StreamEvent::ResponseStart(e) => Some(&e.extras),
        StreamEvent::BlockStart(e) => Some(&e.extras),
        StreamEvent::BlockDelta(e) => Some(&e.extras),
        StreamEvent::BlockStop(e) => Some(&e.extras),
        StreamEvent::ResponseDelta(e) => Some(&e.extras),
        StreamEvent::ResponseStop(e) => Some(&e.extras),
        _ => None,
    }
}

impl StreamDecoder for ChatStreamDecoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        let mut items = Vec::new();
        let result = self.parser.feed_into(bytes, &mut items);
        for item in items {
            self.item(item, &mut out);
        }
        if let Err(e) = result
            && !self.inert
        {
            self.release_all(&mut out);
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
                    self.item(item, &mut out);
                }
            }
            Err(e) => {
                self.release_all(&mut out);
                self.fail(&mut out, e.into());
                return out;
            }
        }
        self.release_all(&mut out);
        if self.inert || self.done {
            self.inert = true;
            return out;
        }
        if self.finished {
            self.report_running_usage(&mut out);
            let mut stop = ResponseStop::new();
            stop.extras = self.extras();
            stop.extras
                .form
                .insert(FORM_NO_DONE.to_owned(), Value::Bool(true));
            out.push(StreamEvent::ResponseStop(stop));
            self.inert = true;
        } else {
            self.fail(
                &mut out,
                KppError::provider(
                    "The provider's OpenAI Chat Completions stream ended before the response was complete.",
                ),
            );
        }
        out
    }
}
