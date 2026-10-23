#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_core::clock::Clock;
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_server::services::credentials::{self, ResolveError};
use kaeso_server::services::master_key;
use kaeso_testkit::oauth::{Fault, MockOAuth};
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

const NOT_CALLBACK: &str =
    "Paste the address of the page that didn't load; it starts with `http://127.0.0.1`.";
const EXPIRED: &str = "This sign-in expired or was already used; start again.";

fn query_of(url: &str) -> std::collections::HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

fn with_param(callback: &str, edit: impl FnOnce(&mut Vec<(String, String)>)) -> String {
    let mut url = url::Url::parse(callback).unwrap();
    let mut pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    edit(&mut pairs);
    url.query_pairs_mut().clear().extend_pairs(&pairs);
    url.to_string()
}

#[track_caller]
fn refused_with(r: &common::Resp, status: StatusCode, message: &str) {
    assert_eq!(r.status, status, "{:?}", r.body);
    assert_eq!(r.body["error"]["message"], message, "{:?}", r.body);
}

#[tokio::test]
async fn a_chatgpt_account_becomes_a_provider_key_without_showing_its_tokens() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let providers = app
        .get(&format!("/orgs/{}/providers", owner.org_id), &owner.cookie)
        .await;
    let by_id = |id: &str| {
        providers.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        by_id("openai")["account"],
        json!({ "method": "chatgpt", "product": "ChatGPT", "usage_url": "https://chatgpt.com/settings/usage" })
    );
    assert_eq!(by_id("anthropic")["account"], json!(null));

    let (attempt, url) = common::begin_sign_in(&app, &owner, "Ada's plan").await;
    let q = query_of(&url);
    assert_eq!(q["client_id"], "dynamic_agent_client");
    assert_eq!(q["agent_name_hint"], "Kaeso");
    assert_eq!(
        q["scope"],
        "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct"
    );
    assert!(url.starts_with(&format!("{}/api/accounts/authorize?", oauth.base_url())));
    let (_, second) = common::begin_sign_in(&app, &owner, "Another").await;
    let host = &q["ext_agent_host_id"];
    assert_eq!(
        &query_of(&second)["ext_agent_host_id"],
        host,
        "one host id per install"
    );
    assert!(host.starts_with("urn:uuid:") && host.len() == 45, "{host}");

    let back = oauth.approve(&url, "sub-ada", "ada@example.com");
    let r = common::finish_sign_in(&app, &owner, &attempt, &back).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["auth_method"], "chatgpt");
    assert_eq!(r.body["account"], "ada@example.com");
    assert_eq!(r.body["auth_error"], json!(null));
    assert_eq!(r.body["last4"], "");
    assert_eq!(r.body["name"], "Ada's plan");
    assert_eq!(r.body["is_default"], true);
    let id = r.body["id"].as_str().unwrap().to_owned();

    let base = format!("/orgs/{}/credentials", owner.org_id);
    let list = app.get(&base, &owner.cookie).await;
    let one = app.get(&format!("{base}/{id}"), &owner.cookie).await;
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    assert!(audit.body.to_string().contains("credential.create"));
    let issued = oauth.issued();
    assert_eq!(
        issued.len(),
        3,
        "an ID token, an access token and a refresh token"
    );
    for body in [&r.body, &list.body, &one.body, &audit.body] {
        for token in issued.iter().chain([&back]) {
            assert!(!body.to_string().contains(token.as_str()), "{body}");
        }
    }
    let row: (String, String, time::OffsetDateTime, Vec<u8>) = sqlx::query_as(
        "SELECT auth_method, account_label, expires_at, ciphertext FROM provider_credentials",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(
        (row.0.as_str(), row.1.as_str()),
        ("chatgpt", "ada@example.com")
    );
    let lifetime = row.2 - app.clock.now();
    assert!(
        (time::Duration::minutes(59)..=time::Duration::minutes(61)).contains(&lifetime),
        "{lifetime}"
    );
    for token in &issued {
        assert!(
            !row.3
                .windows(16)
                .any(|w| token.as_bytes().windows(16).any(|t| t == w))
        );
    }
    let tokens = common::account_tokens(&app, &owner, &id).await;
    assert_eq!(tokens.client_id, "oaiapp_mock0");
    assert_eq!(tokens.sub, "sub-ada");
    assert!(
        tokens
            .scopes
            .iter()
            .any(|s| s == "chatgpt.tokens.use.direct")
    );
    let project = default_project(&app, &owner).await;
    let secret = secret_of(&app, &owner, &project).await.unwrap();
    assert_eq!(secret, tokens.access_token);
    assert!(issued.contains(&secret));
    s.cleanup().await;
}

