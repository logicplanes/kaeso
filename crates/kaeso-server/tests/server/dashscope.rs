#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Env, call};
use http_body_util::BodyExt;
use kaeso_testkit::mock_provider::DASHSCOPE_LINK;
use serde_json::{Value, json};
use tower::ServiceExt;

const QWENCLOUD_SECRET: &str = "sk-test-qwencloud-media-000000000000000";
const GENERATION: &str = "/api/v1/services/aigc/multimodal-generation/generation";
const WAV: &[u8] = b"RIFF\x24\0\0\0WAVEfmt ";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "alibaba", "name": "QwenCloud", "secret": QWENCLOUD_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        "qwen-image-3.0",
        "qwen-image-2.0",
        "wan2.7-image",
        "z-image-turbo",
        "qwen-image-3.0-pro",
        "wan2.6-t2i",
        "wan2.2-t2i-flash",
        "qwen-audio-3.0-tts-flash",
        "qwen3-tts-flash",
        "qwen-audio-3.0-asr-flash",
        "qwen3-asr-flash-filetrans",
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "alibaba", "model_id": model }),
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

async fn transcribe(env: &Env, model: &str, extra: &[(&str, &str)]) -> Answer {
    let mut body = Vec::new();
    for (name, value) in [("model", model)].iter().chain(extra) {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        b"--kb\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
    );
    body.extend_from_slice(WAV);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri("/v1/audio/transcriptions")
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "multipart/form-data; boundary=kb")
        .body(Body::from(body))
        .unwrap();
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

async fn outcome(env: &Env) -> kaeso_server::model_api::hooks::Outcome {
    env.outcomes().await.into_iter().last().unwrap()
}

#[tokio::test]
async fn qwencloud_image_models_reach_dashscope_or_the_openai_compatible_mode_by_model() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "wan2.7-image", "prompt": "A red fox", "size": "1024x1024", "n": 1 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["data"][0]["url"], DASHSCOPE_LINK);
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, GENERATION);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {QWENCLOUD_SECRET}").as_str())
    );
    assert_eq!(
        sent.json(),
        json!({
            "model": "wan2.7-image",
            "input": {"messages": [{"role": "user", "content": [{"text": "A red fox"}]}]},
            "parameters": {"n": 1, "size": "1024*1024"},
        })
    );
    let o = outcome(&env).await;
    assert_eq!(o.media.as_ref().unwrap().output_images, 1);
    assert!(kaeso_server::services::usage::costed(&o).1 > rust_decimal::Decimal::ZERO);

    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "z-image-turbo", "prompt": "A red fox" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["data"][0]["revised_prompt"],
        "Photo of a stylish young woman..."
    );

    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "qwen-image-3.0", "prompt": "A red fox" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/images/generations");
}

#[tokio::test]
async fn bytes_are_fetched_only_from_qwenclouds_result_hosts() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "qwen-image-2.0", "prompt": "A red fox", "response_format": "b64_json" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "QwenCloud answered with a link Kaeso does not fetch."
    );
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, GENERATION);
    assert!(sent.json()["parameters"].get("response_format").is_none());
    assert!(
        !env.mock
            .received()
            .iter()
            .any(|r| r.path.contains("result.png")),
        "the link was not followed"
    );
}

