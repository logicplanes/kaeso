use kaeso_kpp::{Block, GenerateRequest, Message, RawJson, Response, Role, Source};
use serde::{Deserialize, Serialize};

use super::Phase;

/// What Kaeso posts to a guardrail.
#[derive(Clone, Debug, Serialize)]
pub struct Asked<'a> {
    pub id: &'a str,
    pub phase: Phase,
    pub request_id: &'a str,
    pub organization: &'a str,
    pub project: Option<&'a str>,
    pub key: Option<&'a str>,
    pub member: Option<&'a str>,
    pub service_account: Option<&'a str>,
    pub model: &'a str,
    pub format: &'a str,
    pub source: &'a str,
    pub messages: Vec<Said>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<Said>,
}

/// One message of a conversation, or a reply, as roles with their parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[schema(as = GuardrailMessage, description = "A message of a conversation, or a reply, with its parts.")]
pub struct Said {
    #[doc = "`system`, `user` or `assistant`."]
    #[schema(example = "user")]
    pub role: String,
    #[doc = "Its parts, in order."]
    pub content: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schema(as = GuardrailPart, description = "A part of a message: text, a tool call or result, or media as its type and\n\
    size.")]
pub enum Part {
    #[doc = "Text."]
    Text {
        #[doc = "The text."]
        text: String,
    },
    #[doc = "A model's refusal."]
    Refusal {
        #[doc = "The refusal's text."]
        text: String,
    },
    #[doc = "A call of a tool."]
    ToolCall {
        #[doc = "The call's id."]
        id: String,
        #[doc = "The tool's name."]
        name: String,
        #[doc = "The arguments as JSON text."]
        arguments: String,
    },
    #[doc = "The result of a tool call."]
    ToolResult {
        #[doc = "The id of the call it answers."]
        call_id: String,
        #[doc = "Whether the tool failed."]
        #[serde(default)]
        is_error: bool,
        #[doc = "The result's parts."]
        #[schema(no_recursion)]
        content: Vec<Part>,
    },
    #[doc = "An image, as its type and size."]
    Image {
        #[doc = "Its media type, when known."]
        #[serde(default)]
        media_type: Option<String>,
        #[doc = "Its size in bytes, when it was sent inline."]
        #[serde(default)]
        size: Option<u64>,
    },
    #[doc = "Audio, as its type and size."]
    Audio {
        #[doc = "Its media type, when known."]
        #[serde(default)]
        media_type: Option<String>,
        #[doc = "Its size in bytes, when it was sent inline."]
        #[serde(default)]
        size: Option<u64>,
    },
    #[doc = "A video, as its type and size."]
    Video {
        #[doc = "Its media type, when known."]
        #[serde(default)]
        media_type: Option<String>,
        #[doc = "Its size in bytes, when it was sent inline."]
        #[serde(default)]
        size: Option<u64>,
    },
    #[doc = "A document, as its type and size."]
    Document {
        #[doc = "Its media type, when known."]
        #[serde(default)]
        media_type: Option<String>,
        #[doc = "Its size in bytes, when it was sent inline."]
        #[serde(default)]
        size: Option<u64>,
    },
    #[doc = "Structured data as a part of its own."]
    Json {
        #[doc = "The data as JSON text."]
        text: String,
    },
    #[doc = "A model's reasoning, which a guardrail sees only as present."]
    Reasoning {},
    #[doc = "A part Kaeso has no form for here."]
    Other {},
}

/// A guardrail's answer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Verdict {
    Allow {
        #[serde(default)]
        category: Option<String>,
    },
    Block {
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    Replace {
        messages: Vec<Said>,
        #[serde(default)]
        category: Option<String>,
    },
}

/// A `replace` whose messages don't have the shape of what was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Misshapen;

const MAX_CATEGORY: usize = 64;
const MAX_MESSAGE: usize = 300;

