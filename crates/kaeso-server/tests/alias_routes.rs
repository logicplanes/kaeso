#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::v1::{Answer, Capture, Env, call, call_app, chat, messages};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
enum App {
    Messages,
    Chat,
    Responses,
}

const APPS: [App; 3] = [App::Messages, App::Chat, App::Responses];

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
}

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn model_id(env: &Env, model: &str) -> String {
    let models = env
        .app
        .get(&format!("{}/models", base(env)), &env.owner.cookie)
        .await;
    models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == model)
        .unwrap_or_else(|| panic!("{model} is in the catalogue"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn alias(env: &Env, name: &str, route: Value) -> Value {
    let r = env
        .app
        .post(
            &format!("{}/aliases", base(env)),
            json!({ "name": name, "route": route }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn ask_as(env: &Env, key: &str, app: App, model: &str, text: &str, stream: bool) -> Answer {
    call(
        env,
        app.path(),
        Some(key),
        matches!(app, App::Messages),
        Some(app.body(model, text, stream)),
    )
    .await
}

async fn ask(env: &Env, app: App, model: &str, text: &str, stream: bool) -> Answer {
    ask_as(env, &env.key, app, model, text, stream).await
}

async fn ask_with(env: &Env, model: &str, headers: &[(&str, &str)]) -> Answer {
    let mut req = axum::http::Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", env.key));
    for (n, v) in headers {
        req = req.header(*n, *v);
    }
    let req = req
        .body(axum::body::Body::from(
            serde_json::to_vec(&chat(model, "Hi")).unwrap(),
        ))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = http_body_util::BodyExt::collect(resp.into_body())
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

fn model_step(model: &str) -> Value {
    json!({ "type": "model", "model": model })
}

fn with_fallback(model: &str, fallback: Value) -> Value {
    json!({ "type": "model", "model": model, "fallback": fallback })
}

fn walked(r: &Answer) -> Vec<String> {
    r.header("kaeso-route")
        .map(|h| {
            h.split(", ")
                .map(|step| {
                    step.split(';')
                        .filter(|p| !p.starts_with("dur="))
                        .collect::<Vec<_>>()
                        .join(";")
                })
                .collect()
        })
        .unwrap_or_default()
}

fn asked_models(env: &Env) -> Vec<String> {
    env.mock
        .received()
        .iter()
        .filter_map(|r| r.json()["model"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn a_route_falls_back_when_its_model_is_overloaded_in_every_format() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    for app in APPS {
        for stream in [false, true] {
            let what = format!("{app:?} (stream {stream})");
            let r = ask(
                &env,
                app,
                "sturdy",
                "Hi [mock:fail=claude-haiku-4-5 overloaded]",
                stream,
            )
            .await;
            assert_eq!(r.status, StatusCode::OK, "{what}: {}", r.text());
            assert_eq!(r.header("kaeso-model"), Some("gpt-4o"), "{what}");
            assert_eq!(r.header("kaeso-provider"), Some("openai"), "{what}");
            assert_eq!(
                walked(&r),
                [
                    "\"route\";model=\"anthropic/claude-haiku-4-5\";status=529",
                    "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
                ],
                "{what}"
            );
            env.app.clock.advance(time::Duration::seconds(31));
        }
    }
    let outcomes = env.outcomes().await;
    assert_eq!(outcomes.len(), 6);
    for o in outcomes {
        assert_eq!(o.status, 200);
        assert_eq!(o.alias.as_deref(), Some("sturdy"));
        assert_eq!(o.requested_model.as_deref(), Some("sturdy"));
        assert_eq!(o.model_id.as_deref(), Some("gpt-4o"));
        assert_eq!(o.fallbacks.len(), 1);
        let f = &o.fallbacks[0];
        assert_eq!(
            (f.path.as_str(), f.model.as_str()),
            ("route", "anthropic/claude-haiku-4-5")
        );
        assert_eq!((f.status, f.error_kind.as_str()), (529, "overloaded"));
        assert!(!f.error_message.is_empty());
    }
}

#[tokio::test]
async fn a_route_that_works_names_the_one_step_it_tried() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let r = ask(&env, App::Chat, "sturdy", "Hi", false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200"]
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5"]);
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.alias.as_deref(), Some("sturdy"));
    assert!(o.fallbacks.is_empty());
}

#[tokio::test]
async fn a_failure_the_step_does_not_fall_back_on_goes_back_to_the_app() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "picky",
        json!({ "type": "model", "model": claude, "fall_back_on": ["limited"], "fallback": model_step(&gpt) }),
    )
    .await;
    let r = ask(
        &env,
        App::Messages,
        "picky",
        "Hi [mock:fail=claude-haiku-4-5 overloaded]",
        false,
    )
    .await;
    assert_eq!(r.status.as_u16(), 529, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "overloaded_error");
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=529"]
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5"]);
    let r = ask(
        &env,
        App::Messages,
        "picky",
        "Hi [mock:fail=claude-haiku-4-5 rate-limit]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=429",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
}

#[tokio::test]
async fn a_refused_request_falls_back_only_when_the_step_says_so() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "default", with_fallback(&claude, model_step(&gpt))).await;
    alias(
        &env,
        "forgiving",
        json!({ "type": "model", "model": claude, "fall_back_on": ["unavailable", "rejected"], "fallback": model_step(&gpt) }),
    )
    .await;
    let text = "Hi [mock:fail=claude-haiku-4-5 invalid-request-prefill]";
    let r = ask(&env, App::Messages, "default", text, false).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = ask(&env, App::Messages, "forgiving", text, false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
}

#[tokio::test]
async fn when_every_step_fails_the_app_gets_the_last_error_and_the_earlier_ones_by_name() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "Hi [mock:fail=claude-haiku-4-5 overloaded] [mock:fail=gpt-4o rate-limit-tokens-429]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=529",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=429"
        ]
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5", "gpt-4o"]);
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.status, 429);
    assert_eq!(o.model_id.as_deref(), Some("gpt-4o"));
    assert_eq!(o.error_kind.as_deref(), Some("rate_limited"));
    assert_eq!(o.fallbacks.len(), 1);
}

#[tokio::test]
async fn a_step_that_failed_for_a_lasting_reason_is_skipped_until_the_wait_ends() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "Hi [mock:fail=claude-haiku-4-5 overloaded]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = ask(&env, App::Chat, "sturdy", "Hi again", false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";skipped",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5", "gpt-4o", "gpt-4o"]);
    let skipped = env.outcomes().await.pop().unwrap().fallbacks;
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].skipped);
    assert_eq!(
        (
            skipped[0].status,
            skipped[0].error_kind.as_str(),
            skipped[0].duration_ms
        ),
        (529, "overloaded", 0)
    );

    env.app.clock.advance(time::Duration::seconds(31));
    let r = ask(&env, App::Chat, "sturdy", "Hi once more", false).await;
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200"]
    );
    let r = ask(&env, App::Chat, "sturdy", "And again", false).await;
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
}

