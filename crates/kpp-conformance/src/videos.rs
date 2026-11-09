//! OpenAI's Videos API for the SDK conformance suite: every video job fixture of every video
//! dialect, as `<dialect>:<folder>/<name>` in `model` or the video id, decoded from the
//! provider's form and answered as OpenAI's `VideoResource`.

use std::path::Path;

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderMap;
use axum::response::Response;
use kaeso_codecs::openai_videos::{self, VideosProfile};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    DialectId, KppError, Reply, Request, RequestKind, StreamMode, VideoJob, VideoList,
};
use serde_json::{Value, json};

use crate::{App, error_response, headers_in, read, respond, scenario};

const D: DialectId = DialectId::OpenAiVideos;

/// The video dialects and the folders of their fixtures.
pub(crate) const SOURCES: &[(DialectId, &str)] = &[
    (DialectId::OpenAiVideos, ""),
    (DialectId::OpenAiVideos, "providers/togetherai/"),
    (DialectId::XaiVideos, ""),
    (DialectId::MinimaxVideos, ""),
    (DialectId::ZaiVideos, ""),
    (DialectId::OpenRouterVideos, ""),
    (DialectId::Veo, ""),
    (DialectId::GeminiInteractions, ""),
    (DialectId::ElevenLabs, "video/"),
];

fn ctx(name: &str) -> EgressCtx {
    let ctx = EgressCtx::for_kind(RequestKind::Video, StreamMode::Off);
    if name.starts_with("providers/togetherai/") {
        ctx.with_videos_profile(VideosProfile::TOGETHER)
    } else {
        ctx
    }
}

/// The job a fixture holds: a job the provider started (`responses/`) or reported (`polls/`).
pub(crate) fn job_of(dir: &Path, source: DialectId, name: &str) -> Result<VideoJob, KppError> {
    let body = read(&dir.join(format!("{name}.json")))?;
    let ctx = ctx(name);
    if name.contains("polls/") {
        dispatch::decode_video_poll(source, 200, &HeaderList::new(), &body, &ctx)
    } else {
        match dispatch::decode_response(source, 200, &HeaderList::new(), &body, &ctx)? {
            Reply::Video(job) => Ok(job),
            _ => Err(KppError::invalid_request("conformance: not a video job")),
        }
    }
}

fn answer(app: &App, id: &str) -> Response {
    let sc = match scenario(id) {
        Ok(s) => s,
        Err(e) => return error_response(D, &e.with_status(400)),
    };
    let dir = app.fixtures.join(sc.source.as_str());
    match job_of(&dir, sc.source, &sc.name) {
        Ok(mut job) => {
            id.clone_into(&mut job.id);
            job.created_at.get_or_insert(crate::CREATED_AT as i64);
            json_answer(dispatch::encode_response(
                D,
                &Reply::Video(job),
                &EncodeCtx::new(RequestKind::Video, StreamMode::Off),
            ))
        }
        Err(e) => error_response(D, &e),
    }
}

fn json_answer(body: Result<Vec<u8>, KppError>) -> Response {
    match body {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(D, &e),
    }
}

pub(crate) async fn create(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    match dispatch::decode_request(D, RequestKind::Video, &body, &headers_in(&headers)) {
        Ok(Request::Video(r)) => answer(&app, &r.model),
        Ok(_) => error_response(D, &KppError::invalid_request("not a video request")),
        Err(e) => error_response(D, &e.with_status(400)),
    }
}

pub(crate) async fn remix(
    State(app): State<App>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match openai_videos::decode_remix(&body, &headers_in(&headers)) {
        Ok(Request::Video(r)) if r.prompt.is_some() => answer(&app, &id),
        Ok(_) => error_response(
            D,
            &KppError::invalid_request("a remix needs a prompt").with_status(400),
        ),
        Err(e) => error_response(D, &e.with_status(400)),
    }
}

pub(crate) async fn retrieve(State(app): State<App>, UrlPath(id): UrlPath<String>) -> Response {
    answer(&app, &id)
}

pub(crate) async fn delete(UrlPath(id): UrlPath<String>) -> Response {
    json_answer(openai_videos::encode_deleted(&id))
}

pub(crate) async fn content(State(app): State<App>) -> Response {
    match read(&app.fixtures.join("openai_videos/content/sample.mp4")) {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            if let Ok(v) = axum::http::HeaderValue::from_str("video/mp4") {
                res.headers_mut()
                    .insert(axum::http::header::CONTENT_TYPE, v);
            }
            res
        }
        Err(e) => error_response(D, &e),
    }
}

/// Every reported job of OpenAI's own fixtures, a page at a time.
pub(crate) async fn list(
    State(app): State<App>,
    axum::extract::RawQuery(q): axum::extract::RawQuery,
) -> Response {
    let query = match openai_videos::decode_list_query(q.as_deref().unwrap_or("")) {
        Ok(q) => q,
        Err(e) => return error_response(D, &e.with_status(400)),
    };
    let dir = app.fixtures.join(D.as_str());
    let mut jobs = Vec::new();
    for name in names(&dir, "responses") {
        if let Ok(mut job) = job_of(&dir, D, &format!("responses/{name}")) {
            job.id = format!("openai_videos:responses/{name}");
            jobs.push(job);
        }
    }
    if let Some(after) = &query.after {
        match jobs.iter().position(|j| &j.id == after) {
            Some(i) => {
                jobs.drain(..=i);
            }
            None => {
                return error_response(
                    D,
                    &KppError::new(kaeso_kpp::ErrorKind::NotFound, "no such video")
                        .with_status(404),
                );
            }
        }
    }
    let limit = query.limit() as usize;
    let has_more = jobs.len() > limit;
    jobs.truncate(limit);
    json_answer(openai_videos::encode_list(&VideoList::new(jobs, has_more)))
}

pub(crate) fn names(dir: &Path, folder: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir.join(folder)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".json") && !n.ends_with(".meta.json"))
        .map(|n| n.trim_end_matches(".json").to_owned())
        .collect();
    names.sort();
    names
}

/// What the SDK must read from a job fixture.
pub(crate) fn expected(dir: &Path, source: DialectId, name: &str) -> Value {
    match job_of(dir, source, name) {
        Ok(job) => json!({
            "status": job.status.as_str(),
            "progress": job.progress.unwrap_or(if job.status == kaeso_kpp::VideoStatus::Completed { 100 } else { 0 }),
            "seconds": job.seconds.map(|s| s.to_string()).unwrap_or_default(),
            "error_code": job.error.map(|e| e.code),
        }),
        Err(e) => json!({ "error": e.message }),
    }
}
