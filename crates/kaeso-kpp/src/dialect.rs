use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DialectId {
    #[serde(rename = "anthropic_messages")]
    AnthropicMessages,
    #[serde(rename = "openai_chat")]
    OpenAiChat,
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "openai_embeddings")]
    OpenAiEmbeddings,
}

impl DialectId {
    pub const ALL: [DialectId; 4] = [
        DialectId::AnthropicMessages,
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
        DialectId::OpenAiEmbeddings,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            DialectId::AnthropicMessages => "anthropic_messages",
            DialectId::OpenAiChat => "openai_chat",
            DialectId::OpenAiResponses => "openai_responses",
            DialectId::OpenAiEmbeddings => "openai_embeddings",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            DialectId::AnthropicMessages => "Anthropic Messages",
            DialectId::OpenAiChat => "OpenAI Chat Completions",
            DialectId::OpenAiResponses => "OpenAI Responses",
            DialectId::OpenAiEmbeddings => "OpenAI Embeddings",
        }
    }
}

impl fmt::Display for DialectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown dialect id `{0}`")]
pub struct UnknownDialect(String);

impl UnknownDialect {
    pub fn id(&self) -> &str {
        &self.0
    }
}

impl FromStr for DialectId {
    type Err = UnknownDialect;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DialectId::ALL
            .into_iter()
            .find(|d| d.as_str() == s)
            .ok_or_else(|| UnknownDialect(s.to_owned()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Origin {
    pub dialect: DialectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl Origin {
    pub const fn new(dialect: DialectId) -> Self {
        Origin {
            dialect,
            provider: None,
        }
    }

    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn is(&self, dialect: DialectId) -> bool {
        self.dialect == dialect
    }
}

impl From<DialectId> for Origin {
    fn from(dialect: DialectId) -> Self {
        Origin::new(dialect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_strings_and_serde() {
        for d in DialectId::ALL {
            assert_eq!(d.as_str().parse::<DialectId>().unwrap(), d);
            let json = serde_json::to_string(&d).unwrap();
            assert_eq!(json, format!("\"{}\"", d.as_str()));
            assert_eq!(serde_json::from_str::<DialectId>(&json).unwrap(), d);
        }
        let err = "openai".parse::<DialectId>().unwrap_err();
        assert_eq!(err.id(), "openai");
        assert_eq!(err.to_string(), "unknown dialect id `openai`");
    }

    #[test]
    fn origin_omits_missing_provider() {
        let o = Origin::new(DialectId::OpenAiChat);
        assert_eq!(
            serde_json::to_string(&o).unwrap(),
            r#"{"dialect":"openai_chat"}"#
        );
        let o = o.with_provider("openai");
        assert_eq!(
            serde_json::to_string(&o).unwrap(),
            r#"{"dialect":"openai_chat","provider":"openai"}"#
        );
    }
}
