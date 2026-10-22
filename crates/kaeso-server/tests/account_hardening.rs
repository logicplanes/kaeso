#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, PASSWORD, totp_code as code};
use kaeso_core::clock::Clock;
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
async fn an_email_change_tells_whether_an_address_is_taken_only_after_the_second_factor() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (eve, _) = common::join(
        &app,
        &owner,
        json!({ "email": "eve@example.com", "role": "member" }),
        "Eve",
    )
    .await;
    let (cookie, secret) = turn_on_totp(&app, &eve).await;
    let r = app
        .post(
            "/me/email",
            json!({ "email": "owner@example.com" }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["param"], "code",
        "without the second factor the answer is the same for every address"
    );
    let r = app
        .post(
            "/me/email",
            json!({ "email": "owner@example.com", "code": "000000" }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["message"], WRONG);
    next_step(&app).await;
    let r = app
        .post(
            "/me/email",
            json!({ "email": "owner@example.com", "code": code(&secret, &app) }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "Another account uses this address."
    );
    s.cleanup().await;
}

async fn sign_in_as(app: &App, email: &str) -> String {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await
    .expect_status(StatusCode::OK)
    .cookie()
    .unwrap()
}

async fn change_email(app: &App, cookie: &str, to: &str, mails: usize) -> String {
    app.post("/me/email", json!({ "email": to }), Some(cookie))
        .await
        .expect_status(StatusCode::OK);
    app.wait_for_mail(to, mails).await;
    app.last_token(to)
}

#[tokio::test]
async fn confirming_a_new_address_signs_out_every_other_session() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let laptop = sign_in_as(&app, "owner@example.com").await;
    let phone = sign_in_as(&app, "owner@example.com").await;
    let signed_in = |cookie: String| {
        let app = &app;
        async move { app.get("/auth/session", &cookie).await.status }
    };

    let token = change_email(&app, &owner.cookie, "new@example.com", 1).await;
    app.post("/me/email/confirm", json!({ "token": token }), None)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        signed_in(owner.cookie.clone()).await,
        StatusCode::OK,
        "the session that asked for the change stays when the link opens elsewhere"
    );
    for other in [&laptop, &phone] {
        assert_eq!(signed_in(other.clone()).await, StatusCode::UNAUTHORIZED);
    }

    let tablet = sign_in_as(&app, "new@example.com").await;
    let token = change_email(&app, &owner.cookie, "newer@example.com", 1).await;
    app.post(
        "/me/email/confirm",
        json!({ "token": token }),
        Some(&tablet),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(
        signed_in(tablet).await,
        StatusCode::OK,
        "the session that opens the link stays"
    );
    assert_eq!(
        signed_in(owner.cookie.clone()).await,
        StatusCode::UNAUTHORIZED
    );
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
async fn a_locked_account_stays_locked_however_its_address_is_written() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    sqlx::query("UPDATE users SET email = 'kim@example.com' WHERE email = 'owner@example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let attempt = |email: &'static str, password: &'static str| {
        let app = &app;
        async move {
            app.post(
                "/auth/password",
                json!({ "email": email, "password": password }),
                None,
            )
            .await
            .status
        }
    };
    for _ in 0..10 {
        assert_eq!(
            attempt("kim@example.com", "wrong password!!").await,
            StatusCode::UNAUTHORIZED
        );
    }
    for written in [
        "KIM@example.com",
        "\u{212A}IM@example.com",
        "kim@EXAMPLE.com",
    ] {
        assert_eq!(
            attempt(written, PASSWORD).await,
            StatusCode::TOO_MANY_REQUESTS,
            "{written} is the locked account's address"
        );
    }
    for _ in 0..3 {
        assert_eq!(
            attempt("K\u{130}M@example.com", PASSWORD).await,
            StatusCode::UNAUTHORIZED,
            "a dotted capital I is another address and never signs in to the locked account"
        );
    }
    s.cleanup().await;
}

#[tokio::test]
async fn emails_to_one_account_count_together_however_its_address_is_written() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    sqlx::query("UPDATE users SET email = 'kim@example.com' WHERE email = 'owner@example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    for written in [
        "kim@example.com",
        "KIM@example.com",
        "\u{212A}im@example.com",
        "\u{212A}IM@EXAMPLE.COM",
        "ｋｉｍ@example.com",
        "Kim@example.com",
    ] {
        app.post("/auth/magic-link", json!({ "email": written }), None)
            .await
            .expect_status(StatusCode::OK);
    }
    app.post(
        "/auth/password-reset",
        json!({ "email": "kim@example.com" }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);
    kaeso_store::pg::users::insert(
        &app.state.pg,
        &kaeso_store::pg::users::NewUser {
            id: uuid::Uuid::now_v7(),
            email: "ada@example.com",
            name: "Ada",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: false,
        },
    )
    .await
    .unwrap();
    app.post(
        "/auth/magic-link",
        json!({ "email": "ada@example.com" }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);
    app.wait_for_mail("ada@example.com", 1).await;
    assert_eq!(
        kinds_to(&app, "kim@example.com"),
        ["magic_link"; 5],
        "five emails an hour reach one address, whichever way it is written"
    );
    s.cleanup().await;
}

fn kinds_to(app: &App, to: &str) -> Vec<&'static str> {
    app.mail
        .all()
        .into_iter()
        .filter(|m| m.to == to)
        .map(|m| m.kind.as_str())
        .collect()
}

#[tokio::test]
async fn links_for_an_account_go_to_its_own_address_however_it_is_typed() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    sqlx::query("UPDATE users SET email = 'kim@example.com' WHERE email = 'owner@example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let kelvin = "\u{212A}im@example.com";
    app.post("/auth/password-reset", json!({ "email": kelvin }), None)
        .await
        .expect_status(StatusCode::OK);
    app.post("/auth/magic-link", json!({ "email": kelvin }), None)
        .await
        .expect_status(StatusCode::OK);
    app.wait_for_mail("kim@example.com", 2).await;
    assert_eq!(
        kinds_to(&app, "kim@example.com"),
        ["password_reset", "magic_link"],
        "a compatibility form of the address is the account's own address"
    );
    assert!(
        kinds_to(&app, kelvin).is_empty(),
        "nothing goes to the address as it was typed"
    );

    app.post(
        "/auth/password-reset",
        json!({ "email": "K\u{130}M@example.com" }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);
    app.post(
        "/auth/magic-link",
        json!({ "email": "KIM@example.com" }),
        None,
    )
    .await
    .expect_status(StatusCode::OK);
    app.wait_for_mail("kim@example.com", 3).await;
    assert_eq!(
        kinds_to(&app, "kim@example.com"),
        ["password_reset", "magic_link", "magic_link"],
        "a dotted capital I names another address, though PostgreSQL's lower() folds it to i"
    );
    assert!(kinds_to(&app, "K\u{130}M@example.com").is_empty());

    let token = app.last_token("kim@example.com");
    let r = app
        .post("/auth/magic-link/lookup", json!({ "token": token }), None)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["email"], "kim@example.com");
    s.cleanup().await;
}

const DECOMPOSED: &str = "e\u{301}va@example.com";
const COMPOSED: &str = "\u{E9}va@example.com";

async fn store_owner_as_before_the_upgrade(app: &App) {
    sqlx::query("UPDATE users SET email = $1 WHERE email = 'owner@example.com'")
        .bind(DECOMPOSED)
        .execute(&app.state.pg)
        .await
        .unwrap();
}

async fn password_sign_in(app: &App, email: &str) -> StatusCode {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await
    .status
}

async fn ask_for_links(app: &App, email: &str) {
    for path in ["/auth/password-reset", "/auth/magic-link"] {
        app.post(path, json!({ "email": email }), None)
            .await
            .expect_status(StatusCode::OK);
    }
}

#[tokio::test]
async fn an_address_stored_decomposed_before_the_upgrade_still_signs_in_and_gets_links() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    store_owner_as_before_the_upgrade(&app).await;
    for typed in [COMPOSED, DECOMPOSED, "\u{C9}VA@example.com"] {
        assert_eq!(
            password_sign_in(&app, typed).await,
            StatusCode::OK,
            "{typed}"
        );
    }
    ask_for_links(&app, COMPOSED).await;
    app.wait_for_mail(DECOMPOSED, 2).await;
    assert_eq!(
        kinds_to(&app, DECOMPOSED),
        ["password_reset", "magic_link"],
        "the links go to the address as the account stores it"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn accounts_that_share_a_canonical_address_are_never_picked_by_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    store_owner_as_before_the_upgrade(&app).await;
    sqlx::query(
        "INSERT INTO users (id, email, name, password_hash, email_verified_at) \
         SELECT $1, $2, 'Twin', password_hash, now() FROM users WHERE email = $3",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(COMPOSED)
    .bind(DECOMPOSED)
    .execute(&app.state.pg)
    .await
    .unwrap();
    for typed in [COMPOSED, DECOMPOSED] {
        assert_eq!(
            password_sign_in(&app, typed).await,
            StatusCode::UNAUTHORIZED,
            "{typed} names two accounts, so it signs in to neither"
        );
        ask_for_links(&app, typed).await;
    }
    kaeso_store::pg::users::insert(
        &app.state.pg,
        &kaeso_store::pg::users::NewUser {
            id: uuid::Uuid::now_v7(),
            email: "ada@example.com",
            name: "Ada",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: false,
        },
    )
    .await
    .unwrap();
    ask_for_links(&app, "ada@example.com").await;
    app.wait_for_mail("ada@example.com", 2).await;
    assert!(kinds_to(&app, DECOMPOSED).is_empty());
    assert!(
        kinds_to(&app, COMPOSED).is_empty(),
        "no link goes to either account"
    );
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
async fn a_session_from_an_ipv4_mapped_peer_shows_its_ipv4_address() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let peer: std::net::IpAddr = "::ffff:203.0.113.9".parse().unwrap();
    let r = app
        .raw(
            axum::http::Request::post("/platform/v1/auth/password")
                .header(axum::http::header::ORIGIN, common::ORIGIN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(std::net::SocketAddr::new(
                    peer, 40000,
                )))
                .body(axum::body::Body::from(
                    json!({ "email": "owner@example.com", "password": PASSWORD }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::OK);
    let listed = app
        .get("/auth/sessions", &r.cookie().unwrap())
        .await
        .expect_status(StatusCode::OK);
    let current = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["current"] == true)
        .unwrap();
    assert_eq!(current["ip"], "203.0.113.9");
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
async fn a_session_touched_as_it_is_revoked_is_not_cached_again() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    app.get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    app.clock.advance(time::Duration::seconds(61));
    app.get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    let touched: time::OffsetDateTime = sqlx::query_scalar("SELECT last_used_at FROM sessions")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        touched,
        app.clock.now(),
        "the second use touched the session"
    );
    sqlx::query("UPDATE sessions SET revoked_at = now()")
        .execute(&app.state.pg)
        .await
        .unwrap();
    app.get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
    s.cleanup().await;
}

#[tokio::test]
async fn turning_totp_off_needs_a_current_code() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cookie, _, old) = totp_with_recovery_codes(&app, &owner.cookie).await;
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
    let new = app
        .post(
            "/me/recovery-codes",
            json!({ "recovery_code": old[0] }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::OK)
        .body["recovery_codes"][0]
        .as_str()
        .unwrap()
        .to_owned();
    app.call(
        Method::DELETE,
        "/me/totp",
        Some(json!({ "recovery_code": old[1] })),
        Some(&cookie),
    )
    .await
    .expect_status(StatusCode::BAD_REQUEST);
    app.call(
        Method::DELETE,
        "/me/totp",
        Some(json!({ "recovery_code": new })),
        Some(&cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    s.cleanup().await;
}

#[tokio::test]
async fn new_recovery_codes_need_the_second_factor() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cookie, secret, recovery) = totp_with_recovery_codes(&app, &owner.cookie).await;
    let regenerate = |factor: Value| {
        let app = &app;
        let cookie = cookie.clone();
        async move { app.post("/me/recovery-codes", factor, Some(&cookie)).await }
    };
    let r = regenerate(json!({}))
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        (&r.body["error"]["param"], &r.body["error"]["message"]),
        (
            &json!("code"),
            &json!("Enter a code from your authenticator app or a recovery code.")
        ),
        "a stolen session alone can't replace the recovery codes"
    );
    let r = regenerate(json!({ "code": wrong_code(&secret, &app) }))
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["message"], WRONG);
    next_step(&app).await;
    let r = regenerate(json!({ "code": code(&secret, &app) }))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["recovery_codes"].as_array().unwrap().len(), 10);
    regenerate(json!({ "recovery_code": recovery[0] }))
        .await
        .expect_status(StatusCode::BAD_REQUEST);
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

