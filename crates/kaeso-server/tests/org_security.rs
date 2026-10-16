#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner, Resp};
use kaeso_testkit::oidc::MockIdp;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

fn security(owner: &Owner) -> String {
    format!("/orgs/{}/security", owner.org_id)
}

async fn put(app: &App, cookie: &str, path: &str, body: Value) -> Resp {
    app.call(Method::PUT, path, Some(body), Some(cookie)).await
}

async fn delete(app: &App, cookie: &str, path: &str) -> Resp {
    app.call(Method::DELETE, path, None, Some(cookie)).await
}

async fn sso_session(app: &App, idp: &MockIdp, email: &str) -> String {
    let start = app
        .post("/auth/oidc/start", json!({ "email": email }), None)
        .await;
    assert_eq!(start.status, StatusCode::OK, "{:?}", start.body);
    let binding = start.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize(
        start.body["authorization_url"].as_str().unwrap(),
        email,
        "Ada Lovelace",
    );
    let callback = Request::get(format!(
        "/platform/v1/auth/oidc/callback?code={}&state={}",
        auth.code, auth.state
    ))
    .header(header::COOKIE, format!("__Host-kaeso_oidc={binding}"))
    .body(Body::empty())
    .unwrap();
    let r = app.raw(callback).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.headers);
    r.cookie().unwrap()
}

#[tokio::test]
async fn security_settings_start_open_and_follow_each_switch() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = security(&owner);
    let cookie = owner.cookie.as_str();

    let r = app.get(&base, cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["require_2fa"], false);
    assert_eq!(r.body["require_sso"], false);
    assert_eq!(r.body["max_key_lifetime_days"], Value::Null);
    assert_eq!(r.body["sso"], Value::Null);
    assert_eq!(r.body["sso_default_role"], Value::Null);

    let r = put(
        &app,
        cookie,
        &format!("{base}/require-2fa"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["require_2fa"], true);

    for days in [0, 3651] {
        let r = put(
            &app,
            cookie,
            &format!("{base}/key-policy"),
            json!({ "max_key_lifetime_days": days }),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{days}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "max_key_lifetime_days");
    }
    let r = put(
        &app,
        cookie,
        &format!("{base}/key-policy"),
        json!({ "max_key_lifetime_days": 90 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["max_key_lifetime_days"], 90);
    let r = put(
        &app,
        cookie,
        &format!("{base}/key-policy"),
        json!({ "max_key_lifetime_days": null }),
    )
    .await;
    assert_eq!(r.body["max_key_lifetime_days"], Value::Null);

    let r = put(
        &app,
        cookie,
        &format!("{base}/require-sso"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = delete(&app, cookie, &format!("{base}/sso")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), cookie)
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .collect();
    assert!(actions.contains(&"security.require_2fa"), "{actions:?}");
    assert!(actions.contains(&"security.key_policy"), "{actions:?}");
    s.cleanup().await;
}

#[tokio::test]
async fn an_sso_connection_is_checked_before_it_is_stored() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let sso = format!("{}/sso", security(&owner));
    let valid = json!({
        "issuer": idp.issuer(),
        "client_id": "kaeso",
        "client_secret": "client-secret",
        "domains": ["acme.example"],
    });
    let with = |key: &str, value: Value| {
        let mut body = valid.clone();
        body[key] = value;
        body
    };
    let refused = [
        (with("issuer", json!("not a url")), "issuer"),
        (with("issuer", json!("ftp://login.acme.example")), "issuer"),
        (with("client_id", json!("  ")), "client_id"),
        (with("domains", json!(["not a domain"])), "domains[0]"),
        (with("domains", json!([])), "domains"),
        (with("default_role", json!("owner")), "default_role"),
        (with("client_secret", Value::Null), "client_secret"),
        (with("issuer", json!("http://127.0.0.1:9")), "issuer"),
    ];
    for (body, param) in refused {
        let r = put(&app, &owner.cookie, &sso, body.clone()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{body}");
    }
    let r = app.get(&security(&owner), &owner.cookie).await;
    assert_eq!(r.body["sso"], Value::Null);
    s.cleanup().await;
}

#[tokio::test]
async fn an_sso_connection_is_stored_kept_required_and_removed() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let base = security(&owner);
    let sso = format!("{base}/sso");
    let cookie = owner.cookie.as_str();

    let r = put(
        &app,
        cookie,
        &sso,
        json!({
            "issuer": format!("{}/", idp.issuer()),
            "client_id": " kaeso ",
            "client_secret": "client-secret",
            "domains": ["Acme.example", "acme.example", " eu.acme.example "],
            "default_role": "developer",
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso"]["issuer"], idp.issuer());
    assert_eq!(r.body["sso"]["client_id"], "kaeso");
    assert_eq!(
        r.body["sso"]["domains"],
        json!(["acme.example", "eu.acme.example"])
    );
    assert!(
        r.body["sso"]["redirect_url"]
            .as_str()
            .unwrap()
            .starts_with(common::ORIGIN)
    );
    assert_eq!(r.body["sso_default_role"]["id"], "developer");
    assert_eq!(r.body["sso_default_preset"], "developer");

    let role = app
        .post(
            &format!("/orgs/{}/roles", owner.org_id),
            json!({ "scope": "org", "name": "SSO reader", "rights": ["org.view"], "project_rights": [] }),
            Some(cookie),
        )
        .await;
    assert_eq!(role.status, StatusCode::CREATED, "{:?}", role.body);
    let role_id = role.body["id"].as_str().unwrap();
    let r = put(
        &app,
        cookie,
        &sso,
        json!({ "issuer": idp.issuer(), "client_id": "kaeso", "domains": ["acme.example"], "default_role": role_id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso_default_role"]["id"], role_id);
    assert_eq!(r.body["sso_default_role"]["name"], "SSO reader");
    assert_eq!(r.body["sso_default_preset"], "custom");

    let other = app
        .post("/me/orgs", json!({ "name": "Other" }), Some(cookie))
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.body);
    let other_id = other.body["id"].as_str().unwrap();
    let r = put(
        &app,
        cookie,
        &format!("/orgs/{other_id}/security/sso"),
        json!({ "issuer": idp.issuer(), "client_id": "kaeso", "client_secret": "client-secret", "domains": ["acme.example"] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);

    let r = put(
        &app,
        cookie,
        &sso,
        json!({ "issuer": idp.issuer(), "client_id": "kaeso", "domains": ["acme.example"], "default_role": "admin" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = put(
        &app,
        cookie,
        &format!("{base}/require-sso"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["require_sso"], true);
    let r = app.get(&base, cookie).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);

    let admin = sso_session(&app, &idp, "ada@acme.example").await;
    let r = delete(&app, &admin, &sso).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = put(
        &app,
        &admin,
        &format!("{base}/require-sso"),
        json!({ "enabled": false }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["require_sso"], false);
    let r = delete(&app, &admin, &sso).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);

    let r = app.get(&base, cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso"], Value::Null);
    assert_eq!(r.body["sso_default_role"], Value::Null);
    s.cleanup().await;
}
