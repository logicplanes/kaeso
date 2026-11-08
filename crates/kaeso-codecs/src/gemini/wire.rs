use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::wire::Ext;

pub(crate) type Raw = Box<RawValue>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) contents: Vec<Ext<ContentW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) system_instruction: Option<Ext<ContentW>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_config: Option<Ext<ToolConfigW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generation_config: Option<Ext<GenerationConfigW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CountTokensW {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) contents: Vec<Ext<ContentW>>,
    #[serde(
        rename = "generateContentRequest",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) generate_content_request: Option<Ext<RequestW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ContentW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) parts: Vec<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PartW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thought: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thought_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inline_data: Option<Ext<BlobW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file_data: Option<Ext<FileDataW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) function_call: Option<Ext<FunctionCallW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) function_response: Option<Ext<FunctionResponseW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlobW {
    pub(crate) mime_type: String,
    pub(crate) data: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileDataW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mime_type: Option<String>,
    pub(crate) file_uri: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionCallW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) args: Option<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FunctionResponseW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response: Option<Raw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) parts: Vec<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResponsePartW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inline_data: Option<Ext<BlobW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file_data: Option<Ext<FileDataW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolW {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) function_declarations: Vec<Ext<FunctionDeclarationW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionDeclarationW {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parameters: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parameters_json_schema: Option<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) function_calling_config: Option<Ext<FunctionCallingConfigW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionCallingConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) allowed_function_names: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerationConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_k: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stop_sequences: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) presence_penalty: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frequency_penalty: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_schema: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_json_schema: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thinking_config: Option<Ext<ThinkingConfigW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_modalities: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) speech_config: Option<Ext<SpeechConfigW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<Raw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeechConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) voice_config: Option<Ext<VoiceConfigW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prebuilt_voice_config: Option<Ext<PrebuiltVoiceW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) voice: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrebuiltVoiceW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) voice_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThinkingConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) include_thoughts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thinking_budget: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thinking_level: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResponseW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidates: Option<Vec<Ext<CandidateW>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usage_metadata: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<Ext<ContentW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageW {
    #[serde(default)]
    pub(crate) prompt_token_count: u64,
    #[serde(default)]
    pub(crate) cached_content_token_count: u64,
    #[serde(default)]
    pub(crate) tool_use_prompt_token_count: u64,
    #[serde(default)]
    pub(crate) candidates_token_count: u64,
    #[serde(default)]
    pub(crate) thoughts_token_count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountTokensResponseW {
    #[serde(default)]
    pub(crate) total_tokens: u64,
}
