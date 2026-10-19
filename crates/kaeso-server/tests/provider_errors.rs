#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Answer, Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const KEY_REFUSED: &str = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";
const ACCOUNT_REFUSED: &str = "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers.";

#[derive(Clone, Copy)]
enum App {
    Messages,
    Chat,
    Responses,
}

impl App {
    fn path(self) -> &'static str {
        match self {
            App::Messages => "/v1/messages",
            App::Chat => "/v1/chat/completions",
            App::Responses => "/v1/responses",
        }
    }

    fn body(self, model: &str, text: &str, stream: bool) -> Value {
        let mut body = match self {
            App::Messages => messages(model, text),
            App::Chat => chat(model, text),
            App::Responses => json!({ "model": model, "input": text }),
        };
        if stream {
            body["stream"] = json!(true);
        }
        body
    }

    fn message(self, r: &Answer) -> String {
        r.json()["error"]["message"].as_str().unwrap().to_owned()
    }

    fn kind(self, r: &Answer) -> String {
        r.json()["error"]["type"].as_str().unwrap().to_owned()
    }
}

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

async fn ask(env: &Env, app: App, model: &str, fixture: &str, stream: bool) -> Answer {
    let text = format!("Hi [mock:error={fixture}]");
    call(
        env,
        app.path(),
        Some(&env.key),
        matches!(app, App::Messages),
        Some(app.body(model, &text, stream)),
    )
    .await
}

const APPS: [App; 3] = [App::Messages, App::Chat, App::Responses];

#[tokio::test]
async fn a_refused_provider_key_is_a_provider_error_in_every_format() {
    let env = env().await;
    for (model, fixture, message) in [
        ("claude-haiku-4-5", "authentication", KEY_REFUSED),
        ("claude-haiku-4-5", "permission", KEY_REFUSED),
        ("claude-haiku-4-5", "billing", ACCOUNT_REFUSED),
        ("gpt-4o", "invalid-api-key-401", KEY_REFUSED),
        ("gpt-4o", "unsupported-region-403", KEY_REFUSED),
        ("gpt-4o", "insufficient-quota-429", ACCOUNT_REFUSED),
    ] {
        for app in APPS {
            for stream in [false, true] {
                let r = ask(&env, app, model, fixture, stream).await;
                let what = format!("{fixture} to {} (stream {stream})", app.path());
                assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{what}: {}", r.text());
                assert_eq!(app.message(&r), message, "{what}");
                let kind = app.kind(&r);
                match app {
                    App::Messages => assert_eq!(kind, "api_error", "{what}"),
                    App::Chat | App::Responses => assert_eq!(kind, "server_error", "{what}"),
                }
                assert!(r.header("retry-after").is_none(), "{what}");
                assert!(!r.text().contains("sk-"), "{what}: {}", r.text());
            }
        }
    }
}

#[tokio::test]
async fn the_request_log_keeps_the_providers_answer_without_key_fragments() {
    let env = env().await;
    let r = ask(&env, App::Chat, "gpt-4o", "invalid-api-key-401", false).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    common::eventually("the request is recorded", || async {
        !env.capture.all().is_empty()
    })
    .await;
    let o = env.capture.all().pop().unwrap();
    assert_eq!(o.status, 502);
    assert_eq!(o.error_kind.as_deref(), Some("authentication"));
    let logged = o.error_message.unwrap();
    assert!(
        logged.starts_with("Incorrect API key provided: sk-[redacted]."),
        "{logged}"
    );
    assert!(!logged.contains("Ab1x"), "{logged}");
}

#[tokio::test]
async fn other_provider_errors_keep_their_meaning_in_every_format() {
    let env = env().await;
    for (fixture, status, same) in [
        ("invalid-request-prefill", StatusCode::BAD_REQUEST, true),
        ("api-error", StatusCode::INTERNAL_SERVER_ERROR, true),
    ] {
        for app in APPS {
            let r = ask(&env, app, "claude-haiku-4-5", fixture, false).await;
            assert_eq!(
                r.status,
                status,
                "{fixture} to {}: {}",
                app.path(),
                r.text()
            );
            let message = app.message(&r);
            assert!(!message.contains("Ask an organization admin"), "{message}");
            if same && matches!(app, App::Messages) {
                let fixture_body: Value = serde_json::from_slice(
                    &std::fs::read(
                        kaeso_testkit::mock_provider::kpp_fixtures()
                            .join(format!("anthropic_messages/errors/{fixture}.json")),
                    )
                    .unwrap(),
                )
                .unwrap();
                assert_eq!(message, fixture_body["error"]["message"], "{fixture}");
            }
        }
    }
    let r = ask(&env, App::Messages, "gpt-4o", "server-error-500", true).await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR, "{}", r.text());
    assert_eq!(App::Messages.kind(&r), "api_error");
}
