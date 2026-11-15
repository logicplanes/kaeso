use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};

use crate::wire::Ext;

/// The body of Jina AI's `POST /v1/rerank`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankRequestWire {
    pub model: String,
    pub query: String,
    /// Strings, or `{"text"}` objects.
    pub documents: Vec<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    /// True when left out, unlike Cohere's and Voyage AI's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
    /// `jina-reranker-v3` and `jina-reranker-v3.5` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_doc_length: Option<u32>,
}

/// A document written as an object.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct DocumentWire {
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankResponseWire {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<RawJson>,
    pub results: Vec<Ext<ResultWire>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResultWire {
    pub index: u32,
    pub relevance_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<RawJson>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UsageWire {
    pub total_tokens: u64,
}
