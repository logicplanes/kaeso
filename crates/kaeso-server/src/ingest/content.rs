//! The exchanges sources report, as KPP messages for the request log: every message before the
//! reply with its tool calls, tool results and reasoning, and the reply's parts. Content a source
//! reports is shown for people to read, never sent anywhere, so it is read leniently: a part
//! Kaeso doesn't know is left out of the view, never refused.

use kaeso_kpp::{Block, Message, RawJson, ReasoningBlock, Role};
use serde_json::Value as Json;

/// A conversation as a source reported it: the system prompt and the messages before the reply.
#[derive(Debug, Default, PartialEq)]
pub struct Exchange {
    pub system: Option<String>,
    pub messages: Vec<Message>,
}

impl Exchange {
    /// The text of the last message a person wrote, for the log's one-line view of the request.
    pub fn last_user_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .filter(|m| m.role == Role::User)
            .find_map(|m| texts(&m.blocks))
    }
}

fn texts(blocks: &[Block]) -> Option<String> {
    let t: Vec<&str> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .filter(|t| !t.is_empty())
        .collect();
    (!t.is_empty()).then(|| t.join("\n"))
}

/// Arguments as JSON: a JSON string is parsed, anything else kept as the value it is.
fn arguments(v: Option<&Json>) -> Option<RawJson> {
    match v {
        None | Some(Json::Null) => RawJson::from_string("{}".to_owned()).ok(),
        Some(Json::String(s)) => RawJson::from_string(s.clone())
            .or_else(|_| RawJson::try_from(&Json::String(s.clone())))
            .ok(),
        Some(other) => RawJson::try_from(other).ok(),
    }
}

fn reasoning(text: &str) -> Option<Block> {
    (!text.is_empty()).then(|| {
        let mut b = ReasoningBlock::default();
        b.text = vec![text.to_owned()];
        Block::Reasoning(b)
    })
}

fn tool_result(id: &str, content: &Json, is_error: bool) -> Block {
    let text = match content {
        Json::String(s) => s.clone(),
        Json::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Json::as_str).or(p.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        Json::Null => String::new(),
        other => other.to_string(),
    };
    Block::tool_result(id, vec![Block::text(text)], is_error)
}

fn push(messages: &mut Vec<Message>, role: Role, blocks: Vec<Block>) {
    if !blocks.is_empty() {
        messages.push(Message::new(role, blocks));
    }
}

fn str_of<'a>(v: &'a Json, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Json::as_str)
}

