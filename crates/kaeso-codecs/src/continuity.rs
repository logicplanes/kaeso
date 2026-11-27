use std::collections::{HashMap, HashSet};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, Delta, DialectId, GenerateRequest, KppError, OpaqueKind,
    ReasoningBlock, Role, StreamEvent,
};
use serde::{Deserialize, Serialize};

pub const REASONING_BLOB_PREFIX: &str = "kpp:reasoning:v1:";

pub(crate) const MAX_SUMMARY_PARTS: u32 = 4096;

pub(crate) fn summary_part(summary: &mut Vec<String>, index: u32) -> Result<&mut String, KppError> {
    if index >= MAX_SUMMARY_PARTS {
        return Err(KppError::provider(format!(
            "The provider sent reasoning summary part {index}; Kaeso keeps at most {MAX_SUMMARY_PARTS} parts per block."
        )));
    }
    let slot = usize::try_from(index).unwrap_or(usize::MAX);
    if summary.len() <= slot {
        summary.resize_with(slot.saturating_add(1), String::new);
    }
    summary.get_mut(slot).ok_or_else(|| {
        KppError::provider("Kaeso could not keep a reasoning summary part.".to_owned())
    })
}

pub(crate) fn new_reasoning_part(d: &BlockDelta) -> bool {
    d.extras.form_flag(
        DialectId::OpenAiResponses,
        crate::openai_responses::stream::NEW_PART,
    )
}

pub(crate) fn add_reasoning_delta(
    block: &mut ReasoningBlock,
    delta: &Delta,
    new_part: bool,
) -> Result<(), KppError> {
    match delta {
        Delta::ReasoningText(t) => match block.text.last_mut() {
            Some(last) if !new_part => last.push_str(t),
            _ => block.text.push(t.clone()),
        },
        Delta::ReasoningSummary(s) => {
            summary_part(&mut block.summary, s.summary_index)?.push_str(&s.text)
        }
        Delta::Opaque(o) => block.opaque.push(o.clone()),
        _ => {}
    }
    Ok(())
}

