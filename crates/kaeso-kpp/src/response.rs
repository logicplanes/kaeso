use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::block::Block;
use crate::dialect::Origin;
use crate::extras::Extras;
use crate::request::RequestKind;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reply {
    Generate(Response),
    CountTokens(CountTokensResponse),
    Embed(EmbedResponse),
}

impl Reply {
    pub fn kind(&self) -> RequestKind {
        match self {
            Reply::Generate(_) => RequestKind::Generate,
            Reply::CountTokens(_) => RequestKind::CountTokens,
            Reply::Embed(_) => RequestKind::Embed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Response {
    pub id: String,
    pub model: String,
    pub blocks: Vec<Block>,
    pub stop: StopReason,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl EmbedResponse {
    pub fn new(model: impl Into<String>, data: Vec<Embedding>) -> Self {
        EmbedResponse {
            model: model.into(),
            data,
            ..EmbedResponse::default()
        }
    }
}

impl Response {
    pub fn new(id: impl Into<String>, model: impl Into<String>, blocks: Vec<Block>) -> Self {
        Response {
            id: id.into(),
            model: model.into(),
            blocks,
            ..Response::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopReason {
    #[default]
    EndTurn,
    MaxTokens,
    StopSequence(String),
    ToolUse,
    ContentFilter,
    Refusal,
    PauseTurn,
    Other(String),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Usage {
    pub input_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_breakdown: Option<CacheWriteBreakdown>,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawUsage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RawUsage {
    pub origin: Origin,
    pub value: Value,
}

impl RawUsage {
    pub fn new(origin: impl Into<Origin>, value: Value) -> Self {
        RawUsage {
            origin: origin.into(),
            value,
        }
    }
}

impl Usage {
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Usage {
            input_tokens,
            output_tokens,
            ..Usage::default()
        }
    }

    pub fn raw_for(&self, dialect: crate::DialectId) -> Option<&Value> {
        self.raw
            .as_ref()
            .filter(|r| r.origin.is(dialect))
            .map(|r| &r.value)
    }

    pub fn total_input_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_write_tokens)
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens().saturating_add(self.output_tokens)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CacheWriteBreakdown {
    pub ephemeral_5m: u64,
    pub ephemeral_1h: u64,
}

impl CacheWriteBreakdown {
    pub fn new(ephemeral_5m: u64, ephemeral_1h: u64) -> Self {
        CacheWriteBreakdown {
            ephemeral_5m,
            ephemeral_1h,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CountTokensResponse {
    pub input_tokens: u64,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl CountTokensResponse {
    pub fn new(input_tokens: u64) -> Self {
        CountTokensResponse {
            input_tokens,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedResponse {
    pub model: String,
    pub data: Vec<Embedding>,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Embedding {
    pub index: u32,
    pub vector: EmbeddingVector,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Embedding {
    pub fn new(index: u32, vector: EmbeddingVector) -> Self {
        Embedding {
            index,
            vector,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmbeddingVector {
    Float(Vec<f64>),
    Base64(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_totals_follow_d1() {
        let u = Usage {
            input_tokens: 10,
            cache_read_tokens: 100,
            cache_write_tokens: 5,
            output_tokens: 7,
            ..Usage::default()
        };
        assert_eq!(u.total_input_tokens(), 115);
        assert_eq!(u.total_tokens(), 122);
    }

    #[test]
    fn stop_reason_stable_form() {
        assert_eq!(
            serde_json::to_string(&StopReason::EndTurn).unwrap(),
            "\"end_turn\""
        );
        assert_eq!(
            serde_json::to_string(&StopReason::StopSequence("END".into())).unwrap(),
            r#"{"stop_sequence":"END"}"#
        );
    }
}
