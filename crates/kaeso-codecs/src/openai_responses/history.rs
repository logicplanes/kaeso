use kaeso_kpp::{Block, KppError, Message, RawItem, RawJson, Role, ToolCallBlock, ToolResultBlock};
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

pub(crate) const AGENT_MESSAGE: &str = "agent_message";

// Codex's subagents talk through `agent_message` input items: a task from the root agent to a
// subagent, and each agent's messages to the others. Only OpenAI reads them.
#[derive(Deserialize)]
struct AgentMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    recipient: Option<String>,
    #[serde(default)]
    content: Vec<AgentPart>,
}

#[derive(Deserialize)]
struct AgentPart {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    encrypted_content: Option<String>,
}

fn item_type(item: &RawItem) -> Option<String> {
    if !item.origin.is(DIALECT) {
        return None;
    }
    type_tag(item.raw.as_raw()).ok().flatten()
}

fn agent_message(m: &Message) -> Option<&RawItem> {
    match m.blocks.as_slice() {
        [Block::Unknown(item)] if item_type(item).as_deref() == Some(AGENT_MESSAGE) => Some(item),
        _ => None,
    }
}

// Codex writes the task's own text into `encrypted_content`. Real ciphertext is one unbroken
// run of base64, which no model could read anywhere but at OpenAI.
fn looks_encrypted(s: &str) -> bool {
    s.len() >= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_'))
}

fn agent_text(m: &AgentMessage, provider: &str) -> Result<String, KppError> {
    let mut text = String::new();
    for part in &m.content {
        let piece = match part.ty.as_str() {
            "input_text" | "output_text" | "text" => part.text.as_deref().unwrap_or_default(),
            "encrypted_content" => {
                let s = part.encrypted_content.as_deref().unwrap_or_default();
                if looks_encrypted(s) {
                    return Err(KppError::unsupported(
                        "encrypted agent message",
                        format!(
                            "This Codex agent message is encrypted, and only OpenAI can read it; {provider} can't. Run this session's subagents on an OpenAI model."
                        ),
                    )
                    .with_param("input"));
                }
                s
            }
            other => {
                return Err(KppError::unsupported(
                    format!("agent message {other} content"),
                    format!("{provider} can't take a Codex agent message with `{other}` content."),
                )
                .with_param("input"));
            }
        };
        text.push_str(piece);
    }
    // Codex's own header names the sender and the task; without one, say who wrote to whom.
    if !text.trim_start().starts_with("Message Type:") {
        let author = m.author.as_deref().unwrap_or("another agent");
        let recipient = m.recipient.as_deref().unwrap_or("this agent");
        text = format!("Message from {author} to {recipient}:\n\n{text}");
    }
    Ok(text)
}

/// Turns Codex's `agent_message` input items into messages for a provider that can't read
/// them. A message this agent wrote becomes an assistant message, and one it received a user
/// message. This agent is the recipient of the first agent message when nothing in the
/// conversation answered before it (a subagent's task), and its author otherwise (the root
/// agent writing to a subagent it started).
pub(crate) fn agent_messages(d: &mut Draft<'_>, provider: &str) -> Result<(), KppError> {
    let messages = &d.get().messages;
    let Some(first) = messages.iter().position(|m| agent_message(m).is_some()) else {
        return Ok(());
    };
    let parse = |item: &RawItem| -> Result<AgentMessage, KppError> {
        item.raw.parse::<AgentMessage>().map_err(|e| {
            KppError::invalid_request(format!(
                "A Codex `agent_message` input item can't be read: {e}."
            ))
            .with_param("input")
        })
    };
    let Some(opening) = messages.iter().find_map(agent_message) else {
        return Ok(());
    };
    let opening = parse(opening)?;
    let answered_before = messages
        .iter()
        .take(first)
        .any(|m| m.role == Role::Assistant);
    let me = if answered_before {
        opening.author
    } else {
        opening.recipient
    };
    let mut converted = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        let Some(item) = agent_message(m) else {
            continue;
        };
        let message = parse(item)?;
        let role = if message.author.is_some() && message.author == me {
            Role::Assistant
        } else {
            Role::User
        };
        let id = message.id.clone().unwrap_or_else(|| format!("at {i}"));
        converted.push((
            i,
            Message::new(role, vec![Block::text(agent_text(&message, provider)?)]),
            id,
        ));
    }
    for (i, message, id) in converted {
        if let Some(slot) = d.edit().messages.get_mut(i) {
            *slot = message;
        }
        d.adapt(format!("agent message {id}"));
    }
    Ok(())
}

/// Keeps a request to a provider that takes only `allowed` input item types within them:
/// Codex's local shell items become function calls and their outputs, and any other type
/// the provider doesn't take is refused, naming the provider, before the request is sent.
pub(crate) fn only_items(
    d: &mut Draft<'_>,
    allowed: &[&str],
    provider: &str,
) -> Result<(), KppError> {
    let mut refused = None;
    let mut convert = false;
    for item in d
        .get()
        .messages
        .iter()
        .flat_map(|m| m.blocks.iter())
        .filter_map(|b| match b {
            Block::Unknown(item) => Some(item),
            _ => None,
        })
    {
        let Some(ty) = item_type(item) else {
            continue;
        };
        if allowed.contains(&ty.as_str()) {
            continue;
        }
        if matches!(ty.as_str(), "local_shell_call" | "local_shell_call_output") {
            convert = true;
        } else if refused.is_none() {
            refused = Some(ty);
        }
    }
    if let Some(ty) = refused {
        return Err(KppError::unsupported(
            format!("{ty} input items"),
            format!(
                "{provider} doesn't take `{ty}` input items; Kaeso can only send them to OpenAI."
            ),
        )
        .with_param("input"));
    }
    if convert {
        for block in d
            .edit()
            .messages
            .iter_mut()
            .flat_map(|m| m.blocks.iter_mut())
        {
            if let Block::Unknown(item) = block
                && matches!(
                    item_type(item).as_deref(),
                    Some("local_shell_call" | "local_shell_call_output")
                )
                && let Some(b) = converted(item)?
            {
                *block = b;
            }
        }
    }
    Ok(())
}
