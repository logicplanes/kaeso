use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use serde::ser::SerializeMap as _;
use serde::{Serialize, Serializer};

use kaeso_kpp::{
    BlockHeader, Delta, KppError, RawJson, ResponseStart, StreamEvent, StreamMode, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Map, Value};

use super::DIALECT;
use super::convert::to_raw;
use super::error::error_body;
use super::response::{encode_finish, encode_usage};
use super::stream::{
    CHUNK_OBJECT, ENVELOPE_FIELDS, ENVELOPE_KEYS, FORM_CHUNK, FORM_CONTENT_SLOTS, FORM_EMPTY_TEXT,
    FORM_ENV, FORM_ENV_ABSENT, FORM_IN_CHUNK, FORM_NO_DONE, FORM_PART, FORM_REASONING_FIELD,
    FORM_RUNNING_USAGE, FORM_SSE, FORM_TOOL_CALL, Obj, event_extras, start_envelope, value_obj,
};
use crate::codec::{EncodeCtx, StreamEncoder};
use crate::keepalive::{self, Pacer};
use crate::sse::write_event;
use crate::wire::NO_ARGUMENTS;

#[derive(Default)]
struct Entry {
    obj: Obj,
    function: Option<Obj>,
    merge: Option<u32>,
    raw: Option<RawJson>,
}

#[derive(Default)]
struct Chunk {
    own: bool,
    remaining: usize,
    form: Map<String, Value>,
    fields: Obj,
    choice_fields: Obj,
    delta_fields: Obj,
    has_choice: bool,
    content: Option<String>,
    reasoning: Option<String>,
    refusal: Option<String>,
    tool_calls: Vec<Entry>,
    audio: Option<Obj>,
    finish: Option<String>,
    usage: Option<Value>,
}

pub(crate) struct ChatStreamEncoder {
    include_usage: bool,
    created_at: u64,
    started: bool,
    envelope: Obj,
    role_pending: bool,
    tool_index: HashMap<u32, u32>,
    unknown_tools: HashSet<u32>,
    blank_tools: HashSet<u32>,
    next_tool: u32,
    pending: Option<Chunk>,
    start_usage: Option<Usage>,
    usage: Option<Usage>,
    usage_sent: bool,
    done: bool,
    pacer: Pacer,
}

impl ChatStreamEncoder {
    pub(crate) fn new(ctx: &EncodeCtx) -> Self {
        ChatStreamEncoder {
            include_usage: matches!(
                ctx.stream,
                StreamMode::On {
                    include_usage: true,
                    ..
                }
            ),
            created_at: ctx.created_at.unwrap_or(0),
            started: false,
            envelope: Obj::new(),
            role_pending: false,
            tool_index: HashMap::new(),
            unknown_tools: HashSet::new(),
            blank_tools: HashSet::new(),
            next_tool: 0,
            pending: None,
            start_usage: None,
            usage: None,
            usage_sent: false,
            done: false,
            pacer: Pacer::default(),
        }
    }

    fn chat_tool_index(&mut self, block: u32, wanted: Option<u32>) -> u32 {
        if let Some(index) = self.tool_index.get(&block) {
            return *index;
        }
        let index = wanted.unwrap_or(self.next_tool);
        self.next_tool = self.next_tool.max(index.saturating_add(1));
        self.tool_index.insert(block, index);
        index
    }

