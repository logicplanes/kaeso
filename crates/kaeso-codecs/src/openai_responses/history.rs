use kaeso_kpp::{Block, KppError, RawItem, RawJson, ToolCallBlock, ToolResultBlock};
use serde::Deserialize;
use serde_json::json;
use serde_json::value::RawValue;

use super::DIALECT;
use super::namespaces::flat_name;
use super::parts::decode_function_call_output;
use crate::adapt::Draft;
use crate::wire::type_tag;

pub(crate) const LOCAL_SHELL: &str = "local_shell";

#[derive(Deserialize)]
struct CustomCall {
    call_id: String,
    name: String,
    input: String,
    #[serde(default)]
    namespace: Option<String>,
}

#[derive(Deserialize)]
struct ShellCall {
    call_id: String,
    action: Box<RawValue>,
}

#[derive(Deserialize)]
struct ShellOutput {
    id: String,
    output: String,
}

fn is_convertible(item: &RawItem) -> bool {
    item.origin.is(DIALECT)
        && type_tag(item.raw.as_raw()).ok().flatten().is_some_and(|t| {
            matches!(
                t.as_str(),
                "custom_tool_call"
                    | "custom_tool_call_output"
                    | "local_shell_call"
                    | "local_shell_call_output"
            )
        })
}

fn converted(item: &RawItem) -> Result<Option<Block>, KppError> {
    let raw = item.raw.as_raw();
    let ty = type_tag(raw).ok().flatten();
    Ok(match ty.as_deref() {
        Some("custom_tool_call") => match item.raw.parse::<CustomCall>() {
            Ok(c) => {
                let name = match &c.namespace {
                    Some(ns) => flat_name(ns, &c.name),
                    None => c.name,
                };
                let args = RawJson::from_serialize(&json!({ "input": c.input })).map_err(|e| {
                    KppError::invalid_request(format!("`custom_tool_call` cannot be sent: {e}."))
                })?;
                Some(Block::ToolCall(ToolCallBlock::new(c.call_id, name, args)))
            }
            Err(_) => None,
        },
        Some("custom_tool_call_output") => {
            decode_function_call_output(raw, &item.origin).map(Block::ToolResult)
        }
        Some("local_shell_call") => item.raw.parse::<ShellCall>().ok().map(|c| {
            Block::ToolCall(ToolCallBlock::new(
                c.call_id,
                LOCAL_SHELL,
                RawJson::from(c.action.as_ref()),
            ))
        }),
        Some("local_shell_call_output") => item
            .raw
            .parse::<ShellOutput>()
            .ok()
            .map(|o| Block::ToolResult(ToolResultBlock::new(o.id, vec![Block::text(o.output)]))),
        _ => None,
    })
}

pub(crate) fn as_function_calls(d: &mut Draft<'_>) -> Result<(), KppError> {
    let any = d.get().messages.iter().any(|m| {
        m.blocks
            .iter()
            .any(|b| matches!(b, Block::Unknown(item) if is_convertible(item)))
    });
    if !any {
        return Ok(());
    }
    for block in d
        .edit()
        .messages
        .iter_mut()
        .flat_map(|m| m.blocks.iter_mut())
    {
        if let Block::Unknown(item) = block
            && is_convertible(item)
            && let Some(b) = converted(item)?
        {
            *block = b;
        }
    }
    Ok(())
}
