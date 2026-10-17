#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

use common::{PASSWORD, totp_code as code};

async fn sign_in(app: &App, email: &str, password: &str) -> common::Resp {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": password }),
        None,
    )
    .await
}

#[tokio::test]
async fn setup_creates_the_first_admin_owner_and_works_once() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let url = kaeso_server::setup::issue_link(&app.state)
        .await
        .unwrap()
        .unwrap();
    let token = url.split("#token=").nth(1).unwrap().to_owned();
    let r = app
        .post("/auth/setup", json!({ "token": token, "email": "owner@example.com", "name": "O", "password": "short", "org_name": "Acme" }), None)
        .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("password"))
    );
    let body = json!({ "token": token, "email": " Owner@Example.com ", "name": "Owner", "password": PASSWORD, "org_name": "Acme" });
    let r = app.post("/auth/setup", body.clone(), None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["status"], "signed_in");
    assert_eq!(r.body["user"]["email"], "Owner@example.com");
    assert_eq!(r.body["user"]["is_instance_admin"], true);
    let cookie = r.cookie().unwrap();
    let set_cookie = r.headers["set-cookie"].to_str().unwrap();
    assert!(
        set_cookie.contains("HttpOnly")
            && set_cookie.contains("Secure")
            && set_cookie.contains("SameSite=Lax")
            && set_cookie.contains("Path=/"),
        "{set_cookie}"
    );

    let who = app.get("/auth/session", &cookie).await;
    assert_eq!(who.status, StatusCode::OK);
    let m = &who.body["memberships"][0];
    assert_eq!(
        (
            m["org_name"].as_str(),
            m["preset"].as_str(),
            m["access"].as_str()
        ),
        (Some("Acme"), Some("owner"), Some("ok"))
    );
    assert_eq!(who.body["session"]["method"], "setup");

    let again = app.post("/auth/setup", body, None).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert!(
        kaeso_server::setup::issue_link(&app.state)
            .await
            .unwrap()
            .is_none()
    );

    let (instance_events, org_events): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE scope = 'instance'), count(*) FILTER (WHERE scope = 'org') FROM audit_events",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!((instance_events, org_events), (1, 1));
    s.cleanup().await;
}

const SSO_ONLY: [(&str, &str); 2] = [
    ("KAESO_AUTH_PASSWORD", "false"),
    ("KAESO_AUTH_MAGIC_LINK", "false"),
];

async fn setup_token(app: &App) -> String {
    let url = kaeso_server::setup::issue_link(&app.state)
        .await
        .unwrap()
        .unwrap();
    url.split("#token=").nth(1).unwrap().to_owned()
}

