use std::collections::HashSet;

use kaeso_kpp::{Block, DialectId, GenerateRequest, KppError, ToolChoice, ToolDef};

use crate::anthropic_messages::tool_reference;
use crate::cross::foreign;

const ANTHROPIC: DialectId = DialectId::AnthropicMessages;
const DEFER: &str = "defer_loading";

fn deferral(t: &ToolDef) -> Option<bool> {
    let ToolDef::Function(f) = t else {
        return None;
    };
    if !f.extras.is_from(ANTHROPIC) {
        return None;
    }
    f.extras.fields.get(DEFER)?.parse::<bool>().ok()
}

fn holds_reference(b: &Block) -> bool {
    let Block::ToolResult(res) = b else {
        return false;
    };
    res.content
        .iter()
        .any(|c| matches!(c, Block::Unknown(item) if tool_reference(item).is_some()))
}

fn has_tool_search(r: &GenerateRequest) -> bool {
    r.tools.iter().any(|t| deferral(t).is_some())
        || r.messages
            .iter()
            .any(|m| m.blocks.iter().any(holds_reference))
}

fn loaded_by_references(r: &mut GenerateRequest) -> Vec<(String, String)> {
    let mut loaded = Vec::new();
    for (mi, m) in r.messages.iter_mut().enumerate() {
        for (bi, b) in m.blocks.iter_mut().enumerate() {
            let Block::ToolResult(res) = b else {
                continue;
            };
            for (ci, c) in res.content.iter_mut().enumerate() {
                if let Block::Unknown(item) = c
                    && let Some((name, text)) = tool_reference(item)
                {
                    loaded.push((name, format!("messages[{mi}].blocks[{bi}].content[{ci}]")));
                    *c = text;
                }
            }
        }
    }
    loaded
}

fn not_loaded(name: &str, target: DialectId) -> KppError {
    foreign(
        format!("tool_choice `{name}`"),
        &kaeso_kpp::Origin::new(ANTHROPIC),
        target,
        "tool_choice",
        "The tool has `defer_loading: true` and no `tool_reference` has loaded it yet. Load it through tool search first, or remove `defer_loading`.",
    )
}

pub(crate) fn for_foreign(
    r: &GenerateRequest,
    target: DialectId,
) -> Result<Option<GenerateRequest>, KppError> {
    if target == ANTHROPIC || !has_tool_search(r) {
        return Ok(None);
    }
    let mut out = r.clone();
    let references = loaded_by_references(&mut out);
    let names: HashSet<&str> = out
        .tools
        .iter()
        .filter_map(|t| match t {
            ToolDef::Function(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();
    if let Some((name, path)) = references.iter().find(|(n, _)| !names.contains(n.as_str())) {
        return Err(KppError::invalid_request(format!(
            "Tool reference '{name}' not found in available tools."
        ))
        .with_param(path.clone()));
    }
    if !r.tools.is_empty() && r.tools.iter().all(|t| deferral(t) == Some(true)) {
        return Err(KppError::invalid_request(
            "At least one tool must have defer_loading=false. All tools cannot be deferred.",
        )
        .with_param("tools"));
    }
    let loaded: HashSet<&str> = references.iter().map(|(n, _)| n.as_str()).collect();
    let mut tools = Vec::with_capacity(out.tools.len());
    for mut t in std::mem::take(&mut out.tools) {
        let deferred = deferral(&t);
        if let ToolDef::Function(f) = &mut t {
            if deferred == Some(true) && !loaded.contains(f.name.as_str()) {
                if matches!(&out.tool_choice, Some(ToolChoice::Tool(chosen)) if *chosen == f.name) {
                    return Err(not_loaded(&f.name, target));
                }
                continue;
            }
            if deferred.is_some() {
                f.extras.fields.shift_remove(DEFER);
            }
        }
        tools.push(t);
    }
    out.tools = tools;
    Ok(Some(out))
}
