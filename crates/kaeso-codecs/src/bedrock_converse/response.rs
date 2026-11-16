use kaeso_kpp::{
    Block, KppError, Opaque, OpaqueKind, Origin, RawItem, RawJson, RawUsage, ReasoningBlock,
    Response, StopReason, TextBlock, ToolCallBlock, Usage,
};
use serde_json::Value;

use super::D;
use crate::codec::{EgressCtx, HeaderList};

pub(super) fn bad(what: &str) -> KppError {
    KppError::provider(format!(
        "Amazon Bedrock answered Converse in a form Kaeso can't read: {what}."
    ))
    .with_origin(D)
}

// Converse's `stopReason` values
// (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html#API_runtime_Converse_ResponseSyntax).
pub(super) fn stop_reason(reason: &str) -> StopReason {
    match reason {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" | "model_context_window_exceeded" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence(String::new()),
        "guardrail_intervened" | "content_filtered" => StopReason::ContentFilter,
        other => StopReason::Other(other.to_owned()),
    }
}

/// `usage`, with the cached tokens apart from `inputTokens`, which counts only the uncached
/// ones (<https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_TokenUsage.html>).
pub(super) fn usage(v: &Value, origin: &Origin) -> Usage {
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let mut u = Usage::new(n("inputTokens"), n("outputTokens"));
    u.cache_read_tokens = n("cacheReadInputTokens");
    u.cache_write_tokens = n("cacheWriteInputTokens");
    u.raw = Some(RawUsage::new(origin.clone(), v.clone()));
    u
}

pub(super) fn reasoning(v: &Value, origin: &Origin) -> ReasoningBlock {
    let mut r = ReasoningBlock::new(origin.clone());
    if let Some(t) = v.get("reasoningText") {
        if let Some(text) = t.get("text").and_then(Value::as_str) {
            r.text.push(text.to_owned());
        }
        if let Some(sig) = t.get("signature").and_then(Value::as_str) {
            r.opaque.push(Opaque::new(
                origin.clone(),
                OpaqueKind::BedrockReasoningSignature,
                sig,
            ));
        }
    }
    if let Some(data) = v.get("redactedContent").and_then(Value::as_str) {
        r.opaque.push(Opaque::new(
            origin.clone(),
            OpaqueKind::BedrockRedactedReasoning,
            data,
        ));
    }
    r
}

fn raw(v: &Value) -> Result<RawJson, KppError> {
    RawJson::from_string(v.to_string()).map_err(|_| bad("a block isn't JSON"))
}

pub(super) fn decode(
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Response, KppError> {
    let origin = ctx.origin(D);
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("the body isn't JSON"))?;
    let content = v
        .pointer("/output/message/content")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("no `output.message.content`"))?;
    let mut blocks = Vec::with_capacity(content.len());
    for c in content {
        if let Some(text) = c.get("text").and_then(Value::as_str) {
            blocks.push(Block::Text(TextBlock::new(text)));
        } else if let Some(t) = c.get("toolUse") {
            let id = t
                .get("toolUseId")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("a `toolUse` without `toolUseId`"))?;
            let name = t
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("a `toolUse` without `name`"))?;
            let input = t
                .get("input")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default()));
            blocks.push(Block::ToolCall(ToolCallBlock::new(
                ctx.original_tool_id(id),
                ctx.original_tool_name(name),
                raw(&input)?,
            )));
        } else if let Some(r) = c.get("reasoningContent") {
            blocks.push(Block::Reasoning(reasoning(r, &origin)));
        } else {
            blocks.push(Block::Unknown(RawItem::new(origin.clone(), raw(c)?)));
        }
    }
    let id = headers.get("x-amzn-requestid").unwrap_or_default();
    let mut r = Response::new(id, "", blocks);
    r.stop = stop_reason(
        v.get("stopReason")
            .and_then(Value::as_str)
            .unwrap_or("end_turn"),
    );
    if let Some(u) = v.get("usage") {
        r.usage = usage(u, &origin);
    }
    Ok(r)
}
