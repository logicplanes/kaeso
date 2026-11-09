//! Video jobs at the mock provider, built from KPP's video fixtures: each job answers as
//! running the first time it is asked and as finished after, with its file served by the mock.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

/// The path the mock serves every made video's file at.
pub const SAMPLE_PATH: &str = "/__mock/videos/sample.mp4";

#[derive(Debug, Default)]
pub(crate) struct Polls(Mutex<HashMap<String, u32>>);

impl Polls {
    pub(crate) fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    fn next(&self, job: &str) -> u32 {
        let mut polls = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let n = polls.entry(job.to_owned()).or_default();
        *n += 1;
        *n
    }
}

fn read(fixtures: &Path, rel: &str) -> Value {
    let path = fixtures.join(rel);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("mock: no fixture {}: {e}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("mock: {}: {e}", path.display()))
}

// Links to made files point at the mock, so the gateway fetches them from it.
fn local_links(v: &mut Value, link: &str) {
    match v {
        Value::String(s)
            if s.starts_with("https://")
                && (s.contains(".mp4")
                    || s.contains("content?index")
                    || s.contains("/generations/")
                    || s.contains("download")) =>
        {
            *s = link.to_owned();
        }
        Value::Array(items) => items.iter_mut().for_each(|i| local_links(i, link)),
        Value::Object(map) => map.values_mut().for_each(|i| local_links(i, link)),
        _ => {}
    }
}

fn json(v: Value) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&v).unwrap_or_default()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn staged(
    polls: &Polls,
    fixtures: &Path,
    job: &str,
    running: &str,
    done: &str,
    link: &str,
) -> Response {
    let rel = if polls.next(job) <= 1 { running } else { done };
    let mut v = read(fixtures, rel);
    local_links(&mut v, link);
    json(v)
}

