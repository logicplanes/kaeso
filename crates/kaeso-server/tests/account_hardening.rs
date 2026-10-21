#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, PASSWORD, totp_code as code};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn turn_on_totp(app: &App, cookie: &str) -> (String, String) {
    let (cookie, secret, _) = totp_with_recovery_codes(app, cookie).await;
    (cookie, secret)
}

async fn totp_with_recovery_codes(app: &App, cookie: &str) -> (String, String, Vec<String>) {
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
    let recovery = serde_json::from_value(confirm.body["recovery_codes"].clone()).unwrap();
    (confirm.cookie().unwrap(), secret, recovery)
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
    assert!(warning.text.contains("/me/email/confirm#cancel="));
    let confirm = app.last_token("new@example.com");
    let start = warning.text.find("#cancel=").unwrap() + 8;
    let cancel = warning.text[start..]
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();

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

const WRONG: &str = "That code isn't valid or was already used.";
const ENDED: &str = "Too many wrong codes for this sign-in; start signing in again.";
const LOCKED: &str =
    "Too many wrong codes in a row; sign in with a recovery code or reset your password.";
const OWNER: &str = "owner@example.com";

fn wrong_code(secret: &str, app: &App) -> String {
    let mut near = vec![code(secret, app)];
    for seconds in [-30, 30] {
        app.clock.advance(time::Duration::seconds(seconds));
        near.push(code(secret, app));
        app.clock.advance(time::Duration::seconds(-seconds));
    }
    (0..10)
        .map(|d: u8| d.to_string().repeat(6))
        .find(|c| !near.contains(c))
        .unwrap()
}

async fn password_challenge(app: &App, ip: &str, password: &str) -> Value {
    app.raw(from(
        ip,
        "/auth/password",
        json!({ "email": OWNER, "password": password }),
    ))
    .await
    .expect_status(StatusCode::ACCEPTED)
    .body
}

async fn link_challenge(app: &App, ip: &str) -> Value {
    let token = magic_link(app, OWNER).await;
    app.raw(from(
        ip,
        "/auth/magic-link/verify",
        json!({ "token": token }),
    ))
    .await
    .expect_status(StatusCode::ACCEPTED)
    .body
}

async fn answer(app: &App, ip: &str, challenge: &Value, mut factor: Value) -> common::Resp {
    factor["challenge"] = challenge["challenge"].clone();
    app.raw(from(ip, "/auth/totp", factor)).await
}

#[track_caller]
fn refused(r: &common::Resp, status: StatusCode, message: &str) {
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str()),
        (status, Some(message)),
        "{:?}",
        r.body
    );
    assert!(r.cookie().is_none());
}

fn locked_emails(app: &App) -> Vec<String> {
    app.mail
        .to(OWNER)
        .into_iter()
        .filter(|m| m.kind.as_str() == "two_factor_locked")
        .map(|m| m.text)
        .collect()
}

#[tokio::test]
async fn a_challenge_ends_after_three_wrong_codes() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, secret) = turn_on_totp(&app, &owner.cookie).await;
    next_step(&app).await;
    let ip = "2001:db8:1::1";

    let challenge = password_challenge(&app, ip, PASSWORD).await;
    assert_eq!(
        (&challenge["attempts"], &challenge["codes_locked"]),
        (&json!(3), &json!(false))
    );
    let bad = json!({ "code": wrong_code(&secret, &app) });
    for _ in 0..2 {
        refused(
            &answer(&app, ip, &challenge, bad.clone()).await,
            StatusCode::UNAUTHORIZED,
            WRONG,
        );
    }
    refused(
        &answer(&app, ip, &challenge, bad.clone()).await,
        StatusCode::UNAUTHORIZED,
        ENDED,
    );
    refused(
        &answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) })).await,
        StatusCode::UNAUTHORIZED,
        "The sign-in has expired; enter your email and password again.",
    );

    let challenge = link_challenge(&app, ip).await;
    for _ in 0..2 {
        refused(
            &answer(&app, ip, &challenge, bad.clone()).await,
            StatusCode::UNAUTHORIZED,
            WRONG,
        );
    }
    let r = answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) }))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["session"]["method"], "magic_link");
    s.cleanup().await;
}

