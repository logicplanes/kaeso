//! Rerank requests: a query and the documents to order by how well each answers it.

use serde::{Deserialize, Serialize};

use crate::error::KppError;
use crate::extras::Extras;
use crate::response::Usage;

/// A query and the documents a rerank model scores against it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RerankRequest {
    pub model: String,
    pub query: String,
    pub documents: Vec<RerankDocument>,
    /// How many of the best documents the reply lists; every document when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    /// Whether each result carries its document, as the app said it; `None` when it didn't
    /// say, which means no documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
    /// The tokens of each document the model reads; the provider cuts off the rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_doc: Option<u32>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl RerankRequest {
    pub fn new(
        model: impl Into<String>,
        query: impl Into<String>,
        documents: Vec<RerankDocument>,
    ) -> Self {
        RerankRequest {
            model: model.into(),
            query: query.into(),
            documents,
            ..RerankRequest::default()
        }
    }

    #[must_use]
    pub fn with_top_n(mut self, top_n: u32) -> Self {
        self.top_n = Some(top_n);
        self
    }

    #[must_use]
    pub fn with_return_documents(mut self, return_documents: bool) -> Self {
        self.return_documents = Some(return_documents);
        self
    }

    #[must_use]
    pub fn with_max_tokens_per_doc(mut self, tokens: u32) -> Self {
        self.max_tokens_per_doc = Some(tokens);
        self
    }

    /// Whether the app asked for each result's document.
    pub fn returns_documents(&self) -> bool {
        self.return_documents == Some(true)
    }

    pub fn validate(&self) -> Result<(), KppError> {
        let invalid =
            |message: &str, param: &str| Err(KppError::invalid_request(message).with_param(param));
        if self.model.is_empty() {
            return invalid("`model` must not be empty.", "model");
        }
        if self.documents.is_empty() {
            return invalid("`documents` must not be empty.", "documents");
        }
        if self.top_n == Some(0) {
            return invalid("`top_n` must be at least 1.", "top_n");
        }
        if self.max_tokens_per_doc == Some(0) {
            return invalid(
                "`max_tokens_per_doc` must be at least 1.",
                "max_tokens_per_doc",
            );
        }
        Ok(())
    }
}

/// One document to score: its text, and what else the app sent with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RerankDocument {
    pub text: String,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl RerankDocument {
    pub fn new(text: impl Into<String>) -> Self {
        RerankDocument {
            text: text.into(),
            extras: Extras::default(),
        }
    }
}

/// The documents a rerank model scored, best first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RerankResponse {
    /// The provider's id for the reply, when it sends one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub model: String,
    pub results: Vec<RerankResult>,
    /// The tokens the provider billed, as `input_tokens`.
    pub usage: Usage,
    /// The search units the provider billed, for providers that bill searches instead of
    /// tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_units: Option<u64>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl RerankResponse {
    pub fn new(model: impl Into<String>, results: Vec<RerankResult>) -> Self {
        RerankResponse {
            model: model.into(),
            results,
            ..RerankResponse::default()
        }
    }
}

/// One scored document: its position in the request, its score as the provider gave it, and
/// the document when the app asked for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RerankResult {
    pub index: u32,
    pub relevance_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<RerankDocument>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl RerankResult {
    pub fn new(index: u32, relevance_score: f64) -> Self {
        RerankResult {
            index,
            relevance_score,
            document: None,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_document(mut self, document: RerankDocument) -> Self {
        self.document = Some(document);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_needs_a_model_documents_and_positive_counts() {
        let docs = || vec![RerankDocument::new("a")];
        assert!(RerankRequest::new("m", "q", docs()).validate().is_ok());
        let err = RerankRequest::new("", "q", docs()).validate().unwrap_err();
        assert_eq!(err.param.as_deref(), Some("model"));
        let err = RerankRequest::new("m", "q", vec![]).validate().unwrap_err();
        assert_eq!(err.param.as_deref(), Some("documents"));
        let err = RerankRequest::new("m", "q", docs())
            .with_top_n(0)
            .validate()
            .unwrap_err();
        assert_eq!(err.param.as_deref(), Some("top_n"));
        let err = RerankRequest::new("m", "q", docs())
            .with_max_tokens_per_doc(0)
            .validate()
            .unwrap_err();
        assert_eq!(err.param.as_deref(), Some("max_tokens_per_doc"));
    }

    #[test]
    fn documents_come_back_only_when_asked() {
        let r = RerankRequest::new("m", "q", vec![RerankDocument::new("a")]);
        assert!(!r.returns_documents());
        assert!(!r.clone().with_return_documents(false).returns_documents());
        assert!(r.with_return_documents(true).returns_documents());
    }

    #[test]
    fn the_json_form_is_small_and_stable() {
        let r = RerankRequest::new("m", "q", vec![RerankDocument::new("a")]).with_top_n(1);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(
            json,
            r#"{"model":"m","query":"q","documents":[{"text":"a"}],"top_n":1}"#
        );
        assert_eq!(serde_json::from_str::<RerankRequest>(&json).unwrap(), r);
        let mut reply = RerankResponse::new(
            "m",
            vec![RerankResult::new(0, 0.5).with_document(RerankDocument::new("a"))],
        );
        reply.usage = Usage::new(7, 0);
        let json = serde_json::to_string(&reply).unwrap();
        assert!(
            json.starts_with(r#"{"model":"m","results":[{"index":0,"relevance_score":0.5,"document":{"text":"a"}}]"#),
            "{json}"
        );
        assert_eq!(
            serde_json::from_str::<RerankResponse>(&json).unwrap(),
            reply
        );
    }
}
