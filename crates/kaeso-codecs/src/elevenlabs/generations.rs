//! ElevenLabs' media generations as apps and ElevenLabs report them: the
//! `MediaGenerationCreateResponse` a create call answers, the `MediaGenerationResponse` of a
//! status call (pending, generating, completed with its link, or failed with its reason), the
//! `MediaGenerationListResponse` of a listing and the query of `GET /v1/flows/image`
//! (`list_image_generations`), in ElevenLabs' OpenAPI document, read 2026-11-10.

use kaeso_kpp::KppError;
use serde_json::{Map, Value};

use super::D;
use crate::codec::HeaderList;
use crate::video_jobs::{body, provider_object, put};

/// How many generations a listing returns when its query names no `page_size`.
pub const GENERATION_PAGE_SIZE: u32 = 30;

const MAX_PAGE_SIZE: u32 = 100;

/// How far a generation is; it ends as `completed` or `failed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GenerationStatus {
    /// Queued, as every new generation is.
    Pending,
    /// Being made.
    Generating,
    /// Made; its link is ready.
    Completed,
    /// Ended without media.
    Failed,
}

impl GenerationStatus {
    /// The status as ElevenLabs writes it, or `None` for one it doesn't document.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => GenerationStatus::Pending,
            "generating" => GenerationStatus::Generating,
            "completed" => GenerationStatus::Completed,
            "failed" => GenerationStatus::Failed,
            _ => return None,
        })
    }

    /// The status as ElevenLabs writes it: `pending`, `generating`, `completed` or `failed`.
    pub const fn as_wire(self) -> &'static str {
        match self {
            GenerationStatus::Pending => "pending",
            GenerationStatus::Generating => "generating",
            GenerationStatus::Completed => "completed",
            GenerationStatus::Failed => "failed",
        }
    }

    /// Whether the generation has ended.
    pub const fn is_done(self) -> bool {
        matches!(self, GenerationStatus::Completed | GenerationStatus::Failed)
    }
}

/// One generation: its id, how far it is, the link to what it made once it is completed and
/// why it failed once it has failed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Generation {
    pub id: String,
    pub status: GenerationStatus,
    /// A signed link to the made media, which ElevenLabs keeps valid for about an hour after it
    /// answered.
    pub content_url: Option<String>,
    pub content_mime_type: Option<String>,
    /// Why it failed: `timeout`, `model_error`, `moderated`, `invalid_parameters`,
    /// `dependency_failed`, `charging_failed` or `internal_error`.
    pub failure_reason: Option<String>,
    pub error_message: Option<String>,
}

impl Generation {
    pub fn new(id: impl Into<String>, status: GenerationStatus) -> Self {
        Generation {
            id: id.into(),
            status,
            content_url: None,
            content_mime_type: None,
            failure_reason: None,
            error_message: None,
        }
    }
}

/// Reads ElevenLabs' answer to the create or the status call of a generation, or the error it
/// sent instead.
pub fn decode_generation(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> Result<Generation, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(status, headers, body));
    }
    let mut f = provider_object(body, D)?;
    let id = f.str("id")?.unwrap_or_default();
    if id.is_empty() {
        return Err(KppError::provider(
            "ElevenLabs answered for a generation without naming it.",
        ));
    }
    let text = f.str("status")?.unwrap_or_default();
    let status = GenerationStatus::from_wire(&text).ok_or_else(|| {
        KppError::provider(format!(
            "ElevenLabs sent the unknown generation status `{text}`."
        ))
    })?;
    let mut g = Generation::new(id, status);
    g.content_url = f.str("content_url")?;
    g.content_mime_type = f.str("content_mime_type")?;
    g.failure_reason = f.str("failure_reason")?;
    g.error_message = f.str("error_message")?;
    if status == GenerationStatus::Completed && g.content_url.is_none() {
        return Err(KppError::provider(
            "ElevenLabs finished a generation without its link.",
        ));
    }
    Ok(g)
}

fn value_of(g: &Generation) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), g.id.clone().into());
    out.insert("status".into(), g.status.as_wire().into());
    match g.status {
        GenerationStatus::Completed => {
            put(&mut out, "content_url", g.content_url.clone());
            put(&mut out, "content_mime_type", g.content_mime_type.clone());
        }
        GenerationStatus::Failed => {
            let reason = g.failure_reason.as_deref().unwrap_or("internal_error");
            let message = g
                .error_message
                .as_deref()
                .unwrap_or("The generation failed.");
            out.insert("failure_reason".into(), reason.into());
            out.insert("error_message".into(), message.into());
        }
        _ => {}
    }
    Value::Object(out)
}

/// A generation in ElevenLabs' shape: its id and status, with its link and media type once it
/// is completed and its reason and message once it has failed.
pub fn encode_generation(g: &Generation) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(&value_of(g))
}

/// One page of generations, newest first: `next_cursor` names where the next page starts, and
/// is `None` on the last.
pub fn encode_generation_list(
    generations: &[Generation],
    next_cursor: Option<&str>,
    has_more: bool,
) -> Result<Vec<u8>, KppError> {
    let mut out = Map::new();
    out.insert(
        "generations".into(),
        Value::Array(generations.iter().map(value_of).collect()),
    );
    out.insert(
        "next_cursor".into(),
        next_cursor.map_or(Value::Null, |c| c.into()),
    );
    out.insert("has_more".into(), has_more.into());
    body(&out)
}

/// The query of a generation listing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GenerationQuery {
    /// The previous page's `next_cursor`; `None` for the first page.
    pub cursor: Option<String>,
    /// How many generations a page holds, 1 to 100.
    pub page_size: u32,
    /// Only generations with this status.
    pub status: Option<GenerationStatus>,
    /// Only generations of this model.
    pub model_id: Option<String>,
}

impl Default for GenerationQuery {
    fn default() -> Self {
        GenerationQuery {
            cursor: None,
            page_size: GENERATION_PAGE_SIZE,
            status: None,
            model_id: None,
        }
    }
}

/// Reads the query of `GET /v1/flows/image`: `cursor`, `page_size` (1 to 100, 30 when left
/// out), `status` and `model_id`. Parameters ElevenLabs doesn't list are ignored, as ElevenLabs
/// ignores them.
pub fn decode_generation_query(query: &str) -> Result<GenerationQuery, KppError> {
    let mut q = GenerationQuery::default();
    for (key, value) in crate::openai_videos::form_pairs(query) {
        match key.as_str() {
            _ if value.is_empty() => {}
            "cursor" => q.cursor = Some(value),
            "page_size" => {
                q.page_size = crate::video_jobs::digits(&value)
                    .filter(|n| (1..=MAX_PAGE_SIZE).contains(n))
                    .ok_or_else(|| {
                        KppError::invalid_request(
                            "`page_size` must be a whole number from 1 to 100.",
                        )
                        .with_param("page_size")
                    })?;
            }
            "status" => {
                q.status = Some(GenerationStatus::from_wire(&value).ok_or_else(|| {
                    KppError::invalid_request(
                        "`status` must be `pending`, `generating`, `completed` or `failed`.",
                    )
                    .with_param("status")
                })?);
            }
            "model_id" => q.model_id = Some(value),
            _ => {}
        }
    }
    Ok(q)
}