#[tokio::test]
async fn wrong_codes_in_a_row_lock_app_codes_until_a_recovery_code_signs_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, secret, recovery) = totp_with_recovery_codes(&app, &owner.cookie).await;
    next_step(&app).await;
    let bad = json!({ "code": wrong_code(&secret, &app) });

    for n in 1..=3 {
        let ip = format!("2001:db8:2:{n}::1");
        let challenge = if n == 2 {
            link_challenge(&app, &ip).await
        } else {
            password_challenge(&app, &ip, PASSWORD).await
        };
        for _ in 0..3 {
            answer(&app, &ip, &challenge, bad.clone())
                .await
                .expect_status(StatusCode::UNAUTHORIZED);
        }
    }
    assert!(locked_emails(&app).is_empty());

    app.clock.advance(time::Duration::days(2));
    let bad = json!({ "code": wrong_code(&secret, &app) });
    let ip = "2001:db8:2:4::1";
    let challenge = password_challenge(&app, ip, PASSWORD).await;
    assert_eq!(challenge["codes_locked"], false);
    let before = app.mail.to(OWNER).len();
    refused(
        &answer(&app, ip, &challenge, bad.clone()).await,
        StatusCode::FORBIDDEN,
        LOCKED,
    );
    app.wait_for_mail(OWNER, before + 1).await;
    let sent = locked_emails(&app);
    assert_eq!(sent.len(), 1, "the lock engaging sends one email");
    assert!(sent[0].contains("10 wrong codes in a row"), "{}", sent[0]);
    assert!(
        sent[0].contains(&format!("{}/auth/password-reset", common::ORIGIN)),
        "{}",
        sent[0]
    );

    refused(
        &answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) })).await,
        StatusCode::FORBIDDEN,
        LOCKED,
    );
    let ip = "2001:db8:2:5::1";
    let challenge = link_challenge(&app, ip).await;
    assert_eq!(challenge["codes_locked"], true);
    refused(
        &answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) })).await,
        StatusCode::FORBIDDEN,
        LOCKED,
    );
    assert_eq!(locked_emails(&app).len(), 1);

    for n in 6..=7 {
        let ip = format!("2001:db8:2:{n}::1");
        let challenge = password_challenge(&app, &ip, PASSWORD).await;
        for _ in 0..3 {
            refused(
                &answer(&app, &ip, &challenge, bad.clone()).await,
                StatusCode::FORBIDDEN,
                LOCKED,
            );
        }
    }
    assert_eq!(locked_emails(&app).len(), 1);
    let before = app.mail.to(OWNER).len();
    let ip = "2001:db8:2:9::1";
    let challenge = password_challenge(&app, ip, PASSWORD).await;
    for _ in 0..2 {
        refused(
            &answer(&app, ip, &challenge, bad.clone()).await,
            StatusCode::FORBIDDEN,
            LOCKED,
        );
    }
    app.wait_for_mail(OWNER, before + 1).await;
    let sent = locked_emails(&app);
    assert_eq!(sent.len(), 2, "every ten more wrong codes send another");
    assert!(sent[1].contains("20 wrong codes in a row"), "{}", sent[1]);

    let r = answer(
        &app,
        ip,
        &challenge,
        json!({ "recovery_code": recovery[0] }),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(r.body["session"]["second_factor"], true);
    let challenge = password_challenge(&app, ip, PASSWORD).await;
    assert_eq!(
        challenge["codes_locked"], false,
        "a recovery code unlocks app codes"
    );
    answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) }))
        .await
        .expect_status(StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn a_password_reset_by_email_unlocks_app_codes() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, secret) = turn_on_totp(&app, &owner.cookie).await;
    next_step(&app).await;
    let bad = json!({ "code": wrong_code(&secret, &app) });
    for n in 1..=4 {
        let ip = format!("2001:db8:3:{n}::1");
        let challenge = password_challenge(&app, &ip, PASSWORD).await;
        for _ in 0..3 {
            answer(&app, &ip, &challenge, bad.clone()).await;
        }
    }
    let ip = "2001:db8:3:5::1";
    assert_eq!(
        password_challenge(&app, ip, PASSWORD).await["codes_locked"],
        true
    );

    let before = app.mail.to(OWNER).len();
    app.post("/auth/password-reset", json!({ "email": OWNER }), None)
        .await
        .expect_status(StatusCode::OK);
    app.wait_for_mail(OWNER, before + 1).await;
    let new_password = "a brand new and long password";
    app.post(
        "/auth/password-reset/verify",
        json!({ "token": app.last_token(OWNER), "password": new_password }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);

    let challenge = password_challenge(&app, ip, new_password).await;
    assert_eq!(challenge["codes_locked"], false);
    answer(&app, ip, &challenge, json!({ "code": code(&secret, &app) }))
        .await
        .expect_status(StatusCode::OK);
    s.cleanup().await;
}
