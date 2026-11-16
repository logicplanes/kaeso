use std::collections::HashMap;

use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, KppError, Opaque, OpaqueKind, Origin,
    ReasoningHeader, ResponseDelta, ResponseStart, ResponseStop, StopReason, StreamEvent,
    ToolCallHeader, Usage,
};
use serde_json::Value;

use super::D;
use super::error::in_stream;
use super::eventstream::{Framer, Message};
use super::response::{bad, stop_reason, usage};
use crate::codec::{EgressCtx, StreamDecoder};

/// Reads `ConverseStream`'s events (<https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html>):
/// `messageStart`, then per block an optional `contentBlockStart` (tool calls only), its
/// `contentBlockDelta`s and `contentBlockStop`, then `messageStop` with the stop reason and
/// `metadata` with the usage.
pub(crate) struct Decoder {
    framer: Framer,
    ctx: EgressCtx,
    origin: Origin,
    blocks: HashMap<u64, (u32, bool)>,
    next: u32,
    started: bool,
    stop: Option<StopReason>,
    usage: Option<Usage>,
    done: bool,
    inert: bool,
}

impl Decoder {
    pub(crate) fn new(ctx: &EgressCtx) -> Self {
        Decoder {
            framer: Framer::new(),
            origin: ctx.origin(D),
            ctx: ctx.clone(),
            blocks: HashMap::new(),
            next: 0,
            started: false,
            stop: None,
            usage: None,
            done: false,
            inert: false,
        }
    }

    fn fail(&mut self, e: KppError, out: &mut Vec<StreamEvent>) {
        out.push(StreamEvent::Error(e));
        self.inert = true;
    }

    fn start(&mut self, out: &mut Vec<StreamEvent>) {
        if !self.started {
            self.started = true;
            out.push(StreamEvent::ResponseStart(ResponseStart::new("", "")));
        }
    }

    // The KPP index of Converse's block `index`, opened with `header` the first time it's seen.
    fn open(&mut self, index: u64, header: BlockHeader, out: &mut Vec<StreamEvent>) -> u32 {
        self.start(out);
        if let Some((kpp, _)) = self.blocks.get(&index) {
            return *kpp;
        }
        let kpp = self.next;
        self.next += 1;
        self.blocks.insert(index, (kpp, true));
        out.push(StreamEvent::BlockStart(BlockStart::new(kpp, header)));
        kpp
    }