/// The mock's answer to a video call, or `None` for any other path.
pub(crate) fn answer(
    polls: &Polls,
    fixtures: &Path,
    method: &Method,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
) -> Option<Response> {
    if method == Method::GET && path == SAMPLE_PATH {
        return Some(sample(fixtures, headers));
    }
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1");
    let link = format!("http://{host}{SAMPLE_PATH}");
    let tail = |prefix: &str| {
        path.strip_prefix(prefix)
            .filter(|t| !t.is_empty() && !t.contains('/'))
    };
    let get = method == Method::GET;
    let post = method == Method::POST;
    Some(match path {
        "/v1/videos/generations" | "/v1/videos/edits" if post => {
            json(read(fixtures, "xai_videos/responses/started.json"))
        }
        "/v1/videos" if post => json(read(fixtures, "openrouter_videos/responses/started.json")),
        "/v2/videos" if post => json(read(
            fixtures,
            "openai_videos/providers/togetherai/responses/in-progress.json",
        )),
        "/v1/video_generation" if post => json(read(
            fixtures,
            "minimax_videos/responses/hailuo-started.json",
        )),
        "/v2/video_generation" if post => {
            json(read(fixtures, "minimax_videos/responses/h3-started.json"))
        }
        "/api/paas/v4/videos/generations" if post => {
            json(read(fixtures, "zai_videos/responses/started.json"))
        }
        "/v1beta/interactions" if post => {
            json(read(fixtures, "gemini_interactions/responses/started.json"))
        }
        "/v1/flows/video" if post => {
            json(read(fixtures, "elevenlabs/video/responses/started.json"))
        }
        "/api/v1/services/aigc/video-generation/video-synthesis"
        | "/api/v1/services/aigc/image2video/video-synthesis"
            if post =>
        {
            json(read(fixtures, "dashscope/responses/started.json"))
        }
        _ if get && tail("/api/v1/tasks/").is_some() => staged(
            polls,
            fixtures,
            path,
            "dashscope/polls/running.json",
            "dashscope/polls/succeeded.json",
            &link,
        ),
        "/v1/query/video_generation" if get => {
            let job = query.unwrap_or_default();
            staged(
                polls,
                fixtures,
                job,
                "minimax_videos/polls/hailuo-processing.json",
                "minimax_videos/polls/hailuo-success.json",
                &link,
            )
        }
        "/v1/files/retrieve" if get => {
            let mut v = read(fixtures, "minimax_videos/files/retrieved.json");
            local_links(&mut v, &link);
            json(v)
        }
        _ if post && path.ends_with(":predictLongRunning") => {
            json(read(fixtures, "veo/responses/started.json"))
        }
        _ if post && path.ends_with(":fetchPredictOperation") => staged(
            polls,
            fixtures,
            path,
            "veo/polls/running.json",
            "veo/polls/done.json",
            &link,
        ),
        _ if get && tail("/v1/videos/").is_some_and(|t| t.starts_with("gen-vid-")) => staged(
            polls,
            fixtures,
            path,
            "openrouter_videos/polls/in-progress.json",
            "openrouter_videos/polls/completed.json",
            &link,
        ),
        _ if get && tail("/v1/videos/").is_some() => staged(
            polls,
            fixtures,
            path,
            "xai_videos/polls/pending.json",
            "xai_videos/polls/done.json",
            &link,
        ),
        _ if get && tail("/v2/videos/").is_some() => staged(
            polls,
            fixtures,
            path,
            "openai_videos/providers/togetherai/responses/in-progress.json",
            "openai_videos/providers/togetherai/responses/completed.json",
            &link,
        ),
        _ if get && tail("/v2/query/video_generation/").is_some() => staged(
            polls,
            fixtures,
            path,
            "minimax_videos/polls/h3-running.json",
            "minimax_videos/polls/h3-succeeded.json",
            &link,
        ),
        _ if method == Method::DELETE && tail("/v2/video_generation/").is_some() => json(
            serde_json::json!({ "task_id": tail("/v2/video_generation/"), "action": "deleted", "status": "deleted" }),
        ),
        _ if get && tail("/api/paas/v4/async-result/").is_some() => staged(
            polls,
            fixtures,
            path,
            "zai_videos/polls/processing.json",
            "zai_videos/polls/success.json",
            &link,
        ),
        _ if get && tail("/v1beta/interactions/").is_some() => staged(
            polls,
            fixtures,
            path,
            "gemini_interactions/polls/in-progress.json",
            "gemini_interactions/polls/completed.json",
            &link,
        ),
        _ if method == Method::DELETE && tail("/v1beta/interactions/").is_some() => {
            json(serde_json::json!({}))
        }
        _ if get && tail("/v1/flows/video/").is_some() => staged(
            polls,
            fixtures,
            path,
            "elevenlabs/video/polls/generating.json",
            "elevenlabs/video/polls/completed.json",
            &link,
        ),
        _ => return None,
    })
}

// A made video, with one `Range` answered as RFC 9110 asks.
fn sample(fixtures: &Path, headers: &HeaderMap) -> Response {
    let bytes =
        std::fs::read(fixtures.join("openai_videos/content/sample.mp4")).unwrap_or_default();
    let total = bytes.len();
    let range = headers
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.split_once('-'))
        .and_then(|(s, e)| {
            let start: usize = s.parse().ok()?;
            let end: usize = if e.is_empty() {
                total.checked_sub(1)?
            } else {
                e.parse().ok()?
            };
            (start <= end && end < total).then_some((start, end))
        });
    let builder = Response::builder()
        .header("content-type", "video/mp4")
        .header("accept-ranges", "bytes");
    match range {
        Some((start, end)) => builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header("content-range", format!("bytes {start}-{end}/{total}"))
            .header("content-length", end - start + 1)
            .body(Body::from(
                bytes
                    .get(start..=end)
                    .map(<[u8]>::to_vec)
                    .unwrap_or_default(),
            )),
        None => builder
            .status(StatusCode::OK)
            .header("content-length", total)
            .body(Body::from(bytes)),
    }
    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