#[tokio::test]
async fn an_empty_balance_skips_every_model_of_the_provider_at_once() {
    let env = env().await;
    let added = env
        .app
        .post(
            &format!("{}/models", base(&env)),
            json!({ "provider": "anthropic", "model_id": "claude-sonnet-4-5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(added.status, StatusCode::CREATED, "{:?}", added.body);
    let haiku = model_id(&env, "claude-haiku-4-5").await;
    let sonnet = model_id(&env, "claude-sonnet-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "save",
        with_fallback(&haiku, with_fallback(&sonnet, model_step(&gpt))),
    )
    .await;
    let r = ask(
        &env,
        App::Messages,
        "save",
        "Hi [mock:fail=claude-haiku-4-5 billing-credit-balance]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=502",
            "\"route.fallback\";model=\"anthropic/claude-sonnet-4-5\";skipped",
            "\"route.fallback.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5", "gpt-4o"]);
    let fallbacks = env.outcomes().await.pop().unwrap().fallbacks;
    let kinds: Vec<_> = fallbacks
        .iter()
        .map(|f| (f.model.as_str(), f.error_kind.as_str(), f.skipped))
        .collect();
    assert_eq!(
        kinds,
        [
            ("anthropic/claude-haiku-4-5", "billing", false),
            ("anthropic/claude-sonnet-4-5", "billing", true)
        ]
    );
    let r = ask(&env, App::Messages, "save", "Hi again", false).await;
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(asked_models(&env), ["claude-haiku-4-5", "gpt-4o", "gpt-4o"]);
    env.app.clock.advance(time::Duration::minutes(5));
    let r = ask(&env, App::Messages, "save", "Topped up", false).await;
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
}

#[tokio::test]
async fn a_step_held_by_one_gateway_is_skipped_by_every_other_until_one_sees_it_answer() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let other = env.second_gateway().await;
    let ask_other = |text: &'static str| {
        call_app(
            &other,
            App::Chat.path(),
            Some(&env.key),
            false,
            Some(App::Chat.body("sturdy", text, false)),
        )
    };
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "Hi [mock:fail=claude-haiku-4-5 overloaded]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = ask_other("Hi from the other gateway").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";skipped",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ],
        "the other gateway never called the held model"
    );
    assert_eq!(asked_models(&env), ["claude-haiku-4-5", "gpt-4o", "gpt-4o"]);

    env.app.clock.advance(time::Duration::seconds(31));
    let r = ask_other("The wait is over").await;
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "Hi [mock:fail=claude-haiku-4-5 overloaded]",
        false,
    )
    .await;
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    let r = ask_other("Held again, seen by the other").await;
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    env.app.clock.advance(time::Duration::seconds(31));
    let r = ask_other("Answers again").await;
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "And the first gateway sees the release",
        false,
    )
    .await;
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200"]
    );
}

