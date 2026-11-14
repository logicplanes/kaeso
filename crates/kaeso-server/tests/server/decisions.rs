#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::{Answer, Capture, Env};
use http_body_util::BodyExt;
use kaeso_server::model_api::hooks::Outcome;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use tower::ServiceExt;

const GOOGLE_SECRET: &str = "AIzaSyTestGoogleDecisionCredential00000000";
const OPENROUTER_SECRET: &str = "sk-or-v1-test-openrouter-decisions-000000000000";
const VERCEL_SECRET: &str = "vck_test-vercel-decisions-0000000000000000";
const BASETEN_SECRET: &str = "test-baseten-decisions-00000000000000";
const FIREWORKS_SECRET: &str = "fw_test-fireworks-decisions-000000000000";
const PERPLEXITY_SECRET: &str = "pplx-testperplexitydecisions000000000000";

const MAMMOTH: &str = "decision-models/baseten-mammoth-27b-latest";
const RUNE: &str = "accounts/fireworks/routers/rune";

/// Where a decision request goes, and the format the provider takes there.
struct Target {
    model: &'static str,
    provider: &'static str,
    path: &'static str,
    format: &'static str,
}

const TARGETS: [Target; 7] = [
    Target {
        model: "gpt-6-luna",
        provider: "openai",
        path: "/v1/decisions",
        format: "openai_decisions",
    },
    Target {
        model: "gemini-decision-1",
        provider: "google",
        path: "/v1beta/decisions",
        format: "gemini_decisions",
    },
    Target {
        model: "typesafe/jev-1.14",
        provider: "openrouter",
        path: "/v1/systemone",
        format: "system_one",
    },
    Target {
        model: "typesafe-ai/jev",
        provider: "vercel",
        path: "/v1/decisions",
        format: "openai_decisions",
    },
    Target {
        model: MAMMOTH,
        provider: "baseten",
        path: "/v1/systemone",
        format: "system_one",
    },
    Target {
        model: RUNE,
        provider: "fireworks-ai",
        path: "/v1/systemone",
        format: "system_one",
    },
    Target {
        model: "pplx-decider-v1.2",
        provider: "perplexity",
        path: "/v1/decisions",
        format: "system_one",
    },
];

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
            App::SystemOne => json!({
                "model": model,
                "state": state,
                "questions": {
                    "refund": { "type": "noul", "instructions": "Does the customer ask for a refund?" },
                    "team": { "type": "choice", "instructions": "Which team handles it?",
                              "criteria": { "billing": "Payments and refunds.", "support": "Everything else." } },
                    "urgency": { "type": "score", "instructions": "How urgent is it?",
                                 "criteria": ["low", "medium", "high"] }
                }
            }),
            App::Gemini => json!({
                "model": model,
                "input": state,
                "questions": {
                    "refund": { "type": "noul", "instructions": "Does the customer ask for a refund?" },
                    "team": { "type": "choice", "instructions": "Which team handles it?",
                              "criteria": { "billing": "Payments and refunds.", "support": "Everything else." } },
                    "urgency": { "type": "score", "instructions": "How urgent is it?",
                                 "criteria": ["low", "medium", "high"] }
                }
            }),
        }
    }

    // The yes/no answer, as the app's format writes it.
    fn refund_probability(self, reply: &Value) -> f64 {
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

async fn env() -> Env {
    let capture = Arc::new(Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    for (provider, secret) in [
        ("google", GOOGLE_SECRET),
        ("openrouter", OPENROUTER_SECRET),
        ("vercel", VERCEL_SECRET),
        ("baseten", BASETEN_SECRET),
        ("fireworks-ai", FIREWORKS_SECRET),
        ("perplexity", PERPLEXITY_SECRET),
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
    let custom = |model: &str, provider: &str| {
        json!({ "provider": provider, "model_id": model, "custom": true, "overrides": {
            "input_modalities": ["text"], "input_price": "0.042",
            "decisions": { "only": true, "max_options": 255, "max_levels": 10 } } })
    };
    let mut adds: Vec<Value> = TARGETS
        .iter()
        .filter(|t| ![MAMMOTH, RUNE].contains(&t.model))
        .map(|t| json!({ "provider": t.provider, "model_id": t.model }))
        .collect();
    adds.push(custom(MAMMOTH, "baseten"));
    adds.push(custom(RUNE, "fireworks-ai"));
    adds.push(json!({ "provider": "openrouter", "model_id": "openai/gpt-5" }));
    for add in adds {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                add.clone(),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{add}: {:?}", r.body);
    }
    env
}

async fn send(env: &Env, app: App, body: &Value) -> Answer {
    send_to(env, app.path(), app, body, &[]).await
}

async fn send_to(
    env: &Env,
    path: &str,
    app: App,
    body: &Value,
    headers: &[(&str, &str)],
) -> Answer {
    let mut req = Request::builder()
        .uri(path)
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

async fn outcome(env: &Env, r: &Answer) -> Outcome {
    env.capture
        .recorded(r.header("kaeso-request-id").unwrap())
        .await
}

fn catalogue_cost(o: &Outcome) -> Decimal {
    kaeso_server::services::usage::costed(o).1
}

#[tokio::test]
async fn every_decision_format_reaches_every_decisions_provider_in_its_own_format() {
    let env = env().await;
    for app in APPS {
        for t in &TARGETS {
            env.mock.clear();
            let r = send(
                &env,
                app,
                &app.body(t.model, "I was charged twice; refund me."),
            )
            .await;
            assert_eq!(
                r.status,
                StatusCode::OK,
                "{app:?} → {}: {}",
                t.model,
                r.text()
            );
            let reply = r.json();
            assert!(
                (app.refund_probability(&reply) - 0.82).abs() < 1e-9,
                "{app:?} → {}: {reply}",
                t.model
            );
            assert_eq!(r.header("kaeso-provider"), Some(t.provider), "{}", t.model);
            assert_eq!(r.header("kaeso-model"), Some(t.model));
            let sent = env.mock.last().unwrap();
            assert_eq!(sent.path, t.path, "{app:?} → {}", t.model);
            let body = sent.json();
            assert_eq!(body["model"], t.model);
            match t.format {
                "openai_decisions" => assert!(body["questions"].is_array(), "{body}"),
                "system_one" => assert!(body["state"].is_string(), "{body}"),
                _ => assert!(body["input"].is_string() && body["questions"].is_object()),
            }
            let o = outcome(&env, &r).await;
            assert_eq!(o.egress.unwrap().as_str(), t.format);
            assert_eq!(
                kaeso_server::services::logs::endpoint(&o),
                match app {
                    App::SystemOne => "systemone",
                    App::OpenAi | App::Gemini => "decisions",
                }
            );
        }
    }
    let gemini = send(&env, App::Gemini, &App::Gemini.body("gpt-6-luna", "Hi")).await;
    assert!(
        gemini.json()["usage"]["total_input_tokens"].is_u64(),
        "a Gemini app reads usage as Gemini writes it: {}",
        gemini.text()
    );
    let key = env.mock.last().unwrap();
    assert_eq!(
        key.header("authorization"),
        Some(format!("Bearer {}", common::v1::OPENAI_SECRET).as_str())
    );
    send(
        &env,
        App::OpenAi,
        &App::OpenAi.body("gemini-decision-1", "Hi"),
    )
    .await;
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.header("x-goog-api-key"), Some(GOOGLE_SECRET));
}

#[tokio::test]
async fn decisions_kaeso_cant_send_are_refused_before_anything_is_sent() {
    let env = env().await;
    let refused = |r: &Answer, app: App| (r.status, app.error_message(&r.json()));
    let cases: Vec<(App, Value, &str)> = vec![
        (
            App::OpenAi,
            App::OpenAi.body("gpt-4o", "Hi"),
            "The model `gpt-4o` is a generation model; it can't answer decision questions.",
        ),
        (
            App::SystemOne,
            App::SystemOne.body("openai/gpt-5", "Hi"),
            "The model `openai/gpt-5` is a generation model; it can't answer decision questions.",
        ),
        (
            App::Gemini,
            App::Gemini.body("claude-haiku-4-5", "Hi"),
            "Anthropic offers no decisions API Kaeso can call, so `claude-haiku-4-5` can't make decisions.",
        ),
        (
            App::SystemOne,
            json!({ "model": "gpt-6-luna", "state": "A photo",
                    "questions": { "where": { "type": "box", "instructions": "the cat" } } }),
            "The model `gpt-6-luna` does not answer box questions.",
        ),
        (
            App::OpenAi,
            json!({ "model": "gpt-6-luna", "input": "Hi", "questions":
                (0..401).map(|i| json!({ "type": "predicate", "name": format!("q{i}"), "instructions": "Yes?" }))
                    .collect::<Vec<_>>() }),
            "The model `gpt-6-luna` answers at most 400 questions per request.",
        ),
    ];
    env.mock.clear();
    for (app, body, message) in cases {
        let r = send(&env, app, &body).await;
        assert_eq!(
            refused(&r, app),
            (StatusCode::BAD_REQUEST, message.to_owned()),
            "{app:?}"
        );
    }
    let url_image = json!({ "model": "typesafe/jev-1.14", "input": [{ "role": "user", "content": [
        { "type": "input_text", "text": "Is this a cat?" },
        { "type": "input_image", "image_url": "https://example.com/cat.png" }] }],
        "questions": [{ "type": "predicate", "instructions": "A cat?" }] });
    let r = send(&env, App::OpenAi, &url_image).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        App::OpenAi.error_message(&r.json()).contains("data URL"),
        "{}",
        r.text()
    );
    assert!(env.mock.received().is_empty(), "{:?}", env.mock.received());

    let chat = common::v1::chat("gemini-decision-1", "Hi");
    let r = common::v1::call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.json()["error"]["message"],
        "The model `gemini-decision-1` is a decision model; send it to `/v1/decisions` or `/v1/systemone`."
    );

    let system_one_at_decisions = App::SystemOne.body("gpt-6-luna", "Hi");
    let r = send_to(
        &env,
        "/v1/decisions",
        App::OpenAi,
        &system_one_at_decisions,
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        App::OpenAi
            .error_message(&r.json())
            .contains("/v1/systemone"),
        "{}",
        r.text()
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    for (app, status) in [
        (App::OpenAi, StatusCode::BAD_REQUEST),
        (App::SystemOne, StatusCode::BAD_REQUEST),
        (App::Gemini, StatusCode::BAD_REQUEST),
    ] {
        let r = send(&env, app, &app.body("claude-haiku-4-5", "Hi")).await;
        assert_eq!(r.status, status, "{app:?}");
        let body = r.json();
        match app {
            App::OpenAi => assert_eq!(body["error"]["type"], "invalid_request_error", "{body}"),
            App::SystemOne => assert!(body["detail"].is_string(), "{body}"),
            App::Gemini => {
                assert_eq!(body["error"]["code"], 400, "{body}");
                assert_eq!(body["error"]["status"], "INVALID_ARGUMENT", "{body}");
            }
        }
    }
    for (app, model, fixture) in [
        (App::SystemOne, "gpt-6-luna", "overloaded-503"),
        (App::OpenAi, "gemini-decision-1", "overloaded-503"),
        (App::Gemini, "pplx-decider-v1.2", "overloaded-529"),
    ] {
        let body = app.body(model, &format!("Hi [mock:error={fixture}]"));
        let r = send(&env, app, &body).await;
        assert!(
            r.status.is_server_error(),
            "{app:?}: {} {}",
            r.status,
            r.text()
        );
        let message = app.error_message(&r.json());
        assert!(!message.is_empty(), "{app:?}: {}", r.text());
    }
    let r = send(
        &env,
        App::Gemini,
        &App::Gemini.body("gpt-6-luna", "Hi [mock:refuse]"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("refund"),
        "a Gemini app learns which questions OpenAI refused: {}",
        r.text()
    );
    let r = send(
        &env,
        App::SystemOne,
        &App::SystemOne.body("gpt-6-luna", "Hi [mock:refuse]"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["answers"]["refund"]["type"], "refusal");
    let r = send_to(
        &env,
        "/v1/decisions",
        App::OpenAi,
        &App::SystemOne.body("gpt-6-luna", "Hi"),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("`/v1/systemone`"),
        "a System One body sent to OpenAI's path is pointed to its own: {}",
        r.text()
    );
}

#[tokio::test]
async fn usage_and_cost_follow_each_providers_report_and_output_is_free() {
    let env = env().await;
    let r = send(
        &env,
        App::SystemOne,
        &App::SystemOne.body("gpt-6-luna", "Hi"),
    )
    .await;
    let o = outcome(&env, &r).await;
    let u = o.usage.clone().unwrap();
    assert_eq!((u.input_tokens, u.cache_read_tokens), (200, 1000));
    assert!(!o.usage_estimated);
    assert_eq!(
        catalogue_cost(&o),
        Decimal::new(3, 5),
        "200 × $0.10 + 1,000 × $0.01 per million"
    );

    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let with_image = json!({ "model": "gemini-decision-1", "input": [{ "role": "user", "content": [
        { "type": "input_text", "text": "Is this a cat?" },
        { "type": "input_image", "image_url": format!("data:image/png;base64,{png}") }] }],
        "questions": [{ "type": "predicate", "name": "cat", "instructions": "A cat?" }] });
    let r = send(&env, App::OpenAi, &with_image).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let u = outcome(&env, &r).await.usage.unwrap();
    assert_eq!((u.input_tokens, u.input_image_tokens), (300, Some(258)));

    let r = env
        .app
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    let decider = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "pplx-decider-v1.2")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/models/{decider}",
                env.owner.org_id, env.project
            ),
            Some(json!({ "overrides": { "output_price": "5" } })),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let r = send(
        &env,
        App::OpenAi,
        &App::OpenAi.body("pplx-decider-v1.2", "Hi"),
    )
    .await;
    let o = outcome(&env, &r).await;
    assert_eq!(
        o.usage.as_ref().unwrap().output_tokens,
        3,
        "one per question"
    );
    assert_eq!(
        catalogue_cost(&o),
        Decimal::new(24, 7),
        "120 input tokens at $0.02 per million; output costs nothing, whatever its price"
    );
}

#[tokio::test]
async fn adaptations_and_hints_left_out_are_named_in_the_reply_headers() {
    let env = env().await;
    let with_criteria = json!({ "model": "gpt-6-luna", "state": "Hi", "questions": {
        "refund": { "type": "noul", "instructions": "A refund?",
                    "criteria": { "true": "They ask for money back.", "false": "Anything else." } } } });
    let r = send(&env, App::SystemOne, &with_criteria).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("criteria"));
    let with_user = json!({ "model": "pplx-decider-v1.2", "input": "Hi", "safety_identifier": "user-7",
        "questions": [{ "type": "predicate", "name": "refund", "instructions": "A refund?" }] });
    let r = send(&env, App::OpenAi, &with_user).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("user"));
    assert!(
        env.mock
            .last()
            .unwrap()
            .json()
            .get("safety_identifier")
            .is_none()
    );
}

