#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env, call, chat, messages};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const SECRET: &str = "00000000-0000-4000-8000-000000000001";
const GPT_OSS: &str = "gpt-oss-120b";
const QWEN: &str = "qwen3.5-397b-a17b";
const EMBED: &str = "bge-multilingual-gemma2";
const WHISPER: &str = "scaleway/whisper-large-v3";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/scaleway/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/scaleway/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "scaleway", "name": "scaleway", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GPT_OSS, QWEN, EMBED, "whisper-large-v3"] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "scaleway", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

fn sent_to_scaleway(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

async fn upload(env: &Env, path: &str, fields: &[(&str, &str)]) -> Answer {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        b"--kb\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n",
    );
    body.extend_from_slice(MP3);
    body.extend_from_slice(b"\r\n--kb--\r\n");
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

#[tokio::test]
async fn chat_apps_reach_scaleway_with_developer_messages_as_system_and_get_its_reasoning() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GPT_OSS,
            "messages": [
                { "role": "developer", "content": "Be vivid." },
                { "role": "user", "content": format!("Describe a futuristic city. {}", fixture("responses", "reasoning")) }
            ],
            "reasoning_effort": "medium",
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("scaleway"));
    assert!(
        r.json()["choices"][0]["message"]["reasoning_content"]
            .as_str()
            .unwrap()
            .starts_with("The user wants a description")
    );
    let sent = sent_to_scaleway(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert!(
        sent.get("safety_identifier").is_none() && sent.get("user").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_scaleways_streamed_reasoning_as_thinking() {
    let env = env().await;
    let mut body = messages(
        QWEN,
        &format!("Hello! {}", fixture("streams", "reasoning-stream")),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let blocks: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "content_block_start")
        .map(|e| &e["content_block"]["type"])
        .collect();
    assert_eq!(blocks, [&json!("thinking"), &json!("text")]);
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 17);
}

#[tokio::test]
async fn a_missing_key_is_refused_and_a_key_scaleway_doesnt_know_is_not_allowed() {
    let env = env().await;
    for (name, message) in [
        (
            "recorded-missing-api-key",
            "The provider refused this project's provider key. Ask an organization admin to check it under Providers.",
        ),
        (
            "recorded-invalid-api-key",
            "The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers.",
        ),
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GPT_OSS, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        assert_eq!(r.json()["error"]["message"], message, "{name}");
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("scaleway"));
    assert!(!format!("{o:?}").contains(SECRET));
}

#[tokio::test]
async fn scaleway_embeds_transcribes_to_json_and_refuses_what_it_lacks() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": "Embeddings can represent text in a numerical format. [mock:fixture=providers/scaleway/bge-multilingual-gemma2]"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    sent_to_scaleway(&env, "/v1/embeddings");
    assert_eq!(r.json()["usage"]["prompt_tokens"], 11);

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", WHISPER), ("language", "en")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/audio/transcriptions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );

    env.mock.clear();
    for (path, fields) in [
        (
            "/v1/audio/transcriptions",
            vec![("model", WHISPER), ("response_format", "verbose_json")],
        ),
        ("/v1/audio/translations", vec![("model", WHISPER)]),
    ] {
        let r = upload(&env, path, &fields).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{path}: {}", r.text());
    }
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GPT_OSS, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}
