use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::b64::Base64Text;
use crate::dialect::{DialectId, Origin};
use crate::error::KppError;
use crate::extras::Extras;
use crate::raw::RawJson;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Block {
    Text(TextBlock),
    Image(ImageBlock),
    Document(DocumentBlock),
    Audio(AudioBlock),
    Video(VideoBlock),
    ToolCall(ToolCallBlock),
    ToolResult(ToolResultBlock),
    Reasoning(ReasoningBlock),
    Refusal(RefusalBlock),
    Unknown(RawItem),
}

impl Block {
    pub fn text(text: impl Into<String>) -> Self {
        Block::Text(TextBlock::new(text))
    }

    pub fn tool_call(id: impl Into<String>, name: impl Into<String>, arguments: RawJson) -> Self {
        Block::ToolCall(ToolCallBlock::new(id, name, arguments))
    }

    pub fn tool_result(call_id: impl Into<String>, content: Vec<Block>, is_error: bool) -> Self {
        let mut result = ToolResultBlock::new(call_id, content);
        result.is_error = is_error;
        Block::ToolResult(result)
    }

    pub fn image(source: Source) -> Self {
        Block::Image(ImageBlock::new(source))
    }

    pub fn audio(source: Source) -> Self {
        Block::Audio(AudioBlock::new(source))
    }

