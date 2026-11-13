#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use common::v1::{Answer, Capture, Env, call, chat, messages};
use http_body_util::BodyExt;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use tower::ServiceExt;

// TypeSafe keys have no documented prefix; the marker makes the mock list models as TypeSafe does.
const SECRET: &str = "ts-[mock:models=typesafe]-0123456789abcdef0123";

#[derive(Clone, Copy, Debug)]
enum App {
    OpenAi,
    SystemOne,
    Gemini,
}

const APPS: [App; 3] = [App::OpenAi, App::SystemOne, App::Gemini];

impl App {
    fn path(self) -> &'static str {
        match self {
            App::OpenAi => "/v1/decisions",
            App::SystemOne => "/v1/systemone",
            App::Gemini => "/v1beta/decisions",
        }
    }

    // A yes/no, a choice and a score question about a support message.
    fn body(self, model: &str, state: &str) -> Value {
        match self {
            App::OpenAi => json!({
                "model": model,
                "input": state,
                "questions": [
                    { "type": "predicate", "name": "refund", "instructions": "Does the customer ask for a refund?" },
                    { "type": "choice", "name": "team", "instructions": "Which team handles it?",
                      "choices": [{ "value": "billing" }, { "value": "support" }] },
                    { "type": "score", "name": "urgency", "instructions": "How urgent is it?",
                      "levels": [{ "label": "low" }, { "label": "medium" }, { "label": "high" }] }
                ]
            }),
            App::SystemOne | App::Gemini => {
                let input = if matches!(self, App::SystemOne) {
                    "state"
                } else {
                    "input"
                };
                json!({
                    "model": model,
                    input: state,
                    "questions": {
                        "refund": { "type": "noul", "instructions": "Does the customer ask for a refund?" },
                        "team": { "type": "choice", "instructions": "Which team handles it?",
                                  "criteria": { "billing": "Payments and refunds.", "support": "Everything else." } },
                        "urgency": { "type": "score", "instructions": "How urgent is it?",
                                     "criteria": ["low", "medium", "high"] }
                    }
                })
            }
        }
    }

    fn refund_probability(self, reply: &Value) -> f64 {
        match self {
            App::OpenAi => reply["answers"][0]["probability"].as_f64().unwrap(),
            App::SystemOne | App::Gemini => reply["answers"]["refund"]["noul"].as_f64().unwrap(),
        }
    }
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "typesafe", "name": "typesafe", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        json!({ "provider": "typesafe", "model_id": "jev-latest" }),
        json!({ "provider": "typesafe", "model_id": "jev-1.14.0" }),
        json!({ "provider": "typesafe", "model_id": "jev-1.13.0" }),
        // A model TypeSafe's listing doesn't name.
        json!({ "provider": "typesafe", "model_id": "jev-1.12.0", "custom": true, "overrides": {
            "input_modalities": ["text"], "input_price": "0.042",
            "decisions": { "only": true, "max_options": 255, "max_levels": 10 } } }),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                model.clone(),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

