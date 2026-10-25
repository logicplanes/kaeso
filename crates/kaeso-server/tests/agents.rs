#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const REDIRECT: &str = "http://127.0.0.1:33418/callback";

struct Signed {
    client: String,
    access: String,
    refresh: String,
    agent: String,
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
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

async fn register(app: &App, name: &str) -> String {
    let r = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "client_name": name, "redirect_uris": [REDIRECT], "token_endpoint_auth_method": "none" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::CREATED);
    r.body["client_id"].as_str().unwrap().to_owned()
}

fn location(r: &common::Resp) -> String {
    r.headers[header::LOCATION].to_str().unwrap().to_owned()
}

async fn authorize_url(app: &App, query: &str) -> common::Resp {
    app.raw(
        Request::builder()
            .uri(format!("/oauth/authorize?{query}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn request_token(app: &App, client: &str) -> String {
    let q = form(&[
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", "http://127.0.0.1:50000/callback"),
        ("state", "st4te"),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("resource", "https://kaeso.test/mcp"),
        ("scope", "kaeso"),
    ]);
    let r = authorize_url(app, &q).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.body);
    let to = location(&r);
    to.split_once("#request=").unwrap().1.to_owned()
}

fn grant(org: &str, org_rights: &[&str], project_rights: &[&str]) -> Value {
    json!({
        "preset": "builder",
        "customized": false,
        "instance": [],
        "account": ["account.view"],
        "orgs": [{
            "org": org,
            "included": true,
            "rights": org_rights,
            "projects": { "all": true, "rights": project_rights, "selected": [] }
        }],
        "future_orgs": null
    })
}

fn builder(org: &str) -> Value {
    grant(
        org,
        &[
            "org.view",
            "members.view",
            "usage.org.view",
            "costs.org.view",
        ],
        &[
            "project.view",
            "models.view",
            "limits.view",
            "usage.view",
            "costs.view",
        ],
    )
}

async fn approve(app: &App, cookie: &str, request: &str, grant: Value) -> common::Resp {
    app.post(
        "/me/agent-requests/approve",
        json!({ "request": request, "name": "Release helper", "grant": grant }),
        Some(cookie),
    )
    .await
}

fn query_of(url: &str) -> std::collections::HashMap<String, String> {
    let u = url::Url::parse(url).unwrap();
    u.query_pairs().into_owned().collect()
}

async fn exchange(app: &App, client: &str, code: &str) -> common::Resp {
    oauth_post(
        app,
        "/oauth/token",
        form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client),
            ("redirect_uri", "http://127.0.0.1:50000/callback"),
            ("code_verifier", VERIFIER),
        ]),
    )
    .await
}

async fn sign_in(app: &App, cookie: &str, grant: Value) -> Signed {
    let client = register(app, "Claude Code").await;
    let request = request_token(app, &client).await;
    let approved = approve(app, cookie, &request, grant)
        .await
        .expect_status(StatusCode::OK);
    let to = approved.body["redirect_to"].as_str().unwrap();
    let q = query_of(to);
    assert_eq!(q["state"], "st4te");
    assert_eq!(q["iss"], "https://kaeso.test");
    let tokens = exchange(app, &client, &q["code"])
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(tokens.body["token_type"], "Bearer");
    assert_eq!(tokens.body["expires_in"], 3600);
    Signed {
        client,
        access: tokens.body["access_token"].as_str().unwrap().to_owned(),
        refresh: tokens.body["refresh_token"].as_str().unwrap().to_owned(),
        agent: approved.body["agent"].as_str().unwrap().to_owned(),
    }
}

