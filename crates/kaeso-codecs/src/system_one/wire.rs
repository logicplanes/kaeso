use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::wire::{B64, Maybe};

/// `SystemOneRequest` as an app sends it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RequestIn {
    pub model: String,
    pub state: RawJson,
    pub questions: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<RawJson>,
}

/// The request Kaeso sends, with a Root provider's `images`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RequestOut {
    pub model: String,
    pub state: RawJson,
    pub questions: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct QuestionWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct NoulCriteriaWire {
    #[serde(rename = "true", default, skip_serializing_if = "Option::is_none")]
    pub when_true: Option<RawJson>,
    #[serde(rename = "false", default, skip_serializing_if = "Option::is_none")]
    pub when_false: Option<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct AssetWire {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<B64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
}

/// An OpenAI-style image part, as Perplexity, OpenRouter and Baseten take images in the state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImagePartWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub image_url: crate::wire::Ext<ImageUrlWire>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImageUrlWire {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResponseWire {
    pub model: String,
    pub answers: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UsageWire {
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct NoulAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub noul: Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChoiceAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub choice: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub confidence: Maybe<Number>,
    pub probabilities: RawJson,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ScoreAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    pub score: Number,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub confidence: Maybe<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<RawJson>,
    pub probabilities: RawJson,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct BoxAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(rename = "box")]
    pub coordinates: crate::wire::Ext<BoxWire>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub confidence: Maybe<Number>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct BoxWire {
    pub top: Number,
    pub bottom: Number,
    pub left: Number,
    pub right: Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RefusalAnswerWire {
    #[serde(rename = "type")]
    pub ty: String,
}
