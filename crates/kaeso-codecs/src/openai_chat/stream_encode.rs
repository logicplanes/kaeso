use std::collections::{HashMap, HashSet};

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
    CHUNK_OBJECT, ENVELOPE_FIELDS, ENVELOPE_KEYS, FORM_CHUNK, FORM_ENV, FORM_ENV_ABSENT,
    FORM_IN_CHUNK, FORM_NO_DONE, FORM_SSE, FORM_TOOL_CALL, Obj, event_extras, start_envelope,
    value_obj,
};
use crate::codec::{EncodeCtx, StreamEncoder};
use crate::keepalive::{self, Pacer};
use crate::sse::write_event;

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
    refusal: Option<String>,
    tool_calls: Vec<Entry>,
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
                BlockHeader::Unknown(item) if item.origin.is(DIALECT) => {
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
                Delta::ToolArguments(arguments) => {
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
            StreamEvent::BlockStop(_) => {}
            StreamEvent::ResponseDelta(delta) => {
                if let Some(stop) = &delta.stop {
                    chunk.finish = Some(encode_finish(stop).to_owned());
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
                let trimmed = if self.include_usage {
                    None
                } else {
                    without_null_usage(item.raw.as_raw()).map_err(internal)?
                };
                let raw = trimmed.as_deref().unwrap_or(item.raw.as_raw());
                crate::sse::write_frame_extra(&mut out, raw).map_err(internal)?;
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
        let mut obj = self.envelope.clone();
        if own {
            for (key, value) in value_obj(form.get(FORM_ENV)) {
                if ENVELOPE_KEYS.contains(&key.as_str()) {
                    obj.insert(key, value);
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
                obj.insert("choices".to_owned(), to_raw(&Vec::<Value>::new())?);
            }
            _ => {
                let choice = self.choice(&chunk)?;
                obj.insert("choices".to_owned(), to_raw(&vec![choice])?);
            }
        }
        if self.include_usage {
            if let Some(usage) = &chunk.usage {
                obj.insert("usage".to_owned(), to_raw(usage)?);
                self.usage_sent = true;
            } else if !own {
                obj.insert("usage".to_owned(), RawJson::null());
            }
        }
        if own {
            if let Some(obf) = form.get("obf") {
                obj.insert("obfuscation".to_owned(), to_raw(obf)?);
            }
            for (key, value) in &chunk.fields {
                obj.entry(key.clone()).or_insert_with(|| value.clone());
            }
            for (key, value) in value_obj(form.get("kv")) {
                if key == "usage" && !self.include_usage {
                    continue;
                }
                obj.entry(key).or_insert(value);
            }
        }
        let data = serde_json::to_string(&obj).map_err(internal)?;
        let mut event = crate::sse::SseEvent::new(None, data);
        if own && let Some(framing) = form.get(FORM_SSE) {
            crate::sse::set_framing(&mut event, framing);
        }
        crate::sse::write_item(&mut out, &crate::sse::SseItem::Event(event));
        Ok(out)
    }

    fn choice(&mut self, chunk: &Chunk) -> Result<RawJson, KppError> {
        let own = chunk.own;
        let mut delta = Obj::new();
        if own {
            if let Some(role) = chunk.form.get("role") {
                delta.insert("role".to_owned(), to_raw(role)?);
            }
        } else if std::mem::take(&mut self.role_pending) {
            delta.insert("role".to_owned(), to_raw("assistant")?);
            if chunk.content.is_none() && chunk.refusal.is_none() && chunk.tool_calls.is_empty() {
                delta.insert("content".to_owned(), to_raw("")?);
                delta.insert("refusal".to_owned(), RawJson::null());
            }
        }
        if let Some(content) = &chunk.content {
            delta.insert("content".to_owned(), to_raw(content)?);
        }
        if let Some(refusal) = &chunk.refusal {
            delta.insert("refusal".to_owned(), to_raw(refusal)?);
        }
        if !chunk.tool_calls.is_empty() {
            let mut entries = Vec::new();
            for entry in &chunk.tool_calls {
                entries.push(match &entry.raw {
                    Some(raw) => raw.clone(),
                    None => {
                        let mut obj = entry.obj.clone();
                        if let Some(function) = &entry.function {
                            obj.insert("function".to_owned(), to_raw(function)?);
                        }
                        to_raw(&obj)?
                    }
                });
            }
            delta.insert("tool_calls".to_owned(), to_raw(&entries)?);
        }
        let mut choice = Obj::new();
        choice.insert("index".to_owned(), to_raw(&0)?);
        if own {
            for (key, value) in &chunk.delta_fields {
                delta.entry(key.clone()).or_insert_with(|| value.clone());
            }
            for (key, value) in value_obj(chunk.form.get("dv")) {
                delta.entry(key).or_insert(value);
            }
            choice.insert("delta".to_owned(), to_raw(&delta)?);
            if let Some(finish) = &chunk.finish {
                choice.insert("finish_reason".to_owned(), to_raw(finish)?);
            }
            for (key, value) in &chunk.choice_fields {
                choice.entry(key.clone()).or_insert_with(|| value.clone());
            }
            for (key, value) in value_obj(chunk.form.get("cv")) {
                choice.entry(key).or_insert(value);
            }
        } else {
            choice.insert("delta".to_owned(), to_raw(&delta)?);
            choice.insert("logprobs".to_owned(), RawJson::null());
            match &chunk.finish {
                Some(finish) => choice.insert("finish_reason".to_owned(), to_raw(finish)?),
                None => choice.insert("finish_reason".to_owned(), RawJson::null()),
            };
        }
        to_raw(&choice)
    }

    fn collect_fields(event: &StreamEvent, chunk: &mut Chunk) {
        let Some(extras) = event_extras(event) else {
            return;
        };
        if !extras.is_from(DIALECT) {
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

fn without_null_usage(raw: &RawValue) -> Result<Option<Box<RawValue>>, serde_json::Error> {
    let mut frame: Value = serde_json::from_str(raw.get())?;
    let Some(Value::Object(data)) = frame.get_mut("data") else {
        return Ok(None);
    };
    if !data.get("usage").is_some_and(Value::is_null) {
        return Ok(None);
    }
    data.remove("usage");
    serde_json::value::to_raw_value(&frame).map(Some)
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
