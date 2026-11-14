#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "cfut_cloudflare-test-credential-0001";
const ACCOUNT: &str = "0123456789abcdef0123456789abcdef";
const GATEWAY: &str = "kaeso-gateway";
const WORKERS_AI: &str = "@cf/zai-org/glm-4.7-flash";
const CLAUDE: &str = "anthropic/claude-sonnet-5.5";
const GPT: &str = "openai/gpt-6-luna";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/cloudflare-ai-gateway/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/cloudflare-ai-gateway/errors/{name}]")
}

fn credentials(env: &Env) -> String {
    format!("/orgs/{}/credentials", env.owner.org_id)
}

async fn add_key(env: &Env, body: Value) -> common::Resp {
    env.app
        .post(&credentials(env), body, Some(&env.owner.cookie))
        .await
}

async fn env_with(gateway: Option<&str>) -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let mut key = json!({ "provider": "cloudflare-ai-gateway", "name": "Cloudflare", "secret": SECRET, "account_id": ACCOUNT });
    if let Some(g) = gateway {
        key["gateway_id"] = json!(g);
    }
    let r = add_key(&env, key).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [WORKERS_AI, CLAUDE, GPT] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "cloudflare-ai-gateway", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

fn sent(env: &Env, path: &str) -> kaeso_testkit::mock_provider::Received {
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        format!("/client/v4/accounts/{ACCOUNT}/ai/v1/{path}")
    );
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent
}

#[tokio::test]
async fn requests_go_to_the_keys_account_through_its_gateway_with_thinking_for_anthropic_apps() {
    let env = env_with(Some(GATEWAY)).await;
    let mut body = messages(
        WORKERS_AI,
        &format!(
            "Is 91 a prime number? {}",
            fixture("responses", "recorded-workers-ai-reply")
        ),
    );
    body["max_tokens"] = json!(400);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("cloudflare-ai-gateway"));
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    assert_eq!(reply["content"][1]["type"], "text", "{reply}");
    let sent = sent(&env, "chat/completions");
    assert_eq!(sent.header("cf-aig-gateway-id"), Some(GATEWAY));
    assert_eq!(sent.json()["model"], WORKERS_AI);
}

#[tokio::test]
async fn without_a_gateway_requests_name_none_and_the_account_default_takes_them() {
    let env = env_with(None).await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            WORKERS_AI,
            &format!("Hi {}", fixture("responses", "recorded-workers-ai-reply")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent(&env, "chat/completions").header("cf-aig-gateway-id"),
        None
    );
}

#[tokio::test]
async fn a_claude_model_goes_out_as_messages_and_a_streamed_reply_reports_its_whole_usage() {
    let env = env_with(Some(GATEWAY)).await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(CLAUDE, "Hello [mock:fixture=recorded-text-simple]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent(&env, "messages");
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.header("cf-aig-gateway-id"), Some(GATEWAY));
    assert_eq!(sent.json()["model"], CLAUDE);

    let mut body = chat(
        WORKERS_AI,
        &format!("Count {}", fixture("streams", "recorded-reasoning-stream")),
    );
    body["stream"] = json!(true);
    body["stream_options"] = json!({ "include_usage": true });
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let chunks: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .filter(|c| c["usage"].is_object())
        .collect();
    let usage = chunks[chunks.len() - 1]["usage"].clone();
    assert_eq!(
        (&usage["prompt_tokens"], &usage["completion_tokens"]),
        (&json!(17), &json!(400)),
        "{usage}"
    );
}

#[tokio::test]
async fn cloudflare_refusals_reach_apps_with_cloudflare_message_and_token_counts_are_refused() {
    let env = env_with(None).await;
    for (name, status, said) in [
        (
            "recorded-insufficient-balance",
            StatusCode::BAD_GATEWAY,
            "billing or quota",
        ),
        (
            "recorded-invalid-api-key",
            StatusCode::BAD_GATEWAY,
            "refused",
        ),
        (
            "recorded-unknown-model",
            StatusCode::NOT_FOUND,
            "Model not found",
        ),
        (
            "recorded-retired-model",
            StatusCode::GONE,
            "openai/gpt-5.6-sol",
        ),
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GPT, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, status, "{name}: {}", r.text());
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains(said), "{name}: {message}");
    }
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": WORKERS_AI, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{WORKERS_AI}` cannot count tokens.")
    );
}