#[tokio::test]
async fn a_refusal_of_the_request_itself_never_holds_a_step() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "forgiving",
        json!({ "type": "model", "model": claude, "fall_back_on": ["unavailable", "rejected"], "fallback": model_step(&gpt) }),
    )
    .await;
    let r = ask(
        &env,
        App::Messages,
        "forgiving",
        "Hi [mock:fail=claude-haiku-4-5 invalid-request-prefill]",
        false,
    )
    .await;
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    let r = ask(&env, App::Messages, "forgiving", "Hi again", false).await;
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
}

#[tokio::test]
async fn a_held_step_is_tried_when_no_later_step_is_ready() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let r = ask(
        &env,
        App::Chat,
        "sturdy",
        "Hi [mock:fail=claude-haiku-4-5 overloaded] [mock:fail=gpt-4o rate-limit-tokens-429]",
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let r = ask(&env, App::Chat, "sturdy", "Hi again", false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200"]
    );
}

#[tokio::test]
async fn conditions_send_requests_by_header_feature_and_size() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "tiered",
        json!({
            "type": "condition", "match": "all",
            "rules": [{ "field": "header", "name": "x-plan", "op": "equals", "value": "paid" }],
            "then": model_step(&claude),
            "else": model_step(&gpt)
        }),
    )
    .await;
    let paid = ask_with(&env, "tiered", &[("x-plan", "paid")]).await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text());
    assert_eq!(paid.header("kaeso-model"), Some("claude-haiku-4-5"));
    let free = ask_with(&env, "tiered", &[("x-plan", "free")]).await;
    assert_eq!(free.header("kaeso-model"), Some("gpt-4o"));
    let none = ask_with(&env, "tiered", &[]).await;
    assert_eq!(none.header("kaeso-model"), Some("gpt-4o"));

    alias(
        &env,
        "long",
        json!({
            "type": "condition", "match": "any",
            "rules": [
                { "field": "input_tokens", "op": "at_least", "value": 2000 },
                { "field": "format", "op": "in", "value": ["anthropic_messages"] }
            ],
            "then": model_step(&claude),
            "else": model_step(&gpt)
        }),
    )
    .await;
    let short = ask(&env, App::Chat, "long", "Hi", false).await;
    assert_eq!(short.header("kaeso-model"), Some("gpt-4o"));
    let big = ask(&env, App::Chat, "long", &"word ".repeat(2000), false).await;
    assert_eq!(big.header("kaeso-model"), Some("claude-haiku-4-5"));
    let anthropic = ask(&env, App::Messages, "long", "Hi", false).await;
    assert_eq!(anthropic.header("kaeso-model"), Some("claude-haiku-4-5"));
}

#[tokio::test]
async fn a_caller_split_keeps_each_key_on_one_model() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "ab",
        json!({
            "type": "split", "by": "caller",
            "branches": [
                { "weight": 50, "step": model_step(&claude) },
                { "weight": 50, "step": model_step(&gpt) }
            ]
        }),
    )
    .await;
    let mut seen = BTreeSet::new();
    for _ in 0..12 {
        let r = ask(&env, App::Chat, "ab", "Hi", false).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        seen.insert(r.header("kaeso-model").unwrap().to_owned());
    }
    assert_eq!(seen.len(), 1, "one key stays on one branch: {seen:?}");
}

