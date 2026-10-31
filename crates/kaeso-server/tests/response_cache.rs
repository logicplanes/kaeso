#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::{Answer, Env, call, chat, messages, open_stream};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn env(extra: &[(&str, &str)]) -> Env {
    let e = common::v1::env(None, Some(extra)).await;
    e.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&e.app.state.pg)
        .await
        .unwrap();
    e
}

fn project_path(env: &Env, project: &str) -> String {
    format!(
        "/orgs/{}/projects/{project}/response-cache",
        env.owner.org_id
    )
}

async fn set_project(env: &Env, project: &str, body: Value) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &project_path(env, project),
            Some(body),
            Some(&env.owner.cookie),
        )
        .await
}

async fn turn_on(env: &Env) {
    let r = set_project(env, &env.project, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["effective"], true);
}

async fn ask(env: &Env, key: &str, body: Value, cache_control: Option<&str>) -> Answer {
    let mut req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json");
    if let Some(c) = cache_control {
        req = req.header("cache-control", c);
    }
    let req = req
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
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
    env.app.state.in_flight.settled().await;
    Answer {
        status,
        headers,
        body,
    }
}

fn hello() -> Value {
    messages("claude-haiku-4-5", "Hello")
}

fn text(a: &Answer) -> String {
    a.json()["content"][0]["text"].as_str().unwrap().to_owned()
}

async fn logged(env: &Env, query: &str) -> Vec<Value> {
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/logs{query}",
                env.owner.org_id, env.project
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].as_array().unwrap().clone()
}

#[tokio::test]
async fn the_cache_is_off_until_a_project_turns_it_on() {
    let env = env(&[]).await;
    let r = env
        .app
        .get(&project_path(&env, &env.project), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.body,
        json!({ "enabled": false, "ttl_seconds": 3600, "org_allowed": true, "available": true, "effective": false })
    );
    for _ in 0..2 {
        let a = ask(&env, &env.key, hello(), None).await;
        assert_eq!(a.status, StatusCode::OK);
        assert_eq!(a.header("kaeso-cache"), None);
    }
    assert_eq!(env.mock.received().len(), 2);
}

#[tokio::test]
async fn a_repeated_request_is_answered_from_the_cache_at_no_provider_cost() {
    let env = env(&[]).await;
    turn_on(&env).await;

    let first = ask(&env, &env.key, hello(), None).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.header("kaeso-cache"), Some("miss"));
    assert_eq!(first.header("age"), None);

    env.app.clock.advance(time::Duration::seconds(7));
    let second = ask(&env, &env.key, hello(), None).await;
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(second.header("kaeso-cache"), Some("hit"));
    assert_eq!(second.header("age"), Some("7"));
    assert_eq!(second.header("kaeso-model"), Some("claude-haiku-4-5"));
    assert_eq!(second.header("kaeso-provider"), Some("anthropic"));
    assert_eq!(text(&second), text(&first));
    assert_eq!(second.json()["usage"], first.json()["usage"]);
    assert_eq!(env.mock.received().len(), 1, "the provider was called once");

    let logs = logged(&env, "").await;
    assert_eq!(logs.len(), 2);
    let (hit, miss) = (&logs[0], &logs[1]);
    assert_eq!(hit["response_cache"], "hit");
    assert_eq!(miss["response_cache"], "miss");
    assert_eq!(hit["cost"], "0");
    assert_ne!(miss["cost"], "0");
    assert_eq!(hit["usage"]["input_tokens"], miss["usage"]["input_tokens"]);
    assert_eq!(
        hit["usage"]["output_tokens"],
        miss["usage"]["output_tokens"]
    );
    assert!(hit["ttfb_ms"].is_null(), "no provider answered: {hit}");
    assert_eq!(hit["upstream_request_id"], "");

    let only_hits = logged(&env, "?response_cache=hit").await;
    assert_eq!(only_hits.len(), 1);
    assert_eq!(only_hits[0]["id"], hit["id"]);
}

#[tokio::test]
async fn any_change_to_the_request_misses() {
    let env = env(&[]).await;
    turn_on(&env).await;
    ask(&env, &env.key, hello(), None).await;
    let mut longer = hello();
    longer["max_tokens"] = json!(512);
    for body in [
        messages("claude-haiku-4-5", "Hello!"),
        longer,
        messages("gpt-4o", "Hello"),
    ] {
        let a = ask(&env, &env.key, body, None).await;
        assert_eq!(a.status, StatusCode::OK, "{}", a.text());
        assert_eq!(a.header("kaeso-cache"), Some("miss"));
    }
    assert_eq!(env.mock.received().len(), 4);
}