async fn mcp(app: &App, token: &str, method: &str, params: Value) -> common::Resp {
    app.raw(
        Request::builder()
            .method(Method::POST)
            .uri("/mcp")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await
}

async fn tool(app: &App, token: &str, name: &str, args: Value) -> Value {
    let r = mcp(
        app,
        token,
        "tools/call",
        json!({ "name": name, "arguments": args }),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(r.body["id"], 7, "{:?}", r.body);
    r.body["result"].clone()
}

async fn tool_names(app: &App, token: &str) -> Vec<String> {
    let r = mcp(app, token, "tools/list", json!({}))
        .await
        .expect_status(StatusCode::OK);
    r.body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

async fn setup() -> (TestServices, App, Owner) {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    (s, app, owner)
}

#[tokio::test]
async fn an_agent_discovers_signs_in_and_calls_tools_for_its_user() {
    let (_s, app, owner) = setup().await;
    let resource = app
        .raw(
            Request::builder()
                .uri("/.well-known/oauth-protected-resource/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(resource.body["resource"], "https://kaeso.test/mcp");
    assert_eq!(
        resource.body["authorization_servers"][0],
        "https://kaeso.test"
    );
    let server = app
        .raw(
            Request::builder()
                .uri("/.well-known/oauth-authorization-server")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(server.body["issuer"], "https://kaeso.test");
    assert_eq!(
        server.body["code_challenge_methods_supported"],
        json!(["S256"])
    );
    assert_eq!(
        server.body["registration_endpoint"],
        "https://kaeso.test/oauth/register"
    );

    let unauthenticated = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    let challenge = unauthenticated.headers[header::WWW_AUTHENTICATE]
        .to_str()
        .unwrap();
    assert!(
        challenge.contains(
            "resource_metadata=\"https://kaeso.test/.well-known/oauth-protected-resource/mcp\""
        ),
        "{challenge}"
    );

    let client = register(&app, "Claude Code").await;
    let request = request_token(&app, &client).await;
    let viewed = app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(viewed.body["client"], "Claude Code");
    assert_eq!(viewed.body["redirect_host"], "127.0.0.1");

    let a = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let init = mcp(&app, &a.access, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "claude-code", "version": "2.1.0" } }))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(init.body["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init.body["result"]["serverInfo"]["name"], "kaeso");
    assert!(init.body["result"]["capabilities"]["tools"].is_object());
    let note = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header(header::AUTHORIZATION, format!("Bearer {}", a.access))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(note.status, StatusCode::ACCEPTED);

    let names = tool_names(&app, &a.access).await;
    assert!(
        names.contains(&"get_orgs_org_usage".to_owned()),
        "{names:?}"
    );
    assert!(
        names.contains(&"get_orgs_org_projects_project_models".to_owned()),
        "{names:?}"
    );
    assert!(
        !names.contains(&"post_orgs_org_projects".to_owned()),
        "not granted"
    );
    assert!(
        !names.iter().any(|n| n.starts_with("get_instance_")),
        "no instance rights"
    );
    assert!(
        !names.iter().any(|n| n.starts_with("get_me_agents")),
        "never a tool"
    );

    let org = tool(
        &app,
        &a.access,
        "get_orgs_org",
        json!({ "org": owner.org_id }),
    )
    .await;
    assert_eq!(org["isError"], false, "{org:?}");
    assert_eq!(org["structuredContent"]["name"], "Acme");

    let refused = tool(
        &app,
        &a.access,
        "post_orgs_org_projects",
        json!({ "org": owner.org_id, "body": { "name": "New" } }),
    )
    .await;
    assert_eq!(refused["isError"], true);
    assert_eq!(
        refused["structuredContent"]["error"]["type"],
        "insufficient_rights"
    );
    assert_eq!(
        refused["structuredContent"]["error"]["right"],
        "projects.create"
    );
    let approve_url = refused["structuredContent"]["error"]["approve_url"]
        .as_str()
        .unwrap();
    assert!(
        approve_url.starts_with(&format!(
            "https://kaeso.test/settings/account/agents/{}?add=projects.create&org={}",
            a.agent, owner.org_id
        )),
        "{approve_url}"
    );

    let activity = app
        .get(&format!("/me/agents/{}/activity", a.agent), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    let calls = activity.body["data"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["tool"], "post_orgs_org_projects");
    assert_eq!(calls[0]["result"], "insufficient_rights");
    assert_eq!(calls[0]["status"], 403);
    assert_eq!(calls[0]["org_id"], owner.org_id);
    assert_eq!(calls[1]["result"], "ok");
}

#[tokio::test]
async fn tokens_rotate_and_a_reused_refresh_token_ends_the_sign_in() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let refresh = |token: String| {
        let client = a.client.clone();
        let app = &app;
        async move {
            oauth_post(
                app,
                "/oauth/token",
                form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", &token),
                    ("client_id", &client),
                ]),
            )
            .await
        }
    };
    let next = refresh(a.refresh.clone())
        .await
        .expect_status(StatusCode::OK);
    let newer = next.body["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(newer, a.refresh);
    assert_eq!(next.headers[header::CACHE_CONTROL], "no-store");
    let replay = refresh(a.refresh.clone())
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(replay.body["error"], "invalid_grant");
    refresh(newer).await.expect_status(StatusCode::BAD_REQUEST);
    let still = mcp(
        &app,
        next.body["access_token"].as_str().unwrap(),
        "ping",
        json!({}),
    )
    .await;
    assert_eq!(
        still.status,
        StatusCode::UNAUTHORIZED,
        "the whole sign-in ended"
    );
}

#[tokio::test]
async fn codes_need_the_right_verifier_client_and_redirect_once() {
    let (_s, app, owner) = setup().await;
    let client = register(&app, "Codex").await;
    let request = request_token(&app, &client).await;
    let approved = approve(&app, &owner.cookie, &request, builder(&owner.org_id))
        .await
        .expect_status(StatusCode::OK);
    let code = query_of(approved.body["redirect_to"].as_str().unwrap())["code"].clone();
    let wrong_verifier = oauth_post(
        &app,
        "/oauth/token",
        form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", &client),
            ("redirect_uri", "http://127.0.0.1:50000/callback"),
            ("code_verifier", &"a".repeat(43)),
        ]),
    )
    .await
    .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(wrong_verifier.body["error"], "invalid_grant");
    let reused = exchange(&app, &client, &code)
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        reused.body["error"], "invalid_grant",
        "a code works once, even after a failure"
    );
    let again = approve(&app, &owner.cookie, &request, builder(&owner.org_id)).await;
    assert_eq!(
        again.status,
        StatusCode::NOT_FOUND,
        "a request is used once"
    );

    let other = register(&app, "Other").await;
    let request = request_token(&app, &client).await;
    let approved = approve(&app, &owner.cookie, &request, builder(&owner.org_id))
        .await
        .expect_status(StatusCode::OK);
    let code = query_of(approved.body["redirect_to"].as_str().unwrap())["code"].clone();
    let stolen = exchange(&app, &other, &code)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    assert_eq!(stolen.body["error"], "invalid_client");
}

#[tokio::test]
async fn an_agent_whose_registration_is_gone_signs_in_again_from_its_own_machine() {
    let (_s, app, owner) = setup().await;
    let gone = "cli_00000000000000000000000001";
    let request = request_token(&app, gone).await;
    let viewed = app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(viewed.body["client"], "MCP client");
    assert_eq!(viewed.body["redirect_host"], "127.0.0.1");
    let again = request_token(&app, gone).await;
    assert_ne!(again, request);
    let remote = authorize_url(
        &app,
        &form(&[
            ("client_id", "cli_00000000000000000000000002"),
            ("redirect_uri", "https://agent.example/cb"),
            ("response_type", "code"),
        ]),
    )
    .await;
    assert_eq!(
        location(&remote),
        "https://kaeso.test/authorize#error=invalid_client"
    );
}

#[tokio::test]
async fn authorize_refuses_bad_requests_without_leaking_to_strangers() {
    let (_s, app, _owner) = setup().await;
    let unknown = authorize_url(
        &app,
        "response_type=code&client_id=cli_00000000000000000000000000",
    )
    .await;
    assert_eq!(
        location(&unknown),
        "https://kaeso.test/authorize#error=invalid_client"
    );
    let client = register(&app, "Claude Code").await;
    let elsewhere = authorize_url(
        &app,
        &form(&[
            ("client_id", &client),
            ("redirect_uri", "https://evil.example/cb"),
            ("response_type", "code"),
        ]),
    )
    .await;
    assert_eq!(
        location(&elsewhere),
        "https://kaeso.test/authorize#error=invalid_redirect_uri"
    );
    for (bad, error) in [
        (
            form(&[
                ("client_id", &client),
                ("redirect_uri", REDIRECT),
                ("response_type", "token"),
            ]),
            "unsupported_response_type",
        ),
        (
            form(&[
                ("client_id", &client),
                ("redirect_uri", REDIRECT),
                ("response_type", "code"),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "plain"),
            ]),
            "invalid_request",
        ),
        (
            form(&[
                ("client_id", &client),
                ("redirect_uri", REDIRECT),
                ("response_type", "code"),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
                ("resource", "https://other.example/mcp"),
            ]),
            "invalid_target",
        ),
        (
            form(&[
                ("client_id", &client),
                ("redirect_uri", REDIRECT),
                ("response_type", "code"),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
                ("scope", "admin"),
            ]),
            "invalid_scope",
        ),
    ] {
        let r = authorize_url(&app, &bad).await;
        let q = query_of(&location(&r));
        assert_eq!(q["error"], error);
        assert!(location(&r).starts_with(REDIRECT));
    }
    let bad_redirect = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "redirect_uris": ["http://evil.example/cb"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(bad_redirect.body["error"], "invalid_redirect_uri");
}

#[tokio::test]
async fn an_agent_never_exceeds_its_user_or_the_policies() {
    let (_s, app, owner) = setup().await;
    let (member, member_id) = common::join(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let developer = grant(
        &owner.org_id,
        &["org.view", "members.view", "usage.org.view"],
        &["project.view", "models.view", "usage.view"],
    );
    let a = sign_in(&app, &member, developer).await;
    let projects = tool(
        &app,
        &a.access,
        "get_orgs_org_projects",
        json!({ "org": owner.org_id }),
    )
    .await;
    assert_eq!(projects["isError"], false, "{projects:?}");

    app.call(
        Method::PATCH,
        &format!("/orgs/{}/members/{member_id}", owner.org_id),
        Some(json!({ "role": "member" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let usage = tool(
        &app,
        &a.access,
        "get_orgs_org_usage",
        json!({ "org": owner.org_id }),
    )
    .await;
    assert_eq!(
        usage["structuredContent"]["error"]["right"], "usage.org.view",
        "the user lost it, so the agent did"
    );
    assert!(
        !tool_names(&app, &a.access)
            .await
            .contains(&"get_orgs_org_usage".to_owned())
    );

    app.call(
        Method::PUT,
        &format!("/orgs/{}/security/agents", owner.org_id),
        Some(json!({ "allowed": false, "max": "full" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let closed = tool(
        &app,
        &a.access,
        "get_orgs_org",
        json!({ "org": owner.org_id }),
    )
    .await;
    assert_eq!(closed["isError"], true);
    assert!(
        closed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("doesn't allow agents"),
        "{closed:?}"
    );

    app.call(
        Method::PUT,
        "/instance/settings/agents",
        Some(json!({ "allowed": false, "max": "configure" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let off = mcp(&app, &a.access, "ping", json!({})).await;
    assert_eq!(off.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_org_policy_caps_rights_and_only_sessions_change_policies() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(
        &app,
        &owner.cookie,
        grant(
            &owner.org_id,
            &["org.view", "members.view", "projects.create"],
            &["project.view"],
        ),
    )
    .await;
    assert!(
        tool_names(&app, &a.access)
            .await
            .contains(&"post_orgs_org_projects".to_owned())
    );
    app.call(
        Method::PUT,
        &format!("/orgs/{}/security/agents", owner.org_id),
        Some(json!({ "allowed": true, "max": "observer" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    assert!(
        !tool_names(&app, &a.access)
            .await
            .contains(&"post_orgs_org_projects".to_owned())
    );
    let policy = app
        .get(
            &format!("/orgs/{}/security/agents", owner.org_id),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(policy.body, json!({ "allowed": true, "max": "observer" }));
    let by_agent = app
        .raw(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/platform/v1/orgs/{}/security/agents",
                    owner.org_id
                ))
                .header(header::AUTHORIZATION, format!("Bearer {}", a.access))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "allowed": true, "max": "full" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(by_agent.status, StatusCode::FORBIDDEN);
    for path in ["/me/agents", "/me/sessions"] {
        let r = app
            .raw(
                Request::builder()
                    .uri(format!("/platform/v1{path}"))
                    .header(header::AUTHORIZATION, format!("Bearer {}", a.access))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(
            r.body["error"]["message"],
            "Agents can't do this; it needs a console session."
        );
    }
}

#[tokio::test]
async fn consent_refuses_what_agents_may_never_have() {
    let (_s, app, owner) = setup().await;
    let client = register(&app, "Cursor").await;
    let request = request_token(&app, &client).await;
    let sso = approve(
        &app,
        &owner.cookie,
        &request,
        grant(&owner.org_id, &["org.view", "security.sso.manage"], &[]),
    )
    .await;
    assert_eq!(sso.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        sso.body["error"]["message"],
        "`security.sso.manage` can't be given to agents."
    );
    let mut g = builder(&owner.org_id);
    g["instance"] = json!([
        "instance.updates.apply",
        "instance.updates.manage",
        "instance.system.view"
    ]);
    let apply = approve(&app, &owner.cookie, &request, g).await;
    assert_eq!(
        apply.status,
        StatusCode::BAD_REQUEST,
        "the default policy allows Configure only"
    );
    assert!(
        apply.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("instance.updates.apply")
    );

    let (member, _) = common::join(
        &app,
        &owner,
        json!({ "email": "m@example.com", "role": "member" }),
        "M",
    )
    .await;
    let mut g = builder(&owner.org_id);
    g["instance"] = json!(["instance.system.view"]);
    let not_admin = approve(&app, &member, &request, g).await;
    assert_eq!(not_admin.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        not_admin.body["error"]["message"],
        "Only instance admins can give an agent instance rights."
    );
    let beyond = approve(&app, &member, &request, builder(&owner.org_id)).await;
    assert_eq!(
        beyond.status,
        StatusCode::BAD_REQUEST,
        "a member holds no usage rights"
    );
}

#[tokio::test]
async fn sensitive_rights_need_a_recent_sign_in_and_instance_rights_reach_the_instance() {
    let (_s, app, owner) = setup().await;
    app.clock.advance(time::Duration::minutes(20));
    let client = register(&app, "Claude Code").await;
    let request = request_token(&app, &client).await;
    let mut g = builder(&owner.org_id);
    g["account"] = json!(["account.view", "account.keys.view", "account.keys.manage"]);
    let stale = approve(&app, &owner.cookie, &request, g).await;
    assert_eq!(stale.status, StatusCode::FORBIDDEN);
    assert_eq!(stale.error_type(), "recent_sign_in_required");
    let mut g = builder(&owner.org_id);
    g["instance"] = json!(["instance.system.view", "instance.audit_log.view"]);
    let a = approve(&app, &owner.cookie, &request, g)
        .await
        .expect_status(StatusCode::OK);
    let q = query_of(a.body["redirect_to"].as_str().unwrap());
    let tokens = exchange(&app, &client, &q["code"])
        .await
        .expect_status(StatusCode::OK);
    let access = tokens.body["access_token"].as_str().unwrap();
    let names = tool_names(&app, access).await;
    assert!(names.contains(&"get_instance_system".to_owned()));
    assert!(names.contains(&"get_instance_audit_log".to_owned()));
    assert!(!names.contains(&"get_instance_users".to_owned()));
    let system = tool(&app, access, "get_instance_system", json!({})).await;
    assert_eq!(system["isError"], false, "{system:?}");
    let users = tool(&app, access, "get_instance_users", json!({})).await;
    assert_eq!(
        users["structuredContent"]["error"]["right"],
        "instance.users.view"
    );
}

#[tokio::test]
async fn audit_logs_name_the_agent_and_filter_by_agents() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(
        &app,
        &owner.cookie,
        grant(
            &owner.org_id,
            &[
                "org.view",
                "members.view",
                "projects.create",
                "audit_log.view",
            ],
            &["project.view"],
        ),
    )
    .await;
    let made = tool(
        &app,
        &a.access,
        "post_orgs_org_projects",
        json!({ "org": owner.org_id, "body": { "name": "Agent work" }, "agent_model": "gpt" }),
    )
    .await;
    assert_eq!(made["isError"], false, "{made:?}");
    let log = app
        .get(
            &format!("/orgs/{}/audit-log?by_agents=true", owner.org_id),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    let events = log.body["data"].as_array().unwrap();
    assert!(!events.is_empty());
    let e = &events[0];
    assert_eq!(e["action"], "project.create");
    assert_eq!(e["actor"]["kind"], "agent");
    assert_eq!(e["actor"]["id"], owner.user_id);
    assert_eq!(e["actor"]["agent"]["id"], a.agent);
    assert_eq!(e["actor"]["agent"]["name"], "Release helper");
    assert_eq!(e["actor"]["agent"]["model"], "gpt");
    assert!(events.iter().all(|e| e["actor"]["kind"] == "agent"));
    let all = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert!(all.body["data"].as_array().unwrap().len() > events.len());
    let instance = app
        .get("/instance/audit-log?action=agent.", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(instance.body["data"][0]["action"], "agent.authorize");
    assert_eq!(instance.body["data"][0]["actor"]["kind"], "user");

    let odd = tool(
        &app,
        &a.access,
        "get_orgs_org",
        json!({ "org": owner.org_id, "agent_model": "parrot" }),
    )
    .await;
    assert_eq!(odd["isError"], false, "{odd:?}");
    let viewed = app
        .get(&format!("/me/agents/{}", a.agent), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(viewed.body["model"], "unknown");
}

#[tokio::test]
async fn the_agents_tab_lists_renames_regrants_and_revokes() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let listed = app
        .get("/me/agents", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    let agent = &listed.body["data"][0];
    assert_eq!(agent["id"], a.agent);
    assert_eq!(agent["client"], "Claude Code");
    assert_eq!(agent["model"], "unknown");
    assert_eq!(agent["grant"]["preset"], "builder");
    assert_eq!(
        agent["grant"]["orgs"][0]["rights"],
        json!([
            "org.view",
            "members.view",
            "usage.org.view",
            "costs.org.view"
        ])
    );
    assert!(agent.get("expires_at").is_none());

    let renamed = app
        .call(
            Method::PATCH,
            &format!("/me/agents/{}", a.agent),
            Some(json!({ "name": "Ops" })),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(renamed.body["name"], "Ops");
    assert_eq!(renamed.body["model"], "unknown");

    let mut more = builder(&owner.org_id);
    more["customized"] = json!(true);
    more["orgs"][0]["rights"] = json!([
        "org.view",
        "members.view",
        "usage.org.view",
        "costs.org.view",
        "projects.create"
    ]);
    let regranted = app
        .call(
            Method::PUT,
            &format!("/me/agents/{}/rights", a.agent),
            Some(more),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(regranted.body["grant"]["customized"], true);
    assert!(
        tool_names(&app, &a.access)
            .await
            .contains(&"post_orgs_org_projects".to_owned())
    );

    let org_view = app
        .get(&format!("/orgs/{}/agents", owner.org_id), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(org_view.body["data"][0]["id"], a.agent);
    assert_eq!(org_view.body["data"][0]["user"]["id"], owner.user_id);
    app.call(
        Method::DELETE,
        &format!("/orgs/{}/agents/{}", owner.org_id, a.agent),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    let closed = tool(
        &app,
        &a.access,
        "get_orgs_org",
        json!({ "org": owner.org_id }),
    )
    .await;
    assert_eq!(closed["isError"], true);
    assert_eq!(closed["structuredContent"]["error"]["type"], "not_found");

    app.call(
        Method::DELETE,
        &format!("/me/agents/{}", a.agent),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(
        mcp(&app, &a.access, "ping", json!({})).await.status,
        StatusCode::UNAUTHORIZED
    );
    let empty = app
        .get("/me/agents", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(empty.body["data"], json!([]));
}

#[tokio::test]
async fn an_agent_keeps_working_until_it_goes_unused_for_ninety_days() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let b = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let refresh = |s: &Signed| {
        form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &s.refresh),
            ("client_id", &s.client),
        ])
    };
    app.clock.advance(time::Duration::days(60));
    let used = oauth_post(&app, "/oauth/token", refresh(&a))
        .await
        .expect_status(StatusCode::OK);
    assert!(used.body["access_token"].is_string());
    app.clock.advance(time::Duration::days(31));
    let unused = oauth_post(&app, "/oauth/token", refresh(&b))
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(unused.body["error"], "invalid_grant");
    let report = kaeso_server::jobs::cleanup::run(app.state.clone())
        .await
        .unwrap();
    assert!(report.contains("revoked 1 agents"), "{report}");
}

#[tokio::test]
async fn deny_sends_the_browser_back_with_access_denied() {
    let (_s, app, owner) = setup().await;
    let client = register(&app, "OpenCode").await;
    let request = request_token(&app, &client).await;
    let denied = app
        .post(
            "/me/agent-requests/deny",
            json!({ "request": request }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let q = query_of(denied.body["redirect_to"].as_str().unwrap());
    assert_eq!(q["error"], "access_denied");
    assert_eq!(q["state"], "st4te");
    let used = app
        .post(
            "/me/agent-requests/view",
            json!({ "request": request }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(used.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_catalogue_names_instance_and_account_rights_presets_and_models() {
    let (_s, app, owner) = setup().await;
    let c = app
        .get("/me/agents/catalogue", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(c.body["presets"].as_array().unwrap().len(), 4);
    assert_eq!(c.body["presets"][1]["id"], "builder");
    assert_eq!(
        c.body["never_grantable"],
        json!([
            "org.ownership.transfer",
            "security.sso.manage",
            "security.sso.require"
        ])
    );
    assert_eq!(
        c.body["instance"]["categories"][0]["rights"][0]["id"],
        "instance.system.view"
    );
    assert!(c.body["models"].as_array().unwrap().last() == Some(&json!("unknown")));
    let g = app
        .get("/me/agents/grantable", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(g.body["allowed"], true);
    assert_eq!(g.body["instance_max"], "configure");
    assert_eq!(g.body["orgs"][0]["id"], owner.org_id);
    assert!(
        !g.body["orgs"][0]["rights"]
            .as_array()
            .unwrap()
            .contains(&json!("security.sso.manage"))
    );
    assert!(!g.body["orgs"][0]["projects"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn mcp_speaks_json_rpc_and_refuses_strangers() {
    let (_s, app, owner) = setup().await;
    let a = sign_in(&app, &owner.cookie, builder(&owner.org_id)).await;
    let unknown = mcp(&app, &a.access, "resources/list", json!({}))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(unknown.body["error"]["code"], -32601);
    let no_tool = mcp(&app, &a.access, "tools/call", json!({ "name": "nope" })).await;
    assert_eq!(no_tool.body["error"]["code"], -32602);
    let bad_args = tool(
        &app,
        &a.access,
        "get_orgs_org",
        json!({ "org": owner.org_id, "extra": 1 }),
    )
    .await;
    assert_eq!(bad_args["isError"], true);
    let get = app
        .raw(Request::builder().uri("/mcp").body(Body::empty()).unwrap())
        .await;
    assert_eq!(get.status, StatusCode::METHOD_NOT_ALLOWED);
    let foreign = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header(header::ORIGIN, "https://evil.example")
                .header(header::AUTHORIZATION, format!("Bearer {}", a.access))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(foreign.status, StatusCode::FORBIDDEN);
    let old = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("mcp-protocol-version", "2024-01-01")
                .header(header::AUTHORIZATION, format!("Bearer {}", a.access))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(old.status, StatusCode::BAD_REQUEST);
    let revoked = oauth_post(&app, "/oauth/revoke", form(&[("token", &a.refresh)])).await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert_eq!(
        mcp(&app, &a.access, "ping", json!({})).await.status,
        StatusCode::UNAUTHORIZED
    );
}
