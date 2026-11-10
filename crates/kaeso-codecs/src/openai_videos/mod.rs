//! OpenAI's Videos API (openai-openapi `926a7b8`): video jobs created from JSON or an upload,
//! remixed, listed, polled, downloaded and deleted, and the providers that speak a form of it,
//! each described by a [`VideosProfile`].

mod profile;
mod reply;
mod request;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ListOrder, Reply, Request, RequestKind, VideoJob, VideoList,
    VideoListQuery, VideoVariant,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{no_stream, openai_listing};

pub use profile::{VideosBody, VideosProfile};
pub use request::{MAX_FORM_PARTS, MAX_REFERENCE_BYTES};

const DIALECT: DialectId = DialectId::OpenAiVideos;

/// OpenAI's Videos API: `POST /v1/videos`, `POST /v1/videos/{id}/remix`, `GET /v1/videos`,
/// `GET /v1/videos/{id}`, `GET /v1/videos/{id}/content` and `DELETE /v1/videos/{id}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiVideos;

impl crate::codec::sealed::Sealed for OpenAiVideos {}

impl OpenAiVideos {
    pub(crate) fn decode_checked(
        _kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode_create(body, headers)
    }
}

/// Decodes the body of `POST /v1/videos/{video_id}/remix`: the new prompt, as JSON or an
/// upload. The request names no model and no video; the caller fills in both from the video
/// the path names.
pub fn decode_remix(body: &[u8], headers: &HeaderList) -> Result<Request, KppError> {
    crate::limits::request_shape(DIALECT, body)?;
    request::decode_remix(body, headers)
}

/// The query of `GET /v1/videos`: `after`, `limit` (0 to 100) and `order` (`asc` or `desc`).
pub fn decode_list_query(query: &str) -> Result<VideoListQuery, KppError> {
    let mut q = VideoListQuery::default();
    for (key, value) in form_pairs(query) {
        match key.as_str() {
            "after" if !value.is_empty() => q.after = Some(value),
            "after" => {}
            "limit" => {
                let n = crate::video_jobs::digits(&value)
                    .filter(|n| *n <= VideoListQuery::MAX_LIMIT)
                    .ok_or_else(|| {
                        KppError::invalid_request("`limit` must be a whole number from 0 to 100.")
                            .with_param("limit")
                    })?;
                q.limit = Some(n);
            }
            "order" => {
                q.order = match value.as_str() {
                    "asc" => ListOrder::Asc,
                    "desc" => ListOrder::Desc,
                    _ => {
                        return Err(
                            KppError::invalid_request("`order` must be `asc` or `desc`.")
                                .with_param("order"),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(q)
}

/// The `variant` query parameter of `GET /v1/videos/{video_id}/content`: `video` (the
/// default), `thumbnail` or `spritesheet`.
pub fn decode_content_query(query: &str) -> Result<VideoVariant, KppError> {
    let mut variant = VideoVariant::Video;
    for (key, value) in form_pairs(query) {
        if key == "variant" {
            variant = VideoVariant::from_wire(&value).ok_or_else(|| {
                KppError::invalid_request(
                    "`variant` must be `video`, `thumbnail` or `spritesheet`.",
                )
                .with_param("variant")
            })?;
        }
    }
    Ok(variant)
}

/// A page of videos as OpenAI's `VideoListResource`.
pub fn encode_list(list: &VideoList) -> Result<Vec<u8>, KppError> {
    reply::encode_list(list)
}

/// The answer to `DELETE /v1/videos/{video_id}`: OpenAI's `DeletedVideoResource`.
pub fn encode_deleted(id: &str) -> Result<Vec<u8>, KppError> {
    reply::encode_deleted(id)
}

pub(crate) fn form_pairs(query: &str) -> impl Iterator<Item = (String, String)> + '_ {
    query.split('&').filter(|p| !p.is_empty()).map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (unescape(k), unescape(v))
    })
}

fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'+' => out.push(b' '),
            b'%' => {
                let hex = bytes
                    .get(i + 1..i + 3)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b),
                }
            }
            _ => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Codec for OpenAiVideos {
    const DIALECT: DialectId = DIALECT;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        if kind != RequestKind::Video {
            return Err(crate::video_jobs::not_videos(DIALECT, kind));
        }
        crate::limits::request_shape(DIALECT, body)?;
        Self::decode_checked(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        let job = crate::video_jobs::video_reply(reply, DIALECT)?;
        reply::encode_video(job, ctx)
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(kaeso_kpp::Origin::new(DIALECT))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        request::check(req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        request::encode(req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        reply::decode_job(body, ctx).map(Reply::Video)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(DIALECT))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        crate::openai_responses::error::decode(DIALECT, status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            format!(
                "{}/{}",
                ctx.videos_profile.path,
                crate::video_jobs::path_segment(job)
            ),
            HeaderList::new(),
        ))
    }

    fn decode_video_poll(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<VideoJob, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        reply::decode_job(body, ctx)
    }

    fn encode_video_delete(
        job: &str,
        _model: &str,
        ctx: &EgressCtx,
    ) -> Result<Option<EncodedRequest>, KppError> {
        Ok(ctx.videos_profile.deletes.then(|| {
            EncodedRequest::delete(
                format!(
                    "{}/{}",
                    ctx.videos_profile.path,
                    crate::video_jobs::path_segment(job)
                ),
                HeaderList::new(),
            )
        }))
    }

    openai_listing!(DIALECT);
}