/// The parts of an OpenAI message's `content`: a string, or a list of `text` (and other) parts.
fn openai_content(content: Option<&Json>) -> Vec<Block> {
    match content {
        Some(Json::String(s)) if !s.is_empty() => vec![Block::text(s.clone())],
        Some(Json::Array(parts)) => parts
            .iter()
            .filter_map(|p| match str_of(p, "type") {
                Some("text" | "input_text" | "output_text") | None => {
                    str_of(p, "text").filter(|t| !t.is_empty()).map(Block::text)
                }
                Some("refusal") => str_of(p, "refusal").map(Block::text),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn openai_tool_calls(m: &Json) -> Vec<Block> {
    m.get("tool_calls")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let f = c.get("function")?;
            let name = str_of(f, "name")?;
            Some(Block::tool_call(
                str_of(c, "id").unwrap_or_default(),
                name,
                arguments(f.get("arguments"))?,
            ))
        })
        .collect()
}

/// An assistant message's parts: its reasoning, its text and its tool calls.
pub fn openai_reply(m: &Json) -> Vec<Block> {
    let mut blocks: Vec<Block> = str_of(m, "reasoning_content")
        .or(str_of(m, "reasoning"))
        .and_then(reasoning)
        .into_iter()
        .collect();
    blocks.extend(openai_content(m.get("content")));
    blocks.extend(openai_tool_calls(m));
    blocks
}

/// An OpenAI-style message list, as LiteLLM logs the messages it sent.
pub fn openai_messages(list: Option<&Json>) -> Exchange {
    let mut out = Exchange::default();
    let mut system = Vec::new();
    for m in list.and_then(Json::as_array).into_iter().flatten() {
        match str_of(m, "role") {
            Some("system" | "developer") => {
                if let Some(t) = texts(&openai_content(m.get("content"))) {
                    system.push(t);
                }
            }
            Some("assistant") => push(&mut out.messages, Role::Assistant, openai_reply(m)),
            Some("tool" | "function") => push(
                &mut out.messages,
                Role::User,
                vec![tool_result(
                    str_of(m, "tool_call_id")
                        .or(str_of(m, "name"))
                        .unwrap_or_default(),
                    m.get("content").unwrap_or(&Json::Null),
                    false,
                )],
            ),
            _ => push(
                &mut out.messages,
                Role::User,
                openai_content(m.get("content")),
            ),
        }
    }
    out.system = (!system.is_empty()).then(|| system.join("\n\n"));
    out
}

/// The parts of a message under the GenAI conventions (`gen_ai.input.messages`,
/// `gen_ai.output.messages`): `text`, `reasoning`, `tool_call` and `tool_call_response`.
fn semconv_parts(m: &Json) -> Vec<Block> {
    let parts = match m.get("parts").or(m.get("content")) {
        Some(Json::String(s)) => return vec![Block::text(s.clone())],
        Some(Json::Array(parts)) => parts,
        _ => return Vec::new(),
    };
    parts
        .iter()
        .filter_map(|p| match str_of(p, "type") {
            Some("text") | None => str_of(p, "content")
                .or(str_of(p, "text"))
                .filter(|t| !t.is_empty())
                .map(Block::text),
            Some("reasoning" | "thinking") => str_of(p, "content")
                .or(str_of(p, "text"))
                .and_then(reasoning),
            Some("tool_call") => Some(Block::tool_call(
                str_of(p, "id").unwrap_or_default(),
                str_of(p, "name")?,
                arguments(p.get("arguments"))?,
            )),
            Some("tool_call_response") => Some(tool_result(
                str_of(p, "id").unwrap_or_default(),
                p.get("response").or(p.get("result")).unwrap_or(&Json::Null),
                false,
            )),
            _ => None,
        })
        .collect()
}

/// `gen_ai.input.messages` and `gen_ai.system_instructions` as JSON, or None when the value isn't
/// such a list, as the plain-text `gen_ai.prompt` some instrumentations write.
pub fn semconv_messages(input: &str, system: Option<&str>) -> Option<Exchange> {
    let Ok(Json::Array(messages)) = serde_json::from_str::<Json>(input) else {
        return None;
    };
    let mut out = Exchange::default();
    let mut instructions: Vec<String> = system
        .and_then(|s| serde_json::from_str::<Json>(s).ok())
        .map(|s| texts(&semconv_parts(&serde_json::json!({ "parts": s }))))
        .into_iter()
        .flatten()
        .collect();
    for m in &messages {
        match str_of(m, "role") {
            Some("system" | "developer") => {
                if let Some(t) = texts(&semconv_parts(m)) {
                    instructions.push(t);
                }
            }
            Some("assistant" | "model") => {
                push(&mut out.messages, Role::Assistant, semconv_parts(m))
            }
            Some("tool") => {
                // A tool's answer written as plain content names its call by `id` on the message.
                let parts = semconv_parts(m)
                    .into_iter()
                    .map(|b| match b {
                        Block::Text(t) => Block::tool_result(
                            str_of(m, "id").unwrap_or_default(),
                            vec![Block::Text(t)],
                            false,
                        ),
                        other => other,
                    })
                    .collect();
                push(&mut out.messages, Role::User, parts);
            }
            _ => push(&mut out.messages, Role::User, semconv_parts(m)),
        }
    }
    out.system = (!instructions.is_empty()).then(|| instructions.join("\n\n"));
    Some(out)
}

/// The parts of the reply in `gen_ai.output.messages`; the first output when there are several
/// choices.
pub fn semconv_reply(output: &str) -> Option<Vec<Block>> {
    let Ok(Json::Array(messages)) = serde_json::from_str::<Json>(output) else {
        return None;
    };
    Some(messages.first().map(semconv_parts).unwrap_or_default())
}

fn gemini_parts(content: &Json) -> Vec<Block> {
    content
        .get("parts")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            if let Some(call) = p.get("functionCall") {
                return Some(Block::tool_call(
                    str_of(call, "id").unwrap_or_default(),
                    str_of(call, "name")?,
                    arguments(call.get("args"))?,
                ));
            }
            if let Some(answer) = p.get("functionResponse") {
                return Some(tool_result(
                    str_of(answer, "id")
                        .or(str_of(answer, "name"))
                        .unwrap_or_default(),
                    answer.get("response").unwrap_or(&Json::Null),
                    false,
                ));
            }
            let text = str_of(p, "text").filter(|t| !t.is_empty())?;
            if p.get("thought").and_then(Json::as_bool) == Some(true) {
                reasoning(text)
            } else {
                Some(Block::text(text))
            }
        })
        .collect()
}

/// A Gemini request's `contents` (a list of `{role, parts}`), as Gemini CLI logs them.
pub fn gemini_contents(contents: &Json) -> Option<Exchange> {
    let list = contents
        .as_array()
        .or_else(|| contents.get("contents")?.as_array())?;
    let mut out = Exchange::default();
    for c in list {
        let role = match str_of(c, "role") {
            Some("model") => Role::Assistant,
            _ => Role::User,
        };
        push(&mut out.messages, role, gemini_parts(c));
    }
    Some(out)
}

fn first_candidate(response: &Json) -> Option<&Json> {
    response
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .or(response.get("content"))
}

