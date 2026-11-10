#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env, call, chat, messages};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const SECRET: &str = "sk-test-credential-0001";
const R1: &str = "deepseek-ai/DeepSeek-R1";
const QWEN: &str = "Qwen/Qwen3-8B";
const EMBED: &str = "Qwen/Qwen3-Embedding-8B";
const TTS: &str = "FunAudioLLM/CosyVoice2-0.5B";
const ASR: &str = "FunAudioLLM/SenseVoiceSmall";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/siliconflow/{folder}/{name}]")
}

async fn add_model(env: &Env, model: &str, overrides: Value) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "siliconflow", "model_id": model, "custom": true, "overrides": overrides }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
}

fn text_model(egress: &str) -> Value {
    json!({
        "input_modalities": ["text"], "output_modalities": ["text"],
        "context_limit": 131072, "output_limit": 16384,
        "input_price": "0.5", "output_price": "2.18",
        "reasoning": true, "tool_call": true,
        "egress_dialect": egress
    })
}

fn media_model(input: &str, output: &str, per: &str, of: &str) -> Value {
    json!({
        "input_modalities": [input], "output_modalities": [output],
        "unit_prices": [{ "per": per, "of": of, "usd": "0.01" }]
    })
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "siliconflow", "name": "siliconflow", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_model(&env, R1, text_model("openai_chat")).await;
    add_model(&env, QWEN, text_model("openai_chat")).await;
    let mut embed = text_model("openai_embeddings");
    embed["output_price"] = json!("0");
    add_model(&env, EMBED, embed).await;
    add_model(
        &env,
        TTS,
        media_model("text", "audio", "million_characters", "input"),
    )
    .await;
    add_model(&env, ASR, media_model("audio", "text", "minute", "input")).await;
    env
}

fn sent_to_siliconflow(env: &Env, path: &str) -> Value {
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
async fn chat_apps_reach_siliconflow_with_max_tokens_system_messages_and_no_end_user() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": R1,
            "messages": [
                { "role": "developer", "content": "Name three." },
                { "role": "user", "content": format!("Who are the legendary athletes of the Olympic Games? {}", fixture("responses", "reasoning-doc")) }
            ],
            "max_completion_tokens": 4096,
            "thinking_budget": 1024,
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("siliconflow"));
    assert_eq!(
        r.json()["choices"][0]["message"]["reasoning_content"],
        "The user asks for legendary Olympians. Pick a few from different sports and eras."
    );
    let sent = sent_to_siliconflow(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (
            &sent["max_tokens"],
            sent.get("max_completion_tokens"),
            &sent["thinking_budget"]
        ),
        (&json!(4096), None, &json!(1024))
    );
    assert!(
        sent.get("safety_identifier").is_none() && sent.get("user").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_switch_thinking_on_with_a_budget_and_get_it_back_streamed() {
    let env = env().await;
    let mut body = messages(
        R1,
        &format!(
            "Who are the legendary athletes of the Olympic Games? {}",
            fixture("streams", "reasoning-stream-doc")
        ),
    );
    body["stream"] = json!(true);
    body["max_tokens"] = json!(4096);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 1024 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_siliconflow(&env, "/v1/chat/completions");
    assert_eq!(
        (&sent["enable_thinking"], &sent["thinking_budget"]),
        (&json!(true), &json!(1024))
    );
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
    assert_eq!(last["usage"]["output_tokens"], 31);
}

#[tokio::test]
async fn a_refused_key_reaches_apps_as_the_refused_key() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            QWEN,
            "Hello [mock:error=../providers/siliconflow/errors/recorded-invalid-api-key]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("siliconflow"));
    assert!(!format!("{o:?}").contains(SECRET));
}

#[tokio::test]
async fn siliconflow_embeds_speaks_transcribes_and_refuses_what_it_lacks() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": "Silicon flow embedding online. [mock:fixture=providers/siliconflow/qwen3-embedding]",
            "dimensions": 1024
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_siliconflow(&env, "/v1/embeddings")["dimensions"],
        1024
    );
    assert_eq!(r.json()["usage"]["prompt_tokens"], 19);

    let r = call(
        &env,
        "/v1/audio/speech",
        Some(&env.key),
        false,
        Some(json!({
            "model": TTS,
            "input": "I'm so happy, Spring Festival is coming!",
            "voice": "FunAudioLLM/CosyVoice2-0.5B:alex",
            "response_format": "mp3"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_siliconflow(&env, "/v1/audio/speech");
    assert_eq!(
        (&sent["model"], &sent["voice"], &sent["response_format"]),
        (
            &json!(TTS),
            &json!("FunAudioLLM/CosyVoice2-0.5B:alex"),
            &json!("mp3")
        )
    );

    let r = upload(&env, "/v1/audio/transcriptions", &[("model", ASR)]).await;
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
            vec![("model", ASR), ("response_format", "verbose_json")],
        ),
        ("/v1/audio/translations", vec![("model", ASR)]),
    ] {
        let r = upload(&env, path, &fields).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{path}: {}", r.text());
    }
    for (path, anthropic, body) in [
        (
            "/v1/messages/count_tokens",
            true,
            json!({ "model": QWEN, "messages": [{ "role": "user", "content": "Hello" }] }),
        ),
        (
            "/v1/images/generations",
            false,
            json!({ "model": QWEN, "prompt": "A lighthouse at dusk" }),
        ),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert!(r.status.is_client_error(), "{path}: {}", r.text());
    }
    assert!(env.mock.received().is_empty());
}
