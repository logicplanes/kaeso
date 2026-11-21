#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::http::StatusCode;
use common::v1::{Answer, Env, call};
use kaeso_server::model_api::hooks::{Outcome, VideoCall};
use kaeso_testkit::mock_videos::FAL_REQUEST;
use rust_decimal::Decimal;
use serde_json::{Value, json};

const FAL_SECRET: &str = "fal-test-key-0000000000000000:000000000000000000000000";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    with_fal(common::v1::env(Some(capture), None).await).await
}

async fn charged_env() -> Env {
    with_fal(common::v1::env_charged(std::sync::Arc::new(common::v1::Capture::default())).await)
        .await
}

async fn with_fal(env: Env) -> Env {
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "fal", "name": "fal", "secret": FAL_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        "fal-ai/flux/dev",
        "fal-ai/flux-pro/v1.1",
        "bytedance/seedream/v5.5/text-to-image",
        "kling/v4/text-to-video",
        "fal-ai/veo3.1",
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "fal", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

async fn post(env: &Env, path: &str, body: Value) -> Answer {
    call(env, path, Some(&env.key), false, Some(body)).await
}

async fn get(env: &Env, path: &str) -> Answer {
    call(env, path, Some(&env.key), false, None).await
}

async fn last_outcome(env: &Env) -> Outcome {
    env.outcomes().await.into_iter().last().unwrap()
}

fn cost(o: &Outcome) -> Decimal {
    kaeso_server::services::usage::costed(o).1
}

fn received_since(env: &Env, before: usize) -> Vec<kaeso_testkit::mock_provider::Received> {
    env.mock.received().into_iter().skip(before).collect()
}

#[tokio::test]
async fn an_openai_image_request_follows_fals_queue_and_is_billed_per_megapixel() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "fal-ai/flux/dev", "prompt": "a sunset over mountains", "size": "1024x768", "n": 1 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["data"][0]["url"],
        "https://v3.fal.media/files/rabbit/abc123.png"
    );
    let sent = received_since(&env, before);
    let paths: Vec<(&str, &str)> = sent
        .iter()
        .map(|r| (r.method.as_str(), r.path.as_str()))
        .collect();
    let base = format!("/fal-ai/flux/requests/{FAL_REQUEST}");
    assert_eq!(
        paths,
        [
            ("POST", "/fal-ai/flux/dev"),
            ("GET", format!("{base}/status").as_str()),
            ("GET", base.as_str()),
        ]
    );
    for r in &sent {
        assert_eq!(
            r.header("authorization"),
            Some(format!("Key {FAL_SECRET}").as_str())
        );
    }
    assert_eq!(sent[0].header("x-fal-store-io"), Some("0"));
    assert_eq!(
        sent[0].json(),
        json!({ "prompt": "a sunset over mountains", "image_size": { "width": 1024, "height": 768 }, "num_images": 1 })
    );
    let o = last_outcome(&env).await;
    let media = o.media.as_ref().unwrap();
    assert_eq!(
        (media.output_images, media.output_pixels),
        (1, Some(1_048_576))
    );
    assert_eq!(cost(&o), "0.0262144".parse::<Decimal>().unwrap());
}

#[tokio::test]
async fn b64_json_asks_fal_for_data_uris_and_seedream_is_billed_per_image() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "fal-ai/flux-pro/v1.1", "prompt": "a red apple", "size": "1024x1024", "n": 2, "response_format": "b64_json" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let data = r.json()["data"].as_array().unwrap().clone();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["b64_json"], "iVBORw0KGgo=");
    assert_eq!(received_since(&env, before)[0].json()["sync_mode"], true);
    assert_eq!(
        cost(&last_outcome(&env).await),
        "0.08388608".parse::<Decimal>().unwrap()
    );

    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "bytedance/seedream/v5.5/text-to-image", "prompt": "a red apple", "size": "1024x1024" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        cost(&last_outcome(&env).await),
        "0.03".parse::<Decimal>().unwrap()
    );
}