    fn message(&mut self, m: Message, out: &mut Vec<StreamEvent>) -> Result<(), KppError> {
        match m.message_type.as_deref() {
            Some("exception") => {
                let ty = m.exception_type.unwrap_or_default();
                return Err(in_stream(&ty, &m.payload));
            }
            Some("error") => {
                let code = m.error_code.unwrap_or_default();
                let message = m
                    .error_message
                    .unwrap_or_else(|| format!("Amazon Bedrock ended the stream with {code}."));
                return Err(in_stream(
                    &code,
                    serde_json::json!({ "message": message })
                        .to_string()
                        .as_bytes(),
                ));
            }
            _ => {}
        }
        let v: Value =
            serde_json::from_slice(&m.payload).map_err(|_| bad("an event isn't JSON"))?;
        let index = || {
            v.get("contentBlockIndex")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        match m.event_type.as_deref().unwrap_or_default() {
            "messageStart" => self.start(out),
            "contentBlockStart" => {
                if let Some(t) = v.pointer("/start/toolUse") {
                    let id = t
                        .get("toolUseId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let name = t.get("name").and_then(Value::as_str).unwrap_or_default();
                    let header = ToolCallHeader::new(
                        self.ctx.original_tool_id(id),
                        self.ctx.original_tool_name(name),
                    );
                    self.open(index(), BlockHeader::ToolCall(header), out);
                }
            }
            "contentBlockDelta" => {
                let Some(delta) = v.get("delta") else {
                    return Ok(());
                };
                if let Some(text) = delta.get("text").and_then(Value::as_str) {
                    let kpp = self.open(index(), BlockHeader::Text, out);
                    out.push(StreamEvent::BlockDelta(BlockDelta::new(
                        kpp,
                        Delta::Text(text.to_owned()),
                    )));
                } else if let Some(input) = delta.pointer("/toolUse/input").and_then(Value::as_str)
                {
                    let kpp = self.open(
                        index(),
                        BlockHeader::ToolCall(ToolCallHeader::new("", "")),
                        out,
                    );
                    out.push(StreamEvent::BlockDelta(BlockDelta::new(
                        kpp,
                        Delta::ToolArguments(input.to_owned()),
                    )));
                } else if let Some(r) = delta.get("reasoningContent") {
                    let header = BlockHeader::Reasoning(ReasoningHeader::new(self.origin.clone()));
                    let kpp = self.open(index(), header, out);
                    if let Some(text) = r.get("text").and_then(Value::as_str) {
                        out.push(StreamEvent::BlockDelta(BlockDelta::new(
                            kpp,
                            Delta::ReasoningText(text.to_owned()),
                        )));
                    }
                    for (key, kind) in [
                        ("signature", OpaqueKind::BedrockReasoningSignature),
                        ("redactedContent", OpaqueKind::BedrockRedactedReasoning),
                    ] {
                        if let Some(data) = r.get(key).and_then(Value::as_str) {
                            let o = Opaque::new(self.origin.clone(), kind, data);
                            out.push(StreamEvent::BlockDelta(BlockDelta::new(
                                kpp,
                                Delta::Opaque(o),
                            )));
                        }
                    }
                }
            }
            "contentBlockStop" => {
                if let Some((kpp, open)) = self.blocks.get_mut(&index())
                    && *open
                {
                    *open = false;
                    out.push(StreamEvent::BlockStop(BlockStop::new(*kpp)));
                }
            }
            "messageStop" => {
                self.start(out);
                self.close_blocks(out);
                let reason = v
                    .get("stopReason")
                    .and_then(Value::as_str)
                    .unwrap_or("end_turn");
                self.stop = Some(stop_reason(reason));
            }
            "metadata" => {
                if let Some(u) = v.get("usage") {
                    self.usage = Some(usage(u, &self.origin));
                }
                self.end(out);
            }
            _ => {}
        }
        Ok(())
    }

    fn close_blocks(&mut self, out: &mut Vec<StreamEvent>) {
        let mut open: Vec<u32> = self
            .blocks
            .values_mut()
            .filter(|(_, open)| *open)
            .map(|(kpp, open)| {
                *open = false;
                *kpp
            })
            .collect();
        open.sort_unstable();
        out.extend(
            open.into_iter()
                .map(|k| StreamEvent::BlockStop(BlockStop::new(k))),
        );
    }

    fn end(&mut self, out: &mut Vec<StreamEvent>) {
        if self.done || self.stop.is_none() {
            return;
        }
        self.done = true;
        out.push(StreamEvent::ResponseDelta(ResponseDelta::new(
            self.stop.take(),
            self.usage.take(),
        )));
        out.push(StreamEvent::ResponseStop(ResponseStop::new()));
    }
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        match self.framer.feed(bytes) {
            Ok(messages) => {
                for m in messages {
                    if let Err(e) = self.message(m, &mut out) {
                        self.fail(e, &mut out);
                        break;
                    }
                }
            }
            Err(e) => self.fail(bad(&e.0), &mut out),
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.inert {
            return out;
        }
        if self.framer.has_partial() {
            self.fail(bad("the stream ended in the middle of a message"), &mut out);
        } else if self.stop.is_some() {
            self.end(&mut out);
        } else if !self.done {
            self.fail(
                KppError::provider(
                    "Amazon Bedrock's Converse stream ended before the response was complete.",
                )
                .with_origin(D),
                &mut out,
            );
        }
        self.inert = true;
        out
    }

    fn flush(&mut self) -> Vec<StreamEvent> {
        self.inert = true;
        Vec::new()
    }
}