    fn apply(&mut self, event: &StreamEvent, chunk: &mut Chunk) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        let own = chunk.own;
        match event {
            StreamEvent::ResponseStart(start) => self.start(start, own, chunk),
            StreamEvent::BlockStart(start) => match &start.block {
                BlockHeader::ToolCall(header) => {
                    let tc = tc_form(&start.extras);
                    let wanted = tc
                        .get("index")
                        .and_then(Value::as_u64)
                        .and_then(|i| u32::try_from(i).ok());
                    let index = self.chat_tool_index(start.index, wanted);
                    let mut entry = Entry::default();
                    entry.obj.insert("index".to_owned(), to_raw(&index)?);
                    if !flag(&tc, "no_id") {
                        let id = tc.get("id").and_then(Value::as_str).unwrap_or(&header.id);
                        entry.obj.insert("id".to_owned(), to_raw(id)?);
                    }
                    if !flag(&tc, "no_type") {
                        entry.obj.insert("type".to_owned(), to_raw("function")?);
                    }
                    if !flag(&tc, "no_fn") {
                        let mut function = Obj::new();
                        if !flag(&tc, "no_name") {
                            let name = tc
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or(&header.name);
                            function.insert("name".to_owned(), to_raw(name)?);
                        }
                        if !own {
                            function.insert("arguments".to_owned(), to_raw("")?);
                            self.blank_tools.insert(start.index);
                        }
                        entry.function = Some(function);
                    }
                    if own && !flag(&tc, "no_args") {
                        entry.merge = Some(start.index);
                    }
                    add_entry_extras(&mut entry, &start.extras, &tc);
                    chunk.tool_calls.push(entry);
                    chunk.has_choice = true;
                }
                BlockHeader::Audio(_) => {
                    let audio = chunk.audio.get_or_insert_with(Obj::new);
                    match start.extras.fields_for(DIALECT) {
                        Some(fields) if own => audio.extend(fields.clone()),
                        _ if own => {}
                        _ => {
                            let id = self
                                .envelope
                                .get("id")
                                .and_then(|id| id.parse::<String>().ok())
                                .unwrap_or_default();
                            audio.insert("id".to_owned(), to_raw(&format!("audio_{id}"))?);
                        }
                    }
                    chunk.has_choice = true;
                }
                BlockHeader::Unknown(item)
                    if item.origin.is(DIALECT)
                        && start.extras.form_value(DIALECT, FORM_PART).is_none() =>
                {
                    self.unknown_tools.insert(start.index);
                    chunk.tool_calls.push(Entry {
                        raw: Some(item.raw.clone()),
                        ..Entry::default()
                    });
                    chunk.has_choice = true;
                }
                _ => {}
            },
            StreamEvent::BlockDelta(delta) => match &delta.delta {
                Delta::Text(text) => {
                    chunk.content.get_or_insert_with(String::new).push_str(text);
                    chunk.has_choice = true;
                }
                Delta::Refusal(text) => {
                    chunk.refusal.get_or_insert_with(String::new).push_str(text);
                    chunk.has_choice = true;
                }
                Delta::Audio(piece) => {
                    let audio = chunk.audio.get_or_insert_with(Obj::new);
                    if own && let Some(fields) = delta.extras.fields_for(DIALECT) {
                        audio.extend(fields.clone());
                    }
                    for (key, value) in [("data", &piece.data), ("transcript", &piece.transcript)] {
                        let Some(value) = value else { continue };
                        let joined = match audio.get(key).and_then(|v| v.parse::<String>().ok()) {
                            Some(earlier) if key == "data" => {
                                crate::media::join_base64([earlier.as_str(), value.as_str()])
                                    .unwrap_or_else(|| format!("{earlier}{value}"))
                            }
                            Some(earlier) => format!("{earlier}{value}"),
                            None => value.clone(),
                        };
                        audio.insert(key.to_owned(), to_raw(&joined)?);
                    }
                    chunk.has_choice = true;
                }
                Delta::ReasoningText(text) if own && delta.extras.is_from(DIALECT) => {
                    chunk
                        .reasoning
                        .get_or_insert_with(String::new)
                        .push_str(text);
                    chunk.has_choice = true;
                }
                Delta::ToolArguments(arguments) => {
                    if !arguments.trim().is_empty() {
                        self.blank_tools.remove(&delta.index);
                    }
                    let Some(index) = self.tool_index.get(&delta.index).copied() else {
                        return Ok(out);
                    };
                    let header = chunk
                        .tool_calls
                        .last_mut()
                        .filter(|e| e.merge == Some(delta.index));
                    if let Some(header) = header {
                        if let Some(function) = header.function.as_mut() {
                            function.insert("arguments".to_owned(), to_raw(arguments)?);
                        }
                        header.merge = None;
                    } else {
                        let tc = tc_form(&delta.extras);
                        let mut entry = Entry::default();
                        entry.obj.insert("index".to_owned(), to_raw(&index)?);
                        for key in ["id", "type"] {
                            if let Some(value) = tc.get(key) {
                                entry.obj.insert(key.to_owned(), to_raw(value)?);
                            }
                        }
                        if !flag(&tc, "no_fn") {
                            let mut function = Obj::new();
                            if let Some(name) = tc.get("name") {
                                function.insert("name".to_owned(), to_raw(name)?);
                            }
                            if !flag(&tc, "no_args") {
                                function.insert("arguments".to_owned(), to_raw(arguments)?);
                            }
                            entry.function = Some(function);
                        }
                        add_entry_extras(&mut entry, &delta.extras, &tc);
                        chunk.tool_calls.push(entry);
                    }
                    chunk.has_choice = true;
                }
                Delta::Unknown(item)
                    if item.origin.is(DIALECT) && self.unknown_tools.contains(&delta.index) =>
                {
                    chunk.tool_calls.push(Entry {
                        raw: Some(item.raw.clone()),
                        ..Entry::default()
                    });
                    chunk.has_choice = true;
                }
                _ => {}
            },
            StreamEvent::BlockStop(stop) => {
                if self.blank_tools.remove(&stop.index)
                    && let Some(index) = self.tool_index.get(&stop.index)
                {
                    let mut entry = Entry::default();
                    entry.obj.insert("index".to_owned(), to_raw(index)?);
                    let mut function = Obj::new();
                    function.insert("arguments".to_owned(), to_raw(NO_ARGUMENTS)?);
                    entry.function = Some(function);
                    chunk.tool_calls.push(entry);
                    chunk.has_choice = true;
                }
            }
            StreamEvent::ResponseDelta(delta) => {
                if let Some(stop) = &delta.stop {
                    chunk.finish = Some(encode_finish(stop, own).to_owned());
                    chunk.has_choice = true;
                }
                if let Some(usage) = &delta.usage {
                    if own {
                        chunk.usage = Some(encode_usage(usage));
                    }
                    self.usage = Some(usage.clone());
                }
            }
            StreamEvent::ResponseStop(stop) => {
                let no_done = own && stop.extras.form_flag(DIALECT, FORM_NO_DONE);
                out.extend(self.close(!no_done)?);
            }
            StreamEvent::Error(err) => {
                let body = serde_json::to_string(&error_body(err)).map_err(internal)?;
                write_event(&mut out, None, &body);
                self.done = true;
            }
            StreamEvent::Extra(item) if item.origin.is(DIALECT) => {
                let raw = item.raw.as_raw();
                if self.include_usage {
                    self.usage_sent |= carries_usage(raw);
                    crate::sse::write_frame_extra(&mut out, raw).map_err(internal)?;
                } else {
                    match without_usage(raw).map_err(internal)? {
                        Unasked::Keep => {
                            crate::sse::write_frame_extra(&mut out, raw).map_err(internal)?;
                        }
                        Unasked::Trimmed(trimmed) => {
                            crate::sse::write_frame_extra(&mut out, &trimmed).map_err(internal)?;
                        }
                        Unasked::Hidden => {}
                    }
                }
            }
            _ => {}
        }
        Ok(out)
    }

    fn start(&mut self, start: &ResponseStart, own: bool, chunk: &mut Chunk) {
        self.started = true;
        self.start_usage = start.usage.clone();
        if own {
            self.envelope = start_envelope(start);
            chunk.has_choice = true;
            for (key, value) in start.extras.body_fields() {
                if !key.contains('.') && !ENVELOPE_FIELDS.contains(&key) {
                    chunk.fields.insert(key.to_owned(), value.clone());
                }
            }
        } else {
            let mut env = Obj::new();
            for (key, value) in [
                ("id", to_raw(&start.id)),
                ("object", to_raw(CHUNK_OBJECT)),
                ("created", to_raw(&self.created_at)),
                ("model", to_raw(&start.model)),
            ] {
                if let Ok(value) = value {
                    env.insert(key.to_owned(), value);
                }
            }
            self.envelope = env;
            self.role_pending = true;
            chunk.has_choice = true;
        }
    }

    fn close(&mut self, done_frame: bool) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if self.done {
            return Ok(out);
        }
        if self.include_usage && !self.usage_sent {
            let usage = match (self.usage.clone(), self.start_usage.clone()) {
                (Some(mut usage), Some(start)) if usage.total_input_tokens() == 0 => {
                    usage.input_tokens = start.input_tokens;
                    usage.cache_read_tokens = start.cache_read_tokens;
                    usage.cache_write_tokens = start.cache_write_tokens;
                    Some(usage)
                }
                (Some(usage), _) => Some(usage),
                (None, start) => start,
            };
            if let Some(usage) = usage {
                let chunk = Chunk {
                    usage: Some(encode_usage(&usage)),
                    ..Chunk::default()
                };
                out.extend(self.write(chunk)?);
            }
        }
        if done_frame {
            write_event(&mut out, None, "[DONE]");
        }
        self.done = true;
        Ok(out)
    }

    fn write(&mut self, chunk: Chunk) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if self.done {
            return Ok(out);
        }
        let own = chunk.own;
        let form = &chunk.form;
        if own
            && !self.include_usage
            && chunk.usage.is_some()
            && form.get("ch").and_then(Value::as_str) == Some("empty")
        {
            return Ok(out);
        }
        let mut obj: Tree<'_> = self
            .envelope
            .iter()
            .map(|(k, v)| (Cow::Borrowed(k.as_str()), J::Raw(v.as_raw())))
            .collect();
        if own {
            if let Some(Value::Object(env)) = form.get(FORM_ENV) {
                for (key, value) in env {
                    if ENVELOPE_KEYS.contains(&key.as_str()) {
                        obj.insert(Cow::Borrowed(key.as_str()), J::Value(value));
                    }
                }
            }
            if let Some(Value::Array(absent)) = form.get(FORM_ENV_ABSENT) {
                for key in absent.iter().filter_map(Value::as_str) {
                    if ENVELOPE_KEYS.contains(&key) {
                        obj.shift_remove(key);
                    }
                }
            }
        }
        let choice_mode = if own {
            form.get("ch").and_then(Value::as_str).unwrap_or("one")
        } else if chunk.has_choice {
            "one"
        } else {
            "empty"
        };
        match choice_mode {
            "absent" => {}
            "empty" => {
                obj.insert(Cow::Borrowed("choices"), J::List(Vec::new()));
            }
            _ => {
                let role_pending = !own && std::mem::take(&mut self.role_pending);
                let choice = choice(&chunk, role_pending)?;
                obj.insert(Cow::Borrowed("choices"), J::List(vec![choice]));
            }
        }
        let running = if own {
            form.get(FORM_RUNNING_USAGE)
        } else {
            None
        };
        if self.include_usage {
            if let Some(usage) = chunk.usage.as_ref().or(running) {
                obj.insert(Cow::Borrowed("usage"), J::Value(usage));
                self.usage_sent = true;
            } else if !own {
                obj.insert(Cow::Borrowed("usage"), J::Null);
            }
        }
        if own {
            if let Some(obf) = form.get("obf") {
                obj.insert(Cow::Borrowed("obfuscation"), J::Value(obf));
            }
            for (key, value) in &chunk.fields {
                obj.or_insert(Cow::Borrowed(key.as_str()), J::Raw(value.as_raw()));
            }
            if let Some(Value::Object(kv)) = form.get("kv") {
                for (key, value) in kv {
                    if key == "usage" && !self.include_usage {
                        continue;
                    }
                    obj.or_insert(Cow::Borrowed(key.as_str()), J::Value(value));
                }
            }
        }
        match form.get(FORM_SSE).filter(|_| own) {
            Some(framing) => {
                let data = serde_json::to_string(&obj).map_err(internal)?;
                let mut event = crate::sse::SseEvent::new(None, data);
                crate::sse::set_framing(&mut event, framing);
                crate::sse::write_item(&mut out, &crate::sse::SseItem::Event(event));
            }
            // What `write_item` writes for an event with only data, serialized in place. A raw
            // value the provider split across `data:` lines keeps its line breaks, and only
            // `write_item` splits those into lines of their own again.
            None => {
                let begin = out.len();
                out.extend_from_slice(b"data: ");
                let json = out.len();
                serde_json::to_writer(&mut out, &obj).map_err(internal)?;
                if out
                    .get(json..)
                    .is_some_and(|written| written.iter().any(|b| matches!(b, b'\n' | b'\r')))
                {
                    out.truncate(begin);
                    let data = serde_json::to_string(&obj).map_err(internal)?;
                    let event = crate::sse::SseEvent::new(None, data);
                    crate::sse::write_item(&mut out, &crate::sse::SseItem::Event(event));
                } else {
                    out.extend_from_slice(b"\n\n");
                }
            }
        }
        Ok(out)
    }

    fn collect_fields(event: &StreamEvent, chunk: &mut Chunk) {
        let Some(extras) = event_extras(event) else {
            return;
        };
        let audio = matches!(event, StreamEvent::BlockStart(s) if matches!(s.block, BlockHeader::Audio(_)))
            || matches!(event, StreamEvent::BlockDelta(d) if matches!(d.delta, Delta::Audio(_)));
        if !extras.is_from(DIALECT) || audio {
            return;
        }
        let is_start = matches!(event, StreamEvent::ResponseStart(_));
        for (key, value) in extras.body_fields() {
            if let Some(rest) = key.strip_prefix("choice.") {
                chunk.choice_fields.insert(rest.to_owned(), value.clone());
            } else if let Some(rest) = key.strip_prefix("delta.") {
                chunk.delta_fields.insert(rest.to_owned(), value.clone());
            } else if !key.contains('.') && !is_start {
                chunk.fields.insert(key.to_owned(), value.clone());
            }
        }
    }
}

