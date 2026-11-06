#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_server::mcp::cimd::{self, Described};
use kaeso_testkit::mcp::{MockMcp, Options};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const CLAUDE: &str = "https://claude.example/oauth/mcp-client-metadata.json";
const CALLBACK: &str = "https://claude.example/api/mcp/auth_callback";

struct Env {
    app: App,
    owner: Owner,
    project: String,
    other: String,
}

async fn env(s: &TestServices, mock: &MockMcp) -> Env {
    let app = App::start_with(s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let other = common::create_project(&app, &owner.org_id, &owner.cookie, "Other").await;
    let created = app
        .post(
            &format!("/orgs/{}/mcp-servers", owner.org_id),
            json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project, other] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
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
    cimd::remember(
        &app.state,
        CLAUDE,
        Described {
            name: "Claude".into(),
            redirect_uris: vec![CALLBACK.into()],
            software_id: None,
            software_version: None,
        },
    )
    .await
    .unwrap();
    Env {
        app,
        owner,
        project,
        other,
    }
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
}

fn resource(project: &str) -> String {
    format!("https://kaeso.test/mcp/{project}")
}

async fn authorize(app: &App, client: &str, target: &str, scope: &str) -> common::Resp {
    let q = form(&[
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", CALLBACK),
        ("state", "st4te"),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("resource", target),
        ("scope", scope),
    ]);
    app.raw(
        Request::builder()
            .uri(format!("/oauth/authorize?{q}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

fn request_of(r: &common::Resp) -> String {
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.body);
    let to = r.headers[header::LOCATION].to_str().unwrap();
    to.split_once("#request=").map_or(to, |(_, r)| r).to_owned()
}

fn query_of(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

async fn oauth_post(app: &App, path: &str, body: String) -> common::Resp {
    app.raw(
        Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
}

async fn exchange(app: &App, code: &str, target: &str) -> common::Resp {
    oauth_post(
        app,
        "/oauth/token",
        form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", CLAUDE),
            ("redirect_uri", CALLBACK),
            ("code_verifier", VERIFIER),
            ("resource", target),
        ]),
    )
    .await
}

async fn mcp(app: &App, path: &str, token: &str) -> common::Resp {
    app.raw(
        Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await
}

async fn signed_in(e: &Env) -> (String, String, String) {
    let request = request_of(&authorize(&e.app, CLAUDE, &resource(&e.project), "tools").await);
    let approved = e
        .app
        .post(
            "/me/agent-requests/approve-portal",
            json!({ "request": request }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(approved.status, StatusCode::OK, "{:?}", approved.body);
    assert_eq!(approved.body["agent"], Value::Null);
    let q = query_of(approved.body["redirect_to"].as_str().unwrap());
    assert_eq!(
        (q["state"].as_str(), q["iss"].as_str()),
        ("st4te", "https://kaeso.test")
    );
    let tokens = exchange(&e.app, &q["code"], &resource(&e.project)).await;
    assert_eq!(tokens.status, StatusCode::OK, "{:?}", tokens.body);
    assert_eq!(tokens.body["scope"], "tools");
    (
        tokens.body["access_token"].as_str().unwrap().to_owned(),
        tokens.body["refresh_token"].as_str().unwrap().to_owned(),
        q["code"].clone(),
    )
}

#[tokio::test]
async fn kaeso_advertises_metadata_documents_and_each_portal_as_its_own_resource() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let get =
        |path: &'static str| app.raw(Request::builder().uri(path).body(Body::empty()).unwrap());
    let server = get("/.well-known/oauth-authorization-server").await;
    assert_eq!(server.body["client_id_metadata_document_supported"], true);
    assert_eq!(server.body["scopes_supported"], json!(["kaeso", "tools"]));
    let portal =
        get("/.well-known/oauth-protected-resource/mcp/proj_01m6pxfa7peycr8zt6sae4mpcd").await;
    assert_eq!(
        portal.body["resource"],
        "https://kaeso.test/mcp/proj_01m6pxfa7peycr8zt6sae4mpcd"
    );
    assert_eq!(
        portal.body["authorization_servers"],
        json!(["https://kaeso.test"])
    );
    assert_eq!(portal.body["scopes_supported"], json!(["tools"]));
    let bad = get("/.well-known/oauth-protected-resource/mcp/not-a-project").await;
    assert_eq!(bad.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_client_with_a_metadata_document_signs_in_to_one_portal_after_consent() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;

    let github = e
        .app
        .post(
            &format!("/orgs/{}/mcp-servers", e.owner.org_id),
            json!({ "handle": "gh", "name": "GitHub", "url": mock.url(), "auth_method": "header", "credential_mode": "per_user", "projects": [e.project] }),
            Some(&e.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .clone();
    let request = request_of(&authorize(&e.app, CLAUDE, &resource(&e.project), "tools").await);
    let view = e
        .app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(view.status, StatusCode::OK, "{:?}", view.body);
    assert_eq!(view.body["client"], "Claude");
    assert_eq!(view.body["redirect_host"], "claude.example");
    let portal = &view.body["portal"];
    assert_eq!(portal["project"], e.project);
    assert_eq!(portal["project_name"], "Checkout");
    let servers = portal["servers"]
        .as_array()
        .unwrap_or_else(|| panic!("{portal:?}"));
    assert_eq!(servers.len(), 1, "{servers:?}");
    assert_eq!(servers[0]["handle"], "x");
    assert_eq!(servers[0]["name"], "X");
    assert_eq!(servers[0]["tools"], json!(["x__create_post"]));
    assert!(servers[0]["server"].as_str().unwrap().starts_with("mcp_"));
    assert_eq!(portal["client_host"], "claude.example");
    assert_eq!(portal["local"], false);
    assert_eq!(
        portal["connect"],
        json!([{ "server": github, "name": "GitHub", "auth_method": "header" }]),
        "a per-person server the person hasn't connected yet"
    );
    let r = e
        .app
        .post(
            "/me/agent-requests/approve",
            json!({ "request": request, "name": "Claude", "grant": { "preset": "observer", "customized": false, "instance": [], "account": [], "orgs": [], "future_orgs": null } }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This sign-in is for a project's MCP portal; reload the page."
    );

    let (access, refresh, code) = signed_in(&e).await;
    assert!(access.starts_with("kso_pa_"));
    let list = mcp(&e.app, &format!("/mcp/{}", e.project), &access).await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.body);
    assert_eq!(list.body["result"]["tools"][0]["name"], "x__create_post");
    let renewed = oauth_post(
        &e.app,
        "/oauth/token",
        form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", CLAUDE),
        ]),
    )
    .await;
    assert_eq!(renewed.status, StatusCode::OK, "{:?}", renewed.body);
    let fresh = renewed.body["access_token"].as_str().unwrap();
    assert_eq!(
        mcp(&e.app, &format!("/mcp/{}", e.project), fresh)
            .await
            .status,
        StatusCode::OK
    );
    let again = exchange(&e.app, &code, &resource(&e.project)).await;
    assert_eq!(again.body["error"], "invalid_grant", "a code works once");
    let reused = oauth_post(
        &e.app,
        "/oauth/token",
        form(&[
            ("grant_type", "refresh_token"),
            (
                "refresh_token",
                renewed.body["refresh_token"].as_str().unwrap(),
            ),
            ("client_id", CLAUDE),
        ]),
    )
    .await;
    assert_eq!(
        reused.body["error"], "invalid_grant",
        "a replayed code ends the whole sign-in"
    );

    let grants = e
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/portal-grants",
                e.owner.org_id, e.project
            ),
            &e.owner.cookie,
        )
        .await;
    assert_eq!(grants.body["data"][0]["client_name"], "Claude");
    let audit = e
        .app
        .get(
            &format!("/orgs/{}/audit-log?limit=50", e.owner.org_id),
            &e.owner.cookie,
        )
        .await;
    assert!(
        audit.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|ev| ev["action"] == "portal_grant.create")
    );
}

#[tokio::test]
async fn a_token_works_only_where_it_was_issued() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;
    let (portal_token, _, _) = signed_in(&e).await;

    let r = mcp(&e.app, &format!("/mcp/{}", e.other), &portal_token).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "a portal token on another portal"
    );
    assert_eq!(
        r.body["error"]["message"],
        "This sign-in is for another resource; sign in to this project's portal."
    );
    let r = mcp(&e.app, "/mcp", &portal_token).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "a portal token on Kaeso's own MCP server"
    );

    let registered = e
        .app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "client_name": "Claude Code", "redirect_uris": ["http://127.0.0.1:33418/callback"], "token_endpoint_auth_method": "none" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    let client = registered.body["client_id"].as_str().unwrap().to_owned();
    let q = form(&[
        ("response_type", "code"),
        ("client_id", &client),
        ("redirect_uri", "http://127.0.0.1:33418/callback"),
        ("state", "s"),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("resource", "https://kaeso.test/mcp"),
    ]);
    let request = request_of(
        &e.app
            .raw(
                Request::builder()
                    .uri(format!("/oauth/authorize?{q}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await,
    );
    let view = e
        .app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(view.body["portal"], Value::Null);
    let r = e
        .app
        .post(
            "/me/agent-requests/approve-portal",
            json!({ "request": request }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let approved = e
        .app
        .post(
            "/me/agent-requests/approve",
            json!({ "request": request, "name": "Claude Code", "grant": {
                "preset": "observer", "customized": false, "instance": [], "account": ["account.view"],
                "orgs": [{ "org": e.owner.org_id, "included": true, "rights": ["org.view"],
                    "projects": { "all": true, "rights": ["project.view"], "selected": [] } }],
                "future_orgs": null } }),
            Some(&e.owner.cookie),
        )
        .await;
    assert_eq!(approved.status, StatusCode::OK, "{:?}", approved.body);
    let code = query_of(approved.body["redirect_to"].as_str().unwrap())["code"].clone();
    let tokens = oauth_post(
        &e.app,
        "/oauth/token",
        form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", &client),
            ("redirect_uri", "http://127.0.0.1:33418/callback"),
            ("code_verifier", VERIFIER),
        ]),
    )
    .await;
    let agent_token = tokens.body["access_token"].as_str().unwrap();
    let r = mcp(&e.app, &format!("/mcp/{}", e.project), agent_token).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "an agent's token on a portal"
    );

    let r = authorize(&e.app, CLAUDE, &resource(&e.project), "kaeso").await;
    assert!(
        r.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("error=invalid_scope")
            || r.headers[header::LOCATION]
                .to_str()
                .unwrap()
                .contains("#error=invalid_scope"),
        "{:?}",
        r.headers
    );
    let r = authorize(&e.app, CLAUDE, "https://kaeso.test/mcp/nothing", "tools").await;
    assert!(
        r.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("invalid_target")
    );
}

#[tokio::test]
async fn disconnecting_a_client_ends_its_tokens_at_once() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;
    let (access, refresh, _) = signed_in(&e).await;
    let portal = format!("/mcp/{}", e.project);
    assert_eq!(mcp(&e.app, &portal, &access).await.status, StatusCode::OK);

    let grants = format!(
        "/orgs/{}/projects/{}/portal-grants",
        e.owner.org_id, e.project
    );
    let id = e.app.get(&grants, &e.owner.cookie).await.body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    e.app
        .call(
            Method::DELETE,
            &format!("{grants}/{id}"),
            None,
            Some(&e.owner.cookie),
        )
        .await
        .expect_status(StatusCode::NO_CONTENT);
    let r = mcp(&e.app, &portal, &access).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This sign-in has ended or was revoked; sign in to the portal again."
    );
    let renewed = oauth_post(
        &e.app,
        "/oauth/token",
        form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", CLAUDE),
        ]),
    )
    .await;
    assert_eq!(renewed.body["error"], "invalid_grant");

    let (access, _, _) = signed_in(&e).await;
    let revoked = oauth_post(&e.app, "/oauth/revoke", form(&[("token", &access)])).await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert_eq!(
        mcp(&e.app, &portal, &access).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_person_without_tool_access_cannot_connect_a_client() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let e = env(&s, &mock).await;
    let (outsider, _) = common::join(
        &e.app,
        &e.owner,
        json!({ "email": "outsider@example.com", "role": "member" }),
        "Outsider",
    )
    .await;
    let request = request_of(&authorize(&e.app, CLAUDE, &resource(&e.project), "tools").await);
    let r = e
        .app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&outsider),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "You need the right `tools.use` in this project; ask one of its admins for it."
    );
}
