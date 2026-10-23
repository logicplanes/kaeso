#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::v1::{Answer, Capture, call_app, chat, messages};
use common::{App, ORIGIN, Owner};
use kaeso_server::model_api::hooks::Sink;
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::oauth::{Fault, MockOAuth, Reply};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const USAGE: &str =
    "Your ChatGPT plan's usage limit is reached. Review it at https://chatgpt.com/settings/usage.";
const SIGNED_OUT: &str = "The ChatGPT account of this project's OpenAI key is signed out. Ask an organization admin to sign in again under Providers.";
const HELLO: &str = "Hi there! How can I assist you today?";

struct Plan {
    app: App,
    oauth: MockOAuth,
    api: MockProvider,
    owner: Owner,
    project: String,
    credential: String,
    key: String,
    capture: Arc<Capture>,
    services: TestServices,
}

impl Plan {
    fn base(&self) -> String {
        format!("/orgs/{}/projects/{}", self.owner.org_id, self.project)
    }

    async fn call(&self, path: &str, anthropic: bool, body: Value) -> Answer {
        call_app(&self.app, path, Some(&self.key), anthropic, Some(body)).await
    }

    async fn access_token(&self) -> String {
        common::account_tokens(&self.app, &self.owner, &self.credential)
            .await
            .access_token
    }

    fn sent(&self) -> Vec<kaeso_testkit::mock_provider::Received> {
        self.oauth
            .received()
            .into_iter()
            .filter(|r| r.path.starts_with("/v1/"))
            .collect()
    }
}

async fn plan() -> Plan {
    plan_with(&[]).await
}

