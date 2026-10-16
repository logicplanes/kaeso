use serde::{Deserialize, Serialize};

use crate::block::{Block, CacheHint, RawItem};
use crate::extras::Extras;
use crate::raw::RawJson;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Request {
    Generate(GenerateRequest),
    CountTokens(GenerateRequest),
    Embed(EmbedRequest),
}

impl Request {
    pub fn kind(&self) -> RequestKind {
        match self {
            Request::Generate(_) => RequestKind::Generate,
            Request::CountTokens(_) => RequestKind::CountTokens,
            Request::Embed(_) => RequestKind::Embed,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Request::Generate(r) | Request::CountTokens(r) => &r.model,
            Request::Embed(r) => &r.model,
        }
    }

    pub fn stream_mode(&self) -> StreamMode {
        match self {
            Request::Generate(r) => r.stream,
            Request::CountTokens(_) | Request::Embed(_) => StreamMode::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RequestKind {
    Generate,
    CountTokens,
    Embed,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GenerateRequest {
    pub model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system: Vec<Block>,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "OutputSpec::is_text")]
    pub output: OutputSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(default, skip_serializing_if = "Sampling::is_empty")]
    pub sampling: Sampling,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "StreamMode::is_off")]
    pub stream: StreamMode,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl GenerateRequest {
    pub fn new(model: impl Into<String>) -> Self {
        GenerateRequest {
            model: model.into(),
            ..GenerateRequest::default()
        }
    }

    #[must_use]
    pub fn with_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    #[must_use]
    pub fn with_system(mut self, text: impl Into<String>) -> Self {
        self.system = vec![Block::text(text)];
        self
    }

    #[must_use]
    pub fn with_tool(mut self, tool: ToolDef) -> Self {
        self.tools.push(tool);
        self
    }

    #[must_use]
    pub fn with_stream(mut self, stream: StreamMode) -> Self {
        self.stream = stream;
        self
    }

    pub fn all_blocks(&self) -> impl Iterator<Item = &Block> {
        let mut stack: Vec<&Block> = Vec::new();
        for m in self.messages.iter().rev() {
            stack.extend(m.blocks.iter().rev());
        }
        stack.extend(self.system.iter().rev());
        std::iter::from_fn(move || {
            let block = stack.pop()?;
            if let Block::ToolResult(result) = block {
                stack.extend(result.content.iter().rev());
            }
            Some(block)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Message {
    pub fn new(role: Role, blocks: Vec<Block>) -> Self {
        Message {
            role,
            blocks,
            extras: Extras::default(),
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Message::new(Role::User, vec![Block::text(text)])
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Message::new(Role::Assistant, vec![Block::text(text)])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolDef {
    Function(FunctionTool),
    Unknown(RawItem),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FunctionTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl FunctionTool {
    pub fn new(name: impl Into<String>, parameters: Option<RawJson>) -> Self {
        FunctionTool {
            name: name.into(),
            description: None,
            parameters,
            strict: None,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Tool(String),
    Unknown(RawItem),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutputSpec {
    #[default]
    Text,
    JsonObject,
    JsonSchema(JsonSchemaSpec),
}

impl OutputSpec {
    pub fn is_text(&self) -> bool {
        matches!(self, OutputSpec::Text)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JsonSchemaSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schema: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl JsonSchemaSpec {
    pub fn new(schema: RawJson) -> Self {
        JsonSchemaSpec {
            name: None,
            description: None,
            schema,
            strict: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReasoningConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ReasoningMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummary>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_encrypted: bool,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReasoningMode {
    Enabled,
    Disabled,
    Adaptive,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
    Other(String),
}

impl ReasoningEffort {
    pub fn from_wire(s: &str) -> Self {
        match s {
            "none" => ReasoningEffort::None,
            "minimal" => ReasoningEffort::Minimal,
            "low" => ReasoningEffort::Low,
            "medium" => ReasoningEffort::Medium,
            "high" => ReasoningEffort::High,
            "xhigh" => ReasoningEffort::XHigh,
            "max" => ReasoningEffort::Max,
            other => ReasoningEffort::Other(other.to_owned()),
        }
    }

    pub fn as_wire(&self) -> &str {
        match self {
            ReasoningEffort::None => "none",
            ReasoningEffort::Minimal => "minimal",
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
            ReasoningEffort::XHigh => "xhigh",
            ReasoningEffort::Max => "max",
            ReasoningEffort::Other(s) => s,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    Other(String),
}

impl ReasoningSummary {
    pub fn from_wire(s: &str) -> Self {
        match s {
            "auto" => ReasoningSummary::Auto,
            "concise" => ReasoningSummary::Concise,
            "detailed" => ReasoningSummary::Detailed,
            other => ReasoningSummary::Other(other.to_owned()),
        }
    }

    pub fn as_wire(&self) -> &str {
        match self {
            ReasoningSummary::Auto => "auto",
            ReasoningSummary::Concise => "concise",
            ReasoningSummary::Detailed => "detailed",
            ReasoningSummary::Other(s) => s,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sampling {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
}

impl Sampling {
    pub fn is_empty(&self) -> bool {
        *self == Sampling::default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StreamMode {
    #[default]
    Off,
    On {
        include_usage: bool,
    },
}

impl StreamMode {
    pub fn is_off(&self) -> bool {
        matches!(self, StreamMode::Off)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedRequest {
    pub model: String,
    pub input: Vec<EmbedInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(default)]
    pub encoding: EmbedEncoding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl EmbedRequest {
    pub fn new(model: impl Into<String>, input: Vec<EmbedInput>) -> Self {
        EmbedRequest {
            model: model.into(),
            input,
            ..EmbedRequest::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmbedInput {
    Text(String),
    Tokens(Vec<u32>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmbedEncoding {
    #[default]
    Float,
    Base64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_blocks_walks_tool_results_in_order() {
        let req = GenerateRequest::new("m")
            .with_system("sys")
            .with_message(Message::user("a"))
            .with_message(Message::new(
                Role::User,
                vec![Block::tool_result(
                    "c1",
                    vec![Block::text("r1"), Block::text("r2")],
                    false,
                )],
            ));
        let kinds: Vec<_> = req
            .all_blocks()
            .map(|b| match b {
                Block::Text(t) => t.text.clone(),
                other => other.kind().to_owned(),
            })
            .collect();
        assert_eq!(kinds, ["sys", "a", "tool_result", "r1", "r2"]);
    }

    #[test]
    fn minimal_request_has_a_small_stable_form() {
        let req = GenerateRequest::new("m").with_message(Message::user("hi"));
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"model":"m","messages":[{"role":"user","blocks":[{"text":{"text":"hi"}}]}]}"#
        );
        let back: GenerateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn effort_and_summary_round_trip_wire_values() {
        for s in [
            "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
        ] {
            assert_eq!(ReasoningEffort::from_wire(s).as_wire(), s);
        }
        for s in ["auto", "concise", "detailed", "brief"] {
            assert_eq!(ReasoningSummary::from_wire(s).as_wire(), s);
        }
    }
}
