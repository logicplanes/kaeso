#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_testkit::mcp::{MockMcp, Options};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");

struct Env {
    app: App,
    owner: Owner,
    project: String,
    key: String,
    key_id: String,
}

impl Env {
    fn at(&self) -> String {
        format!("/orgs/{}/projects/{}", self.owner.org_id, self.project)
    }
}

async fn env(s: &TestServices, mock: &MockMcp) -> Env {
    let app = App::start_with(s, &[LOOPBACK]).await;
    app.state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&app.state.pg)
        .await
        .unwrap();
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    app.post(
        &format!("/orgs/{}/mcp-servers", owner.org_id),
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project] }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    app.state.in_flight.settled().await;
    let at = format!("/orgs/{}/projects/{project}", owner.org_id);
    let tools = app.get(&format!("{at}/tools"), &owner.cookie).await;
    let post = tools.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "create_post")
        .unwrap()["id"]
        .clone();
    app.call(
        Method::PATCH,
        &format!("{at}/tools"),
        Some(json!({ "tools": [{ "id": post, "enabled": true }] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let key = app
        .post(
            &format!("{at}/keys"),
            json!({ "name": "claude", "tools": true }),
            Some(&owner.cookie),
        )
        .await;
    Env {
        key: key.body["secret"].as_str().unwrap().to_owned(),
        key_id: key.body["id"].as_str().unwrap().to_owned(),
        app,
        owner,
        project,
    }
}

async fn call(e: &Env, tool: &str) -> common::Resp {
    e.app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/mcp/{}", e.project))
                .header(header::AUTHORIZATION, format!("Bearer {}", e.key))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": { "text": "Hello" } } })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
}

async fn flushed(e: &Env) {
    e.app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_once(&e.app.state)
        .await
        .unwrap();
    kaeso_server::services::usage::flush(&e.app.state)
        .await
        .unwrap();
}

async fn calls(e: &Env, query: &str) -> Value {
    let r = e
        .app
        .get(
            &format!("{}/logs/tool-calls{query}", e.at()),
            &e.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

#[tokio::test]
async fn tool_calls_are_logged_with_their_content_where_content_is_kept() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;
    assert_eq!(
        call(&e, "x__create_post").await.body["result"]["isError"],
        false
    );
    assert_eq!(call(&e, "x__get_me").await.body["error"]["code"], -32602);
    flushed(&e).await;

    let all = calls(&e, "").await;
    assert_eq!(all["data"].as_array().unwrap().len(), 2);
    let refused = calls(&e, "?status=refused").await;
    let off = &refused["data"][0];
    assert_eq!(
        (off["tool"].as_str(), off["error_type"].as_str()),
        (Some("x__get_me"), Some("tool_off"))
    );
    let ok = &calls(&e, "?status=ok").await["data"][0];
    assert_eq!(ok["tool"], "x__create_post");
    assert_eq!(ok["upstream_tool"], "create_post");
    assert_eq!(ok["caller"]["kind"], "user");
    assert_eq!(ok["caller"]["id"], e.owner.user_id);
    assert_eq!(ok["key_id"], e.key_id);
    assert_eq!(ok["source"], "portal");
    assert_eq!(ok["has_content"], true);
    assert!(ok["upstream_ms"].is_number());
    let one = e
        .app
        .get(
            &format!("{}/logs/tool-calls/{}", e.at(), ok["id"].as_str().unwrap()),
            &e.owner.cookie,
        )
        .await;
    assert_eq!(one.body["content"]["arguments"], json!({ "text": "Hello" }));
    assert_eq!(one.body["content"]["result"]["result"]["isError"], false);
    assert_eq!(
        calls(&e, &format!("?key={}&tool=x__create_post", e.key_id)).await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let r = e
        .app
        .get(
            &format!("{}/logs/tool-calls?status=maybe", e.at()),
            &e.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    e.app
        .call(
            Method::PATCH,
            &format!("{}/logs/settings", e.at()),
            Some(json!({ "content_logging": false })),
            Some(&e.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    e.app.clock.advance(time::Duration::seconds(1));
    call(&e, "x__create_post").await;
    flushed(&e).await;
    let listed = calls(&e, "?status=ok").await;
    let contents: Vec<&Value> = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["has_content"])
        .collect();
    assert_eq!(contents, [&json!(false), &json!(true)], "{listed:#?}");
    let r = e
        .app
        .get(
            &format!("{}/logs/tool-calls/call_00000000000000000000000000", e.at()),
            &e.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(
        r.body["error"]["message"],
        "No such tool call, or you cannot see it."
    );
}

#[tokio::test]
async fn a_tool_call_rate_stops_calls_with_retry_after_and_usage_counts_them() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;
    let r = e
        .app
        .post(
            &format!("{}/limits", e.at()),
            json!({ "level": "key", "key": e.key_id, "kind": "tool_call_rate", "period": "minute", "amount": "2" }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = e
        .app
        .post(
            &format!("{}/limits", e.at()),
            json!({ "level": "project", "kind": "tool_call_rate", "period": "day", "amount": "2" }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["message"], "Rates are per `minute`.");

    for _ in 0..2 {
        assert_eq!(
            call(&e, "x__create_post").await.body["result"]["isError"],
            false
        );
    }
    let limited = call(&e, "x__create_post").await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(limited.headers.contains_key(header::RETRY_AFTER));
    assert_eq!(limited.body["error"]["code"], -32000);
    assert!(
        limited.body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with(
                "Rate limit reached: this key's tool call rate of 2 tool calls per minute; retry in"
            ),
        "{:?}",
        limited.body
    );
    flushed(&e).await;
    let rows = calls(&e, "?error_type=limited").await;
    assert_eq!(rows["data"].as_array().unwrap().len(), 1);

    let usage = e
        .app
        .get(&format!("{}/usage?group_by=day", e.at()), &e.owner.cookie)
        .await;
    let today = &usage.body["data"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        (
            today["tool_calls"].clone(),
            today["tool_call_errors"].clone()
        ),
        (json!(3), json!(1))
    );
    assert_eq!(today["requests"], 0, "tool calls aren't model requests");
    let by_model = e
        .app
        .get(&format!("{}/usage?group_by=model", e.at()), &e.owner.cookie)
        .await;
    assert_eq!(by_model.body["data"], json!([]));
}
