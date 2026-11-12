//! The wire shapes of OpenAI's Decisions API, from its API reference and `openai-python`'s
//! `decision_create_params.py` and `decision.py`.

use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::wire::Maybe;

/// `DecisionCreateParams`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RequestWire {
    pub model: String,
    pub input: RawJson,
    pub questions: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_options: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
}

/// `PromptCacheOptions`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct CacheOptionsWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

/// A user, developer, system or assistant message of the input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct MessageWire {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    pub role: String,
    pub content: RawJson,
}

/// `function_call`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct FunctionCallWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

/// `custom_tool_call`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct CustomCallWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub call_id: String,
    pub name: String,
    pub input: String,
}

/// `function_call_output` and `custom_tool_call_output`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct CallOutputWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub call_id: String,
    pub output: RawJson,
}

/// `DecisionInputText`, and an assistant's `output_text`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TextPartWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_breakpoint: Option<RawJson>,
}

/// `DecisionInputImage`; its `detail` travels as a field of the image's extras.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ImagePartWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub image_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_breakpoint: Option<RawJson>,
}

/// `DecisionInputJSON`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct JsonPartWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub data: RawJson,
}

/// An assistant's `refusal` part.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RefusalPartWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub refusal: String,
}

/// A `predicate`, `choice` or `score` question.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct QuestionWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<RawJson>,
}

/// One of a choice question's `choices`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChoiceWire {
    pub value: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One of a score question's `levels`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct LevelWire {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `Decision`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResponseWire {
    pub model: String,
    pub answers: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PredicateAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub name: Maybe<String>,
    pub probability: Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChoiceAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub name: Maybe<String>,
    pub choice: RawJson,
    pub probabilities: Vec<crate::wire::Ext<ChoiceProbabilityWire>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Number>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChoiceProbabilityWire {
    pub value: RawJson,
    pub probability: Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ScoreAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub name: Maybe<String>,
    pub score: Number,
    pub probabilities: Vec<crate::wire::Ext<ScoreProbabilityWire>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Number>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ScoreProbabilityWire {
    pub value: u64,
    pub label: String,
    pub probability: Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RefusalAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub name: Maybe<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct UsageWire {
    pub input_tokens: u64,
    #[serde(default)]
    pub input_tokens_details: Option<InputDetailsWire>,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub output_tokens_details: Option<OutputDetailsWire>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct InputDetailsWire {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct OutputDetailsWire {
    #[serde(default)]
    pub reasoning_tokens: u64,
}
