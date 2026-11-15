use kaeso_kpp::RawJson;
use serde::{Deserialize, Serialize};

use crate::wire::Ext;

/// The body of `POST /v2/rerank`, and the v1 fields Kaeso reads as well.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankRequestWire {
    pub model: String,
    pub query: String,
    /// Strings, or v1's `{"text"}` objects.
    pub documents: Vec<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_doc: Option<u32>,
    /// Cohere v1's switch; v2 has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
}

/// A document written as an object, as v1 takes it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct DocumentWire {
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RerankResponseWire {
    pub results: Vec<Ext<ResultWire>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<RawJson>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ResultWire {
    pub index: u32,
    pub relevance_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<RawJson>,
}

/// What Kaeso reads of `meta`: the units Cohere billed.
#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct MetaWire {
    #[serde(default)]
    pub billed_units: Option<BilledUnitsWire>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct BilledUnitsWire {
    #[serde(default)]
    pub input_tokens: Option<f64>,
    #[serde(default)]
    pub search_units: Option<f64>,
}