#[tokio::test]
async fn a_key_names_a_valid_account_a_gateway_may_change_and_other_providers_take_neither() {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let key = |extra: Value| {
        let mut k =
            json!({ "provider": "cloudflare-ai-gateway", "name": "Cloudflare", "secret": SECRET });
        for (name, value) in extra.as_object().unwrap() {
            k[name] = value.clone();
        }
        k
    };
    for (extra, param) in [
        (json!({}), "account_id"),
        (json!({ "account_id": "1234" }), "account_id"),
        (
            json!({ "account_id": ACCOUNT, "gateway_id": "no spaces" }),
            "gateway_id",
        ),
    ] {
        let r = add_key(&env, key(extra.clone())).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{extra}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{extra}: {:?}", r.body);
    }
    let r = add_key(
        &env,
        json!({ "provider": "openai", "name": "OpenAI", "secret": "sk-test-credential-0001", "account_id": ACCOUNT }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "account_id");

    let r = add_key(
        &env,
        key(json!({ "account_id": ACCOUNT.to_uppercase(), "gateway_id": GATEWAY })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(
        (&r.body["account_id"], &r.body["gateway_id"]),
        (&json!(ACCOUNT), &json!(GATEWAY))
    );
    let id = r.body["id"].as_str().unwrap().to_owned();
    let path = format!("{}/{id}", credentials(&env));
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Cloudflare", "gateway_id": "" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (&r.body["account_id"], &r.body["gateway_id"]),
        (&json!(ACCOUNT), &Value::Null)
    );
    let other = "fedcba9876543210fedcba9876543210";
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Cloudflare", "account_id": other, "gateway_id": "eu" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(
        (&r.body["account_id"], &r.body["gateway_id"]),
        (&json!(other), &json!("eu")),
        "{:?}",
        r.body
    );
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": "cfut_cloudflare-test-credential-0002" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (
            &r.body["account_id"],
            &r.body["gateway_id"],
            &r.body["last4"]
        ),
        (&json!(other), &json!("eu"), &json!("0002")),
        "a replaced token keeps its account and gateway"
    );
    let listed = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let cloudflare = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "cloudflare-ai-gateway")
        .unwrap()
        .clone();
    assert_eq!(
        cloudflare["account_scope"]["gateway_header"],
        "cf-aig-gateway-id"
    );
}

#[tokio::test]
async fn a_check_finds_workers_ai_models_in_cloudflare_search_and_cannot_tell_for_others() {
    let env = env_with(Some(GATEWAY)).await;
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/models/check",
                env.owner.org_id, env.project
            ),
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
    assert_eq!(availability(WORKERS_AI), "available");
    let failed: Vec<(&str, &str)> = r.body["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["model_id"].as_str().unwrap(),
                f["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        failed,
        [(CLAUDE, "inconclusive"), (GPT, "inconclusive")],
        "{:?}",
        r.body
    );
    // One search per Cloudflare model, and no billed model request.
    let cloudflare = env
        .mock
        .received()
        .into_iter()
        .filter(|s| s.path.starts_with("/client/v4/"))
        .collect::<Vec<_>>();
    // One search per Cloudflare model, and no billed model request.
    assert_eq!(
        cloudflare
            .iter()
            .map(|s| s.path.clone())
            .collect::<Vec<_>>(),
        vec![format!("/client/v4/accounts/{ACCOUNT}/ai/models/search"); 3]
    );
}

const CLEF: &str = "@cf/cloudflare/clef";
const CLEF_FLASH: &str = "@cf/cloudflare/clef-flash";
// A 1×1 PNG.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