async fn logged_content(env: &Env, o: &Outcome) -> Value {
    let state = &env.app.state;
    let clickhouse = state.clickhouse.as_ref().unwrap();
    clickhouse.migrate(&state.pg).await.unwrap();
    kaeso_server::services::logs::record(state, o).await;
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
    r.body
}

#[tokio::test]
async fn decisions_are_logged_with_their_request_and_answers() {
    let env = env().await;
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let body = json!({ "model": "typesafe/jev-1.14", "state": "Is <photo> a cat?",
        "assets": { "<photo>": { "type": "image", "format": "png", "data_base64": png } },
        "questions": { "cat": { "type": "noul", "instructions": "A cat?" } } });
    let r = send(&env, App::SystemOne, &body).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env, &r).await;
    let log = logged_content(&env, &o).await;
    assert_eq!(log["endpoint"], "systemone");
    assert_eq!(log["ingress_dialect"], "system_one");
    assert_eq!(log["egress_dialect"], "system_one");
    let content = &log["content"];
    assert_eq!(
        content["request"]["assets"]["<photo>"]["data_base64"]["kaeso_media"]["media_type"],
        "image/png",
        "{content}"
    );
    assert_eq!(content["reply"]["format"], 2, "{content}");
    let answer = &content["reply"]["decide"]["answers"][0];
    assert_eq!(
        answer["value"]["predicate"]["probability"], 0.82,
        "{content}"
    );
}

