//! Video jobs and image generations at the mock provider, built from KPP's fixtures: each job
//! answers as running the first time it is asked and as finished after, with its file served by the
//! mock. An ElevenLabs image generation whose prompt carries `[mock:generation=failed]` ends as
//! failed. fal's queue, told apart by its `Authorization: Key …`, answers images at once and videos
//! after one status call; a prompt with `[mock:fal=failed]` ends as failed and one with
//! `[mock:fal=nsfw]` as FLUX's black safety image. Replicate's predictions answer images at once
//! and videos after one poll; a prompt with `[mock:replicate=failed]` ends as failed, one with
//! `[mock:replicate=wait]` makes an image only on the first poll, and one with
//! `[mock:replicate=foreign]` names an image off Replicate's hosts.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

const FAILED: &str = "failed-";

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
    body: &str,
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
    if let Some(resp) = replicate(polls, fixtures, method, path, body, &link) {
        return Some(resp);
    }
    let get = method == Method::GET;
    let post = method == Method::POST;
    let fal_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Key "));
    if fal_key {
        return fal(polls, fixtures, method, path, body, &link);
    }
    Some(match path {
        "/v1/videos/generations" | "/v1/videos/edits" if post => {
            json(read(fixtures, "xai_videos/responses/started.json"))
        }
        "/v1/videos" if post => json(read(fixtures, "openrouter_videos/responses/started.json")),
        // A server that speaks OpenAI's own Videos API, as a custom provider at `{mock}/openai`.
        "/openai/v1/videos" if post => json(read(fixtures, "openai_videos/responses/queued.json")),
        _ if get && tail("/openai/v1/videos/").is_some() => staged(
            polls,
            fixtures,
            path,
            "openai_videos/responses/queued.json",
            "openai_videos/responses/completed.json",
            &link,
        ),
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
        "/v1/flows/image" if post => {
            let mut started = read(fixtures, "elevenlabs/image/responses/started.json");
            if body.contains("[mock:generation=failed]") {
                started["id"] = Value::from(format!("{FAILED}JWr5N6X9ZTqf8jD2LmQb"));
            }
            json(started)
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
        _ if get && tail("/v1/flows/image/").is_some_and(|t| t.starts_with(FAILED)) => staged(
            polls,
            fixtures,
            path,
            "elevenlabs/image/polls/generating.json",
            "elevenlabs/image/polls/failed.json",
            &link,
        ),
        _ if get && tail("/v1/flows/image/").is_some() => staged(
            polls,
            fixtures,
            path,
            "elevenlabs/image/polls/generating.json",
            "elevenlabs/image/polls/completed.json",
            &link,
        ),
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

/// The request id the mock's fal queue gives every request, followed by what the request asked
/// the mock to answer.
pub const FAL_REQUEST: &str = "764cabcf-b745-4b3e-ae38-1200304cf45b";

fn fal_queue(status: &str, base: &str, fixtures: &Path, rel: &str) -> Value {
    let mut v = read(fixtures, rel);
    v["status"] = Value::from(status);
    if let Some(id) = base.rsplit('/').next() {
        v["request_id"] = Value::from(id);
    }
    v["response_url"] = Value::from(format!("https://queue.fal.run{base}"));
    if v.get("status_url").is_some() {
        v["status_url"] = Value::from(format!("https://queue.fal.run{base}/status"));
        v["cancel_url"] = Value::from(format!("https://queue.fal.run{base}/cancel"));
    }
    v
}

// fal's queue keeps a request under its owner and app (`fal-ai/flux` for `fal-ai/flux/dev`), and
// the mock names in the request id what it will answer.
fn fal(
    polls: &Polls,
    fixtures: &Path,
    method: &Method,
    path: &str,
    body: &str,
    link: &str,
) -> Option<Response> {
    if *method == Method::POST {
        let mut segments = path.trim_start_matches('/').split('/');
        let app = format!("{}/{}", segments.next()?, segments.next()?);
        let tag = if body.contains("[mock:fal=failed]") {
            "-failed"
        } else if body.contains("[mock:fal=nsfw]") {
            "-nsfw"
        } else if body.contains("\"sync_mode\":true") {
            "-sync"
        } else {
            ""
        };
        let base = format!("/{app}/requests/{FAL_REQUEST}{tag}");
        return Some(json(fal_queue(
            "IN_QUEUE",
            &base,
            fixtures,
            "fal/queue/submitted-short.json",
        )));
    }
    if *method != Method::GET {
        return None;
    }
    let video = !(path.starts_with("/fal-ai/flux") || path.starts_with("/bytedance/seedream"));
    if let Some(base) = path.strip_suffix("/status") {
        let failed = base.ends_with("-failed");
        if video && polls.next(path) <= 1 {
            return Some(json(fal_queue(
                "IN_QUEUE",
                base,
                fixtures,
                "fal/queue/in-queue.json",
            )));
        }
        let rel = if failed {
            "fal/queue/failed.json"
        } else {
            "fal/queue/completed.json"
        };
        return Some(json(fal_queue("COMPLETED", base, fixtures, rel)));
    }
    let rel = if video {
        "fal/results/videos/kling.json"
    } else if path.ends_with("-nsfw") {
        "fal/moderated/nsfw-black-image.json"
    } else if path.ends_with("-sync") {
        "fal/results/images/flux-sync.json"
    } else if path.starts_with("/bytedance/seedream") {
        "fal/results/images/seedream.json"
    } else {
        "fal/results/images/queue-doc.json"
    };
    let mut v = read(fixtures, rel);
    local_links(&mut v, link);
    Some(json(v))
}

/// The prediction id the mock's Replicate gives every prediction, followed by what the request
/// asked the mock to answer.
pub const REPLICATE_PREDICTION: &str = "ab12cd34ef56gh78ij90kl12mn";

fn prediction(fixtures: &Path, rel: &str, id: &str, link: &str) -> Response {
    let mut v = read(fixtures, rel);
    v["id"] = Value::from(id);
    local_links(&mut v, link);
    json(v)
}

fn replicate(
    polls: &Polls,
    fixtures: &Path,
    method: &Method,
    path: &str,
    body: &str,
    link: &str,
) -> Option<Response> {
    if *method == Method::GET && path == "/v1/collections/official" {
        return Some(json(read(
            fixtures,
            "replicate/models/official-collection-built.json",
        )));
    }
    if *method == Method::POST {
        let model = path
            .strip_prefix("/v1/models/")?
            .strip_suffix("/predictions")?;
        let video = matches!(model, "google/veo-3.1" | "kwaivgi/kling-v2.5-turbo-pro");
        let tag = if body.contains("[mock:replicate=failed]") {
            "-failed"
        } else if body.contains("[mock:replicate=wait]") {
            "-wait"
        } else {
            ""
        };
        let id = format!(
            "{REPLICATE_PREDICTION}-{}{tag}",
            model.split('/').nth(1).unwrap_or("model")
        );
        if video || tag == "-wait" {
            let rel = if video {
                "replicate/predictions/videos/starting-built.json"
            } else {
                "replicate/predictions/images/starting-built.json"
            };
            return Some(prediction(fixtures, rel, &id, link));
        }
        if tag == "-failed" {
            return Some(prediction(
                fixtures,
                "replicate/predictions/images/failed-built.json",
                &id,
                link,
            ));
        }
        let rel = match model {
            "black-forest-labs/flux-1.1-pro" => "replicate/predictions/images/flux-pro-built.json",
            "google/nano-banana-pro" => "replicate/predictions/images/nano-banana-pro-built.json",
            "bytedance/seedream-4.5" => "replicate/predictions/images/seedream-4.5-published.json",
            _ => "replicate/predictions/images/flux-schnell-published.json",
        };
        let mut v = read(fixtures, rel);
        v["id"] = Value::from(id);
        if body.contains("[mock:replicate=foreign]") {
            v["output"] = Value::from(vec!["https://example.com/out.png"]);
        }
        return Some(json(v));
    }
    if *method != Method::GET {
        return None;
    }
    let id = path.strip_prefix("/v1/predictions/")?;
    if id.ends_with("-wait") {
        return Some(prediction(
            fixtures,
            "replicate/predictions/images/flux-schnell-published.json",
            id,
            link,
        ));
    }
    let rel = if polls.next(path) <= 1 {
        "replicate/predictions/videos/processing-built.json"
    } else if id.ends_with("-failed") {
        "replicate/predictions/videos/failed-built.json"
    } else if id.contains("-kling") {
        "replicate/predictions/videos/kling-published.json"
    } else {
        "replicate/predictions/videos/veo-3.1-published.json"
    };
    let mut v = read(fixtures, rel);
    v["id"] = Value::from(id);
    // Replicate deletes a video an hour after it ends, and the published predictions ended
    // long ago: the mock's videos end far ahead, so they are never past their hour.
    if v.get("completed_at").is_some() && v["status"] == "succeeded" {
        v["completed_at"] = Value::from("2099-01-01T00:00:00.000Z");
    }
    local_links(&mut v, link);
    Some(json(v))
}