/// A Gemini reply: one response, or a streamed reply's chunks, each a response whose first
/// candidate holds the next parts; a text or a thought split across chunks is joined again.
pub fn gemini_reply(reply: &Json) -> Option<Vec<Block>> {
    let parts: Vec<Block> = match reply {
        Json::Array(chunks) => chunks
            .iter()
            .filter_map(first_candidate)
            .flat_map(gemini_parts)
            .collect(),
        one => gemini_parts(first_candidate(one)?),
    };
    let mut joined: Vec<Block> = Vec::with_capacity(parts.len());
    for part in parts {
        match (joined.last_mut(), part) {
            (Some(Block::Text(last)), Block::Text(next)) => last.text.push_str(&next.text),
            (Some(Block::Reasoning(last)), Block::Reasoning(next)) => {
                if let (Some(l), Some(n)) = (last.text.last_mut(), next.text.first()) {
                    l.push_str(n);
                }
            }
            (_, part) => joined.push(part),
        }
    }
    Some(joined)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use serde_json::json;

    fn kinds(blocks: &[Block]) -> Vec<&'static str> {
        blocks.iter().map(Block::kind).collect()
    }

    #[test]
    fn openai_messages_keep_the_system_prompt_tool_calls_their_results_and_reasoning() {
        let e = openai_messages(Some(&json!([
            { "role": "system", "content": "Be brief." },
            { "role": "user", "content": "What's the weather?" },
            { "role": "assistant", "content": null, "reasoning_content": "Call the tool.",
              "tool_calls": [{ "id": "call_1", "type": "function",
                "function": { "name": "weather", "arguments": "{\"city\":\"Bern\"}" } }] },
            { "role": "tool", "tool_call_id": "call_1", "content": "Sunny" },
            { "role": "user", "content": [{ "type": "text", "text": "And tomorrow?" }] }
        ])));
        assert_eq!(e.system.as_deref(), Some("Be brief."));
        assert_eq!(e.messages.len(), 4);
        assert_eq!(kinds(&e.messages[1].blocks), ["reasoning", "tool_call"]);
        assert_eq!(kinds(&e.messages[2].blocks), ["tool_result"]);
        assert_eq!(e.last_user_text().as_deref(), Some("And tomorrow?"));
        let reply = openai_reply(&json!({ "content": "Rain.", "tool_calls": [{ "id": "c2",
            "function": { "name": "weather", "arguments": "not json" } }] }));
        assert_eq!(kinds(&reply), ["text", "tool_call"]);
    }

    #[test]
    fn genai_messages_read_every_part_type_and_plain_text_is_no_list() {
        let e = semconv_messages(
            &json!([
                { "role": "user", "parts": [{ "type": "text", "content": "Find the bug." }] },
                { "role": "assistant", "parts": [
                    { "type": "reasoning", "content": "Look at main." },
                    { "type": "tool_call", "id": "t1", "name": "read", "arguments": { "path": "main.rs" } }] },
                { "role": "tool", "parts": [{ "type": "tool_call_response", "id": "t1", "response": "fn main() {}" }] }
            ])
            .to_string(),
            Some(&json!([{ "type": "text", "content": "You fix bugs." }]).to_string()),
        )
        .unwrap();
        assert_eq!(e.system.as_deref(), Some("You fix bugs."));
        assert_eq!(kinds(&e.messages[1].blocks), ["reasoning", "tool_call"]);
        assert_eq!(kinds(&e.messages[2].blocks), ["tool_result"]);
        assert!(semconv_messages("Hello", None).is_none());
        let reply = semconv_reply(
            &json!([{ "role": "assistant", "parts": [{ "type": "text", "content": "Fixed." }], "finish_reason": "stop" }])
                .to_string(),
        )
        .unwrap();
        assert_eq!(kinds(&reply), ["text"]);
    }

    #[test]
    fn gemini_contents_read_text_thoughts_and_function_calls() {
        let e = gemini_contents(&json!([
            { "role": "user", "parts": [{ "text": "List the files." }] },
            { "role": "model", "parts": [{ "text": "I'll list them.", "thought": true },
                { "functionCall": { "name": "list_directory", "args": { "path": "." } } }] },
            { "role": "user", "parts": [{ "functionResponse": { "name": "list_directory", "response": { "output": "a.rs" } } }] }
        ]))
        .unwrap();
        assert_eq!(e.messages.len(), 3);
        assert_eq!(kinds(&e.messages[1].blocks), ["reasoning", "tool_call"]);
        assert_eq!(kinds(&e.messages[2].blocks), ["tool_result"]);
        assert_eq!(e.last_user_text().as_deref(), Some("List the files."));
        let reply = gemini_reply(&json!({ "candidates": [{ "content": { "role": "model", "parts": [{ "text": "a.rs" }] } }] }))
            .unwrap();
        assert_eq!(kinds(&reply), ["text"]);
        let streamed = gemini_reply(&json!([
            { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "Lo", "thought": true }] } }] },
            { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "oking.", "thought": true }] } }] },
            { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "a." }] } }] },
            { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "rs" }] } }], "usageMetadata": {} }
        ]))
        .unwrap();
        assert_eq!(kinds(&streamed), ["reasoning", "text"]);
        let Block::Text(t) = &streamed[1] else {
            unreachable!()
        };
        assert_eq!(t.text, "a.rs");
    }
}
