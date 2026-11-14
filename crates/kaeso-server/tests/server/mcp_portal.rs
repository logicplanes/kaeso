#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use crate::common;

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use futures::StreamExt;
use kaeso_store::clickhouse::tool_calls::ToolCallRow;
use kaeso_testkit::mcp::{Era, MockMcp, Options, VIEW_HTML, default_tools};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use tower::ServiceExt;

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");
const MODERN: &str = "2026-07-28";

struct Portal {
    app: App,
    owner: Owner,
    project: String,
    key: String,
    server: String,
    tools: Value,
}

async fn portal(s: &TestServices, mock: &MockMcp, extra: &[(&str, &str)]) -> Portal {
    let mut env = vec![LOOPBACK];
    env.extend_from_slice(extra);
    let app = App::start_with(s, &env).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let created = app
        .post(
            &format!("/orgs/{}/mcp-servers", owner.org_id),
            json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    app.state.in_flight.settled().await;
    let at_project = format!("/orgs/{}/projects/{project}", owner.org_id);
    let tools = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    let id = |name: &str| {
        tools.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()["id"]
            .clone()
    };
    let on = app
        .call(
            Method::PATCH,
            &format!("{at_project}/tools"),
            Some(json!({ "tools": [
                { "id": id("create_post"), "enabled": true },
                { "id": id("search_posts"), "enabled": true },
            ] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.body);
    let key = app
        .post(
            &format!("{at_project}/keys"),
            json!({ "name": "claude", "tools": true }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    assert_eq!(key.body["tools"], true);
    Portal {
        server: created.body["id"].as_str().unwrap().to_owned(),
        key: key.body["secret"].as_str().unwrap().to_owned(),
        tools: on.body,
        app,
        owner,
        project,
    }
}

fn tool_id(p: &Portal, name: &str) -> Value {
    p.tools["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap()["id"]
        .clone()
}

fn request(p: &Portal, key: Option<&str>, version: Option<&str>, message: &Value) -> Request<Body> {
    let mut message = message.clone();
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(format!("/mcp/{}", p.project))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream");
    if let Some(k) = key {
        req = req.header(header::AUTHORIZATION, format!("Bearer {k}"));
    }
    if let Some(v) = version {
        req = req.header("mcp-protocol-version", v);
    }
    if version == Some(MODERN) {
        let method = message["method"].as_str().unwrap().to_owned();
        req = req.header("mcp-method", &method);
        let name = if method == "resources/read" {
            "uri"
        } else {
            "name"
        };
        if let Some(name) = message["params"][name].as_str() {
            req = req.header("mcp-name", name);
        }
        message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!(MODERN);
        message["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"] = json!({});
        message["params"]["_meta"]["io.modelcontextprotocol/clientInfo"] =
            json!({ "name": "claude-ai", "version": "1" });
    }
    req.body(Body::from(serde_json::to_vec(&message).unwrap()))
        .unwrap()
}

async fn send(p: &Portal, version: Option<&str>, message: Value) -> common::Resp {
    p.app.raw(request(p, Some(&p.key), version, &message)).await
}

fn rpc(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn names(list: &common::Resp) -> Vec<String> {
    list.body["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("{:?}", list.body))
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

async fn logged(p: &Portal) -> Vec<ToolCallRow> {
    p.app.state.in_flight.settled().await;
    p.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&p.app.state.pg)
        .await
        .unwrap();
    kaeso_server::services::logs::flush_once(&p.app.state)
        .await
        .unwrap();
    p.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .client()
        .query("SELECT ?fields FROM tool_calls WHERE project_id = ? ORDER BY started_at, id")
        .bind(common::uuid(&p.project))
        .fetch_all::<ToolCallRow>()
        .await
        .unwrap()
}

#[tokio::test]
async fn a_newest_version_client_calling_an_older_server_keeps_its_meta_but_not_its_claims() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        era: Era::Legacy,
        ..Options::default()
    })
    .await;
    let p = portal(&s, &mock, &[]).await;

    let call = rpc(
        1,
        "tools/call",
        json!({ "name": "x__create_post", "arguments": { "text": "Hi" }, "_meta": { "progressToken": "p1" } }),
    );
    let r = send(&p, Some(MODERN), call).await;
    assert_eq!(r.body["result"]["isError"], false, "{:?}", r.body);
    let upstream = mock
        .seen()
        .into_iter()
        .rfind(|s| s.method == "tools/call")
        .unwrap();
    assert!(
        upstream.session.is_some(),
        "the older server got its session"
    );
    assert_eq!(upstream.protocol_version.as_deref(), Some("2025-11-25"));
    assert_eq!(
        upstream.body["params"]["_meta"],
        json!({ "progressToken": "p1" }),
        "the client's own `_meta` stays; the newest version's claims go"
    );

    let bare = rpc(
        2,
        "tools/call",
        json!({ "name": "x__create_post", "arguments": { "text": "Hi" } }),
    );
    let r = send(&p, Some(MODERN), bare).await;
    assert_eq!(r.body["result"]["isError"], false, "{:?}", r.body);
    let upstream = mock
        .seen()
        .into_iter()
        .rfind(|s| s.method == "tools/call")
        .unwrap();
    assert!(
        upstream.body["params"].get("_meta").is_none(),
        "an emptied `_meta` is dropped: {:?}",
        upstream.body
    );
}

#[tokio::test]
async fn a_key_lists_and_calls_the_projects_tools_in_both_protocol_eras() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;

    let r = send(&p, Some(MODERN), rpc(1, "server/discover", json!({}))).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(
        r.body["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!(MODERN))
    );
    assert_eq!(
        r.body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "kaeso-portal"
    );
    let list = send(&p, Some(MODERN), rpc(2, "tools/list", json!({}))).await;
    assert_eq!(names(&list), ["x__create_post", "x__search_posts"]);
    assert_eq!(list.body["result"]["resultType"], "complete");
    assert_eq!(list.body["result"]["cacheScope"], "private");
    assert_eq!(
        list.body["result"]["tools"][1]["outputSchema"]["type"],
        "object"
    );
    let again = send(&p, Some(MODERN), rpc(2, "tools/list", json!({}))).await;
    assert_eq!(again.body, list.body, "one snapshot, the same bytes");

    let called = send(
        &p,
        Some(MODERN),
        rpc(
            3,
            "tools/call",
            json!({ "name": "x__create_post", "arguments": { "text": "Hello, X" } }),
        ),
    )
    .await;
    assert_eq!(called.status, StatusCode::OK, "{:?}", called.body);
    assert_eq!(called.body["id"], 3);
    let timing = called.headers["server-timing"].to_str().unwrap();
    assert!(
        timing.starts_with("kaeso;dur=") && timing.contains(", upstream;dur="),
        "{timing}"
    );
    assert_eq!(called.body["result"]["isError"], false);
    assert_eq!(
        called.body["result"]["content"][0]["text"],
        r#"create_post ran with {"text":"Hello, X"}"#
    );
    let upstream = mock
        .seen()
        .into_iter()
        .rfind(|s| s.method == "tools/call")
        .unwrap();
    assert_eq!(upstream.body["params"]["name"], "create_post");
    assert_eq!(upstream.mcp_name.as_deref(), Some("create_post"));
    assert_eq!(
        upstream.body["params"]["_meta"]["io.modelcontextprotocol/clientInfo"]["name"],
        "kaeso"
    );
    assert_eq!(
        upstream.authorization, None,
        "the client's key never goes upstream"
    );

    let init = send(
        &p,
        None,
        rpc(4, "initialize", json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "old", "version": "1" } })),
    )
    .await;
    assert_eq!(init.body["result"]["protocolVersion"], "2025-11-25");
    assert!(
        init.headers.get("mcp-session-id").is_none(),
        "the portal keeps no sessions"
    );
    let list = send(&p, Some("2025-11-25"), rpc(5, "tools/list", json!({}))).await;
    assert_eq!(names(&list), ["x__create_post", "x__search_posts"]);
    assert!(list.body["result"].get("resultType").is_none());
    let called = send(
        &p,
        Some("2025-11-25"),
        rpc(
            6,
            "tools/call",
            json!({ "name": "x__search_posts", "arguments": { "query": "kaeso" } }),
        ),
    )
    .await;
    assert_eq!(called.body["result"]["isError"], false, "{:?}", called.body);

    let mut rows = logged(&p).await;
    rows.sort_by_key(|r| r.protocol_version != MODERN);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (
            rows[0].tool.as_str(),
            rows[0].upstream_tool.as_str(),
            rows[0].status.as_str()
        ),
        ("x__create_post", "create_post", "ok")
    );
    assert_eq!(rows[0].client_name, "claude-ai");
    assert_eq!(rows[0].protocol_version, MODERN);
    assert_eq!(rows[0].caller_kind, "user");
    assert_eq!(rows[1].protocol_version, "2025-11-25");
}

#[tokio::test]
async fn held_switched_off_and_denied_tools_are_hidden_and_refused() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;
    let call = |name: &str| rpc(9, "tools/call", json!({ "name": name, "arguments": {} }));

    let r = send(&p, Some(MODERN), call("x__get_me")).await;
    assert_eq!(r.body["error"]["code"], -32602);
    assert_eq!(
        r.body["error"]["message"],
        "The tool `x__get_me` is off in this project; a project admin turns it on under Tools."
    );

    let mut changed = default_tools();
    changed[0]["description"] = json!("Posts to X and reads your direct messages.");
    mock.set_tools(changed);
    let servers = format!("/orgs/{}/mcp-servers/{}", p.owner.org_id, p.server);
    let refreshed = p
        .app
        .post(
            &format!("{servers}/refresh"),
            json!({}),
            Some(&p.owner.cookie),
        )
        .await;
    assert_eq!(refreshed.body["held"], json!(["create_post"]));
    let list = send(&p, Some(MODERN), rpc(1, "tools/list", json!({}))).await;
    assert_eq!(names(&list), ["x__search_posts"], "a held tool is hidden");
    let r = send(&p, Some(MODERN), call("x__create_post")).await;
    assert_eq!(
        r.body["error"]["message"],
        "The tool `x__create_post` changed and waits for an admin's review; try again once it's approved."
    );
    let tool = tool_id(&p, "create_post");
    let change = p
        .app
        .get(
            &format!("{servers}/tools/{}/change", tool.as_str().unwrap()),
            &p.owner.cookie,
        )
        .await;
    p.app
        .post(
            &format!("{servers}/tools/{}/approve", tool.as_str().unwrap()),
            json!({ "hash": change.body["tool"]["hash"] }),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let list = send(&p, Some(MODERN), rpc(1, "tools/list", json!({}))).await;
    assert_eq!(
        names(&list),
        ["x__create_post", "x__search_posts"],
        "visible again after approval"
    );
    assert_eq!(
        list.body["result"]["tools"][0]["description"],
        "Posts to X and reads your direct messages."
    );

    let r = p
        .app
        .call(
            Method::PUT,
            &format!(
                "/orgs/{}/projects/{}/tool-access/{}",
                p.owner.org_id, p.project, p.owner.user_id
            ),
            Some(json!({ "tools": [tool_id(&p, "search_posts")] })),
            Some(&p.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let list = send(&p, Some(MODERN), rpc(1, "tools/list", json!({}))).await;
    assert_eq!(names(&list), ["x__search_posts"]);
    let r = send(&p, Some(MODERN), call("x__create_post")).await;
    assert_eq!(
        r.body["error"]["message"],
        "You may not call `x__create_post`; ask a project admin for access to it."
    );
    let r = send(&p, Some(MODERN), call("x__nothing")).await;
    assert_eq!(
        r.body["error"]["message"],
        "No tool `x__nothing` in this project's portal; list the tools again."
    );
    let mut kinds: Vec<(String, String)> = logged(&p)
        .await
        .into_iter()
        .map(|r| (r.status, r.error_type))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            ("refused".to_owned(), "denied".to_owned()),
            ("refused".to_owned(), "tool_changed".to_owned()),
            ("refused".to_owned(), "tool_off".to_owned()),
        ]
    );
}

#[tokio::test]
async fn only_a_key_with_tool_access_of_this_project_gets_in() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;
    let list = rpc(1, "tools/list", json!({}));

    let r = p.app.raw(request(&p, None, Some(MODERN), &list)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        r.headers[header::WWW_AUTHENTICATE],
        format!(
            "Bearer resource_metadata=\"https://kaeso.test/.well-known/oauth-protected-resource/mcp/{}\"",
            p.project
        )
        .as_str()
    );
    let r = p
        .app
        .raw(request(&p, Some("sk-kso-nothing"), Some(MODERN), &list))
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(
        r.headers[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .contains("invalid_token")
    );

    let at_project = format!("/orgs/{}/projects/{}", p.owner.org_id, p.project);
    let plain = p
        .app
        .post(
            &format!("{at_project}/keys"),
            json!({ "name": "models" }),
            Some(&p.owner.cookie),
        )
        .await;
    assert_eq!(plain.body["tools"], false);
    let r = p
        .app
        .raw(request(
            &p,
            plain.body["secret"].as_str(),
            Some(MODERN),
            &list,
        ))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "This API key has no tool access; create a key with tool access for the portal."
    );

    let other = common::create_project(&p.app, &p.owner.org_id, &p.owner.cookie, "Other").await;
    let elsewhere = p
        .app
        .post(
            &format!("/orgs/{}/projects/{other}/keys", p.owner.org_id),
            json!({ "name": "other", "tools": true }),
            Some(&p.owner.cookie),
        )
        .await;
    let r = p
        .app
        .raw(request(
            &p,
            elsewhere.body["secret"].as_str(),
            Some(MODERN),
            &list,
        ))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "This API key belongs to another project; use a key of this project."
    );

    let r = p
        .app
        .raw(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/mcp/{}", p.project))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
    let r = p
        .app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp/not-a-project")
                .header(header::AUTHORIZATION, format!("Bearer {}", p.key))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_newest_version_checks_its_headers() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;
    let mut req = request(
        &p,
        Some(&p.key),
        Some(MODERN),
        &rpc(
            1,
            "tools/call",
            json!({ "name": "x__create_post", "arguments": {} }),
        ),
    );
    req.headers_mut()
        .insert("mcp-name", "x__search_posts".parse().unwrap());
    let r = p.app.raw(req).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["code"], -32020);

    let mut req = request(
        &p,
        Some(&p.key),
        Some(MODERN),
        &rpc(1, "tools/list", json!({})),
    );
    req.headers_mut()
        .insert("mcp-protocol-version", "2099-01-01".parse().unwrap());
    let r = p.app.raw(req).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = send(
        &p,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": "2099-01-01" } } }),
    )
    .await;
    assert_eq!(r.body["error"]["code"], -32022);
    assert_eq!(r.body["error"]["data"]["requested"], "2099-01-01");

    let r = send(&p, Some(MODERN), rpc(1, "resources/subscribe", json!({}))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.body["error"]["code"], -32601);
    let r = send(
        &p,
        Some(MODERN),
        json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {} }),
    )
    .await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn warm_calls_do_not_touch_postgres() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;
    let call = || {
        rpc(
            7,
            "tools/call",
            json!({ "name": "x__create_post", "arguments": { "text": "warm" } }),
        )
    };
    for _ in 0..2 {
        assert_eq!(
            names(&send(&p, Some(MODERN), rpc(1, "tools/list", json!({}))).await).len(),
            2
        );
        let r = send(&p, Some(MODERN), call()).await;
        assert_eq!(r.body["result"]["isError"], false, "{:?}", r.body);
    }
    p.app.state.in_flight.settled().await;
    let mut blocker = p.app.state.pg.begin().await.unwrap();
    sqlx::query(
        "LOCK TABLE api_keys, projects, organizations, users, org_memberships, project_memberships, \
         mcp_servers, mcp_connections, mcp_tools, project_mcp_servers, project_tools, tool_access, \
         portal_grants, portal_tokens IN ACCESS EXCLUSIVE MODE",
    )
    .execute(&mut *blocker)
    .await
    .unwrap();
    let started = Instant::now();
    let warm = tokio::time::timeout(Duration::from_secs(5), async {
        let list = send(&p, Some(MODERN), rpc(1, "tools/list", json!({}))).await;
        let called = send(&p, Some(MODERN), call()).await;
        (list, called)
    })
    .await
    .expect("a warm call waited for PostgreSQL");
    assert_eq!(names(&warm.0).len(), 2);
    assert_eq!(warm.1.body["result"]["isError"], false, "{:?}", warm.1.body);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    blocker.rollback().await.unwrap();
}