async fn send(env: &Env, app: App, body: &Value, headers: &[(&str, &str)]) -> Answer {
    let mut req = Request::builder()
        .uri(app.path())
        .method("POST")
        .header("content-type", "application/json");
    req = match app {
        App::Gemini => req.header("x-goog-api-key", &env.key),
        App::OpenAi | App::SystemOne => req.header("authorization", format!("Bearer {}", env.key)),
    };
    for (n, v) in headers {
        req = req.header(*n, *v);
    }
    let resp = env
        .app
        .router
        .clone()
        .oneshot(
            req.body(Body::from(serde_json::to_vec(body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    env.app.state.in_flight.settled().await;
    Answer {
        status,
        headers,
        body,
    }
}

// What reached TypeSafe: always `POST /v1/systemone` with the org's key.
fn sent_to_typesafe(env: &Env) -> kaeso_testkit::mock_provider::Received {
    let sent = env.mock.last().unwrap();
    assert_eq!(
        (sent.method.as_str(), sent.path.as_str()),
        ("POST", "/v1/systemone")
    );
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent
}

// A PNG header TypeSafe takes: 300 × 300, within its 280 to 1568 pixels a side.
fn png_base64() -> String {
    let mut b = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    b.extend(300u32.to_be_bytes());
    b.extend(300u32.to_be_bytes());
    b.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    base64::engine::general_purpose::STANDARD.encode(b)
}

#[tokio::test]
async fn every_decision_format_reaches_typesafe_as_system_one_and_is_priced() {
    let env = env().await;
    for app in APPS {
        for model in ["jev-latest", "jev-1.13.0"] {
            env.mock.clear();
            let r = send(
                &env,
                app,
                &app.body(model, "I was charged twice; refund me."),
                &[],
            )
            .await;
            assert_eq!(r.status, StatusCode::OK, "{app:?} → {model}: {}", r.text());
            assert!((app.refund_probability(&r.json()) - 0.82).abs() < 1e-9);
            assert_eq!(r.header("kaeso-provider"), Some("typesafe"));
            assert_eq!(r.header("kaeso-model"), Some(model));
            let sent = sent_to_typesafe(&env);
            let body = sent.json();
            assert_eq!(body["model"], model);
            assert_eq!(body["state"], "I was charged twice; refund me.");
            assert_eq!(body["questions"]["refund"]["type"], "noul", "{body}");
            assert_eq!(body["questions"]["urgency"]["type"], "score");
            // No image, no box question and a short state: neither danger zone.
            assert_eq!(sent.header("typesafe-danger-zone-images"), None);
            assert_eq!(sent.header("typesafe-danger-zone-long-context"), None);
            let o = env
                .capture
                .recorded(r.header("kaeso-request-id").unwrap())
                .await;
            assert_eq!(o.provider, Some("typesafe"));
            assert_eq!(o.egress.unwrap().as_str(), "system_one");
            // The mock reports 120 input tokens; TypeSafe charges $0.042 per million.
            assert_eq!(
                kaeso_server::services::usage::costed(&o).1,
                Decimal::new(504, 8),
                "{app:?} → {model}"
            );
            assert!(!format!("{o:?}").contains("0123456789abcdef0123"));
        }
    }
}

#[tokio::test]
async fn images_and_box_questions_go_with_typesafes_images_header() {
    let env = env().await;
    let pictured = json!({
        "model": "jev-1.14.0",
        "state": { "photo": "door_photo", "note": "Is the door open?" },
        "questions": {
            "open": { "type": "noul", "instructions": "Is the door in `door_photo` open?" },
            "door": { "type": "box", "instructions": "Where is the door in `door_photo`?" }
        },
        "assets": { "door_photo": { "type": "image", "format": "png", "data_base64": png_base64() } }
    });
    let r = send(&env, App::SystemOne, &pictured, &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["answers"]["door"]["box"]["top"], 0.1);
    let sent = sent_to_typesafe(&env);
    assert_eq!(sent.header("typesafe-danger-zone-images"), Some("true"));
    assert_eq!(
        sent.json()["assets"]["door_photo"]["data_base64"],
        png_base64()
    );

    // A box question alone needs the header too.
    let boxed = json!({
        "model": "jev-1.14.0",
        "state": "A screenshot description.",
        "questions": { "button": { "type": "box", "instructions": "Where is the button?" } }
    });
    let r = send(&env, App::SystemOne, &boxed, &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_typesafe(&env).header("typesafe-danger-zone-images"),
        Some("true")
    );

    // An app's own images header is sent once.
    let r = send(
        &env,
        App::SystemOne,
        &boxed,
        &[("TypeSafe-Danger-Zone-Images", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_typesafe(&env);
    let images = sent
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("typesafe-danger-zone-images"))
        .count();
    assert_eq!(images, 1);

    // Jev 1.13 answers no box questions: refused before anything is sent.
    env.mock.clear();
    let mut older = boxed.clone();
    older["model"] = json!("jev-1.13.0");
    let r = send(&env, App::SystemOne, &older, &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["detail"],
        "The model `jev-1.13.0` does not answer box questions."
    );
    assert!(env.mock.last().is_none());
}

#[tokio::test]
async fn a_long_request_asks_typesafe_for_long_context_and_says_so() {
    let env = env().await;
    // 300,000 bytes of state are about 75,000 tokens: beyond 64k a request and 32k a question.
    let long = "The contract renews every year unless cancelled. ".repeat(6000);
    for app in APPS {
        let r = send(&env, app, &app.body("jev-latest", &long), &[]).await;
        assert_eq!(r.status, StatusCode::OK, "{app:?}: {}", r.text());
        assert_eq!(
            sent_to_typesafe(&env).header("typesafe-danger-zone-long-context"),
            Some("true"),
            "{app:?}"
        );
        let adapted = r.header("kaeso-adapted").unwrap_or_default();
        assert!(
            adapted.split(", ").any(|a| a == "long_context"),
            "{app:?}: {adapted}"
        );
    }

    // An app that asks for long context itself is forwarded as it asked, and nothing is adapted.
    let short = App::SystemOne.body("jev-latest", "Short.");
    let r = send(
        &env,
        App::SystemOne,
        &short,
        &[("TypeSafe-Danger-Zone-Long-Context", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_typesafe(&env).header("typesafe-danger-zone-long-context"),
        Some("true")
    );
    assert!(
        !r.header("kaeso-adapted")
            .unwrap_or_default()
            .contains("long_context")
    );
}

#[tokio::test]
async fn a_key_typesafe_refuses_is_reported_as_the_providers_key() {
    let env = env().await;
    // TypeSafe answers a key it doesn't know with a 401 and a request without one with a 403.
    for fixture in ["recorded-invalid-key-401", "recorded-missing-key-403"] {
        let body = App::OpenAi.body(
            "jev-latest",
            &format!("Hello [mock:error=../providers/typesafe/errors/{fixture}]"),
        );
        let r = send(&env, App::OpenAi, &body, &[]).await;
        // Read as a refused key, not as a request the key may not make.
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{fixture}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            "The provider refused this project's provider key. Ask an organization admin to check it under Providers.",
            "{fixture}"
        );
        let o = env
            .capture
            .recorded(r.header("kaeso-request-id").unwrap())
            .await;
        assert_eq!(o.provider, Some("typesafe"));
        assert!(!format!("{o:?}").contains("0123456789abcdef0123"));
    }
}

#[tokio::test]
async fn typesafe_writes_no_text_so_chat_never_reaches_it() {
    let env = env().await;
    env.mock.clear();
    let decision_model = "The model `jev-latest` is a decision model; send it to `/v1/decisions` or `/v1/systemone`.";
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("jev-latest", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], decision_model);
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("jev-latest", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], decision_model);
    assert!(env.mock.last().is_none());
}

#[tokio::test]
async fn model_checks_read_typesafes_listing_and_never_call_an_unlisted_model_unavailable() {
    let env = env().await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let r = env
        .app
        .post(
            &format!("{base}/models/check"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    // Only the model the listing leaves out is in doubt, and it is never called unavailable.
    let failed: Vec<(String, String, String)> = r.body["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["model_id"].as_str().unwrap().to_owned(),
                f["reason"].as_str().unwrap().to_owned(),
                f["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        failed,
        [(
            "jev-1.12.0".to_owned(),
            "inconclusive".to_owned(),
            "TypeSafe cannot confirm whether it serves the model to this key.".to_owned()
        )]
    );
    let r = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let availability = |model: &str| {
        r.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == model)
            .unwrap()["availability"]
            .clone()
    };
    for listed in ["jev-latest", "jev-1.14.0", "jev-1.13.0"] {
        assert_eq!(availability(listed), "available", "{listed}");
    }
    assert_ne!(availability("jev-1.12.0"), "unavailable");
    let gets: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET" && r.header("authorization").is_some())
        .map(|r| r.path)
        .collect();
    assert!(gets.iter().any(|p| p == "/v1/models"), "{gets:?}");
}