#[tokio::test]
async fn what_fal_cannot_take_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    for (body, param) in [
        (
            json!({ "model": "fal-ai/flux/dev", "prompt": "a fox", "quality": "high" }),
            "quality",
        ),
        (
            json!({ "model": "fal-ai/flux/dev", "prompt": "a fox", "n": 5 }),
            "n",
        ),
        (
            json!({ "model": "fal-ai/flux/dev", "prompt": "a fox", "output_format": "webp" }),
            "output_format",
        ),
        (
            json!({ "model": "fal-ai/flux/dev", "prompt": "a fox", "user": "u-1" }),
            "user",
        ),
    ] {
        let r = post(&env, "/v1/images/generations", body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["param"], param, "{}", r.text());
    }
    let r = post(
        &env,
        "/v1/videos",
        json!({ "model": "kling/v4/text-to-video", "prompt": "a fox" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "seconds");
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn a_black_safety_image_and_a_failed_request_are_provider_errors() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "fal-ai/flux/dev", "prompt": "a fox [mock:fal=nsfw]" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("safety checker"),
        "{}",
        r.text()
    );
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "fal-ai/flux/dev", "prompt": "a fox [mock:fal=failed]" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert!(r.text().contains("Request timed out"), "{}", r.text());
}

#[tokio::test]
async fn a_video_job_is_created_polled_billed_and_fetched_through_fals_queue() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/videos",
        json!({ "model": "kling/v4/text-to-video", "prompt": "a paper boat", "seconds": "5", "size": "1280x720" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["status"], "queued");
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let sent = received_since(&env, before);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "/kling/v4/text-to-video");
    assert_eq!(
        sent[0].json(),
        json!({ "prompt": "a paper boat", "duration": 5, "resolution": "720p", "aspect_ratio": "16:9" })
    );
    assert_eq!(
        get(&env, &format!("/v1/videos/{id}")).await.json()["status"],
        "queued"
    );
    let done = get(&env, &format!("/v1/videos/{id}")).await;
    assert_eq!(done.json()["status"], "completed", "{}", done.text());
    let status = format!("/kling/v4/requests/{FAL_REQUEST}/status");
    assert!(env.mock.received().iter().any(|r| r.path == status));
    let billed: Vec<Outcome> = env
        .outcomes()
        .await
        .into_iter()
        .filter(|o| o.video == Some(VideoCall::Completed))
        .collect();
    assert_eq!(billed.len(), 1);
    assert_eq!(cost(&billed[0]), "0.785".parse::<Decimal>().unwrap());
    let file = get(&env, &format!("/v1/videos/{id}/content")).await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    assert!(!file.body.is_empty());
    let result = format!("/kling/v4/requests/{FAL_REQUEST}");
    assert!(env.mock.received().iter().any(|r| r.path == result));
}

#[tokio::test]
async fn veo_is_billed_with_sound_unless_it_is_turned_off() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/videos",
        json!({ "model": "fal-ai/veo3.1", "prompt": "a glacier", "seconds": "4", "size": "1920x1080" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.last().unwrap().json(),
        json!({ "prompt": "a glacier", "duration": "4s", "resolution": "1080p", "aspect_ratio": "16:9" })
    );
    let id = r.json()["id"].as_str().unwrap().to_owned();
    get(&env, &format!("/v1/videos/{id}")).await;
    assert_eq!(
        get(&env, &format!("/v1/videos/{id}")).await.json()["status"],
        "completed"
    );
    let billed: Vec<Outcome> = env
        .outcomes()
        .await
        .into_iter()
        .filter(|o| o.video == Some(VideoCall::Completed))
        .collect();
    assert_eq!(cost(&billed[0]), "1.6".parse::<Decimal>().unwrap());
    let r = post(
        &env,
        "/v1/videos",
        json!({ "model": "fal-ai/veo3.1", "prompt": "a glacier", "seconds": "5" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "seconds");
}

#[tokio::test]
async fn a_failed_video_frees_what_it_held_of_a_hard_budget() {
    let env = charged_env().await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project),
            json!({ "level": "project", "kind": "spend_budget", "period": "day", "amount": "2" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let video = |prompt: &str| json!({ "model": "kling/v4/text-to-video", "prompt": prompt, "seconds": "5", "size": "1280x720" });
    let r = post(&env, "/v1/videos", video("a paper boat [mock:fal=failed]")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let next = post(&env, "/v1/videos", video("a kite")).await;
    assert_eq!(
        next.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        next.text()
    );
    let mut status = Value::Null;
    for _ in 0..3 {
        status = get(&env, &format!("/v1/videos/{id}")).await.json()["status"].clone();
    }
    assert_eq!(status, "failed");
    let next = post(&env, "/v1/videos", video("a kite")).await;
    assert_eq!(next.status, StatusCode::OK, "{}", next.text());
}