async fn plan_with(env: &[(&str, &str)]) -> Plan {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let api = MockProvider::start().await;
    let capture = Arc::new(Capture::default());
    let mut env = env.to_vec();
    env.push(("KAESO_PROVIDERS_OPENAI_BASE_URL", api.base_url.as_str()));
    let app = App::start_full(
        &s,
        &env,
        None,
        Some(capture.clone() as Arc<dyn Sink>),
        Some(common::accounts_at(oauth.base_url())),
    )
    .await;
    let owner = app.setup().await;
    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created =
        common::sign_in_chatgpt(&app, &owner, &oauth, "Ada's plan", "ada@example.com").await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    for model in ["gpt-5", "gpt-5-mini", "gpt-4o", "text-embedding-3-small"] {
        app.post(
            &format!("{base}/models"),
            json!({ "provider": "openai", "model_id": model }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    }
    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "app" }),
            Some(&owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    Plan {
        app,
        oauth,
        api,
        owner,
        project,
        credential: created["id"].as_str().unwrap().to_owned(),
        key,
        capture,
        services: s,
    }
}

fn sse(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn every_request_goes_to_the_responses_api_as_a_stream_without_refused_fields() {
    let p = plan().await;
    let mut asked = chat("openai/gpt-4o", "Hi");
    asked["temperature"] = json!(0.2);
    asked["top_p"] = json!(0.9);
    asked["max_completion_tokens"] = json!(64);
    asked["user"] = json!("end-user-7");
    let r = p.call("/v1/chat/completions", false, asked).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let answer = r.json();
    assert_eq!(answer["object"], "chat.completion");
    assert_eq!(answer["choices"][0]["message"]["content"], HELLO);
    assert_eq!(r.header("kaeso-provider"), Some("openai"));
    assert!(
        p.api.received().is_empty(),
        "the configured base URL is never used"
    );
    let sent = p.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].method.as_str(), sent[0].path.as_str()),
        ("POST", "/v1/responses")
    );
    assert_eq!(
        sent[0].header("authorization"),
        Some(format!("Bearer {}", p.access_token().await).as_str())
    );
    let body = sent[0].json();
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["model"], "gpt-4o");
    assert!(body["input"].is_array());
    for field in [
        "temperature",
        "top_p",
        "max_output_tokens",
        "user",
        "safety_identifier",
    ] {
        assert!(body.get(field).is_none(), "{field} in {body}");
    }

    let r = p
        .call(
            "/v1/responses",
            false,
            json!({
                "model": "openai/gpt-4o",
                "instructions": "Be brief.",
                "input": "Hi",
                "store": false,
                "temperature": 0.5,
                "top_p": 0.5,
                "max_output_tokens": 64,
                "max_tool_calls": 2,
                "metadata": { "ticket": "42" },
                "prompt_cache_retention": "24h",
                "safety_identifier": "end-user-7",
                "top_logprobs": 2,
                "truncation": "auto",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["output"][0]["content"][0]["text"], HELLO);
    let body = p.sent().pop().unwrap().json();
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    assert_eq!(body["instructions"], "Be brief.");
    assert!(body["input"].is_array(), "{body}");
    for field in [
        "temperature",
        "top_p",
        "max_output_tokens",
        "max_tool_calls",
        "metadata",
        "prompt_cache_retention",
        "safety_identifier",
        "top_logprobs",
        "truncation",
        "user",
    ] {
        assert!(body.get(field).is_none(), "{field} in {body}");
    }

    let r = p
        .call("/v1/messages", true, messages("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][0]["text"], HELLO);

    let mut streamed = chat("openai/gpt-5", "Hi");
    streamed["stream"] = json!(true);
    let r = p.call("/v1/chat/completions", false, streamed).await;
    assert_eq!(r.status, StatusCode::OK);
    let text: String = sse(&r.text())
        .iter()
        .filter_map(|e| e["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(text, HELLO);
    assert_eq!(p.sent().len(), 4, "no request is retried");
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_tool_call_from_the_plan_stops_for_tool_use_in_every_format() {
    let p = plan().await;
    p.oauth.reply_with(Reply::ToolCall);
    let r = p
        .call(
            "/v1/chat/completions",
            false,
            chat("openai/gpt-5", "Weather in Paris?"),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let choice = &r.json()["choices"][0];
    assert_eq!(choice["finish_reason"], "tool_calls", "{choice}");
    assert_eq!(
        choice["message"]["tool_calls"][0]["function"],
        json!({ "name": "get_weather", "arguments": "{\"city\":\"Paris\"}" })
    );

    let mut streamed = chat("openai/gpt-5", "Weather in Paris?");
    streamed["stream"] = json!(true);
    let r = p.call("/v1/chat/completions", false, streamed).await;
    assert_eq!(r.status, StatusCode::OK);
    let events = sse(&r.text());
    let finished: Vec<&str> = events
        .iter()
        .filter_map(|e| e["choices"][0]["finish_reason"].as_str())
        .collect();
    assert_eq!(finished, ["tool_calls"], "{events:?}");

    let r = p
        .call(
            "/v1/messages",
            true,
            messages("openai/gpt-5", "Weather in Paris?"),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["stop_reason"], "tool_use", "{}", r.text());
    assert_eq!(r.json()["content"][0]["type"], "tool_use");
    assert_eq!(r.json()["content"][0]["input"], json!({ "city": "Paris" }));

    let mut streamed = messages("openai/gpt-5", "Weather in Paris?");
    streamed["stream"] = json!(true);
    let r = p.call("/v1/messages", true, streamed).await;
    assert_eq!(r.status, StatusCode::OK);
    let events = sse(&r.text());
    let stopped: Vec<&str> = events
        .iter()
        .filter_map(|e| e["delta"]["stop_reason"].as_str())
        .collect();
    assert_eq!(stopped, ["tool_use"], "{events:?}");
    p.services.cleanup().await;
}

#[tokio::test]
async fn explicit_defaults_and_later_system_items_never_reach_the_plan_route() {
    let p = plan().await;
    let r = p
        .call(
            "/v1/responses",
            false,
            json!({
                "model": "openai/gpt-4o",
                "input": [
                    { "role": "user", "content": "Hi" },
                    { "role": "system", "content": "be brief" },
                    { "role": "user", "content": "Go" },
                ],
                "background": false,
                "metadata": {},
                "truncation": "disabled",
                "temperature": null,
                "max_output_tokens": null,
                "user": null,
                "previous_response_id": null,
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = p.sent().pop().unwrap().json();
    for field in [
        "background",
        "metadata",
        "truncation",
        "temperature",
        "max_output_tokens",
        "user",
        "previous_response_id",
    ] {
        assert!(body.get(field).is_none(), "{field} in {body}");
    }
    let roles: Vec<&str> = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["role"].as_str())
        .collect();
    assert_eq!(roles, ["user", "developer", "user"], "{body}");
    assert_eq!(
        (body["store"].clone(), body["stream"].clone()),
        (json!(false), json!(true))
    );
    p.services.cleanup().await;
}

#[tokio::test]
async fn embeddings_and_token_counts_need_an_api_key() {
    let p = plan().await;
    let r = p
        .call(
            "/v1/embeddings",
            false,
            json!({ "model": "openai/text-embedding-3-small", "input": "hi" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "A ChatGPT account can't create embeddings. Use an API key for OpenAI in this project instead."
    );
    assert_eq!(r.json()["error"]["param"], "model");
    let r = p
        .call(
            "/v1/responses/input_tokens",
            false,
            json!({ "model": "openai/gpt-5", "input": "hi" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "A ChatGPT account can't count tokens. Use an API key for OpenAI in this project instead."
    );
    let r = p
        .call(
            "/v1/messages/count_tokens",
            true,
            json!({ "model": "openai/gpt-5", "messages": [{ "role": "user", "content": "hi" }] }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(p.sent().is_empty());
    assert!(p.api.received().is_empty());
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_plan_request_holds_the_models_whole_output_limit_whatever_cap_it_asked_for() {
    let p = plan_with(&[("KAESO_UPSTREAM_REQUEST_TIMEOUT", "2s")]).await;
    p.app
        .post(
            &format!("/orgs/{}/projects/{}/limits", p.owner.org_id, p.project),
            json!({ "level": "project", "kind": "token_budget", "period": "day", "amount": 1000 }),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    p.oauth.reply_with(Reply::Trickle);
    let capped = json!({ "model": "openai/gpt-5", "input": "Hi", "max_output_tokens": 16 });
    let first = p.call("/v1/responses", false, capped.clone());
    let second = async {
        common::eventually("the first request reaches the plan", || async {
            !p.sent().is_empty()
        })
        .await;
        p.call("/v1/responses", false, capped.clone()).await
    };
    let (first, second) = tokio::join!(first, second);
    assert_eq!(
        first.status,
        StatusCode::GATEWAY_TIMEOUT,
        "{}",
        first.text()
    );
    assert_eq!(
        second.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        second.text()
    );
    assert_eq!(
        second.json()["error"]["message"],
        "The project's daily token budget of 1000 tokens is nearly used up by requests in progress; retry in a moment."
    );
    assert_eq!(p.sent().len(), 1);
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_reply_that_keeps_trickling_ends_at_the_request_timeout() {
    let p = plan_with(&[("KAESO_UPSTREAM_REQUEST_TIMEOUT", "1s")]).await;
    p.oauth.reply_with(Reply::Trickle);
    let started = std::time::Instant::now();
    let r = p
        .call(
            "/v1/responses",
            false,
            json!({ "model": "openai/gpt-5", "input": "Hi" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider did not answer in time (request). Kaeso does not retry; try again or raise the model's timeout."
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_reached_usage_limit_answers_429_with_the_usage_page() {
    let p = plan().await;
    p.oauth.reply_with(Reply::UsageLimit);
    let r = p
        .call("/v1/chat/completions", false, chat("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], USAGE);
    let r = p
        .call("/v1/messages", true, messages("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], USAGE);
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");

    p.oauth.reply_with(Reply::UsageLimitInStream);
    let r = p
        .call(
            "/v1/responses",
            false,
            json!({ "model": "openai/gpt-5", "input": "Hi" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], USAGE);
    let mut streamed = chat("openai/gpt-5", "Hi");
    streamed["stream"] = json!(true);
    let r = p.call("/v1/chat/completions", false, streamed).await;
    let events = sse(&r.text());
    assert!(
        events.iter().any(|e| e["error"]["message"] == USAGE),
        "{events:?}"
    );
    assert_eq!(p.sent().len(), 4, "no request is retried");
    let logged = p.capture.all();
    assert!(
        logged
            .iter()
            .all(|o| o.error_message.as_deref() == Some(USAGE)),
        "{logged:?}"
    );
    p.services.cleanup().await;
}

#[tokio::test]
async fn other_errors_reach_the_app_as_openai_sent_them() {
    let p = plan().await;
    p.oauth.reply_with(Reply::ModelNotFound);
    let r = p
        .call("/v1/chat/completions", false, chat("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The requested model does not exist."
    );
    p.oauth.reply_with(Reply::Text);
    p.oauth.end_sessions();
    let r = p
        .call("/v1/chat/completions", false, chat("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    p.services.cleanup().await;
}

#[tokio::test]
async fn only_models_the_account_lists_are_served() {
    let p = plan().await;
    p.oauth.reply_with(Reply::ModelNotFound);
    for (model, listed) in [("gpt-5-mini", false), ("gpt-4o", false), ("gpt-5", true)] {
        let r = p
            .call(
                "/v1/chat/completions",
                false,
                chat(&format!("openai/{model}"), "Hi"),
            )
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert_eq!(
            message.starts_with("OpenAI does not serve"),
            !listed,
            "{model}: {message}"
        );
    }
    let listings = p
        .sent()
        .into_iter()
        .filter(|r| r.method == "GET")
        .map(|r| r.path)
        .collect::<Vec<_>>();
    assert!(
        listings.iter().all(|path| path == "/v1/models"),
        "{listings:?}"
    );

    let check = p
        .app
        .post(
            &format!("{}/models/check", p.base()),
            json!({}),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let availability = |id: &str| {
        check.body["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == id)
            .unwrap()["availability"]
            .clone()
    };
    assert_eq!(availability("gpt-5"), "available");
    assert_eq!(
        availability("gpt-5-mini"),
        "unavailable",
        "hidden models aren't offered"
    );
    assert_eq!(availability("gpt-4o"), "unavailable");
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_signed_out_account_asks_for_a_new_sign_in_everywhere() {
    let p = plan().await;
    p.app.clock.advance(time::Duration::minutes(56));
    p.oauth.set_fault(Fault::SignedOut);
    let r = p
        .call("/v1/chat/completions", false, chat("openai/gpt-5", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], SIGNED_OUT);
    let view = p
        .app
        .get(
            &format!("/orgs/{}/credentials/{}", p.owner.org_id, p.credential),
            &p.owner.cookie,
        )
        .await;
    assert_eq!(view.body["auth_error"], "signed_out");
    assert!(p.sent().is_empty(), "nothing reached OpenAI");

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/platform/v1{}/playground/responses", p.base()))
        .header(
            header::COOKIE,
            format!("__Host-kaeso_session={}", p.owner.cookie),
        )
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "model": "openai/gpt-5", "input": "Hi", "stream": true }))
                .unwrap(),
        ))
        .unwrap();
    let r = p.app.raw(req).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "The ChatGPT account of this project's provider key is signed out; sign in again."
    );
    let check = p
        .app
        .post(
            &format!("{}/models/check", p.base()),
            json!({}),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        check.body["failed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["reason"].clone())
            .collect::<Vec<_>>(),
        vec![json!("key_refused"); 4]
    );
    assert_eq!(p.oauth.refreshes(), 1);
    p.services.cleanup().await;
}
