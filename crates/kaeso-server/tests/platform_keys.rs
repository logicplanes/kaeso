mod common;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

#[tokio::test]
async fn an_org_platform_key_is_created_used_disabled_enabled_and_archived() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = format!("/orgs/{}/platform-keys", owner.org_id);

    let created = app
        .post(
            &base,
            json!({ "name": "CI", "expires_in_days": 30 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let secret = created.body["secret"].as_str().unwrap().to_owned();
    let id = created.body["key"]["id"]
        .as_str()
        .or_else(|| created.body["id"].as_str())
        .unwrap()
        .to_owned();

    let listed = app.get(&base, &owner.cookie).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.body);
    assert_eq!(listed.body["data"].as_array().unwrap().len(), 1);
    let one = app.get(&format!("{base}/{id}"), &owner.cookie).await;
    assert_eq!(one.status, StatusCode::OK, "{:?}", one.body);
    assert_eq!(one.body["name"], "CI");
    assert!(one.body["expires_at"].is_string(), "{:?}", one.body);

    let projects = format!("/orgs/{}/projects", owner.org_id);
    let r = app.with_key(Method::GET, &projects, None, &secret).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = app
        .post(
            &format!("{base}/{id}/disable"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["state"], "disabled", "{:?}", r.body);
    let r = app.with_key(Method::GET, &projects, None, &secret).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);

    let r = app
        .post(
            &format!("{base}/{id}/enable"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app.with_key(Method::GET, &projects, None, &secret).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = app
        .post(
            &format!("{base}/{id}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["state"], "archived", "{:?}", r.body);
    let r = app
        .post(
            &format!("{base}/{id}/enable"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    let r = app.with_key(Method::GET, &projects, None, &secret).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);

    let active = app.get(&base, &owner.cookie).await;
    assert!(
        active.body["data"].as_array().unwrap().is_empty(),
        "{:?}",
        active.body
    );
    let all = app
        .get(&format!("{base}?include_archived=true"), &owner.cookie)
        .await;
    assert_eq!(
        all.body["data"].as_array().unwrap().len(),
        1,
        "{:?}",
        all.body
    );

    for path in [
        format!("{base}/not-a-key/disable"),
        format!("{base}/opk_0000000000000000000000/archive"),
    ] {
        let r = app.post(&path, json!({}), Some(&owner.cookie)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}: {:?}", r.body);
    }
    s.cleanup().await;
}

#[tokio::test]
async fn platform_keys_are_made_only_in_the_console() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = format!("/orgs/{}/platform-keys", owner.org_id);
    let created = app
        .post(&base, json!({ "name": "first" }), Some(&owner.cookie))
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let secret = created.body["secret"].as_str().unwrap().to_owned();
    let r = app
        .with_key(
            Method::POST,
            &base,
            Some(json!({ "name": "second" })),
            &secret,
        )
        .await;
    assert!(r.status.is_client_error(), "{:?}", r.body);
    assert_ne!(r.status, StatusCode::CREATED);
    s.cleanup().await;
}
