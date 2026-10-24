#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner, PASSWORD};
use kaeso_core::clock::Clock;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"][0]["id"].as_str().unwrap().to_owned()
}

async fn created(app: &App, path: &str, body: Value, cookie: &str) -> Value {
    let r = app.post(path, body, Some(cookie)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{path}: {:?}", r.body);
    r.body
}

async fn fresh_sign_in(app: &App, email: &str) -> String {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await
    .cookie()
    .unwrap()
}

#[tokio::test]
async fn a_rotated_key_keeps_its_settings_and_the_old_key_works_until_the_grace_period_ends() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}");
    let model = created(
        &app,
        &format!("{base}/models"),
        json!({ "provider": "openai", "model_id": "gpt-4o" }),
        &owner.cookie,
    )
    .await["id"]
        .clone();
    let old = created(
        &app,
        &format!("{base}/keys"),
        json!({
            "name": "checkout-web",
            "access": "both",
            "expires_in_days": 90,
            "models": [model],
            "limits": [{ "kind": "request_rate", "period": "minute", "amount": 5 }],
        }),
        &owner.cookie,
    )
    .await;
    let old_id = old["id"].as_str().unwrap().to_owned();
    let old_secret = old["secret"].as_str().unwrap().to_owned();

    let rotated = created(
        &app,
        &format!("{base}/keys/{old_id}/rotate"),
        json!({ "grace_hours": 24, "expires_in_days": 30 }),
        &owner.cookie,
    )
    .await;
    let new_id = rotated["id"].as_str().unwrap().to_owned();
    let new_secret = rotated["secret"].as_str().unwrap().to_owned();
    assert_ne!(new_id, old_id);
    assert_ne!(new_secret, old_secret);
    assert_eq!(rotated["name"], "checkout-web");
    assert_eq!(rotated["access"], "both");
    assert_eq!(rotated["principal"]["id"], owner.user_id);
    assert_eq!(rotated["rotated_from"], old_id.as_str());
    assert!(rotated["replaced_by"].is_null(), "{rotated:?}");

    let now = app.clock.now();
    let expires = |v: &Value| {
        time::OffsetDateTime::parse(
            v["expires_at"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    };
    assert_eq!(expires(&rotated), now + time::Duration::days(30));
    let before = app
        .get(&format!("{base}/keys/{old_id}"), &owner.cookie)
        .await;
    assert_eq!(before.body["replaced_by"], new_id.as_str());
    assert_eq!(before.body["state"], "active");
    assert_eq!(expires(&before.body), now + time::Duration::hours(24));

    let models = app
        .get(&format!("{base}/keys/{new_id}/models"), &owner.cookie)
        .await;
    assert_eq!(models.body["models"], json!([model]), "{:?}", models.body);
    let limits = app
        .get(&format!("{base}/limits?key={new_id}"), &owner.cookie)
        .await;
    let limits = limits.body["data"].as_array().unwrap();
    assert_eq!(limits.len(), 1, "{limits:?}");
    assert_eq!(limits[0]["kind"], "request_rate");
    assert_eq!(limits[0]["amount"], "5");

    let audit = app
        .get(
            &format!("/orgs/{org}/audit-log?action=key.rotate"),
            &owner.cookie,
        )
        .await;
    let event = &audit.body["data"][0];
    assert_eq!(event["target_id"], old_id.as_str(), "{event:?}");
    assert_eq!(event["after"]["replaced_by"], new_id.as_str());

    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 0 key expiry warnings",
        "a replaced key's end is no surprise"
    );

    assert_eq!(
        app.with_key(Method::GET, &base, None, &old_secret)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.with_key(Method::GET, &base, None, &new_secret)
            .await
            .status,
        StatusCode::OK
    );
    app.clock.advance(time::Duration::hours(24));
    let refused = app.with_key(Method::GET, &base, None, &old_secret).await;
    assert_eq!(
        refused.status,
        StatusCode::UNAUTHORIZED,
        "{:?}",
        refused.body
    );
    assert_eq!(
        app.with_key(Method::GET, &base, None, &new_secret)
            .await
            .status,
        StatusCode::OK
    );

    let cookie = fresh_sign_in(&app, "owner@example.com").await;
    let again = app
        .post(
            &format!("{base}/keys/{old_id}/rotate"),
            json!({ "grace_hours": 0, "expires_in_days": 30 }),
            Some(&cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{:?}", again.body);
    assert_eq!(
        again.body["error"]["message"],
        "This key was rotated already; rotate the key that replaced it."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_rotation_without_a_grace_period_stops_the_old_key_at_once() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let old = created(
        &app,
        &format!("{base}/keys"),
        json!({ "name": "script", "access": "platform", "expires_in_days": 7 }),
        &owner.cookie,
    )
    .await;
    let old_secret = old["secret"].as_str().unwrap();
    assert_eq!(
        app.with_key(Method::GET, &base, None, old_secret)
            .await
            .status,
        StatusCode::OK
    );
    let rotate = format!("{base}/keys/{}/rotate", old["id"].as_str().unwrap());
    let r = app
        .post(&rotate, json!({ "grace_hours": 2 }), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "grace_hours");
    let r = app
        .post(&rotate, json!({ "grace_hours": 0 }), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert!(r.body["expires_at"].is_null(), "{:?}", r.body);
    let new_secret = r.body["secret"].as_str().unwrap();
    let refused = app.with_key(Method::GET, &base, None, old_secret).await;
    assert_eq!(
        refused.status,
        StatusCode::UNAUTHORIZED,
        "{:?}",
        refused.body
    );
    assert_eq!(
        app.with_key(Method::GET, &base, None, new_secret)
            .await
            .status,
        StatusCode::OK
    );
    s.cleanup().await;
}

#[tokio::test]
async fn only_a_keys_owner_rotates_it_and_a_service_accounts_key_needs_its_rights() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}");
    let (dev, dev_id) = common::join(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "preset": "developer" }),
        "Dev",
    )
    .await;
    let devs = created(
        &app,
        &format!("{base}/keys"),
        json!({ "name": "dev-key" }),
        &dev,
    )
    .await;
    let devs_id = devs["id"].as_str().unwrap();
    let r = app
        .post(
            &format!("{base}/keys/{devs_id}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "Only a key's owner can rotate it; the new key acts with the owner's rights."
    );
    let r = app
        .post(
            &format!("{base}/keys/{devs_id}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let second = r.body["id"].as_str().unwrap().to_owned();

    app.call(
        Method::PATCH,
        &format!("{base}/members/{dev_id}"),
        Some(json!({ "role": "developer", "rights_removed": ["keys.own.manage"] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let r = app
        .post(
            &format!("{base}/keys/{second}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(r.body["error"]["right"], "keys.own.manage");

    let account = created(
        &app,
        &format!("{base}/service-accounts"),
        json!({ "name": "billing-sync", "preset": "viewer" }),
        &owner.cookie,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let svc_key = created(
        &app,
        &format!("{base}/service-accounts/{account}/keys"),
        json!({ "name": "sync" }),
        &owner.cookie,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = app
        .post(
            &format!("{base}/keys/{svc_key}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = app
        .post(
            &format!("{base}/service-accounts/{account}/keys/{svc_key}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(r.body["error"]["right"], "service_accounts.keys.create");
    let r = app
        .post(
            &format!("{base}/service-accounts/{account}/keys/{devs_id}/rotate"),
            json!({ "grace_hours": 1 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    let r = app
        .post(
            &format!("{base}/service-accounts/{account}/keys/{svc_key}/rotate"),
            json!({ "grace_hours": 24 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["principal"]["kind"], "service_account");
    assert_eq!(r.body["principal"]["id"], account.as_str());
    assert_eq!(r.body["rotated_from"], svc_key.as_str());

    let automation = created(
        &app,
        &format!("/orgs/{org}/platform-keys"),
        json!({ "name": "automation" }),
        &owner.cookie,
    )
    .await["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let replacement = r.body["id"].as_str().unwrap();
    let r = app
        .with_key(
            Method::POST,
            &format!("{base}/service-accounts/{account}/keys/{replacement}/rotate"),
            Some(json!({ "grace_hours": 24, "expires_in_days": 90 })),
            &automation,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["rotated_from"], replacement);
    s.cleanup().await;
}

#[tokio::test]
async fn a_platform_key_rotates_in_a_session_with_a_recent_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let base = format!("/orgs/{org}/platform-keys");
    let old = created(
        &app,
        &base,
        json!({ "name": "terraform", "expires_in_days": 90 }),
        &owner.cookie,
    )
    .await;
    let old_id = old["id"].as_str().unwrap().to_owned();
    let old_secret = old["secret"].as_str().unwrap().to_owned();
    let projects = format!("/orgs/{org}/projects");

    let r = app
        .with_key(
            Method::POST,
            &format!("{base}/{old_id}/rotate"),
            Some(json!({ "grace_hours": 1 })),
            &old_secret,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);

    let rotated = created(
        &app,
        &format!("{base}/{old_id}/rotate"),
        json!({ "grace_hours": 1, "expires_in_days": 30 }),
        &owner.cookie,
    )
    .await;
    assert_eq!(rotated["name"], "terraform");
    assert_eq!(rotated["rotated_from"], old_id.as_str());
    let new_secret = rotated["secret"].as_str().unwrap().to_owned();
    assert!(new_secret.starts_with("sk-kso-org-"));
    let before = app.get(&format!("{base}/{old_id}"), &owner.cookie).await;
    assert_eq!(before.body["replaced_by"], rotated["id"]);
    assert_eq!(
        app.with_key(Method::GET, &projects, None, &old_secret)
            .await
            .status,
        StatusCode::OK
    );
    let audit = app
        .get(
            &format!("/orgs/{org}/audit-log?action=platform_key.rotate"),
            &owner.cookie,
        )
        .await;
    assert_eq!(audit.body["data"][0]["target_id"], old_id.as_str());

    app.clock.advance(time::Duration::hours(1));
    assert_eq!(
        app.with_key(Method::GET, &projects, None, &old_secret)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.with_key(Method::GET, &projects, None, &new_secret)
            .await
            .status,
        StatusCode::OK
    );
    let r = app
        .post(
            &format!("{base}/{}/rotate", rotated["id"].as_str().unwrap()),
            json!({ "grace_hours": 0, "expires_in_days": 30 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.error_type(), "recent_sign_in_required", "{:?}", r.body);
    s.cleanup().await;
}