async fn platform_text(env: &Env, path: &str) -> String {
    let req = Request::builder()
        .uri(format!("/platform/v1{path}"))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// System One names its questions and a choice's options as object keys, and the answers come back
// in that order: the log serves the body as the app wrote it, so the console labels each answer
// with its own question and option.
#[tokio::test]
async fn a_logged_decision_keeps_its_questions_and_options_in_the_apps_order() {
    let env = env().await;
    let body = r#"{"model": "typesafe/jev-1.14", "state": "I was charged twice.", "questions": {
        "team": {"type": "choice", "instructions": "Which team?",
                 "criteria": {"support": "Everything else.", "billing": "Payments."}},
        "refund": {"type": "noul", "instructions": "A refund?"}}}"#;
    let req = Request::builder()
        .uri(App::SystemOne.path())
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", env.key))
        .body(Body::from(body))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let id = resp.headers()["kaeso-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    env.app.state.in_flight.settled().await;
    let o = env.capture.recorded(&id).await;
    logged_content(&env, &o).await;
    let base = format!("/orgs/{}/projects/{}/logs", env.owner.org_id, env.project);
    for text in [
        platform_text(&env, &format!("{base}/{id}")).await,
        platform_text(&env, &format!("{base}/export?include_content=true")).await,
    ] {
        let at = |key: &str| {
            text.find(&format!("\"{key}\""))
                .unwrap_or_else(|| panic!("{key} in {text}"))
        };
        assert!(at("team") < at("refund"), "{text}");
        assert!(at("support") < at("billing"), "{text}");
        assert!(at("state") < at("questions"), "{text}");
    }
}

