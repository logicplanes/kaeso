#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, PASSWORD, totp_code as code};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn turn_on_totp(app: &App, cookie: &str) -> (String, String) {
    let begin = app
        .post("/me/totp", json!({}), Some(cookie))
        .await
        .expect_status(StatusCode::OK);
    let secret = begin.body["secret"].as_str().unwrap().to_owned();
    let confirm = app
        .post(
            "/me/totp/confirm",
            json!({ "code": code(&secret, app) }),
            Some(cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    (confirm.cookie().unwrap(), secret)
}

async fn next_step(app: &App) {
    app.clock.advance(time::Duration::seconds(30));
}

async fn magic_link(app: &App, email: &str) -> String {
    let before = app.mail.to(email).len();
    app.post("/auth/magic-link", json!({ "email": email }), None)
        .await
        .expect_status(StatusCode::OK);
    app.wait_for_mail(email, before + 1).await;
    app.last_token(email)
}

fn from(ip: &str, path: &str, body: Value) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::post(format!("/platform/v1{path}"))
        .header(axum::http::header::ORIGIN, common::ORIGIN)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", ip)
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))))
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn sign_in_links_ask_for_the_second_factor_and_name_their_account_first() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, secret) = turn_on_totp(&app, &owner.cookie).await;

    let token = magic_link(&app, "owner@example.com").await;
    let named = app
        .post("/auth/magic-link/lookup", json!({ "token": token }), None)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(named.body["email"], "owner@example.com");
    let r = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await
        .expect_status(StatusCode::ACCEPTED);
    assert!(r.cookie().is_none());
    assert_eq!(r.body["status"], "totp_required");
    let challenge = r.body["challenge"].as_str().unwrap().to_owned();
    app.post("/auth/magic-link/lookup", json!({ "token": token }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);

    next_step(&app).await;
    let signed_in = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "code": code(&secret, &app) }),
            None,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(signed_in.body["session"]["method"], "magic_link");
    assert_eq!(signed_in.body["session"]["second_factor"], true);
    s.cleanup().await;
}

#[tokio::test]
async fn totp_needs_a_recent_sign_in_to_start_and_confirms_only_once() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    app.clock.advance(time::Duration::minutes(16));
    let r = app
        .post("/me/totp", json!({}), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(r.error_type(), "recent_sign_in_required");

    let fresh = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .expect_status(StatusCode::OK)
        .cookie()
        .unwrap();
    let before = app.get("/auth/session", &fresh).await.body["session"]["expires_at"].clone();
    let (rotated, secret) = turn_on_totp(&app, &fresh).await;
    let after = app.get("/auth/session", &rotated).await;
    assert_eq!(after.body["session"]["expires_at"], before);

    next_step(&app).await;
    let again = app
        .post(
            "/me/totp/confirm",
            json!({ "code": code(&secret, &app) }),
            Some(&rotated),
        )
        .await
        .expect_status(StatusCode::CONFLICT);
    assert_eq!(
        again.body["error"]["message"],
        "Two-factor authentication is already on; turn it off to set it up again."
    );
    let me = app.get("/me", &rotated).await;
    assert_eq!(me.body["recovery_codes_left"], 10);
    s.cleanup().await;
}