#[tokio::test]
async fn an_event_stream_reaches_the_client_as_it_comes_and_the_limit_cuts_it_cleanly() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        stream_calls: true,
        ..Options::default()
    })
    .await;
    let p = portal(&s, &mock, &[("KAESO_MCP_PORTAL_RESPONSE_LIMIT", "1KiB")]).await;
    mock.hold_calls();
    let res = p
        .app
        .router
        .clone()
        .oneshot(request(
            &p,
            Some(&p.key),
            Some(MODERN),
            &rpc(
                5,
                "tools/call",
                json!({ "name": "x__create_post", "arguments": { "text": "streamed" } }),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");
    let mut body = res.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(5), body.next())
        .await
        .expect("the progress event arrived while the server still works")
        .unwrap()
        .unwrap();
    let first = String::from_utf8(first.to_vec()).unwrap();
    assert!(first.contains("notifications/progress"), "{first}");
    mock.release();
    let mut rest = String::new();
    while let Some(chunk) = body.next().await {
        rest.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    assert!(rest.contains(r#""id":5"#), "{rest}");
    assert!(rest.contains("create_post ran with"), "{rest}");

    let (_, _, bytes) = p
        .app
        .raw_bytes(request(
            &p,
            Some(&p.key),
            Some(MODERN),
            &rpc(
                6,
                "tools/call",
                json!({ "name": "x__create_post", "arguments": { "text": "x".repeat(2048) } }),
            ),
        ))
        .await;
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains(
            "X's answer is larger than the portal passes on (1 KiB); ask for less at once."
        ),
        "{text}"
    );
    assert!(text.contains(r#""isError":true"#), "{text}");
    let rows = logged(&p).await;
    assert!(rows.iter().any(|r| r.error_type == "too_large"), "{rows:?}");
}

#[tokio::test]
async fn kaeso_sends_its_own_credential_and_marks_a_refused_one() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("upstream-secret".into()),
        era: Era::Legacy,
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let created = app
        .post(
            &format!("/orgs/{}/mcp-servers", owner.org_id),
            json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "header", "projects": [project] }),
            Some(&owner.cookie),
        )
        .await;
    let server = created.body["id"].as_str().unwrap().to_owned();
    let connection = format!("/orgs/{}/mcp-servers/{server}/connection", owner.org_id);
    let r = app
        .post(
            &connection,
            json!({ "header": { "name": "Authorization", "value": "Bearer upstream-secret" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["server"]["status"], "ok", "{:?}", r.body);
    let at_project = format!("/orgs/{}/projects/{project}", owner.org_id);
    let tools = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    let post = tools.body["data"][0]["id"].clone();
    app.call(
        Method::PATCH,
        &format!("{at_project}/tools"),
        Some(json!({ "tools": [{ "id": post, "enabled": true }] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let key = app
        .post(
            &format!("{at_project}/keys"),
            json!({ "name": "c", "tools": true }),
            Some(&owner.cookie),
        )
        .await;
    let p = Portal {
        key: key.body["secret"].as_str().unwrap().to_owned(),
        server,
        tools: tools.body,
        app,
        owner,
        project,
    };
    let call = rpc(
        1,
        "tools/call",
        json!({ "name": "x__create_post", "arguments": {} }),
    );
    let r = send(&p, Some("2025-11-25"), call.clone()).await;
    assert_eq!(r.body["result"]["isError"], false, "{:?}", r.body);
    let upstream = mock
        .seen()
        .into_iter()
        .rfind(|s| s.method == "tools/call")
        .unwrap();
    assert_eq!(
        upstream.authorization.as_deref(),
        Some("Bearer upstream-secret")
    );
    assert!(
        upstream.session.is_some(),
        "an older server gets its session"
    );

    mock.end_sessions();
    let r = send(&p, Some("2025-11-25"), call.clone()).await;
    assert_eq!(
        r.body["result"]["isError"], false,
        "a lost session is rebuilt: {:?}",
        r.body
    );

    let r = p
        .app
        .post(
            &connection,
            json!({ "header": { "name": "Authorization", "value": "Bearer wrong" } }),
            Some(&p.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = send(&p, Some("2025-11-25"), call).await;
    assert_eq!(r.body["result"]["isError"], true);
    assert_eq!(
        r.body["result"]["content"][0]["text"],
        "X refused Kaeso's sign-in. An admin connects X again under MCP servers; then try again."
    );
    let server = p
        .app
        .get(
            &format!("/orgs/{}/mcp-servers/{}", p.owner.org_id, p.server),
            &p.owner.cookie,
        )
        .await;
    assert_eq!(server.body["connection"]["signed_out"], true);
    let rows = logged(&p).await;
    assert!(
        rows.iter().any(|r| r.error_type == "upstream_auth"),
        "{rows:?}"
    );
}

#[tokio::test]
async fn a_servers_mcp_apps_views_reach_the_client_under_the_portals_names() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let p = portal(&s, &mock, &[]).await;
    let at_project = format!("/orgs/{}/projects/{}", p.owner.org_id, p.project);
    p.app
        .call(
            Method::PATCH,
            &format!("{at_project}/tools"),
            Some(json!({ "tools": [{ "id": tool_id(&p, "get_me"), "enabled": true }] })),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let view = "ui://x/x/me";
    for version in [Some(MODERN), Some("2025-11-25")] {
        let init = send(&p, None, rpc(1, "initialize", json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "claude-ai", "version": "1" } }))).await;
        assert!(init.body["result"]["capabilities"]["resources"].is_object());
        let list = send(&p, version, rpc(2, "tools/list", json!({}))).await;
        let me = list.body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "x__get_me")
            .unwrap()
            .clone();
        assert_eq!(me["_meta"]["ui"]["resourceUri"], view, "{version:?}");
        let resources = send(&p, version, rpc(3, "resources/list", json!({}))).await;
        assert_eq!(
            resources.body["result"]["resources"],
            json!([{ "uri": view, "name": "x/x/me", "mimeType": "text/html;profile=mcp-app" }]),
            "{version:?}"
        );
        let read = send(
            &p,
            version,
            rpc(4, "resources/read", json!({ "uri": view })),
        )
        .await;
        let item = &read.body["result"]["contents"][0];
        assert_eq!(item["uri"], view, "{:?}", read.body);
        assert_eq!(item["mimeType"], "text/html;profile=mcp-app");
        assert_eq!(item["text"], VIEW_HTML);
        assert_eq!(
            item["_meta"]["ui"],
            json!({ "csp": { "connectDomains": ["https://api.x.example"] }, "prefersBorder": false }),
            "the server's policy goes on as it is"
        );
        if version == Some(MODERN) {
            assert_eq!(read.body["result"]["resultType"], "complete");
            assert_eq!(resources.body["result"]["ttlMs"], 300_000);
            assert_eq!(resources.body["result"]["cacheScope"], "private");
        }
        let from_view = send(
            &p,
            version,
            rpc(
                5,
                "tools/call",
                json!({ "name": "get_me", "arguments": {} }),
            ),
        )
        .await;
        assert_eq!(
            from_view.body["result"]["content"][0]["text"], "get_me ran with {}",
            "a view calls its server's tool by the server's name: {:?}",
            from_view.body
        );
    }
    let asked = mock.seen();
    let read = asked.iter().find(|s| s.method == "resources/read").unwrap();
    assert_eq!(read.body["params"]["uri"], "ui://x/me");
    let unknown = send(
        &p,
        Some(MODERN),
        rpc(6, "resources/read", json!({ "uri": "ui://x/x/other" })),
    )
    .await;
    assert_eq!(unknown.body["error"]["code"], -32002);

    p.app
        .call(
            Method::PUT,
            &format!("{at_project}/tool-access/{}", p.owner.user_id),
            Some(json!({ "tools": [tool_id(&p, "search_posts")] })),
            Some(&p.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let hidden = send(
        &p,
        Some(MODERN),
        rpc(7, "resources/read", json!({ "uri": view })),
    )
    .await;
    assert_eq!(
        hidden.body["error"]["code"], -32002,
        "a view of a tool the caller may not use is unknown"
    );
    let resources = send(&p, Some(MODERN), rpc(8, "resources/list", json!({}))).await;
    assert_eq!(resources.body["result"]["resources"], json!([]));
}