const APP_CODES_LOCKED: &str =
    "Codes from your app are locked after too many wrong ones; use a recovery code.";

#[tokio::test]
async fn security_changes_count_wrong_codes_and_keep_to_their_lock() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cookie, secret, recovery) = totp_with_recovery_codes(&app, &owner.cookie).await;
    next_step(&app).await;
    let bad = json!({ "code": wrong_code(&secret, &app) });
    for n in 1..=3 {
        let ip = format!("2001:db8:3:{n}::1");
        let challenge = password_challenge(&app, &ip, PASSWORD).await;
        for _ in 0..3 {
            answer(&app, &ip, &challenge, bad.clone())
                .await
                .expect_status(StatusCode::UNAUTHORIZED);
        }
    }
    let turn_off = |factor: Value| {
        let app = &app;
        let cookie = cookie.clone();
        async move {
            app.call(Method::DELETE, "/me/totp", Some(factor), Some(&cookie))
                .await
        }
    };
    let before = app.mail.to(OWNER).len();
    let r = turn_off(bad.clone())
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        (&r.body["error"]["message"], &r.body["error"]["param"]),
        (&json!(APP_CODES_LOCKED), &json!("code")),
        "the tenth wrong code in a row locks app codes, wherever it was entered"
    );
    app.wait_for_mail(OWNER, before + 1).await;
    assert_eq!(locked_emails(&app).len(), 1);

    let r = turn_off(json!({ "code": code(&secret, &app) }))
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"], APP_CODES_LOCKED,
        "a right code from the app doesn't pass the lock"
    );
    let challenge = password_challenge(&app, "2001:db8:3:9::1", PASSWORD).await;
    assert_eq!(challenge["codes_locked"], true);

    turn_off(json!({ "recovery_code": recovery[0] }))
        .await
        .expect_status(StatusCode::NO_CONTENT);
    s.cleanup().await;
}
