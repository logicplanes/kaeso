#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "AIzaSyTestGoogleCredential000000000000";
const FLASH: &str = "gemini-3.5-flash";
const OLD_FLASH: &str = "gemini-2.5-flash";
const EMBEDDING: &str = "gemini-embedding-2";
const IMAGE_MODEL: &str = "gemini-3.1-flash-image";

fn fixture(name: &str) -> String {
    format!("[mock:fixture={name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error={name}]")
}

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "google", "name": "google", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [FLASH, OLD_FLASH, EMBEDDING, IMAGE_MODEL] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "google", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_google(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(sent.header("x-goog-api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(sent.header("x-api-key"), None);
    assert!(
        !sent.query.as_deref().unwrap_or_default().contains("key="),
        "{:?}",
        sent.query
    );
    sent.json()
}

#[tokio::test]
async fn a_chat_app_reaches_google_with_its_key_in_the_header_and_the_model_in_the_path() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            &format!("Hello {}", fixture("recorded-text-simple")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("google"));
    assert_eq!(r.header("kaeso-model"), Some(FLASH));
    let sent = sent_to_google(&env, "/v1beta/models/gemini-3.5-flash:generateContent");
    assert!(sent.get("model").is_none(), "{sent}");
    assert_eq!(sent["contents"][0]["role"], "user");
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello, how are you today?"
    );
    assert_eq!(reply["usage"]["prompt_tokens"], 7);
    assert_eq!(reply["usage"]["completion_tokens"], 482);
}

#[tokio::test]
async fn a_chat_apps_audio_reaches_gemini_inline_and_a_model_without_audio_input_refuses_it() {
    let env = env().await;
    let audio = |model: &str| {
        json!({"model": model, "messages": [{"role": "user", "content": [
            {"type": "input_audio", "input_audio": {"data": "SUQzBAAAAAAAI1RTU0U=", "format": "mp3"}},
            {"type": "text", "text": format!("Transcribe this clip. {}", fixture("recorded-audio-input"))}
        ]}]})
    };
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(audio(FLASH)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_google(&env, "/v1beta/models/gemini-3.5-flash:generateContent");
    assert_eq!(
        sent["contents"][0]["parts"][0],
        json!({"inlineData": {"mimeType": "audio/mpeg", "data": "SUQzBAAAAAAAI1RTU0U="}})
    );
    assert_eq!(r.json()["choices"][0]["message"]["content"], "Hello Kazo");

    let calls = env.mock.received().len();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(audio(IMAGE_MODEL)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `gemini-3.1-flash-image` does not accept audio."
    );
    assert_eq!(r.json()["error"]["param"], "messages");
    assert_eq!(env.mock.received().len(), calls);
}

#[tokio::test]
async fn anthropic_and_responses_apps_get_googles_streams_tool_calls_and_counts_in_their_format() {
    let env = env().await;
    let mut stream = messages(FLASH, &format!("Count {}", fixture("recorded-stream-text")));
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/v1beta/models/gemini-3.5-flash:streamGenerateContent"
    );
    assert_eq!(sent.query.as_deref(), Some("alt=sse"));
    let text = r.text();
    assert!(text.contains("event: message_stop"), "{text}");
    assert!(text.contains("Two\\nThree"), "{text}");

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": FLASH,
            "input": format!("Weather in Paris? {}", fixture("recorded-tools-single")),
            "tools": [{
                "type": "function",
                "name": "get_weather",
                "parameters": { "type": "object", "properties": { "location": { "type": "string" } } }
            }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_google(&env, "/v1beta/models/gemini-3.5-flash:generateContent");
    assert_eq!(
        sent["tools"][0]["functionDeclarations"][0]["name"],
        "get_weather"
    );
    let output = r.json()["output"].clone();
    let call_ = output
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "function_call")
        .unwrap();
    assert_eq!(call_["name"], "get_weather");
    assert_eq!(call_["call_id"], "call_18261916837581647102");

    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": FLASH, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    sent_to_google(&env, "/v1beta/models/gemini-3.5-flash:countTokens");
    assert_eq!(r.json()["input_tokens"], 8);
}

