#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::App;
use kaeso_core::clock::Clock;
use kaeso_core::id::OrgId;
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
