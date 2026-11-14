#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Env};
use http_body_util::BodyExt;
use kaeso_server::model_api::hooks::{Outcome, VideoCall};
use serde_json::{Value, json};
use tower::ServiceExt;

const XAI_SECRET: &str = "xai-test-credential-0000000000000000000000000000";
const GOOGLE_SECRET: &str = "AIzaSyTestGoogleVideosCredential000000000";
const ZAI_SECRET: &str = "zai-test-credential.00000000000000000000";
const MINIMAX_SECRET: &str = "sk-api-test-minimax-videos-0000000000000000";
const QWENCLOUD_SECRET: &str = "sk-test-qwencloud-videos-00000000000000";
const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    setup(&env).await;
    env
}

async fn setup(env: &Env) {
    let org = &env.owner.org_id;
    for (provider, secret) in [
        ("xai", XAI_SECRET),
        ("google", GOOGLE_SECRET),
        ("zai", ZAI_SECRET),
        ("minimax", MINIMAX_SECRET),
        ("alibaba", QWENCLOUD_SECRET),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/credentials"),
                json!({ "provider": provider, "name": provider, "secret": secret }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{provider}: {:?}", r.body);
    }
    for (provider, model) in [
        ("xai", "grok-imagine-video-1.5"),
        ("google", "gemini-omni-1.2-flash"),
        ("zai", "cogvideox-3"),
        ("minimax", "MiniMax-Hailuo-2.3"),
        ("minimax", "MiniMax-H3"),
        ("alibaba", "wan2.7-t2v"),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
}

async fn send(
    env: &Env,
    method: &str,
    path: &str,
    key: &str,
    body: Option<Value>,
    range: Option<&str>,
) -> Answer {
    let mut req = Request::builder()
        .uri(path)
        .method(method)
        .header("authorization", format!("Bearer {key}"));
    if let Some(r) = range {
        req = req.header("range", r);
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

async fn create(env: &Env, body: Value) -> Answer {
    send(env, "POST", "/v1/videos", &env.key, Some(body), None).await
}

async fn retrieve(env: &Env, id: &str) -> Answer {
    send(
        env,
        "GET",
        &format!("/v1/videos/{id}"),
        &env.key,
        None,
        None,
    )
    .await
}

async fn completions(env: &Env) -> Vec<Outcome> {
    env.outcomes()
        .await
        .into_iter()
        .filter(|o| o.video == Some(VideoCall::Completed))
        .collect()
}

async fn other_project_key(env: &Env) -> String {
    let org = &env.owner.org_id;
    let project = env
        .app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Elsewhere" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn sample() -> Vec<u8> {
    std::fs::read(
        kaeso_testkit::mock_provider::kpp_fixtures().join("openai_videos/content/sample.mp4"),
    )
    .unwrap()
}

#[tokio::test]
async fn an_openai_app_makes_a_spacex_video_and_kaeso_bills_it_once_when_it_ends() {
    let env = env().await;
    let r = create(&env, json!({ "model": "grok-imagine-video-1.5", "prompt": "A serene lake at sunrise", "seconds": "8", "size": "1280x720" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let video = r.json();
    let id = video["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("video_"), "{id}");
    assert!(
        !id.contains("a3d1008e"),
        "Kaeso's id never shows the provider's"
    );
    assert_eq!(video["object"], "video");
    assert_eq!(video["status"], "queued");
    assert_eq!(video["model"], "grok-imagine-video-1.5");
    assert_eq!(
        (video["seconds"].as_str(), video["size"].as_str()),
        (Some("8"), Some("1280x720"))
    );
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/videos/generations");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {XAI_SECRET}").as_str())
    );
    assert_eq!(
        sent.json(),
        json!({ "model": "grok-imagine-video-1.5", "prompt": "A serene lake at sunrise", "duration": 8, "aspect_ratio": "16:9", "resolution": "720p" })
    );
    let started = env.outcomes().await.into_iter().last().unwrap();
    assert_eq!(kaeso_server::services::logs::endpoint(&started), "videos");
    assert_eq!(
        kaeso_server::services::usage::costed(&started).1,
        rust_decimal::Decimal::ZERO
    );
    assert_eq!(started.media.unwrap().output_videos, 1);

    let running = retrieve(&env, &id).await.json();
    assert_eq!(
        (running["status"].as_str(), running["progress"].as_u64()),
        (Some("in_progress"), Some(40))
    );
    assert!(completions(&env).await.is_empty());
    let (a, b) = tokio::join!(retrieve(&env, &id), retrieve(&env, &id));
    assert_eq!(a.json()["status"], "completed");
    assert_eq!(b.json()["status"], "completed");
    let done = retrieve(&env, &id).await.json();
    assert_eq!(
        (done["progress"].as_u64(), done["seconds"].as_str()),
        (Some(100), Some("6"))
    );
    let billed = completions(&env).await;
    assert_eq!(
        billed.len(),
        1,
        "two requests saw the job end, one billed it"
    );
    let o = &billed[0];
    assert_eq!(
        kaeso_server::services::logs::endpoint(o),
        "videos/completed"
    );
    let media = o.media.clone().unwrap();
    assert_eq!(
        (
            media.output_videos,
            media.output_ms,
            media.resolution.as_deref()
        ),
        (1, 6000, Some("720p"))
    );
    assert!(kaeso_server::services::usage::costed(o).1 > rust_decimal::Decimal::ZERO);
}

#[tokio::test]
async fn a_finished_video_streams_from_its_provider_with_ranges_and_refuses_what_it_lacks() {
    let env = env().await;
    let id = create(
        &env,
        json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" }),
    )
    .await
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let early = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(early.status, StatusCode::BAD_REQUEST, "{}", early.text());
    retrieve(&env, &id).await;
    let whole = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(whole.status, StatusCode::OK, "{}", whole.text());
    assert_eq!(whole.header("content-type"), Some("video/mp4"));
    assert_eq!(whole.body, sample());
    let part = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content?variant=video"),
        &env.key,
        None,
        Some("bytes=0-9"),
    )
    .await;
    assert_eq!(part.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.body, sample()[..10]);
    assert!(
        part.header("content-range")
            .unwrap()
            .starts_with("bytes 0-9/")
    );
    let download = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path.ends_with("sample.mp4"))
        .unwrap();
    assert_eq!(
        download.header("authorization"),
        Some(format!("Bearer {XAI_SECRET}").as_str()),
        "the mock serves the file on the provider's own host"
    );
    let thumb = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content?variant=thumbnail"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(thumb.status, StatusCode::NOT_FOUND);
    assert_eq!(thumb.json()["error"]["param"], "variant");
    let bad = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content?variant=gif"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn videos_answer_only_their_own_project_and_ids_never_leak() {
    let env = env().await;
    let id = create(
        &env,
        json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" }),
    )
    .await
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let other = other_project_key(&env).await;
    for (method, path, body) in [
        ("GET", format!("/v1/videos/{id}"), None),
        ("GET", format!("/v1/videos/{id}/content"), None),
        ("DELETE", format!("/v1/videos/{id}"), None),
        (
            "POST",
            format!("/v1/videos/{id}/remix"),
            Some(json!({ "prompt": "snow" })),
        ),
    ] {
        let r = send(&env, method, &path, &other, body, None).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            r.text()
        );
        assert!(
            r.json()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("belongs to this project")
        );
    }
    let listed = send(&env, "GET", "/v1/videos", &other, None, None)
        .await
        .json();
    assert_eq!(listed["data"].as_array().unwrap().len(), 0);
    let unknown = retrieve(&env, "video_01k7a3f8m2xq4v9c5r6t8w0yz1").await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let garbage = retrieve(&env, "a3d1008e-4544-40d4-d075-11527e794e4a").await;
    assert_eq!(garbage.status, StatusCode::NOT_FOUND);
    let video: kaeso_core::id::VideoId = id.parse().unwrap();
    let org: kaeso_core::id::OrgId = env.owner.org_id.parse().unwrap();
    let project: kaeso_core::id::ProjectId = env.project.parse().unwrap();
    let pg = &env.app.state.pg;
    let get = |o, p| kaeso_store::pg::videos::get(pg, o, p, video.uuid());
    assert!(get(org.uuid(), project.uuid()).await.unwrap().is_some());
    assert!(
        get(uuid::Uuid::new_v4(), project.uuid())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        get(org.uuid(), uuid::Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn listings_page_in_both_orders() {
    let env = env().await;
    let mut ids = Vec::new();
    for n in 0..3 {
        ids.push(
            create(
                &env,
                json!({ "model": "grok-imagine-video-1.5", "prompt": format!("fox {n}") }),
            )
            .await
            .json()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let page = send(&env, "GET", "/v1/videos?limit=2", &env.key, None, None)
        .await
        .json();
    assert_eq!(page["object"], "list");
    let first: Vec<&str> = page["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(first, [ids[2].as_str(), ids[1].as_str()]);
    assert_eq!(page["has_more"], true);
    assert_eq!(page["last_id"], ids[1]);
    let rest = send(
        &env,
        "GET",
        &format!("/v1/videos?limit=2&after={}", ids[1]),
        &env.key,
        None,
        None,
    )
    .await
    .json();
    assert_eq!(rest["data"][0]["id"], ids[0]);
    assert_eq!(rest["has_more"], false);
    let asc = send(
        &env,
        "GET",
        "/v1/videos?order=asc&limit=1",
        &env.key,
        None,
        None,
    )
    .await
    .json();
    assert_eq!(asc["data"][0]["id"], ids[0]);
    let missing = send(
        &env,
        "GET",
        "/v1/videos?after=video_01k7a3f8m2xq4v9c5r6t8w0yz1",
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json()["error"]["param"], "after");
}

#[tokio::test]
async fn a_hailuo_video_is_fetched_through_its_file_id_and_billed_per_clip() {
    let env = env().await;
    let r = create(
        &env,
        json!({ "model": "MiniMax-Hailuo-2.3", "prompt": "A man picks up a book" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/video_generation");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {MINIMAX_SECRET}").as_str())
    );
    assert_eq!(retrieve(&env, &id).await.json()["status"], "in_progress");
    assert_eq!(retrieve(&env, &id).await.json()["status"], "completed");
    let file = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    assert_eq!(file.body, sample());
    assert!(
        env.mock
            .received()
            .iter()
            .any(|r| r.path == "/v1/files/retrieve"
                && r.query.as_deref() == Some("file_id=176844028768320"))
    );
    let billed = completions(&env).await;
    assert_eq!(billed.len(), 1);
    assert_eq!(
        billed[0].media.as_ref().unwrap().size.as_deref(),
        Some("1920x1080")
    );
}

#[tokio::test]
async fn deleting_a_video_deletes_it_at_providers_that_can_and_forgets_it() {
    let env = env().await;
    let id = create(&env, json!({ "model": "MiniMax-H3", "prompt": "A boy playing basketball", "seconds": "5", "size": "1366x768" })).await.json()["id"].as_str().unwrap().to_owned();
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v2/video_generation");
    assert_eq!(
        (
            sent.json()["resolution"].as_str(),
            sent.json()["ratio"].as_str()
        ),
        (Some("768P"), Some("16:9"))
    );
    let r = send(
        &env,
        "DELETE",
        &format!("/v1/videos/{id}"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json(),
        json!({ "object": "video.deleted", "deleted": true, "id": id })
    );
    assert!(
        env.mock
            .received()
            .iter()
            .any(|r| r.method == "DELETE" && r.path == "/v2/video_generation/424010985738629")
    );
    assert_eq!(retrieve(&env, &id).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_remix_goes_to_the_provider_that_made_the_video() {
    let env = env().await;
    let id = create(
        &env,
        json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" }),
    )
    .await
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let early = send(
        &env,
        "POST",
        &format!("/v1/videos/{id}/remix"),
        &env.key,
        Some(json!({ "prompt": "in snow" })),
        None,
    )
    .await;
    assert_eq!(early.status, StatusCode::BAD_REQUEST, "{}", early.text());
    retrieve(&env, &id).await;
    let r = send(
        &env,
        "POST",
        &format!("/v1/videos/{id}/remix"),
        &env.key,
        Some(json!({ "prompt": "in snow" })),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let remix = r.json();
    assert_eq!(remix["remixed_from_video_id"], id);
    assert_ne!(remix["id"], id);
    let sent = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path == "/v1/videos/edits")
        .unwrap();
    assert_eq!(sent.json()["prompt"], "in snow");
    assert!(
        sent.json()["video"]["url"]
            .as_str()
            .unwrap()
            .ends_with("sample.mp4")
    );
    let o = env
        .outcomes()
        .await
        .into_iter()
        .rev()
        .find(|o| o.video == Some(VideoCall::Remix))
        .unwrap();
    assert_eq!(kaeso_server::services::logs::endpoint(&o), "videos/remix");
    let remix_id = remix["id"].as_str().unwrap();
    for _ in 0..3 {
        if retrieve(&env, remix_id).await.json()["status"] == "completed" {
            break;
        }
    }
    let billed = completions(&env).await;
    assert_eq!(
        billed.last().unwrap().media.as_ref().unwrap().input_ms,
        6000,
        "the edited video's length is billed as input"
    );

    let zai = create(&env, json!({ "model": "cogvideox-3", "prompt": "cat" }))
        .await
        .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    retrieve(&env, &zai).await;
    retrieve(&env, &zai).await;
    let refused = send(
        &env,
        "POST",
        &format!("/v1/videos/{zai}/remix"),
        &env.key,
        Some(json!({ "prompt": "dog" })),
        None,
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{}",
        refused.text()
    );
    assert_eq!(refused.json()["error"]["param"], "video_id");
}

#[tokio::test]
async fn requests_kaeso_cannot_send_are_refused_before_anything_is_sent() {
    let env = env().await;
    let sora = create(&env, json!({ "model": "sora-2", "prompt": "cat" })).await;
    assert_eq!(sora.status, StatusCode::BAD_REQUEST);
    assert!(
        sora.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("shut down Sora")
    );
    let claude = create(
        &env,
        json!({ "model": "claude-haiku-4-5", "prompt": "cat" }),
    )
    .await;
    assert_eq!(claude.status, StatusCode::BAD_REQUEST);
    assert!(
        claude.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("offers no video API")
    );
    let seconds = create(
        &env,
        json!({ "model": "cogvideox-3", "prompt": "cat", "seconds": "8" }),
    )
    .await;
    assert_eq!(seconds.status, StatusCode::BAD_REQUEST);
    assert_eq!(seconds.json()["error"]["param"], "seconds");
    let text_model = create(&env, json!({ "model": "gpt-4o", "prompt": "cat" })).await;
    assert_eq!(text_model.status, StatusCode::BAD_REQUEST);
    let edits = send(
        &env,
        "POST",
        "/v1/videos/edits",
        &env.key,
        Some(json!({ "prompt": "x" })),
        None,
    )
    .await;
    assert_eq!(edits.status, StatusCode::NOT_FOUND);
    assert!(
        env.mock
            .received()
            .iter()
            .all(|r| !r.path.contains("video"))
    );
}

#[tokio::test]
async fn an_upload_reaches_spacex_as_its_first_frame() {
    let env = env().await;
    let mut body = Vec::new();
    for (name, value) in [
        ("model", "grok-imagine-video-1.5"),
        ("prompt", "The lake ripples"),
    ] {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(b"--kb\r\nContent-Disposition: form-data; name=\"input_reference\"; filename=\"first.png\"\r\nContent-Type: image/png\r\n\r\n");
    body.extend_from_slice(PNG);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri("/v1/videos")
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "multipart/form-data; boundary=kb")
        .body(Body::from(body))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sent = env.mock.last().unwrap();
    assert!(
        sent.json()["image"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let id = serde_json::from_slice::<Value>(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    retrieve(&env, &id).await;
    retrieve(&env, &id).await;
    let billed = completions(&env).await;
    assert_eq!(
        billed.last().unwrap().media.as_ref().unwrap().input_images,
        1
    );
}

#[tokio::test]
async fn gemini_omni_makes_a_video_in_the_background_and_its_file_comes_inline() {
    let env = env().await;
    let r = create(&env, json!({ "model": "gemini-omni-1.2-flash", "prompt": "A marble rolling", "seconds": "8", "size": "1280x720" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1beta/interactions");
    assert_eq!(sent.header("x-goog-api-key"), Some(GOOGLE_SECRET));
    assert_eq!(sent.json()["background"], true);
    assert_eq!(
        sent.json()["response_format"],
        json!({ "type": "video", "aspect_ratio": "16:9", "resolution": "720p", "duration": "8s" })
    );
    retrieve(&env, &id).await;
    assert_eq!(retrieve(&env, &id).await.json()["status"], "completed");
    let file = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    assert_eq!(
        file.body,
        base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            "AAAAIGZ0eXBpc29t"
        )
        .unwrap()
    );
    let billed = completions(&env).await;
    assert_eq!(billed[0].usage.as_ref().unwrap().output_tokens, 29_285);
    let deleted = send(
        &env,
        "DELETE",
        &format!("/v1/videos/{id}"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(
        env.mock
            .received()
            .iter()
            .any(|r| r.method == "DELETE" && r.path.starts_with("/v1beta/interactions/"))
    );
}

#[tokio::test]
async fn gemini_omni_runs_on_the_agent_platforms_global_interactions_with_a_service_account() {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env_with_google_token(Some(capture)).await;
    let org = &env.owner.org_id;
    let key = kaeso_testkit::google::ServiceAccountKey::new(
        "kaeso@kaeso-example.iam.gserviceaccount.com",
        "kaeso-example",
    );
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "gemini-agent-platform", "name": "sa", "secret": key.json, "auth_method": "google_service_account", "location": "europe-west4" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({
                "provider": "gemini-agent-platform", "model_id": "gemini-omni-1.2-flash",
                "overrides": {
                    "input_modalities": ["text", "image"], "output_modalities": ["video"],
                    "context_limit": 32000, "output_limit": 32000,
                    "input_price": "1.5", "output_price": "12", "egress_dialect": "gemini"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = create(&env, json!({ "model": "gemini-omni-1.2-flash", "prompt": "A marble rolling", "seconds": "8", "size": "1280x720" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let interactions = "/v1beta1/projects/kaeso-example/locations/global/interactions";
    let sent = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path == interactions)
        .unwrap();
    assert!(
        sent.header("authorization")
            .unwrap()
            .starts_with("Bearer ya29.")
    );
    assert_eq!(sent.json()["background"], true);
    retrieve(&env, &id).await;
    assert_eq!(retrieve(&env, &id).await.json()["status"], "completed");
    assert!(
        env.mock
            .received()
            .iter()
            .any(|r| r.method == "GET" && r.path.starts_with(&format!("{interactions}/")))
    );
    let file = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    let deleted = send(
        &env,
        "DELETE",
        &format!("/v1/videos/{id}"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(
        !env.mock.received().iter().any(|r| r.method == "DELETE"),
        "the platform documents no delete, so Kaeso only forgets the video"
    );
}

#[tokio::test]
async fn veo_runs_on_the_agent_platform_with_a_service_account_and_without_a_price() {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env_with_google_token(Some(capture)).await;
    let org = &env.owner.org_id;
    let key = kaeso_testkit::google::ServiceAccountKey::new(
        "kaeso@kaeso-example.iam.gserviceaccount.com",
        "kaeso-example",
    );
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "gemini-agent-platform", "name": "sa", "secret": key.json, "auth_method": "google_service_account", "location": "us-central1" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "gemini-agent-platform", "model_id": "veo-3.1-generate-001" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = create(&env, json!({ "model": "veo-3.1-generate-001", "prompt": "A paper plane", "seconds": "8", "size": "1920x1080" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let sent = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path.ends_with(":predictLongRunning"))
        .unwrap();
    assert_eq!(
        sent.path,
        "/v1/projects/kaeso-example/locations/us-central1/publishers/google/models/veo-3.1-generate-001:predictLongRunning"
    );
    assert!(
        sent.header("authorization")
            .unwrap()
            .starts_with("Bearer ya29.")
    );
    assert_eq!(
        sent.json()["parameters"],
        json!({ "durationSeconds": 8, "aspectRatio": "16:9", "resolution": "1080p" })
    );
    retrieve(&env, &id).await;
    assert_eq!(retrieve(&env, &id).await.json()["status"], "completed");
    let poll = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path.ends_with(":fetchPredictOperation"))
        .unwrap();
    assert!(
        poll.json()["operationName"]
            .as_str()
            .unwrap()
            .contains("/operations/")
    );
    let file = send(
        &env,
        "GET",
        &format!("/v1/videos/{id}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    let billed = completions(&env).await;
    assert_eq!(billed.len(), 1);
    assert_eq!(
        kaeso_server::services::usage::costed(&billed[0]).1,
        rust_decimal::Decimal::ZERO
    );
    assert_eq!(billed[0].media.as_ref().unwrap().output_ms, 8000);
    assert!(
        !kaeso_server::services::usage::cost_known(&billed[0]),
        "without a price per clip, the cost is unknown, not 0"
    );
    let sums = kaeso_server::services::usage::media_sums(&billed[0]);
    assert_eq!((sums.unpriced, sums.videos, sums.video_ms), (1, 1, 8000));
    let started = env.outcomes().await;
    assert!(
        started
            .iter()
            .filter(|o| o.video != Some(VideoCall::Completed))
            .all(kaeso_server::services::usage::cost_known),
        "the request that starts a job is billed when the job ends"
    );
}

#[tokio::test]
async fn a_wan_video_runs_as_a_dashscope_task_at_qwencloud() {
    let env = env().await;
    let r = create(
        &env,
        json!({ "model": "wan2.7-t2v", "prompt": "A kitten running in the moonlight", "seconds": "10", "size": "1280x720" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    assert_eq!(r.json()["status"], "queued");
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/api/v1/services/aigc/video-generation/video-synthesis"
    );
    assert_eq!(sent.header("x-dashscope-async"), Some("enable"));
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {QWENCLOUD_SECRET}").as_str())
    );
    assert_eq!(
        sent.json(),
        json!({ "model": "wan2.7-t2v", "input": { "prompt": "A kitten running in the moonlight" }, "parameters": { "resolution": "720P", "ratio": "16:9", "duration": 10 } })
    );
    assert_eq!(retrieve(&env, &id).await.json()["status"], "in_progress");
    let done = retrieve(&env, &id).await.json();
    assert_eq!(done["status"], "completed");
    assert!(env.mock.received().iter().any(|r| r.path
        == "/api/v1/tasks/86ecf553-d340-4e21-af6e-a0c6a421c010"
        || r.path.starts_with("/api/v1/tasks/0385dc79")));
    let billed = completions(&env).await;
    assert_eq!(billed.len(), 1);
    let media = billed[0].media.clone().unwrap();
    assert_eq!(
        (
            media.output_videos,
            media.output_ms,
            media.resolution.as_deref()
        ),
        (1, 10_000, Some("720p"))
    );
    assert!(kaeso_server::services::usage::costed(&billed[0]).1 > rust_decimal::Decimal::ZERO);
    let refused = create(
        &env,
        json!({ "model": "wan2.7-t2v", "prompt": "A cat", "size": "1024x768" }),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{}",
        refused.text()
    );
    assert_eq!(refused.json()["error"]["param"], "size");
}

#[tokio::test]
async fn asking_about_a_video_counts_against_the_keys_request_rate() {
    let env = env().await;
    let org = &env.owner.org_id;
    let base = format!("/orgs/{org}/projects/{}", env.project);
    let key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "rated" }),
            Some(&env.owner.cookie),
        )
        .await
        .body;
    let rated = key["secret"].as_str().unwrap().to_owned();
    let r = env
        .app
        .post(
            &format!("{base}/limits"),
            json!({ "level": "key", "key": key["id"], "kind": "request_rate", "period": "minute", "amount": 3 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let made = send(
        &env,
        "POST",
        "/v1/videos",
        &rated,
        Some(json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" })),
        None,
    )
    .await;
    assert_eq!(made.status, StatusCode::OK, "{}", made.text());
    let id = made.json()["id"].as_str().unwrap().to_owned();
    let path = format!("/v1/videos/{id}");
    for _ in 0..2 {
        let r = send(&env, "GET", &path, &rated, None, None).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    let asked = env.mock.received().len();
    for (method, path) in [("GET", format!("{path}/content")), ("DELETE", path.clone())] {
        let r = send(&env, method, &path, &rated, None, None).await;
        assert_eq!(
            r.status,
            StatusCode::TOO_MANY_REQUESTS,
            "{method} {path}: {}",
            r.text()
        );
        assert_eq!(r.json()["error"]["type"], "rate_limit_error");
        assert!(r.headers.contains_key("retry-after"));
    }
    assert_eq!(
        env.mock.received().len(),
        asked,
        "the provider was not asked"
    );
    let r = send(&env, "GET", &path, &rated, None, None).await;
    assert_eq!(
        (r.status, r.json()["status"].as_str()),
        (StatusCode::OK, Some("completed")),
        "a finished video is answered from Kaeso's record without asking the provider"
    );
    let r = send(
        &env,
        "GET",
        &format!("{path}/content"),
        &env.key,
        None,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "another key is not held back");
}

#[tokio::test]
async fn a_key_sees_no_video_of_a_model_it_may_not_use() {
    let env = env().await;
    let id = create(
        &env,
        json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" }),
    )
    .await
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let models = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let other = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "cogvideox-3")
        .unwrap()["id"]
        .clone();
    let narrow = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "zai only", "models": [other] }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    for (method, path) in [
        ("GET", format!("/v1/videos/{id}")),
        ("GET", format!("/v1/videos/{id}/content")),
        ("DELETE", format!("/v1/videos/{id}")),
    ] {
        let r = send(&env, method, &path, &narrow, None, None).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            r.text()
        );
    }
    let listed = send(&env, "GET", "/v1/videos", &narrow, None, None)
        .await
        .json();
    assert_eq!(listed["data"].as_array().unwrap().len(), 0);
    assert_eq!(retrieve(&env, &id).await.status, StatusCode::OK);
}