pub fn wrap_reasoning(block: &ReasoningBlock) -> String {
    let mut block = block.clone();
    block.extras.form.clear();
    let json = serde_json::to_vec(&block).unwrap_or_default();
    format!("{REASONING_BLOB_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
}

pub fn unwrap_reasoning(data: &str) -> Option<ReasoningBlock> {
    let encoded = data.strip_prefix(REASONING_BLOB_PREFIX)?;
    let json = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let block: ReasoningBlock = serde_json::from_slice(&json).ok()?;
    block.origin.is_some().then_some(block)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CarriedReasoning {
    pub tool_call_id: String,
    pub blocks: Vec<ReasoningBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turn_calls: Vec<String>,
}

impl CarriedReasoning {
    pub fn new(tool_call_id: impl Into<String>, blocks: Vec<ReasoningBlock>) -> Self {
        CarriedReasoning {
            tool_call_id: tool_call_id.into(),
            blocks,
            turn_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_turn_calls(mut self, calls: Vec<String>) -> Self {
        self.turn_calls = calls;
        self
    }
}

fn replayable(block: &ReasoningBlock) -> bool {
    block.opaque.iter().any(|o| {
        !o.data.is_empty()
            && matches!(
                o.kind,
                OpaqueKind::AnthropicThinkingSignature
                    | OpaqueKind::AnthropicRedactedThinking
                    | OpaqueKind::OpenAiEncryptedReasoning
                    | OpaqueKind::GeminiThoughtSignature
                    | OpaqueKind::BedrockReasoningSignature
                    | OpaqueKind::BedrockRedactedReasoning
            )
    })
}

/// The reasoning before each tool call that a provider must get back signed, as an OpenAI
/// Chat Completions or Gemini app can't carry it.
pub fn reasoning_for_tool_calls(blocks: &[Block]) -> Vec<CarriedReasoning> {
    carried(blocks, replayable)
}

/// Every reasoning block before each tool call, as an OpenAI Responses reply hands it to the app
/// for the next turn; an app that references the reply's reasoning instead of sending it back
/// gets it from what was kept.
pub fn every_reasoning_for_tool_calls(blocks: &[Block]) -> Vec<CarriedReasoning> {
    carried(blocks, |_| true)
}

fn carried(blocks: &[Block], keep: fn(&ReasoningBlock) -> bool) -> Vec<CarriedReasoning> {
    let calls: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall(call) => Some(call.id.clone()),
            _ => None,
        })
        .collect();
    let mut kept = Vec::new();
    let mut run: Vec<ReasoningBlock> = Vec::new();
    for block in blocks {
        match block {
            Block::Reasoning(r) if keep(r) => run.push(r.clone()),
            Block::ToolCall(call) if !run.is_empty() => {
                kept.push(
                    CarriedReasoning::new(call.id.clone(), std::mem::take(&mut run))
                        .with_turn_calls(calls.clone()),
                );
            }
            _ => {}
        }
    }
    kept
}

#[derive(Clone, Debug, Default)]
pub struct ReasoningCollector {
    order: Vec<(u32, Started)>,
    every: bool,
}

#[derive(Clone, Debug)]
enum Started {
    Reasoning(Box<ReasoningBlock>),
    ToolCall(String),
}

impl ReasoningCollector {
    pub fn new() -> Self {
        ReasoningCollector::default()
    }

    /// A collector that keeps every reasoning block before a tool call, as
    /// [`every_reasoning_for_tool_calls`] does.
    pub fn every() -> Self {
        ReasoningCollector {
            order: Vec::new(),
            every: true,
        }
    }

    pub fn observe(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::BlockStart(s) => match &s.block {
                BlockHeader::Reasoning(h) => {
                    let mut block = ReasoningBlock::default();
                    block.origin.clone_from(&h.origin);
                    block.opaque.clone_from(&h.opaque);
                    self.order
                        .push((s.index, Started::Reasoning(Box::new(block))));
                }
                BlockHeader::ToolCall(h) => {
                    self.order.push((s.index, Started::ToolCall(h.id.clone())));
                }
                _ => {}
            },
            StreamEvent::BlockDelta(d) => {
                let Some(Started::Reasoning(block)) = self
                    .order
                    .iter_mut()
                    .rev()
                    .find(|(i, _)| *i == d.index)
                    .map(|(_, b)| b)
                else {
                    return;
                };
                if add_reasoning_delta(block, &d.delta, new_reasoning_part(d)).is_err() {
                    self.order.retain(|(i, _)| *i != d.index);
                }
            }
            _ => {}
        }
    }

    pub fn finish(&mut self) -> Vec<CarriedReasoning> {
        let blocks: Vec<Block> = std::mem::take(&mut self.order)
            .into_iter()
            .map(|(_, started)| match started {
                Started::Reasoning(r) => Block::Reasoning(*r),
                Started::ToolCall(id) => Block::ToolCall(kaeso_kpp::ToolCallBlock::new(
                    id,
                    "",
                    kaeso_kpp::RawJson::null(),
                )),
            })
            .collect();
        if self.every {
            every_reasoning_for_tool_calls(&blocks)
        } else {
            reasoning_for_tool_calls(&blocks)
        }
    }
}

fn carries_reasoning(message: &kaeso_kpp::Message) -> bool {
    message
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Reasoning(r) if replayable(r)))
}

pub fn tool_calls_missing_reasoning(req: &GenerateRequest) -> Vec<String> {
    req.messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter(|m| !carries_reasoning(m))
        .flat_map(|m| &m.blocks)
        .filter_map(|b| match b {
            Block::ToolCall(c) => Some(c.id.clone()),
            _ => None,
        })
        .collect()
}

pub fn restore_reasoning(req: &mut GenerateRequest, kept: &[CarriedReasoning]) -> usize {
    if kept.is_empty() {
        return 0;
    }
    let by_call = by_call(kept);
    let mut inserted = 0;
    for message in &mut req.messages {
        if message.role != Role::Assistant || carries_reasoning(message) {
            continue;
        }
        inserted += put_back(message, &by_call);
    }
    inserted
}

/// The tool calls of the turns in which an OpenAI Responses app points to earlier reasoning
/// with an `item_reference` instead of sending it.
pub fn referenced_tool_calls(req: &GenerateRequest) -> Vec<String> {
    req.messages
        .iter()
        .filter(|m| m.role == Role::Assistant && references(m).next().is_some())
        .flat_map(|m| &m.blocks)
        .filter_map(|b| match b {
            Block::ToolCall(c) => Some(c.id.clone()),
            _ => None,
        })
        .collect()
}

