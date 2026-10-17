#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const UNSERVED_CLAUDE: &str = "unserved-claude-sonnet";
const UNSERVED_GPT: &str = "unserved-gpt";
const ANTHROPIC_NOT_FOUND: &str = "The requested resource could not be found.";
const OPENAI_NOT_FOUND: &str =
    "The model `gpt-9-ultra` does not exist or you do not have access to it.";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    for (provider, model, dialect) in [
        ("anthropic", UNSERVED_CLAUDE, "anthropic_messages"),
        ("openai", UNSERVED_GPT, "openai_responses"),
    ] {
        let r = env
            .app
            .post(
                &format!("{}/models", base(&env)),
                json!({
                    "provider": provider, "model_id": model, "custom": true,
                    "overrides": {
                        "input_modalities": ["text"], "output_modalities": ["text"],
                        "context_limit": 200000, "output_limit": 8000,
                        "input_price": "3", "output_price": "15", "egress_dialect": dialect
                    }
                }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn catalogue(env: &Env) -> Vec<Value> {
    let r = env
        .app
        .get(&format!("{}/models", base(env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].as_array().unwrap().clone()
}

async fn entry(env: &Env, model: &str) -> Value {
    catalogue(env)
        .await
        .into_iter()
        .find(|m| m["model_id"] == model)
        .unwrap_or_else(|| panic!("no catalogue entry {model}"))
}

async fn listed(env: &Env, anthropic: bool) -> Vec<String> {
    let r = call(env, "/v1/models?limit=100", Some(&env.key), anthropic, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect()
}

fn unserved_message(provider: &str, model: &str) -> String {
    format!(
        "{provider} does not serve the model `{model}` to this project's provider key; check the key's access to it, or remove the model from the catalogue."
    )
}

#[tokio::test]
async fn a_listed_model_the_provider_does_not_serve_fails_clearly_in_each_dialect() {
    let env = env().await;
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unchecked"
    );
    assert!(
        listed(&env, true)
            .await
            .contains(&UNSERVED_CLAUDE.to_owned())
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(UNSERVED_CLAUDE, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("anthropic"));
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "not_found_error");
    assert_eq!(
        body["error"]["message"],
        unserved_message("Anthropic", UNSERVED_CLAUDE)
    );
    let gets: Vec<_> = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET")
        .collect();
    let probe = &gets[0];
    assert_eq!(probe.path, format!("/v1/models/{UNSERVED_CLAUDE}"));
    assert_eq!(probe.header("anthropic-version"), Some("2023-06-01"));
    assert!(probe.header("x-api-key").is_some());
    let pages: Vec<_> = gets[1..]
        .iter()
        .map(|r| (r.path.as_str(), r.query.as_deref()))
        .collect();
    assert_eq!(
        pages,
        [
            ("/v1/models", None),
            ("/v1/models", Some("after_id=claude-haiku-4-5-20251001"))
        ]
    );
    let o = env.capture.all().pop().unwrap();
    assert_eq!(o.status, 404);
    assert_eq!(o.error_kind.as_deref(), Some("not_found"));
    assert_eq!(
        o.error_message.as_deref(),
        Some(ANTHROPIC_NOT_FOUND),
        "the request log keeps the provider's own answer"
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(UNSERVED_GPT, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("openai"));
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["param"], "model");
    assert_eq!(
        body["error"]["message"],
        unserved_message("OpenAI", UNSERVED_GPT)
    );
    let o = env.capture.all().pop().unwrap();
    assert_eq!(o.error_message.as_deref(), Some(OPENAI_NOT_FOUND));

    let mut streamed = messages(UNSERVED_GPT, "Hi");
    streamed["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(streamed)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        unserved_message("OpenAI", UNSERVED_GPT)
    );

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": format!("anthropic/{UNSERVED_CLAUDE}"), "input": "Hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
}

#[tokio::test]
async fn a_request_teaches_the_catalogue_and_listings_leave_the_model_out() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(UNSERVED_CLAUDE, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let e = entry(&env, UNSERVED_CLAUDE).await;
    assert_eq!(e["availability"], "unavailable");
    assert!(e["availability_checked_at"].is_string());
    assert_eq!(e["revision"], 1, "learning is not a change to the entry");

    for anthropic in [true, false] {
        let ids = listed(&env, anthropic).await;
        assert!(!ids.contains(&UNSERVED_CLAUDE.to_owned()), "{ids:?}");
        assert!(ids.contains(&UNSERVED_GPT.to_owned()), "{ids:?}");
        assert!(ids.contains(&"claude-haiku-4-5".to_owned()), "{ids:?}");
    }
    let r = call(
        &env,
        &format!("/v1/models/{UNSERVED_CLAUDE}"),
        Some(&env.key),
        true,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        unserved_message("Anthropic", UNSERVED_CLAUDE)
    );
}

#[tokio::test]
async fn a_success_marks_an_unavailable_model_available_again() {
    let env = env().await;
    let haiku = entry(&env, "claude-haiku-4-5").await;
    sqlx::query("UPDATE catalogue_models SET availability = 'unavailable', availability_checked_at = now() WHERE model_id = 'claude-haiku-4-5'")
        .execute(&env.app.state.pg)
        .await
        .unwrap();
    kaeso_server::services::catalogue::forget(&env.app.state, common::uuid(env.project.as_str()))
        .await;
    assert!(
        !listed(&env, true)
            .await
            .contains(&"claude-haiku-4-5".to_owned())
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let after = entry(&env, "claude-haiku-4-5").await;
    assert_eq!(after["availability"], "available");
    assert_eq!(after["revision"], haiku["revision"]);
    assert!(
        listed(&env, true)
            .await
            .contains(&"claude-haiku-4-5".to_owned())
    );
}

#[tokio::test]
async fn a_not_found_about_something_else_keeps_the_providers_answer() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "[mock:error=not-found-file]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(message.starts_with("File `file_"), "{message}");
    assert_eq!(
        entry(&env, "claude-haiku-4-5").await["availability"],
        "unchecked"
    );
}

#[tokio::test]
async fn a_check_asks_each_provider_and_records_the_answers() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!("{}/models/check", base(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["failed"], json!([]));
    let by_id = |id: &str| {
        r.body["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(by_id(UNSERVED_CLAUDE)["availability"], "unavailable");
    assert_eq!(by_id(UNSERVED_GPT)["availability"], "unavailable");
    assert_eq!(by_id("claude-haiku-4-5")["availability"], "available");
    assert_eq!(by_id("gpt-4o")["availability"], "available");
    assert_eq!(by_id("text-embedding-3-small")["availability"], "available");
    let gets: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET")
        .map(|r| r.path)
        .collect();
    assert!(gets.contains(&"/v1/models/gpt-4o".to_owned()), "{gets:?}");
    assert!(
        gets.contains(&format!("/v1/models/{UNSERVED_CLAUDE}")),
        "{gets:?}"
    );

    let audit = env
        .app
        .get(
            &format!(
                "/orgs/{}/audit-log?action=catalogue_model.check",
                env.owner.org_id
            ),
            &env.owner.cookie,
        )
        .await;
    let event = &audit.body["data"][0];
    assert_eq!(event["action"], "catalogue_model.check");
    assert_eq!(event["target_kind"], "project");
    assert!(
        event["after"]["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!(format!("anthropic/{UNSERVED_CLAUDE}")))
    );

    let ids = listed(&env, false).await;
    assert!(!ids.contains(&UNSERVED_GPT.to_owned()), "{ids:?}");
}

#[tokio::test]
async fn changing_a_provider_key_resets_its_models_to_unchecked() {
    let env = env().await;
    let check = format!("{}/models/check", base(&env));
    env.app
        .post(&check, json!({}), Some(&env.owner.cookie))
        .await
        .expect_status(StatusCode::OK);
    let creds = env
        .app
        .get(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let anthropic = creds.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["provider"] == "anthropic")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.app
        .post(
            &format!("/orgs/{}/credentials/{anthropic}/replace", env.owner.org_id),
            json!({ "secret": "sk-ant-test-credential-0003" }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let e = entry(&env, UNSERVED_CLAUDE).await;
    assert_eq!(e["availability"], "unchecked");
    assert!(e["availability_checked_at"].is_null());
    assert_eq!(
        entry(&env, UNSERVED_GPT).await["availability"],
        "unavailable"
    );
    assert!(
        listed(&env, true)
            .await
            .contains(&UNSERVED_CLAUDE.to_owned())
    );

    env.app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/credentials/{anthropic}", env.owner.org_id),
            None,
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::NO_CONTENT);
    let r = env
        .app
        .post(&check, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let failed = r.body["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 2, "{failed:?}");
    for f in failed {
        assert_eq!(f["provider"], "anthropic");
        assert_eq!(f["reason"], "no_key");
        assert_eq!(
            f["message"],
            "No provider key for Anthropic is set up for this project."
        );
    }
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unchecked"
    );
}

#[tokio::test]
async fn deprecated_models_say_so_in_the_catalogue_and_the_anthropic_listing() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!("{}/models", base(&env)),
            json!({ "provider": "openai", "model_id": "gpt-4" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["deprecated"], true);
    assert_eq!(entry(&env, "gpt-4o").await["deprecated"], false);
    let r = call(&env, "/v1/models/gpt-4", Some(&env.key), true, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["lifecycle"], "deprecated");
    let r = call(&env, "/v1/models/gpt-4o", Some(&env.key), true, None).await;
    assert_eq!(r.json()["lifecycle"], "active");
    let r = call(
        &env,
        "/v1/models?lifecycle=active",
        Some(&env.key),
        true,
        None,
    )
    .await;
    let ids = r.json()["data"].to_string();
    assert!(!ids.contains("\"gpt-4\""), "{ids}");
}

#[tokio::test]
async fn an_archived_project_is_not_checked() {
    let env = env().await;
    let created = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Old bot" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let other = format!(
        "/orgs/{}/projects/{}",
        env.owner.org_id,
        created.body["id"].as_str().unwrap()
    );
    let check = format!("{other}/models/check");
    let archived = env
        .app
        .post(
            &format!("{other}/archive"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(archived.status.is_success(), "{:?}", archived.body);
    let r = env
        .app
        .post(&check, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
}

async fn credential(env: &Env, provider: &str) -> String {
    let creds = env
        .app
        .get(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    creds.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["provider"] == provider)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn replace_key(env: &Env, provider: &str, secret: &str) {
    let id = credential(env, provider).await;
    env.app
        .post(
            &format!("/orgs/{}/credentials/{id}/replace", env.owner.org_id),
            json!({ "secret": secret }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
}

async fn check(env: &Env) -> Value {
    let r = env
        .app
        .post(
            &format!("{}/models/check", base(env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

fn failures(body: &Value) -> Vec<(String, String, String)> {
    let mut found: Vec<_> = body["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["model_id"].as_str().unwrap().to_owned(),
                f["reason"].as_str().unwrap().to_owned(),
                f["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn a_key_change_during_a_request_keeps_its_answer_out_of_the_catalogue() {
    let env = env().await;
    let mut gate = env.mock.gate();
    let held = messages(UNSERVED_CLAUDE, &format!("Hi {}", gate.marker()));
    let change = async {
        gate.wait_for(1).await;
        replace_key(&env, "anthropic", "sk-ant-test-credential-0004").await;
        gate.open();
    };
    let (r, ()) = tokio::join!(
        call(&env, "/v1/messages", Some(&env.key), true, Some(held)),
        change
    );
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        unserved_message("Anthropic", UNSERVED_CLAUDE)
    );
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unchecked"
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(UNSERVED_CLAUDE, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unavailable"
    );
}

#[tokio::test]
async fn a_key_change_during_a_check_keeps_its_answers_out_of_the_catalogue() {
    let env = env().await;
    let mut gate = env.mock.gate();
    replace_key(&env, "anthropic", &format!("sk-ant-{}", gate.marker())).await;
    let change = async {
        gate.wait_for(2).await;
        replace_key(&env, "anthropic", "sk-ant-test-credential-0005").await;
        gate.open();
    };
    let (body, ()) = tokio::join!(check(&env), change);
    let changed = "The Anthropic provider key changed during the check; check again.";
    assert_eq!(
        failures(&body),
        [
            (
                "claude-haiku-4-5".to_owned(),
                "key_changed".to_owned(),
                changed.to_owned()
            ),
            (
                UNSERVED_CLAUDE.to_owned(),
                "key_changed".to_owned(),
                changed.to_owned()
            ),
        ]
    );
    for model in ["claude-haiku-4-5", UNSERVED_CLAUDE] {
        assert_eq!(entry(&env, model).await["availability"], "unchecked");
    }
    assert_eq!(entry(&env, "gpt-4o").await["availability"], "available");
    assert_eq!(
        entry(&env, UNSERVED_GPT).await["availability"],
        "unavailable"
    );
    let audit = env
        .app
        .get(
            &format!(
                "/orgs/{}/audit-log?action=catalogue_model.check",
                env.owner.org_id
            ),
            &env.owner.cookie,
        )
        .await;
    assert!(
        audit.body["data"][0]["after"]["failed"]
            .as_array()
            .unwrap()
            .contains(&json!(format!("anthropic/{UNSERVED_CLAUDE}")))
    );
}

#[tokio::test]
async fn availability_is_written_only_for_the_key_that_answered() {
    use kaeso_store::pg::catalogue::{Answer, Recorded, record_availability};
    use kaeso_store::pg::credentials::{KeyInUse, key_in_use};

    let env = env().await;
    let pg = &env.app.state.pg;
    let org = common::uuid(env.owner.org_id.as_str());
    let project = common::uuid(env.project.as_str());
    let haiku = common::uuid(
        entry(&env, "claude-haiku-4-5").await["id"]
            .as_str()
            .unwrap(),
    );
    let key = key_in_use(pg, org, project, "anthropic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(key.revision, 1);
    let answer = |entry, key| Answer {
        org_id: org,
        project_id: project,
        entry,
        provider: "anthropic",
        key,
        availability: kaeso_core::catalogue::Availability::Unavailable,
        at: env.app.state.clock.now(),
    };
    let stale = KeyInUse {
        revision: key.revision + 1,
        ..key
    };
    let other = KeyInUse {
        id: uuid::Uuid::new_v4(),
        ..key
    };
    for (entry, key, expected) in [
        (haiku, stale, Recorded::KeyChanged),
        (haiku, other, Recorded::KeyChanged),
        (uuid::Uuid::new_v4(), key, Recorded::EntryGone),
    ] {
        let mut tx = pg.begin().await.unwrap();
        assert_eq!(
            record_availability(&mut tx, &answer(entry, key))
                .await
                .unwrap(),
            expected
        );
        tx.commit().await.unwrap();
    }
    assert_eq!(
        entry(&env, "claude-haiku-4-5").await["availability"],
        "unchecked"
    );

    replace_key(&env, "anthropic", "sk-ant-test-credential-0006").await;
    let replaced = key_in_use(pg, org, project, "anthropic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((replaced.id, replaced.revision), (key.id, 2));
    let mut tx = pg.begin().await.unwrap();
    assert_eq!(
        record_availability(&mut tx, &answer(haiku, replaced))
            .await
            .unwrap(),
        Recorded::Yes
    );
    tx.commit().await.unwrap();
    kaeso_server::services::catalogue::forget(&env.app.state, project).await;
    assert_eq!(
        entry(&env, "claude-haiku-4-5").await["availability"],
        "unavailable"
    );

    let mut changing = pg.begin().await.unwrap();
    kaeso_store::pg::credentials::lock_provider(&mut changing, org, "anthropic")
        .await
        .unwrap();
    sqlx::query("UPDATE provider_credentials SET revision = revision + 1 WHERE id = $1")
        .bind(key.id)
        .execute(&mut *changing)
        .await
        .unwrap();
    let waiting = {
        let pg = pg.clone();
        let answer = Answer {
            availability: kaeso_core::catalogue::Availability::Available,
            ..answer(haiku, replaced)
        };
        tokio::spawn(async move {
            let mut tx = pg.begin().await.unwrap();
            let recorded = record_availability(&mut tx, &answer).await.unwrap();
            tx.commit().await.unwrap();
            recorded
        })
    };
    changing.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap(), Recorded::KeyChanged);
}

#[tokio::test]
async fn a_provider_without_model_endpoints_keeps_its_own_not_found() {
    let env = env().await;
    replace_key(&env, "openai", "sk-[mock:models=absent]-0001").await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(UNSERVED_GPT, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], OPENAI_NOT_FOUND);
    let gets: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET")
        .map(|r| r.path)
        .collect();
    assert_eq!(
        gets,
        [
            format!("/v1/models/{UNSERVED_GPT}"),
            "/v1/models".to_owned()
        ]
    );
    assert_eq!(entry(&env, UNSERVED_GPT).await["availability"], "unchecked");
    assert!(listed(&env, false).await.contains(&UNSERVED_GPT.to_owned()));

    let body = check(&env).await;
    let inconclusive = "OpenAI cannot confirm whether it serves the model to this key.";
    let models = [
        "gpt-4o",
        "gpt-4o-mini",
        "text-embedding-3-small",
        UNSERVED_GPT,
    ];
    assert_eq!(
        failures(&body),
        models
            .iter()
            .map(|m| (
                (*m).to_owned(),
                "inconclusive".to_owned(),
                inconclusive.to_owned()
            ))
            .collect::<Vec<_>>()
    );
    for model in models {
        assert_eq!(entry(&env, model).await["availability"], "unchecked");
    }
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unavailable"
    );
}

#[tokio::test]
async fn a_model_listing_answers_when_the_model_endpoint_is_missing() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!("{}/models", base(&env)),
            json!({
                "provider": "openai", "model_id": "model-id-0", "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 8000, "output_limit": 1000,
                    "input_price": "1", "output_price": "2", "egress_dialect": "openai_chat"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    replace_key(&env, "openai", "sk-[mock:models=list-only]-0002").await;
    let body = check(&env).await;
    assert_eq!(body["failed"], json!([]));
    assert_eq!(entry(&env, "model-id-0").await["availability"], "available");
    assert_eq!(
        entry(&env, UNSERVED_GPT).await["availability"],
        "unavailable"
    );
}

#[tokio::test]
async fn a_model_listing_that_never_ends_decides_nothing() {
    let env = env().await;
    replace_key(&env, "anthropic", "sk-ant-[mock:models=endless]-0003").await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(UNSERVED_CLAUDE, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], ANTHROPIC_NOT_FOUND);
    let pages = env
        .mock
        .received()
        .into_iter()
        .filter(|r| r.method == "GET" && r.path == "/v1/models")
        .count();
    assert_eq!(pages, 2);
    assert_eq!(
        entry(&env, UNSERVED_CLAUDE).await["availability"],
        "unchecked"
    );
}