#[tokio::test]
async fn a_multipart_system_one_upload_is_decided_and_logged_as_its_fields() {
    let env = env().await;
    let request = json!({ "model": "typesafe/jev-1.14", "state": "Is <photo> a cat?",
        "assets": { "<photo>": { "part": "photo" } },
        "questions": { "cat": { "type": "noul", "instructions": "A cat?" } } });
    let png = base64_png();
    let mut form = Vec::new();
    form.extend_from_slice(
        format!("--kb\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{request}\r\n").as_bytes(),
    );
    form.extend_from_slice(b"--kb\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"cat.png\"\r\nContent-Type: image/png\r\n\r\n");
    form.extend_from_slice(&png);
    form.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri("/v1/systemone")
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "multipart/form-data; boundary=kb")
        .body(Body::from(form))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let id = resp.headers()["kaeso-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let status = resp.status();
    let text = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{text}");
    env.app.state.in_flight.settled().await;
    let o = env.capture.recorded(&id).await;
    let log = logged_content(&env, &o).await;
    let request = &log["content"]["request"];
    assert_eq!(request["model"], "typesafe/jev-1.14", "{request}");
    assert_eq!(
        request["assets"]["<photo>"]["source"]["base64"]["data"]["kaeso_media"]["reason"],
        "not_kept",
        "{request}"
    );
}