#[tokio::test]
async fn a_step_the_key_may_not_use_is_skipped_without_naming_it() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let only_gpt = env
        .app
        .post(
            &format!("{}/keys", base(&env)),
            json!({ "name": "gpt only", "models": [gpt] }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = ask_as(&env, &only_gpt, App::Chat, "sturdy", "Hi", false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&r),
        [
            "\"route\";skipped",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    assert_eq!(asked_models(&env), ["gpt-4o"]);

    alias(&env, "claude-only", model_step(&claude)).await;
    let r = ask_as(&env, &only_gpt, App::Chat, "claude-only", "Hi", false).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `claude-only` does not exist or this key may not use it."
    );
}

#[tokio::test]
async fn removing_a_model_keeps_the_routes_that_use_others_and_deletes_the_rest() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    let sturdy = alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    alias(&env, "haiku", model_step(&claude)).await;
    let paid = json!({
        "type": "condition",
        "match": "all",
        "rules": [{ "field": "header", "name": "x-plan", "op": "equals", "value": "paid" }],
        "then": model_step(&claude),
        "else": model_step(&gpt),
    });
    alias(&env, "by-plan", paid).await;
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("{}/models/{claude}", base(&env)),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let aliases = env
        .app
        .get(&format!("{}/aliases", base(&env)), &env.owner.cookie)
        .await;
    let data = aliases.body["data"].as_array().unwrap();
    let names: Vec<&str> = data.iter().filter_map(|a| a["name"].as_str()).collect();
    assert_eq!(names, ["by-plan", "sturdy"], "{data:?}");
    let kept = data.iter().find(|a| a["name"] == "sturdy").unwrap();
    assert_eq!(kept["models"][0]["target"], Value::Null);
    assert_eq!(kept["models"][1]["target"], "openai/gpt-4o");
    let r = ask(&env, App::Chat, "sturdy", "Hi", false).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&r),
        [
            "\"route\";skipped",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    let r = ask(&env, App::Chat, "haiku", "Hi", false).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = ask_with(&env, "by-plan", &[("x-plan", "paid")]).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert!(
        r.text().contains(
            "The alias `by-plan` has no model left for this request; ask a project admin."
        ),
        "{}",
        r.text()
    );
    assert_eq!(walked(&r), ["\"route.then\";skipped"]);
    let r = ask_with(&env, "by-plan", &[("x-plan", "free")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    let at = format!("{}/aliases/{}", base(&env), sturdy["id"].as_str().unwrap());
    let r = env
        .app
        .call(
            Method::PATCH,
            &at,
            Some(json!({ "name": "sturdier" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["name"], "sturdier");
    assert_eq!(r.body["route"], sturdy["route"]);
    let r = env
        .app
        .call(
            Method::PATCH,
            &at,
            Some(json!({ "route": sturdy["route"] })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "route.model");
}

#[tokio::test]
async fn a_step_that_takes_too_long_moves_on() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "quick",
        json!({ "type": "model", "model": claude, "timeout_seconds": 1, "fallback": model_step(&gpt) }),
    )
    .await;
    let started = std::time::Instant::now();
    let r = tokio::time::timeout(
        Duration::from_secs(20),
        ask(
            &env,
            App::Chat,
            "quick",
            "Hi [mock:fail=claude-haiku-4-5 stall]",
            false,
        ),
    )
    .await
    .expect("the stalled step gives up");
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=504",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    let tried = r.header("kaeso-route").unwrap_or_default();
    let ms: u64 = tried
        .split(", ")
        .next()
        .and_then(|first| first.split(";dur=").nth(1))
        .and_then(|ms| ms.parse().ok())
        .expect("a failed step names its duration");
    assert!((900..10_000).contains(&ms), "{tried}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn a_limit_on_one_model_sends_its_requests_to_the_fallback() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "capped", with_fallback(&claude, model_step(&gpt))).await;
    let r = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "request_rate", "model": claude, "period": "minute", "amount": 1 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let first = ask(&env, App::Chat, "capped", "Hi", false).await;
    assert_eq!(first.header("kaeso-model"), Some("claude-haiku-4-5"));
    let second = ask(&env, App::Chat, "capped", "Hi", false).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert_eq!(second.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&second),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=429",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
}

#[tokio::test]
async fn models_list_an_alias_as_its_main_model() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(&env, "sturdy", with_fallback(&claude, model_step(&gpt))).await;
    let r = call(&env, "/v1/models", Some(&env.key), false, None).await;
    assert_eq!(r.status, StatusCode::OK);
    let ids: Vec<String> = r.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&"sturdy".to_owned()), "{ids:?}");
    let one = call(&env, "/v1/models/sturdy", Some(&env.key), false, None).await;
    assert_eq!(one.status, StatusCode::OK, "{}", one.text());
    assert_eq!(one.json()["id"], "sturdy");
}

fn ab(by: &str, header: Option<&str>, a: &str, b: &str) -> Value {
    let mut split = json!({
        "type": "split", "by": by,
        "branches": [
            { "weight": 50, "step": model_step(a) },
            { "weight": 50, "step": model_step(b) }
        ]
    });
    if let Some(h) = header {
        split["header"] = json!(h);
    }
    split
}

#[tokio::test]
async fn a_header_split_keeps_each_end_user_on_one_model() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "users",
        ab("header", Some("x-user-id"), &claude, &gpt),
    )
    .await;
    let mut across = BTreeSet::new();
    for user in 0..16 {
        let value = format!("user-{user}");
        let mut seen = BTreeSet::new();
        for _ in 0..3 {
            let r = ask_with(&env, "users", &[("x-user-id", &value)]).await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.text());
            seen.insert(r.header("kaeso-model").unwrap().to_owned());
        }
        assert_eq!(seen.len(), 1, "{value} stays on one model: {seen:?}");
        across.extend(seen);
    }
    assert_eq!(across.len(), 2, "end users land on both models");
}