fn choice<'a>(chunk: &'a Chunk, role_pending: bool) -> Result<J<'a>, KppError> {
    let own = chunk.own;
    let mut delta = Tree::new();
    if own {
        if let Some(role) = chunk.form.get("role") {
            delta.insert(Cow::Borrowed("role"), J::Value(role));
        }
    } else if role_pending {
        delta.insert(Cow::Borrowed("role"), J::Str("assistant"));
        if chunk.content.is_none()
            && chunk.refusal.is_none()
            && chunk.tool_calls.is_empty()
            && chunk.audio.is_none()
        {
            delta.insert(Cow::Borrowed("content"), J::Str(""));
            delta.insert(Cow::Borrowed("refusal"), J::Null);
        }
    }
    if let Some(parts) = content_parts(chunk)? {
        delta.insert(Cow::Borrowed("content"), J::Owned(parts));
    } else if let Some(content) = &chunk.content {
        delta.insert(Cow::Borrowed("content"), J::Str(content));
    } else if own && chunk.form.get(FORM_EMPTY_TEXT) == Some(&Value::Bool(true)) {
        delta.insert(Cow::Borrowed("content"), J::Str(""));
    }
    if let Some(refusal) = &chunk.refusal {
        delta.insert(Cow::Borrowed("refusal"), J::Str(refusal));
    }
    if let (true, Some(Value::String(field)), Some(reasoning)) =
        (own, chunk.form.get(FORM_REASONING_FIELD), &chunk.reasoning)
    {
        delta.insert(Cow::Borrowed(field.as_str()), J::Str(reasoning));
    }
    if !chunk.tool_calls.is_empty() {
        let entries = chunk
            .tool_calls
            .iter()
            .map(|entry| match &entry.raw {
                Some(raw) => J::Raw(raw.as_raw()),
                None => {
                    let mut obj: Tree<'_> = entry
                        .obj
                        .iter()
                        .map(|(k, v)| (Cow::Borrowed(k.as_str()), J::Raw(v.as_raw())))
                        .collect();
                    if let Some(function) = &entry.function {
                        obj.insert(Cow::Borrowed("function"), J::Obj(function));
                    }
                    J::Map(obj)
                }
            })
            .collect();
        delta.insert(Cow::Borrowed("tool_calls"), J::List(entries));
    }
    if let Some(audio) = &chunk.audio {
        delta.insert(Cow::Borrowed("audio"), J::Obj(audio));
    }
    let mut choice = Tree::new();
    choice.insert(Cow::Borrowed("index"), J::Zero);
    if own {
        for (key, value) in &chunk.delta_fields {
            delta.or_insert(Cow::Borrowed(key.as_str()), J::Raw(value.as_raw()));
        }
        if let Some(Value::Object(dv)) = chunk.form.get("dv") {
            for (key, value) in dv {
                delta.or_insert(Cow::Borrowed(key.as_str()), J::Value(value));
            }
        }
        choice.insert(Cow::Borrowed("delta"), J::Map(delta));
        if let Some(finish) = &chunk.finish {
            choice.insert(Cow::Borrowed("finish_reason"), J::Str(finish));
        }
        for (key, value) in &chunk.choice_fields {
            choice.or_insert(Cow::Borrowed(key.as_str()), J::Raw(value.as_raw()));
        }
        if let Some(Value::Object(cv)) = chunk.form.get("cv") {
            for (key, value) in cv {
                choice.or_insert(Cow::Borrowed(key.as_str()), J::Value(value));
            }
        }
    } else {
        choice.insert(Cow::Borrowed("delta"), J::Map(delta));
        choice.insert(Cow::Borrowed("logprobs"), J::Null);
        match &chunk.finish {
            Some(finish) => choice.insert(Cow::Borrowed("finish_reason"), J::Str(finish)),
            None => choice.insert(Cow::Borrowed("finish_reason"), J::Null),
        };
    }
    Ok(J::Map(choice))
}

