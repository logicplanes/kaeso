use std::collections::BTreeMap;

use kaeso_kpp::{
    Block, BlockHeader, Delta, KppError, RawJson, ReasoningBlock, RefusalBlock, Response,
    StopReason, StreamEvent, TextBlock, ToolCallBlock, Usage,
};

enum Slot {
    Text(TextBlock),
    Tool {
        id: String,
        name: String,
        arguments: String,
    },
    Reasoning(ReasoningBlock, String, BTreeMap<u32, String>),
    Refusal(RefusalBlock),
    Unknown(Block),
}

#[derive(Default)]
pub struct Assembler {
    id: String,
    model: String,
    started: bool,
    slots: BTreeMap<u32, Slot>,
    stop: Option<StopReason>,
    usage: Option<Usage>,
    complete: bool,
    error: Option<KppError>,
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

impl Assembled {
    pub fn usage(&self) -> Usage {
        self.response
            .as_ref()
            .map(|r| r.usage.clone())
            .unwrap_or_default()
    }
}

impl Assembler {
    pub fn new() -> Assembler {
        Assembler::default()
    }

    pub fn feed(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ResponseStart(s) => {
                self.started = true;
                self.id.clone_from(&s.id);
                self.model.clone_from(&s.model);
                if let Some(u) = &s.usage {
                    self.usage = Some(u.clone());
                }
            }
            StreamEvent::BlockStart(s) => {
                self.started = true;
                let slot = match &s.block {
                    BlockHeader::Text => Slot::Text(TextBlock::new("")),
                    BlockHeader::ToolCall(h) => Slot::Tool {
                        id: h.id.clone(),
                        name: h.name.clone(),
                        arguments: String::new(),
                    },
                    BlockHeader::Reasoning(h) => {
                        let mut r = ReasoningBlock::default();
                        r.origin.clone_from(&h.origin);
                        r.opaque.clone_from(&h.opaque);
                        Slot::Reasoning(r, String::new(), BTreeMap::new())
                    }
                    BlockHeader::Refusal => Slot::Refusal(RefusalBlock::new("")),
                    BlockHeader::Unknown(raw) => Slot::Unknown(Block::Unknown(raw.clone())),
                    _ => Slot::Unknown(Block::text("")),
                };
                self.slots.insert(s.index, slot);
            }
            StreamEvent::BlockDelta(d) => {
                let Some(slot) = self.slots.get_mut(&d.index) else {
                    return;
                };
                match (slot, &d.delta) {
                    (Slot::Text(t), Delta::Text(s)) => t.text.push_str(s),
                    (Slot::Text(t), Delta::Citation(c)) => t.citations.push(c.clone()),
                    (Slot::Tool { arguments, .. }, Delta::ToolArguments(s)) => {
                        arguments.push_str(s)
                    }
                    (Slot::Reasoning(_, text, _), Delta::ReasoningText(s)) => text.push_str(s),
                    (Slot::Reasoning(_, _, summaries), Delta::ReasoningSummary(s)) => summaries
                        .entry(s.summary_index)
                        .or_default()
                        .push_str(&s.text),
                    (Slot::Reasoning(r, _, _), Delta::Opaque(o)) => r.opaque.push(o.clone()),
                    (Slot::Refusal(r), Delta::Refusal(s)) => r.text.push_str(s),
                    _ => {}
                }
            }
            StreamEvent::ResponseDelta(d) => {
                if let Some(stop) = &d.stop {
                    self.stop = Some(stop.clone());
                }
                if let Some(u) = &d.usage {
                    self.usage = Some(u.clone());
                }
            }
            StreamEvent::ResponseStop(_) => self.complete = true,
            StreamEvent::Error(e) => self.error = Some(e.clone()),
            StreamEvent::BlockStop(_) | StreamEvent::Extra(_) | _ => {}
        }
    }

    pub fn finish(self) -> Assembled {
        let response = self.started.then(|| {
            let blocks = self.slots.into_values().map(into_block).collect();
            let mut r = Response::new(self.id, self.model, blocks);
            r.stop = self.stop.unwrap_or_default();
            r.usage = self.usage.unwrap_or_default();
            r
        });
        Assembled {
            response,
            complete: self.complete,
            error: self.error,
        }
    }
}

fn into_block(slot: Slot) -> Block {
    match slot {
        Slot::Text(t) => Block::Text(t),
        Slot::Tool {
            id,
            name,
            arguments,
        } => {
            let raw = if arguments.trim().is_empty() {
                RawJson::empty_object()
            } else {
                RawJson::from_string(arguments.clone())
                    .or_else(|_| RawJson::from_serialize(&arguments))
                    .unwrap_or_else(|_| RawJson::empty_object())
            };
            Block::ToolCall(ToolCallBlock::new(id, name, raw))
        }
        Slot::Reasoning(mut r, text, summaries) => {
            if !text.is_empty() {
                r.text.push(text);
            }
            r.summary.extend(summaries.into_values());
            Block::Reasoning(r)
        }
        Slot::Refusal(r) => Block::Refusal(r),
        Slot::Unknown(b) => b,
    }
}
