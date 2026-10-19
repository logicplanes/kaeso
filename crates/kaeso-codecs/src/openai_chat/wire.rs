use kaeso_kpp::RawJson;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;

use crate::wire::{Ext, StringOr};

#[cfg(test)]
pub(crate) const PASSTHROUGH_REQUEST_FIELDS: &[&str] = &[
    "audio",
    "function_call",
    "functions",
    "logit_bias",
    "logprobs",
    "metadata",
    "modalities",
    "moderation",
    "prediction",
    "prompt_cache_key",
    "prompt_cache_options",
    "prompt_cache_retention",
    "service_tier",
    "store",
    "top_logprobs",
    "verbosity",
    "web_search_options",
];

pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(crate) fn role_tag(raw: &RawValue) -> Result<Option<String>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Tag {
        #[serde(default)]
        role: Option<String>,
    }
    if !raw.get().trim_start().starts_with('{') {
        return Ok(None);
    }
    Ok(serde_json::from_str::<Tag>(raw.get())?.role)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChatRequest {
    pub(crate) model: String,
    pub(crate) messages: Vec<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_completion_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stop: Option<StringOr<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frequency_penalty: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) presence_penalty: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) n: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<RawJson>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) tools: Vec<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stream_options: Option<Ext<StreamOptions>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct StreamOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) include_usage: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct InstructionMessage {
    pub(crate) role: String,
    pub(crate) content: StringOr<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct UserMessage {
    pub(crate) role: String,
    pub(crate) content: StringOr<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct AssistantMessage {
    pub(crate) role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<StringOr<RawJson>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refusal: Option<String>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) tool_calls: Vec<RawJson>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) annotations: Vec<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ToolMessage {
    pub(crate) role: String,
    pub(crate) content: StringOr<RawJson>,
    pub(crate) tool_call_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextPart {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_breakpoint: Option<Ext<CacheBreakpoint>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct RefusalPart {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) refusal: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ImagePart {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) image_url: Ext<ImageUrl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_breakpoint: Option<Ext<CacheBreakpoint>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ImageUrl {
    pub(crate) url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FilePart {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) file: Ext<FileRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_breakpoint: Option<Ext<CacheBreakpoint>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FileRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CacheBreakpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolCall {
    pub(crate) id: String,
    #[serde(rename = "type", default, skip_serializing_if = "String::is_empty")]
    pub(crate) ty: String,
    pub(crate) function: Ext<FunctionCall>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionToolWire {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) function: Ext<FunctionObject>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionObject {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parameters: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) strict: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NamedToolChoice {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) function: Ext<NamedFunction>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NamedFunction {
    pub(crate) name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ResponseFormat {
    #[serde(rename = "type")]
    pub(crate) ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) json_schema: Option<Ext<JsonSchemaWire>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct JsonSchemaWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) schema: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) strict: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChatResponse {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) object: Option<String>,
    pub(crate) model: String,
    pub(crate) choices: Vec<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usage: Option<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Choice {
    pub(crate) index: u64,
    pub(crate) message: Ext<ResponseMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ResponseMessage {
    pub(crate) role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refusal: Option<String>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) tool_calls: Vec<RawJson>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) annotations: Vec<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct UsageWire {
    #[serde(default)]
    pub(crate) prompt_tokens: u64,
    #[serde(default)]
    pub(crate) completion_tokens: u64,
    #[serde(default)]
    pub(crate) total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) completion_tokens_details: Option<CompletionTokensDetails>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct PromptTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cached_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_write_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CompletionTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChunkWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) choices: Option<Vec<RawJson>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usage: Option<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChunkChoice {
    pub(crate) index: u64,
    #[serde(default)]
    pub(crate) delta: Ext<DeltaWire>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct DeltaWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refusal: Option<String>,
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(crate) tool_calls: Vec<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ToolCallChunk {
    pub(crate) index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(crate) ty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) function: Option<Ext<FunctionChunk>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionChunk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) arguments: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: Ext<ErrorBody>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ErrorBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(crate) ty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) param: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<serde_json::Value>,
}
