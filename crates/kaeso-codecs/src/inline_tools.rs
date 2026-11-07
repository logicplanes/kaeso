use kaeso_kpp::{Block, DialectId, GenerateRequest, KppError, Origin, Role, ToolDef};
use serde::Deserialize;

use crate::adapt::Draft;
use crate::anthropic_messages::{ToolChange, ToolHandle, ToolReferent, decode_tool, tool_change};
use crate::cross::foreign;
use crate::tool_search::deferral;

const ANTHROPIC: DialectId = DialectId::AnthropicMessages;
const DEFER: &str = "defer_loading";
const HINT: &str = "Remove it, or send the request to a model served through its original format.";

// `None`: as declared in `tools`; `Some(true)`: offered from a `tool_addition` on, which
// also surfaces a deferred tool; `Some(false)`: withdrawn by a `tool_removal`.
struct Slot {
    tool: ToolDef,
    offered: Option<bool>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

fn name_of(tool: &ToolDef) -> Option<String> {
    match tool {
        ToolDef::Function(f) => Some(f.name.clone()),
        ToolDef::Unknown(item) => serde_json::from_str::<Named>(item.raw.get())
            .ok()
            .map(|n| n.name),
        _ => None,
    }
}

fn holds_change(b: &Block) -> bool {
    matches!(b, Block::Unknown(item) if tool_change(item).is_some())
}

fn has_changes(r: &GenerateRequest) -> bool {
    r.messages
        .iter()
        .any(|m| m.role == Role::System && m.blocks.iter().any(holds_change))
}

fn unreadable(path: &str, origin: &Origin, target: DialectId, reason: &str) -> KppError {
    foreign(
        format!("inline tool change with {reason}"),
        origin,
        target,
        path,
        HINT,
    )
}

fn unresolved(name: &str, path: &str) -> KppError {
    KppError::invalid_request(format!(
        "Tool reference '{name}' not found in available tools"
    ))
    .with_param(path)
}

fn apply(
    slots: &mut Vec<Slot>,
    change: ToolChange,
    origin: &Origin,
    target: DialectId,
    path: &str,
) -> Result<(), KppError> {
    let (handle, offered, block) = match change {
        ToolChange::Added(h) => (h, true, "tool_addition"),
        ToolChange::Removed(h) => (h, false, "tool_removal"),
    };
    let ToolHandle { tool, cache } = handle;
    match tool {
        ToolReferent::Reference(name) => {
            let Some(slot) = slots
                .iter_mut()
                .find(|s| name_of(&s.tool).as_deref() == Some(&name))
            else {
                return Err(unresolved(&name, path));
            };
            slot.offered = Some(offered);
        }
        ToolReferent::Definition(raw) if offered => {
            let mut tool = decode_tool(&raw, origin, path)?;
            if let ToolDef::Function(f) = &mut tool
                && f.cache.is_none()
            {
                f.cache = cache;
            }
            let offered = (deferral(&tool) != Some(true)).then_some(true);
            let name = name_of(&tool);
            let slot = Slot { tool, offered };
            match slots
                .iter_mut()
                .find(|s| name.is_some() && name_of(&s.tool) == name)
            {
                Some(existing) => *existing = slot,
                None => slots.push(slot),
            }
        }
        ToolReferent::Definition(_) => {
            return Err(KppError::invalid_request(
                "A `tool_removal` block names the tool to withdraw with a `tool_reference`; it takes no `tool_definition`.",
            )
            .with_param(path));
        }
        ToolReferent::Other(ty) => {
            return Err(foreign(
                format!("`{ty}` in a `{block}` block"),
                origin,
                target,
                path,
                HINT,
            ));
        }
    }
    Ok(())
}

// Anthropic offers or withdraws a tool from the position of its block onward; other formats
// take one tool list per request, so they get the tools offered at the end of the
// conversation, and the blocks leave the system messages.
pub(crate) fn for_foreign(d: &mut Draft<'_>, target: DialectId) -> Result<(), KppError> {
    if target == ANTHROPIC || !has_changes(d.get()) {
        return Ok(());
    }
    let r = d.edit();
    let mut slots: Vec<Slot> = std::mem::take(&mut r.tools)
        .into_iter()
        .map(|tool| Slot {
            tool,
            offered: None,
        })
        .collect();
    let mut changed = Vec::new();
    let mut messages = Vec::with_capacity(r.messages.len());
    for (mi, mut m) in std::mem::take(&mut r.messages).into_iter().enumerate() {
        if m.role != Role::System || !m.blocks.iter().any(holds_change) {
            messages.push(m);
            continue;
        }
        let mut kept = Vec::with_capacity(m.blocks.len());
        for (bi, b) in std::mem::take(&mut m.blocks).into_iter().enumerate() {
            let Block::Unknown(item) = &b else {
                kept.push(b);
                continue;
            };
            let Some(change) = tool_change(item) else {
                kept.push(b);
                continue;
            };
            let path = format!("messages[{mi}].blocks[{bi}]");
            let change =
                change.map_err(|reason| unreadable(&path, &item.origin, target, &reason))?;
            apply(&mut slots, change, &item.origin, target, &path)?;
            changed.push(path);
        }
        m.blocks = kept;
        if !m.blocks.is_empty() || !m.extras.fields.is_empty() || !m.extras.form.is_empty() {
            messages.push(m);
        }
    }
    r.messages = messages;
    r.tools = slots
        .into_iter()
        .filter_map(|slot| match slot.offered {
            Some(false) => None,
            Some(true) => {
                let mut tool = slot.tool;
                if let ToolDef::Function(f) = &mut tool {
                    f.extras.fields.shift_remove(DEFER);
                }
                Some(tool)
            }
            None => Some(slot.tool),
        })
        .collect();
    for path in changed {
        d.adapt(path);
    }
    Ok(())
}
