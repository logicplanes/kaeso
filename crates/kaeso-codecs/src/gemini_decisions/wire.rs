//! The wire shapes of Gemini's Decisions API, from its API reference and `google-genai`'s
//! `types/decisions` and the Interactions content blocks they reuse.

use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::wire::B64;

/// `CreateDecisionRequest`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RequestWire {
    pub model: String,
    pub input: RawJson,
    pub questions: RawJson,
}

/// `TextContent`, the only block question instructions take.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TextWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub text: String,
}

/// `ImageContent`, `AudioContent`, `VideoContent` and `DocumentContent`: inline base64 `data` or
/// a `uri`, and the MIME type. `resolution` and the other fields of each block travel as extras.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct MediaWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<B64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// `NoulQuestion`, `ChoiceQuestion` and `ScoreQuestion`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct QuestionWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<RawJson>,
}

/// `NoulCriteria`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct NoulCriteriaWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub true_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_description: Option<String>,
}

/// `CreateDecisionResponse`; every field is optional in the SDK's type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResponseWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<RawJson>,
}

/// `PromptFeedback`.
#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct PromptFeedbackWire {
    #[serde(default)]
    pub block_reason: Option<String>,
    #[serde(default)]
    pub block_reason_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct NoulAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub noul: Number,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Number>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChoiceAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub choice: String,
    pub probabilities: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Number>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ScoreAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub score: Number,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<RawJson>,
    pub probabilities: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Number>,
}

/// The fields of the Interactions `Usage` KPP reads; the rest is kept with the usage as sent.
#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct UsageWire {
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub input_tokens_by_modality: Vec<ModalityWire>,
    #[serde(default)]
    pub total_cached_tokens: u64,
    #[serde(default)]
    pub total_output_tokens: u64,
    #[serde(default)]
    pub total_thought_tokens: Option<u64>,
}

/// `ModalityTokens`.
#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct ModalityWire {
    #[serde(default)]
    pub modality: String,
    #[serde(default)]
    pub tokens: u64,
}