// The few keys of a chunk object in the order `IndexMap` keeps them: `insert` replaces a value in
// its place, `or_insert` keeps the first, and `shift_remove` closes the gap.
#[derive(Default)]
struct Tree<'a>(Vec<(Cow<'a, str>, J<'a>)>);

impl<'a> Tree<'a> {
    fn new() -> Self {
        Tree(Vec::new())
    }

    fn slot(&mut self, key: &str) -> Option<&mut J<'a>> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn insert(&mut self, key: Cow<'a, str>, value: J<'a>) {
        match self.slot(&key) {
            Some(slot) => *slot = value,
            None => self.0.push((key, value)),
        }
    }

    fn or_insert(&mut self, key: Cow<'a, str>, value: J<'a>) {
        if self.slot(&key).is_none() {
            self.0.push((key, value));
        }
    }

    fn shift_remove(&mut self, key: &str) {
        if let Some(i) = self.0.iter().position(|(k, _)| k == key) {
            self.0.remove(i);
        }
    }
}

impl<'a> FromIterator<(Cow<'a, str>, J<'a>)> for Tree<'a> {
    fn from_iter<I: IntoIterator<Item = (Cow<'a, str>, J<'a>)>>(iter: I) -> Self {
        let mut tree = Tree::new();
        for (k, v) in iter {
            tree.insert(k, v);
        }
        tree
    }
}

