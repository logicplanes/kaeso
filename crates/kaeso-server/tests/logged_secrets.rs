#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::App;
use kaeso_core::clock::Clock;
use kaeso_core::id::OrgId;
use kaeso_testkit::grok::{Fault as GrokFault, MockGrok};
use kaeso_testkit::oauth::{Fault, MockOAuth, Reply};
use kaeso_testkit::oidc::MockIdp;
use kaeso_testkit::services::TestServices;
use secrecy::ExposeSecret;
use serde_json::json;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Captured {
        self.clone()
    }
}

async fn connect(app: &App, org: &str, idp: &MockIdp) {
    let org: OrgId = org.parse().unwrap();
    let sealed = kaeso_crypto::seal(
        &app.state.keyring,
        b"client-secret",
        &kaeso_server::platform::auth::oidc::secret_aad(org),
    )
    .unwrap();
    let mut conn = app.state.pg.acquire().await.unwrap();
    kaeso_store::pg::oidc::upsert(
        &mut conn,
        &kaeso_store::pg::oidc::Connection {
            org_id: org.uuid(),
            issuer: idp.issuer().to_owned(),
            client_id: "kaeso".into(),
            secret_ciphertext: sealed.ciphertext,
            secret_nonce: sealed.nonce,
            wrapped_key: sealed.wrapped_key,
            key_id: sealed.key_id,
            domains: vec!["example.com".into()],
            verified_domains: Vec::new(),
            verification_token: "token".into(),
            display_name: None,
            auto_redirect: false,
            require_email_verified: true,
            confirms_sign_in: None,
        },
    )
    .await
    .unwrap();
    kaeso_store::pg::oidc::mark_verified(&mut *conn, org.uuid(), "example.com", app.clock.now())
        .await
        .unwrap();
}