#[derive(Clone, Copy, Debug)]
enum App {
    OpenAi,
    SystemOne,
    Gemini,
}

const APPS: [App; 3] = [App::OpenAi, App::SystemOne, App::Gemini];

impl App {
    fn path(self, env: &Env) -> String {
        match self {
            App::OpenAi => "/v1/decisions".to_owned(),
            App::SystemOne => "/v1/systemone".to_owned(),
            App::Gemini => format!("/v1beta/decisions?key={}", env.key),
        }
    }

    // A yes/no, a choice and a score question about a support message, after `images` photos.
    fn body(self, model: &str, images: usize) -> Value {
        let text = "I was charged twice; refund me.";
        let url = format!("data:image/png;base64,{PNG}");
        match self {
            App::OpenAi => {
                let mut content: Vec<Value> = (0..images)
                    .map(|_| json!({ "type": "input_image", "image_url": url }))
                    .collect();
                content.push(json!({ "type": "input_text", "text": text }));
                json!({ "model": model, "input": [{ "role": "user", "content": content }], "questions": [
                    { "type": "predicate", "name": "refund", "instructions": "Does the customer ask for a refund?" },
                    { "type": "choice", "name": "team", "instructions": "Which team handles it?",
                      "choices": [{ "value": "billing" }, { "value": "support" }] },
                    { "type": "score", "name": "urgency", "instructions": "How urgent is it?",
                      "levels": [{ "label": "low" }, { "label": "high" }] }
                ] })
            }
            App::SystemOne => {
                let mut state: Vec<Value> =
                    (0..images).map(|i| json!(format!("<photo-{i}>"))).collect();
                state.push(json!(text));
                let assets: serde_json::Map<String, Value> = (0..images)
                    .map(|i| {
                        (
                            format!("<photo-{i}>"),
                            json!({ "type": "image", "format": "png", "data_base64": PNG }),
                        )
                    })
                    .collect();
                let mut body = json!({ "model": model, "state": state, "questions": {
                    "refund": { "type": "noul", "instructions": "Does the customer ask for a refund?" },
                    "team": { "type": "choice", "instructions": "Which team handles it?",
                              "criteria": { "billing": "Payments and refunds.", "support": "Everything else." } },
                    "urgency": { "type": "score", "instructions": "How urgent is it?", "criteria": ["low", "high"] }
                } });
                if images > 0 {
                    body["assets"] = Value::Object(assets);
                }
                body
            }
            App::Gemini => {
                let mut parts: Vec<Value> = (0..images)
                    .map(|_| json!({ "type": "image", "data": PNG, "mime_type": "image/png" }))
                    .collect();
                parts.push(json!({ "type": "text", "text": text }));
                json!({ "model": model, "input": parts, "questions": {
                    "refund": { "type": "noul", "instructions": "Does the customer ask for a refund?" },
                    "team": { "type": "choice", "instructions": "Which team handles it?",
                              "criteria": { "billing": "Payments and refunds.", "support": "Everything else." } },
                    "urgency": { "type": "score", "instructions": "How urgent is it?", "criteria": ["low", "high"] }
                } })
            }
        }
    }

    fn refund(self, reply: &Value) -> f64 {
        match self {
            App::OpenAi => reply["answers"][0]["probability"].as_f64().unwrap(),
            App::SystemOne | App::Gemini => reply["answers"]["refund"]["noul"].as_f64().unwrap(),
        }
    }

    fn error_message(self, reply: &Value) -> String {
        match self {
            App::OpenAi | App::Gemini => reply["error"]["message"].as_str().unwrap().to_owned(),
            App::SystemOne => match &reply["detail"] {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            },
        }
    }
}