async fn trace(env: &Env, body: Value) -> common::Resp {
    env.app
        .post(
            &format!("{}/aliases/trace", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await
}

#[tokio::test]
async fn a_trace_shows_the_steps_a_request_would_take_and_sends_nothing() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    let route = json!({
        "type": "condition", "match": "any",
        "rules": [
            { "field": "header", "name": "x-plan", "op": "equals", "value": "enterprise" },
            { "field": "input_tokens", "op": "at_least", "value": 100000 }
        ],
        "then": with_fallback(&claude, model_step(&gpt)),
        "else": ab("request", None, &claude, &gpt)
    });
    let r = trace(
        &env,
        json!({
            "route": route,
            "request": { "headers": [{ "name": "x-plan", "value": "enterprise" }], "format": "openai_chat" }
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["steps"],
        json!([
            { "type": "condition", "path": "route", "holds": true, "rules": [true, false] },
            { "type": "model", "path": "route.then", "catalogue_model_id": claude, "attempt": 1 },
            { "type": "model", "path": "route.then.fallback", "catalogue_model_id": gpt, "attempt": 2 }
        ])
    );

    let mut branches = BTreeSet::new();
    for seed in 0..40u64 {
        let request = json!({ "input_tokens": 1200, "seed": seed });
        let once = trace(&env, json!({ "route": route, "request": request })).await;
        let again = trace(&env, json!({ "route": route, "request": request })).await;
        assert_eq!(once.body, again.body, "the same seed draws the same branch");
        let split = &once.body["steps"][1];
        assert_eq!(split["type"], "split");
        assert_eq!(split["path"], "route.else");
        let draw = split["draw"].as_u64().unwrap();
        assert_eq!(split["branch"], json!(u64::from(draw >= 50)));
        assert_eq!(split["weight"], 50);
        branches.insert(split["branch"].as_u64().unwrap());
    }
    assert_eq!(branches.len(), 2, "seeds draw both branches");
    assert!(env.mock.received().is_empty(), "a trace calls no model");
}

#[tokio::test]
async fn a_trace_draws_a_saved_alias_as_its_requests_do() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let gpt = model_id(&env, "gpt-4o").await;
    let route = ab("caller", None, &claude, &gpt);
    let saved = alias(&env, "ab", route.clone()).await;
    let keys = env
        .app
        .get(&format!("{}/keys", base(&env)), &env.owner.cookie)
        .await;
    let key = keys.body["data"][0]["id"].as_str().unwrap().to_owned();
    let r = trace(
        &env,
        json!({ "route": route, "alias": saved["id"], "request": { "key": key } }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let traced = r.body["steps"][1]["catalogue_model_id"].clone();
    let live = ask(&env, App::Chat, "ab", "Hi", false).await;
    let answered = live.header("kaeso-model").unwrap();
    let expected = if answered == "gpt-4o" { &gpt } else { &claude };
    assert_eq!(
        traced,
        json!(expected),
        "the trace picks the model the key gets"
    );
}

#[tokio::test]
async fn a_trace_refuses_what_saving_would_and_headers_routes_cannot_read() {
    let env = env().await;
    let claude = model_id(&env, "claude-haiku-4-5").await;
    let cases = [
        (
            json!({ "route": with_fallback(&claude, model_step("model_00000000000000000000000000")), "request": {} }),
            "route.fallback.model",
        ),
        (
            json!({ "route": model_step(&claude), "request": { "headers": [{ "name": "X Plan", "value": "a" }] } }),
            "request.headers.0.name",
        ),
        (
            json!({ "route": model_step(&claude), "request": { "headers": [{ "name": "authorization", "value": "Bearer x" }] } }),
            "request.headers.0.name",
        ),
        (
            json!({ "route": model_step(&claude), "request": { "headers": (0..21).map(|i| json!({ "name": format!("x-{i}"), "value": "v" })).collect::<Vec<_>>() } }),
            "request.headers",
        ),
    ];
    for (body, param) in cases {
        let r = trace(&env, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{param}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{:?}", r.body);
    }
}