fn size(source: &Source) -> (Option<String>, Option<u64>) {
    match source {
        Source::Base64(b) => (Some(b.media_type.clone()), u64::try_from(b.data.len()).ok()),
        Source::Text(t) => (Some(t.media_type.clone()), u64::try_from(t.data.len()).ok()),
        _ => (source.media_type().map(str::to_owned), None),
    }
}

pub fn part(block: &Block) -> Part {
    match block {
        Block::Text(t) => Part::Text {
            text: t.text.clone(),
        },
        Block::Refusal(r) => Part::Refusal {
            text: r.text.clone(),
        },
        Block::ToolCall(c) => Part::ToolCall {
            id: c.id.clone(),
            name: c.name.clone(),
            arguments: c.arguments.get().to_owned(),
        },
        Block::ToolResult(r) => Part::ToolResult {
            call_id: r.call_id.clone(),
            is_error: r.is_error,
            content: r.content.iter().map(part).collect(),
        },
        Block::Image(i) => {
            let (media_type, size) = size(&i.source);
            Part::Image { media_type, size }
        }
        Block::Audio(a) => {
            let (media_type, size) = size(&a.source);
            Part::Audio { media_type, size }
        }
        Block::Video(v) => {
            let (media_type, size) = size(&v.source);
            Part::Video { media_type, size }
        }
        Block::Document(d) => {
            let (media_type, size) = size(&d.source);
            Part::Document { media_type, size }
        }
        Block::Json(j) => Part::Json {
            text: j.data.get().to_owned(),
        },
        Block::Reasoning(_) => Part::Reasoning {},
        _ => Part::Other {},
    }
}

fn role(r: Role) -> &'static str {
    match r {
        Role::User => "user",
        Role::Assistant => "assistant",
        _ => "system",
    }
}

/// The conversation of a request: its system prompt as a first `system` message, then its
/// messages.
pub fn conversation(g: &GenerateRequest) -> Vec<Said> {
    let system = (!g.system.is_empty()).then(|| Said {
        role: "system".to_owned(),
        content: g.system.iter().map(part).collect(),
    });
    system
        .into_iter()
        .chain(g.messages.iter().map(|m: &Message| Said {
            role: role(m.role).to_owned(),
            content: m.blocks.iter().map(part).collect(),
        }))
        .collect()
}

pub fn reply(r: &Response) -> Said {
    Said {
        role: "assistant".to_owned(),
        content: r.blocks.iter().map(part).collect(),
    }
}

// Writes a replacement's text back into a block of the same kind; everything a guardrail can't
// change (media, reasoning, ids and names) stays as it was.
fn write(block: &mut Block, with: &Part) -> Result<(), Misshapen> {
    match (block, with) {
        (Block::Text(t), Part::Text { text }) => t.text.clone_from(text),
        (Block::Refusal(r), Part::Refusal { text }) => r.text.clone_from(text),
        (Block::ToolCall(c), Part::ToolCall { arguments, .. }) => {
            c.arguments = RawJson::from_string(arguments.clone()).map_err(|_| Misshapen)?;
        }
        (Block::ToolResult(r), Part::ToolResult { content, .. }) => {
            write_all(&mut r.content, content)?;
        }
        (Block::Json(j), Part::Json { text }) => {
            j.data = RawJson::from_string(text.clone()).map_err(|_| Misshapen)?;
        }
        (block, with) if part(block).same_kind(with) => {}
        _ => return Err(Misshapen),
    }
    Ok(())
}

fn write_all(blocks: &mut [Block], parts: &[Part]) -> Result<(), Misshapen> {
    if blocks.len() != parts.len() {
        return Err(Misshapen);
    }
    for (block, with) in blocks.iter_mut().zip(parts) {
        write(block, with)?;
    }
    Ok(())
}

