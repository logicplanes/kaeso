use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kaeso_store::pg::avatars;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::platform::error::{ApiError, ApiResult};
use crate::platform::params::Path;
use crate::state::AppState;

pub const MAX_BYTES: usize = 512 * 1024;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A picture to set.")]
pub struct AvatarUpload {
    #[doc = "`image/png`, `image/jpeg` or `image/webp`."]
    #[schema(example = "image/png")]
    pub content_type: String,
    #[doc = "The image, base64-encoded; at most 512 KiB once decoded."]
    pub data: String,
}

pub struct Checked {
    pub hash: String,
    pub content_type: &'static str,
    pub data: Vec<u8>,
}

fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.get(0..4) == Some(b"RIFF".as_slice()) && b.get(8..12) == Some(b"WEBP".as_slice()) {
        Some("image/webp")
    } else {
        None
    }
}

pub fn check(req: &AvatarUpload) -> ApiResult<Checked> {
    let data = STANDARD
        .decode(req.data.trim())
        .map_err(|_| ApiError::invalid("data", "The picture isn't valid base64."))?;
    if data.is_empty() {
        return Err(ApiError::invalid("data", "The picture is empty."));
    }
    if data.len() > MAX_BYTES {
        return Err(ApiError::invalid(
            "data",
            "The picture is larger than 512 KiB.",
        ));
    }
    let content_type = sniff(&data).ok_or_else(|| {
        ApiError::invalid("data", "The picture must be a PNG, JPEG or WebP image.")
    })?;
    if content_type != req.content_type {
        return Err(ApiError::invalid(
            "content_type",
            format!("The picture is {content_type}, not {}.", req.content_type),
        ));
    }
    let hash = crate::cache::hex(&Sha256::digest(&data));
    Ok(Checked {
        hash,
        content_type,
        data,
    })
}

#[utoipa::path(
    get,
    path = "/me/avatars/{hash}",
    summary = "A stored picture, by its hash.",
    tag = "me",
    params(("hash" = String, Path, description = "The picture's SHA-256, from an `avatar_url`.")),
    responses((status = 200, description = "The image", content_type = "image/png"))
)]
pub async fn get(State(state): State<AppState>, Path(hash): Path<String>) -> ApiResult<Response> {
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::not_found("picture"));
    }
    let picture = avatars::get(&state.pg, &hash)
        .await?
        .ok_or_else(|| ApiError::not_found("picture"))?;
    let content_type = HeaderValue::from_str(&picture.content_type).map_err(ApiError::internal)?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=31536000, immutable"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        picture.data,
    )
        .into_response())
}
