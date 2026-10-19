#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_server::services::credentials::{self, ResolveError};
use kaeso_server::services::master_key;
use kaeso_testkit::services::TestServices;
use secrecy::ExposeSecret;
use serde_json::json;

const K1: &str = "k1:AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";
const K2: &str = "k2:AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=";
const ORG_SECRET: &str = "sk-test-org-secret-0123456789WXYZ";
const SECOND_SECRET: &str = "sk-test-second-org-key-98765QRST";
const THIRD_SECRET: &str = "sk-test-third-org-key-24680LMNO";

#[test]
fn the_api_names_every_provider_a_key_can_be_for() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let description =
        doc["components"]["schemas"]["CredentialCreate"]["properties"]["provider"]["description"]
            .as_str()
            .unwrap()
            .to_owned();
    for p in kaeso_core::provider::PROVIDERS {
        assert!(
            description.contains(&format!("`{}`", p.id)),
            "{description} lacks {}",
            p.id
        );
    }
}

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"][0]["id"].as_str().unwrap().to_owned()
}

fn ids(owner: &Owner, project: &str) -> (OrgId, ProjectId) {
    (owner.org_id.parse().unwrap(), project.parse().unwrap())
}

async fn secret_of(app: &App, owner: &Owner, project: &str) -> Result<String, String> {
    let (org, project) = ids(owner, project);
    match credentials::resolve(&app.state, org, project, "openai").await {
        Ok(r) => Ok(r.secret.expose_secret().to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test]
async fn credentials_are_write_only_and_encrypted_at_rest() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = format!("/orgs/{}/credentials", owner.org_id);
    let r = app
        .post(
            &base,
            json!({ "provider": "openai", "name": "Production", "secret": format!("  {ORG_SECRET}\n") }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let id = r.body["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("cred_"));
    assert_eq!(r.body["last4"], "WXYZ");
    assert_eq!(r.body["key_id"], "k1");
    assert_eq!(r.body["is_default"], true);
    assert_eq!(r.body["projects"], json!([]));
    assert_eq!(r.body["created_by"]["id"], owner.user_id);

    let list = app.get(&base, &owner.cookie).await;
    let one = app.get(&format!("{base}/{id}"), &owner.cookie).await;
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    for body in [&r.body, &list.body, &one.body, &audit.body] {
        assert!(!body.to_string().contains("org-secret"), "{body}");
    }
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);
    assert!(audit.body.to_string().contains("credential.create"));

    let row: (Vec<u8>, Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT ciphertext, nonce, wrapped_key FROM provider_credentials WHERE org_id = $1::uuid",
    )
    .bind(owner.org_id.parse::<OrgId>().unwrap().uuid().to_string())
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    for column in [&row.0, &row.1, &row.2] {
        assert!(
            !column
                .windows(10)
                .any(|w| ORG_SECRET.as_bytes().windows(10).any(|s| s == w))
        );
    }

    let again = app
        .post(
            &base,
            json!({ "provider": "openai", "name": "Second", "secret": SECOND_SECRET }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CREATED, "{:?}", again.body);
    assert_eq!(again.body["is_default"], false);
    for (provider, secret) in [
        ("nope", ORG_SECRET),
        ("anthropic", "short"),
        ("anthropic", "has a space in it"),
    ] {
        let bad = app
            .post(
                &base,
                json!({ "provider": provider, "name": "Bad", "secret": secret }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(bad.error_type(), "invalid_request", "{:?}", bad.body);
    }

    app.clock.advance(time::Duration::minutes(16));
    let stale = app
        .post(
            &format!("{base}/{id}/replace"),
            json!({ "secret": "sk-test-replacement-000000000000" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(stale.error_type(), "recent_sign_in_required");
    let gone = app
        .call(
            Method::DELETE,
            &format!("{base}/{id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{:?}", gone.body);
    assert_eq!(
        app.get(&format!("{base}/{id}"), &owner.cookie).await.status,
        StatusCode::NOT_FOUND
    );
}

async fn create(app: &App, owner: &Owner, body: serde_json::Value) -> serde_json::Value {
    let r = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            body,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn assign(app: &App, owner: &Owner, cred: &str, projects: &[&str]) -> common::Resp {
    app.call(
        Method::PUT,
        &format!("/orgs/{}/credentials/{cred}/projects", owner.org_id),
        Some(json!({ "projects": projects })),
        Some(&owner.cookie),
    )
    .await
}

async fn listed(app: &App, owner: &Owner) -> Vec<serde_json::Value> {
    app.get(
        &format!("/orgs/{}/credentials", owner.org_id),
        &owner.cookie,
    )
    .await
    .body["data"]
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn projects_use_their_assigned_key_then_the_default() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let other = app.second_instance(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let second = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Second project" }),
            Some(&owner.cookie),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let err = secret_of(&app, &owner, &project).await.unwrap_err();
    assert!(
        err.contains("`openai`") && err.contains("/credentials"),
        "{err}"
    );
    let (org_id, project_id) = ids(&owner, &project);
    assert!(matches!(
        credentials::resolve(&app.state, org_id, project_id, "openai").await,
        Err(ResolveError::Missing { .. })
    ));

    let first = create(
        &app,
        &owner,
        json!({ "provider": "openai", "name": "Org", "secret": ORG_SECRET }),
    )
    .await;
    let first_id = first["id"].as_str().unwrap().to_owned();
    assert_eq!(first["is_default"], true);
    assert_eq!(secret_of(&app, &owner, &project).await.unwrap(), ORG_SECRET);
    assert_eq!(
        secret_of(&other, &owner, &project).await.unwrap(),
        ORG_SECRET
    );

    let second_key = create(
        &app,
        &owner,
        json!({ "provider": "openai", "name": "Team", "secret": SECOND_SECRET }),
    )
    .await;
    let second_id = second_key["id"].as_str().unwrap().to_owned();
    assert_eq!(secret_of(&app, &owner, &project).await.unwrap(), ORG_SECRET);
    let r = assign(&app, &owner, &second_id, &[&project]).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["projects"][0]["id"], project);
    assert_eq!(r.body["projects"][0]["name"], "Default project");
    assert_eq!(
        secret_of(&app, &owner, &project).await.unwrap(),
        SECOND_SECRET
    );
    eventually(&other, &owner, &project, SECOND_SECRET).await;
    assert_eq!(secret_of(&app, &owner, &second).await.unwrap(), ORG_SECRET);
    let keys = listed(&app, &owner).await;
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["id"], first_id, "the default is listed first");
    assert_eq!(keys[1]["projects"][0]["id"], project);

    let bad = assign(
        &app,
        &owner,
        &second_id,
        &["proj_00000000000000000000000000"],
    )
    .await;
    assert_eq!(bad.error_type(), "invalid_request", "{:?}", bad.body);
    let elsewhere = Owner {
        org_id: app
            .post(
                "/me/orgs",
                json!({ "name": "Elsewhere" }),
                Some(&owner.cookie),
            )
            .await
            .body["id"]
            .as_str()
            .unwrap()
            .to_owned(),
        cookie: owner.cookie.clone(),
        user_id: owner.user_id.clone(),
    };
    let foreign = default_project(&app, &elsewhere).await;
    let bad = assign(&app, &owner, &second_id, &[&project, &foreign]).await;
    assert_eq!(bad.error_type(), "invalid_request", "{:?}", bad.body);
    let foreign_key = assign(&app, &elsewhere, &second_id, &[&foreign]).await;
    assert_eq!(foreign_key.status, StatusCode::NOT_FOUND);

    let r = assign(&app, &owner, &first_id, &[&project]).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(secret_of(&app, &owner, &project).await.unwrap(), ORG_SECRET);
    let keys = listed(&app, &owner).await;
    assert_eq!(keys[1]["projects"], json!([]));
    let r = assign(&app, &owner, &first_id, &[]).await;
    assert_eq!(r.body["projects"], json!([]));

    let r = app
        .post(
            &format!("/orgs/{}/credentials/{second_id}/default", owner.org_id),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["is_default"], true);
    eventually(&other, &owner, &second, SECOND_SECRET).await;
    let keys = listed(&app, &owner).await;
    assert_eq!(
        keys.iter()
            .filter(|k| k["is_default"] == true)
            .map(|k| k["id"].clone())
            .collect::<Vec<_>>(),
        [json!(second_id)]
    );

    let renamed = app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/credentials/{second_id}", owner.org_id),
            Some(json!({ "name": "Team v2" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.body);
    assert_eq!(renamed.body["name"], "Team v2");
    let replaced = app
        .post(
            &format!("/orgs/{}/credentials/{second_id}/replace", owner.org_id),
            json!({ "secret": "sk-test-org-rotated-0000000000ABCD" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(replaced.status, StatusCode::OK, "{:?}", replaced.body);
    assert_eq!(replaced.body["last4"], "ABCD");
    assert_eq!(replaced.body["name"], "Team v2");
    assert_eq!(replaced.body["id"], second_id);
    eventually(
        &other,
        &owner,
        &second,
        "sk-test-org-rotated-0000000000ABCD",
    )
    .await;

    let third = create(
        &app,
        &owner,
        json!({ "provider": "openai", "name": "Newest", "secret": THIRD_SECRET }),
    )
    .await;
    let third_id = third["id"].as_str().unwrap().to_owned();
    assert_eq!(third["is_default"], false);
    assign(&app, &owner, &second_id, &[&project]).await;
    let gone = app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/credentials/{second_id}", owner.org_id),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    eventually(&other, &owner, &project, THIRD_SECRET).await;
    assert_eq!(
        secret_of(&app, &owner, &second).await.unwrap(),
        THIRD_SECRET
    );
    let keys = listed(&app, &owner).await;
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["id"], third_id);
    assert_eq!(keys[0]["is_default"], true);
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await
        .body
        .to_string();
    for action in [
        "credential.assign",
        "credential.default",
        "credential.rename",
        "credential.replace",
        "credential.delete",
    ] {
        assert!(audit.contains(action), "{action}");
    }

    let fourth = create(
        &app,
        &owner,
        json!({ "provider": "openai", "name": "Takeover", "secret": SECOND_SECRET, "make_default": true }),
    )
    .await;
    assert_eq!(fourth["is_default"], true);
    eventually(&other, &owner, &second, SECOND_SECRET).await;

    let mut used = serde_json::Value::Null;
    for _ in 0..100 {
        used = app
            .get(
                &format!("/orgs/{}/credentials/{third_id}", owner.org_id),
                &owner.cookie,
            )
            .await
            .body;
        if !used["last_used_at"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!used["last_used_at"].is_null(), "{used}");
}

async fn eventually(app: &App, owner: &Owner, project: &str, expected: &str) {
    for _ in 0..100 {
        if secret_of(app, owner, project).await.as_deref() == Ok(expected) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the other instance never saw the new credential");
}

#[tokio::test]
async fn master_keys_rotate_without_reentering_credentials() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = default_project(&app, &owner).await;
    let r = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            json!({ "provider": "openai", "name": "Org", "secret": ORG_SECRET }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    let totp = app.post("/me/totp", json!({}), Some(&owner.cookie)).await;
    assert_eq!(totp.status, StatusCode::OK, "{:?}", totp.body);

    let err = kaeso_server::serve::prepare(common::config(&s, &[("KAESO_MASTER_KEYS", K2)]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("k1") && !err.contains("k2"), "{err}");

    let both = format!("{K2},{K1}");
    let rotated = App::start_with(&s, &[("KAESO_MASTER_KEYS", both.as_str())]).await;
    assert_eq!(
        secret_of(&rotated, &owner, &project).await.unwrap(),
        ORG_SECRET
    );
    let usage = master_key::usage(&rotated.state.pg).await.unwrap();
    assert_eq!(usage.get("k1"), Some(&2));
    assert_eq!(usage.get("k2"), None);

    let before: Vec<u8> = sqlx::query_scalar("SELECT ciphertext FROM provider_credentials")
        .fetch_one(&rotated.state.pg)
        .await
        .unwrap();
    let report = master_key::rewrap(&rotated.state.pg, &rotated.state.keyring)
        .await
        .unwrap();
    assert_eq!(report.rewrapped, 2);
    assert_eq!(report.usage.get("k2"), Some(&2));
    assert_eq!(report.usage.get("k1"), None);
    let after: Vec<u8> = sqlx::query_scalar("SELECT ciphertext FROM provider_credentials")
        .fetch_one(&rotated.state.pg)
        .await
        .unwrap();
    assert_eq!(before, after);
    let again = master_key::rewrap(&rotated.state.pg, &rotated.state.keyring)
        .await
        .unwrap();
    assert_eq!(again.rewrapped, 0);

    let (id, current): (uuid::Uuid, Vec<u8>) =
        sqlx::query_as("SELECT id, wrapped_key FROM provider_credentials LIMIT 1")
            .fetch_one(&rotated.state.pg)
            .await
            .unwrap();
    let stale = kaeso_store::pg::credentials::set_wrapped_key(
        &rotated.state.pg,
        id,
        b"a wrapped key read before the secret was replaced",
        b"garbage",
        "k1",
    )
    .await
    .unwrap();
    assert!(
        !stale,
        "a re-wrap never overwrites a secret that changed meanwhile"
    );
    let kept: Vec<u8> =
        sqlx::query_scalar("SELECT wrapped_key FROM provider_credentials WHERE id = $1")
            .bind(id)
            .fetch_one(&rotated.state.pg)
            .await
            .unwrap();
    assert_eq!(kept, current);

    let only_new = App::start_with(&s, &[("KAESO_MASTER_KEYS", K2)]).await;
    assert_eq!(
        secret_of(&only_new, &owner, &project).await.unwrap(),
        ORG_SECRET
    );
    let err = kaeso_server::serve::prepare(common::config(&s, &[("KAESO_MASTER_KEYS", K1)]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("k2"), "{err}");
}
