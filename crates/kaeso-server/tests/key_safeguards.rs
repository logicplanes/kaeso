#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["is_default"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn new_key(app: &App, base: &str, cookie: &str, body: Value) -> (String, String) {
    let r = app
        .post(&format!("{base}/keys"), body, Some(cookie))
        .await
        .expect_status(StatusCode::CREATED);
    (
        r.body["id"].as_str().unwrap().to_owned(),
        r.body["secret"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn a_key_someone_else_disabled_stays_off_for_its_owner() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let (dev, _) = common::join(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let (id, _) = new_key(&app, &base, &dev, json!({ "name": "laptop" })).await;

    app.post(
        &format!("{base}/keys/{id}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let r = app
        .post(&format!("/me/keys/{id}/enable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "Someone else disabled this key; ask a holder of `keys.all.manage` to enable it."
    );
    app.post(
        &format!("{base}/keys/{id}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    app.post(&format!("/me/keys/{id}/disable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::OK);
    let r = app
        .post(&format!("/me/keys/{id}/enable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["state"], "active");
    s.cleanup().await;
}

#[tokio::test]
async fn keys_cannot_undo_what_a_person_did_or_mint_platform_keys() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}");
    let (_, script) = new_key(
        &app,
        &base,
        &owner.cookie,
        json!({ "name": "script", "access": "platform" }),
    )
    .await;
    let (target, _) = new_key(&app, &base, &owner.cookie, json!({ "name": "target" })).await;

    let r = app
        .with_key(
            Method::POST,
            &format!("{base}/keys"),
            Some(json!({ "name": "copy", "access": "platform" })),
            &script,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "Keys can't create keys with platform access; use a console session."
    );
    app.with_key(
        Method::POST,
        &format!("{base}/keys"),
        Some(json!({ "name": "inference only" })),
        &script,
    )
    .await
    .expect_status(StatusCode::CREATED);

    app.post(
        &format!("{base}/keys/{target}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let refused = app
        .with_key(
            Method::POST,
            &format!("{base}/keys/{target}/enable"),
            Some(json!({})),
            &script,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        refused.body["error"]["message"],
        "A person disabled this key, so only a person can enable it; use a console session."
    );
    app.post(
        &format!("{base}/keys/{target}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.with_key(
        Method::POST,
        &format!("{base}/keys/{target}/disable"),
        Some(json!({})),
        &script,
    )
    .await
    .expect_status(StatusCode::OK);
    app.with_key(
        Method::POST,
        &format!("{base}/keys/{target}/enable"),
        Some(json!({})),
        &script,
    )
    .await
    .expect_status(StatusCode::OK);

    let platform = format!("/orgs/{org}/platform-keys");
    let org_key = |name: &'static str| {
        let app = &app;
        let platform = platform.clone();
        let cookie = owner.cookie.clone();
        async move {
            let r = app
                .post(&platform, json!({ "name": name }), Some(&cookie))
                .await
                .expect_status(StatusCode::CREATED);
            (
                r.body["id"].as_str().unwrap().to_owned(),
                r.body["secret"].as_str().unwrap().to_owned(),
            )
        }
    };
    let (_, automation) = org_key("automation").await;
    let (other, _) = org_key("other").await;
    app.post(
        &format!("{platform}/{other}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let refused = app
        .with_key(
            Method::POST,
            &format!("{platform}/{other}/enable"),
            Some(json!({})),
            &automation,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        refused.body["error"]["message"],
        "A person disabled this key, so only a person can enable it; use a console session."
    );
    app.post(
        &format!("{platform}/{other}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn instance_changes_to_people_need_a_recent_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, grace) = common::join(
        &app,
        &owner,
        json!({ "email": "grace@example.com", "role": "member" }),
        "Grace",
    )
    .await;
    app.clock.advance(time::Duration::minutes(16));
    for (method, path, body) in [
        (
            Method::PUT,
            format!("/instance/users/{grace}/instance-admin"),
            Some(json!({ "is_instance_admin": true })),
        ),
        (
            Method::POST,
            format!("/instance/users/{grace}/reset-2fa"),
            Some(json!({})),
        ),
        (Method::DELETE, format!("/instance/users/{grace}"), None),
        (
            Method::POST,
            format!("/instance/orgs/{}/owner", owner.org_id),
            Some(json!({ "user_id": grace })),
        ),
    ] {
        let r = app.call(method, &path, body, Some(&owner.cookie)).await;
        assert_eq!(
            (r.status, r.error_type()),
            (StatusCode::FORBIDDEN, "recent_sign_in_required"),
            "{path}: {:?}",
            r.body
        );
    }
    let member = app
        .get(
            &format!("/orgs/{}/members/{grace}", owner.org_id),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(member.body["grant"]["role"]["id"], "member");
    s.cleanup().await;
}