impl Part {
    fn same_kind(&self, other: &Part) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Applies a `replace` to a request: the messages must match the conversation Kaeso sent,
/// message by message and part by part.
pub fn replace_request(g: &mut GenerateRequest, with: &[Said]) -> Result<(), Misshapen> {
    let mut with = with.iter();
    if !g.system.is_empty() {
        let first = with.next().ok_or(Misshapen)?;
        if first.role != "system" {
            return Err(Misshapen);
        }
        write_all(&mut g.system, &first.content)?;
    }
    let rest: Vec<&Said> = with.collect();
    if rest.len() != g.messages.len() {
        return Err(Misshapen);
    }
    for (m, said) in g.messages.iter_mut().zip(rest) {
        if said.role != role(m.role) {
            return Err(Misshapen);
        }
        write_all(&mut m.blocks, &said.content)?;
    }
    Ok(())
}

/// Applies a `replace` to a reply: one `assistant` message with the reply's parts.
pub fn replace_reply(r: &mut Response, with: &[Said]) -> Result<(), Misshapen> {
    let [said] = with else {
        return Err(Misshapen);
    };
    if said.role != "assistant" {
        return Err(Misshapen);
    }
    write_all(&mut r.blocks, &said.content)
}

fn clean(text: &str, max: usize) -> Option<String> {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max)
        .collect();
    let cleaned = cleaned.trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

/// A category as the log keeps it: at most 64 characters, without control characters.
pub fn category(raw: Option<&str>) -> Option<String> {
    raw.and_then(|c| clean(c, MAX_CATEGORY))
}

/// A block's message as an app sees it: at most 300 characters, ending with a period.
pub fn message(raw: Option<&str>) -> Option<String> {
    let mut m = raw.and_then(|m| clean(m, MAX_MESSAGE))?;
    if !m.ends_with(['.', '!', '?']) {
        m.push('.');
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{Block, GenerateRequest, Message, Role, Source};

    fn request() -> GenerateRequest {
        GenerateRequest::new("claude-haiku-4-5")
            .with_system("Be brief.")
            .with_message(Message::new(
                Role::User,
                vec![
                    Block::text("Call ada@example.com"),
                    Block::image(Source::bytes("image/png", vec![1, 2, 3])),
                ],
            ))
            .with_message(Message::new(
                Role::Assistant,
                vec![Block::tool_call(
                    "call_1",
                    "lookup",
                    RawJson::from_string(r#"{"q":"ada@example.com"}"#.to_owned()).unwrap(),
                )],
            ))
            .with_message(Message::new(
                Role::User,
                vec![Block::tool_result(
                    "call_1",
                    vec![Block::text("found ada@example.com")],
                    false,
                )],
            ))
    }

    #[test]
    fn a_conversation_carries_text_tool_calls_and_media_without_their_bytes() {
        let said = conversation(&request());
        let json = serde_json::to_value(&said).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                {"role": "system", "content": [{"type": "text", "text": "Be brief."}]},
                {"role": "user", "content": [
                    {"type": "text", "text": "Call ada@example.com"},
                    {"type": "image", "media_type": "image/png", "size": 3}
                ]},
                {"role": "assistant", "content": [
                    {"type": "tool_call", "id": "call_1", "name": "lookup", "arguments": "{\"q\":\"ada@example.com\"}"}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "call_id": "call_1", "is_error": false, "content": [
                        {"type": "text", "text": "found ada@example.com"}
                    ]}
                ]}
            ])
        );
    }

    #[test]
    fn a_replacement_of_the_same_shape_rewrites_text_and_arguments() {
        let mut g = request();
        let mut with = conversation(&g);
        for said in &mut with {
            for p in &mut said.content {
                match p {
                    Part::Text { text } => *text = text.replace("ada@example.com", "[EMAIL]"),
                    Part::ToolCall { arguments, .. } => {
                        *arguments = arguments.replace("ada@example.com", "[EMAIL]");
                    }
                    Part::ToolResult { content, .. } => {
                        content[0] = Part::Text {
                            text: "found [EMAIL]".to_owned(),
                        };
                    }
                    _ => {}
                }
            }
        }
        replace_request(&mut g, &with).unwrap();
        assert_eq!(conversation(&g), with);
        assert!(matches!(&g.messages[0].blocks[1], Block::Image(_)));
    }

    #[test]
    fn a_replacement_of_another_shape_is_refused() {
        let g = request();
        let sent = conversation(&g);
        let mut fewer = sent.clone();
        fewer.pop();
        assert_eq!(replace_request(&mut g.clone(), &fewer), Err(Misshapen));
        let mut other_role = sent.clone();
        other_role[1].role = "assistant".to_owned();
        assert_eq!(replace_request(&mut g.clone(), &other_role), Err(Misshapen));
        let mut other_kind = sent.clone();
        other_kind[1].content[1] = Part::Text {
            text: "no image".to_owned(),
        };
        assert_eq!(replace_request(&mut g.clone(), &other_kind), Err(Misshapen));
        let mut bad_json = sent.clone();
        bad_json[2].content[0] = Part::ToolCall {
            id: "call_1".to_owned(),
            name: "lookup".to_owned(),
            arguments: "{not json".to_owned(),
        };
        assert_eq!(replace_request(&mut g.clone(), &bad_json), Err(Misshapen));
        let mut no_system = sent;
        no_system.remove(0);
        assert_eq!(replace_request(&mut g.clone(), &no_system), Err(Misshapen));
    }

    #[test]
    fn a_reply_is_replaced_by_one_assistant_message() {
        let mut r = Response::new("r", "m", vec![Block::text("secret")]);
        let with = [Said {
            role: "assistant".to_owned(),
            content: vec![Part::Text {
                text: "[REDACTED]".to_owned(),
            }],
        }];
        replace_reply(&mut r, &with).unwrap();
        assert_eq!(reply(&r).content, with[0].content);
        assert_eq!(replace_reply(&mut r, &[]), Err(Misshapen));
        let user = [Said {
            role: "user".to_owned(),
            content: with[0].content.clone(),
        }];
        assert_eq!(replace_reply(&mut r, &user), Err(Misshapen));
    }

    #[test]
    fn verdicts_read_every_action_and_ignore_unknown_fields() {
        let allow: Verdict = serde_json::from_str(r#"{"action":"allow","score":0.1}"#).unwrap();
        assert_eq!(allow, Verdict::Allow { category: None });
        let block: Verdict = serde_json::from_str(
            r#"{"action":"block","message":"No secrets","category":"secrets"}"#,
        )
        .unwrap();
        assert_eq!(
            block,
            Verdict::Block {
                message: Some("No secrets".to_owned()),
                category: Some("secrets".to_owned())
            }
        );
        let replace: Verdict = serde_json::from_str(
            r#"{"action":"replace","messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]}"#,
        )
        .unwrap();
        assert!(matches!(replace, Verdict::Replace { messages, .. } if messages.len() == 1));
        assert!(serde_json::from_str::<Verdict>(r#"{"action":"maybe"}"#).is_err());
    }

    #[test]
    fn messages_and_categories_are_cleaned() {
        assert_eq!(message(Some("No secrets")).as_deref(), Some("No secrets."));
        assert_eq!(message(Some("Stop!")).as_deref(), Some("Stop!"));
        assert_eq!(message(Some(" \n ")), None);
        assert_eq!(message(None), None);
        assert_eq!(message(Some(&"x".repeat(400))).unwrap().len(), 301);
        assert_eq!(category(Some("pii\u{7}")).as_deref(), Some("pii"));
        assert_eq!(category(Some(&"c".repeat(80))).unwrap().len(), 64);
    }

    #[test]
    fn media_from_links_has_no_size() {
        assert_eq!(
            part(&Block::image(Source::Url(
                "https://example.com/a.png".to_owned()
            ))),
            Part::Image {
                media_type: None,
                size: None
            }
        );
        let doc = Block::Document(kaeso_kpp::DocumentBlock::new(Source::text(
            "text/plain",
            "hello",
        )));
        assert_eq!(
            part(&doc),
            Part::Document {
                media_type: Some("text/plain".to_owned()),
                size: Some(5)
            }
        );
    }
}
