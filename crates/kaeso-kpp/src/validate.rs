use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::block::{Block, Source};
use crate::error::KppError;
use crate::request::{
    EmbedRequest, GenerateRequest, OutputSpec, ReasoningEffort, ReasoningMode, Request, Role,
    StreamMode, ToolChoice, ToolDef,
};

impl Request {
    pub fn validate(&self) -> Result<(), KppError> {
        match self {
            Request::Generate(r) | Request::CountTokens(r) => r.validate(),
            Request::Embed(r) => r.validate(),
        }
    }
}

impl GenerateRequest {
    pub fn validate(&self) -> Result<(), KppError> {
        if self.model.is_empty() {
            return Err(invalid("`model` must not be empty.", "model"));
        }
        for (i, block) in self.system.iter().enumerate() {
            if !matches!(block, Block::Text(_) | Block::Unknown(_)) {
                return Err(invalid(
                    format!(
                        "The system prompt may only contain text, but contains a {} block.",
                        block.kind()
                    ),
                    format!("system[{i}]"),
                ));
            }
        }

        let mut calls: HashSet<String> = HashSet::new();
        for (mi, message) in self.messages.iter().enumerate() {
            if message.blocks.is_empty() && message.extras.fields.is_empty() {
                return Err(invalid(
                    format!("Message {mi} has no content."),
                    format!("messages[{mi}]"),
                ));
            }
            for (bi, block) in message.blocks.iter().enumerate() {
                let param = || format!("messages[{mi}].blocks[{bi}]");
                match block {
                    Block::ToolCall(call) => {
                        let id = &call.id;
                        if message.role != Role::Assistant {
                            return Err(invalid(
                                format!(
                                    "Tool calls may only appear in assistant messages, but message {mi} is a {} message.",
                                    message.role.as_str()
                                ),
                                param(),
                            ));
                        }
                        if !calls.insert(id.clone()) {
                            return Err(invalid(
                                format!("Tool call id `{id}` is used more than once."),
                                param(),
                            ));
                        }
                    }
                    Block::ToolResult(result) => {
                        let (call_id, content) = (&result.call_id, &result.content);
                        if message.role != Role::User {
                            return Err(invalid(
                                format!(
                                    "Tool results may only appear in user messages, but message {mi} is a {} message.",
                                    message.role.as_str()
                                ),
                                param(),
                            ));
                        }
                        if !calls.contains(call_id) {
                            return Err(invalid(
                                format!(
                                    "The tool result for `{call_id}` does not answer any earlier tool call."
                                ),
                                param(),
                            ));
                        }
                        if let Some((ci, nested)) = content.iter().enumerate().find(|(_, b)| {
                            !matches!(
                                b,
                                Block::Text(_)
                                    | Block::Image(_)
                                    | Block::Document(_)
                                    | Block::Unknown(_)
                            )
                        }) {
                            return Err(invalid(
                                format!(
                                    "A tool result may contain text, images and documents, but not a {} block.",
                                    nested.kind()
                                ),
                                format!("{}.content[{ci}]", param()),
                            ));
                        }
                    }
                    Block::Unknown(item) => calls.extend(unknown_call_ids(&item.raw)),
                    _ => {}
                }
            }
        }

        let mut names: HashSet<&str> = HashSet::new();
        for (i, tool) in self.tools.iter().enumerate() {
            if let ToolDef::Function(f) = tool {
                if f.name.is_empty() {
                    return Err(invalid(
                        "Tool names must not be empty.",
                        format!("tools[{i}]"),
                    ));
                }
                if !names.insert(&f.name) {
                    return Err(invalid(
                        format!("Tool `{}` is defined more than once.", f.name),
                        format!("tools[{i}]"),
                    ));
                }
            }
        }
        if let Some(ToolChoice::Tool(name)) = &self.tool_choice {
            let has_unknown_tools = self.tools.iter().any(|t| matches!(t, ToolDef::Unknown(_)));
            if !names.contains(name.as_str()) && !has_unknown_tools {
                return Err(invalid(
                    format!("`tool_choice` names `{name}`, which is not a defined tool."),
                    "tool_choice",
                ));
            }
        }
        Ok(())
    }

