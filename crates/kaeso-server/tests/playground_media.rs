#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::v1::{Answer, Capture, Env, call};
use common::{App, ORIGIN, Owner};
use kaeso_server::model_api::hooks::{Outcome, Source};
use serde_json::{Value, json};

const XAI_SECRET: &str = "xai-test-playground-media-0000000000000000000";
const ELEVENLABS_SECRET: &str = "sk_test-elevenlabs-playground-000000000000";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";
const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    for (provider, secret) in [("xai", XAI_SECRET), ("elevenlabs", ELEVENLABS_SECRET)] {
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
        ("openai", "gpt-image-2"),
        ("openai", "tts-1"),
        ("openai", "whisper-1"),
        ("openai", "text-embedding-3-small"),
        ("elevenlabs", "eleven_text_to_sound_v2"),
        ("elevenlabs", "music_v1"),
        ("xai", "grok-imagine-video-1.5"),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert!(
            matches!(r.status, StatusCode::CREATED | StatusCode::CONFLICT),
            "{model}: {:?}",
            r.body
        );
    }
    env
}

fn path(env: &Env, rest: &str) -> String {
    format!(
        "/platform/v1/orgs/{}/projects/{}/playground/{rest}",
        env.owner.org_id, env.project
    )
}

async fn send(
    app: &App,
    method: Method,
    path: &str,
    cookie: &str,
    body: Option<(&str, Vec<u8>)>,
) -> Answer {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .header(header::ORIGIN, ORIGIN);
    let body = match body {
        Some((content_type, bytes)) => {
            req = req.header(header::CONTENT_TYPE, content_type);
            Body::from(bytes)
        }
        None => Body::empty(),
    };
    let (status, headers, body) = app.raw_bytes(req.body(body).unwrap()).await;
    Answer {
        status,
        headers,
        body,
    }
}

async fn post_json(env: &Env, rest: &str, body: Value) -> Answer {
    send(
        &env.app,
        Method::POST,
        &path(env, rest),
        &env.owner.cookie,
        Some(("application/json", serde_json::to_vec(&body).unwrap())),
    )
    .await
}