#[tokio::test]
async fn an_effort_reaches_a_gemini_3_model_as_a_level_and_is_left_out_for_a_gemini_2_5_model() {
    let env = env().await;
    let ask = |model: &'static str| {
        json!({
            "model": model,
            "reasoning": { "effort": "low" },
            "input": format!("Hi {}", fixture("recorded-text-simple"))
        })
    };
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(ask(FLASH)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_google(&env, "/v1beta/models/gemini-3.5-flash:generateContent");
    assert_eq!(
        sent["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "LOW"
    );
    assert_eq!(r.header("kaeso-left-out"), None);

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(ask(OLD_FLASH)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("reasoning.effort"));
    let sent = sent_to_google(&env, "/v1beta/models/gemini-2.5-flash:generateContent");
    assert!(
        sent.pointer("/generationConfig/thinkingConfig/thinkingLevel")
            .is_none(),
        "{sent}"
    );

    let mut thinking = messages(
        OLD_FLASH,
        &format!("Hi {}", fixture("recorded-text-simple")),
    );
    thinking["thinking"] = json!({ "type": "enabled", "budget_tokens": 2048 });
    thinking["max_tokens"] = json!(4096);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(thinking)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_google(&env, "/v1beta/models/gemini-2.5-flash:generateContent");
    assert_eq!(
        sent["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        2048
    );
}

#[tokio::test]
async fn an_openai_embeddings_app_reaches_googles_batch_embeddings() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBEDDING, "input": ["The quick brown fox", "jumps over"] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_google(&env, "/v1beta/models/gemini-embedding-2:batchEmbedContents");
    assert_eq!(sent["requests"].as_array().unwrap().len(), 2);
    let reply = r.json();
    assert_eq!(reply["data"].as_array().unwrap().len(), 2);
    assert_eq!(reply["usage"]["prompt_tokens"], 6);
    assert_eq!(reply["model"], EMBEDDING);
}

#[tokio::test]
async fn googles_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            &format!("Hi {}", error("recorded-error-invalid-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            FLASH,
            &format!("Hi {}", error("recorded-error-invalid-argument")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn a_check_retrieves_each_google_model_and_pages_through_googles_listing_with_its_token() {
    let env = env().await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({
                "provider": "google", "model_id": "unserved-gemini", "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 1048576, "output_limit": 65536,
                    "input_price": "0.3", "output_price": "2.5", "egress_dialect": "gemini"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env.mock.clear();
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models/check", env.project),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let availability = |id: &str| {
        r.body["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == id)
            .unwrap()["availability"]
            .clone()
    };
    assert_eq!(availability(FLASH), "available");
    assert_eq!(availability("unserved-gemini"), "unavailable");
    let asked: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET" && r.path.starts_with("/v1beta/"))
        .map(|r| match r.query {
            Some(q) => format!("{}?{q}", r.path),
            None => r.path,
        })
        .collect();
    assert!(
        asked.contains(&"/v1beta/models/gemini-3.5-flash".to_owned()),
        "{asked:?}"
    );
    assert!(
        asked.contains(&"/v1beta/models/unserved-gemini".to_owned()),
        "{asked:?}"
    );
    let pages: Vec<&String> = asked
        .iter()
        .filter(|p| p.starts_with("/v1beta/models?") || *p == "/v1beta/models")
        .collect();
    assert_eq!(
        pages,
        [
            "/v1beta/models",
            "/v1beta/models?pageToken=Chltb2RlbHMvZ2VtbWEtNC0yNmItYTRiLWl0",
            "/v1beta/models?pageToken=Chxtb2RlbHMvZ2VtaW5pLTIuNS1mbGFzaC1saXRl",
        ]
    );
    for sent in env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.path.starts_with("/v1beta/"))
    {
        assert_eq!(sent.header("x-goog-api-key"), Some(SECRET));
    }
}
