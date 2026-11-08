#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Env, call};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";
const XAI_SECRET: &str = "xai-test-credential-0000000000000000000000000000";
const GOOGLE_SECRET: &str = "AIzaSyTestGoogleImagesCredential000000000";
const ZAI_SECRET: &str = "zai-test-credential.00000000000000000000";
const MINIMAX_SECRET: &str = "sk-api-test-minimax-images-0000000000000000";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    for (provider, secret) in [
        ("xai", XAI_SECRET),
        ("google", GOOGLE_SECRET),
        ("zai", ZAI_SECRET),
        ("minimax", MINIMAX_SECRET),
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
        ("openai", "gpt-image-2"),
        ("xai", "grok-imagine-image"),
        ("google", "gemini-3.1-flash-image"),
        ("zai", "glm-image"),
        ("minimax", "image-01"),
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
    env
}

async fn generate(env: &Env, body: Value) -> Answer {
    call(
        env,
        "/v1/images/generations",
        Some(&env.key),
        false,
        Some(body),
    )
    .await
}

async fn upload(env: &Env, path: &str, fields: &[(&str, &str)], files: &[(&str, &[u8])]) -> Answer {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    for (name, bytes) in files {
        body.extend_from_slice(
            format!(
                "--kb\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--kb--\r\n");
    let req = Request::builder()
        .uri(path)
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
async fn an_openai_app_generates_images_with_a_gpt_image_model_and_is_billed_for_its_tokens() {
    let env = env().await;
    let r = generate(&env, json!({ "model": "gpt-image-2", "prompt": "A cute baby sea otter", "size": "1024x1024", "quality": "high" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert!(body["data"][0]["b64_json"].as_str().is_some());
    assert_eq!(body["usage"]["input_tokens_details"]["image_tokens"], 40);
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/images/generations");
    assert_eq!(
        sent.header("authorization"),
        Some("Bearer sk-test-openai-credential-0001")
    );
    assert_eq!(sent.json()["quality"], "high");
    let o = outcome(&env).await;
    assert_eq!(
        kaeso_server::services::logs::endpoint(o.ingress, o.kind, o.gemini, o.image),
        "images/generations"
    );
    let media = o.media.clone().unwrap();
    assert_eq!((media.input_images, media.output_images), (0, 1));
    assert_eq!(media.size.as_deref(), Some("1024x1024"));
    let usage = o.usage.clone().unwrap();
    assert_eq!(usage.input_image_tokens, Some(40));
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert!(cost > rust_decimal::Decimal::ZERO, "{cost}");
}

#[tokio::test]
async fn an_upload_with_a_mask_reaches_openai_as_an_upload_and_is_logged_without_its_bytes() {
    let env = env().await;
    let r = upload(
        &env,
        "/v1/images/edits",
        &[
            ("model", "gpt-image-2"),
            ("prompt", "Put a hat on it"),
            ("n", "1"),
        ],
        &[("image[]", PNG), ("image[]", PNG), ("mask", PNG)],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/images/edits");
    let ct = sent.header("content-type").unwrap().to_owned();
    assert!(ct.starts_with("multipart/form-data; boundary="), "{ct}");
    let boundary = kaeso_codecs::multipart::form_boundary(&ct).unwrap();
    let parts = kaeso_codecs::multipart::read_form(
        &sent.body,
        &boundary,
        kaeso_codecs::multipart::FormLimits::new(64, 1 << 20),
    )
    .unwrap();
    let files: Vec<&str> = parts
        .iter()
        .filter(|p| p.filename.is_some())
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(files, ["image[]", "image[]", "mask"]);
    assert!(
        parts
            .iter()
            .filter(|p| p.filename.is_some())
            .all(|p| p.body == PNG)
    );
    let o = outcome(&env).await;
    assert_eq!(
        kaeso_server::services::logs::endpoint(o.ingress, o.kind, o.gemini, o.image),
        "images/edits"
    );
    assert_eq!(o.media.as_ref().unwrap().input_images, 2);
    if let Some(logged) = &o.request_body {
        let text = String::from_utf8_lossy(logged);
        assert!(
            !text.contains("IHDR") && text.contains("not logged"),
            "{text}"
        );
    }
}

#[tokio::test]
async fn a_streamed_generation_brings_partial_and_finished_images() {
    let env = env().await;
    let r = generate(&env, json!({ "model": "gpt-image-2", "prompt": "A red fox", "stream": true, "partial_images": 1 })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(
        text.contains("event: image_generation.partial_image"),
        "{text}"
    );
    assert!(text.contains("event: image_generation.completed"), "{text}");
    assert_eq!(env.mock.last().unwrap().json()["stream"], true);
    let o = outcome(&env).await;
    assert_eq!(o.media.unwrap().output_images, 1);
    assert_eq!(o.usage.unwrap().output_tokens, 50);
    assert!(!o.usage_estimated);
}

#[tokio::test]
async fn a_gemini_image_model_answers_an_openai_app_and_a_stream_it_cannot_send() {
    let env = env().await;
    let prompt = "A red circle [mock:fixture=image-output]";
    let r = generate(
        &env,
        json!({ "model": "gemini-3.1-flash-image", "prompt": prompt, "size": "1024x1024" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["data"][0]["b64_json"].as_str().is_some());
    assert_eq!(
        r.json()["usage"]["output_tokens_details"]["image_tokens"],
        1290
    );
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/v1beta/models/gemini-3.1-flash-image:generateContent"
    );
    assert_eq!(
        sent.json()["generationConfig"]["imageConfig"],
        json!({"aspectRatio": "1:1", "imageSize": "1K"})
    );
    let r = generate(
        &env,
        json!({ "model": "gemini-3.1-flash-image", "prompt": prompt, "stream": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.header("content-type")
            .unwrap()
            .starts_with("text/event-stream")
    );
    assert!(
        r.text().starts_with("event: image_generation.completed\n"),
        "{}",
        r.text()
    );
    let r = generate(
        &env,
        json!({ "model": "gemini-3.1-flash-image", "prompt": prompt, "size": "1536x1024" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"]["param"], "size");
}

#[tokio::test]
async fn spacex_zai_and_minimax_get_image_requests_in_their_own_forms() {
    let env = env().await;
    let r = generate(
        &env,
        json!({ "model": "grok-imagine-image", "prompt": "A fox", "aspect_ratio": "16:9" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(
        (sent.path.as_str(), sent.header("authorization")),
        (
            "/v1/images/generations",
            Some(format!("Bearer {XAI_SECRET}").as_str())
        )
    );
    assert_eq!(sent.json()["aspect_ratio"], "16:9");
    let r = generate(
        &env,
        json!({ "model": "grok-imagine-image", "prompt": "A fox", "size": "1024x1024" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"]["param"], "size");
    let r = generate(
        &env,
        json!({ "model": "glm-image", "prompt": "A fox", "quality": "hd" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/api/paas/v4/images/generations");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {ZAI_SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    let r = generate(
        &env,
        json!({ "model": "image-01", "prompt": "A fox", "n": 3 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["data"].as_array().unwrap().len(), 3);
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/image_generation");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {MINIMAX_SECRET}").as_str())
    );
    let o = outcome(&env).await;
    assert_eq!(o.media.clone().unwrap().output_images, 3);
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(
        cost.to_string(),
        "0.0105",
        "three images at MiniMax's price"
    );
}

#[tokio::test]
async fn image_requests_are_refused_where_they_cannot_be_served() {
    let env = env().await;
    let r = generate(
        &env,
        json!({ "model": "claude-haiku-4-5", "prompt": "A fox" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.json()["error"]["message"],
        "Anthropic offers no image API Kaeso can call, so `claude-haiku-4-5` can't make images."
    );
    let r = generate(&env, json!({ "model": "gpt-4o", "prompt": "A fox" })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.json()["error"]["message"],
        "The model `gpt-4o` does not produce images."
    );
    let r = upload(
        &env,
        "/v1/images/variations",
        &[("model", "gpt-image-2")],
        &[("image", PNG)],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("/v1/images/edits")
    );
    let r = generate(&env, json!({ "model": "gpt-image-2" })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"]["param"], "prompt");
    assert_eq!(env.mock.received().len(), 0, "nothing reached the provider");
}

#[tokio::test]
async fn a_responses_app_keeps_openais_image_generation_tool_and_other_targets_refuse_it() {
    let env = env().await;
    let body = |model: &str| {
        json!({
            "model": model,
            "input": "Draw a fox",
            "tools": [{ "type": "image_generation", "quality": "low", "partial_images": 1 }]
        })
    };
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(body("gpt-4o")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/responses");
    assert_eq!(
        sent.json()["tools"][0],
        json!({ "type": "image_generation", "quality": "low", "partial_images": 1 })
    );
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(body("claude-haiku-4-5")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("image_generation"), "{}", r.text());
}

#[tokio::test]
async fn the_agent_platform_takes_image_requests_on_its_own_paths_with_output_options() {
    let env = env().await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "gemini-agent-platform", "name": "express", "secret": "AQ.TestExpressModeKey0000000000000000" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "gemini-agent-platform", "model_id": "gemini-3.1-flash-image" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = generate(
        &env,
        json!({
            "model": "gemini-agent-platform/gemini-3.1-flash-image",
            "prompt": "A red circle [mock:fixture=image-output]",
            "output_format": "jpeg",
            "output_compression": 80
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/v1/publishers/google/models/gemini-3.1-flash-image:generateContent"
    );
    assert_eq!(
        sent.json()["generationConfig"]["imageConfig"]["imageOutputOptions"],
        json!({ "mimeType": "image/jpeg", "compressionQuality": 80 })
    );
}