#[tokio::test]
async fn setup_asks_for_a_password_only_while_passwords_sign_in() {
    let s = TestServices::start().await;
    let with_passwords = App::start(&s).await;
    let token = setup_token(&with_passwords).await;
    let r = with_passwords
        .post(
            "/auth/setup",
            json!({ "token": token, "email": "owner@example.com", "name": "Owner", "org_name": "Acme" }),
            None,
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("password"))
    );
    assert_eq!(r.body["error"]["message"], "`password` is required.");

    let sso_only = App::start_with(&s, &SSO_ONLY).await;
    let r = sso_only
        .post(
            "/auth/setup",
            json!({ "token": token, "email": "owner@example.com", "name": "Owner", "password": PASSWORD, "org_name": "Acme" }),
            None,
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("password"))
    );
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("turned off"),
        "{:?}",
        r.body
    );
    let r = sso_only
        .post(
            "/auth/setup",
            json!({ "token": token, "email": "owner@example.com", "name": "Owner", "org_name": "Acme" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let cookie = r.cookie().unwrap();
    let me = sso_only.get("/me", &cookie).await;
    assert_eq!(me.body["has_password"], false);
    let stored: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&sso_only.state.pg)
        .await
        .unwrap();
    assert_eq!(stored, None);
    let audited: serde_json::Value =
        sqlx::query_scalar("SELECT after FROM audit_events WHERE action = 'instance.setup'")
            .fetch_one(&sso_only.state.pg)
            .await
            .unwrap();
    assert_eq!(audited["has_password"], false);
    s.cleanup().await;
}

#[tokio::test]
async fn an_operator_sign_in_link_works_while_email_links_are_off() {
    use kaeso_server::services::users::{OperatorSignIn, operator_sign_in_link};
    use secrecy::ExposeSecret;

    let s = TestServices::start().await;
    let app = App::start_with(&s, &SSO_ONLY).await;
    let owner = app.setup().await;
    let link = |email: &'static str| {
        let app = &app;
        async move { operator_sign_in_link(&app.state, email).await.unwrap() }
    };
    let OperatorSignIn::Link(url) = link(" Owner@Example.com ").await else {
        panic!("no link for the owner");
    };
    let url = url.expose_secret().to_owned();
    assert!(
        url.starts_with(&format!("{}/auth/magic-link#token=", common::ORIGIN)),
        "{url}"
    );
    let token = url.split("#token=").nth(1).unwrap().to_owned();
    let r = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let who = app.get("/auth/session", &r.cookie().unwrap()).await;
    assert_eq!(who.body["user"]["id"], owner.user_id.as_str());
    assert_eq!(who.body["session"]["method"], "magic_link");
    let again = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(again.status, StatusCode::UNAUTHORIZED);
    let (audited,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_events WHERE action = 'user.sign_in_link' AND target_id = $1 AND actor_kind = 'system'",
    )
    .bind(&owner.user_id)
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(audited, 1);

    assert!(matches!(
        link("nobody@example.com").await,
        OperatorSignIn::UnknownEmail
    ));
    assert!(matches!(
        link("not an address").await,
        OperatorSignIn::UnknownEmail
    ));
    sqlx::query("UPDATE users SET state = 'deactivated'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert!(matches!(
        link("owner@example.com").await,
        OperatorSignIn::Deactivated
    ));
    assert_eq!(
        format!("{:?}", OperatorSignIn::Link("secret".to_owned().into())),
        "Link(…)"
    );

    let emailed = "an-emailed-link";
    kaeso_store::pg::tokens::insert(
        &app.state.pg,
        kaeso_store::pg::tokens::NewToken {
            kind: kaeso_store::pg::tokens::TokenKind::MagicLink,
            hash: &kaeso_core::secret::hash(emailed),
            user_id: Some(common::uuid(&owner.user_id)),
            email: Some("owner@example.com"),
            data: json!({}),
            expires_at: kaeso_core::clock::Clock::now(&app.clock) + time::Duration::minutes(15),
        },
    )
    .await
    .unwrap();
    let r = app
        .post("/auth/magic-link/verify", json!({ "token": emailed }), None)
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "Sign-in links are turned off on this install."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn password_sign_in_never_reveals_whether_an_account_exists() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let ok = sign_in(&app, "OWNER@example.com", PASSWORD).await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.body);
    assert_eq!(ok.body["session"]["method"], "password");
    let wrong = sign_in(&app, "owner@example.com", "wrong password!!").await;
    let unknown = sign_in(&app, "nobody@example.com", "wrong password!!").await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown.status, wrong.status);
    assert_eq!(unknown.body, wrong.body);
    let a = app
        .post(
            "/auth/magic-link",
            json!({ "email": "nobody@example.com" }),
            None,
        )
        .await;
    let b = app
        .post(
            "/auth/magic-link",
            json!({ "email": "owner@example.com" }),
            None,
        )
        .await;
    assert_eq!((a.status, &a.body), (b.status, &b.body));
    let c = app
        .post(
            "/auth/password-reset",
            json!({ "email": "nobody@example.com" }),
            None,
        )
        .await;
    assert_eq!(c.status, StatusCode::OK);
    app.wait_for_mail("owner@example.com", 1).await;
    assert!(app.mail.to("nobody@example.com").is_empty());
    s.cleanup().await;
}