    pub fn video(source: Source) -> Self {
        Block::Video(VideoBlock::new(source))
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Block::Text(_) => "text",
            Block::Image(_) => "image",
            Block::Document(_) => "document",
            Block::Audio(_) => "audio",
            Block::Video(_) => "video",
            Block::ToolCall(_) => "tool_call",
            Block::ToolResult(_) => "tool_result",
            Block::Reasoning(_) => "reasoning",
            Block::Refusal(_) => "refusal",
            Block::Unknown(_) => "unknown",
        }
    }

    pub fn extras(&self) -> Option<&Extras> {
        match self {
            Block::Text(b) => Some(&b.extras),
            Block::Image(b) => Some(&b.extras),
            Block::Document(b) => Some(&b.extras),
            Block::Audio(b) => Some(&b.extras),
            Block::Video(b) => Some(&b.extras),
            Block::ToolCall(b) => Some(&b.extras),
            Block::ToolResult(b) => Some(&b.extras),
            Block::Reasoning(b) => Some(&b.extras),
            Block::Refusal(b) => Some(&b.extras),
            Block::Unknown(_) => None,
        }
    }

    pub fn cache(&self) -> Option<&CacheHint> {
        match self {
            Block::Text(b) => b.cache.as_ref(),
            Block::Image(b) => b.cache.as_ref(),
            Block::Document(b) => b.cache.as_ref(),
            Block::Audio(b) => b.cache.as_ref(),
            Block::Video(b) => b.cache.as_ref(),
            Block::ToolCall(b) => b.cache.as_ref(),
            Block::ToolResult(b) => b.cache.as_ref(),
            Block::Reasoning(_) | Block::Refusal(_) | Block::Unknown(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextBlock {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Citation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl TextBlock {
    pub fn new(text: impl Into<String>) -> Self {
        TextBlock {
            text: text.into(),
            ..TextBlock::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageBlock {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ImageBlock {
    pub fn new(source: Source) -> Self {
        ImageBlock {
            source,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AudioBlock {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl AudioBlock {
    pub fn new(source: Source) -> Self {
        AudioBlock {
            source,
            transcript: None,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoBlock {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl VideoBlock {
    pub fn new(source: Source) -> Self {
        VideoBlock {
            source,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DocumentBlock {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub citations: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl DocumentBlock {
    pub fn new(source: Source) -> Self {
        DocumentBlock {
            source,
            title: None,
            context: None,
            citations: false,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolCallBlock {
    pub id: String,
    pub name: String,
    pub arguments: RawJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ToolCallBlock {
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: RawJson) -> Self {
        ToolCallBlock {
            id: id.into(),
            name: name.into(),
            arguments,
            cache: None,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolResultBlock {
    pub call_id: String,
    pub content: Vec<Block>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ToolResultBlock {
    pub fn new(call_id: impl Into<String>, content: Vec<Block>) -> Self {
        ToolResultBlock {
            call_id: call_id.into(),
            content,
            ..ToolResultBlock::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReasoningBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opaque: Vec<Opaque>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ReasoningBlock {
    pub fn new(origin: impl Into<Origin>) -> Self {
        ReasoningBlock {
            origin: Some(origin.into()),
            ..ReasoningBlock::default()
        }
    }

    pub fn replayable_to(&self, dialect: DialectId) -> bool {
        self.origin.as_ref().is_some_and(|o| o.is(dialect))
    }

    pub fn opaque_of(&self, kind: OpaqueKind) -> Option<&Opaque> {
        self.opaque.iter().find(|o| o.kind == kind)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RefusalBlock {
    pub text: String,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl RefusalBlock {
    pub fn new(text: impl Into<String>) -> Self {
        RefusalBlock {
            text: text.into(),
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RawItem {
    pub origin: Origin,
    pub raw: RawJson,
}

impl RawItem {
    pub fn new(origin: impl Into<Origin>, raw: RawJson) -> Self {
        RawItem {
            origin: origin.into(),
            raw,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Source {
    Url(String),
    Base64(Base64Source),
    Text(TextSource),
    FileId(FileIdSource),
}

impl Source {
    /// Media given as base64 text, decoded once. Padding is optional; text that is not standard
    /// base64 is an `InvalidRequest`.
    pub fn base64(media_type: impl Into<String>, data: impl AsRef<[u8]>) -> Result<Self, KppError> {
        Base64Source::from_base64(media_type, data).map(Source::Base64)
    }

    /// Media given as its bytes. Cloning the source shares them.
    pub fn bytes(media_type: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Source::Base64(Base64Source::new(media_type, data))
    }

    pub fn text(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Source::Text(TextSource {
            media_type: media_type.into(),
            data: data.into(),
        })
    }

    pub fn file_id(origin: impl Into<Origin>, id: impl Into<String>) -> Self {
        Source::FileId(FileIdSource {
            origin: origin.into(),
            id: id.into(),
        })
    }

    pub fn media_type(&self) -> Option<&str> {
        match self {
            Source::Base64(s) => Some(&s.media_type),
            Source::Text(s) => Some(&s.media_type),
            Source::Url(_) | Source::FileId(_) => None,
        }
    }
}

/// Inline media: its bytes, held once and decoded, and their media type.
///
/// The bytes are a [`Bytes`] buffer, so cloning a request shares them instead of copying, and a
/// codec that read them from an upload can slice them out of the request body. KPP's JSON form
/// writes them as standard base64: `{"base64": {"media_type": "image/png", "data": "iVBOR…"}}`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Base64Source {
    pub media_type: String,
    #[serde(with = "crate::b64")]
    pub data: Bytes,
}

impl Base64Source {
    pub fn new(media_type: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Base64Source {
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    /// Decodes base64 text; padding is optional.
    pub fn from_base64(
        media_type: impl Into<String>,
        text: impl AsRef<[u8]>,
    ) -> Result<Self, KppError> {
        let data = crate::b64::decode(text.as_ref())
            .map_err(|_| KppError::invalid_request("The media data is not valid base64."))?;
        Ok(Base64Source::new(media_type, data))
    }

    /// The bytes as standard, padded base64, to format or serialize without building the text.
    pub fn base64(&self) -> Base64Text<'_> {
        Base64Text(&self.data)
    }

    /// The bytes as standard, padded base64 text.
    pub fn to_base64(&self) -> String {
        let mut out = String::new();
        self.push_base64(&mut out);
        out
    }

    /// Appends the bytes as standard, padded base64 to `out`, such as after a data URL's header.
    pub fn push_base64(&self, out: &mut String) {
        crate::b64::push(&self.data, out);
    }

    /// A `data:` URL of the bytes: `data:<media type>;base64,<data>`.
    pub fn data_url(&self) -> String {
        self.data_url_with(&format!("data:{};base64", self.media_type))
    }

    /// A `data:` URL of the bytes after `header`, the part before the comma as an app wrote it.
    pub fn data_url_with(&self, header: &str) -> String {
        let mut out = String::with_capacity(header.len() + 1 + self.base64().len());
        out.push_str(header);
        out.push(',');
        self.push_base64(&mut out);
        out
    }
}

impl std::fmt::Debug for Base64Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Base64Source")
            .field("media_type", &self.media_type)
            .field("data", &format_args!("{} bytes", self.data.len()))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextSource {
    pub media_type: String,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FileIdSource {
    pub origin: Origin,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Opaque {
    pub origin: Origin,
    pub kind: OpaqueKind,
    pub data: String,
}

impl Opaque {
    pub fn new(origin: impl Into<Origin>, kind: OpaqueKind, data: impl Into<String>) -> Self {
        Opaque {
            origin: origin.into(),
            kind,
            data: data.into(),
        }
    }

    pub fn replayable_to(&self, dialect: DialectId) -> bool {
        self.origin.is(dialect)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OpaqueKind {
    #[serde(rename = "anthropic_thinking_signature")]
    AnthropicThinkingSignature,
    #[serde(rename = "anthropic_redacted_thinking")]
    AnthropicRedactedThinking,
    #[serde(rename = "openai_encrypted_reasoning")]
    OpenAiEncryptedReasoning,
    #[serde(rename = "openai_reasoning_item_id")]
    OpenAiReasoningItemId,
    #[serde(rename = "gemini_thought_signature")]
    GeminiThoughtSignature,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CacheHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheTtl>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl CacheHint {
    pub fn new(ttl: Option<CacheTtl>) -> Self {
        CacheHint {
            ttl,
            extras: Extras::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CacheTtl {
    FiveMinutes,
    OneHour,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Citation {
    pub origin: Origin,
    pub raw: RawJson,
}

impl Citation {
    pub fn new(origin: impl Into<Origin>, raw: RawJson) -> Self {
        Citation {
            origin: origin.into(),
            raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_json_form_round_trips() {
        let mut reasoning = ReasoningBlock::new(DialectId::AnthropicMessages);
        reasoning.text.push("thinking".into());
        reasoning.opaque.push(Opaque::new(
            DialectId::AnthropicMessages,
            OpaqueKind::AnthropicThinkingSignature,
            "sig",
        ));
        let blocks = vec![
            Block::text("hi"),
            Block::tool_call(
                "toolu_01",
                "get_weather",
                RawJson::from_string("{\"city\": \"Paris\"}".into()).unwrap(),
            ),
            Block::tool_result("toolu_01", vec![Block::text("sunny")], false),
            Block::Reasoning(reasoning),
            Block::Unknown(RawItem::new(
                DialectId::AnthropicMessages,
                RawJson::from_string("{\"type\":\"server_tool_use\"}".into()).unwrap(),
            )),
            Block::image(Source::base64("image/png", "AAAA").unwrap()),
            Block::audio(Source::base64("audio/wav", "UklGRg==").unwrap()),
            Block::video(Source::Url(
                "https://www.youtube.com/watch?v=9hE5-98ZeCg".into(),
            )),
        ];
        let json = serde_json::to_string(&blocks).unwrap();
        let back: Vec<Block> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, blocks);
        assert!(
            json.contains("{\"city\": \"Paris\"}"),
            "raw bytes kept: {json}"
        );
        assert!(json.starts_with(r#"[{"text":{"text":"hi"}}"#), "{json}");
    }

    #[test]
    fn reasoning_and_opaque_are_bound_to_origin() {
        let o = Opaque::new(
            DialectId::OpenAiResponses,
            OpaqueKind::OpenAiEncryptedReasoning,
            "gAAAA",
        );
        assert!(o.replayable_to(DialectId::OpenAiResponses));
        assert!(!o.replayable_to(DialectId::OpenAiChat));
        let mut r = ReasoningBlock::new(DialectId::OpenAiResponses);
        r.opaque.push(o);
        assert!(r.replayable_to(DialectId::OpenAiResponses));
        assert!(!r.replayable_to(DialectId::AnthropicMessages));
        assert!(r.opaque_of(OpaqueKind::OpenAiEncryptedReasoning).is_some());
        assert!(!ReasoningBlock::default().replayable_to(DialectId::OpenAiChat));
    }

    #[test]
    fn opaque_kinds_spell_providers_like_dialect_ids() {
        let kinds = [
            (
                OpaqueKind::AnthropicThinkingSignature,
                "anthropic_thinking_signature",
            ),
            (
                OpaqueKind::AnthropicRedactedThinking,
                "anthropic_redacted_thinking",
            ),
            (
                OpaqueKind::OpenAiEncryptedReasoning,
                "openai_encrypted_reasoning",
            ),
            (
                OpaqueKind::OpenAiReasoningItemId,
                "openai_reasoning_item_id",
            ),
            (
                OpaqueKind::GeminiThoughtSignature,
                "gemini_thought_signature",
            ),
        ];
        for (kind, name) in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<OpaqueKind>(&json).unwrap(), kind);
        }
        assert!(serde_json::from_str::<OpaqueKind>("\"open_ai_encrypted_reasoning\"").is_err());
    }
}
