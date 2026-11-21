#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, ORIGIN, Owner};
use kaeso_testkit::mcp::{MockMcp, OAuth, Options};
use kaeso_testkit::oidc::MockIdp;
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

#[tokio::test]
async fn a_person_is_never_sent_to_a_sign_in_address_that_isnt_a_web_address() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    for (n, address) in [
        "javascript:void(0)//",
        "data:text/html,Kaeso",
        "kaeso-app://sign-in",
    ]
    .into_iter()
    .enumerate()
    {
        let mock = MockMcp::start(Options {
            oauth: Some(OAuth {
                metadata: vec![("authorization_endpoint", address.to_owned())],
                ..OAuth::default()
            }),
            ..Options::default()
        })
        .await;
        let server = add(
            &app,
            &owner,
            json!({ "handle": format!("x{n}"), "name": "X", "url": mock.url(), "auth_method": "oauth", "credential_mode": "per_user" }),
        )
        .await;
        let r = app
            .post(
                &format!("{MINE}/{server}"),
                json!({ "oauth": true }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{address}: {:?}", r.body);
        assert_eq!(
            r.body["error"]["message"],
            "The server's sign-in names an address Kaeso can't use; check the server.",
            "{address}"
        );
        assert_eq!(r.body["redirect_url"], Value::Null);
    }
}

async fn add_in(app: &App, org: &str, cookie: &str, handle: &str, url: &str) -> String {
    let r = app
        .post(
            &format!("/orgs/{org}/mcp-servers"),
            json!({ "handle": handle, "name": handle, "url": url, "auth_method": "header", "credential_mode": "per_user" }),
            Some(cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

async fn connect_header(app: &App, cookie: &str, server: &str) -> common::Resp {
    app.post(
        &format!("{MINE}/{server}"),
        json!({ "header": { "name": "Authorization", "value": "Bearer person-token" } }),
        Some(cookie),
    )
    .await
}

async fn mcp_actions(app: &App, org: &str, cookie: &str) -> Vec<String> {
    let r = app
        .get(&format!("/orgs/{org}/audit-log?limit=100"), cookie)
        .await
        .expect_status(StatusCode::OK);
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap().to_owned())
        .filter(|a| a.starts_with("mcp_server."))
        .collect()
}

struct Org<'a> {
    id: &'a str,
    connected: String,
    fresh: String,
}

async fn org_with_servers<'a>(app: &App, id: &'a str, cookie: &str, mock: &MockMcp) -> Org<'a> {
    let connected = add_in(app, id, cookie, "x", &mock.url()).await;
    let fresh = add_in(app, id, cookie, "y", &mock.url()).await;
    connect_header(app, cookie, &connected)
        .await
        .expect_status(StatusCode::OK);
    Org {
        id,
        connected,
        fresh,
    }
}

async fn refuses_changes(app: &App, org: &Org<'_>, cookie: &str, refusal: impl Fn(&common::Resp)) {
    refusal(&connect_header(app, cookie, &org.fresh).await);
    refusal(&connect_header(app, cookie, &org.connected).await);
    refusal(
        &app.call(
            Method::DELETE,
            &format!("{MINE}/{}", org.connected),
            None,
            Some(cookie),
        )
        .await,
    );
}

async fn left_alone(app: &App, org: &Org<'_>, reader: &str) {
    let listed = mine(app, reader).await;
    let connection = |server: &str| {
        listed
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["server"] == server)
            .unwrap()["connection"]
            .clone()
    };
    assert_eq!(connection(&org.fresh), Value::Null, "{listed:?}");
    assert_ne!(connection(&org.connected), Value::Null, "{listed:?}");
    let tools = app
        .get(
            &format!("/orgs/{}/mcp-servers/{}/tools", org.id, org.fresh),
            reader,
        )
        .await;
    assert_eq!(
        tools.body["data"],
        json!([]),
        "nobody connected, so the server lists no tools"
    );
    assert_eq!(
        mcp_actions(app, org.id, reader).await,
        [
            "mcp_server.connect",
            "mcp_server.create",
            "mcp_server.create"
        ]
    );
}

