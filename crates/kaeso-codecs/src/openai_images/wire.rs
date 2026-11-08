use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};

use crate::wire::Ext;

// One request type for generations and JSON edits: `CreateImageRequest` and
// `EditImageBodyJsonParam` at openai-openapi 926a7b8 share every field but the edit's images.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImageRequestW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<Ext<ImageRefW>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Ext<ImageRefW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
}

pub(super) const REQUEST_FIELDS: &[&str] = &[
    "model",
    "prompt",
    "images",
    "mask",
    "n",
    "size",
    "quality",
    "response_format",
    "output_format",
    "output_compression",
    "background",
    "stream",
    "partial_images",
    "safety_identifier",
    "user",
    "aspect_ratio",
    "resolution",
    "seed",
    "negative_prompt",
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImageRefW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImagesResponseW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(default)]
    pub data: Vec<Ext<ImageW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImageW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

// `ImageGenUsage` and the streams' `ImagesUsage`; OpenRouter's `ImageGenerationUsage` names
// the totals `prompt_tokens` and `completion_tokens`.
#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct UsageW {
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
    #[serde(default)]
    pub input_tokens_details: Option<TokenDetailsW>,
    #[serde(default)]
    pub output_tokens_details: Option<TokenDetailsW>,
    #[serde(default)]
    pub completion_tokens_details: Option<TokenDetailsW>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct TokenDetailsW {
    #[serde(default)]
    pub image_tokens: Option<u64>,
    #[serde(default)]
    pub cached_tokens: Option<u64>,
}

// `ImageGenPartialImageEvent`, `ImageGenCompletedEvent` and their `image_edit.*` twins.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ImageEventW {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_image_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}