#[tokio::test]
async fn email_changes_need_the_second_factor_and_can_be_cancelled_from_the_old_address() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cookie, secret) = turn_on_totp(&app, &owner.cookie).await;

    let r = app
        .post(
            "/me/email",
            json!({ "email": "new@example.com" }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "code");
    next_step(&app).await;
    app.post(
        "/me/email",
        json!({ "email": "new@example.com", "code": code(&secret, &app) }),
        Some(&cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.wait_for_mail("new@example.com", 1).await;
    app.wait_for_mail("owner@example.com", 2).await;
    let warning = app.mail.to("owner@example.com").last().unwrap().clone();
    assert_eq!(warning.kind.as_str(), "email_change_requested");
    assert!(warning.text.contains("new@example.com"), "{}", warning.text);
    assert!(warning.text.contains("/me/email/cancel#token="));
    let confirm = app.last_token("new@example.com");
    let cancel = app.last_token("owner@example.com");

    app.post("/me/email/cancel", json!({ "token": cancel }), None)
        .await
        .expect_status(StatusCode::NO_CONTENT);
    app.post("/me/email/confirm", json!({ "token": confirm }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    app.post("/me/email/cancel", json!({ "token": cancel }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    let me = app.get("/me", &cookie).await;
    assert_eq!(me.body["email"], "owner@example.com");

    next_step(&app).await;
    app.post(
        "/me/email",
        json!({ "email": "new@example.com", "code": code(&secret, &app) }),
        Some(&cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.wait_for_mail("new@example.com", 2).await;
    let confirm = app.last_token("new@example.com");
    app.post("/me/email/confirm", json!({ "token": confirm }), None)
        .await
        .expect_status(StatusCode::OK);
    let me = app.get("/me", &cookie).await;
    assert_eq!(me.body["email"], "new@example.com");
    s.cleanup().await;
}

#[tokio::test]
async fn ending_sessions_or_resetting_the_password_ends_a_pending_email_change() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let ask = |cookie: String, n: usize| {
        let app = &app;
        async move {
            app.post(
                "/me/email",
                json!({ "email": "new@example.com" }),
                Some(&cookie),
            )
            .await
            .expect_status(StatusCode::OK);
            app.wait_for_mail("new@example.com", n).await;
            app.last_token("new@example.com")
        }
    };

    let other = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let from_other = ask(other.clone(), 1).await;
    app.post("/auth/sessions/revoke-all", json!({}), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::OK);
    app.post("/me/email/confirm", json!({ "token": from_other }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);

    let pending = ask(owner.cookie.clone(), 2).await;
    let mails = app.mail.to("owner@example.com").len();
    app.post(
        "/auth/password-reset",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", mails + 1).await;
    let reset = app
        .mail
        .to("owner@example.com")
        .iter()
        .rev()
        .find(|m| m.kind.as_str() == "password_reset")
        .map(|m| {
            let start = m.text.find("#token=").unwrap() + 7;
            m.text[start..]
                .split_whitespace()
                .next()
                .unwrap()
                .to_owned()
        })
        .unwrap();
    app.post(
        "/auth/password-reset/verify",
        json!({ "token": reset, "password": "a brand new passphrase" }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);
    app.post("/me/email/confirm", json!({ "token": pending }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    s.cleanup().await;
}

#[tokio::test]
async fn a_password_change_retires_sign_in_links() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let link = magic_link(&app, "owner@example.com").await;
    app.post(
        "/me/password",
        json!({ "current_password": PASSWORD, "new_password": "a brand new passphrase" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.post("/auth/magic-link/verify", json!({ "token": link }), None)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    s.cleanup().await;
}

#[tokio::test]
async fn failed_sign_ins_lock_one_address_out_of_one_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let attempt = |ip: &'static str, password: &'static str| {
        let app = &app;
        async move {
            app.raw(from(
                ip,
                "/auth/password",
                json!({ "email": "owner@example.com", "password": password }),
            ))
            .await
            .status
        }
    };
    for _ in 0..3 {
        assert_eq!(attempt("198.51.100.7", PASSWORD).await, StatusCode::OK);
    }
    for _ in 0..10 {
        assert_eq!(
            attempt("198.51.100.7", "wrong password!!").await,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        attempt("198.51.100.7", PASSWORD).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(attempt("203.0.113.9", PASSWORD).await, StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn sign_in_is_refused_while_its_throttles_cannot_be_counted() {
    let valkey = common::flaky_valkey::FlakyValkey::start().await;
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[("KAESO_VALKEY_URL", valkey.url.as_str())]).await;
    app.setup().await;
    valkey.cut();
    let r = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        "{:?}",
        r.body
    );
    valkey.restore();
    common::eventually("sign-in once the counters are back", || async {
        app.post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .status
            == StatusCode::OK
    })
    .await;
    s.cleanup().await;
}

#[tokio::test]
async fn a_revoked_session_is_not_brought_back_by_its_next_use() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    app.get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    sqlx::query("UPDATE sessions SET revoked_at = now()")
        .execute(&app.state.pg)
        .await
        .unwrap();
    app.clock.advance(time::Duration::seconds(61));
    app.get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    app.get("/me", &owner.cookie)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    s.cleanup().await;
}

#[tokio::test]
async fn turning_totp_off_needs_a_current_code() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cookie, _) = turn_on_totp(&app, &owner.cookie).await;
    let r = app
        .call(
            Method::DELETE,
            "/me/totp",
            Some(json!({ "code": "000000" })),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "That code isn't valid or was already used."
    );
    let recovery = app
        .post("/me/recovery-codes", json!({}), Some(&cookie))
        .await
        .expect_status(StatusCode::OK)
        .body["recovery_codes"][0]
        .as_str()
        .unwrap()
        .to_owned();
    app.call(
        Method::DELETE,
        "/me/totp",
        Some(json!({ "recovery_code": recovery })),
        Some(&cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    s.cleanup().await;
}