async fn begin_again(app: &App, owner: &Owner, id: &str) -> (String, String) {
    let r = app
        .call(
            Method::POST,
            &format!("/orgs/{}/credentials/{id}/oauth", owner.org_id),
            None,
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    (
        r.body["attempt"].as_str().unwrap().to_owned(),
        r.body["authorize_url"].as_str().unwrap().to_owned(),
    )
}

async fn finish_again(
    app: &App,
    owner: &Owner,
    id: &str,
    attempt: &str,
    callback: &str,
) -> common::Resp {
    app.post(
        &format!("/orgs/{}/credentials/{id}/oauth/{attempt}", owner.org_id),
        json!({ "callback_url": callback }),
        Some(&owner.cookie),
    )
    .await
}

#[tokio::test]
async fn signing_in_again_keeps_the_registration_and_the_account() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let first = common::sign_in_chatgpt(&app, &owner, &oauth, "Plan", "ada@example.com").await;
    let id = first["id"].as_str().unwrap().to_owned();
    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let q = query_of(&url);
    assert_eq!(q["client_id"], "oaiapp_mock0", "the registration is kept");
    assert_eq!(q["login_hint"], "ada@example.com");
    assert!(!q.contains_key("agent_name_hint"));
    assert_eq!(
        q["ext_agent_host_id"],
        oauth.authorizations()[0]["ext_agent_host_id"]
    );
    let before = common::account_tokens(&app, &owner, &id).await;
    let back = oauth.approve(&url, "sub-ada@example.com", "ada@example.com");
    assert!(
        !back.contains("client_id="),
        "a known registration comes back without its id"
    );
    let r = finish_again(&app, &owner, &id, &attempt, &back).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["id"], id.as_str());
    assert_eq!(r.body["account"], "ada@example.com");
    let after = common::account_tokens(&app, &owner, &id).await;
    assert_ne!(after.access_token, before.access_token);
    assert_eq!(after.client_id, before.client_id);
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    assert!(audit.body.to_string().contains("credential.replace"));

    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let back = oauth.approve(&url, "sub-eve", "eve@example.com");
    refused_with(
        &finish_again(&app, &owner, &id, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        "You signed in to another ChatGPT account; sign in as ada@example.com.",
    );
    assert_eq!(
        common::account_tokens(&app, &owner, &id).await.access_token,
        after.access_token
    );

    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let back = with_param(
        &oauth.approve(&url, "sub-ada@example.com", "ada@example.com"),
        |p| {
            p.push(("client_id".into(), "oaiapp_someone_else".into()));
        },
    );
    refused_with(
        &finish_again(&app, &owner, &id, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        "The answer from ChatGPT is incomplete; start again.",
    );

    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let back = oauth.approve(&url, "sub-ada@example.com", "ada@example.com");
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        EXPIRED,
    );

    let replace = app
        .post(
            &format!("/orgs/{}/credentials/{id}/replace", owner.org_id),
            json!({ "secret": ORG_SECRET }),
            Some(&owner.cookie),
        )
        .await;
    refused_with(
        &replace,
        StatusCode::BAD_REQUEST,
        "This key signs in with an account; sign in again instead of replacing it.",
    );
    let key = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            json!({ "provider": "openai", "name": "Key", "secret": ORG_SECRET }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let r = app
        .call(
            Method::POST,
            &format!(
                "/orgs/{}/credentials/{}/oauth",
                owner.org_id,
                key.body["id"].as_str().unwrap()
            ),
            None,
            Some(&owner.cookie),
        )
        .await;
    refused_with(
        &r,
        StatusCode::BAD_REQUEST,
        "This key is an API key; replace it instead of signing in again.",
    );
    s.cleanup().await;
}

#[tokio::test]
async fn only_the_address_of_this_sign_in_finishes_it() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let (attempt, url) = common::begin_sign_in(&app, &owner, "Plan").await;
    let back = oauth.approve(&url, "sub-ada", "ada@example.com");
    let port = url::Url::parse(&back).unwrap().port().unwrap();
    let query = back.split_once('?').unwrap().1.to_owned();
    for pasted in [
        String::new(),
        "the page didn't load".to_owned(),
        format!("https://127.0.0.1:{port}/auth/callback?{query}"),
        format!("http://localhost:{port}/auth/callback?{query}"),
        format!("http://127.0.0.1:{port}/callback?{query}"),
        format!("http://127.0.0.1:{port}/auth/callback#{query}"),
        format!(
            "http://127.0.0.1:{}/auth/callback?{query}",
            port.wrapping_add(1).max(1)
        ),
        format!("http://127.0.0.1:{port}/auth/callback?code=x"),
        format!(
            "{}/?next=http://127.0.0.1:{port}/auth/callback?{query}",
            oauth.base_url()
        ),
    ] {
        let r = common::finish_sign_in(&app, &owner, &attempt, &pasted).await;
        refused_with(&r, StatusCode::BAD_REQUEST, NOT_CALLBACK);
        assert_eq!(r.body["error"]["param"], "callback_url");
    }
    for repeated in ["code", "state", "client_id", "scope"] {
        let pasted = with_param(&back, |p| {
            let value = p.iter().find(|(k, _)| k == repeated).unwrap().1.clone();
            p.push((repeated.to_owned(), value));
        });
        refused_with(
            &common::finish_sign_in(&app, &owner, &attempt, &pasted).await,
            StatusCode::BAD_REQUEST,
            "The pasted address repeats a parameter; paste it as the browser shows it.",
        );
    }
    let (other_attempt, _) = common::begin_sign_in(&app, &owner, "Other").await;
    for wrong in [
        other_attempt.as_str(),
        "not-an-attempt",
        "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5b",
    ] {
        refused_with(
            &common::finish_sign_in(&app, &owner, wrong, &back).await,
            StatusCode::BAD_REQUEST,
            EXPIRED,
        );
    }
    let (eve, _) = common::join(
        &app,
        &owner,
        json!({ "email": "eve@example.com", "role": "admin" }),
        "Eve",
    )
    .await;
    let eve = Owner {
        cookie: eve,
        user_id: String::new(),
        org_id: owner.org_id.clone(),
    };
    refused_with(
        &common::finish_sign_in(&app, &eve, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        EXPIRED,
    );

    let done = common::finish_sign_in(&app, &owner, &attempt, &back).await;
    assert_eq!(
        done.status,
        StatusCode::CREATED,
        "every refusal left the attempt usable: {:?}",
        done.body
    );
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        EXPIRED,
    );

    let (attempt, url) = common::begin_sign_in(&app, &owner, "Late").await;
    let back = oauth.approve(&url, "sub-ada", "ada@example.com");
    app.clock.advance(time::Duration::minutes(10));
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        EXPIRED,
    );

    let (attempt, url) = common::begin_sign_in(&app, &owner, "Declined").await;
    let denied = oauth.deny(&url);
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &denied).await,
        StatusCode::BAD_REQUEST,
        "You declined the sign-in at ChatGPT; start again.",
    );
    let back = with_param(&denied, |p| {
        p.retain(|(k, _)| k == "state");
        p.push(("code".into(), "a-code".into()));
    });
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        EXPIRED,
    );
    for (edit, message) in [
        (
            ("error", "server_error"),
            "ChatGPT refused the sign-in; start again.",
        ),
        (
            ("code", ""),
            "The answer from ChatGPT is incomplete; start again.",
        ),
        (
            ("client_id", "dynamic_agent_client"),
            "The answer from ChatGPT is incomplete; start again.",
        ),
        (
            ("code", "a-forged-code"),
            "ChatGPT refused the sign-in; start again.",
        ),
    ] {
        let (attempt, url) = common::begin_sign_in(&app, &owner, "Edited").await;
        let back = with_param(&oauth.approve(&url, "sub-ada", "ada@example.com"), |p| {
            p.retain(|(k, _)| k != edit.0);
            p.push((edit.0.to_owned(), edit.1.to_owned()));
        });
        refused_with(
            &common::finish_sign_in(&app, &owner, &attempt, &back).await,
            StatusCode::BAD_REQUEST,
            message,
        );
    }
    let (attempt, url) = common::begin_sign_in(&app, &owner, "Unregistered").await;
    let back = with_param(&oauth.approve(&url, "sub-ada", "ada@example.com"), |p| {
        p.retain(|(k, _)| k != "client_id");
    });
    refused_with(
        &common::finish_sign_in(&app, &owner, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        "The answer from ChatGPT is incomplete; start again.",
    );
    let list = app
        .get(
            &format!("/orgs/{}/credentials", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);
    s.cleanup().await;
}

#[tokio::test]
async fn the_sign_in_service_must_vouch_for_the_account_and_its_plan() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    for (fault, status, message) in [
        (
            Fault::WithoutPlan,
            StatusCode::BAD_REQUEST,
            "The account didn't allow using its ChatGPT plan; start again and allow it.",
        ),
        (
            Fault::WrongNonce,
            StatusCode::BAD_REQUEST,
            "The answer from ChatGPT could not be verified; start again.",
        ),
        (
            Fault::WrongAudience,
            StatusCode::BAD_REQUEST,
            "The answer from ChatGPT could not be verified; start again.",
        ),
        (
            Fault::WrongIssuer,
            StatusCode::BAD_REQUEST,
            "The answer from ChatGPT could not be verified; start again.",
        ),
        (
            Fault::NoEmail,
            StatusCode::BAD_REQUEST,
            "The answer from ChatGPT is incomplete; start again.",
        ),
        (
            Fault::Down,
            StatusCode::SERVICE_UNAVAILABLE,
            "ChatGPT can't be reached right now; try again shortly.",
        ),
    ] {
        oauth.set_fault(Fault::None);
        let (attempt, url) = common::begin_sign_in(&app, &owner, "Plan").await;
        let back = oauth.approve(&url, "sub-ada", "ada@example.com");
        oauth.set_fault(fault);
        let r = common::finish_sign_in(&app, &owner, &attempt, &back).await;
        assert_eq!(r.status, status, "{fault:?}: {:?}", r.body);
        assert_eq!(r.body["error"]["message"], message, "{fault:?}");
    }
    let list = app
        .get(
            &format!("/orgs/{}/credentials", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"], json!([]));

    let created = app
        .post(
            &format!("/orgs/{}/platform-keys", owner.org_id),
            json!({ "name": "automation" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let r = app
        .with_key(
            Method::POST,
            &format!("/orgs/{}/credentials/oauth", owner.org_id),
            Some(json!({ "provider": "openai", "name": "Plan" })),
            created.body["secret"].as_str().unwrap(),
        )
        .await;
    refused_with(
        &r,
        StatusCode::FORBIDDEN,
        "This needs a console session; API keys cannot manage accounts or the instance.",
    );
    let r = app
        .post(
            &format!("/orgs/{}/credentials/oauth", owner.org_id),
            json!({ "provider": "anthropic", "name": "Plan" }),
            Some(&owner.cookie),
        )
        .await;
    refused_with(
        &r,
        StatusCode::BAD_REQUEST,
        "Anthropic has no account sign-in; add an API key for it.",
    );
    assert_eq!(r.body["error"]["param"], "provider");
    s.cleanup().await;
}

#[tokio::test]
async fn deleting_an_account_ends_its_session_even_when_the_provider_is_down() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let ada = common::sign_in_chatgpt(&app, &owner, &oauth, "Ada", "ada@example.com").await;
    let eve = common::sign_in_chatgpt(&app, &owner, &oauth, "Eve", "eve@example.com").await;
    let (ada, eve) = (ada["id"].as_str().unwrap(), eve["id"].as_str().unwrap());
    let ada_tokens = common::account_tokens(&app, &owner, ada).await;
    let base = format!("/orgs/{}/credentials", owner.org_id);
    app.call(
        Method::DELETE,
        &format!("{base}/{ada}"),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(
        oauth.revoked(),
        [(
            ada_tokens.refresh_token.clone(),
            ada_tokens.client_id.clone()
        )]
    );
    oauth.set_fault(Fault::RevokeDown);
    app.call(
        Method::DELETE,
        &format!("{base}/{eve}"),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(oauth.revoked().len(), 1);
    let list = app.get(&base, &owner.cookie).await;
    assert_eq!(list.body["data"], json!([]));
    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    assert_eq!(
        audit.body.to_string().matches("credential.delete").count(),
        2
    );
    s.cleanup().await;
}

async fn waiting_for_a_lock(app: &App) -> bool {
    let (waiting,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM pg_locks l JOIN pg_database d ON d.oid = l.database \
         WHERE l.locktype = 'advisory' AND NOT l.granted AND d.datname = current_database()",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    waiting > 0
}

async fn renewing(app: &App, id: &str) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut renewal = app.state.pg.begin().await.unwrap();
    kaeso_store::pg::credentials::lock_refresh(&mut renewal, common::uuid(id))
        .await
        .unwrap();
    renewal
}

async fn rotate_once_waited_for(
    app: &App,
    owner: &Owner,
    id: &str,
    mut renewal: sqlx::Transaction<'static, sqlx::Postgres>,
    refresh_token: &str,
) {
    let credential = common::uuid(id);
    common::eventually("the change waits for the renewal", || {
        waiting_for_a_lock(app)
    })
    .await;
    let mut tokens = common::account_tokens(app, owner, id).await;
    tokens.refresh_token = refresh_token.to_owned();
    tokens.access_token = format!("{refresh_token}-access");
    let sealed = credentials::seal(
        &app.state,
        id.parse().unwrap(),
        &tokens.sealed_form().unwrap(),
    )
    .unwrap();
    kaeso_store::pg::credentials::refreshed(
        &mut *renewal,
        credential,
        &kaeso_store::pg::credentials::Sealed {
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
        },
        app.clock.now() + time::Duration::hours(1),
    )
    .await
    .unwrap();
    renewal.commit().await.unwrap();
}

#[tokio::test]
async fn signing_in_again_and_deleting_wait_for_a_renewal_in_progress() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let created = common::sign_in_chatgpt(&app, &owner, &oauth, "Plan", "ada@example.com").await;
    let id = created["id"].as_str().unwrap().to_owned();

    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let back = oauth.approve(&url, "sub-ada@example.com", "ada@example.com");
    let renewal = renewing(&app, &id).await;
    let (again, ()) = tokio::join!(
        finish_again(&app, &owner, &id, &attempt, &back),
        rotate_once_waited_for(&app, &owner, &id, renewal, "rt_from_a_renewal")
    );
    assert_eq!(again.status, StatusCode::OK, "{:?}", again.body);
    let kept = common::account_tokens(&app, &owner, &id).await;
    assert_ne!(
        kept.refresh_token, "rt_from_a_renewal",
        "the renewal's tokens of the old session never replace a new sign-in"
    );
    assert!(oauth.issued().contains(&kept.refresh_token));

    let path = format!("/orgs/{}/credentials/{id}", owner.org_id);
    let renewal = renewing(&app, &id).await;
    let (deleted, ()) = tokio::join!(
        app.call(Method::DELETE, &path, None, Some(&owner.cookie)),
        rotate_once_waited_for(&app, &owner, &id, renewal, "rt_rotated_meanwhile")
    );
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{:?}", deleted.body);
    assert_eq!(
        oauth.revoked(),
        [("rt_rotated_meanwhile".to_owned(), kept.client_id)],
        "the delete ends the session the renewal left, not the token it replaced"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_sign_in_that_cannot_be_saved_ends_its_session_at_openai() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let (attempt, url) = common::begin_sign_in(&app, &owner, "Plan").await;
    let back = oauth.approve(&url, "sub-ada@example.com", "ada@example.com");
    sqlx::raw_sql(
        "CREATE FUNCTION refuse_keys() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'refused'; END $$; \
         CREATE TRIGGER refuse_keys BEFORE INSERT ON provider_credentials \
         FOR EACH ROW EXECUTE FUNCTION refuse_keys();",
    )
    .execute(&app.state.pg)
    .await
    .unwrap();
    let r = common::finish_sign_in(&app, &owner, &attempt, &back).await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR, "{:?}", r.body);
    let issued = oauth.issued();
    let refresh = issued.iter().find(|t| t.starts_with("rt_")).unwrap();
    assert_eq!(
        oauth.revoked(),
        [(refresh.clone(), "oaiapp_mock0".to_owned())]
    );
    let list = app
        .get(
            &format!("/orgs/{}/credentials", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"], json!([]));
    s.cleanup().await;
}

fn newest_refresh_token(oauth: &MockOAuth) -> String {
    oauth
        .issued()
        .into_iter()
        .rfind(|t| t.starts_with("rt_"))
        .unwrap()
}

#[tokio::test]
async fn a_sign_in_refused_after_openai_issued_tokens_ends_that_session() {
    let s = TestServices::start().await;
    let oauth = MockOAuth::start().await;
    let app = App::start_signing_in(&s, &oauth).await;
    let owner = app.setup().await;
    let created = common::sign_in_chatgpt(&app, &owner, &oauth, "Plan", "ada@example.com").await;
    let id = created["id"].as_str().unwrap().to_owned();

    let (attempt, url) = begin_again(&app, &owner, &id).await;
    let back = oauth.approve(&url, "sub-eve", "eve@example.com");
    refused_with(
        &finish_again(&app, &owner, &id, &attempt, &back).await,
        StatusCode::BAD_REQUEST,
        "You signed in to another ChatGPT account; sign in as ada@example.com.",
    );
    assert_eq!(
        oauth.revoked(),
        [(newest_refresh_token(&oauth), "oaiapp_mock0".to_owned())]
    );

    for (fault, message) in [
        (
            Fault::WithoutPlan,
            "The account didn't allow using its ChatGPT plan; start again and allow it.",
        ),
        (
            Fault::WrongNonce,
            "The answer from ChatGPT could not be verified; start again.",
        ),
    ] {
        oauth.set_fault(Fault::None);
        let (attempt, url) = common::begin_sign_in(&app, &owner, "Second").await;
        let back = oauth.approve(&url, "sub-bob", "bob@example.com");
        oauth.set_fault(fault);
        refused_with(
            &common::finish_sign_in(&app, &owner, &attempt, &back).await,
            StatusCode::BAD_REQUEST,
            message,
        );
        assert_eq!(
            oauth.revoked().pop().unwrap().0,
            newest_refresh_token(&oauth),
            "{fault:?}"
        );
    }
    assert_eq!(oauth.revoked().len(), 3);
    let list = app
        .get(
            &format!("/orgs/{}/credentials", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);
    s.cleanup().await;
}