#[tokio::test]
async fn failed_sign_ins_are_throttled_per_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    for _ in 0..10 {
        assert_eq!(
            sign_in(&app, "owner@example.com", "wrong password!!")
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
    let blocked = sign_in(&app, "owner@example.com", PASSWORD).await;
    assert_eq!(
        blocked.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{:?}",
        blocked.body
    );
    assert!(blocked.headers.contains_key("retry-after"));
    assert_eq!(blocked.error_type(), "rate_limited");
    s.cleanup().await;
}

#[tokio::test]
async fn concurrent_sign_in_guesses_share_the_account_allowance() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let answers = futures::future::join_all(
        (0..30).map(|_| sign_in(&app, "owner@example.com", "wrong password!!")),
    )
    .await;
    let wrong = answers
        .iter()
        .filter(|r| r.status == StatusCode::UNAUTHORIZED)
        .count();
    let throttled = answers
        .iter()
        .filter(|r| r.status == StatusCode::TOO_MANY_REQUESTS)
        .count();
    assert_eq!((wrong, throttled), (10, 20));
    s.cleanup().await;
}

#[tokio::test]
async fn addresses_of_one_ipv6_network_share_the_address_allowance() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let mut statuses = Vec::new();
    for n in 0..31 {
        let req = axum::http::Request::post("/platform/v1/auth/magic-link")
            .header(axum::http::header::ORIGIN, common::ORIGIN)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", format!("2001:db8:7:9::{n:x}"))
            .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                40000,
            ))))
            .body(axum::body::Body::from(
                json!({ "email": format!("guess{n}@example.com") }).to_string(),
            ))
            .unwrap();
        statuses.push(app.raw(req).await.status);
    }
    assert!(
        statuses[..30].iter().all(|s| *s == StatusCode::OK),
        "{statuses:?}"
    );
    assert_eq!(statuses[30], StatusCode::TOO_MANY_REQUESTS);
    s.cleanup().await;
}

#[tokio::test]
async fn totp_setup_challenge_replay_and_recovery_codes() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let begin = app.post("/me/totp", json!({}), Some(&owner.cookie)).await;
    assert_eq!(begin.status, StatusCode::OK, "{:?}", begin.body);
    let secret = begin.body["secret"].as_str().unwrap().to_owned();
    assert!(
        begin.body["otpauth_uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/Kaeso:")
    );
    let bad = app
        .post(
            "/me/totp/confirm",
            json!({ "code": "000000" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let confirm = app
        .post(
            "/me/totp/confirm",
            json!({ "code": code(&secret, &app) }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::OK, "{:?}", confirm.body);
    let recovery: Vec<String> =
        serde_json::from_value(confirm.body["recovery_codes"].clone()).unwrap();
    assert_eq!(recovery.len(), 10);
    let rotated = confirm.cookie().unwrap();
    assert_ne!(rotated, owner.cookie);
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    let who = app.get("/auth/session", &rotated).await;
    assert_eq!(
        (
            who.body["totp_enabled"].as_bool(),
            who.body["session"]["second_factor"].as_bool()
        ),
        (Some(true), Some(true))
    );
    assert_eq!(
        app.post("/me/totp", json!({}), Some(&rotated)).await.status,
        StatusCode::CONFLICT
    );

    app.clock.advance(time::Duration::seconds(60));
    let first = sign_in(&app, "owner@example.com", PASSWORD).await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{:?}", first.body);
    assert!(first.cookie().is_none());
    let challenge = first.body["challenge"].as_str().unwrap().to_owned();
    let current = code(&secret, &app);
    let ok = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "code": current }),
            None,
        )
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.body);
    assert_eq!(ok.body["session"]["second_factor"], true);
    let reused = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "code": current }),
            None,
        )
        .await;
    assert_eq!(reused.status, StatusCode::UNAUTHORIZED);
    let second = sign_in(&app, "owner@example.com", PASSWORD).await;
    let challenge = second.body["challenge"].as_str().unwrap().to_owned();
    let replay = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "code": current }),
            None,
        )
        .await;
    assert_eq!(replay.status, StatusCode::UNAUTHORIZED, "{:?}", replay.body);
    let rc = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "recovery_code": recovery[0].to_uppercase() }),
            None,
        )
        .await;
    assert_eq!(rc.status, StatusCode::OK, "{:?}", rc.body);
    let third = sign_in(&app, "owner@example.com", PASSWORD).await;
    let challenge = third.body["challenge"].as_str().unwrap().to_owned();
    let rc_again = app
        .post(
            "/auth/totp",
            json!({ "challenge": challenge, "recovery_code": recovery[0] }),
            None,
        )
        .await;
    assert_eq!(rc_again.status, StatusCode::UNAUTHORIZED);
    let me = app.get("/me", &rc.cookie().unwrap()).await;
    assert_eq!(me.body["recovery_codes_left"], 9);
    s.cleanup().await;
}

