use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};

use crate::wire::Ext;

/// The body of Voyage AI's `POST /v1/rerank`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankRequestWire {
    pub query: String,
    pub documents: Vec<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankResponseWire {
    pub object: String,
    pub data: Vec<Ext<ResultWire>>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResultWire {
    pub index: u32,
    pub relevance_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UsageWire {
    pub total_tokens: u64,
}
