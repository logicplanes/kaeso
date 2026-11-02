use kaeso_kpp::{KppError, RawJson, ResponseStart, StreamEvent};
use serde_json::Value;

use super::{EVENT_KEY, Emitted, Engine, SSE_KEY, event_extras, skipped};
use crate::codec::{EncodeCtx, StreamEncoder};
use crate::keepalive::{self, Pacer};
use crate::openai_responses::DIALECT;
use crate::openai_responses::namespaces::Namespaces;
use crate::openai_responses::node::Node;
use crate::openai_responses::overrides::apply;
use crate::openai_responses::response::echo_fields;
use crate::sse::{SseEvent, SseItem, write_item};

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
            engine: Engine::new(
                echo_fields(ctx),
                ctx.created_at.unwrap_or(0),
                Namespaces::of(ctx.request.as_deref()),
            ),
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
        let mut emit = |scope: &str, mut node: Node, seq: i64| -> Result<Emitted, KppError> {
            if skipped(extras, scope) {
                return Ok(Emitted::Skipped(node));
            }
            if !node.has("sequence_number") {
                node.insert("sequence_number", Node::Val(Value::from(seq)));
            }
            apply(extras, DIALECT, scope, &[], &mut node);
            write_obj(out, &node)?;
            node.remove(EVENT_KEY);
            node.remove(SSE_KEY);
            Ok(Emitted::Written(node))
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
    let framing = node.get(SSE_KEY).and_then(|n| n.to_value().ok());
    node.remove(SSE_KEY);
    let data = serde_json::to_string(&node).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not serialize a stream event ({e}). This is a bug in Kaeso; please report it."
        ))
    })?;
    let mut event = SseEvent::new(name.as_deref(), data);
    if let Some(framing) = framing {
        crate::sse::set_framing(&mut event, &framing);
    }
    write_item(out, &SseItem::Event(event));
    Ok(())
}

// `write_node` without copying the node when it is an object: the same event name, framing and
// data, the data serialized without the two marker keys instead of from a copy that lost them.
fn write_obj(out: &mut Vec<u8>, node: &Node) -> Result<(), KppError> {
    let Node::Obj(entries) = node else {
        return write_node(out, node.clone());
    };
    let name = if entries.contains_key(EVENT_KEY) {
        node.get_str(EVENT_KEY)
    } else {
        node.get_str("type")
    };
    let framing = entries.get(SSE_KEY).and_then(|n| n.to_value().ok());
    let data = serde_json::to_string(&Unmarked(entries)).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not serialize a stream event ({e}). This is a bug in Kaeso; please report it."
        ))
    })?;
    let mut event = SseEvent::new(name.as_deref(), data);
    if let Some(framing) = framing {
        crate::sse::set_framing(&mut event, &framing);
    }
    write_item(out, &SseItem::Event(event));
    Ok(())
}

struct Unmarked<'a>(&'a indexmap::IndexMap<String, Node>);

impl serde::Serialize for Unmarked<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let kept = self
            .0
            .iter()
            .filter(|(k, _)| *k != EVENT_KEY && *k != SSE_KEY);
        let mut map = serializer.serialize_map(Some(kept.clone().count()))?;
        for (k, v) in kept {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

pub(super) fn write_extra(
    out: &mut Vec<u8>,
    raw: &RawJson,
) -> Result<(Option<i64>, bool), KppError> {
    #[derive(serde::Deserialize)]
    struct SequenceOnly {
        sequence_number: Option<i64>,
    }
    let written = crate::sse::write_frame_extra(out, raw.as_raw()).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not write a stream event it kept: {e}. This is a bug in Kaeso; please report it."
        ))
    })?;
    Ok(match written {
        crate::sse::WrittenFrame::Comment => (None, false),
        crate::sse::WrittenFrame::Event(data) => {
            let seq = data
                .and_then(|d| serde_json::from_str::<SequenceOnly>(d.get()).ok())
                .and_then(|d| d.sequence_number);
            (seq, true)
        }
    })
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
                let (seq, is_event) = write_extra(&mut out, &item.raw)?;
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
