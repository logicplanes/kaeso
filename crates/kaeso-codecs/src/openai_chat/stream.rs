use indexmap::IndexMap;
use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, Extras, KppError, Origin, RawItem,
    RawJson, ResponseDelta, ResponseStart, ResponseStop, StreamEvent, ToolCallHeader,
};
use serde_json::{Map, Value};

use super::DIALECT;
use super::convert::{nulls_to_verbatim, parse_ext};
use super::error::decode_body;
use super::response::{decode_finish, decode_usage};
use super::wire::{ChunkChoice, ChunkWire, ToolCallChunk};
use crate::codec::{EgressCtx, StreamDecoder};
use crate::sse::{SseEvent, SseItem, SseParser};
use crate::wire::Ext;

pub(crate) const FORM_CHUNK: &str = "chunk";
pub(crate) const FORM_IN_CHUNK: &str = "in_chunk";
pub(crate) const FORM_ENV: &str = "env";
pub(crate) const FORM_ENV_ABSENT: &str = "env_absent";
pub(crate) const FORM_TOOL_CALL: &str = "tc";
pub(crate) const FORM_NO_DONE: &str = "no_done";
pub(crate) const FORM_OBJECT: &str = "object";
pub(crate) const FORM_SSE: &str = "sse";

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

fn framing_form(id: Option<String>, retry: Option<u64>) -> Option<Value> {
    if id.is_none() && retry.is_none() {
        return None;
    }
    let mut framing = Map::new();
    if let Some(id) = id {
        framing.insert("id".to_owned(), Value::String(id));
    }
    if let Some(retry) = retry {
        framing.insert("retry".to_owned(), Value::from(retry));
    }
    Some(Value::Object(framing))
}

struct ToolState {
    block: u32,
    unknown: bool,
    open: bool,
}

pub(crate) struct ChatStreamDecoder {
    parser: SseParser,
    origin: Origin,
    tool_names: IndexMap<String, String>,
    started: bool,
    envelope: Obj,
    next_block: u32,
    text: Option<u32>,
    refusal: Option<u32>,
    tools: IndexMap<u32, ToolState>,
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
            envelope: Obj::new(),
            next_block: 0,
            text: None,
            refusal: None,
            tools: IndexMap::new(),
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
        match item {
            SseItem::Comment(text) => {
                let raw = serde_json::json!({ "comment": text });
                self.extra(raw, out);
            }
            SseItem::Event(event) => self.event(event, out),
        }
    }

    fn extra(&mut self, raw: Value, out: &mut Vec<StreamEvent>) {
        match RawJson::from_serialize(&raw) {
            Ok(raw) => out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw))),
            Err(e) => self.fail(out, KppError::provider(e.to_string())),
        }
    }

    fn event(&mut self, event: SseEvent, out: &mut Vec<StreamEvent>) {
        if event.event.as_deref() == Some("error") {
            let err = decode_body(None, event.data.as_bytes());
            self.fail(out, err);
            return;
        }
        let framed = event.id.is_some() || event.retry.is_some();
        let plain =
            event.event.is_none() && event.other.is_empty() && !(framed && event.data == "[DONE]");
        if !plain || self.done {
            let data = serde_json::from_str::<Value>(&event.data)
                .unwrap_or_else(|_| Value::String(event.data.clone()));
            let mut raw = Map::new();
            raw.insert(
                "event".to_owned(),
                event.event.map_or(Value::Null, Value::String),
            );
            raw.insert("data".to_owned(), data);
            if let Some(id) = event.id {
                raw.insert("id".to_owned(), Value::String(id));
            }
            if let Some(retry) = event.retry {
                raw.insert("retry".to_owned(), Value::from(retry));
            }
            if !event.other.is_empty() {
                let fields: Vec<Value> = event
                    .other
                    .into_iter()
                    .map(|(k, v)| Value::Array(vec![Value::String(k), Value::String(v)]))
                    .collect();
                raw.insert("fields".to_owned(), Value::Array(fields));
            }
            self.extra(Value::Object(raw), out);
            return;
        }
        if event.data == "[DONE]" {
            self.close_blocks(out);
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
                        super::convert::serde_text(&e)
                    )),
                );
                return;
            }
        };
        if is_error_frame(&raw) {
            let err = decode_body(None, event.data.as_bytes());
            self.fail(out, err);
            return;
        }
        let framing = framing_form(event.id, event.retry);
        match self.chunk(&raw, framing) {
            Ok(events) if events.is_empty() => match serde_json::from_str::<Value>(raw.get()) {
                Ok(data) => self.extra(serde_json::json!({ "event": null, "data": data }), out),
                Err(e) => self.fail(out, KppError::provider(e.to_string())),
            },
            Ok(events) => out.extend(events),
            Err(err) => self.fail(out, err),
        }
    }

    fn close_blocks(&mut self, out: &mut Vec<StreamEvent>) {
        let mut open: Vec<u32> = self
            .tools
            .values()
            .filter(|t| t.open)
            .map(|t| t.block)
            .chain(self.text)
            .chain(self.refusal)
            .collect();
        open.sort_unstable();
        for index in open {
            let mut stop = BlockStop::new(index);
            stop.extras = self.extras();
            out.push(StreamEvent::BlockStop(stop));
        }
        self.text = None;
        self.refusal = None;
        self.tools.clear();
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
        if !self.started {
            self.started = true;
            let text = |key: &str| {
                env.get(key)
                    .and_then(|v| v.parse::<String>().ok())
                    .unwrap_or_default()
            };
            let mut start = ResponseStart::new(text("id"), text("model"));
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
            self.envelope = env;
            events.push(StreamEvent::ResponseStart(start));
        } else {
            let mut set = Map::new();
            let mut absent = Vec::new();
            for key in ENVELOPE_FIELDS {
                match (env.get(*key), self.envelope.get(*key)) {
                    (Some(now), Some(first)) if same_raw(now, first) => {}
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
        let usage = known
            .usage
            .map(|u| decode_usage(&u, &self.origin))
            .transpose()?;
        if stop_usage.is_some() || usage.is_some() {
            let mut delta = ResponseDelta::new(stop_usage.flatten(), usage);
            delta.extras = self.extras();
            events.push(StreamEvent::ResponseDelta(delta));
        }

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
        for (key, value) in d_extra {
            fields.insert(format!("delta.{key}"), value);
        }
        if let Some(role) = delta.role {
            form.insert("role".to_owned(), Value::String(role));
        }
        if let Some(text) = delta.content {
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
        Ok(known.finish_reason)
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

        let known_call = self.tools.get(&known.index).map(|t| (t.block, t.unknown));
        match known_call {
            None => {
                let block = self.new_block();
                let expected = u32::try_from(self.tools.len()).unwrap_or(u32::MAX);
                let unknown = known.ty.as_deref().is_some_and(|t| t != "function");
                self.tools.insert(
                    known.index,
                    ToolState {
                        block,
                        unknown,
                        open: true,
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
                if known.index != expected {
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

fn is_error_frame(raw: &serde_json::value::RawValue) -> bool {
    #[derive(serde::Deserialize)]
    struct Probe {
        #[serde(default)]
        error: Option<serde::de::IgnoredAny>,
        #[serde(default)]
        choices: Option<serde::de::IgnoredAny>,
    }
    serde_json::from_str::<Probe>(raw.get()).is_ok_and(|p| p.error.is_some() && p.choices.is_none())
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
                self.fail(&mut out, e.into());
                return out;
            }
        }
        if self.inert || self.done {
            self.inert = true;
            return out;
        }
        if self.finished {
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