fn multipart(fields: &[(&str, &str)], file: (&str, &str, &str, &[u8])) -> (String, Vec<u8>) {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    let (field, filename, media_type, bytes) = file;
    body.extend_from_slice(
        format!("--kb\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: {media_type}\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    ("multipart/form-data; boundary=kb".to_owned(), body)
}

async fn upload(
    env: &Env,
    rest: &str,
    fields: &[(&str, &str)],
    file: (&str, &str, &str, &[u8]),
) -> Answer {
    let (content_type, body) = multipart(fields, file);
    send(
        &env.app,
        Method::POST,
        &path(env, rest),
        &env.owner.cookie,
        Some((&content_type, body)),
    )
    .await
}

async fn last(env: &Env) -> Outcome {
    env.outcomes().await.into_iter().last().unwrap()
}

fn by_member(env: &Env, o: &Outcome) {
    assert_eq!(o.source, Source::Playground);
    let caller = o.caller.as_ref().unwrap();
    assert_eq!(caller.key_id, None);
    assert_eq!(caller.user_id, Some(common::uuid(&env.owner.user_id)));
}

#[tokio::test]
async fn every_kind_of_model_is_listed_with_its_mode_and_what_the_playground_offers() {
    let env = env().await;
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/playground/models",
                env.owner.org_id, env.project
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let models = r.body["data"].as_array().unwrap();
    let of = |id: &str| {
        models
            .iter()
            .find(|m| m["model_id"] == id)
            .unwrap_or_else(|| panic!("{id} listed"))
    };
    assert_eq!(of("claude-haiku-4-5")["mode"], "text");
    assert_eq!(of("claude-haiku-4-5")["image"], Value::Null);
    let image = of("gpt-image-2");
    assert_eq!(image["mode"], "image");
    assert_eq!(image["image"]["edits"], true);
    assert_eq!(image["image"]["max_count"], 10);
    assert_eq!(image["image"]["size"], "pixels");
    assert!(
        image["image"]["qualities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q == "high")
    );
    assert_eq!(image["max_request_bytes"], json!(64u64 << 20));
    let speech = of("tts-1");
    assert_eq!(speech["mode"], "speech");
    assert_eq!(speech["max_request_bytes"], json!(1u64 << 20));
    assert_eq!(of("eleven_text_to_sound_v2")["durations"], true);
    assert!(
        speech["voices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "alloy")
    );
    let transcription = of("whisper-1");
    assert_eq!(transcription["mode"], "transcription");
    assert_eq!(transcription["translations"], true);
    assert_eq!(transcription["max_request_bytes"], json!(26u64 << 20));
    assert_eq!(of("eleven_text_to_sound_v2")["mode"], "sound_effects");
    assert_eq!(of("music_v1")["mode"], "music");
    assert_eq!(of("grok-imagine-video-1.5")["mode"], "video");
    assert!(
        models
            .iter()
            .all(|m| m["model_id"] != "text-embedding-3-small")
    );
}

#[tokio::test]
async fn a_member_makes_and_edits_images_and_hears_what_they_cost() {
    let env = env().await;
    let r = post_json(
        &env,
        "images/generations",
        json!({ "model": "gpt-image-2", "prompt": "A sea otter", "n": 1, "quality": "low" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["data"][0]["b64_json"].is_string());
    let cost = r.header("kaeso-cost").unwrap();
    assert!(cost.parse::<f64>().unwrap() > 0.0, "{cost}");
    let o = last(&env).await;
    by_member(&env, &o);
    assert_eq!(
        kaeso_server::services::logs::endpoint(&o),
        "images/generations"
    );

    let r = upload(
        &env,
        "images/edits",
        &[("model", "gpt-image-2"), ("prompt", "Add a hat")],
        ("image[]", "otter.png", "image/png", PNG),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        kaeso_server::services::logs::endpoint(&last(&env).await),
        "images/edits"
    );
    let sent = env.mock.last().unwrap();
    assert!(
        sent.header("cookie").is_none(),
        "the session never reaches the provider"
    );
}

#[tokio::test]
async fn speech_sound_effects_and_music_come_back_as_audio() {
    let env = env().await;
    let r = post_json(
        &env,
        "audio/speech",
        json!({ "model": "tts-1", "input": "Your parcel left the dock.", "voice": "alloy" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.header("content-type").unwrap().starts_with("audio/"));
    assert!(!r.body.is_empty());
    by_member(&env, &last(&env).await);

    let r = post_json(&env, "sound-effects", json!({ "text": "A short click.", "duration_seconds": 0.5, "model_id": "eleven_text_to_sound_v2" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.header("content-type").unwrap().starts_with("audio/"));
    let o = last(&env).await;
    by_member(&env, &o);
    assert_eq!(
        kaeso_server::services::logs::endpoint(&o),
        "sound-generation"
    );

    let r = post_json(
        &env,
        "music",
        json!({ "prompt": "A calm piano", "music_length_ms": 10000, "model_id": "music_v1" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "provider_error");
}

#[tokio::test]
async fn audio_is_transcribed_and_translated() {
    let env = env().await;
    let r = upload(
        &env,
        "audio/transcriptions",
        &[("model", "whisper-1")],
        ("file", "standup.mp3", "audio/mpeg", MP3),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["text"].as_str().is_some_and(|t| !t.is_empty()));
    by_member(&env, &last(&env).await);
    let r = upload(
        &env,
        "audio/translations",
        &[("model", "whisper-1")],
        ("file", "standup.mp3", "audio/mpeg", MP3),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        kaeso_server::services::logs::endpoint(&last(&env).await),
        "audio/translations"
    );
}

#[tokio::test]
async fn a_member_follows_downloads_and_cancels_their_own_videos_only() {
    let env = env().await;
    let r = post_json(
        &env,
        "videos",
        json!({ "model": "grok-imagine-video-1.5", "prompt": "A serene lake at sunrise" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    assert_eq!(r.json()["status"], "queued");
    by_member(&env, &last(&env).await);

    let get = |id: String| {
        let env = &env;
        async move {
            send(
                &env.app,
                Method::GET,
                &path(env, &format!("videos/{id}")),
                &env.owner.cookie,
                None,
            )
            .await
        }
    };
    let early = send(
        &env.app,
        Method::GET,
        &path(&env, &format!("videos/{id}/content")),
        &env.owner.cookie,
        None,
    )
    .await;
    assert_eq!(early.status, StatusCode::BAD_REQUEST, "{}", early.text());
    assert_eq!(early.json()["error"]["param"], "video");
    assert_eq!(get(id.clone()).await.json()["status"], "completed");
    let file = send(
        &env.app,
        Method::GET,
        &path(&env, &format!("videos/{id}/content")),
        &env.owner.cookie,
        None,
    )
    .await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    assert_eq!(file.header("content-type"), Some("video/mp4"));
    let charged = env
        .outcomes()
        .await
        .into_iter()
        .find(|o| kaeso_server::services::logs::endpoint(o) == "videos/completed")
        .expect("the finished job is charged");
    by_member(&env, &charged);

    let (other, _) = join(&env.app, &env.owner, "dev@example.com", "developer").await;
    let theirs = send(
        &env.app,
        Method::GET,
        &path(&env, &format!("videos/{id}")),
        &other,
        None,
    )
    .await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND, "{}", theirs.text());

    let by_key = call(
        &env,
        "/v1/videos",
        Some(&env.key),
        false,
        Some(json!({ "model": "grok-imagine-video-1.5", "prompt": "fox" })),
    )
    .await;
    let keyed = by_key.json()["id"].as_str().unwrap().to_owned();
    let keyed_id = keyed.clone();
    let hidden = get(keyed).await;
    assert_eq!(
        hidden.status,
        StatusCode::NOT_FOUND,
        "an app's videos stay out of the Playground"
    );
    let unknown = get("video_nope".to_owned()).await;
    assert_eq!(
        unknown.json()["error"]["message"],
        "No video of yours in this project has this id."
    );

    let app = |method: Method, rest: String| {
        let env = &env;
        async move {
            let req = Request::builder()
                .method(method)
                .uri(format!("/v1/videos{rest}"))
                .header(header::AUTHORIZATION, format!("Bearer {}", env.key))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"prompt":"again"}"#))
                .unwrap();
            env.app.raw_bytes(req).await
        }
    };
    for (method, rest) in [
        (Method::GET, format!("/{id}")),
        (Method::GET, format!("/{id}/content")),
        (Method::POST, format!("/{id}/remix")),
        (Method::DELETE, format!("/{id}")),
    ] {
        let (status, _, _) = app(method.clone(), rest.clone()).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{method} {rest}: an app never reaches a member's Playground video"
        );
    }
    let (status, _, listed) = app(Method::GET, "?limit=1".to_owned()).await;
    assert_eq!(status, StatusCode::OK);
    let listed: Value = serde_json::from_slice(&listed).unwrap();
    let ids: Vec<&str> = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        vec![keyed_id.as_str()],
        "a page of one skips the member's video"
    );
    assert_eq!(listed["has_more"], false);

    let bad = send(
        &env.app,
        Method::GET,
        &path(&env, &format!("videos/{id}/content?variant=gif")),
        &env.owner.cookie,
        None,
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json()["error"]["param"], "variant");
    let thumb = send(
        &env.app,
        Method::GET,
        &path(&env, &format!("videos/{id}/content?variant=thumbnail")),
        &env.owner.cookie,
        None,
    )
    .await;
    assert_eq!(thumb.status, StatusCode::NOT_FOUND);
    assert_eq!(
        thumb.json()["error"]["message"],
        "The provider keeps no file of this kind for the video."
    );

    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project),
            json!({ "level": "member", "user": env.owner.user_id, "kind": "request_rate", "period": "minute", "amount": 1 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for _ in 0..3 {
        let seek = send(
            &env.app,
            Method::GET,
            &path(&env, &format!("videos/{id}/content")),
            &env.owner.cookie,
            None,
        )
        .await;
        assert_eq!(
            seek.status,
            StatusCode::OK,
            "playing and seeking a finished video never runs into the rate: {}",
            seek.text()
        );
    }
    let limit = r.body["id"].as_str().unwrap().to_owned();
    let removed = send(
        &env.app,
        Method::DELETE,
        &format!(
            "/platform/v1/orgs/{}/projects/{}/limits/{limit}",
            env.owner.org_id, env.project
        ),
        &env.owner.cookie,
        None,
    )
    .await;
    assert!(removed.status.is_success(), "{}", removed.text());

    let deleted = send(
        &env.app,
        Method::DELETE,
        &path(&env, &format!("videos/{id}")),
        &env.owner.cookie,
        None,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.text());
    assert_eq!(get(id).await.status, StatusCode::NOT_FOUND);
}

async fn join(app: &App, owner: &Owner, email: &str, preset: &str) -> (String, String) {
    common::join_first_project(
        app,
        owner,
        json!({ "email": email, "preset": preset }),
        "Member",
    )
    .await
}

#[tokio::test]
async fn sessions_rights_and_body_limits_are_checked_before_anything_is_sent() {
    let env = common::v1::env(
        Some(Arc::new(Capture::default())),
        Some(&[("KAESO_BODY_LIMITS_AUDIO", "1KiB")]),
    )
    .await;
    env.app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "openai", "model_id": "whisper-1" }),
            Some(&env.owner.cookie),
        )
        .await;
    env.mock.clear();
    let big = upload(
        &env,
        "audio/transcriptions",
        &[("model", "whisper-1")],
        ("file", "long.mp3", "audio/mpeg", &[0u8; 4096]),
    )
    .await;
    assert_eq!(big.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", big.text());
    assert_eq!(big.json()["error"]["type"], "request_too_large");
    let long = post_json(
        &env,
        "audio/speech",
        json!({ "model": "tts-1", "input": "a".repeat(1 << 20), "voice": "alloy" }),
    )
    .await;
    assert_eq!(
        long.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{}",
        long.text()
    );

    let unknown = post_json(
        &env,
        "images/generations",
        json!({ "model": "no-such-model", "prompt": "x" }),
    )
    .await;
    assert_eq!(
        unknown.status,
        StatusCode::BAD_REQUEST,
        "{}",
        unknown.text()
    );
    assert_eq!(unknown.json()["error"]["param"], "model");

    let (viewer, _) = join(&env.app, &env.owner, "viewer@example.com", "member").await;
    let refused = send(
        &env.app,
        Method::POST,
        &path(&env, "audio/speech"),
        &viewer,
        Some((
            "application/json",
            br#"{"model":"tts-1","input":"Hi","voice":"alloy"}"#.to_vec(),
        )),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.text());

    let req = Request::builder()
        .method(Method::POST)
        .uri(path(&env, "audio/speech"))
        .header(header::AUTHORIZATION, format!("Bearer {}", env.key))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"tts-1","input":"Hi","voice":"alloy"}"#,
        ))
        .unwrap();
    let (status, _, _) = env.app.raw_bytes(req).await;
    assert!(
        matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        "{status}"
    );
    assert!(env.mock.received().is_empty());
}
