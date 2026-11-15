#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::http::StatusCode;
use common::v1::{Answer, Env, call};
use kaeso_server::model_api::hooks::{Outcome, VideoCall};
use kaeso_testkit::mock_videos::REPLICATE_PREDICTION;
use rust_decimal::Decimal;
use serde_json::{Value, json};

const SECRET: &str = "r8_kaesotest00000000000000000000000000000";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "replicate", "name": "replicate", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        "black-forest-labs/flux-schnell",
        "black-forest-labs/flux-1.1-pro",
        "google/nano-banana-pro",
        "google/veo-3.1",
        "kwaivgi/kling-v2.5-turbo-pro",
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "replicate", "model_id": model }),
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
async fn an_openai_image_request_is_one_prediction_with_a_wait_and_is_billed_per_image() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a lighthouse", "aspect_ratio": "16:9", "n": 1, "output_format": "png" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.json()["data"][0]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://replicate.delivery/"),
        "{}",
        r.text()
    );
    let sent = received_since(&env, before);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].method.as_str(), sent[0].path.as_str()),
        (
            "POST",
            "/v1/models/black-forest-labs/flux-schnell/predictions"
        )
    );
    assert_eq!(
        sent[0].header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent[0].header("prefer"), Some("wait=60"));
    assert_eq!(
        sent[0].json(),
        json!({ "input": { "prompt": "a lighthouse", "aspect_ratio": "16:9", "num_outputs": 1, "output_format": "png" } })
    );
    let o = last_outcome(&env).await;
    assert_eq!(o.provider, Some("replicate"));
    assert_eq!(o.media.as_ref().unwrap().output_images, 1);
    assert_eq!(cost(&o), "0.003".parse::<Decimal>().unwrap());
}

#[tokio::test]
async fn a_prediction_that_outlasts_its_wait_is_polled_until_it_ends() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a lighthouse [mock:replicate=wait]" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let paths: Vec<(String, String)> = received_since(&env, before)
        .into_iter()
        .map(|r| (r.method, r.path))
        .collect();
    let id = format!("{REPLICATE_PREDICTION}-flux-schnell-wait");
    assert_eq!(
        paths,
        [
            (
                "POST".to_owned(),
                "/v1/models/black-forest-labs/flux-schnell/predictions".to_owned()
            ),
            ("GET".to_owned(), format!("/v1/predictions/{id}")),
        ]
    );
}

#[tokio::test]
async fn sizes_and_resolutions_reach_each_model_as_its_input_and_4k_costs_more() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "black-forest-labs/flux-1.1-pro", "prompt": "a lighthouse", "size": "1024x768", "output_format": "jpeg" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.last().unwrap().json()["input"],
        json!({ "prompt": "a lighthouse", "aspect_ratio": "custom", "width": 1024, "height": 768, "output_format": "jpg" })
    );
    assert_eq!(
        cost(&last_outcome(&env).await),
        "0.04".parse::<Decimal>().unwrap()
    );
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "google/nano-banana-pro", "prompt": "a lighthouse", "aspect_ratio": "16:9", "resolution": "4K" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.last().unwrap().json()["input"],
        json!({ "prompt": "a lighthouse", "aspect_ratio": "16:9", "resolution": "4K" })
    );
    assert_eq!(
        cost(&last_outcome(&env).await),
        "0.3".parse::<Decimal>().unwrap()
    );
}

#[tokio::test]
async fn what_a_replicate_model_cannot_take_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    for (body, param) in [
        (
            json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a fox", "size": "1024x1024" }),
            "size",
        ),
        (
            json!({ "model": "black-forest-labs/flux-1.1-pro", "prompt": "a fox", "n": 2 }),
            "n",
        ),
        (
            json!({ "model": "google/nano-banana-pro", "prompt": "a fox", "quality": "high" }),
            "quality",
        ),
        (
            json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a fox", "user": "u-1" }),
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
        json!({ "model": "kwaivgi/kling-v2.5-turbo-pro", "prompt": "a fox", "seconds": "8" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "seconds");
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn bytes_are_fetched_only_from_replicates_file_hosts_and_a_failed_prediction_is_an_error() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a fox [mock:replicate=foreign]", "response_format": "b64_json" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Replicate answered with a link Kaeso does not fetch."
    );
    assert!(
        !env.mock
            .received()
            .iter()
            .any(|r| r.path.contains("out.png")),
        "the link was not followed"
    );
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "black-forest-labs/flux-schnell", "prompt": "a fox [mock:replicate=failed]" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert!(r.text().contains("E6716"), "{}", r.text());
}

#[tokio::test]
async fn a_video_job_is_a_prediction_polled_billed_per_second_and_fetched_from_its_link() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = post(
        &env,
        "/v1/videos",
        json!({ "model": "google/veo-3.1", "prompt": "a paper boat", "seconds": "8", "size": "1280x720" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["status"], "queued");
    let id = r.json()["id"].as_str().unwrap().to_owned();
    let sent = received_since(&env, before);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "/v1/models/google/veo-3.1/predictions");
    assert_eq!(sent[0].header("prefer"), None);
    assert_eq!(
        sent[0].json(),
        json!({ "input": { "prompt": "a paper boat", "duration": 8, "resolution": "720p", "aspect_ratio": "16:9" } })
    );
    assert_eq!(
        get(&env, &format!("/v1/videos/{id}")).await.json()["status"],
        "in_progress"
    );
    let done = get(&env, &format!("/v1/videos/{id}")).await;
    assert_eq!(done.json()["status"], "completed", "{}", done.text());
    let poll = format!("/v1/predictions/{REPLICATE_PREDICTION}-veo-3.1");
    assert!(env.mock.received().iter().any(|r| r.path == poll));
    let billed: Vec<Outcome> = env
        .outcomes()
        .await
        .into_iter()
        .filter(|o| o.video == Some(VideoCall::Completed))
        .collect();
    assert_eq!(billed.len(), 1);
    assert_eq!(cost(&billed[0]), "3.2".parse::<Decimal>().unwrap());
    let file = get(&env, &format!("/v1/videos/{id}/content")).await;
    assert_eq!(file.status, StatusCode::OK, "{}", file.text());
    assert!(!file.body.is_empty());
}

#[tokio::test]
async fn veo_is_billed_with_sound_by_default_and_kling_per_second() {
    let env = env().await;
    for (model, seconds, usd) in [
        ("google/veo-3.1", "4", "1.6"),
        ("kwaivgi/kling-v2.5-turbo-pro", "5", "0.35"),
    ] {
        let r = post(
            &env,
            "/v1/videos",
            json!({ "model": model, "prompt": "a glacier", "seconds": seconds }),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(
            env.mock.last().unwrap().json()["input"],
            json!({ "prompt": "a glacier", "duration": seconds.parse::<u32>().unwrap() })
        );
        let id = r.json()["id"].as_str().unwrap().to_owned();
        get(&env, &format!("/v1/videos/{id}")).await;
        assert_eq!(
            get(&env, &format!("/v1/videos/{id}")).await.json()["status"],
            "completed"
        );
        let billed = env
            .outcomes()
            .await
            .into_iter()
            .filter(|o| o.video == Some(VideoCall::Completed))
            .last()
            .unwrap();
        assert_eq!(cost(&billed), usd.parse::<Decimal>().unwrap(), "{model}");
    }
}
