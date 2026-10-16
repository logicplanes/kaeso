#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner, Resp};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const PASSWORD: &str = "correct horse battery";

async fn join(app: &App, owner: &Owner, email: &str) -> (String, String) {
    let r = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": email, "role": "member", "project_roles": [] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    app.wait_for_mail(email, 1).await;
    let accepted = app
        .post(
            "/auth/invites/accept",
            json!({ "token": app.last_token(email), "name": "Grace Hopper", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.body);
    (
        accepted.cookie().unwrap(),
        accepted.body["user"]["id"].as_str().unwrap().to_owned(),
    )
}

async fn project(app: &App, owner: &Owner) -> String {
    let r = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Support bot" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    format!(
        "/orgs/{}/projects/{}",
        owner.org_id,
        r.body["id"].as_str().unwrap()
    )
}

async fn model(app: &App, owner: &Owner, base: &str, model_id: &str) -> String {
    let r = app
        .post(
            &format!("{base}/models"),
            json!({ "provider": "anthropic", "model_id": model_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

async fn patch(app: &App, cookie: &str, path: &str, body: Value) -> Resp {
    app.call(Method::PATCH, path, Some(body), Some(cookie))
        .await
}

#[tokio::test]
async fn project_members_are_added_changed_and_removed() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = project(&app, &owner).await;
    let (grace, grace_id) = join(&app, &owner, "grace@example.com").await;
    let members = format!("{base}/members");

    let r = app.get(&base, &grace).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    let r = app
        .post(
            &members,
            json!({ "user_id": grace_id, "role": "viewer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = app
        .post(
            &members,
            json!({ "user_id": grace_id, "role": "viewer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .post(
            &members,
            json!({ "user_id": grace_id, "role": "viewer", "preset": "admin" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let one = app
        .get(&format!("{members}/{grace_id}"), &owner.cookie)
        .await;
    assert_eq!(one.status, StatusCode::OK, "{:?}", one.body);
    assert_eq!(one.body["grant"]["role"]["id"], "viewer");
    let listed = app.get(&members, &owner.cookie).await;
    assert!(
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["user"]["id"] == grace_id.as_str()),
        "{:?}",
        listed.body
    );
    let r = app.get(&base, &grace).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = patch(
        &app,
        &owner.cookie,
        &format!("{members}/{grace_id}"),
        json!({ "role": "developer", "rights_added": ["project.audit_log.view"] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["grant"]["role"]["id"], "developer");
    assert_eq!(
        r.body["grant"]["rights_added"],
        json!(["project.audit_log.view"])
    );

    let r = patch(
        &app,
        &grace,
        &format!("{members}/{}", owner.user_id),
        json!({ "role": "viewer" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);

    let r = app
        .call(
            Method::DELETE,
            &format!("{members}/{grace_id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    let r = app
        .get(&format!("{members}/{grace_id}"), &owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    let r = app.get(&base, &grace).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn service_accounts_are_created_renamed_regranted_and_archived() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = project(&app, &owner).await;
    let accounts = format!("{base}/service-accounts");

    let r = app
        .post(
            &accounts,
            json!({ "name": " ", "role": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let created = app
        .post(
            &accounts,
            json!({ "name": "log-shipper", "role": "viewer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let id = created.body["id"].as_str().unwrap().to_owned();
    let account = format!("{accounts}/{id}");

    let r = app.get(&account, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["name"], "log-shipper");

    let r = patch(
        &app,
        &owner.cookie,
        &account,
        json!({ "name": "log shipper" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["name"], "log shipper");
    let r = patch(
        &app,
        &owner.cookie,
        &account,
        json!({ "grant": { "role": "developer", "rights_removed": ["keys.own.create"] } }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["grant"]["role"]["id"], "developer");

    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "shipper", "service_account_id": id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);

    let r = app
        .post(
            &format!("{account}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = patch(&app, &owner.cookie, &account, json!({ "name": "again" })).await;
    assert!(r.status.is_client_error(), "{:?}", r.body);
    let active = app.get(&accounts, &owner.cookie).await;
    assert!(
        active.body["data"].as_array().unwrap().is_empty(),
        "{:?}",
        active.body
    );
    let all = app
        .get(&format!("{accounts}?include_archived=true"), &owner.cookie)
        .await;
    assert_eq!(
        all.body["data"].as_array().unwrap().len(),
        1,
        "{:?}",
        all.body
    );
    let r = app
        .get(
            &format!("{accounts}/svc_000000000000000000000000"),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn model_access_narrows_from_member_to_service_account_to_key() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = project(&app, &owner).await;
    let haiku = model(&app, &owner, &base, "claude-haiku-4-5").await;
    let sonnet = model(&app, &owner, &base, "claude-sonnet-4-5").await;
    let (_, grace_id) = join(&app, &owner, "grace@example.com").await;
    let r = app
        .post(
            &format!("{base}/members"),
            json!({ "user_id": grace_id, "role": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);

    let member_models = format!("{base}/members/{grace_id}/models");
    let r = app.get(&member_models, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], true, "{:?}", r.body);
    let r = patch(
        &app,
        &owner.cookie,
        &member_models,
        json!({ "models": [haiku] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], false);
    assert_eq!(r.body["models"].as_array().unwrap().len(), 1);
    let r = patch(
        &app,
        &owner.cookie,
        &member_models,
        json!({ "all": true, "models": [haiku] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = patch(
        &app,
        &owner.cookie,
        &member_models,
        json!({ "models": ["model_000000000000000000000000"] }),
    )
    .await;
    assert!(r.status.is_client_error(), "{:?}", r.body);
    let r = patch(&app, &owner.cookie, &member_models, json!({ "all": true })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], true);

    let account = app
        .post(
            &format!("{base}/service-accounts"),
            json!({ "name": "bot", "role": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(account.status, StatusCode::CREATED, "{:?}", account.body);
    let account_id = account.body["id"].as_str().unwrap().to_owned();
    let account_models = format!("{base}/service-accounts/{account_id}/models");
    let r = patch(
        &app,
        &owner.cookie,
        &account_models,
        json!({ "models": [haiku, sonnet] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "bot", "service_account_id": account_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let key_models = format!("{base}/keys/{}/models", key.body["id"].as_str().unwrap());
    let r = patch(
        &app,
        &owner.cookie,
        &key_models,
        json!({ "models": [sonnet] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = patch(
        &app,
        &owner.cookie,
        &account_models,
        json!({ "models": [haiku] }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app.get(&key_models, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(
        r.body["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["id"] != sonnet.as_str()),
        "the key's list follows its service account: {:?}",
        r.body
    );
    let r = app.get(&account_models, &owner.cookie).await;
    assert_eq!(
        r.body["models"].as_array().unwrap().len(),
        1,
        "{:?}",
        r.body
    );
    s.cleanup().await;
}
