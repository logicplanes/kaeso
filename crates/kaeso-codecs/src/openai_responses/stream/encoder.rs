use kaeso_kpp::{KppError, RawJson, ResponseStart, StreamEvent};
use serde_json::Value;
use serde_json::value::RawValue;

use super::{EVENT_KEY, Engine, event_extras, skipped};
use crate::codec::{EncodeCtx, StreamEncoder};
use crate::keepalive::{self, Pacer};
use crate::openai_responses::DIALECT;
use crate::openai_responses::node::{Node, entries_of};
use crate::openai_responses::overrides::apply;
use crate::openai_responses::response::echo_fields;
use crate::sse::write_event;

pub(crate) struct Encoder {
    engine: Engine,
    model: String,
    created_at: u64,
    pacer: Pacer,
    finished: bool,
}

impl Encoder {
    pub(crate) fn new(ctx: &EncodeCtx) -> Encoder {
        Encoder {
            engine: Engine::new(echo_fields(ctx), ctx.created_at.unwrap_or(0)),
            created_at: ctx.created_at.unwrap_or(0),
            model: ctx
                .request
                .as_deref()
                .map(|r| r.model().to_owned())
                .unwrap_or_default(),
            pacer: Pacer::default(),
            finished: false,
        }
    }

    fn run(&mut self, event: &StreamEvent, out: &mut Vec<u8>) -> Result<(), KppError> {
        let empty = kaeso_kpp::Extras::default();
        let extras = event_extras(event).unwrap_or(&empty);
        let mut emit = |scope: &str, mut node: Node, seq: i64| -> Result<Option<Node>, KppError> {
            if skipped(extras, scope) {
                return Ok(None);
            }
            if !node.has("sequence_number") {
                node.insert("sequence_number", Node::Val(Value::from(seq)));
            }
            apply(extras, DIALECT, scope, &[], &mut node);
            write_node(out, node.clone())?;
            node.remove(EVENT_KEY);
            Ok(Some(node))
        };
        self.engine.step(event, &mut emit)
    }
}

pub(super) fn write_node(out: &mut Vec<u8>, mut node: Node) -> Result<(), KppError> {
    let name = if node.has(EVENT_KEY) {
        let name = node.get_str(EVENT_KEY);
        node.remove(EVENT_KEY);
        name
    } else {
        node.get_str("type")
    };
    let data = serde_json::to_string(&node).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not serialize a stream event ({e}). This is a bug in Kaeso; please report it."
        ))
    })?;
    write_event(out, name.as_deref(), &data);
    Ok(())
}

pub(super) fn write_extra(out: &mut Vec<u8>, raw: &RawJson) -> (Option<i64>, bool) {
    let Some(entries) = entries_of(raw.as_raw()) else {
        return (None, false);
    };
    let get = |k: &str| entries.iter().find(|(key, _)| key == k).map(|(_, v)| *v);
    let as_str =
        |v: Option<&RawValue>| v.and_then(|v| serde_json::from_str::<String>(v.get()).ok());
    if let Some(comment) = as_str(get("comment")) {
        for line in comment.split('\n') {
            out.extend_from_slice(b": ");
            out.extend_from_slice(line.as_bytes());
            out.push(b'\n');
        }
        out.push(b'\n');
        return (None, false);
    }
    if let Some(id) = as_str(get("id")) {
        out.extend_from_slice(format!("id: {id}\n").as_bytes());
    }
    if let Some(retry) = get("retry").and_then(|v| serde_json::from_str::<u64>(v.get()).ok()) {
        out.extend_from_slice(format!("retry: {retry}\n").as_bytes());
    }
    if let Some(fields) =
        get("fields").and_then(|v| serde_json::from_str::<Vec<(String, String)>>(v.get()).ok())
    {
        for (name, value) in fields {
            out.extend_from_slice(format!("{name}: {value}\n").as_bytes());
        }
    }
    let is_text = get("text").is_some_and(|v| v.get() == "true");
    let data_raw = get("data");
    let data = match data_raw {
        Some(v) if is_text => serde_json::from_str::<String>(v.get()).unwrap_or_default(),
        Some(v) => v.get().to_owned(),
        None => String::new(),
    };
    write_event(out, as_str(get("event")).as_deref(), &data);
    let seq = data_raw
        .filter(|_| !is_text)
        .and_then(|d| serde_json::from_str::<Value>(d.get()).ok())
        .and_then(|d| d.get("sequence_number").and_then(Value::as_i64));
    (seq, true)
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = self.encode_event(event)?;
        if !self.engine.terminated() {
            self.pacer
                .pace(event, DIALECT, &mut out, keepalive::COMMENT_FRAME);
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        self.finished = true;
        Ok(Vec::new())
    }

    fn keepalive(&mut self) -> Vec<u8> {
        if self.finished || self.engine.terminated() {
            return Vec::new();
        }
        self.pacer.reset();
        keepalive::COMMENT_FRAME.to_vec()
    }
}

impl Encoder {
    fn encode_event(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        if let StreamEvent::Extra(item) = event {
            if item.origin.is(DIALECT) {
                let (seq, is_event) = write_extra(&mut out, &item.raw);
                if is_event {
                    self.engine.saw_sequence(seq);
                }
            }
            return Ok(out);
        }
        let needs_start = matches!(
            event,
            StreamEvent::BlockStart(_)
                | StreamEvent::BlockDelta(_)
                | StreamEvent::BlockStop(_)
                | StreamEvent::ResponseDelta(_)
                | StreamEvent::ResponseStop(_)
        );
        if needs_start && !self.engine.started() {
            let start = StreamEvent::ResponseStart(ResponseStart::new(
                format!("resp_kaeso_{}", self.created_at),
                self.model.clone(),
            ));
            self.run(&start, &mut out)?;
        }
        self.run(event, &mut out)?;
        Ok(out)
    }
}
