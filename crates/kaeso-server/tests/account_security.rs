#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

use common::{PASSWORD, totp_code as code};

async fn turn_on_totp(app: &App, cookie: &str) -> String {
    turn_on_totp_with_secret(app, cookie).await.0
}

async fn turn_on_totp_with_secret(app: &App, cookie: &str) -> (String, String) {
    let begin = app.post("/me/totp", json!({}), Some(cookie)).await;
    assert_eq!(begin.status, StatusCode::OK, "{:?}", begin.body);
    let secret = begin.body["secret"].as_str().unwrap().to_owned();
    let confirm = app
        .post(
            "/me/totp/confirm",
            json!({ "code": code(&secret, app) }),
            Some(cookie),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::OK, "{:?}", confirm.body);
    (confirm.cookie().unwrap(), secret)
}

#[tokio::test]
async fn a_password_change_needs_the_current_password() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    let r = app
        .post(
            "/me/password",
            json!({ "new_password": "a brand new passphrase" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "current_password");

    let r = app
        .post(
            "/me/password",
            json!({ "current_password": "not it at all", "new_password": "a brand new passphrase" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "current_password");

    let r = app
        .post(
            "/me/password",
            json!({ "current_password": PASSWORD, "new_password": "a brand new passphrase" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let rotated = r.cookie().unwrap();
    assert_eq!(
        app.get("/auth/session", &rotated).await.status,
        StatusCode::OK
    );
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    app.wait_for_mail("owner@example.com", 1).await;
    s.cleanup().await;
}

#[tokio::test]
async fn recovery_codes_and_turning_totp_off_follow_the_org_policy() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    let r = app
        .post("/me/recovery-codes", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .call(
            Method::DELETE,
            "/me/totp",
            Some(json!({})),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    let (cookie, secret) = turn_on_totp_with_secret(&app, &owner.cookie).await;
    let r = app
        .post("/me/recovery-codes", json!({}), Some(&cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["recovery_codes"].as_array().unwrap().len(), 10);

    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/require-2fa", owner.org_id),
            Some(json!({ "enabled": true })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(Method::DELETE, "/me/totp", Some(json!({})), Some(&cookie))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);

    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/require-2fa", owner.org_id),
            Some(json!({ "enabled": false })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(Method::DELETE, "/me/totp", Some(json!({})), Some(&cookie))
        .await;
    assert_eq!(r.body["error"]["param"], "code", "{:?}", r.body);
    let r = app
        .call(
            Method::DELETE,
            "/me/totp",
            Some(json!({ "code": code(&secret, &app) })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.body["error"]["param"], "code", "{:?}", r.body);
    app.clock.advance(time::Duration::seconds(30));
    let r = app
        .call(
            Method::DELETE,
            "/me/totp",
            Some(json!({ "code": code(&secret, &app) })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    let after = r.cookie().unwrap();
    let me = app.get("/me", &after).await;
    assert_eq!(me.status, StatusCode::OK, "{:?}", me.body);
    assert_eq!(me.body["totp_enabled"], false, "{:?}", me.body);
    s.cleanup().await;
}

#[tokio::test]
async fn turning_on_totp_keeps_the_time_of_the_last_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let signed_in_at =
        app.get("/auth/session", &owner.cookie).await.body["session"]["signed_in_at"].clone();
    app.clock.advance(time::Duration::minutes(10));
    let rotated = turn_on_totp(&app, &owner.cookie).await;
    let who = app.get("/auth/session", &rotated).await;
    assert_eq!(who.body["session"]["signed_in_at"], signed_in_at);
    assert_eq!(who.body["session"]["second_factor"], true);
    app.clock.advance(time::Duration::minutes(6));
    let r = app
        .post(
            "/me/email",
            json!({ "email": "new@example.com" }),
            Some(&rotated),
        )
        .await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::FORBIDDEN, "recent_sign_in_required"),
        "{:?}",
        r.body
    );
    s.cleanup().await;
}

fn refused_keeping_the_cookie(r: &common::Resp) {
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    assert!(
        r.headers.get("set-cookie").is_none(),
        "{:?}",
        r.headers.get("set-cookie")
    );
}

#[tokio::test]
async fn a_request_still_carrying_a_replaced_session_leaves_the_new_cookie_alone() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let other = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();

    let with_totp = turn_on_totp(&app, &owner.cookie).await;
    refused_keeping_the_cookie(&app.get("/auth/session", &owner.cookie).await);
    refused_keeping_the_cookie(&app.get("/me", &owner.cookie).await);
    assert_eq!(
        app.get("/auth/session", &with_totp).await.status,
        StatusCode::OK
    );

    let r = app
        .post(
            "/me/password",
            json!({ "current_password": PASSWORD, "new_password": "a brand new passphrase" }),
            Some(&with_totp),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let after_password = r.cookie().unwrap();
    refused_keeping_the_cookie(&app.get("/auth/session", &with_totp).await);
    assert_eq!(
        app.get("/auth/session", &after_password).await.status,
        StatusCode::OK
    );

    let ended = app.get("/auth/session", &other).await;
    assert_eq!(ended.status, StatusCode::UNAUTHORIZED, "{:?}", ended.body);
    assert!(
        ended.headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    s.cleanup().await;
}

#[tokio::test]
async fn guesses_of_the_current_password_are_limited() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let change = |current: &'static str| {
        let app = &app;
        let cookie = owner.cookie.clone();
        async move {
            app.post(
                "/me/password",
                json!({ "current_password": current, "new_password": "a brand new passphrase" }),
                Some(&cookie),
            )
            .await
        }
    };
    for _ in 0..10 {
        let r = change("not the password").await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    }
    let r = change(PASSWORD).await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
        "{:?}",
        r.body
    );
    s.cleanup().await;
}

#[tokio::test]
async fn whether_an_address_is_taken_is_answered_only_within_the_email_allowance() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    kaeso_store::pg::users::insert(
        &app.state.pg,
        &kaeso_store::pg::users::NewUser {
            id: uuid::Uuid::now_v7(),
            email: "taken@example.com",
            name: "Taken",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: false,
        },
    )
    .await
    .unwrap();
    let mut statuses = Vec::new();
    for _ in 0..6 {
        let r = app
            .post(
                "/me/email",
                json!({ "email": "taken@example.com" }),
                Some(&owner.cookie),
            )
            .await;
        statuses.push(r.status);
    }
    assert_eq!(statuses[..5], [StatusCode::CONFLICT; 5]);
    assert_eq!(statuses[5], StatusCode::TOO_MANY_REQUESTS);
    s.cleanup().await;
}
