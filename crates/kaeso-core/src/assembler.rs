use std::collections::BTreeMap;

use kaeso_codecs::dispatch;
use kaeso_kpp::{
    Block, BlockHeader, Delta, DialectId, Extras, KppError, RawItem, RawJson, ReasoningBlock,
    RefusalBlock, Response, StopReason, StreamEvent, TextBlock, ToolCallBlock, Usage,
};

enum Body {
    Text(TextBlock),
    Tool {
        id: String,
        name: String,
        arguments: String,
        extras: Extras,
    },
    Reasoning(ReasoningBlock, String, BTreeMap<u32, String>),
    Refusal(RefusalBlock),
    Unknown(RawItem, Vec<RawItem>),
}

struct Slot {
    body: Body,
    unknown: Vec<RawItem>,
}

pub struct Assembler {
    origin: DialectId,
    id: String,
    model: String,
    started: bool,
    slots: BTreeMap<u32, Slot>,
    unplaced: Vec<RawItem>,
    stop: Option<StopReason>,
    usage: Option<Usage>,
    extras: Extras,
    complete: bool,
    error: Option<KppError>,
    blocks: bool,
}

impl std::fmt::Debug for Assembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Assembler")
            .field("id", &self.id)
            .field("blocks", &self.slots.len())
            .field("complete", &self.complete)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assembled {
    pub response: Option<Response>,
    pub complete: bool,
    pub error: Option<KppError>,
}

fn kept(origin: DialectId, value: &impl serde::Serialize) -> RawItem {
    RawItem::new(
        origin,
        RawJson::from_serialize(value).unwrap_or_else(|_| RawJson::null()),
    )
}

fn merge(into: &mut Extras, from: &Extras) {
    if from.is_empty() {
        return;
    }
    if into.is_empty() {
        into.origin.clone_from(&from.origin);
    } else if into.origin != from.origin {
        return;
    }
    into.fields
        .extend(from.fields.iter().map(|(k, v)| (k.clone(), v.clone())));
    into.form
        .extend(from.form.iter().map(|(k, v)| (k.clone(), v.clone())));
}

impl Body {
    fn extras(&mut self) -> Option<&mut Extras> {
        match self {
            Body::Text(t) => Some(&mut t.extras),
            Body::Tool { extras, .. } => Some(extras),
            Body::Reasoning(r, _, _) => Some(&mut r.extras),
            Body::Refusal(r) => Some(&mut r.extras),
            Body::Unknown(..) => None,
        }
    }

    fn start(header: &BlockHeader, origin: DialectId) -> Body {
        match header {
            BlockHeader::Text => Body::Text(TextBlock::new("")),
            BlockHeader::ToolCall(h) => Body::Tool {
                id: h.id.clone(),
                name: h.name.clone(),
                arguments: String::new(),
                extras: Extras::default(),
            },
            BlockHeader::Reasoning(h) => {
                let mut r = ReasoningBlock::default();
                r.origin.clone_from(&h.origin);
                r.opaque.clone_from(&h.opaque);
                Body::Reasoning(r, String::new(), BTreeMap::new())
            }
            BlockHeader::Refusal => Body::Refusal(RefusalBlock::new("")),
            BlockHeader::Unknown(raw) => Body::Unknown(raw.clone(), Vec::new()),
            other => Body::Unknown(kept(origin, other), Vec::new()),
        }
    }
}

impl Slot {
    fn feed(&mut self, delta: &Delta, origin: DialectId) {
        match (&mut self.body, delta) {
            (Body::Text(t), Delta::Text(s)) => t.text.push_str(s),
            (Body::Text(t), Delta::Citation(c)) => t.citations.push(c.clone()),
            (Body::Tool { arguments, .. }, Delta::ToolArguments(s)) => arguments.push_str(s),
            (Body::Reasoning(_, text, _), Delta::ReasoningText(s)) => text.push_str(s),
            (Body::Reasoning(_, _, summaries), Delta::ReasoningSummary(s)) => summaries
                .entry(s.summary_index)
                .or_default()
                .push_str(&s.text),
            (Body::Reasoning(r, _, _), Delta::Opaque(o)) => r.opaque.push(o.clone()),
            (Body::Refusal(r), Delta::Refusal(s)) => r.text.push_str(s),
            (Body::Unknown(_, deltas), Delta::Unknown(item)) => deltas.push(item.clone()),
            (_, Delta::Unknown(item)) => self.unknown.push(item.clone()),
            (_, other) => self.unknown.push(kept(origin, other)),
        }
    }
}

impl Assembler {
    pub fn new(origin: DialectId) -> Assembler {
        Assembler {
            origin,
            id: String::new(),
            model: String::new(),
            started: false,
            slots: BTreeMap::new(),
            unplaced: Vec::new(),
            stop: None,
            usage: None,
            extras: Extras::default(),
            complete: false,
            error: None,
            blocks: true,
        }
    }