fn base64_png() -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
        .unwrap()
}

#[tokio::test]
async fn a_repeated_decision_is_answered_from_the_response_cache() {
    let env = env().await;
    env.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&env.app.state.pg)
        .await
        .unwrap();
    env.app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/response-cache",
                env.owner.org_id, env.project
            ),
            Some(json!({ "enabled": true })),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let body = App::SystemOne.body("gemini-decision-1", "Cache me");
    let first = send(&env, App::SystemOne, &body).await;
    assert_eq!(
        first.header("kaeso-cache"),
        Some("miss"),
        "{}",
        first.text()
    );
    let sent = env.mock.received().len();
    let second = send(&env, App::SystemOne, &body).await;
    assert_eq!(second.header("kaeso-cache"), Some("hit"));
    assert_eq!(second.json(), first.json());
    assert_eq!(env.mock.received().len(), sent);
    let o = outcome(&env, &second).await;
    assert_eq!(o.response_model.as_deref(), Some("gemini-decision-1"));
    assert_eq!(catalogue_cost(&o), Decimal::ZERO);
}

#[tokio::test]
async fn an_alias_falls_back_from_a_step_that_cant_decide_to_one_that_can() {
    let env = env().await;
    let models = env
        .app
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    let id = |model: &str| {
        models.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == model)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let route = json!({ "type": "model", "model": id("claude-haiku-4-5"),
        "fallback": { "type": "model", "model": id("gpt-6-luna") } });
    env.app
        .post(
            &format!(
                "/orgs/{}/projects/{}/aliases",
                env.owner.org_id, env.project
            ),
            json!({ "name": "decider", "route": route }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let r = send(&env, App::Gemini, &App::Gemini.body("decider", "Hi")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-6-luna"));
    let route = r.header("kaeso-route").unwrap();
    assert!(
        route.contains("claude-haiku-4-5") && route.contains("gpt-6-luna"),
        "{route}"
    );
    assert_eq!(env.mock.last().unwrap().path, "/v1/decisions");

    let formats = json!({ "type": "condition", "match": "all",
        "rules": [{ "field": "format", "op": "in", "value": ["system_one"] }],
        "then": { "type": "model", "model": id("pplx-decider-v1.2") },
        "else": { "type": "model", "model": id("gemini-decision-1") } });
    env.app
        .post(
            &format!(
                "/orgs/{}/projects/{}/aliases",
                env.owner.org_id, env.project
            ),
            json!({ "name": "by-format", "route": formats }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let r = send(
        &env,
        App::SystemOne,
        &App::SystemOne.body("by-format", "Hi"),
    )
    .await;
    assert_eq!(
        r.header("kaeso-model"),
        Some("pplx-decider-v1.2"),
        "{}",
        r.text()
    );
    let r = send(&env, App::OpenAi, &App::OpenAi.body("by-format", "Hi")).await;
    assert_eq!(
        r.header("kaeso-model"),
        Some("gemini-decision-1"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn a_gemini_app_sends_its_key_in_the_query_and_no_other_parameter() {
    let env = env().await;
    let body = App::Gemini.body("gemini-decision-1", "Hi");
    let req = Request::builder()
        .uri(format!("/v1beta/decisions?key={}", env.key))
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sent = env.mock.last().unwrap();
    assert!(sent.query.is_none(), "{:?}", sent.query);
    let r = send_to(&env, "/v1beta/decisions?alt=sse", App::Gemini, &body, &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"]["status"], "INVALID_ARGUMENT");
    let r = send_to(&env, "/v1/systemone", App::SystemOne, &body, &[]).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert_eq!(r.json()["detail"][0]["loc"], json!(["body", "state"]));
}

#[tokio::test]
async fn v1_decisions_takes_geminis_body_only_from_an_app_with_geminis_key() {
    let env = env().await;
    let gemini = App::Gemini.body("gpt-6-luna", "I was charged twice; refund me.");
    let r = send_to(&env, "/v1/decisions", App::Gemini, &gemini, &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        (App::Gemini.refund_probability(&r.json()) - 0.82).abs() < 1e-9,
        "{}",
        r.text()
    );
    let o = outcome(&env, &r).await;
    assert_eq!(o.ingress, kaeso_kpp::DialectId::GeminiDecisions);
    assert_eq!(kaeso_server::services::logs::endpoint(&o), "decisions");

    let req = Request::builder()
        .uri(format!("/v1/decisions?key={}", env.key))
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&gemini).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(env.mock.last().unwrap().query.is_none());
    let r = send_to(&env, "/v1/decisions?alt=sse", App::Gemini, &gemini, &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"]["status"], "INVALID_ARGUMENT");

    let bearer = format!("Bearer {}", env.key);
    let r = send_to(
        &env,
        "/v1/decisions",
        App::Gemini,
        &gemini,
        &[("authorization", bearer.as_str())],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["type"],
        "invalid_request_error",
        "an app that also sends `Authorization` speaks OpenAI's format: {}",
        r.text()
    );
    let r = send_to(&env, "/v1/decisions", App::OpenAi, &gemini, &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "invalid_request_error");
    let openai = App::OpenAi.body("gpt-6-luna", "Hi");
    let r = send_to(&env, "/v1/decisions", App::OpenAi, &openai, &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        outcome(&env, &r).await.ingress,
        kaeso_kpp::DialectId::OpenAiDecisions
    );
}

#[tokio::test]
async fn the_catalogue_names_the_format_a_decision_only_model_is_sent_in() {
    let env = env().await;
    let r = env
        .app
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let sent_as = |model: &str| {
        r.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == model)
            .unwrap_or_else(|| panic!("{model} is in the catalogue: {}", r.body))["egress_dialect"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(sent_as("gemini-decision-1"), "gemini_decisions");
    assert_eq!(sent_as("pplx-decider-v1.2"), "system_one");
    assert_eq!(sent_as(MAMMOTH), "system_one");
    assert_eq!(
        sent_as("gpt-6-luna"),
        "openai_responses",
        "a model that also generates keeps its generation format"
    );
}
