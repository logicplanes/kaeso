use serde::{Deserialize, Serialize};

use crate::block::{Citation, Opaque, RawItem};
use crate::error::KppError;
use crate::extras::Extras;
use crate::response::{StopReason, Usage};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StreamEvent {
    ResponseStart(ResponseStart),
    BlockStart(BlockStart),
    BlockDelta(BlockDelta),
    BlockStop(BlockStop),
    ResponseDelta(ResponseDelta),
    ResponseStop(ResponseStop),
    Error(KppError),
    Extra(RawItem),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseStart {
    pub id: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ResponseStart {
    pub fn new(id: impl Into<String>, model: impl Into<String>) -> Self {
        ResponseStart {
            id: id.into(),
            model: model.into(),
            ..ResponseStart::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlockStart {
    pub index: u32,
    pub block: BlockHeader,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl BlockStart {
    pub fn new(index: u32, block: BlockHeader) -> Self {
        BlockStart {
            index,
            block,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlockDelta {
    pub index: u32,
    pub delta: Delta,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl BlockDelta {
    pub fn new(index: u32, delta: Delta) -> Self {
        BlockDelta {
            index,
            delta,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlockStop {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl BlockStop {
    pub fn new(index: u32) -> Self {
        BlockStop {
            index,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ResponseDelta {
    pub fn new(stop: Option<StopReason>, usage: Option<Usage>) -> Self {
        ResponseDelta {
            stop,
            usage,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseStop {
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ResponseStop {
    pub fn new() -> Self {
        ResponseStop::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BlockHeader {
    Text,
    ToolCall(ToolCallHeader),
    Reasoning(ReasoningHeader),
    Refusal,
    Unknown(RawItem),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolCallHeader {
    pub id: String,
    pub name: String,
}

impl ToolCallHeader {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        ToolCallHeader {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReasoningHeader {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::Origin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opaque: Vec<Opaque>,
}

impl ReasoningHeader {
    pub fn new(origin: impl Into<crate::Origin>) -> Self {
        ReasoningHeader {
            origin: Some(origin.into()),
            opaque: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Delta {
    Text(String),
    ToolArguments(String),
    ReasoningText(String),
    ReasoningSummary(SummaryDelta),
    Opaque(Opaque),
    Citation(Citation),
    Refusal(String),
    Unknown(RawItem),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryDelta {
    pub summary_index: u32,
    pub text: String,
}

impl SummaryDelta {
    pub fn new(summary_index: u32, text: impl Into<String>) -> Self {
        SummaryDelta {
            summary_index,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DialectId;

    #[test]
    fn stable_json_form_round_trips() {
        let events = vec![
            StreamEvent::ResponseStart(ResponseStart::new("msg_1", "m")),
            StreamEvent::BlockStart(BlockStart::new(
                0,
                BlockHeader::ToolCall(ToolCallHeader::new("c", "f")),
            )),
            StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ToolArguments("{\"a\"".into()))),
            StreamEvent::BlockDelta(BlockDelta::new(
                1,
                Delta::ReasoningSummary(SummaryDelta::new(0, "x")),
            )),
            StreamEvent::BlockStart(BlockStart::new(
                2,
                BlockHeader::Reasoning(ReasoningHeader::new(DialectId::AnthropicMessages)),
            )),
            StreamEvent::BlockStop(BlockStop::new(0)),
            StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::ToolUse), None)),
            StreamEvent::ResponseStop(ResponseStop::new()),
        ];
        let json = serde_json::to_string(&events).unwrap();
        let back: Vec<StreamEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, events);
    }
}