#[tokio::test]
async fn magic_links_work_once_and_expire() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    app.post(
        "/auth/magic-link",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", 1).await;
    let token = app.last_token("owner@example.com");
    let ok = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.body);
    assert_eq!(
        (
            ok.body["session"]["method"].as_str(),
            ok.body["session"]["second_factor"].as_bool()
        ),
        (Some("magic_link"), Some(false))
    );
    let reused = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(reused.status, StatusCode::UNAUTHORIZED);

    app.post(
        "/auth/magic-link",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", 2).await;
    let token = app.last_token("owner@example.com");
    app.clock.advance(time::Duration::minutes(16));
    let expired = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(expired.status, StatusCode::UNAUTHORIZED);
    s.cleanup().await;
}

#[tokio::test]
async fn a_password_reset_retires_the_links_sent_before_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    app.post(
        "/auth/magic-link",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", 1).await;
    let link = app.last_token("owner@example.com");
    for n in 2..=3 {
        app.post(
            "/auth/password-reset",
            json!({ "email": "owner@example.com" }),
            None,
        )
        .await;
        app.wait_for_mail("owner@example.com", n).await;
    }
    let reset = app.last_token("owner@example.com");
    let mails = app.mail.to("owner@example.com");
    let first_reset = &mails[1].text;
    let start = first_reset.find("#token=").unwrap() + 7;
    let first_reset = first_reset[start..]
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let done = app
        .post(
            "/auth/password-reset/verify",
            json!({ "token": reset, "password": "a brand new passphrase" }),
            None,
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.body);
    let r = app
        .post("/auth/magic-link/verify", json!({ "token": link }), None)
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    let r = app
        .post(
            "/auth/password-reset/verify",
            json!({ "token": first_reset, "password": "another new passphrase" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn password_reset_ends_every_session() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let other = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    app.post(
        "/auth/password-reset",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", 1).await;
    let token = app.last_token("owner@example.com");
    let weak = app
        .post(
            "/auth/password-reset/verify",
            json!({ "token": token, "password": "short" }),
            None,
        )
        .await;
    assert_eq!(weak.status, StatusCode::BAD_REQUEST);
    let done = app
        .post(
            "/auth/password-reset/verify",
            json!({ "token": token, "password": "a brand new passphrase" }),
            None,
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.body);
    for cookie in [&owner.cookie, &other] {
        assert_eq!(
            app.get("/auth/session", cookie).await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        sign_in(&app, "owner@example.com", PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sign_in(&app, "owner@example.com", "a brand new passphrase")
            .await
            .status,
        StatusCode::OK
    );
    app.wait_for_mail("owner@example.com", 2).await;
    assert_eq!(
        app.mail
            .to("owner@example.com")
            .last()
            .unwrap()
            .kind
            .as_str(),
        "password_changed"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn sessions_list_revoke_and_expire() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let second = sign_in(&app, "owner@example.com", PASSWORD).await;
    let second_cookie = second.cookie().unwrap();
    let second_id = second.body["session"]["id"].as_str().unwrap().to_owned();
    let list = app.get("/auth/sessions", &owner.cookie).await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 2);
    assert_eq!(
        list.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["current"] == true)
            .count(),
        1
    );
    let gone = app
        .call(
            Method::DELETE,
            &format!("/auth/sessions/{second_id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/auth/session", &second_cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    let missing = app
        .call(
            Method::DELETE,
            &format!("/auth/sessions/{second_id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let third = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    let all = app
        .post("/auth/sessions/revoke-all", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(all.body["revoked"], 1);
    assert_eq!(
        app.get("/auth/session", &third).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::OK
    );

    app.clock
        .advance(time::Duration::days(7) + time::Duration::seconds(1));
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );

    let fresh = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    let out = app.post("/auth/logout", json!({}), Some(&fresh)).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(
        out.headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        app.get("/auth/session", &fresh).await.status,
        StatusCode::UNAUTHORIZED
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_ended_session_cookie_neither_blocks_sign_in_nor_stays() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let other = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    app.post("/auth/sessions/revoke-all", json!({}), Some(&other))
        .await;

    let refused = app.get("/me", &owner.cookie).await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    assert!(
        refused.headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );

    let again = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    let fresh = again.cookie().unwrap();
    assert_eq!(app.get("/me", &fresh).await.status, StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn sessions_used_daily_end_after_thirty_days() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    for _ in 0..29 {
        app.clock.advance(time::Duration::days(1));
        assert_eq!(
            app.get("/auth/session", &owner.cookie).await.status,
            StatusCode::OK
        );
    }
    app.clock.advance(time::Duration::days(1));
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    s.cleanup().await;
}

#[tokio::test]
async fn revocation_reaches_a_second_instance_within_five_seconds() {
    let s = TestServices::start().await;
    let a = App::start(&s).await;
    let b = a.second_instance(&s).await;
    let owner = a.setup().await;
    assert_eq!(
        b.get("/auth/session", &owner.cookie).await.status,
        StatusCode::OK
    );
    assert_eq!(
        b.get("/auth/session", &owner.cookie).await.status,
        StatusCode::OK
    );
    a.post("/auth/logout", json!({}), Some(&owner.cookie)).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if b.get("/auth/session", &owner.cookie).await.status == StatusCode::UNAUTHORIZED {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "instance B still accepts the session"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    s.cleanup().await;
}

#[tokio::test]
async fn cookie_requests_from_other_origins_are_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let body = Some(json!({ "name": "New name" }));
    let none = app
        .call_with(
            Method::PATCH,
            "/me",
            body.clone(),
            Some(&owner.cookie),
            None,
        )
        .await;
    let evil = app
        .call_with(
            Method::PATCH,
            "/me",
            body.clone(),
            Some(&owner.cookie),
            Some("https://evil.example"),
        )
        .await;
    assert_eq!(
        (none.status, evil.status),
        (StatusCode::FORBIDDEN, StatusCode::FORBIDDEN)
    );
    assert!(
        evil.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Cross-site")
    );
    assert_eq!(
        app.call_with(Method::GET, "/me", None, Some(&owner.cookie), None)
            .await
            .status,
        StatusCode::OK
    );
    let ok = app
        .call(Method::PATCH, "/me", body, Some(&owner.cookie))
        .await;
    assert_eq!(
        (ok.status, ok.body["name"].as_str()),
        (StatusCode::OK, Some("New name"))
    );
    let r = app
        .call_with(
            Method::POST,
            "/auth/password",
            Some(json!({ "email": "owner@example.com", "password": PASSWORD })),
            None,
            Some("https://evil.example"),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    s.cleanup().await;
}

#[tokio::test]
async fn a_person_chooses_the_language_of_the_console() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let me = app.get("/me", &owner.cookie).await;
    assert_eq!(me.body["locale"], "auto");
    for locale in ["de", "zh-CN", "en", "auto"] {
        let r = app
            .call(
                Method::PATCH,
                "/me",
                Some(json!({ "locale": locale })),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(
            (r.status, r.body["locale"].as_str()),
            (StatusCode::OK, Some(locale))
        );
    }
    app.call(
        Method::PATCH,
        "/me",
        Some(json!({ "locale": "de" })),
        Some(&owner.cookie),
    )
    .await;
    let r = app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "name": "Ada" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["locale"], "de");
    let session = app.get("/auth/session", &owner.cookie).await;
    assert_eq!(session.body["locale"], "de");
    let r = app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "locale": "fr" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    s.cleanup().await;
}

#[tokio::test]
async fn sensitive_account_changes_need_a_recent_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    app.clock.advance(time::Duration::minutes(16));
    let r = app
        .post(
            "/me/email",
            json!({ "email": "new@example.com" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::FORBIDDEN, "recent_sign_in_required")
    );
    let fresh = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    let r = app
        .post(
            "/me/email",
            json!({ "email": "new@example.com" }),
            Some(&fresh),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    app.wait_for_mail("new@example.com", 1).await;
    let token = app.last_token("new@example.com");
    let confirmed = app
        .post("/me/email/confirm", json!({ "token": token }), None)
        .await;
    assert_eq!(
        (confirmed.status, confirmed.body["email"].as_str()),
        (StatusCode::OK, Some("new@example.com"))
    );
    app.wait_for_mail("owner@example.com", 1).await;
    assert_eq!(
        app.mail
            .to("owner@example.com")
            .last()
            .unwrap()
            .kind
            .as_str(),
        "email_changed"
    );
    assert_eq!(
        sign_in(&app, "new@example.com", PASSWORD).await.status,
        StatusCode::OK
    );
    s.cleanup().await;
}

#[tokio::test]
async fn password_changes_end_other_sessions_and_rotate_this_one() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let other = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    let wrong = app.post("/me/password", json!({ "current_password": "nope nope nope", "new_password": "another long passphrase" }), Some(&owner.cookie)).await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    let changed = app
        .post(
            "/me/password",
            json!({ "current_password": PASSWORD, "new_password": "another long passphrase" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    let fresh = changed.cookie().unwrap();
    assert_eq!(
        app.get("/auth/session", &other).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/auth/session", &fresh).await.status,
        StatusCode::OK
    );
    s.cleanup().await;
}

#[tokio::test]
async fn required_two_factor_and_sso_show_in_the_session() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    sqlx::query("UPDATE organizations SET require_2fa = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let password_session = sign_in(&app, "owner@example.com", PASSWORD)
        .await
        .cookie()
        .unwrap();
    let who = app.get("/auth/session", &password_session).await;
    assert_eq!(who.body["memberships"][0]["access"], "needs_2fa");
    app.post(
        "/auth/magic-link",
        json!({ "email": "owner@example.com" }),
        None,
    )
    .await;
    app.wait_for_mail("owner@example.com", 1).await;
    let magic = app
        .post(
            "/auth/magic-link/verify",
            json!({ "token": app.last_token("owner@example.com") }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    assert_eq!(
        app.get("/auth/session", &magic).await.body["memberships"][0]["access"],
        "ok"
    );
    sqlx::query("UPDATE organizations SET require_sso = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        app.get("/auth/session", &magic).await.body["memberships"][0]["access"],
        "needs_sso"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn sign_up_is_off_until_an_instance_admin_opens_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = app
        .post("/auth/signup", json!({ "email": "ada@example.com" }), None)
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "account_creation": "everyone" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        app.post("/auth/signup", json!({ "email": "ada@example.com" }), None)
            .await
            .status,
        StatusCode::OK
    );
    app.wait_for_mail("ada@example.com", 1).await;
    assert_eq!(app.mail.to("ada@example.com")[0].kind.as_str(), "signup");
    let token = app.last_token("ada@example.com");
    let r = app
        .post(
            "/auth/signup/verify",
            json!({ "token": token, "name": "Ada", "password": "a long enough password" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (
            r.body["session"]["method"].as_str(),
            r.body["user"]["is_instance_admin"].as_bool()
        ),
        (Some("signup"), Some(false))
    );
    let again = app
        .post("/auth/signup", json!({ "email": "ada@example.com" }), None)
        .await;
    assert_eq!(again.status, StatusCode::OK);
    app.wait_for_mail("ada@example.com", 2).await;
    assert_eq!(
        app.mail.to("ada@example.com")[1].kind.as_str(),
        "magic_link"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn resetting_two_factor_removes_it_and_ends_sessions() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let secret = app
        .post("/me/totp", json!({}), Some(&owner.cookie))
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let cookie = app
        .post(
            "/me/totp/confirm",
            json!({ "code": code(&secret, &app) }),
            Some(&owner.cookie),
        )
        .await
        .cookie()
        .unwrap();
    let user: kaeso_core::id::UserId = owner.user_id.parse().unwrap();
    let mut tx = app.state.pg.begin().await.unwrap();
    assert!(
        kaeso_server::services::users::reset_2fa(&app.state, &mut tx, user, "The operator")
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        app.get("/auth/session", &cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sign_in(&app, "owner@example.com", PASSWORD).await.status,
        StatusCode::OK
    );
    app.wait_for_mail("owner@example.com", 2).await;
    assert!(
        app.mail
            .to("owner@example.com")
            .iter()
            .any(|m| m.kind.as_str() == "two_factor_reset")
    );
    s.cleanup().await;
}