fn listed_servers(listed: &Value) -> Vec<String> {
    listed
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["server"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_session_without_the_second_factor_an_org_requires_leaves_its_connections_alone() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("person-token".into()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let org = org_with_servers(&app, &owner.org_id, &owner.cookie, &mock).await;
    let require_2fa = |on: bool| {
        sqlx::query("UPDATE organizations SET require_2fa = $1 WHERE id = $2")
            .bind(on)
            .bind(common::uuid(&owner.org_id))
            .execute(&app.state.pg)
    };
    require_2fa(true).await.unwrap();

    assert_eq!(
        mine(&app, &owner.cookie).await,
        json!([]),
        "the org's servers stay hidden from a session the org refuses"
    );
    refuses_changes(&app, &org, &owner.cookie, |r| {
        assert_eq!(
            (r.status, r.body["error"]["message"].as_str()),
            (
                StatusCode::FORBIDDEN,
                Some(
                    "This organization requires two-factor authentication; set it up, then sign in again."
                )
            ),
            "{:?}",
            r.body
        );
    })
    .await;

    require_2fa(false).await.unwrap();
    left_alone(&app, &org, &owner.cookie).await;
    s.cleanup().await;
}

#[tokio::test]
async fn another_orgs_sso_session_leaves_the_connections_of_an_org_it_may_not_open_alone() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("person-token".into()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    crate::oidc::connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    crate::oidc::link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    let acme = add_in(&app, &owner.org_id, &owner.cookie, "acme", &mock.url()).await;
    let beta = app
        .post("/me/orgs", json!({ "name": "Beta" }), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::CREATED);
    let beta = beta.body["id"].as_str().unwrap().to_owned();
    let org = org_with_servers(&app, &beta, &owner.cookie, &mock).await;
    let sso = crate::oidc::sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();

    assert_eq!(
        listed_servers(&mine(&app, &sso).await),
        [acme.as_str()],
        "only the servers of the org whose SSO signed the session in are listed"
    );
    refuses_changes(&app, &org, &sso, |r| {
        assert_eq!(r.error_type(), "other_sign_in_required", "{:?}", r.body);
    })
    .await;
    connect_header(&app, &sso, &acme)
        .await
        .expect_status(StatusCode::OK);

    left_alone(&app, &org, &owner.cookie).await;
    s.cleanup().await;
}

#[tokio::test]
async fn connections_in_an_org_scheduled_for_deletion_stay_readable_and_unchanged() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("person-token".into()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let beta = app
        .post("/me/orgs", json!({ "name": "Beta" }), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::CREATED);
    let beta = beta.body["id"].as_str().unwrap().to_owned();
    let org = org_with_servers(&app, &beta, &owner.cookie, &mock).await;
    app.call(
        Method::DELETE,
        &format!("/orgs/{beta}"),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let mut listed = listed_servers(&mine(&app, &owner.cookie).await);
    listed.sort();
    let mut expected = [org.connected.clone(), org.fresh.clone()];
    expected.sort();
    assert_eq!(listed, expected, "a scheduled org stays readable");
    refuses_changes(&app, &org, &owner.cookie, |r| {
        assert_eq!(
            (r.status, r.error_type()),
            (StatusCode::CONFLICT, "org_scheduled_for_deletion"),
            "{:?}",
            r.body
        );
    })
    .await;

    left_alone(&app, &org, &owner.cookie).await;
    s.cleanup().await;
}

#[tokio::test]
async fn an_oauth_sign_in_finishes_only_while_the_session_may_still_change_the_org() {
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
            json!({ "oauth": true }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let back = Url::parse(&mock.authorize(r.body["redirect_url"].as_str().unwrap())).unwrap();
    let require_2fa = |on: bool| {
        sqlx::query("UPDATE organizations SET require_2fa = $1 WHERE id = $2")
            .bind(on)
            .bind(common::uuid(&owner.org_id))
            .execute(&app.state.pg)
    };
    require_2fa(true).await.unwrap();

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
        format!("{ORIGIN}/settings/account/connections?server={server}&result=failed").as_str()
    );
    app.state.in_flight.settled().await;

    require_2fa(false).await.unwrap();
    assert_eq!(
        mine(&app, &owner.cookie).await[0]["connection"],
        Value::Null
    );
    assert_eq!(
        mcp_actions(&app, &owner.org_id, &owner.cookie).await,
        ["mcp_server.create"]
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_oauth_sign_in_doesnt_finish_once_the_org_is_scheduled_for_deletion() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: true,
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
            json!({ "oauth": true }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let back = Url::parse(&mock.authorize(r.body["redirect_url"].as_str().unwrap())).unwrap();
    app.call(
        Method::DELETE,
        &format!("/orgs/{}", owner.org_id),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

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
        format!("{ORIGIN}/settings/account/connections?server={server}&result=failed").as_str()
    );
    app.state.in_flight.settled().await;
    assert_eq!(
        mine(&app, &owner.cookie).await[0]["connection"],
        Value::Null
    );
    assert!(mock.token_requests().is_empty());
    assert_eq!(
        mcp_actions(&app, &owner.org_id, &owner.cookie).await,
        ["mcp_server.create"]
    );
    s.cleanup().await;
}