#[tokio::test]
async fn qwen_speech_streams_its_audio_and_is_billed_per_character() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/audio/speech",
        json!({ "model": "qwen-audio-3.0-tts-flash", "input": "你好，今天的天气真不错。", "voice": "longanhuan_v3.6", "response_format": "wav" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("content-type"), Some("audio/wav"));
    assert!(r.body.starts_with(b"RIFF"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/api/v1/services/audio/tts/SpeechSynthesizer");
    assert_eq!(sent.header("x-dashscope-sse"), Some("enable"));
    assert_eq!(sent.json()["input"]["format"], "wav");
    let o = outcome(&env).await;
    assert_eq!(
        o.media.as_ref().unwrap().characters,
        22,
        "the billed characters"
    );

    let r = post(
        &env,
        "/v1/audio/speech",
        json!({ "model": "qwen3-tts-flash", "input": "Hi", "voice": "Cherry", "response_format": "pcm", "stream_format": "sse" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.text().matches("speech.audio.delta").count(), 2);

    let r = post(
        &env,
        "/v1/audio/speech",
        json!({ "model": "qwen3-tts-flash", "input": "Hi", "voice": "Cherry", "response_format": "wav" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert!(
        env.mock.last().unwrap().header("x-dashscope-sse").is_none(),
        "a wav reply is asked for whole"
    );

    let r = post(
        &env,
        "/v1/audio/speech",
        json!({ "model": "qwen3-tts-flash", "input": "Hi", "voice": "alloy", "response_format": "wav" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "voice");
}

#[tokio::test]
async fn qwen_audio_asr_transcribes_through_dashscope_and_is_billed_by_its_seconds() {
    let env = env().await;
    let r = transcribe(&env, "qwen-audio-3.0-asr-flash", &[("language", "en")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["text"], "Hello World, this is Alibaba Speech Lab.");
    let sent = env.mock.last().unwrap().json();
    assert_eq!(
        sent["parameters"],
        json!({"format": "wav", "language_hints": ["en"]})
    );
    assert!(
        sent["input"]["messages"][0]["content"][0]["input_audio"]["data"]
            .as_str()
            .unwrap()
            .starts_with("data:audio/wav;base64,")
    );
    let o = outcome(&env).await;
    assert_eq!(o.media.as_ref().unwrap().input_ms, 4000);

    let before = env.mock.received().len();
    let r = transcribe(&env, "qwen3-asr-flash-filetrans", &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

fn cost(o: &kaeso_server::model_api::hooks::Outcome) -> rust_decimal::Decimal {
    kaeso_server::services::usage::costed(o).1
}

#[tokio::test]
async fn wan_text_to_image_waits_for_its_task_and_kpmr_prices_bill_each_image() {
    let env = env().await;
    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "wan2.2-t2i-flash", "prompt": "A red fox", "size": "1024x1024" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["data"].as_array().unwrap().len(), 1);
    assert_eq!(r.json()["data"][0]["url"], DASHSCOPE_LINK);
    let sent = env.mock.received();
    let started = &sent[sent.len() - 2];
    assert_eq!(
        started.path,
        "/api/v1/services/aigc/text2image/image-synthesis"
    );
    assert_eq!(started.header("x-dashscope-async"), Some("enable"));
    assert_eq!(
        started.json(),
        json!({"model": "wan2.2-t2i-flash", "input": {"prompt": "A red fox"}, "parameters": {"n": 1, "size": "1024*1024"}})
    );
    let polled = env.mock.last().unwrap();
    assert_eq!(
        polled.path,
        format!(
            "/api/v1/tasks/{}",
            kaeso_testkit::mock_provider::DASHSCOPE_IMAGE_TASK
        )
    );
    assert_eq!(
        polled.header("authorization"),
        Some(format!("Bearer {QWENCLOUD_SECRET}").as_str())
    );
    let o = outcome(&env).await;
    assert_eq!(o.media.as_ref().unwrap().output_images, 1);
    assert_eq!(cost(&o), "0.025".parse::<rust_decimal::Decimal>().unwrap());

    let r = post(
        &env,
        "/v1/images/generations",
        json!({ "model": "wan2.6-t2i", "prompt": "A red fox", "size": "1280x1280" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, GENERATION);
    assert_eq!(
        cost(&outcome(&env).await),
        "0.03".parse::<rust_decimal::Decimal>().unwrap()
    );
}

#[tokio::test]
async fn z_image_costs_twice_as_much_with_a_rewritten_prompt_and_qwen_image_by_its_tier() {
    let env = env().await;
    for (extend, price) in [(false, "0.015"), (true, "0.03")] {
        let r = post(
            &env,
            "/v1/images/generations",
            json!({ "model": "z-image-turbo", "prompt": "A red fox", "prompt_extend": extend }),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(
            env.mock.last().unwrap().json()["parameters"]["prompt_extend"],
            extend
        );
        assert_eq!(
            cost(&outcome(&env).await),
            price.parse::<rust_decimal::Decimal>().unwrap(),
            "prompt_extend {extend}"
        );
    }
    for (fixture, price) in [("generate", "0.04"), ("generate-2k", "0.075")] {
        let r = post(
            &env,
            "/v1/images/generations",
            json!({ "model": "qwen-image-3.0-pro", "prompt": format!("A red fox [mock:fixture=providers/alibaba/{fixture}]") }),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        let o = outcome(&env).await;
        assert_eq!(
            o.media.as_ref().unwrap().resolution.as_deref(),
            Some(if fixture == "generate" { "1K" } else { "2K" })
        );
        assert_eq!(
            cost(&o),
            price.parse::<rust_decimal::Decimal>().unwrap(),
            "{fixture}"
        );
    }
}

#[tokio::test]
async fn an_elevenlabs_sound_effect_reaches_qwen_audio_tts_next_as_a_described_scene() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "alibaba", "model_id": "qwen-audio-3.1-tts-next" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let sound = |body: Value| {
        Request::builder()
            .uri("/v1/sound-generation?output_format=mp3_44100_128")
            .method("POST")
            .header("xi-api-key", &env.key)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let before = env.mock.received().len();
    let resp = env
        .app
        .router
        .clone()
        .oneshot(sound(json!({
            "text": "Rain on a tin roof", "model_id": "qwen-audio-3.1-tts-next", "duration_seconds": 3
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["detail"]["param"], "duration", "{error}");
    assert_eq!(
        env.mock.received().len(),
        before,
        "refused before anything is sent"
    );

    let _ = env
        .app
        .router
        .clone()
        .oneshot(sound(
            json!({ "text": "Rain on a tin roof", "model_id": "qwen-audio-3.1-tts-next" }),
        ))
        .await
        .unwrap();
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/api/v1/services/audio/tts/SpeechSynthesizer");
    assert_eq!(sent.header("x-dashscope-sse"), None);
    let body: Value = serde_json::from_slice(&sent.body).unwrap();
    assert_eq!(
        body,
        json!({"model": "qwen-audio-3.1-tts-next", "input": {
            "text_prompt": "Rain on a tin roof", "format": "mp3", "sample_rate": 44100,
            "enable_cbr": true, "bit_rate": 128
        }})
    );
}