/// What became of a request's `item_reference` items to earlier reasoning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Resolved {
    /// Reasoning blocks put back from what was kept.
    pub restored: usize,
    /// References in turns without tool calls, where no provider needs the reasoning back.
    pub left_out: usize,
    /// The size of the restored reasoning's text, summaries and opaque data, in bytes.
    pub restored_bytes: usize,
}

/// Puts the reasoning kept for each turn's tool calls where an OpenAI Responses app's
/// `item_reference` points to it, in place of the references, and leaves out references in
/// turns without tool calls. The error is the id of the first reference in a turn with tool
/// calls that has nothing kept.
pub fn resolve_reasoning_references(
    req: &mut GenerateRequest,
    kept: &[CarriedReasoning],
) -> Result<Resolved, String> {
    let by_call = by_call(kept);
    let mut resolved = Resolved::default();
    let mut emptied = vec![false; req.messages.len()];
    for (message, emptied) in req.messages.iter_mut().zip(&mut emptied) {
        let Some(first) = references(message).next().map(str::to_owned) else {
            continue;
        };
        let asked = references(message).count();
        let calls = message
            .blocks
            .iter()
            .any(|b| matches!(b, Block::ToolCall(_)));
        if calls {
            let (put, bytes) = put_back(message, &by_call);
            if put < asked {
                return Err(first);
            }
            resolved.restored += put;
            resolved.restored_bytes = resolved.restored_bytes.saturating_add(bytes);
        } else {
            resolved.left_out += asked;
        }
        message.blocks.retain(|b| reference_of(b).is_none());
        *emptied = message.blocks.is_empty() && message.extras.fields.is_empty();
    }
    let mut emptied = emptied.into_iter();
    req.messages.retain(|_| !emptied.next().unwrap_or(false));
    Ok(resolved)
}

fn size(r: &ReasoningBlock) -> usize {
    r.text
        .iter()
        .chain(&r.summary)
        .map(String::len)
        .chain(r.opaque.iter().map(|o| o.data.len()))
        .fold(0, usize::saturating_add)
}

// Reasoning the app sent itself next to its references is the same Kaeso kept, wherever it
// came from, when its text, summaries and opaque data match.
fn same_reasoning(a: &ReasoningBlock, b: &ReasoningBlock) -> bool {
    a.text == b.text
        && a.summary == b.summary
        && a.opaque.len() == b.opaque.len()
        && a.opaque
            .iter()
            .zip(&b.opaque)
            .all(|(x, y)| x.data == y.data)
}

fn reference_of(block: &Block) -> Option<&str> {
    match block {
        Block::Reasoning(r) => crate::openai_responses::reasoning_reference(r),
        _ => None,
    }
}

fn references(message: &kaeso_kpp::Message) -> impl Iterator<Item = &str> {
    message.blocks.iter().filter_map(reference_of)
}

fn by_call(kept: &[CarriedReasoning]) -> HashMap<&str, &CarriedReasoning> {
    let mut by_call: HashMap<&str, &CarriedReasoning> = HashMap::with_capacity(kept.len());
    for entry in kept {
        by_call.entry(entry.tool_call_id.as_str()).or_insert(entry);
    }
    by_call
}

