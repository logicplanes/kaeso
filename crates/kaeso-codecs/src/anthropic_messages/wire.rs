use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::wire::{Ext, Maybe, StringOr};

pub(crate) type Raw = Box<RawValue>;

pub(crate) fn is_not_true(b: &Option<bool>) -> bool {
    *b != Some(true)
}

pub(crate) fn is_none_or_empty<T>(v: &Option<Vec<T>>) -> bool {
    v.as_ref().is_none_or(Vec::is_empty)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct RequestW {
    pub(crate) model: String,
    pub(crate) messages: Vec<Ext<MessageW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) system: Option<StringOr<Raw>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<Ext<MetadataW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) output_config: Option<Ext<OutputConfigW>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) stop_sequences: Vec<String>,
    #[serde(default, skip_serializing_if = "is_not_true")]
    pub(crate) stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thinking: Option<Ext<ThinkingConfigW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<Ext<ToolChoiceW>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MessageW {
    pub(crate) role: String,
    pub(crate) content: StringOr<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MetadataW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) user_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct OutputConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct JsonFormatW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) schema: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ThinkingConfigW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) budget_tokens: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolChoiceW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) disable_parallel_tool_use: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolW {
    #[serde(rename = "type", default, skip_serializing)]
    pub(crate) _ty: Option<Raw>,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    pub(crate) input_schema: Raw,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) strict: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CacheControlW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ttl: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
    #[serde(default, skip_serializing_if = "is_none_or_empty")]
    pub(crate) citations: Option<Vec<Raw>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ImageW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) source: Raw,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct DocumentW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) source: Raw,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) citations: Option<Ext<CitationsConfigW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CitationsConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enabled: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct DataSourceW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) media_type: String,
    pub(crate) data: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct UrlSourceW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FileSourceW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) file_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolUseW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) input: Raw,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolResultW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) tool_use_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<StringOr<Raw>>,
    #[serde(default, skip_serializing_if = "is_not_true")]
    pub(crate) is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_control: Option<Ext<CacheControlW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ThinkingW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) thinking: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) signature: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct RedactedThinkingW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) data: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MessageRespW {
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) content: Vec<Raw>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub(crate) stop_reason: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub(crate) stop_sequence: Maybe<String>,
    pub(crate) usage: Raw,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct UsageW {
    #[serde(default)]
    pub(crate) input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) cache_creation: Option<CacheCreationW>,
    #[serde(default)]
    pub(crate) output_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) output_tokens_details: Option<OutputTokensDetailsW>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct CacheCreationW {
    #[serde(default)]
    pub(crate) ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    pub(crate) ephemeral_1h_input_tokens: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct OutputTokensDetailsW {
    #[serde(default)]
    pub(crate) thinking_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CountTokensRespW {
    pub(crate) input_tokens: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ErrorResponseW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) error: ErrorObjectW,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) request_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ErrorObjectW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    #[serde(default)]
    pub(crate) message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MessageStartW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) message: Ext<MessageStartMessageW>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MessageStartMessageW {
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) usage: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct BlockStartW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) index: u32,
    pub(crate) content_block: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct BlockDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) index: u32,
    pub(crate) delta: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct InputJsonDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) partial_json: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ThinkingDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) thinking: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SignatureDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) signature: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CitationsDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) citation: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct BlockStopW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) index: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MessageDeltaW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) delta: Ext<MessageDeltaDeltaW>,
    pub(crate) usage: Raw,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct MessageDeltaDeltaW {
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub(crate) stop_reason: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub(crate) stop_sequence: Maybe<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct TypeOnlyW {
    #[serde(rename = "type")]
    pub(crate) ty: String,
}