#[tokio::test]
async fn keys_of_one_project_share_entries_and_other_projects_never_see_them() {
    let env = env(&[]).await;
    turn_on(&env).await;
    ask(&env, &env.key, hello(), None).await;

    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let second_key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "second" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let a = ask(&env, &second_key, hello(), None).await;
    assert_eq!(a.header("kaeso-cache"), Some("hit"));

    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Other" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.body);
    let other = other.body["id"].as_str().unwrap().to_owned();
    let other_base = format!("/orgs/{}/projects/{other}", env.owner.org_id);
    let r = env
        .app
        .post(
            &format!("{other_base}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let other_key = env
        .app
        .post(
            &format!("{other_base}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = set_project(&env, &other, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let a = ask(&env, &other_key, hello(), None).await;
    assert_eq!(a.header("kaeso-cache"), Some("miss"));
    assert_eq!(env.mock.received().len(), 2);
}

#[tokio::test]
async fn cache_control_skips_the_lookup_or_the_whole_cache() {
    let env = env(&[]).await;
    turn_on(&env).await;

    let a = ask(&env, &env.key, hello(), Some("no-store")).await;
    assert_eq!(a.header("kaeso-cache"), Some("bypass"));
    let a = ask(&env, &env.key, hello(), None).await;
    assert_eq!(
        a.header("kaeso-cache"),
        Some("miss"),
        "no-store kept nothing"
    );

    let a = ask(&env, &env.key, hello(), Some("no-cache")).await;
    assert_eq!(
        a.header("kaeso-cache"),
        Some("miss"),
        "no-cache asks the provider again"
    );
    let fresh = text(&a);
    let a = ask(&env, &env.key, hello(), None).await;
    assert_eq!(a.header("kaeso-cache"), Some("hit"));
    assert_eq!(text(&a), fresh, "no-cache stored the fresh reply");
    assert_eq!(env.mock.received().len(), 3);
}

#[tokio::test]
async fn streams_and_token_counts_pass_the_cache_by() {
    let env = env(&[]).await;
    turn_on(&env).await;
    let mut streamed = hello();
    streamed["stream"] = json!(true);
    for _ in 0..2 {
        let req = Request::builder()
            .uri("/v1/messages")
            .method("POST")
            .header("x-api-key", &env.key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&streamed).unwrap()))
            .unwrap();
        let resp = env.app.router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["kaeso-cache"], "bypass");
        resp.into_body().collect().await.unwrap();
    }
    let (_, body) = open_stream(&env, "/v1/messages", true, &streamed).await;
    body.collect().await.unwrap();
    assert_eq!(env.mock.received().len(), 3);
}

#[tokio::test]
async fn embeddings_are_cached_too() {
    let env = env(&[]).await;
    turn_on(&env).await;
    let body = json!({ "model": "text-embedding-3-small", "input": "Hello" });
    let first = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(body.clone()),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(first.header("kaeso-cache"), Some("miss"));
    env.app.state.in_flight.settled().await;
    let second = call(&env, "/v1/embeddings", Some(&env.key), false, Some(body)).await;
    assert_eq!(second.header("kaeso-cache"), Some("hit"));
    assert_eq!(second.json()["data"], first.json()["data"]);
    assert_eq!(env.mock.received().len(), 1);
}

#[tokio::test]
async fn a_provider_error_is_never_cached() {
    let env = env(&[]).await;
    turn_on(&env).await;
    let failing = chat("claude-haiku-4-5", "[mock:error=rate-limit]");
    for _ in 0..2 {
        let a = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(failing.clone()),
        )
        .await;
        assert_eq!(a.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(a.header("kaeso-cache"), Some("miss"));
    }
    assert_eq!(env.mock.received().len(), 2);
}

#[tokio::test]
async fn clearing_or_turning_off_stops_serving_stored_replies() {
    let env = env(&[]).await;
    turn_on(&env).await;
    ask(&env, &env.key, hello(), None).await;
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("hit")
    );

    let r = env
        .app
        .post(
            &format!("{}/clear", project_path(&env, &env.project)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("miss")
    );
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("hit")
    );

    let r = set_project(&env, &env.project, json!({ "enabled": false })).await;
    assert_eq!(r.body["effective"], false);
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        None
    );
    turn_on(&env).await;
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("miss"),
        "turning the cache off cleared it"
    );
    assert_eq!(env.mock.received().len(), 4);

    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .filter(|a| a.starts_with("response_cache."))
        .collect();
    assert_eq!(
        actions,
        [
            "response_cache.project_update",
            "response_cache.project_update",
            "response_cache.clear",
            "response_cache.project_update",
        ]
    );
}

