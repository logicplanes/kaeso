#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_core::clock::Clock;
use kaeso_crypto::SecretBox;
use kaeso_testkit::services::TestServices;
use serde_json::json;

const PASSWORD: &str = "correct horse battery";

fn base32_decode(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = ALPHABET.iter().position(|a| *a == c).unwrap() as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

fn code(secret: &str, app: &App) -> String {
    let now = app.clock.now().unix_timestamp() as u64;
    kaeso_crypto::totp::code_at(&SecretBox::new(Box::new(base32_decode(secret))), now).unwrap()
}

async fn turn_on_totp(app: &App, cookie: &str) -> String {
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
    confirm.cookie().unwrap()
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
        .call(Method::DELETE, "/me/totp", None, Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    let cookie = turn_on_totp(&app, &owner.cookie).await;
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
        .call(Method::DELETE, "/me/totp", None, Some(&cookie))
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
        .call(Method::DELETE, "/me/totp", None, Some(&cookie))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    let after = r.cookie().unwrap();
    let me = app.get("/me", &after).await;
    assert_eq!(me.status, StatusCode::OK, "{:?}", me.body);
    assert_eq!(me.body["totp_enabled"], false, "{:?}", me.body);
    s.cleanup().await;
}
