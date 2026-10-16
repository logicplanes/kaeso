use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_kpp::{
    Block, BlockHeader, Delta, DialectId, GenerateRequest, OpaqueKind, ReasoningBlock, Role,
    StreamEvent,
};
use serde::{Deserialize, Serialize};

pub const REASONING_BLOB_PREFIX: &str = "kpp:reasoning:v1:";

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
}

impl CarriedReasoning {
    pub fn new(tool_call_id: impl Into<String>, blocks: Vec<ReasoningBlock>) -> Self {
        CarriedReasoning {
            tool_call_id: tool_call_id.into(),
            blocks,
        }
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
            )
    })
}

pub fn reasoning_for_tool_calls(blocks: &[Block]) -> Vec<CarriedReasoning> {
    let mut kept = Vec::new();
    let mut run: Vec<ReasoningBlock> = Vec::new();
    for block in blocks {
        match block {
            Block::Reasoning(r) if replayable(r) => run.push(r.clone()),
            Block::ToolCall(call) if !run.is_empty() => {
                kept.push(CarriedReasoning::new(
                    call.id.clone(),
                    std::mem::take(&mut run),
                ));
            }
            _ => {}
        }
    }
    kept
}

#[derive(Clone, Debug, Default)]
pub struct ReasoningCollector {
    order: Vec<(u32, Started)>,
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
                match &d.delta {
                    Delta::ReasoningText(t) => {
                        let new_part = d.extras.form_flag(
                            DialectId::OpenAiResponses,
                            crate::openai_responses::stream::NEW_PART,
                        );
                        match block.text.last_mut() {
                            Some(last) if !new_part => last.push_str(t),
                            _ => block.text.push(t.clone()),
                        }
                    }
                    Delta::ReasoningSummary(s) => {
                        let slot = usize::try_from(s.summary_index).unwrap_or(usize::MAX);
                        if block.summary.len() <= slot && slot < 4096 {
                            block.summary.resize(slot + 1, String::new());
                        }
                        if let Some(part) = block.summary.get_mut(slot) {
                            part.push_str(&s.text);
                        }
                    }
                    Delta::Opaque(o) => block.opaque.push(o.clone()),
                    _ => {}
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
        reasoning_for_tool_calls(&blocks)
    }
}

pub fn tool_calls_missing_reasoning(req: &GenerateRequest) -> Vec<String> {
    req.messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter(|m| !m.blocks.iter().any(|b| matches!(b, Block::Reasoning(_))))
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
    let mut inserted = 0;
    for message in &mut req.messages {
        if message.role != Role::Assistant
            || message
                .blocks
                .iter()
                .any(|b| matches!(b, Block::Reasoning(_)))
        {
            continue;
        }
        let mut blocks = Vec::with_capacity(message.blocks.len());
        let mut insert_at = 0;
        for block in std::mem::take(&mut message.blocks) {
            if let Block::ToolCall(call) = &block {
                if let Some(entry) = kept.iter().find(|k| k.tool_call_id == call.id) {
                    let restored = entry.blocks.iter().cloned().map(Block::Reasoning);
                    let at = insert_at.min(blocks.len());
                    let before = blocks.len();
                    blocks.splice(at..at, restored);
                    inserted += blocks.len() - before;
                }
                blocks.push(block);
                insert_at = blocks.len();
            } else {
                blocks.push(block);
            }
        }
        message.blocks = blocks;
    }
    inserted
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
        assert_eq!(
            kept,
            [
                CarriedReasoning::new("c1", vec![signed("a")]),
                CarriedReasoning::new("c2", vec![signed("b")]),
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
        assert_eq!(kept, [CarriedReasoning::new("call_1", vec![want])]);
        assert!(c.finish().is_empty());
    }
}