#[tokio::test]
async fn cleared_entries_leave_valkey() {
    let env = env(&[]).await;
    turn_on(&env).await;
    ask(&env, &env.key, hello(), None).await;
    let pattern = env.app.state.valkey.key("rc:v1:*");
    let keys = || async {
        let mut conn = env.app.state.valkey.conn();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap();
        keys
    };
    let stored = keys().await;
    assert_eq!(stored.len(), 1);

    let mut conn = env.app.state.valkey.conn();
    let ttl: i64 = redis::cmd("PTTL")
        .arg(&stored[0])
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!((1..=3_600_000).contains(&ttl), "{ttl}");
    let raw: std::collections::HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
        .arg(&stored[0])
        .query_async(&mut conn)
        .await
        .unwrap();
    let all: Vec<u8> = raw.values().flatten().copied().collect();
    let reply = text(&ask(&env, &env.key, hello(), None).await);
    assert!(
        !String::from_utf8_lossy(&all).contains(&reply),
        "entries are encrypted"
    );

    let r = env
        .app
        .post(
            &format!("{}/clear", project_path(&env, &env.project)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    for _ in 0..200 {
        if keys().await.is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("cleared entries stayed in Valkey");
}

#[tokio::test]
async fn the_ttl_is_validated_and_applies_to_new_entries() {
    let env = env(&[]).await;
    for bad in [59, 604_801] {
        let r = set_project(&env, &env.project, json!({ "ttl_seconds": bad })).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(r.body["error"]["param"], "ttl_seconds");
    }
    let r = set_project(
        &env,
        &env.project,
        json!({ "enabled": true, "ttl_seconds": 120 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["ttl_seconds"], 120);
    ask(&env, &env.key, hello(), None).await;
    let mut conn = env.app.state.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("rc:v1:*"))
        .query_async(&mut conn)
        .await
        .unwrap();
    let ttl: i64 = redis::cmd("PTTL")
        .arg(&keys[0])
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!((1..=120_000).contains(&ttl), "{ttl}");
}

#[tokio::test]
async fn an_organization_can_forbid_the_cache_in_every_project() {
    let env = env(&[]).await;
    turn_on(&env).await;
    ask(&env, &env.key, hello(), None).await;
    let org_path = format!("/orgs/{}/response-cache", env.owner.org_id);
    let r = env.app.get(&org_path, &env.owner.cookie).await;
    assert_eq!(r.body, json!({ "allowed": true, "available": true }));

    let r = env
        .app
        .call(
            Method::PATCH,
            &org_path,
            Some(json!({ "allowed": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["allowed"], false);
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        None
    );

    let r = env
        .app
        .get(&project_path(&env, &env.project), &env.owner.cookie)
        .await;
    assert_eq!(r.body["enabled"], true);
    assert_eq!(r.body["org_allowed"], false);
    assert_eq!(r.body["effective"], false);

    let r = set_project(&env, &env.project, json!({ "enabled": false })).await;
    assert_eq!(r.status, StatusCode::OK);
    let r = set_project(&env, &env.project, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "The organization has the response cache off; a project cannot turn it on."
    );

    let r = env
        .app
        .call(
            Method::PATCH,
            &org_path,
            Some(json!({ "allowed": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    turn_on(&env).await;
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("miss"),
        "forbidding the cache cleared it"
    );
}

#[tokio::test]
async fn an_install_without_the_cache_never_uses_it() {
    let env = env(&[("KAESO_RESPONSE_CACHE_ENABLED", "false")]).await;
    let r = set_project(&env, &env.project, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "The response cache is off on this install."
    );
    let r = env
        .app
        .get(&project_path(&env, &env.project), &env.owner.cookie)
        .await;
    assert_eq!(r.body["available"], false);
    assert_eq!(r.body["effective"], false);
}

#[tokio::test]
async fn a_full_valkey_stops_new_entries_but_not_requests() {
    let env = env(&[]).await;
    turn_on(&env).await;
    env.app.state.response_cache.set_room(false);
    for _ in 0..2 {
        let a = ask(&env, &env.key, hello(), None).await;
        assert_eq!(a.status, StatusCode::OK);
        assert_eq!(a.header("kaeso-cache"), Some("miss"));
    }
    kaeso_server::services::response_cache::check_memory(&env.app.state).await;
    assert!(
        env.app.state.response_cache.has_room(),
        "the test Valkey sets no maxmemory"
    );
    ask(&env, &env.key, hello(), None).await;
    assert_eq!(
        ask(&env, &env.key, hello(), None)
            .await
            .header("kaeso-cache"),
        Some("hit")
    );
}

#[tokio::test]
async fn an_archived_project_cannot_change_its_cache() {
    let env = env(&[]).await;
    let created = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Old" }),
            Some(&env.owner.cookie),
        )
        .await;
    let id = created.body["id"].as_str().unwrap().to_owned();
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{id}/archive", env.owner.org_id),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    let r = set_project(&env, &id, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = env
        .app
        .post(
            &format!("{}/clear", project_path(&env, &id)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}