    pub fn without_blocks(origin: DialectId) -> Assembler {
        Assembler {
            blocks: false,
            ..Assembler::new(origin)
        }
    }

    pub fn feed(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::BlockStart(_) if !self.blocks => self.started = true,
            StreamEvent::BlockDelta(_) | StreamEvent::BlockStop(_) | StreamEvent::Extra(_)
                if !self.blocks => {}
            StreamEvent::ResponseStart(s) => {
                self.started = true;
                self.id.clone_from(&s.id);
                self.model.clone_from(&s.model);
                if let Some(u) = &s.usage {
                    self.usage = Some(u.clone());
                }
                merge(&mut self.extras, &s.extras);
            }
            StreamEvent::BlockStart(s) => {
                self.started = true;
                let mut slot = Slot {
                    body: Body::start(&s.block, self.origin),
                    unknown: Vec::new(),
                };
                if let Some(extras) = slot.body.extras() {
                    merge(extras, &s.extras);
                }
                self.slots.insert(s.index, slot);
            }
            StreamEvent::BlockDelta(d) => match self.slots.get_mut(&d.index) {
                Some(slot) => {
                    slot.feed(&d.delta, self.origin);
                    if let Some(extras) = slot.body.extras() {
                        merge(extras, &d.extras);
                    }
                }
                None => self.unplaced.push(match &d.delta {
                    Delta::Unknown(item) => item.clone(),
                    other => kept(self.origin, other),
                }),
            },
            StreamEvent::BlockStop(s) => {
                if let Some(extras) = self.slots.get_mut(&s.index).and_then(|s| s.body.extras()) {
                    merge(extras, &s.extras);
                }
            }
            StreamEvent::ResponseDelta(d) => {
                if let Some(stop) = &d.stop {
                    self.stop = Some(stop.clone());
                }
                if let Some(u) = &d.usage {
                    self.usage = Some(u.clone());
                }
                merge(&mut self.extras, &d.extras);
            }
            StreamEvent::ResponseStop(s) => {
                self.complete = true;
                merge(&mut self.extras, &s.extras);
            }
            StreamEvent::Error(e) => self.error = Some(e.clone()),
            StreamEvent::Extra(item) => self.unplaced.push(item.clone()),
            other if self.blocks => self.unplaced.push(kept(self.origin, other)),
            _ => {}
        }
    }

    pub fn finish(self) -> Assembled {
        let response = self.started.then(|| {
            let mut blocks = Vec::new();
            for slot in self.slots.into_values() {
                blocks.push(into_block(slot.body, self.origin));
                blocks.extend(slot.unknown.into_iter().map(Block::Unknown));
            }
            blocks.extend(
                self.unplaced
                    .into_iter()
                    .filter(|item| !dispatch::reports_item_progress(self.origin, item))
                    .map(Block::Unknown),
            );
            let mut r = Response::new(self.id, self.model, blocks);
            r.stop = self.stop.unwrap_or_default();
            r.usage = self.usage.unwrap_or_default();
            r.extras = self.extras;
            r
        });
        Assembled {
            response,
            complete: self.complete,
            error: self.error,
        }
    }
}

fn unknown_with_deltas(start: RawItem, deltas: Vec<RawItem>) -> RawItem {
    if deltas.is_empty() {
        return start;
    }
    let raw = RawJson::from_serialize(&serde_json::json!({
        "start": start.raw,
        "deltas": deltas.iter().map(|d| &d.raw).collect::<Vec<_>>(),
    }))
    .unwrap_or_else(|_| start.raw.clone());
    RawItem::new(start.origin, raw)
}