async fn clef_env(gateway: Option<&str>) -> Env {
    let env = env_with(gateway).await;
    for model in [CLEF, CLEF_FLASH] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "cloudflare-ai-gateway", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

async fn decide(env: &Env, app: App, body: Value) -> common::v1::Answer {
    let auth = match app {
        App::Gemini => None,
        App::OpenAi | App::SystemOne => Some(env.key.as_str()),
    };
    call(env, &app.path(env), auth, false, Some(body)).await
}

#[tokio::test]
async fn clef_answers_every_decision_format_at_the_keys_account_through_its_gateway() {
    let env = clef_env(Some(GATEWAY)).await;
    for app in APPS {
        for (model, short) in [(CLEF, "clef"), (CLEF_FLASH, "clef-flash")] {
            env.mock.clear();
            let r = decide(&env, app, app.body(model, 0)).await;
            assert_eq!(r.status, StatusCode::OK, "{app:?} {model}: {}", r.text());
            assert!(
                (app.refund(&r.json()) - 0.82).abs() < 1e-9,
                "{app:?}: {}",
                r.text()
            );
            assert_eq!(r.header("kaeso-provider"), Some("cloudflare-ai-gateway"));
            assert_eq!(r.header("kaeso-model"), Some(model));
            let sent = env.mock.last().unwrap();
            assert_eq!(
                sent.path,
                format!("/client/v4/accounts/{ACCOUNT}/ai/run/{model}")
            );
            assert_eq!(
                sent.header("authorization"),
                Some(format!("Bearer {SECRET}").as_str())
            );
            assert_eq!(sent.header("cf-aig-gateway-id"), Some(GATEWAY));
            let body = sent.json();
            assert_eq!(body["model"], short, "{body}");
            assert!(body["questions"]["refund"].is_object(), "{body}");
            assert!(body.get("images").is_none(), "{body}");
            if let App::SystemOne = app {
                assert_eq!(
                    r.json()["model"],
                    model,
                    "a reply names the model's full id"
                );
            }
            let o = env
                .capture
                .recorded(r.header("kaeso-request-id").unwrap())
                .await;
            assert_eq!(o.egress.unwrap().as_str(), "system_one");
            assert_eq!(o.usage.clone().unwrap().input_tokens, 120);
        }
    }
    let r = decide(&env, App::SystemOne, App::SystemOne.body(CLEF, 0)).await;
    let o = env
        .capture
        .recorded(r.header("kaeso-request-id").unwrap())
        .await;
    assert_eq!(
        kaeso_server::services::usage::costed(&o).1,
        rust_decimal::Decimal::new(288, 7),
        "120 input tokens at $0.24 per million"
    );

    let env = clef_env(None).await;
    let r = decide(&env, App::OpenAi, App::OpenAi.body(CLEF, 0)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().header("cf-aig-gateway-id"), None);
}

#[tokio::test]
async fn clef_takes_up_to_four_images_from_every_format_in_a_list_of_its_own() {
    let env = clef_env(Some(GATEWAY)).await;
    for app in APPS {
        env.mock.clear();
        let r = decide(&env, app, app.body(CLEF, 4)).await;
        assert_eq!(r.status, StatusCode::OK, "{app:?}: {}", r.text());
        let body = env.mock.last().unwrap().json();
        let images = body["images"].as_array().unwrap();
        assert_eq!(images.len(), 4, "{app:?}: {body}");
        assert!(
            images
                .iter()
                .all(|i| i.as_str().unwrap().starts_with("data:image/png;base64,"))
        );
        assert_eq!(
            body["state"],
            json!(["I was charged twice; refund me."]),
            "{app:?}"
        );

        env.mock.clear();
        let r = decide(&env, app, app.body(CLEF, 5)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{app:?}: {}", r.text());
        let message = app.error_message(&r.json());
        assert!(message.contains("at most 4 images"), "{app:?}: {message}");
        assert!(env.mock.received().is_empty(), "{app:?}");
    }
}

#[tokio::test]
async fn questions_clef_cant_take_are_refused_before_anything_is_sent() {
    let env = clef_env(Some(GATEWAY)).await;
    env.mock.clear();
    let many = |n: usize| {
        let questions: serde_json::Map<String, Value> = (0..n)
            .map(|i| {
                (
                    format!("q{i}"),
                    json!({ "type": "noul", "instructions": "Urgent?" }),
                )
            })
            .collect();
        json!({ "model": CLEF, "state": "Hi", "questions": questions })
    };
    let r = decide(&env, App::SystemOne, many(65)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        App::SystemOne
            .error_message(&r.json())
            .contains("at most 64 questions"),
        "{}",
        r.text()
    );
    let boxed = json!({ "model": CLEF, "state": "A photo",
        "questions": { "where": { "type": "box", "instructions": "the cat" } } });
    let r = decide(&env, App::SystemOne, boxed).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        App::SystemOne.error_message(&r.json()),
        format!("The model `{CLEF}` does not answer box questions.")
    );
    let mut extra = App::SystemOne.body(CLEF, 0);
    extra["priority"] = json!("high");
    let r = decide(&env, App::SystemOne, extra).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        App::SystemOne
            .error_message(&r.json())
            .contains("`priority`"),
        "{}",
        r.text()
    );
    let one_level = json!({ "model": CLEF, "input": "Hi", "questions": [
        { "type": "score", "name": "urgency", "instructions": "How urgent?", "levels": [{ "label": "only" }] }] });
    let r = decide(&env, App::OpenAi, one_level).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty(), "{:?}", env.mock.received());

    let r = decide(&env, App::SystemOne, many(64)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let generation = decide(&env, App::SystemOne, App::SystemOne.body(WORKERS_AI, 0)).await;
    assert_eq!(
        App::SystemOne.error_message(&generation.json()),
        format!(
            "The model `{WORKERS_AI}` is a generation model; it can't answer decision questions."
        )
    );
}