impl Serialize for Tree<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

// A chunk as it is built, borrowing from the envelope and the chunk's own fields, serialized
// once when it is written: each level used to be serialized into a `RawJson` of its own.
enum J<'a> {
    Raw(&'a RawValue),
    Owned(RawJson),
    Value(&'a Value),
    Str(&'a str),
    Null,
    Zero,
    Obj(&'a Obj),
    Map(Tree<'a>),
    List(Vec<J<'a>>),
}

impl Serialize for J<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            J::Raw(raw) => raw.serialize(serializer),
            J::Owned(raw) => raw.serialize(serializer),
            J::Value(value) => value.serialize(serializer),
            J::Str(s) => s.serialize(serializer),
            J::Null => serializer.serialize_unit(),
            J::Zero => serializer.serialize_i32(0),
            J::Obj(obj) => obj.serialize(serializer),
            J::Map(map) => map.serialize(serializer),
            J::List(list) => list.serialize(serializer),
        }
    }
}

fn tc_form(extras: &kaeso_kpp::Extras) -> Map<String, Value> {
    match extras.form_value(DIALECT, FORM_TOOL_CALL) {
        Some(Value::Object(map)) => map.clone(),
        _ => Map::new(),
    }
}

fn flag(tc: &Map<String, Value>, key: &str) -> bool {
    tc.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn add_entry_extras(entry: &mut Entry, extras: &kaeso_kpp::Extras, tc: &Map<String, Value>) {
    if !extras.is_from(DIALECT) {
        return;
    }
    for (key, value) in extras.body_fields() {
        if let Some(rest) = key.strip_prefix("tool_call.function.") {
            if let Some(function) = entry.function.as_mut() {
                function.insert(rest.to_owned(), value.clone());
            }
        } else if let Some(rest) = key.strip_prefix("tool_call.") {
            entry.obj.insert(rest.to_owned(), value.clone());
        }
    }
    for (key, value) in value_obj(tc.get("v")) {
        entry.obj.entry(key).or_insert(value);
    }
    if let Some(function) = entry.function.as_mut() {
        for (key, value) in value_obj(tc.get("fv")) {
            function.entry(key).or_insert(value);
        }
    }
}

fn carries_usage(raw: &RawValue) -> bool {
    serde_json::from_str::<Value>(raw.get())
        .ok()
        .and_then(|frame| frame.get("data")?.get("usage").cloned())
        .is_some_and(|usage| !usage.is_null())
}

enum Unasked {
    Keep,
    Trimmed(Box<RawValue>),
    Hidden,
}

fn without_usage(raw: &RawValue) -> Result<Unasked, serde_json::Error> {
    let mut frame: Value = serde_json::from_str(raw.get())?;
    let Some(Value::Object(data)) = frame.get_mut("data") else {
        return Ok(Unasked::Keep);
    };
    let Some(usage) = data.remove("usage") else {
        return Ok(Unasked::Keep);
    };
    let no_choices = data
        .get("choices")
        .is_none_or(|c| c.as_array().is_some_and(Vec::is_empty));
    if !usage.is_null() && no_choices {
        return Ok(Unasked::Hidden);
    }
    serde_json::value::to_raw_value(&frame).map(Unasked::Trimmed)
}

fn internal(e: serde_json::Error) -> KppError {
    KppError::provider(format!(
        "Kaeso could not serialize a stream chunk ({e}). This is a bug in Kaeso; please report it."
    ))
}

impl StreamEncoder for ChatStreamEncoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = self.encode_event(event)?;
        if !self.done {
            self.pacer
                .pace(event, DIALECT, &mut out, keepalive::COMMENT_FRAME);
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if let Some(pending) = self.pending.take()
            && !self.done
        {
            out.extend(self.write(pending)?);
        }
        if self.started && !self.done {
            out.extend(self.close(true)?);
        }
        self.done = true;
        Ok(out)
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.done {
            return Vec::new();
        }
        self.pacer.reset();
        keepalive::COMMENT_FRAME.to_vec()
    }
}

impl ChatStreamEncoder {
    fn encode_event(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        if self.done {
            let mut out = Vec::new();
            if let StreamEvent::Extra(item) = event
                && item.origin.is(DIALECT)
            {
                crate::sse::write_frame_extra(&mut out, item.raw.as_raw()).map_err(internal)?;
            }
            return Ok(out);
        }
        let own = match event {
            StreamEvent::Extra(item) => item.origin.is(DIALECT),
            other => event_extras(other).is_some_and(|e| e.is_from(DIALECT)),
        };
        let marker = if own {
            event_extras(event).and_then(|e| e.form_value(DIALECT, FORM_CHUNK))
        } else {
            None
        };
        let mut out = Vec::new();
        if let Some(marker) = marker {
            if let Some(pending) = self.pending.take() {
                out.extend(self.write(pending)?);
            }
            let n = marker.get("n").and_then(Value::as_u64).unwrap_or(1);
            let mut form = match marker {
                Value::Object(map) => map.clone(),
                _ => Map::new(),
            };
            form.remove("n");
            self.pending = Some(Chunk {
                own: true,
                remaining: usize::try_from(n).unwrap_or(1),
                form,
                ..Chunk::default()
            });
        }
        let in_chunk = own
            && marker.is_none()
            && event_extras(event).is_some_and(|e| e.form_flag(DIALECT, FORM_IN_CHUNK));
        if marker.is_none()
            && !in_chunk
            && let Some(pending) = self.pending.take()
        {
            out.extend(self.write(pending)?);
        }
        if let Some(mut chunk) = self.pending.take() {
            Self::collect_fields(event, &mut chunk);
            out.extend(self.apply(event, &mut chunk)?);
            chunk.remaining = chunk.remaining.saturating_sub(1);
            if chunk.remaining == 0 {
                out.extend(self.write(chunk)?);
            } else {
                self.pending = Some(chunk);
            }
            return Ok(out);
        }
        self.encode_single(event, out)
    }

    fn encode_single(
        &mut self,
        event: &StreamEvent,
        mut out: Vec<u8>,
    ) -> Result<Vec<u8>, KppError> {
        let own = event_extras(event).is_some_and(|e| e.is_from(DIALECT));
        let mut chunk = Chunk {
            own,
            ..Chunk::default()
        };
        out.extend(self.apply(event, &mut chunk)?);
        let speaks = chunk.has_choice
            && !(own
                && matches!(
                    event,
                    StreamEvent::ResponseStart(_) | StreamEvent::BlockStop(_)
                ));
        if speaks {
            out.extend(self.write(chunk)?);
        }
        Ok(out)
    }
}

fn content_parts(chunk: &Chunk) -> Result<Option<RawJson>, KppError> {
    let Some(Value::Array(slots)) = chunk.form.get(FORM_CONTENT_SLOTS).filter(|_| chunk.own) else {
        return Ok(None);
    };
    let mut others = match chunk.delta_fields.get("content") {
        Some(raw) => serde_json::from_str::<Vec<RawJson>>(raw.get())
            .map_err(internal)?
            .into_iter(),
        None => Vec::new().into_iter(),
    };
    let mut text = Pieces::new(chunk.content.as_deref());
    let mut parts = Vec::new();
    for slot in slots {
        match slot {
            Value::Null => match others.next() {
                Some(raw) => parts.push(raw),
                None => return Ok(None),
            },
            len => {
                let Some(piece) = text.next(len) else {
                    return Ok(None);
                };
                parts.push(to_raw(&serde_json::json!({"type": "text", "text": piece}))?);
            }
        }
    }
    if !text.done() {
        return Ok(None);
    }
    Ok(Some(to_raw(&parts)?))
}

struct Pieces<'a> {
    rest: &'a str,
}

impl<'a> Pieces<'a> {
    fn new(text: Option<&'a str>) -> Self {
        Pieces {
            rest: text.unwrap_or(""),
        }
    }

    fn next(&mut self, len: &Value) -> Option<String> {
        let len = usize::try_from(len.as_u64()?).ok()?;
        let piece = self.rest.get(..len)?;
        self.rest = self.rest.get(len..)?;
        Some(piece.to_owned())
    }

    fn done(&self) -> bool {
        self.rest.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use proptest::prelude::*;

    use super::*;

    #[derive(Clone, Debug)]
    enum Op {
        Insert(u8, u8),
        OrInsert(u8, u8),
        Remove(u8),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0u8..6, any::<u8>()).prop_map(|(k, v)| Op::Insert(k, v)),
            (0u8..6, any::<u8>()).prop_map(|(k, v)| Op::OrInsert(k, v)),
            (0u8..6).prop_map(Op::Remove),
        ]
    }

    proptest! {
        #[test]
        fn a_tree_keeps_keys_and_values_as_an_index_map_does(ops in proptest::collection::vec(op(), 0..40)) {
            let keys: Vec<String> = (0..6).map(|k| format!("k{k}")).collect();
            let mut tree = Tree::new();
            let mut map: IndexMap<String, u8> = IndexMap::new();
            for op in &ops {
                match op {
                    Op::Insert(k, v) => {
                        tree.insert(Cow::Borrowed(keys[usize::from(*k)].as_str()), J::Value(Box::leak(Box::new(Value::from(*v)))));
                        map.insert(keys[usize::from(*k)].clone(), *v);
                    }
                    Op::OrInsert(k, v) => {
                        tree.or_insert(Cow::Borrowed(keys[usize::from(*k)].as_str()), J::Value(Box::leak(Box::new(Value::from(*v)))));
                        map.entry(keys[usize::from(*k)].clone()).or_insert(*v);
                    }
                    Op::Remove(k) => {
                        tree.shift_remove(&keys[usize::from(*k)]);
                        map.shift_remove(&keys[usize::from(*k)]);
                    }
                }
            }
            prop_assert_eq!(serde_json::to_string(&tree).unwrap(), serde_json::to_string(&map).unwrap());
        }
    }
}
