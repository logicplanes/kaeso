#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::v1::{Answer, Capture, Env, call, messages};
use common::{App, ORIGIN, Owner};
use http_body_util::BodyExt;
use kaeso_server::model_api::hooks::{Outcome, Source};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use common::uuid;

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

fn responses_path(env: &Env) -> String {
    format!("{}/playground/responses", base(env))
}

fn ask(model: &str, input: &str) -> Value {
    json!({ "model": model, "input": input, "stream": true })
}

async fn play(app: &App, path: &str, cookie: &str, body: &Value) -> Answer {
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/platform/v1{path}"))
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, "Kaeso console test")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap();
    let (status, headers, body) = app.raw_bytes(req).await;
    Answer {
        status,
        headers,
        body,
    }
}

fn events(r: &Answer) -> Vec<Value> {
    r.text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .collect()
}

fn types(evs: &[Value]) -> Vec<String> {
    evs.iter()
        .map(|e| e["type"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn text_of(evs: &[Value], kind: &str) -> String {
    evs.iter()
        .filter(|e| e["type"] == kind)
        .map(|e| e["delta"].as_str().unwrap())
        .collect()
}

fn completed(r: &Answer) -> Vec<Value> {
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.header("content-type"),
        Some("text/event-stream; charset=utf-8")
    );
    let evs = events(r);
    let t = types(&evs);
    assert_eq!(t.first().map(String::as_str), Some("response.created"));
    assert!(t.iter().any(|x| x == "response.output_text.delta"), "{t:?}");
    assert_eq!(t.last().map(String::as_str), Some("response.completed"));
    evs
}

async fn outcome(env: &Env, id: &str) -> Outcome {
    for _ in 0..300 {
        if let Some(o) = env
            .capture
            .all()
            .into_iter()
            .find(|o| o.request_id.to_string() == id)
        {
            return o;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no outcome for {id} in {:#?}", env.capture.all());
}

async fn captured() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

async fn join(app: &App, owner: &Owner, email: &str, preset: &str) -> (String, String) {
    common::join(
        app,
        owner,
        json!({ "email": email, "preset": preset }),
        "Member",
    )
    .await
}

#[tokio::test]
async fn a_member_streams_responses_events_from_either_provider() {
    let env = captured().await;
    let path = responses_path(&env);
    let r = play(
        &env.app,
        &path,
        &env.owner.cookie,
        &ask("claude-haiku-4-5", "Hello"),
    )
    .await;
    let evs = completed(&r);
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
    assert_eq!(r.header("kaeso-provider"), Some("anthropic"));
    let done = evs.last().unwrap();
    assert_eq!(done["response"]["usage"]["input_tokens"], 10);
    assert_eq!(done["response"]["usage"]["output_tokens"], 18);
    assert!(!text_of(&evs, "response.output_text.delta").is_empty());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(common::v1::ANTHROPIC_SECRET));
    assert!(sent.header("cookie").is_none());
    assert!(sent.header("origin").is_none());
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, &id).await;
    let caller = o.caller.clone().unwrap();
    assert_eq!(caller.key_id, None);
    assert_eq!(caller.user_id, Some(uuid(&env.owner.user_id)));
    assert_eq!(caller.project_id, uuid(&env.project));
    assert_eq!(o.source, Source::Playground);
    assert_eq!(o.status, 200);
    assert!(o.stream);
    assert_eq!(o.user_agent.as_deref(), Some("Kaeso console test"));
    assert_eq!(o.usage.unwrap().output_tokens, 18);

    let turn = json!({
        "model": "gpt-4o",
        "instructions": "Answer briefly.",
        "temperature": 0.2,
        "top_p": 0.9,
        "max_output_tokens": 300,
        "stream": true,
        "input": [
            { "role": "user", "content": "Hello" },
            { "role": "assistant", "content": "Hi! How can I help?" },
            { "role": "user", "content": "Tell me more." },
        ],
    });
    let r = play(&env.app, &path, &env.owner.cookie, &turn).await;
    completed(&r);
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/responses");
    let body = sent.json();
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["instructions"], "Answer briefly.");
    assert_eq!(body["max_output_tokens"], 300);
    assert_eq!(body["input"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn a_model_whose_id_names_another_providers_model_is_offered_with_its_provider() {
    let env = captured().await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "togetherai", "name": "togetherai", "secret": "tgp_v1_test-playground-0001" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for body in [
        json!({ "provider": "togetherai", "model_id": "openai/gpt-oss-120b" }),
        json!({
            "provider": "openai", "model_id": "gpt-oss-120b", "custom": true,
            "overrides": {
                "input_modalities": ["text"], "output_modalities": ["text"],
                "context_limit": 128000, "output_limit": 8000,
                "input_price": "1", "output_price": "2", "egress_dialect": "openai_responses"
            }
        }),
    ] {
        let r = env
            .app
            .post(
                &format!("{}/models", base(&env)),
                body,
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    let listed = env
        .app
        .get(
            &format!("{}/playground/models", base(&env)),
            &env.owner.cookie,
        )
        .await;
    let name = |provider: &str| {
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| {
                m["provider"] == provider
                    && m["model_id"].as_str().unwrap().ends_with("gpt-oss-120b")
            })
            .unwrap()["model"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(name("togetherai"), "togetherai/openai/gpt-oss-120b");
    assert_eq!(name("openai"), "gpt-oss-120b");

    let r = play(
        &env.app,
        &responses_path(&env),
        &env.owner.cookie,
        &ask(&name("togetherai"), "Hi"),
    )
    .await;
    completed(&r);
    assert_eq!(r.header("kaeso-provider"), Some("togetherai"));
    assert_eq!(
        env.mock.last().unwrap().json()["model"],
        "openai/gpt-oss-120b"
    );

    let r = call(&env, "/v1/models", Some(&env.key), false, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let ids: Vec<String> = r.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        ids.iter().any(|i| i == "togetherai/openai/gpt-oss-120b"),
        "{ids:?}"
    );
    assert!(ids.iter().any(|i| i == "gpt-oss-120b"), "{ids:?}");
    assert!(!ids.iter().any(|i| i == "openai/gpt-oss-120b"), "{ids:?}");
    let r = call(
        &env,
        "/v1/models/togetherai%2Fopenai%2Fgpt-oss-120b",
        Some(&env.key),
        false,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["owned_by"], "togetherai");
}

#[tokio::test]
async fn reasoning_summaries_stream_from_a_model_that_reasons() {
    let env = captured().await;
    let r = env
        .app
        .post(
            &format!("{}/models", base(&env)),
            json!({ "provider": "anthropic", "model_id": "claude-opus-4-8" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let body = json!({
        "model": "claude-opus-4-8",
        "stream": true,
        "reasoning": { "effort": "medium", "summary": "auto" },
        "input": "Think first. [mock:fixture=thinking-enabled-budget]",
    });
    let r = play(&env.app, &responses_path(&env), &env.owner.cookie, &body).await;
    let evs = completed(&r);
    assert_eq!(
        env.mock.last().unwrap().json()["thinking"]["display"],
        "summarized"
    );
    assert!(
        !text_of(&evs, "response.reasoning_summary_text.delta").is_empty(),
        "{:?}",
        types(&evs)
    );
    let t = types(&evs);
    let at = |x: &str| t.iter().position(|y| y == x).unwrap();
    assert!(at("response.reasoning_summary_text.delta") < at("response.output_text.delta"));

    let list = env
        .app
        .get(
            &format!("{}/playground/models", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.body);
    let data = list.body["data"].as_array().unwrap();
    let opus = data
        .iter()
        .find(|m| m["model_id"] == "claude-opus-4-8")
        .unwrap();
    assert_eq!(opus["reasoning"], true);
    assert_eq!(opus["provider_key"], true);
    assert!(opus["input_price"].is_string(), "{opus}");
    assert!(opus["output_price"].is_string(), "{opus}");
    assert!(!opus["display_name"].as_str().unwrap().is_empty());
    assert!(
        data.iter()
            .all(|m| m["model_id"] != "text-embedding-3-small")
    );
    assert!(data.iter().any(|m| m["model_id"] == "gpt-4o"));
}

#[tokio::test]
async fn reasoning_of_chat_providers_streams_as_summaries_and_is_kept_as_reasoning() {
    let env = captured().await;
    let cases = [
        (
            "deepseek",
            "sk-test-deepseek-credential-0004",
            "deepseek-flash",
            "providers/deepseek/recorded-thinking-stream",
            "391",
        ),
        (
            "groq",
            "gsk_test-groq-credential-0003",
            "openai/gpt-oss-20b",
            "providers/groq/recorded-stream-doc",
            "Hello! 👋 How can I help you today?",
        ),
    ];
    for (provider, secret, model, fixture, answer) in cases {
        let org = &env.owner.org_id;
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/credentials"),
                json!({ "provider": provider, "name": provider, "secret": secret }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        let r = env
            .app
            .post(
                &format!("{}/models", base(&env)),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        let body = json!({
            "model": model,
            "stream": true,
            "reasoning": { "summary": "auto" },
            "input": format!("Think first. [mock:fixture={fixture}]"),
        });
        let r = play(&env.app, &responses_path(&env), &env.owner.cookie, &body).await;
        let evs = completed(&r);
        let thought = text_of(&evs, "response.reasoning_summary_text.delta");
        assert!(!thought.is_empty(), "{provider}: {:?}", types(&evs));
        assert_eq!(text_of(&evs, "response.output_text.delta"), answer);
        let t = types(&evs);
        let at = |x: &str| t.iter().position(|y| y == x).unwrap();
        assert!(
            at("response.reasoning_summary_text.delta") < at("response.output_text.delta"),
            "{provider}: {t:?}"
        );

        let id = r.header("kaeso-request-id").unwrap().to_owned();
        let reply = outcome(&env, &id).await.reply.unwrap();
        let kinds: Vec<&str> = reply
            .blocks
            .iter()
            .map(|b| match b {
                kaeso_kpp::Block::Reasoning(_) => "reasoning",
                kaeso_kpp::Block::Text(_) => "text",
                _ => "other",
            })
            .collect();
        assert_eq!(
            kinds,
            ["reasoning", "text"],
            "{provider}: {:?}",
            reply.blocks
        );
        let kaeso_kpp::Block::Reasoning(kept) = &reply.blocks[0] else {
            unreachable!()
        };
        assert_eq!(kept.text.concat(), thought, "{provider}");
    }
}

#[tokio::test]
async fn models_and_aliases_resolve_in_this_project_only() {
    let env = captured().await;
    let path = responses_path(&env);
    let haiku = env
        .app
        .get(&format!("{}/models", base(&env)), &env.owner.cookie)
        .await
        .body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "claude-haiku-4-5")
        .unwrap()["id"]
        .clone();
    let r = env
        .app
        .post(
            &format!("{}/aliases", base(&env)),
            json!({ "name": "fast", "catalogue_model_id": haiku }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = play(&env.app, &path, &env.owner.cookie, &ask("fast", "Hi")).await;
    completed(&r);
    assert_eq!(env.mock.last().unwrap().json()["model"], "claude-haiku-4-5");
    let o = outcome(&env, r.header("kaeso-request-id").unwrap()).await;
    assert_eq!(o.requested_model.as_deref(), Some("fast"));
    assert_eq!(o.model_id.as_deref(), Some("claude-haiku-4-5"));
    let models = env
        .app
        .get(
            &format!("{}/playground/models", base(&env)),
            &env.owner.cookie,
        )
        .await;
    let entry = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "claude-haiku-4-5")
        .unwrap()
        .clone();
    assert_eq!(entry["aliases"], json!(["fast"]));
    assert_eq!(entry["model"], "claude-haiku-4-5");

    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Other" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.body);
    let other_id = other.body["id"].as_str().unwrap();
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{other_id}/models", env.owner.org_id),
            json!({ "provider": "openai", "model_id": "gpt-4.1" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(&path, ask("gpt-4.1", "Hi"), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "model");
    assert_eq!(
        r.body["error"]["message"],
        "The model `gpt-4.1` is not in this project's catalogue."
    );
    let o = env.outcomes().await;
    let refused = o.last().unwrap();
    assert_eq!((refused.status, refused.source), (400, Source::Playground));
    assert_eq!(refused.error_kind.as_deref(), Some("not_found"));

    let r = env
        .app
        .post(
            &path,
            json!({ "model": "gpt-4o", "input": "Hi", "stream": false }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "stream");
    let r = env
        .app
        .post(
            &path,
            json!({ "model": "gpt-4o", "input": 42, "stream": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.error_type(), "invalid_request");
    let r = env
        .app
        .post(
            &path,
            ask("claude-haiku-4-5", "Hi [mock:error=overloaded]"),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    assert_eq!(r.error_type(), "provider_error");
    assert_eq!(
        r.body["error"]["message"],
        "The provider is overloaded right now; try again shortly."
    );
}

#[tokio::test]
async fn rights_sessions_origins_and_archived_projects_are_checked() {
    let env = captured().await;
    let path = responses_path(&env);
    let (viewer, _) = join(&env.app, &env.owner, "viewer@example.com", "member").await;
    let r = env
        .app
        .post(&path, ask("gpt-4o", "Hi"), Some(&viewer))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("`playground.use`"),
        "{:?}",
        r.body
    );
    let r = env
        .app
        .get(&format!("{}/playground/models", base(&env)), &viewer)
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);

    let (dev, dev_id) = join(&env.app, &env.owner, "dev@example.com", "developer").await;
    let r = play(&env.app, &path, &dev, &ask("gpt-4o", "Hi")).await;
    completed(&r);
    let o = outcome(&env, r.header("kaeso-request-id").unwrap()).await;
    assert_eq!(o.caller.unwrap().user_id, Some(uuid(&dev_id)));
    let catalogue = env
        .app
        .get(&format!("{}/models", base(&env)), &env.owner.cookie)
        .await;
    let gpt = catalogue.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "gpt-4o")
        .unwrap()["id"]
        .clone();
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/members/{dev_id}/models", base(&env)),
            Some(json!({ "all": false, "models": [gpt] })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = env
        .app
        .post(&path, ask("claude-haiku-4-5", "Hi"), Some(&dev))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "The model `claude-haiku-4-5` is not among the models you may use in this project."
    );
    let listed = env
        .app
        .get(&format!("{}/playground/models", base(&env)), &dev)
        .await;
    let ids: Vec<&str> = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["model_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["gpt-4o"]);

    for origin in [None, Some("https://evil.example")] {
        let r = env
            .app
            .call_with(
                Method::POST,
                &path,
                Some(ask("gpt-4o", "Hi")),
                Some(&env.owner.cookie),
                origin,
            )
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{origin:?}: {:?}", r.body);
        assert!(
            r.body["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("Open Kaeso at "),
            "{:?}",
            r.body
        );
    }

    let key = env
        .app
        .post(
            &format!("{}/keys", base(&env)),
            json!({ "name": "script", "access": "both" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let r = env
        .app
        .with_key(
            Method::POST,
            &path,
            Some(ask("gpt-4o", "Hi")),
            key.body["secret"].as_str().unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("console session")
    );

    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Old" }),
            Some(&env.owner.cookie),
        )
        .await;
    let other_id = other.body["id"].as_str().unwrap();
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{other_id}/archive", env.owner.org_id),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{other_id}/playground/responses",
                env.owner.org_id
            ),
            ask("gpt-4o", "Hi"),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
}

#[tokio::test]
async fn an_org_scheduled_for_deletion_refuses_playground_requests() {
    let env = captured().await;
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("/orgs/{}", env.owner.org_id),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = env
        .app
        .post(
            &responses_path(&env),
            ask("gpt-4o", "Hi"),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::CONFLICT, "org_scheduled_for_deletion"),
        "{:?}",
        r.body
    );
    assert!(env.mock.received().is_empty(), "nothing reached a provider");
    let r = env
        .app
        .get(
            &format!("{}/playground/models", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
}

#[tokio::test]
async fn closing_the_stream_cancels_the_provider_request() {
    let env = captured().await;
    let body = ask("claude-haiku-4-5", "Tell me a story [mock:slow]");
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/platform/v1{}", responses_path(&env)))
        .header(
            header::COOKIE,
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let id = resp
        .headers()
        .get("kaeso-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let mut stream = resp.into_body();
    let mut seen = String::new();
    while !seen.contains("response.output_text.delta") {
        let frame = stream.frame().await.unwrap().unwrap();
        if let Ok(data) = frame.into_data() {
            seen.push_str(&String::from_utf8_lossy(&data));
        }
    }
    drop(stream);
    let o = outcome(&env, &id).await;
    assert_eq!((o.status, o.cancelled), (499, true));
    assert_eq!(o.error_kind.as_deref(), Some("client_disconnected"));
    assert_eq!(o.source, Source::Playground);
    assert!(o.usage_estimated);
    for _ in 0..200 {
        if env.mock.stream_counts().2 >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        env.mock.stream_counts().2 >= 1,
        "the provider stream was dropped: {:?}",
        env.mock.stream_counts()
    );
}

async fn limit(env: &Env, body: Value) -> Value {
    let r = env
        .app
        .post(
            &format!("{}/limits", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

#[tokio::test]
async fn member_and_project_limits_and_budgets_apply() {
    let env = common::v1::env(None, None).await;
    let path = responses_path(&env);
    let keys = env
        .app
        .get(&format!("{}/keys", base(&env)), &env.owner.cookie)
        .await;
    let key_id = keys.body["data"][0]["id"].as_str().unwrap().to_owned();
    limit(&env, json!({ "level": "key", "key": key_id, "kind": "request_rate", "period": "minute", "amount": 1 })).await;
    limit(&env, json!({ "level": "member", "user": env.owner.user_id, "kind": "request_rate", "period": "minute", "amount": 2 })).await;
    for _ in 0..2 {
        completed(
            &play(
                &env.app,
                &path,
                &env.owner.cookie,
                &ask("claude-haiku-4-5", "Hi"),
            )
            .await,
        );
    }
    let r = env
        .app
        .post(
            &path,
            ask("claude-haiku-4-5", "Hi"),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", r.body);
    assert_eq!(r.error_type(), "rate_limited");
    let message = r.body["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with(
            "Rate limit reached: the member's request rate of 2 requests per minute; retry in"
        ),
        "{message}"
    );
    let retry: u64 = r.headers["retry-after"].to_str().unwrap().parse().unwrap();
    assert!((1..=60).contains(&retry), "{retry}");

    env.app.clock.advance(time::Duration::seconds(61));
    limit(
        &env,
        json!({ "level": "project", "kind": "token_budget", "period": "day", "amount": 100 }),
    )
    .await;
    let mut capped = ask("claude-haiku-4-5", "Hi");
    capped["max_output_tokens"] = json!(18);
    completed(&play(&env.app, &path, &env.owner.cookie, &capped).await);
    let r = env
        .app
        .post(
            &path,
            ask("claude-haiku-4-5", "Hi"),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This request could exceed the project's daily token budget of 100 tokens; ask for fewer output tokens."
    );
    assert!(r.headers.get("retry-after").is_none());
    let v1 = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(v1.status, StatusCode::TOO_MANY_REQUESTS, "{}", v1.text());
}

#[tokio::test]
async fn a_spend_budget_refuses_a_provider_billed_tool_in_the_playground() {
    let env = common::v1::env(None, None).await;
    let path = responses_path(&env);
    limit(&env, json!({ "level": "member", "user": env.owner.user_id, "kind": "spend_budget", "period": "month", "amount": 5 })).await;
    let mut body = ask("gpt-4o", "Draw a cat.");
    body["tools"] = json!([{ "type": "image_generation" }]);
    env.mock.clear();
    let r = env.app.post(&path, body, Some(&env.owner.cookie)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(r.error_type(), "permission_denied");
    assert_eq!(
        r.body["error"]["message"],
        "The member has a spend budget that can't price `image_generation`; ask an admin."
    );
    assert!(env.mock.received().is_empty());
    completed(&play(&env.app, &path, &env.owner.cookie, &ask("gpt-4o", "Hi")).await);
}

#[tokio::test]
async fn playground_requests_are_charged_and_logged_as_the_members() {
    let env = common::v1::env(None, None).await;
    env.app
        .state
        .clickhouse
        .migrate(&env.app.state.pg)
        .await
        .unwrap();
    let r = play(
        &env.app,
        &responses_path(&env),
        &env.owner.cookie,
        &ask("claude-haiku-4-5", "Hello from the Playground"),
    )
    .await;
    completed(&r);
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    env.app.clock.advance(time::Duration::seconds(1));
    let v1 = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hello from an app")),
    )
    .await;
    assert_eq!(v1.status, StatusCode::OK);
    env.app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();

    let logs = |q: &'static str| {
        let app = &env.app;
        let cookie = env.owner.cookie.clone();
        let path = format!("{}/logs{q}", base(&env));
        async move {
            let r = app.get(&path, &cookie).await;
            assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
            r.body["data"].as_array().unwrap().clone()
        }
    };
    let played = logs("?source=playground").await;
    assert_eq!(played.len(), 1, "{played:?}");
    let entry = &played[0];
    assert_eq!(entry["id"], id);
    assert_eq!(entry["source"], "playground");
    assert_eq!(entry["key_id"], Value::Null);
    assert_eq!(entry["principal_kind"], "user");
    assert_eq!(entry["principal_id"], env.owner.user_id);
    assert_eq!(entry["endpoint"], "responses");
    assert_eq!(entry["ingress_dialect"], "openai_responses");
    assert_eq!(entry["status"], 200);
    assert_eq!(entry["stream"], true);
    assert_eq!(entry["usage"]["output_tokens"], 18);
    assert_ne!(entry["cost"], "0");
    assert_eq!(entry["has_content"], true);
    let api = logs("?source=api").await;
    assert_eq!(api.len(), 1);
    assert_eq!(api[0]["source"], "api");
    assert!(api[0]["key_id"].as_str().unwrap().starts_with("key_"));
    assert_eq!(logs("").await.len(), 2);
    let r = env
        .app
        .get(
            &format!("{}/logs?source=cli", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "source");
    let one = env
        .app
        .get(&format!("{}/logs/{id}", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(
        one.body["content"]["request"]["input"],
        "Hello from the Playground"
    );
    let req = Request::builder()
        .uri(format!(
            "/platform/v1{}/logs/export?format=csv&source=playground",
            base(&env)
        ))
        .header(
            header::COOKIE,
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let csv = env.app.router.clone().oneshot(req).await.unwrap();
    let csv =
        String::from_utf8(csv.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let mut lines = csv.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .ends_with(",usage_estimated,source,overhead_ms,stream_processing_ms")
    );
    let row = lines.next().unwrap();
    let (_, timing) = row.split_once(",\"playground\",").unwrap();
    assert!(
        timing.split(',').all(|ms| ms.parse::<u32>().is_ok()),
        "the overhead and the stream's processing time: {row}"
    );
    assert!(lines.next().is_none());

    let rows: Vec<(Option<Uuid>, Option<Uuid>, i64, i64)> = sqlx::query_as(
        "SELECT api_key_id, user_id, requests, output_tokens FROM usage_daily \
         WHERE project_id = $1 ORDER BY api_key_id NULLS FIRST",
    )
    .bind(uuid(&env.project))
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0], (None, Some(uuid(&env.owner.user_id)), 1, 18));
    assert!(rows[1].0.is_some());
    let usage = env
        .app
        .get(
            &format!("{}/usage?group_by=key", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(usage.status, StatusCode::OK, "{:?}", usage.body);
    let keys: Vec<&str> = usage.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&"playground"), "{keys:?}");
    assert!(keys.iter().any(|k| k.starts_with("key_")), "{keys:?}");
}
