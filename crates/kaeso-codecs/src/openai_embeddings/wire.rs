use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};

use crate::wire::Ext;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct CreateEmbeddingRequest {
    pub input: RawJson,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct CreateEmbeddingResponse {
    pub object: String,
    pub data: Vec<Ext<EmbeddingWire>>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct EmbeddingWire {
    pub object: String,
    pub index: u32,
    pub embedding: RawJson,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UsageWire {
    pub prompt_tokens: u64,
}