async fn callback(app: &App, code: &str, state: &str, binding: &str) -> common::Resp {
    app.raw(
        Request::get(format!(
            "/platform/v1/auth/oidc/callback?code={code}&state={state}"
        ))
        .header(header::COOKIE, format!("__Host-kaeso_oidc={binding}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
}

#[tokio::test]
async fn setup_links_and_sign_in_secrets_never_reach_the_log() {
    let logs = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let mut secrets = Vec::new();

    kaeso_server::setup::announce(&app.state).await;
    let pending = logs.text();
    assert!(
        pending.contains(
            "Kaeso is not set up yet; run `kaeso setup-link` where the gateway runs to print the one-time setup link."
        ),
        "{pending}"
    );
    assert!(!pending.contains("#token="), "{pending}");

    let url = kaeso_server::setup::issue_link(&app.state)
        .await
        .unwrap()
        .unwrap();
    let token = url.split("#token=").nth(1).unwrap().to_owned();
    secrets.push(token.clone());
    let r = app
        .post(
            "/auth/setup",
            json!({ "token": token, "email": "owner@example.com", "name": "Owner", "password": common::PASSWORD, "org_name": "Acme" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    secrets.push(r.cookie().unwrap());
    let org = app.get("/auth/session", &r.cookie().unwrap()).await.body["memberships"][0]["org_id"]
        .as_str()
        .unwrap()
        .to_owned();
    kaeso_server::setup::announce(&app.state).await;
    assert_eq!(
        logs.text().matches("Kaeso is not set up yet").count(),
        1,
        "a set-up install announces nothing"
    );

    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &org, &idp).await;
    let start = app
        .post(
            "/auth/oidc/start",
            json!({ "email": "ada@example.com" }),
            None,
        )
        .await;
    let binding = start.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize(
        start.body["authorization_url"].as_str().unwrap(),
        "ada@example.com",
        "Ada",
    );
    secrets.extend([binding.clone(), auth.code.clone(), auth.state.clone()]);
    let refused = callback(&app, &auth.code, &auth.state, "another-browser").await;
    assert!(
        refused.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("sso_error=")
    );
    let failed = callback(&app, "not-a-code", &auth.state, &binding).await;
    assert_eq!(failed.status, StatusCode::SEE_OTHER);
    assert!(logs.text().contains("SSO sign-in failed"));

    let kaeso_server::services::users::OperatorSignIn::Link(link) =
        kaeso_server::services::users::operator_sign_in_link(&app.state, "owner@example.com")
            .await
            .unwrap()
    else {
        panic!("no sign-in link");
    };
    secrets.push(
        link.expose_secret()
            .split("#token=")
            .nth(1)
            .unwrap()
            .to_owned(),
    );

    let logged = logs.text();
    assert!(!logged.is_empty());
    for secret in &secrets {
        assert!(!logged.contains(secret.as_str()), "{secret} is in the log");
    }
    s.cleanup().await;
}

#[tokio::test]
async fn account_tokens_never_reach_the_log_the_audit_trail_or_clickhouse() {
    let logs = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    app.state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&app.state.pg)
        .await
        .unwrap();
    let owner = app.setup().await;
    let mut secrets = Vec::new();

    let (attempt, url) = common::begin_sign_in(&app, &owner, "Plan").await;
    let (verifier,): (String,) = sqlx::query_as(
        "SELECT data->>'pkce_verifier' FROM sign_in_tokens WHERE kind = 'provider_oauth'",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    let back = oauth.approve(&url, "sub-ada", "ada@example.com");
    let code = url::Url::parse(&back)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    secrets.extend([verifier, code, back.clone()]);
    let refused =
        common::finish_sign_in(&app, &owner, &attempt, &format!("{back}&code=again")).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    let created = common::finish_sign_in(&app, &owner, &attempt, &back)
        .await
        .expect_status(StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap().to_owned();

    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    app.post(
        &format!("{base}/models"),
        json!({ "provider": "openai", "model_id": "gpt-4o" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
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
    let ask = |key: String| {
        let app = &app;
        async move {
            common::v1::call_app(
                app,
                "/v1/chat/completions",
                Some(&key),
                false,
                Some(common::v1::chat("openai/gpt-4o", "Hi")),
            )
            .await
        }
    };
    assert_eq!(ask(key.clone()).await.status, StatusCode::OK);
    oauth.reply_with(Reply::UsageLimit);
    assert_eq!(ask(key.clone()).await.status, StatusCode::TOO_MANY_REQUESTS);
    oauth.end_sessions();
    assert_eq!(ask(key.clone()).await.status, StatusCode::BAD_GATEWAY);
    app.clock.advance(time::Duration::minutes(56));
    oauth.set_fault(Fault::SignedOut);
    assert_eq!(ask(key.clone()).await.status, StatusCode::BAD_GATEWAY);
    oauth.set_fault(Fault::None);
    app.call(
        Method::DELETE,
        &format!("/orgs/{}/credentials/{id}", owner.org_id),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(oauth.revoked().len(), 1);

    app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&app.state, std::time::Duration::from_secs(10)).await;
    let listed = app.get(&format!("{base}/logs"), &owner.cookie).await;
    assert_eq!(
        listed.body["data"].as_array().unwrap().len(),
        4,
        "{:?}",
        listed.body
    );
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    secrets.extend(oauth.issued());
    assert!(secrets.len() >= 6);
    let logged = logs.text();
    assert!(logged.contains("the provider ended an account's session"));
    for secret in &secrets {
        assert!(!logged.contains(secret.as_str()), "{secret} is in the log");
        for body in [&created.body, &listed.body, &audit.body] {
            assert!(
                !body.to_string().contains(secret.as_str()),
                "{secret} in {body}"
            );
        }
    }
    s.cleanup().await;
}

#[tokio::test]
async fn grok_device_codes_and_tokens_never_reach_the_log_the_audit_trail_or_clickhouse() {
    let logs = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let s = TestServices::start().await;
    let grok = MockGrok::start().await;
    let app = App::start_with_grok(&s, &grok).await;
    app.state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&app.state.pg)
        .await
        .unwrap();
    let owner = app.setup().await;

    let declined = common::begin_grok(&app, &owner, "Declined").await;
    grok.deny(&declined.user_code);
    app.clock.advance(time::Duration::seconds(5));
    common::poll_grok(&app, &owner, &declined.attempt)
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    let mut device_codes = grok.device_codes();
    let started = common::begin_grok(&app, &owner, "Grok").await;
    device_codes.extend(grok.device_codes());
    grok.approve(&started.user_code, "sub-ada", "ada@example.com");
    app.clock.advance(time::Duration::seconds(5));
    let created = common::poll_grok(&app, &owner, &started.attempt)
        .await
        .expect_status(StatusCode::CREATED)
        .body;
    let id = created["id"].as_str().unwrap().to_owned();

    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    app.post(
        &format!("{base}/models"),
        json!({ "provider": "xai", "model_id": "grok-4.3" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
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
    let ask = || {
        let (app, key) = (&app, key.clone());
        async move {
            common::v1::call_app(
                app,
                "/v1/chat/completions",
                Some(&key),
                false,
                Some(common::v1::chat("xai/grok-4.3", "Hi")),
            )
            .await
        }
    };
    assert_eq!(ask().await.status, StatusCode::OK);
    app.clock.advance(time::Duration::minutes(29));
    assert_eq!(ask().await.status, StatusCode::OK);
    assert_eq!(grok.refreshes(), 1);
    grok.end_sessions();
    assert_ne!(ask().await.status, StatusCode::OK);
    app.clock.advance(time::Duration::minutes(29));
    grok.set_fault(GrokFault::NoApiAccess);
    assert_eq!(ask().await.status, StatusCode::BAD_GATEWAY);
    grok.set_fault(GrokFault::None);
    app.call(
        Method::DELETE,
        &format!("/orgs/{}/credentials/{id}", owner.org_id),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(grok.revoked().len(), 1);

    app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&app.state, std::time::Duration::from_secs(10)).await;
    let listed = app.get(&format!("{base}/logs"), &owner.cookie).await;
    assert_eq!(
        listed.body["data"].as_array().unwrap().len(),
        4,
        "{:?}",
        listed.body
    );
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    let mut secrets = grok.issued();
    secrets.extend(device_codes);
    assert!(secrets.len() >= 8, "{}", secrets.len());
    let logged = logs.text();
    assert!(logged.contains("the provider refused an account's plan"));
    for secret in &secrets {
        assert!(!logged.contains(secret.as_str()), "{secret} is in the log");
        for body in [&created, &listed.body, &audit.body] {
            assert!(
                !body.to_string().contains(secret.as_str()),
                "{secret} in {body}"
            );
        }
    }
    s.cleanup().await;
}

#[tokio::test]
async fn mcp_sign_in_secrets_and_tokens_never_reach_the_log() {
    use kaeso_server::services::mcp_connections;
    use kaeso_store::pg::mcp_servers::{self, NewServer};
    use kaeso_testkit::mcp::{MockMcp, OAuth, Options};

    let logs = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: false,
            confidential: true,
            expires_in: 120,
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8")]).await;
    let owner = app.setup().await;
    let org = common::uuid(&owner.org_id);
    let add = |handle: &'static str, auth: &'static str| NewServer {
        id: uuid::Uuid::now_v7(),
        org_id: org,
        handle,
        name: "X",
        url: "",
        auth_method: auth,
        credential_mode: "shared",
        oauth_registration: (auth == "oauth").then_some("cimd"),
        scopes: &[],
        change_policy: "hold",
        created_by_kind: "system",
        created_by_id: None,
    };
    let url = mock.url();
    let oauth = NewServer {
        url: &url,
        ..add("x", "oauth")
    };
    mcp_servers::insert(&app.state.pg, &oauth).await.unwrap();
    let row = mcp_servers::get(&app.state.pg, org, oauth.id)
        .await
        .unwrap()
        .unwrap();

    let begun = mcp_connections::begin(&app.state, &row, None, common::uuid(&owner.user_id), None)
        .await
        .unwrap();
    let (verifier,): (String,) =
        sqlx::query_as("SELECT data->>'verifier' FROM sign_in_tokens WHERE kind = 'mcp_oauth'")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    let back = mock.authorize(&begun.url);
    let back_url = url::Url::parse(&back).unwrap();
    let code = back_url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let state = back_url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let r = app
        .raw(
            Request::builder()
                .uri(format!("{}?{}", back_url.path(), back_url.query().unwrap()))
                .header(
                    header::COOKIE,
                    format!("__Host-kaeso_session={}", owner.cookie),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let row = mcp_servers::get(&app.state.pg, org, oauth.id)
        .await
        .unwrap()
        .unwrap();
    app.clock.advance(time::Duration::seconds(61));
    mcp_connections::credential(&app.state, &row, None)
        .await
        .unwrap();
    mock.revoke_refresh_tokens();
    app.clock.advance(time::Duration::seconds(121));
    assert!(
        mcp_connections::credential(&app.state, &row, None)
            .await
            .is_err()
    );

    let header = add("h", "header");
    let header = NewServer {
        url: &url,
        ..header
    };
    mcp_servers::insert(&app.state.pg, &header).await.unwrap();
    let header_row = mcp_servers::get(&app.state.pg, org, header.id)
        .await
        .unwrap()
        .unwrap();
    let header_secret = "hdr-secret-value-7781";
    mcp_connections::set_header(
        &app.state,
        &header_row,
        None,
        "X-Api-Key",
        &secrecy::SecretString::from(header_secret),
    )
    .await
    .unwrap();
    mcp_connections::credential(&app.state, &header_row, None)
        .await
        .unwrap();

    let client = mock.registered_clients().pop().unwrap();
    let secrets = [
        verifier,
        code,
        state,
        format!("secret-{client}"),
        "at-1".to_owned(),
        "at-2".to_owned(),
        "rt-1".to_owned(),
        "rt-2".to_owned(),
        header_secret.to_owned(),
    ];
    let logged = logs.text();
    assert!(logged.contains("an MCP server ended a connection's sign-in"));
    for secret in &secrets {
        assert!(
            !logged.contains(&format!("\"{secret}\"")) && !logged.contains(&format!("={secret}")),
            "{secret} is in the log"
        );
    }
    s.cleanup().await;
}