fn into_block(body: Body, origin: DialectId) -> Block {
    match body {
        Body::Text(t) => Block::Text(t),
        Body::Tool {
            id,
            name,
            arguments,
            extras,
        } => {
            let raw = if arguments.trim().is_empty() {
                RawJson::empty_object()
            } else {
                RawJson::from_string(arguments.clone())
                    .or_else(|_| RawJson::from_serialize(&arguments))
                    .unwrap_or_else(|_| RawJson::empty_object())
            };
            let mut call = ToolCallBlock::new(id, name, raw);
            call.extras = extras;
            Block::ToolCall(call)
        }
        Body::Reasoning(mut r, text, summaries) => {
            if !text.is_empty() {
                r.text.push(text);
            }
            r.summary.extend(summaries.into_values());
            Block::Reasoning(r)
        }
        Body::Refusal(r) => Block::Refusal(r),
        Body::Unknown(start, deltas) => Block::Unknown(
            dispatch::finished_item(origin, &start, &deltas)
                .unwrap_or_else(|| unknown_with_deltas(start, deltas)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{BlockDelta, BlockStart, DialectId, ResponseStart, ResponseStop};
    use serde_json::json;

    use super::*;

    fn raw(v: serde_json::Value) -> RawItem {
        RawItem::new(
            DialectId::AnthropicMessages,
            RawJson::from_serialize(&v).unwrap(),
        )
    }

    fn unknown(b: &Block) -> serde_json::Value {
        match b {
            Block::Unknown(i) => i.raw.to_value().unwrap(),
            other => panic!("{other:?} is not unknown"),
        }
    }

    #[test]
    fn unknown_items_stay_in_the_reply_in_order() {
        let mut a = Assembler::new(DialectId::AnthropicMessages);
        let extras = Extras::new(DialectId::AnthropicMessages).with_field(
            "container",
            RawJson::from_string("{\"id\":\"c\"}".into()).unwrap(),
        );
        let mut start = ResponseStart::new("msg_1", "claude");
        start.extras = extras.clone();
        a.feed(&StreamEvent::ResponseStart(start));
        a.feed(&StreamEvent::Extra(raw(json!({"event": "ping"}))));
        a.feed(&StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Text,
        )));
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Text("Hi".into()),
        )));
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Unknown(raw(json!({"type": "future_delta"}))),
        )));
        a.feed(&StreamEvent::BlockStart(BlockStart::new(
            1,
            BlockHeader::Unknown(raw(json!({"type": "server_tool_use", "input": {}}))),
        )));
        for part in ["{\"q\":", "\"kaeso\"}"] {
            a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
                1,
                Delta::Unknown(raw(
                    json!({"type": "input_json_delta", "partial_json": part}),
                )),
            )));
        }
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            7,
            Delta::Unknown(raw(json!({"type": "orphan"}))),
        )));
        a.feed(&StreamEvent::ResponseStop(ResponseStop::new()));
        let r = a.finish().response.unwrap();
        assert_eq!(r.extras, extras);
        assert_eq!(r.blocks.len(), 5);
        assert!(matches!(&r.blocks[0], Block::Text(t) if t.text == "Hi"));
        assert_eq!(unknown(&r.blocks[1]), json!({"type": "future_delta"}));
        assert_eq!(
            unknown(&r.blocks[2]),
            json!({
                "start": {"type": "server_tool_use", "input": {}},
                "deltas": [
                    {"type": "input_json_delta", "partial_json": "{\"q\":"},
                    {"type": "input_json_delta", "partial_json": "\"kaeso\"}"},
                ],
            })
        );
        assert_eq!(unknown(&r.blocks[3]), json!({"event": "ping"}));
        assert_eq!(unknown(&r.blocks[4]), json!({"type": "orphan"}));
    }

    #[test]
    fn an_unknown_block_without_deltas_stays_as_it_came() {
        let mut a = Assembler::new(DialectId::AnthropicMessages);
        a.feed(&StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Unknown(raw(json!({"type": "web_search_tool_result"}))),
        )));
        let r = a.finish().response.unwrap();
        assert_eq!(
            unknown(&r.blocks[0]),
            json!({"type": "web_search_tool_result"})
        );
    }

    #[test]
    fn block_extras_reach_the_block_and_never_mix_dialects() {
        let mut a = Assembler::new(DialectId::AnthropicMessages);
        let mut start = BlockStart::new(0, BlockHeader::Text);
        start.extras = Extras::new(DialectId::OpenAiResponses)
            .with_field("id", RawJson::from_string("\"msg_9\"".into()).unwrap());
        a.feed(&StreamEvent::BlockStart(start));
        let mut d = BlockDelta::new(0, Delta::Text("x".into()));
        d.extras = Extras::new(DialectId::AnthropicMessages)
            .with_field("other", RawJson::from_string("1".into()).unwrap());
        a.feed(&StreamEvent::BlockDelta(d));
        let r = a.finish().response.unwrap();
        let Block::Text(t) = &r.blocks[0] else {
            panic!("not text")
        };
        assert!(t.extras.is_from(DialectId::OpenAiResponses));
        assert_eq!(t.extras.fields.len(), 1);
    }

    #[test]
    fn deltas_that_fit_no_block_stay_in_the_reply() {
        let mut a = Assembler::new(DialectId::OpenAiResponses);
        a.feed(&StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Text,
        )));
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Text("Hi".into()),
        )));
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ToolArguments("{\"q\":1}".into()),
        )));
        a.feed(&StreamEvent::BlockDelta(BlockDelta::new(
            4,
            Delta::Refusal("no".into()),
        )));
        let r = a.finish().response.unwrap();
        assert_eq!(r.blocks.len(), 3);
        assert!(matches!(&r.blocks[0], Block::Text(t) if t.text == "Hi"));
        let Block::Unknown(misplaced) = &r.blocks[1] else {
            panic!("{:?} is not unknown", r.blocks[1])
        };
        assert!(misplaced.origin.is(DialectId::OpenAiResponses));
        assert_eq!(
            unknown(&r.blocks[1]),
            json!({"tool_arguments": "{\"q\":1}"})
        );
        assert_eq!(unknown(&r.blocks[2]), json!({"refusal": "no"}));
    }
}