#[tokio::test]
async fn cloudflare_refusals_of_a_decision_reach_apps_in_their_own_format() {
    let env = clef_env(Some(GATEWAY)).await;
    for app in APPS {
        let mut body = app.body(CLEF, 0);
        let marker = "[mock:error=unknown-gateway-400]";
        match app {
            App::SystemOne => body["state"] = json!(format!("Hi {marker}")),
            App::OpenAi => body["input"][0]["content"][0]["text"] = json!(format!("Hi {marker}")),
            App::Gemini => body["input"][0]["text"] = json!(format!("Hi {marker}")),
        }
        let r = decide(&env, app, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{app:?}: {}", r.text());
        assert_eq!(
            app.error_message(&r.json()),
            "Please configure AI Gateway in the Cloudflare dashboard",
            "{app:?}"
        );
    }
}

#[tokio::test]
async fn workers_ai_embeds_through_the_keys_account_with_or_without_a_gateway_and_refuses_dimensions()
 {
    for gateway in [None, Some(GATEWAY)] {
        let env = env_with(gateway).await;
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "cloudflare-ai-gateway", "model_id": "@cf/baai/bge-m3" }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        let r = call(
            &env,
            "/v1/embeddings",
            Some(&env.key),
            false,
            Some(json!({
                "model": "@cf/baai/bge-m3",
                "input": ["The food was delicious and the waiter was friendly. [mock:fixture=providers/cloudflare-ai-gateway/recorded-bge-m3]"],
            })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.header("kaeso-provider"), Some("cloudflare-ai-gateway"));
        assert_eq!(r.json()["data"].as_array().unwrap().len(), 2);
        let sent = sent(&env, "embeddings");
        assert_eq!(sent.header("cf-aig-gateway-id"), gateway);
        assert_eq!(sent.json()["model"], "@cf/baai/bge-m3");

        env.mock.clear();
        let r = call(
            &env,
            "/v1/embeddings",
            Some(&env.key),
            false,
            Some(json!({ "model": "@cf/baai/bge-m3", "input": "hi", "dimensions": 256 })),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            "`dimensions` is not supported by Cloudflare AI Gateway; remove it from the request."
        );
        assert!(env.mock.received().is_empty());
    }
}
