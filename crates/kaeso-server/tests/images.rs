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

async fn keep_media(env: &Env) {
    let org = &env.owner.org_id;
    for path in [
        format!("/orgs/{org}/logs/settings"),
        format!("/orgs/{org}/projects/{}/logs/settings", env.project),
    ] {
        let r = env
            .app
            .call(
                axum::http::Method::PATCH,
                &path,
                Some(json!({ "keep_media": true })),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }
}

async fn logged_content(env: &Env) -> Value {
    let o = outcome(env).await;
    let state = &env.app.state;
    let clickhouse = state.clickhouse.as_ref().unwrap();
    clickhouse.migrate(&state.pg).await.unwrap();
    kaeso_server::services::logs::record(state, &o).await;
    kaeso_server::services::logs::flush_all(state, std::time::Duration::from_secs(10)).await;
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/logs/{}",
                env.owner.org_id, env.project, o.request_id
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body.get("content").cloned().unwrap_or_default()
}

#[tokio::test]
async fn generated_and_uploaded_images_are_logged_whole_only_where_media_is_kept() {
    use base64::Engine as _;
    let sent = base64::engine::general_purpose::STANDARD.encode(PNG);
    let env = env().await;
    let ask = json!({ "model": "gpt-image-2", "prompt": "A cute baby sea otter" });
    let r = generate(&env, ask.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    assert_eq!(content["request"]["prompt"], "A cute baby sea otter");
    let left_out =
        &content["reply"]["image"]["images"][0]["source"]["base64"]["data"]["kaeso_media"];
    assert_eq!(left_out["reason"], "not_kept", "{content}");
    assert!(left_out["bytes"].as_u64().unwrap() > 0);

    let streamed = json!({ "model": "gpt-image-2", "prompt": "A red fox", "stream": true, "partial_images": 1 });
    let r = generate(&env, streamed.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    let images = content["reply"]["image"]["images"].as_array().unwrap();
    assert_eq!(images.len(), 1, "only the finished image: {content}");
    assert_eq!(
        images[0]["source"]["base64"]["data"]["kaeso_media"]["reason"],
        "not_kept"
    );

    keep_media(&env).await;
    generate(&env, streamed).await;
    let content = logged_content(&env).await;
    assert!(
        content["reply"]["image"]["images"][0]["source"]["base64"]["data"].is_string(),
        "{content}"
    );
    let r = generate(&env, ask).await;
    let made = r.json()["data"][0]["b64_json"].as_str().unwrap().to_owned();
    let content = logged_content(&env).await;
    assert_eq!(
        content["reply"]["image"]["images"][0]["source"]["base64"]["data"],
        made.as_str()
    );
    let r = upload(
        &env,
        "/v1/images/edits",
        &[("model", "gpt-image-2"), ("prompt", "Put a hat on it")],
        &[("image[]", PNG)],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    assert_eq!(
        content["request"]["images"][0]["source"]["base64"]["data"],
        sent.as_str()
    );
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
        kaeso_server::services::logs::endpoint(&o),
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
    assert_eq!(kaeso_server::services::logs::endpoint(&o), "images/edits");
    assert_eq!(o.media.as_ref().unwrap().input_images, 2);
    if let Some(logged) = &o.request_body {
        let text = String::from_utf8_lossy(logged);
        assert!(
            !text.contains("IHDR") && text.contains(r#""reason":"not_kept""#),
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

#[tokio::test]
async fn an_openai_app_gets_an_elevenlabs_image_once_its_generation_finishes() {
    let env = env().await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "elevenlabs", "name": "elevenlabs", "secret": "sk_test-elevenlabs-images-000000000000000" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "elevenlabs", "model_id": "gpt-image-1.5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);

    let r = generate(
        &env,
        json!({"model": "gpt-image-1.5", "prompt": "A lighthouse", "size": "1536x1024", "quality": "high", "response_format": "url"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.json()["data"][0]["url"]
            .as_str()
            .unwrap()
            .starts_with("http://"),
        "{}",
        r.text()
    );
    let calls: Vec<_> = env
        .mock
        .received()
        .into_iter()
        .filter(|c| c.path.starts_with("/v1/flows/image"))
        .collect();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].header("xi-api-key"),
        Some("sk_test-elevenlabs-images-000000000000000")
    );
    let body: Value = serde_json::from_slice(&calls[0].body).unwrap();
    assert_eq!(
        body,
        json!({"model_id": "gpt-image-1.5", "prompt": "A lighthouse", "quality": "high", "aspect_ratio": "3:2"})
    );
    assert!(
        calls.len() >= 3,
        "created, then polled until it finished: {calls:?}"
    );
    assert!(
        calls[1..]
            .iter()
            .all(|c| c.method == "GET" && c.path == "/v1/flows/image/JWr5N6X9ZTqf8jD2LmQb")
    );
    assert_eq!(outcome(&env).await.media.unwrap().output_images, 1);

    let r = generate(
        &env,
        json!({"model": "gpt-image-1.5", "prompt": "A lighthouse", "n": 2}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "n");
}

const ELEVENLABS_SECRET: &str = "sk_test-elevenlabs-images-000000000000000";

async fn with_elevenlabs(env: &Env) {
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "elevenlabs", "name": "elevenlabs", "secret": ELEVENLABS_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "elevenlabs", "model_id": "gpt-image-1.5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn flows(env: &Env, method: &str, path: &str, key: &str, body: Option<Value>) -> Answer {
    let req = Request::builder()
        .uri(path)
        .method(method)
        .header("xi-api-key", key);
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

async fn start(env: &Env, prompt: &str) -> String {
    let r = flows(
        env,
        "POST",
        "/v1/flows/image",
        &env.key,
        Some(json!({"model_id": "gpt-image-1.5", "prompt": prompt})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["id"].as_str().unwrap().to_owned()
}

async fn generation(env: &Env, id: &str) -> Value {
    let r = flows(env, "GET", &format!("/v1/flows/image/{id}"), &env.key, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()
}

async fn finished_generations(env: &Env) -> Vec<kaeso_server::model_api::hooks::Outcome> {
    env.outcomes()
        .await
        .into_iter()
        .filter(|o| kaeso_server::services::logs::endpoint(o) == "flows/image/completed")
        .collect()
}

fn sent_to_flows(env: &Env) -> Vec<kaeso_testkit::mock_provider::Received> {
    env.mock
        .received()
        .into_iter()
        .filter(|c| c.path.starts_with("/v1/flows/image"))
        .collect()
}

#[tokio::test]
async fn an_elevenlabs_app_keeps_its_image_generation_id_and_kaeso_bills_it_once_it_completes() {
    let env = env().await;
    with_elevenlabs(&env).await;
    let asked = json!({
        "model_id": "gpt-image-1.5",
        "prompt": "A lighthouse",
        "images": [{"type": "inline_base64", "content_base64": "iVBORw0KGgo=", "mime_type": "image/png"}],
        "quality": "high",
        "aspect_ratio": "3:2"
    });
    let r = flows(
        &env,
        "POST",
        "/v1/flows/image",
        &env.key,
        Some(asked.clone()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("imggen_"), "{id}");
    assert_eq!(r.json(), json!({"id": id, "status": "pending"}));
    let sent = sent_to_flows(&env);
    assert_eq!(sent.len(), 1, "Kaeso does not wait for the generation");
    assert_eq!(
        (sent[0].method.as_str(), sent[0].header("xi-api-key")),
        ("POST", Some(ELEVENLABS_SECRET))
    );
    assert_eq!(sent[0].json(), asked);
    let started = outcome(&env).await;
    assert_eq!(
        kaeso_server::services::logs::endpoint(&started),
        "flows/image"
    );
    assert_eq!(
        kaeso_server::services::usage::costed(&started).1,
        rust_decimal::Decimal::ZERO
    );
    assert!(kaeso_server::services::usage::cost_known(&started));

    let running = generation(&env, &id).await;
    assert_eq!(running, json!({"id": id, "status": "generating"}));
    assert!(finished_generations(&env).await.is_empty());
    let (a, b) = tokio::join!(generation(&env, &id), generation(&env, &id));
    for done in [&a, &b] {
        assert_eq!(done["id"], id);
        assert_eq!(done["status"], "completed");
        assert_eq!(done["content_mime_type"], "image/png");
        assert!(
            done["content_url"].as_str().unwrap().starts_with("http://"),
            "{done}"
        );
    }
    assert_eq!(generation(&env, &id).await["status"], "completed");
    let billed = finished_generations(&env).await;
    assert_eq!(billed.len(), 1, "two requests saw it end, one billed it");
    let media = billed[0].media.clone().unwrap();
    assert_eq!((media.input_images, media.output_images), (1, 1));
    assert_eq!(media.quality.as_deref(), Some("high"));
    assert_eq!(billed[0].requested_model.as_deref(), Some("gpt-image-1.5"));
    assert!(
        sent_to_flows(&env)[1..]
            .iter()
            .all(|c| c.method == "GET" && c.path == "/v1/flows/image/JWr5N6X9ZTqf8jD2LmQb")
    );
}

#[tokio::test]
async fn image_generations_answer_only_their_own_project_and_list_in_pages() {
    let env = env().await;
    with_elevenlabs(&env).await;
    let mut ids = Vec::new();
    for n in 0..3 {
        ids.push(start(&env, &format!("A lighthouse {n}")).await);
    }
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
    let other = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    for (key, id) in [
        (other.as_str(), ids[0].as_str()),
        (env.key.as_str(), "imggen_01k7a3f8m2xq4v9c5r6t8w0yz1"),
        (env.key.as_str(), "JWr5N6X9ZTqf8jD2LmQb"),
    ] {
        let r = flows(&env, "GET", &format!("/v1/flows/image/{id}"), key, None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{id}: {}", r.text());
        assert_eq!(r.json()["detail"]["type"], "not_found");
        assert!(
            r.json()["detail"]["message"]
                .as_str()
                .unwrap()
                .contains("belongs to this project")
        );
    }
    let elsewhere = flows(&env, "GET", "/v1/flows/image", &other, None).await;
    assert_eq!(
        elsewhere.json(),
        json!({"generations": [], "next_cursor": null, "has_more": false})
    );
    let pg = &env.app.state.pg;
    let generation: kaeso_core::id::ImageGenerationId = ids[0].parse().unwrap();
    let project: kaeso_core::id::ProjectId = env.project.parse().unwrap();
    let get = |o| kaeso_store::pg::image_generations::get(pg, o, project.uuid(), generation.uuid());
    let owner: kaeso_core::id::OrgId = org.parse().unwrap();
    assert!(get(owner.uuid()).await.unwrap().is_some());
    assert!(get(uuid::Uuid::new_v4()).await.unwrap().is_none());

    let listed = |query: String| {
        let env = &env;
        async move {
            flows(
                env,
                "GET",
                &format!("/v1/flows/image{query}"),
                &env.key,
                None,
            )
            .await
        }
    };
    let ids_of = |v: &Value| -> Vec<String> {
        v["generations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let page = listed("?page_size=2".into()).await.json();
    assert_eq!(ids_of(&page), [ids[2].clone(), ids[1].clone()]);
    assert_eq!(
        (page["has_more"].as_bool(), page["next_cursor"].as_str()),
        (Some(true), Some(ids[1].as_str()))
    );
    let rest = listed(format!("?page_size=2&cursor={}", ids[1]))
        .await
        .json();
    assert_eq!(ids_of(&rest), [ids[0].clone()]);
    assert_eq!(rest["has_more"], false);
    assert!(rest["next_cursor"].is_null());
    assert_eq!(
        rest["generations"][0],
        json!({"id": ids[0], "status": "pending"})
    );
    assert_eq!(
        ids_of(&listed("?status=pending".into()).await.json()).len(),
        3
    );
    assert!(ids_of(&listed("?status=completed".into()).await.json()).is_empty());
    assert_eq!(
        ids_of(&listed("?model_id=gpt-image-1.5".into()).await.json()).len(),
        3
    );
    assert!(ids_of(&listed("?model_id=gpt-image-1".into()).await.json()).is_empty());
    for (query, param) in [
        ("?page_size=0", "page_size"),
        ("?page_size=101", "page_size"),
        ("?status=queued", "status"),
        ("?cursor=imggen_01k7a3f8m2xq4v9c5r6t8w0yz1", "cursor"),
    ] {
        let r = listed(query.into()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{query}: {}", r.text());
        assert_eq!(r.json()["detail"]["param"], param, "{query}");
    }
    assert_eq!(
        flows(&env, "GET", "/v1/flows/image", "sk-kso-nonsense", None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn what_an_image_generation_cannot_do_is_refused_before_anything_is_sent() {
    let env = env().await;
    with_elevenlabs(&env).await;
    for (body, param) in [
        (
            json!({"model_id": "gpt-image-1.5", "prompt": "A lighthouse", "webhook": {"type": "all"}}),
            "webhook",
        ),
        (
            json!({"model_id": "gpt-image-2", "prompt": "A lighthouse"}),
            "model_id",
        ),
        (json!({"prompt": "A lighthouse"}), "model_id"),
        (
            json!({"model_id": "gpt-image-1.5", "prompt": "A lighthouse", "aspect_ratio": "16:9"}),
            "aspect_ratio",
        ),
    ] {
        let r = flows(
            &env,
            "POST",
            "/v1/flows/image",
            &env.key,
            Some(body.clone()),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {}", r.text());
        assert_eq!(r.json()["detail"]["param"], param, "{body}");
    }
    assert!(sent_to_flows(&env).is_empty());
    assert!(
        env.mock
            .received()
            .iter()
            .all(|c| !c.path.starts_with("/v1/images")),
        "another provider's model is never sent"
    );

    let id = start(&env, "A lighthouse [mock:generation=failed]").await;
    assert_eq!(generation(&env, &id).await["status"], "generating");
    let failed = generation(&env, &id).await;
    assert_eq!(
        failed,
        json!({
            "id": id,
            "status": "failed",
            "failure_reason": "timeout",
            "error_message": "Timed out while processing. You were not charged for this generation."
        })
    );
    let asked = sent_to_flows(&env).len();
    assert_eq!(generation(&env, &id).await, failed);
    assert_eq!(
        sent_to_flows(&env).len(),
        asked,
        "a failed generation is not asked again"
    );
    assert!(finished_generations(&env).await.is_empty());
}

#[tokio::test]
async fn a_key_sees_no_image_generation_of_a_model_it_may_not_use() {
    let env = env().await;
    with_elevenlabs(&env).await;
    let id = start(&env, "A lighthouse").await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let models = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let other = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "gpt-image-2")
        .unwrap()["id"]
        .clone();
    let narrow = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "openai only", "models": [other] }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();

    let r = flows(&env, "GET", &format!("/v1/flows/image/{id}"), &narrow, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let r = flows(&env, "GET", "/v1/flows/image", &narrow, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["generations"], json!([]));
    let r = flows(&env, "GET", "/v1/flows/image", &env.key, None).await;
    assert_eq!(r.json()["generations"][0]["id"], id.as_str());
}
