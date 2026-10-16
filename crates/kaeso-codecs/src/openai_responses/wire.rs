use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::wire::StringOr;

#[derive(Debug, Deserialize)]
pub(super) struct CreateResponse {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input: Option<StringOr<Box<RawValue>>>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<Box<RawValue>>>,
    #[serde(default)]
    pub tool_choice: Option<Box<RawValue>>,
    #[serde(default)]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default)]
    pub text: Option<TextParam>,
    #[serde(default)]
    pub reasoning: Option<ReasoningParam>,
    #[serde(default)]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub safety_identifier: Option<String>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub store: Option<bool>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub conversation: Option<Box<RawValue>>,
    #[serde(default)]
    pub background: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TextParam {
    #[serde(default)]
    pub format: Option<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JsonSchemaFormat {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub schema: Box<RawValue>,
    #[serde(default)]
    pub strict: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReasoningParam {
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FunctionToolWire {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<Box<RawValue>>,
    #[serde(default)]
    pub strict: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct MessageItem {
    pub role: String,
    pub content: StringOr<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TextPart {
    pub text: String,
    #[serde(default)]
    pub annotations: Option<Vec<Box<RawValue>>>,
    #[serde(default)]
    pub prompt_cache_breakpoint: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RefusalPart {
    pub refusal: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImagePart {
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub prompt_cache_breakpoint: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FilePart {
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub file_data: Option<String>,
    #[serde(default)]
    pub file_url: Option<String>,
    #[serde(default)]
    pub prompt_cache_breakpoint: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FunctionCallItem {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct FunctionCallOutputItem {
    pub call_id: String,
    pub output: StringOr<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReasoningItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub encrypted_content: Option<String>,
    #[serde(default)]
    pub summary: Option<Vec<TextOnly>>,
    #[serde(default)]
    pub content: Option<Vec<TextOnly>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TextOnly {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ResponseObject {
    pub id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub error: Option<ResponseError>,
    #[serde(default)]
    pub incomplete_details: Option<IncompleteDetails>,
    #[serde(default)]
    pub output: Option<Vec<Box<RawValue>>>,
    #[serde(default)]
    pub usage: Option<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ResponseError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IncompleteDetails {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UsageWire {
    pub input_tokens: u64,
    #[serde(default)]
    pub input_tokens_details: Option<InputDetails>,
    pub output_tokens: u64,
    #[serde(default)]
    pub output_tokens_details: Option<OutputDetails>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InputDetails {
    #[serde(default)]
    pub cached_tokens: Option<u64>,
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OutputDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenCounts {
    pub input_tokens: u64,
}
