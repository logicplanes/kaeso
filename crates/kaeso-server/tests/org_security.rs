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

    let r = put(
        &app,
        cookie,
        &format!("{base}/require-2fa"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["require_2fa"], true);
    let r = app.get(&base, cookie).await;
    assert_eq!(
        r.body["error"]["message"],
        "This organization requires two-factor authentication; set it up, then sign in again.",
        "a sign-in without a second factor no longer opens the org"
    );

    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_events WHERE scope = 'org'")
            .fetch_all(&app.state.pg)
            .await
            .unwrap();
    assert!(
        actions.iter().any(|a| a == "security.require_2fa"),
        "{actions:?}"
    );
    assert!(
        actions.iter().any(|a| a == "security.key_policy"),
        "{actions:?}"
    );
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

#[tokio::test]
async fn an_internationalised_sso_domain_is_one_domain_however_it_is_typed() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let base = security(&owner);
    let cookie = owner.cookie.as_str();
    let r = put(
        &app,
        cookie,
        &format!("{base}/sso"),
        json!({
            "issuer": idp.issuer(),
            "client_id": "kaeso",
            "client_secret": "client-secret",
            "domains": ["BÜCHER.example", "xn--bcher-kva.example", "@bücher.example"],
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso"]["domains"], json!(["bücher.example"]));
    let stored: Vec<String> = sqlx::query_scalar("SELECT domain FROM org_oidc_domains")
        .fetch_all(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(stored, ["xn--bcher-kva.example"]);
    let r = put(
        &app,
        cookie,
        &format!("{base}/require-sso"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    for typed in [
        "ada@bücher.example",
        "ada@BÜCHER.EXAMPLE",
        "ada@xn--bcher-kva.example",
        "ada@Bu\u{308}cher.example",
    ] {
        let m = app
            .post("/auth/methods", json!({ "email": typed }), None)
            .await;
        assert_eq!(m.status, StatusCode::OK, "{:?}", m.body);
        assert_eq!(m.body["sso_required"], true, "{typed}");
        assert_eq!(m.body["password"], false, "{typed}");
        assert_eq!(m.body["magic_link"], false, "{typed}");
    }
    s.cleanup().await;
}

#[tokio::test]
async fn an_sso_connection_names_its_button_and_its_sign_in_link() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let sso = format!("{}/sso", security(&owner));
    let cookie = owner.cookie.as_str();
    let connection = |extra: Value| {
        let mut body = json!({
            "issuer": idp.issuer(),
            "client_id": "kaeso",
            "client_secret": "client-secret",
            "domains": ["acme.example"],
        });
        if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
            body.extend(extra.clone());
        }
        body
    };

    let r = put(&app, cookie, &sso, connection(json!({}))).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso"]["name"], Value::Null);
    assert_eq!(r.body["sso"]["auto_redirect"], false);
    assert_eq!(
        r.body["sso"]["sign_in_url"],
        format!("{}/sign-in?org={}", common::ORIGIN, owner.org_id)
    );

    let r = put(
        &app,
        cookie,
        &sso,
        connection(json!({ "name": "  Acme ID ", "auto_redirect": true })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["sso"]["name"], "Acme ID");
    assert_eq!(r.body["sso"]["auto_redirect"], true);
    let r = put(&app, cookie, &sso, connection(json!({}))).await;
    assert_eq!(
        (&r.body["sso"]["name"], &r.body["sso"]["auto_redirect"]),
        (&json!("Acme ID"), &json!(true)),
        "leaving both out keeps them"
    );
    let audited: Value = sqlx::query_scalar(
        "SELECT after FROM audit_events WHERE action = 'security.sso_update' ORDER BY at DESC, id DESC LIMIT 1",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(
        (&audited["name"], &audited["auto_redirect"]),
        (&json!("Acme ID"), &json!(true))
    );

    let r = put(
        &app,
        cookie,
        &sso,
        connection(json!({ "name": "x".repeat(51) })),
    )
    .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("name"))
    );
    let r = put(
        &app,
        cookie,
        &sso,
        connection(json!({ "name": "", "auto_redirect": false })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (&r.body["sso"]["name"], &r.body["sso"]["auto_redirect"]),
        (&Value::Null, &json!(false))
    );
    s.cleanup().await;
}

#[tokio::test]
async fn only_a_console_session_changes_single_sign_on() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let base = security(&owner);
    let created = app
        .post(
            &format!("/orgs/{}/platform-keys", owner.org_id),
            json!({ "name": "CI", "expires_in_days": 30 }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let key = created.body["secret"].as_str().unwrap().to_owned();
    let connection = json!({
        "issuer": idp.issuer(),
        "client_id": "kaeso",
        "client_secret": "client-secret",
        "domains": ["acme.example"],
    });
    for (method, path, body) in [
        (Method::PUT, format!("{base}/sso"), Some(connection.clone())),
        (Method::DELETE, format!("{base}/sso"), None),
        (
            Method::PUT,
            format!("{base}/require-sso"),
            Some(json!({ "enabled": true })),
        ),
    ] {
        let r = app.with_key(method.clone(), &path, body, &key).await;
        assert_eq!(
            (r.status, r.body["error"]["message"].as_str()),
            (
                StatusCode::FORBIDDEN,
                Some("Only a console session can change SSO; API keys can't.")
            ),
            "{method} {path}"
        );
    }
    assert_eq!(
        app.with_key(Method::GET, &base, None, &key).await.status,
        StatusCode::OK
    );
    app.clock.advance(time::Duration::minutes(16));
    let r = put(&app, &owner.cookie, &format!("{base}/sso"), connection).await;
    assert_eq!(r.error_type(), "recent_sign_in_required", "{:?}", r.body);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM org_oidc")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(stored, 0);
    s.cleanup().await;
}