    pub fn required_capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::default();
        for block in self.all_blocks() {
            match block {
                Block::Image(_) => caps.images = true,
                Block::Document(doc) => match doc.source {
                    Source::Text(_) => caps.text_documents = true,
                    _ => caps.pdf = true,
                },
                _ => {}
            }
            if block.cache().is_some() {
                caps.cache_hints = true;
            }
        }
        let has_tool_blocks = self
            .messages
            .iter()
            .flat_map(|m| &m.blocks)
            .any(|b| matches!(b, Block::ToolCall(_) | Block::ToolResult(_)));
        caps.tools = !self.tools.is_empty() || has_tool_blocks;
        caps.parallel_tools = self.messages.iter().any(|m| {
            m.blocks
                .iter()
                .filter(|b| matches!(b, Block::ToolCall(_)))
                .count()
                > 1
        });
        caps.reasoning = self.reasoning.as_ref().is_some_and(|r| {
            r.mode
                .as_ref()
                .is_some_and(|m| *m != ReasoningMode::Disabled)
                || r.effort
                    .as_ref()
                    .is_some_and(|e| *e != ReasoningEffort::None)
                || r.budget_tokens.is_some()
        });
        caps.json_schema = matches!(self.output, OutputSpec::JsonSchema { .. });
        caps.json_object = matches!(self.output, OutputSpec::JsonObject);
        caps.streaming = matches!(self.stream, StreamMode::On { .. });
        if self.tools.iter().any(|t| match t {
            ToolDef::Function(f) => f.cache.is_some(),
            ToolDef::Unknown(_) => false,
        }) {
            caps.cache_hints = true;
        }
        caps
    }
}

