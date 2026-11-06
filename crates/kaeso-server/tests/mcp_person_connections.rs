#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, ORIGIN, Owner};
use kaeso_testkit::mcp::{MockMcp, OAuth, Options};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use url::Url;

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");
const MINE: &str = "/me/mcp-connections";

async fn add(app: &App, owner: &Owner, body: Value) -> String {
    let r = app
        .post(
            &format!("/orgs/{}/mcp-servers", owner.org_id),
            body,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

async fn mine(app: &App, cookie: &str) -> Value {
    let r = app.get(MINE, cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].clone()
}

async fn call(app: &App, project: &str, key: &str) -> Value {
    app.raw(
        Request::builder()
            .method(Method::POST)
            .uri(format!("/mcp/{project}"))
            .header(header::AUTHORIZATION, format!("Bearer {key}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "x__create_post", "arguments": { "text": "Hi" } } })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await
    .body
}

#[tokio::test]
async fn each_person_connects_their_own_secret_and_their_calls_use_it() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("person-token".into()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let server = add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "header", "credential_mode": "per_user", "projects": [project] }),
    )
    .await;
    let shared = add(
        &app,
        &owner,
        json!({ "handle": "docs", "name": "Docs", "url": mock.url(), "auth_method": "header" }),
    )
    .await;

    let listed = mine(&app, &owner.cookie).await;
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed:?}");
    assert_eq!(listed[0]["server"], server.as_str());
    assert_eq!(listed[0]["auth_method"], "header");
    assert_eq!(listed[0]["connection"], Value::Null);
    let at = format!("/orgs/{}/mcp-servers/{server}", owner.org_id);
    assert_eq!(
        app.get(&format!("{at}/tools"), &owner.cookie).await.body["data"],
        json!([]),
        "nobody has connected, so the server has no tools yet"
    );

    let r = app
        .post(
            &format!("{MINE}/{server}"),
            json!({ "header": { "name": "Authorization", "value": "Bearer person-token" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["redirect_url"], Value::Null);
    assert_eq!(r.body["connection"]["connection"]["last4"], "oken");
    assert_eq!(
        r.body["connection"]["connection"]["header_name"],
        "Authorization"
    );
    let tools = app.get(&format!("{at}/tools"), &owner.cookie).await;
    let post = tools.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "create_post")
        .unwrap()["id"]
        .clone();

    let in_project = format!("/orgs/{}/projects/{project}", owner.org_id);
    app.call(
        Method::PATCH,
        &format!("{in_project}/tools"),
        Some(json!({ "tools": [{ "id": post, "enabled": true }] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let key = app
        .post(
            &format!("{in_project}/keys"),
            json!({ "name": "claude", "tools": true }),
            Some(&owner.cookie),
        )
        .await;
    let key = key.body["secret"].as_str().unwrap().to_owned();
    assert_eq!(call(&app, &project, &key).await["result"]["isError"], false);

    for (body, status) in [
        (json!({ "oauth": true }), StatusCode::BAD_REQUEST),
        (json!({}), StatusCode::BAD_REQUEST),
        (
            json!({ "header": { "name": "Authorization", "value": "x" }, "oauth": true }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "header": { "name": "bad header", "value": "x" } }),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let r = app
            .post(&format!("{MINE}/{server}"), body, Some(&owner.cookie))
            .await;
        assert_eq!(r.status, status, "{:?}", r.body);
    }
    for missing in [shared.as_str(), "mcp_00000000000000000000000000", "nope"] {
        let r = app
            .post(
                &format!("{MINE}/{missing}"),
                json!({ "header": { "name": "Authorization", "value": "x" } }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{missing}: {:?}", r.body);
    }

    let r = app
        .call(
            Method::DELETE,
            &format!("{MINE}/{server}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        mine(&app, &owner.cookie).await[0]["connection"],
        Value::Null
    );
    let refused = call(&app, &project, &key).await;
    assert_eq!(refused["result"]["isError"], true, "{refused:?}");
    let r = app
        .call(
            Method::DELETE,
            &format!("{MINE}/{server}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    let audit = app
        .get(
            &format!("/orgs/{}/audit-log?limit=100", owner.org_id),
            &owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"mcp_server.disconnect"), "{actions:?}");
    assert!(actions.contains(&"mcp_server.connect"), "{actions:?}");
}

#[tokio::test]
async fn a_person_signs_in_with_oauth_and_comes_back_to_their_connections() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: true,
            label: Some("@kaesoapp".into()),
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let server = add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "oauth", "credential_mode": "per_user" }),
    )
    .await;
    let r = app
        .post(
            &format!("{MINE}/{server}"),
            json!({ "header": { "name": "Authorization", "value": "x" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "This server signs in with OAuth; send `oauth`."
    );
    let r = app
        .post(
            &format!("{MINE}/{server}"),
            json!({ "oauth": true }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let start = r.body["redirect_url"].as_str().unwrap();
    let back = Url::parse(&mock.authorize(start)).unwrap();
    let r = app
        .raw(
            Request::builder()
                .uri(format!("{}?{}", back.path(), back.query().unwrap()))
                .header(
                    header::COOKIE,
                    format!("__Host-kaeso_session={}", owner.cookie),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(
        r.headers[header::LOCATION],
        format!("{ORIGIN}/settings/account/connections?server={server}&result=connected").as_str()
    );
    app.state.in_flight.settled().await;
    let listed = mine(&app, &owner.cookie).await;
    assert_eq!(listed[0]["connection"]["account"], "@kaesoapp");
    let after = app
        .get(
            &format!("/orgs/{}/mcp-servers/{server}", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(after.body["tools"]["total"], 3, "{:?}", after.body);
    assert_eq!(after.body["status"], "ok");
}