// The kept reasoning of each tool call goes right before that call, and only into the turn
// it came from: a client that merged or split turns gets nothing back.
fn put_back(
    message: &mut kaeso_kpp::Message,
    by_call: &HashMap<&str, &CarriedReasoning>,
) -> (usize, usize) {
    let turn: HashSet<&str> = message
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall(call) => Some(call.id.as_str()),
            _ => None,
        })
        .collect();
    let same_turn = |entry: &CarriedReasoning| {
        entry.turn_calls.is_empty()
            || (entry.turn_calls.len() == turn.len()
                && entry.turn_calls.iter().all(|c| turn.contains(c.as_str())))
    };
    let restored: Vec<(String, Vec<ReasoningBlock>)> = message
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall(call) => by_call
                .get(call.id.as_str())
                .filter(|entry| same_turn(entry))
                .map(|entry| (call.id.clone(), entry.blocks.clone())),
            _ => None,
        })
        .collect();
    if restored.is_empty() {
        return (0, 0);
    }
    let inline: Vec<ReasoningBlock> = message
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Reasoning(r) if reference_of(b).is_none() => Some(r.clone()),
            _ => None,
        })
        .collect();
    let (mut inserted, mut bytes) = (0, 0usize);
    let mut restored: HashMap<String, Vec<ReasoningBlock>> = restored.into_iter().collect();
    let mut blocks = Vec::with_capacity(message.blocks.len());
    let mut since_call = Vec::new();
    for block in std::mem::take(&mut message.blocks) {
        if let Block::ToolCall(call) = &block {
            if let Some(found) = restored.remove(call.id.as_str()) {
                for r in found {
                    if inline.iter().any(|i| same_reasoning(i, &r)) {
                        continue;
                    }
                    inserted += 1;
                    bytes = bytes.saturating_add(size(&r));
                    blocks.push(Block::Reasoning(r));
                }
            }
            blocks.append(&mut since_call);
            blocks.push(block);
        } else {
            since_call.push(block);
        }
    }
    blocks.append(&mut since_call);
    message.blocks = blocks;
    (inserted, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{
        BlockDelta, BlockStart, BlockStop, Extras, Message, Opaque, Origin, RawJson,
        ReasoningHeader, SummaryDelta, ToolCallHeader,
    };

    fn thinking() -> ReasoningBlock {
        let origin = Origin::new(DialectId::AnthropicMessages).with_provider("anthropic");
        let mut b = ReasoningBlock::new(origin.clone());
        b.text.push("Let me think about Paris.".into());
        b.opaque.push(Opaque::new(
            origin.clone(),
            OpaqueKind::AnthropicThinkingSignature,
            "EqQBCkYIBhgCKkA+sig/==",
        ));
        b.extras = Extras::new(origin).with_form("x", true.into());
        b
    }

    #[test]
    fn a_block_survives_the_blob_exactly_without_form_markers() {
        let blob = wrap_reasoning(&thinking());
        assert!(blob.starts_with(REASONING_BLOB_PREFIX), "{blob}");
        let back = unwrap_reasoning(&blob).unwrap();
        let mut want = thinking();
        want.extras.form.clear();
        assert_eq!(back, want);
    }

    #[test]
    fn provider_data_and_damaged_blobs_are_not_blobs() {
        assert!(unwrap_reasoning("EqQBCkYIBhgCKkA+sig/==").is_none());
        assert!(unwrap_reasoning("gAAAAABoXyz").is_none());
        assert!(unwrap_reasoning("kpp:reasoning:v1:not base64!").is_none());
        let no_origin = URL_SAFE_NO_PAD.encode(br#"{"text":["x"]}"#);
        assert!(unwrap_reasoning(&format!("{REASONING_BLOB_PREFIX}{no_origin}")).is_none());
        assert!(unwrap_reasoning("kpp:reasoning:v2:e30").is_none());
    }

    fn signed(text: &str) -> ReasoningBlock {
        let mut b = thinking();
        b.text = vec![text.to_owned()];
        b.extras = Extras::default();
        b
    }

    fn call(id: &str) -> Block {
        Block::tool_call(id, "f", RawJson::from_string("{}".into()).unwrap())
    }

    #[test]
    fn only_replayable_reasoning_before_a_tool_call_is_kept() {
        let unsigned = ReasoningBlock::new(DialectId::AnthropicMessages);
        let mut id_only = ReasoningBlock::new(DialectId::OpenAiResponses);
        id_only.opaque.push(Opaque::new(
            DialectId::OpenAiResponses,
            OpaqueKind::OpenAiReasoningItemId,
            "rs_1",
        ));
        let kept = reasoning_for_tool_calls(&[
            Block::Reasoning(signed("a")),
            Block::Reasoning(unsigned),
            Block::Reasoning(id_only),
            Block::text("Checking."),
            call("c1"),
            Block::Reasoning(signed("b")),
            call("c2"),
            call("c3"),
            Block::Reasoning(signed("after the last call")),
        ]);
        let turn: Vec<String> = ["c1", "c2", "c3"].map(String::from).to_vec();
        assert_eq!(
            kept,
            [
                CarriedReasoning::new("c1", vec![signed("a")]).with_turn_calls(turn.clone()),
                CarriedReasoning::new("c2", vec![signed("b")]).with_turn_calls(turn),
            ]
        );
        let json = serde_json::to_string(&kept).unwrap();
        let back: Vec<CarriedReasoning> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kept);
    }

    #[test]
    fn restored_reasoning_goes_where_it_came_from() {
        let kept = [
            CarriedReasoning::new("c1", vec![signed("a")]),
            CarriedReasoning::new("c2", vec![signed("b")]),
        ];
        let mut req = GenerateRequest::new("m");
        req.messages = vec![
            Message::user("q"),
            Message::new(
                Role::Assistant,
                vec![Block::text("Checking."), call("c1"), call("c2")],
            ),
            Message::new(Role::User, vec![Block::tool_result("c1", vec![], false)]),
            Message::new(
                Role::Assistant,
                vec![Block::Reasoning(signed("own")), call("c2")],
            ),
        ];
        assert_eq!(tool_calls_missing_reasoning(&req), ["c1", "c2"]);
        assert_eq!(restore_reasoning(&mut req, &kept), 2);
        assert_eq!(
            req.messages[1].blocks,
            [
                Block::Reasoning(signed("a")),
                Block::text("Checking."),
                call("c1"),
                Block::Reasoning(signed("b")),
                call("c2"),
            ]
        );
        assert_eq!(req.messages[3].blocks.len(), 2);
        assert!(tool_calls_missing_reasoning(&req).is_empty());
        assert_eq!(restore_reasoning(&mut req, &kept), 0);
    }

    #[test]
    fn restoring_follows_each_call_through_long_turns() {
        let kept = [
            CarriedReasoning::new("c2", vec![signed("b"), signed("b2")]),
            CarriedReasoning::new("c4", vec![signed("d")]),
            CarriedReasoning::new("c2", vec![signed("duplicate")]),
            CarriedReasoning::new("gone", vec![signed("unused")]),
        ];
        let mut req = GenerateRequest::new("m");
        req.messages = vec![
            Message::new(
                Role::Assistant,
                vec![
                    call("c1"),
                    Block::text("Then."),
                    Block::text("And."),
                    call("c2"),
                    call("c3"),
                    Block::text("Last."),
                ],
            ),
            Message::new(Role::User, vec![Block::tool_result("c1", vec![], false)]),
            Message::new(Role::Assistant, vec![Block::text("More."), call("c4")]),
        ];
        assert_eq!(restore_reasoning(&mut req, &kept), 3);
        assert_eq!(
            req.messages[0].blocks,
            [
                call("c1"),
                Block::Reasoning(signed("b")),
                Block::Reasoning(signed("b2")),
                Block::text("Then."),
                Block::text("And."),
                call("c2"),
                call("c3"),
                Block::text("Last."),
            ]
        );
        assert_eq!(
            req.messages[2].blocks,
            [
                Block::Reasoning(signed("d")),
                Block::text("More."),
                call("c4"),
            ]
        );
    }

    #[test]
    fn readable_reasoning_an_app_replays_does_not_stop_a_restore() {
        let mut readable = ReasoningBlock::new(DialectId::OpenAiChat);
        readable.text.push("from a Chat app".into());
        let mut req = GenerateRequest::new("m");
        req.messages = vec![Message::new(
            Role::Assistant,
            vec![Block::Reasoning(readable.clone()), call("c1")],
        )];
        assert_eq!(tool_calls_missing_reasoning(&req), ["c1"]);
        let kept = [CarriedReasoning::new("c1", vec![signed("a")])];
        assert_eq!(restore_reasoning(&mut req, &kept), 1);
        assert_eq!(
            req.messages[0].blocks,
            [
                Block::Reasoning(signed("a")),
                Block::Reasoning(readable),
                call("c1")
            ],
            "the signed block leads the turn, as its provider sent it"
        );
    }

    #[test]
    fn reasoning_is_restored_only_into_the_turn_it_came_from() {
        let turn = |calls: &[&str]| calls.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
        let kept = [
            CarriedReasoning::new("c1", vec![signed("a")]).with_turn_calls(turn(&["c1"])),
            CarriedReasoning::new("c2", vec![signed("b")]).with_turn_calls(turn(&["c2", "c3"])),
        ];
        let mut merged = GenerateRequest::new("m");
        merged.messages = vec![Message::new(
            Role::Assistant,
            vec![call("c1"), call("c2"), call("c3")],
        )];
        assert_eq!(restore_reasoning(&mut merged, &kept), 0);
        assert_eq!(merged.messages[0].blocks.len(), 3);

        let mut split = GenerateRequest::new("m");
        split.messages = vec![
            Message::new(Role::Assistant, vec![call("c1")]),
            Message::new(Role::User, vec![Block::tool_result("c1", vec![], false)]),
            Message::new(Role::Assistant, vec![call("c3"), call("c2")]),
        ];
        assert_eq!(restore_reasoning(&mut split, &kept), 2);
        assert_eq!(
            split.messages[2].blocks,
            [call("c3"), Block::Reasoning(signed("b")), call("c2")]
        );
    }

    #[test]
    fn restoring_many_tool_calls_keeps_every_one_in_place() {
        let n = 2000;
        let kept: Vec<_> = (0..n)
            .map(|i| CarriedReasoning::new(format!("c{i}"), vec![signed(&format!("r{i}"))]))
            .collect();
        let mut req = GenerateRequest::new("m");
        req.messages = (0..n / 4)
            .map(|t| {
                Message::new(
                    Role::Assistant,
                    (0..4).map(|j| call(&format!("c{}", t * 4 + j))).collect(),
                )
            })
            .collect();
        assert_eq!(restore_reasoning(&mut req, &kept), n);
        for (t, m) in req.messages.iter().enumerate() {
            let want: Vec<Block> = (0..4)
                .flat_map(|j| {
                    let i = t * 4 + j;
                    [
                        Block::Reasoning(signed(&format!("r{i}"))),
                        call(&format!("c{i}")),
                    ]
                })
                .collect();
            assert_eq!(m.blocks, want);
        }
    }

    #[test]
    fn the_collector_rebuilds_streamed_reasoning() {
        let r = DialectId::OpenAiResponses;
        let mut header = ReasoningHeader::new(r);
        header
            .opaque
            .push(Opaque::new(r, OpaqueKind::OpenAiReasoningItemId, "rs_1"));
        let mut c = ReasoningCollector::new();
        for e in [
            StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Reasoning(header))),
            StreamEvent::BlockDelta(BlockDelta::new(
                0,
                Delta::ReasoningSummary(SummaryDelta::new(0, "Fir")),
            )),
            StreamEvent::BlockDelta(BlockDelta::new(
                0,
                Delta::ReasoningSummary(SummaryDelta::new(0, "st.")),
            )),
            StreamEvent::BlockDelta(BlockDelta::new(
                0,
                Delta::ReasoningSummary(SummaryDelta::new(1, "Second.")),
            )),
            StreamEvent::BlockDelta(BlockDelta::new(
                0,
                Delta::Opaque(Opaque::new(r, OpaqueKind::OpenAiEncryptedReasoning, "gAAA")),
            )),
            StreamEvent::BlockStop(BlockStop::new(0)),
            StreamEvent::BlockStart(BlockStart::new(
                1,
                BlockHeader::ToolCall(ToolCallHeader::new("call_1", "f")),
            )),
            StreamEvent::BlockStop(BlockStop::new(1)),
        ] {
            c.observe(&e);
        }
        let kept = c.finish();
        let mut want = ReasoningBlock::new(r);
        want.summary = vec!["First.".into(), "Second.".into()];
        want.opaque = vec![
            Opaque::new(r, OpaqueKind::OpenAiReasoningItemId, "rs_1"),
            Opaque::new(r, OpaqueKind::OpenAiEncryptedReasoning, "gAAA"),
        ];
        assert_eq!(
            kept,
            [CarriedReasoning::new("call_1", vec![want]).with_turn_calls(vec!["call_1".into()])]
        );
        assert!(c.finish().is_empty());
    }

    fn reference(id: &str) -> Block {
        let mut r = ReasoningBlock::new(DialectId::OpenAiResponses);
        r.extras = Extras::new(Origin::new(DialectId::OpenAiResponses))
            .with_form(crate::openai_responses::stored::REFERENCE, id.into());
        Block::Reasoning(r)
    }

    #[test]
    fn a_responses_reply_keeps_every_reasoning_block_before_a_tool_call() {
        let mut plain = ReasoningBlock::new(DialectId::OpenAiChat);
        plain.text.push("DeepSeek thinks in plain text.".into());
        let blocks = [Block::Reasoning(plain.clone()), call("c1")];
        assert!(reasoning_for_tool_calls(&blocks).is_empty());
        assert_eq!(
            every_reasoning_for_tool_calls(&blocks),
            [CarriedReasoning::new("c1", vec![plain]).with_turn_calls(vec!["c1".into()])]
        );
    }

    #[test]
    fn references_take_the_kept_reasoning_of_their_turn_or_name_what_is_missing() {
        let kept = [CarriedReasoning::new("c1", vec![signed("a")])];
        let mut req = GenerateRequest::new("m");
        req.messages = vec![
            Message::user("q"),
            Message::new(Role::Assistant, vec![reference("rs_x"), Block::text("Hi.")]),
            Message::user("again"),
            Message::new(Role::Assistant, vec![reference("rs_y"), call("c1")]),
            Message::new(Role::User, vec![Block::tool_result("c1", vec![], false)]),
        ];
        assert_eq!(referenced_tool_calls(&req), ["c1"]);
        let resolved = resolve_reasoning_references(&mut req, &kept).unwrap();
        assert_eq!((resolved.restored, resolved.left_out), (1, 1));
        assert_eq!(req.messages[1].blocks, [Block::text("Hi.")]);
        assert_eq!(
            req.messages[3].blocks,
            [Block::Reasoning(signed("a")), call("c1")]
        );

        let mut only = GenerateRequest::new("m");
        only.messages = vec![
            Message::user("q"),
            Message::new(Role::Assistant, vec![reference("rs_x")]),
            Message::user("again"),
        ];
        let resolved = resolve_reasoning_references(&mut only, &[]).unwrap();
        assert_eq!(resolved.left_out, 1);
        assert_eq!(
            only.messages.len(),
            2,
            "a turn of nothing but a reference goes"
        );

        let mut unkept = GenerateRequest::new("m");
        unkept.messages = vec![
            Message::user("q"),
            Message::new(Role::Assistant, vec![reference("rs_z"), call("c9")]),
        ];
        assert_eq!(
            resolve_reasoning_references(&mut unkept, &kept),
            Err("rs_z".to_owned())
        );
    }

    #[test]
    fn reasoning_sent_inline_beside_references_is_not_put_back_twice() {
        let kept = [CarriedReasoning::new("c1", vec![signed("a"), signed("b")])];
        let mut req = GenerateRequest::new("m");
        req.messages = vec![
            Message::user("q"),
            Message::new(
                Role::Assistant,
                vec![Block::Reasoning(signed("a")), reference("rs_b"), call("c1")],
            ),
        ];
        let resolved = resolve_reasoning_references(&mut req, &kept).unwrap();
        assert_eq!(resolved.restored, 1);
        assert_eq!(
            resolved.restored_bytes,
            size(&signed("b")),
            "the restored reasoning counts toward the request's size"
        );
        assert_eq!(
            req.messages[1].blocks,
            [
                Block::Reasoning(signed("a")),
                Block::Reasoning(signed("b")),
                call("c1")
            ]
        );

        // Two references and one kept block: one of them points to reasoning Kaeso doesn't keep.
        let mut more = GenerateRequest::new("m");
        more.messages = vec![Message::new(
            Role::Assistant,
            vec![reference("rs_1"), reference("rs_2"), call("c1")],
        )];
        let one = [CarriedReasoning::new("c1", vec![signed("a")])];
        assert_eq!(
            resolve_reasoning_references(&mut more, &one),
            Err("rs_1".to_owned())
        );
    }

    #[test]
    fn only_turns_emptied_by_a_reference_go() {
        let mut req = GenerateRequest::new("m");
        let mut empty = Message::new(Role::Assistant, vec![]);
        empty.extras = Extras::new(Origin::new(DialectId::OpenAiResponses))
            .with_form("content_empty", true.into());
        req.messages = vec![
            Message::user("q"),
            empty.clone(),
            Message::new(Role::Assistant, vec![reference("rs_x")]),
            Message::user("again"),
        ];
        resolve_reasoning_references(&mut req, &[]).unwrap();
        assert_eq!(
            req.messages,
            [Message::user("q"), empty, Message::user("again")],
            "the app's own empty message stays"
        );
    }
}
