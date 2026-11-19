//! fal's queue (<https://fal.ai/docs/documentation/model-apis/inference/queue>, the Queue API's
//! OpenAPI document, read 2026-11-14): `POST /{model id}` with the model's own input as the
//! body answers with the request's `status_url` and `response_url`; the status ends in
//! `COMPLETED`, after which the result URL answers with the model's output. Every model has its
//! own input and output schema (`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=…`),
//! so the dialect maps OpenAI's image and video fields per model, for the models in
//! [`IMAGE_MODELS`] and [`VIDEO_MODELS`] only.
//!
//! Kaeso follows only the URLs fal returns, and only on fal's queue hosts ([`QUEUE_HOSTS`]): it
//! keeps their paths and sends them to the provider's base URL, so a status or result is never
//! asked for anywhere else.

mod error;
mod images;
mod videos;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;

use kaeso_kpp::{DialectId, KppError, Origin, Reply, Request, RequestKind, Source, VideoJob};
use serde_json::Value;

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{no_stream, openai_listing, video_reply};

pub use images::IMAGE_MODELS;
pub use videos::VIDEO_MODELS;

const D: DialectId = DialectId::Fal;
const PROVIDER: &str = "fal";

/// The hosts fal's queue answers on and names in its status and result URLs: `queue.fal.run`
/// and its backup `queue.falrun.com` (the Queue API's OpenAPI `servers`).
pub const QUEUE_HOSTS: &[&str] = &["queue.fal.run", "queue.falrun.com"];

/// Asks fal not to keep the request's input and output in the account's request history
/// (`X-Fal-Store-IO`, a header of the Queue API's OpenAPI document).
const STORE_IO: (&str, &str) = ("x-fal-store-io", "0");

const IN_QUEUE: &str = "IN_QUEUE";
const IN_PROGRESS: &str = "IN_PROGRESS";
const COMPLETED: &str = "COMPLETED";

/// fal's queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Fal;

impl crate::codec::sealed::Sealed for Fal {}

fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
}

/// Whether `path` is an absolute path of plain segments, which no URL parser changes.
pub(crate) fn safe_path(path: &str) -> bool {
    path.strip_prefix('/')
        .is_some_and(|rest| rest.split('/').all(safe_segment))
}

/// The path of a URL fal returned, when it is on one of its queue hosts and has nothing but
/// plain path segments: no port, user, query or fragment.
pub fn queue_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_at(rest.find('/')?);
    if !QUEUE_HOSTS.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        return None;
    }
    safe_path(path).then(|| path.to_owned())
}

/// The request that submits `model`'s input.
fn submit(model: &str, body: Vec<u8>) -> Result<EncodedRequest, KppError> {
    let path = format!("/{model}");
    if !safe_path(&path) {
        return Err(KppError::invalid_request(format!(
            "The model id `{model}` can't be part of a fal URL; use the model's real id."
        ))
        .with_param("model")
        .with_status(400));
    }
    let headers = HeaderList::new()
        .with("content-type", crate::codec::JSON_MEDIA_TYPE)
        .with(STORE_IO.0, STORE_IO.1);
    Ok(EncodedRequest::post_json(path, headers, body))
}

/// A `GET` of a path [`queue_path`] took from a URL fal returned.
fn get(path: &str, param: &'static str) -> Result<EncodedRequest, KppError> {
    if !safe_path(path) {
        return Err(KppError::invalid_request(format!(
            "`{path}` is not a fal queue path; Kaeso asks only the URLs fal returned."
        ))
        .with_param(param)
        .with_status(400));
    }
    Ok(EncodedRequest::get(path, HeaderList::new()))
}

/// A queue status as fal writes it: the status, and the paths of its status and result URLs.
struct Queued {
    status: String,
    status_path: Option<String>,
    result_path: Option<String>,
    failed: bool,
}

// The queue doc's examples of `IN_QUEUE` and `IN_PROGRESS` name the result URL but not the
// status URL, which is the result URL followed by `/status`.
fn queued(v: &Value) -> Option<Queued> {
    let status = v.get("status")?.as_str()?.to_owned();
    if !matches!(status.as_str(), IN_QUEUE | IN_PROGRESS | COMPLETED) {
        return None;
    }
    let path = |k: &str| v.get(k).and_then(Value::as_str).and_then(queue_path);
    let result_path = path("response_url");
    let status_path =
        path("status_url").or_else(|| result_path.as_ref().map(|p| format!("{p}/status")));
    let failed = v.get("error").is_some_and(|e| !e.is_null())
        || v.get("error_type").is_some_and(|e| !e.is_null());
    Some(Queued {
        status,
        status_path,
        result_path,
        failed,
    })
}

/// The path to ask next while fal makes an image: the status URL until the request completes,
/// then the result URL. `None` once the body is the result itself or a failed request's status.
pub(crate) fn pending_task(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let q = queued(&v)?;
    match q.status.as_str() {
        COMPLETED if q.failed => None,
        COMPLETED => q.result_path,
        _ => q.status_path,
    }
}

/// The request that asks the path [`pending_task`] named.
pub(crate) fn poll_request(task: &str) -> Result<EncodedRequest, KppError> {
    get(task, "task_id")
}

fn not_served(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "fal's queue carries images and videos only.",
    )
}

impl Codec for Fal {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, Self::DIALECT)?;
        crate::limits::request_shape(D, body)?;
        match kind {
            RequestKind::Image => images::decode_request(body),
            RequestKind::Video => videos::decode_request(body),
            other => Err(not_served(other)),
        }
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, Self::DIALECT)?;
        match reply {
            Reply::Image(r) => images::encode_response(r),
            other => crate::wire::to_body(&videos::encode_started(video_reply(other, D)?)),
        }
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => images::check(r).map(|_| ()),
            Request::Video(r) => videos::check(r).map(|_| ()),
            other => Err(not_served(other.kind())),
        }
    }

    fn encode_request(req: &Request, _ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => images::encode_request(r),
            Request::Video(r) => videos::encode_request(r),
            other => Err(not_served(other.kind())),
        }
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        match ctx.kind {
            RequestKind::Image => images::decode_response(body, ctx).map(Reply::Image),
            RequestKind::Video => videos::decode_started(body, ctx).map(Reply::Video),
            other => Err(not_served(other)),
        }
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        get(&format!("{job}/status"), "video_id")
    }

    fn decode_video_poll(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<VideoJob, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        videos::decode_poll(body, ctx)
    }

    fn encode_video_file(file: &str, _ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
        get(file, "video_id")
    }

    fn decode_video_file(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<Source, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        videos::decode_file(body)
    }

    openai_listing!(D);
}
