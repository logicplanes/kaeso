#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

use common::PASSWORD;

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"][0]["id"].as_str().unwrap().to_owned()
}

async fn key(app: &App, owner: &Owner, project: &str, body: Value) -> (String, Value) {
    let r = app
        .post(
            &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
            body,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (
        r.body["secret"].as_str().unwrap().to_owned(),
        r.body.clone(),
    )
}

#[tokio::test]
async fn keys_show_their_secret_once_and_follow_the_expiry_rules() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let (secret, created) = key(
        &app,
        &owner,
        &project,
        json!({ "name": "bot", "expires_in_days": 30 }),
    )
    .await;
    assert!(
        secret.starts_with("sk-kso-") && secret.len() == 55,
        "{secret}"
    );
    assert_eq!(created["access"], "inference");
    assert_eq!(created["state"], "active");
    assert_eq!(created["principal"]["id"], owner.user_id);
    let redacted = created["redacted"].as_str().unwrap();
    assert!(redacted.starts_with(&secret[..13]) && redacted.ends_with(&secret[secret.len() - 4..]));
    let list = app
        .get(
            &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert!(!list.body.to_string().contains(&secret));
    let (stored,): (i64,) = sqlx::query_as("SELECT count(*) FROM api_keys WHERE secret_hash = $1")
        .bind(secret.as_bytes())
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(stored, 0);

    let base = format!("/orgs/{}/projects/{project}/keys", owner.org_id);
    assert_eq!(
        app.post(
            &base,
            json!({ "name": "x", "expires_in_days": 5 }),
            Some(&owner.cookie)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    app.call(
        Method::PUT,
        &format!("/orgs/{}/security/key-policy", owner.org_id),
        Some(json!({ "max_key_lifetime_days": 30 })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let r = app
        .post(
            &base,
            json!({ "name": "x", "expires_in_days": 90 }),
            Some(&owner.cookie),
        )
        .await;
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("30 days"),
        "{:?}",
        r.body
    );
    let r = app
        .post(&base, json!({ "name": "x" }), Some(&owner.cookie))
        .await;
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("never"),
        "{:?}",
        r.body
    );
    s.cleanup().await;
}

#[tokio::test]
async fn platform_access_needs_its_right_and_a_recent_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}/keys", owner.org_id);
    let (dev, _) = common::join(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "preset": "developer" }),
        "Dev",
    )
    .await;
    assert_eq!(
        app.post(&base, json!({ "name": "inf" }), Some(&dev))
            .await
            .status,
        StatusCode::CREATED
    );
    let r = app
        .post(
            &base,
            json!({ "name": "plat", "access": "platform" }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("keys.platform_access.grant")
    );
    assert_eq!(
        app.post(
            &base,
            json!({ "name": "plat", "access": "both" }),
            Some(&owner.cookie)
        )
        .await
        .status,
        StatusCode::CREATED
    );
    app.clock.advance(time::Duration::minutes(16));
    let r = app
        .post(
            &base,
            json!({ "name": "late", "access": "platform" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.error_type(), "recent_sign_in_required");
    s.cleanup().await;
}

#[tokio::test]
async fn keys_reach_only_what_their_kind_and_principal_allow() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let other = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Other" }),
            Some(&owner.cookie),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (inference, _) = key(&app, &owner, &project, json!({ "name": "inf" })).await;
    let (platform, _) = key(
        &app,
        &owner,
        &project,
        json!({ "name": "plat", "access": "platform" }),
    )
    .await;

    let r = app
        .with_key(
            Method::GET,
            &format!("/orgs/{org}/projects/{project}"),
            None,
            &inference,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no Platform API access")
    );
    let r = app
        .with_key(
            Method::GET,
            &format!("/orgs/{org}/projects/{project}/members"),
            None,
            &platform,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let created = app
        .with_key(
            Method::POST,
            &format!("/orgs/{org}/projects/{project}/service-accounts"),
            Some(json!({ "name": "by-key", "preset": "viewer" })),
            &platform,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(
        app.with_key(
            Method::GET,
            &format!("/orgs/{org}/projects/{other}"),
            None,
            &platform
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.with_key(
            Method::GET,
            &format!("/orgs/{org}/members"),
            None,
            &platform
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.with_key(Method::GET, "/me", None, &platform)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.with_key(Method::GET, "/instance/users", None, &platform)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let log = app
        .get(
            &format!("/orgs/{org}/audit-log?action=service_account."),
            &owner.cookie,
        )
        .await;
    assert_eq!(log.body["data"][0]["actor"]["kind"], "api_key");

    let r = app
        .post(
            &format!("/orgs/{org}/platform-keys"),
            json!({ "name": "terraform" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let org_key = r.body["secret"].as_str().unwrap().to_owned();
    assert!(org_key.starts_with("sk-kso-org-"));
    let r = app
        .with_key(
            Method::POST,
            &format!("/orgs/{org}/projects"),
            Some(json!({ "name": "By org key" })),
            &org_key,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(
        app.with_key(
            Method::GET,
            &format!("/orgs/{org}/projects/{other}/keys"),
            None,
            &org_key
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        app.with_key(Method::DELETE, &format!("/orgs/{org}"), None, &org_key)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.with_key(
            Method::POST,
            &format!("/orgs/{org}/platform-keys"),
            Some(json!({ "name": "x" })),
            &org_key
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let elsewhere = app
        .post(
            "/me/orgs",
            json!({ "name": "Elsewhere" }),
            Some(&owner.cookie),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        app.with_key(Method::GET, &format!("/orgs/{elsewhere}"), None, &org_key)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let mut tampered = platform.clone();
    tampered.pop();
    tampered.push(if platform.ends_with('a') { 'b' } else { 'a' });
    assert_eq!(
        app.with_key(
            Method::GET,
            &format!("/orgs/{org}/projects/{project}"),
            None,
            &tampered
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    s.cleanup().await;
}

#[tokio::test]
async fn disabled_keys_stop_on_every_instance_within_five_seconds() {
    let s = TestServices::start().await;
    let a = App::start(&s).await;
    let b = a.second_instance(&s).await;
    let owner = a.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&a, &owner).await;
    let (platform, created) = key(
        &a,
        &owner,
        &project,
        json!({ "name": "plat", "access": "platform" }),
    )
    .await;
    let path = format!("/orgs/{org}/projects/{project}");
    assert_eq!(
        b.with_key(Method::GET, &path, None, &platform).await.status,
        StatusCode::OK
    );
    let id = created["id"].as_str().unwrap();
    let r = a
        .post(
            &format!("/orgs/{org}/projects/{project}/keys/{id}/disable"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["state"], "disabled");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while b.with_key(Method::GET, &path, None, &platform).await.status != StatusCode::UNAUTHORIZED {
        assert!(
            tokio::time::Instant::now() < deadline,
            "instance B still accepts the disabled key"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let r = b.with_key(Method::GET, &path, None, &platform).await;
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("disabled")
    );
    a.post(
        &format!("/orgs/{org}/projects/{project}/keys/{id}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while b.with_key(Method::GET, &path, None, &platform).await.status != StatusCode::OK {
        assert!(
            tokio::time::Instant::now() < deadline,
            "instance B still refuses the enabled key"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    s.cleanup().await;
}

#[tokio::test]
async fn expired_archived_and_orphaned_keys_are_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let path = format!("/orgs/{org}/projects/{project}");
    let (short, _) = key(
        &app,
        &owner,
        &project,
        json!({ "name": "short", "access": "platform", "expires_in_days": 1 }),
    )
    .await;
    let (other, created) = key(
        &app,
        &owner,
        &project,
        json!({ "name": "other", "access": "platform" }),
    )
    .await;
    assert_eq!(
        app.with_key(Method::GET, &path, None, &short).await.status,
        StatusCode::OK
    );
    app.clock.advance(time::Duration::days(1));
    let r = app.with_key(Method::GET, &path, None, &short).await;
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("expired"),
        "{:?}",
        r.body
    );
    let listed = app
        .get(
            &format!("{path}/keys"),
            &app.post(
                "/auth/password",
                json!({ "email": "owner@example.com", "password": PASSWORD }),
                None,
            )
            .await
            .cookie()
            .unwrap(),
        )
        .await;
    assert!(
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["state"] == "expired"),
        "expired keys stay listed"
    );
    let fresh = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let id = created["id"].as_str().unwrap();
    app.post(
        &format!("{path}/keys/{id}/archive"),
        json!({}),
        Some(&fresh),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(
        app.post(&format!("{path}/keys/{id}/enable"), json!({}), Some(&fresh))
            .await
            .status,
        StatusCode::CONFLICT
    );
    assert!(
        app.with_key(Method::GET, &path, None, &other).await.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("archived")
    );
    let fresh_owner = Owner {
        cookie: fresh.clone(),
        ..owner
    };
    let (live, _) = key(
        &app,
        &fresh_owner,
        &project,
        json!({ "name": "live", "access": "platform" }),
    )
    .await;
    app.call(Method::DELETE, &format!("/orgs/{org}"), None, Some(&fresh))
        .await
        .expect_status(StatusCode::OK);
    let r = app.with_key(Method::GET, &path, None, &live).await;
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("scheduled for deletion"),
        "{:?}",
        r.body
    );
    s.cleanup().await;
}

#[tokio::test]
async fn users_manage_their_own_keys_and_get_expiry_warnings() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let (_, created) = key(
        &app,
        &owner,
        &project,
        json!({ "name": "soon", "expires_in_days": 7 }),
    )
    .await;
    let mine = app.get("/me/keys", &owner.cookie).await;
    assert_eq!(mine.body["data"].as_array().unwrap().len(), 1);
    let id = created["id"].as_str().unwrap();
    let r = app
        .post(
            &format!("/me/keys/{id}/disable"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["state"], "disabled");
    app.post(
        &format!("/me/keys/{id}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 1 key expiry warnings"
    );
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 0 key expiry warnings"
    );
    app.clock
        .advance(time::Duration::days(6) + time::Duration::hours(1));
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 1 key expiry warnings"
    );
    app.wait_for_mail("owner@example.com", 2).await;
    assert_eq!(
        app.mail
            .to("owner@example.com")
            .iter()
            .filter(|m| m.kind.as_str() == "key_expiry_warning")
            .count(),
        2
    );
    let fresh = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    app.call(
        Method::PUT,
        "/me/notifications",
        Some(json!({ "budget_alerts": true, "key_expiry": false })),
        Some(&fresh),
    )
    .await
    .expect_status(StatusCode::OK);
    key(
        &app,
        &Owner {
            cookie: fresh.clone(),
            ..owner
        },
        &project,
        json!({ "name": "quiet", "expires_in_days": 1 }),
    )
    .await;
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 0 key expiry warnings"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn own_keys_follow_the_orgs_sign_in_requirements() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let (_, created) = key(&app, &owner, &project, json!({ "name": "bot" })).await;
    let id = created["id"].as_str().unwrap();
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/require-2fa", owner.org_id),
            Some(json!({ "enabled": true })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let password = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    for action in ["disable", "enable", "archive"] {
        let r = app
            .post(
                &format!("/me/keys/{id}/{action}"),
                json!({}),
                Some(&password),
            )
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{action}: {:?}", r.body);
        assert!(
            r.body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("two-factor"),
            "{:?}",
            r.body
        );
    }
    let r = app.get(&format!("/me/keys/{id}"), &password).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let untouched: bool =
        sqlx::query_scalar("SELECT disabled_at IS NULL AND archived_at IS NULL FROM api_keys")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert!(untouched);
    s.cleanup().await;
}