impl EmbedRequest {
    pub fn validate(&self) -> Result<(), KppError> {
        if self.model.is_empty() {
            return Err(invalid("`model` must not be empty.", "model"));
        }
        if self.input.is_empty() {
            return Err(invalid("`input` must not be empty.", "input"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Capabilities {
    pub images: bool,
    pub pdf: bool,
    pub text_documents: bool,
    pub tools: bool,
    pub parallel_tools: bool,
    pub reasoning: bool,
    pub json_schema: bool,
    pub json_object: bool,
    pub streaming: bool,
    pub cache_hints: bool,
}

fn unknown_call_ids(raw: &crate::RawJson) -> Vec<String> {
    #[derive(Deserialize)]
    struct Ids {
        #[serde(default)]
        id: Option<serde_json::Value>,
        #[serde(default)]
        call_id: Option<serde_json::Value>,
    }
    if !raw.is_object() {
        return Vec::new();
    }
    raw.parse::<Ids>()
        .map(|ids| {
            [ids.id, ids.call_id]
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{CacheHint, DocumentBlock, ImageBlock};
    use crate::raw::RawJson;
    use crate::request::{FunctionTool, Message, ReasoningConfig};

    fn args() -> RawJson {
        RawJson::from_string("{}".into()).unwrap()
    }

    fn tool_turns() -> GenerateRequest {
        GenerateRequest::new("m")
            .with_message(Message::user("weather?"))
            .with_message(Message::new(
                Role::Assistant,
                vec![Block::tool_call("c1", "get_weather", args())],
            ))
            .with_message(Message::new(
                Role::User,
                vec![Block::tool_result("c1", vec![Block::text("sunny")], false)],
            ))
    }

    #[test]
    fn valid_tool_conversation_passes() {
        tool_turns().validate().unwrap();
    }

    #[test]
    fn empty_model_and_empty_message_fail() {
        let e = GenerateRequest::new("").validate().unwrap_err();
        assert_eq!(e.param.as_deref(), Some("model"));
        let e = GenerateRequest::new("m")
            .with_message(Message::new(Role::User, vec![]))
            .validate()
            .unwrap_err();
        assert_eq!(e.param.as_deref(), Some("messages[0]"));
    }

    #[test]
    fn tool_result_must_answer_an_earlier_call() {
        let req = GenerateRequest::new("m").with_message(Message::new(
            Role::User,
            vec![Block::tool_result("nope", vec![], false)],
        ));
        let e = req.validate().unwrap_err();
        assert_eq!(e.param.as_deref(), Some("messages[0].blocks[0]"));
        assert!(e.message.contains("nope"));
    }

    #[test]
    fn unknown_calls_and_extras_only_messages_are_accepted() {
        use crate::{DialectId, Extras, RawItem};
        let custom = Block::Unknown(RawItem::new(
            DialectId::OpenAiChat,
            RawJson::from_string(r#"{"id":"ct1","type":"custom"}"#.into()).unwrap(),
        ));
        let mut audio_only = Message::new(Role::Assistant, vec![]);
        audio_only.extras =
            Extras::new(DialectId::OpenAiChat).with_field("audio", RawJson::empty_object());
        GenerateRequest::new("m")
            .with_message(Message::user("x"))
            .with_message(audio_only)
            .with_message(Message::new(Role::Assistant, vec![custom]))
            .with_message(Message::new(
                Role::User,
                vec![Block::tool_result("ct1", vec![Block::text("ok")], false)],
            ))
            .validate()
            .unwrap();
    }

    #[test]
    fn tool_blocks_must_have_the_right_role() {
        let req = GenerateRequest::new("m").with_message(Message::new(
            Role::User,
            vec![Block::tool_call("c1", "f", args())],
        ));
        assert!(req.validate().unwrap_err().message.contains("assistant"));

        let mut req = tool_turns();
        req.messages[2].role = Role::Assistant;
        assert!(req.validate().unwrap_err().message.contains("user"));
    }

    #[test]
    fn duplicate_call_ids_and_tool_names_fail() {
        let mut req = tool_turns();
        req.messages[1]
            .blocks
            .push(Block::tool_call("c1", "get_weather", args()));
        assert!(
            req.validate()
                .unwrap_err()
                .message
                .contains("more than once")
        );

        let req = GenerateRequest::new("m")
            .with_tool(ToolDef::Function(FunctionTool::new("f", None)))
            .with_tool(ToolDef::Function(FunctionTool::new("f", None)));
        assert_eq!(
            req.validate().unwrap_err().param.as_deref(),
            Some("tools[1]")
        );
    }

    #[test]
    fn forced_tool_must_exist() {
        let mut req =
            GenerateRequest::new("m").with_tool(ToolDef::Function(FunctionTool::new("f", None)));
        req.tool_choice = Some(ToolChoice::Tool("g".into()));
        assert_eq!(
            req.validate().unwrap_err().param.as_deref(),
            Some("tool_choice")
        );
        req.tool_choice = Some(ToolChoice::Tool("f".into()));
        req.validate().unwrap();
    }

    #[test]
    fn nested_tool_result_content_is_restricted() {
        let mut req = tool_turns();
        req.messages[2].blocks = vec![Block::tool_result(
            "c1",
            vec![Block::tool_call("c2", "f", args())],
            false,
        )];
        assert_eq!(
            req.validate().unwrap_err().param.as_deref(),
            Some("messages[2].blocks[0].content[0]")
        );
    }

    #[test]
    fn system_prompt_is_text_only() {
        let mut req = GenerateRequest::new("m").with_message(Message::user("x"));
        req.system = vec![Block::image(Source::Url("https://x/y.png".into()))];
        assert_eq!(
            req.validate().unwrap_err().param.as_deref(),
            Some("system[0]")
        );
    }

    #[test]
    fn capabilities_cover_inputs_tools_and_options() {
        let mut req = tool_turns();
        assert_eq!(
            req.required_capabilities(),
            Capabilities {
                tools: true,
                ..Capabilities::default()
            }
        );

        let mut image = ImageBlock::new(Source::Url("https://x/y.png".into()));
        image.cache = Some(CacheHint::default());
        req.messages[0].blocks.push(Block::Image(image));
        req.messages[2].blocks = vec![Block::tool_result(
            "c1",
            vec![Block::Document(DocumentBlock::new(Source::base64(
                "application/pdf",
                "JVBERi0=",
            )))],
            false,
        )];
        req.messages[1]
            .blocks
            .push(Block::tool_call("c2", "get_weather", args()));
        req.reasoning = Some(ReasoningConfig {
            budget_tokens: Some(1024),
            ..ReasoningConfig::default()
        });
        req.output = OutputSpec::JsonObject;
        req.stream = StreamMode::on(false);
        let caps = req.required_capabilities();
        assert!(caps.images && caps.pdf && caps.cache_hints && caps.parallel_tools);
        assert!(caps.reasoning && caps.json_object && caps.streaming);
        assert!(!caps.json_schema && !caps.text_documents);
    }

    #[test]
    fn permissions_and_effort_none_need_no_capability() {
        let mut req = GenerateRequest::new("m");
        req.parallel_tool_calls = Some(true);
        req.reasoning = Some(ReasoningConfig {
            effort: Some(ReasoningEffort::None),
            ..ReasoningConfig::default()
        });
        let caps = req.required_capabilities();
        assert!(!caps.parallel_tools && !caps.reasoning);
    }

    #[test]
    fn disabled_reasoning_needs_no_reasoning_capability() {
        let mut req = GenerateRequest::new("m");
        req.reasoning = Some(ReasoningConfig {
            mode: Some(ReasoningMode::Disabled),
            ..ReasoningConfig::default()
        });
        assert!(!req.required_capabilities().reasoning);
    }

    #[test]
    fn embed_requests_need_input() {
        use crate::request::EmbedInput;
        assert!(EmbedRequest::new("m", vec![]).validate().is_err());
        EmbedRequest::new("m", vec![EmbedInput::Text("x".into())])
            .validate()
            .unwrap();
    }
}
