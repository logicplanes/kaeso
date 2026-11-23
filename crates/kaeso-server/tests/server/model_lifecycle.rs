#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// Model retirement: KPMR's dates reach replies as headers, a retired model fails clearly or is
// answered by its replacement, a project sees who still calls it, moves to the replacement,
// and hears about it ahead of time.

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Answer, Capture, Env, call, chat};
use kaeso_core::clock::Clock;
use kaeso_core::kpmr;
use kaeso_server::jobs::Ran;
use kaeso_store::pg::limits::UsageRow;
use serde_json::{Value, json};
use time::{Date, Duration};

const LOOPBACK: (&str, &str) = ("KAESO_WEBHOOKS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), Some(&[LOOPBACK])).await
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

fn today(env: &Env) -> Date {
    env.app.clock.now().date()
}

fn day(d: Date) -> String {
    kpmr::day::format(d)
}

// Publishes the built-in catalogue with `gpt-4o` retiring on `shutdown` in favour of
// `replacement`, as a KPMR sync would.
async fn retire(env: &Env, model: &str, deprecated: Date, shutdown: Date, replacement: &str) {
    let mut doc = common::kpmr::built_in();
    let m = common::kpmr::model(&mut doc, "openai", model);
    m["status"] = json!(if shutdown <= today(env) {
        "retired"
    } else {
        "deprecated"
    });
    m["lifecycle"] = json!({
        "announced_on": day(deprecated),
        "deprecated_on": day(deprecated),
        "shutdown_on": day(shutdown),
        "replacement": replacement,
        "source": "https://example.com/deprecations",
    });
    let bytes = doc.to_string();
    let parsed = kpmr::parse(bytes.as_bytes(), &kaeso_core::provider::known_ids())
        .unwrap()
        .catalogue;
    kaeso_server::services::kpmr::apply(
        &env.app.state,
        parsed,
        env.app.clock.now(),
        "https://kpmr.example.com/v1/catalogue",
        b"lifecycle",
        None,
    )
    .await
    .unwrap();
}

async fn ask(env: &Env, model: &str) -> Answer {
    call(
        env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(model, "Hi")),
    )
    .await
}

async fn model_id(env: &Env, model: &str) -> String {
    let r = env
        .app
        .get(&format!("{}/models", base(env)), &env.owner.cookie)
        .await;
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == model)
        .unwrap_or_else(|| panic!("{model} is in the catalogue"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn settings(env: &Env, body: Value) -> common::Resp {
    env.app
        .call(
            Method::PUT,
            &format!("{}/model-lifecycle", base(env)),
            Some(body),
            Some(&env.owner.cookie),
        )
        .await
}

async fn used(env: &Env, model: &str, on: Date, requests: i64) {
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[UsageRow {
            day: on,
            org_id: common::uuid(&env.owner.org_id),
            project_id: common::uuid(&env.project),
            user_id: Some(common::uuid(&env.owner.user_id)),
            service_account_id: None,
            api_key_id: None,
            catalogue_model_id: None,
            model_id: model.into(),
            provider: "openai".into(),
            ingress_dialect: "openai_chat".into(),
            egress_dialect: "openai_chat".into(),
            credential_id: None,
            source_id: None,
            requests,
            errors: 0,
            input_tokens: 10,
            output_tokens: 10,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost: rust_decimal::Decimal::ZERO,
            media: Default::default(),
        }],
    )
    .await
    .unwrap();
}

async fn run_job(env: &Env) -> Ran {
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "model_lifecycle")
        .unwrap();
    kaeso_server::jobs::run_once(&env.app.state, job)
        .await
        .unwrap()
        .unwrap()
}

async fn queued(env: &Env) -> Vec<(String, Value)> {
    env.app.state.in_flight.settled().await;
    sqlx::query_as::<_, (String, Value)>(
        "SELECT event_type, body::jsonb FROM webhook_deliveries ORDER BY created_at",
    )
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_deprecated_model_answers_with_deprecation_sunset_and_a_link_to_its_replacement() {
    let env = env().await;
    let shutdown = today(&env) + Duration::days(60);
    let deprecated = today(&env) - Duration::days(10);
    retire(&env, "gpt-4o", deprecated, shutdown, "gpt-4o-mini").await;
    let r = ask(&env, "gpt-4o").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let at = deprecated.midnight().assume_utc().unix_timestamp();
    assert_eq!(r.header("deprecation"), Some(format!("@{at}").as_str()));
    let sunset = r.header("sunset").unwrap();
    assert!(sunset.ends_with(" 00:00:00 GMT"), "{sunset}");
    assert!(
        sunset.contains(&format!("{:02} ", shutdown.day()))
            && sunset.contains(&shutdown.year().to_string()),
        "{sunset}"
    );
    let link = r.header("link").unwrap();
    assert!(
        link.contains(
            "<https://kpmr.kaeso.app/models/openai/gpt-4o-mini>; rel=\"successor-version\""
        ),
        "{link}"
    );
    assert!(
        link.contains("<https://example.com/deprecations>; rel=\"deprecation\""),
        "{link}"
    );
    let other = ask(&env, "gpt-4o-mini").await;
    assert_eq!(other.status, StatusCode::OK);
    assert!(other.header("deprecation").is_none() && other.header("sunset").is_none());
}

#[tokio::test]
async fn a_retired_model_fails_with_410_unless_the_project_answers_with_its_replacement() {
    let env = env().await;
    let shutdown = today(&env) - Duration::days(1);
    retire(
        &env,
        "gpt-4o",
        shutdown - Duration::days(90),
        shutdown,
        "gpt-4o-mini",
    )
    .await;
    let r = ask(&env, "gpt-4o").await;
    assert_eq!(r.status, StatusCode::GONE, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!(
            "The model `openai/gpt-4o` was retired on {}; its provider recommends `gpt-4o-mini`.",
            day(shutdown)
        )
    );
    assert!(r.header("sunset").is_some());

    let s = settings(
        &env,
        json!({ "move_on_shutdown": false, "serve_replacement": true }),
    )
    .await;
    assert_eq!(s.status, StatusCode::OK, "{:?}", s.body);
    let r = ask(&env, "gpt-4o").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o-mini"));
    env.app.state.in_flight.settled().await;
    let logged = env.capture.all();
    let last = logged.last().unwrap();
    assert_eq!(last.model_id.as_deref(), Some("gpt-4o-mini"));
    let skipped = &last.fallbacks[0];
    assert!(skipped.skipped && skipped.status == 410, "{skipped:?}");
    assert_eq!(skipped.model, "openai/gpt-4o");

    let r = env
        .app
        .call(
            Method::GET,
            &format!("{}/model-lifecycle", base(&env)),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(
        r.body,
        json!({ "move_on_shutdown": false, "serve_replacement": true })
    );
}

#[tokio::test]
async fn the_deprecations_view_names_aliases_route_steps_and_callers_and_a_move_rewrites_them() {
    let env = env().await;
    let gpt = model_id(&env, "gpt-4o").await;
    let haiku = model_id(&env, "claude-haiku-4-5").await;
    let alias = env
        .app
        .post(
            &format!("{}/aliases", base(&env)),
            json!({ "name": "chat", "route": { "type": "model", "model": haiku, "fallback": { "type": "model", "model": gpt } } }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(alias.status, StatusCode::CREATED, "{:?}", alias.body);
    let shutdown = today(&env) + Duration::days(20);
    retire(
        &env,
        "gpt-4o",
        today(&env) - Duration::days(3),
        shutdown,
        "gpt-4o-mini",
    )
    .await;
    used(&env, "gpt-4o", today(&env) - Duration::days(2), 4).await;
    used(&env, "gpt-4o", today(&env), 3).await;

    let r = env
        .app
        .get(&format!("{}/deprecations", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["soon"], 1);
    let m = &r.body["data"][0];
    assert_eq!(m["model_id"], "gpt-4o");
    assert_eq!(m["state"], "deprecated");
    assert_eq!(m["days_left"], 20);
    assert_eq!(m["lifecycle"]["shutdown_on"], day(shutdown));
    assert_eq!(m["replacement"]["model_id"], "gpt-4o-mini");
    assert_eq!(m["replacement"]["listed"], true);
    assert_eq!(
        m["replacement"]["catalogue_model_id"],
        model_id(&env, "gpt-4o-mini").await
    );
    assert_eq!(
        m["aliases"],
        json!([{ "alias_id": alias.body["id"], "name": "chat", "steps": ["route.fallback"] }])
    );
    assert_eq!(m["usage"]["requests"], 7);
    assert_eq!(m["usage"]["members"][0]["requests"], 7);
    assert_eq!(m["usage"]["daily"].as_array().unwrap().len(), 2);

    let org = env
        .app
        .get(
            &format!("/orgs/{}/deprecations", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(org.body["data"][0]["models"], 1, "{:?}", org.body);
    assert_eq!(org.body["data"][0]["next_shutdown_on"], day(shutdown));

    let move_path = format!("{}/deprecations/{gpt}/move", base(&env));
    let preview = env.app.get(&move_path, &env.owner.cookie).await;
    assert_eq!(preview.status, StatusCode::OK, "{:?}", preview.body);
    assert_eq!(
        (
            preview.body["steps"].as_u64(),
            preview.body["adds_replacement"].as_bool()
        ),
        (Some(1), Some(false))
    );
    let moved = env
        .app
        .post(&move_path, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.body);
    assert_eq!(moved.body["model"]["aliases"], json!([]));
    let a = env
        .app
        .get(
            &format!(
                "{}/aliases/{}",
                base(&env),
                alias.body["id"].as_str().unwrap()
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        a.body["route"]["fallback"]["model"],
        model_id(&env, "gpt-4o-mini").await
    );
    let audit = env
        .app
        .get(
            &format!("{}/audit-log?action=catalogue_model.move", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        audit.body["data"][0]["action"], "catalogue_model.move",
        "{:?}",
        audit.body
    );
}

#[tokio::test]
async fn a_move_adds_a_missing_replacement_and_refuses_a_model_without_one() {
    let env = env().await;
    let gpt = model_id(&env, "gpt-4o").await;
    retire(
        &env,
        "gpt-4o",
        today(&env),
        today(&env) + Duration::days(40),
        "gpt-4.1",
    )
    .await;
    let path = format!("{}/deprecations/{gpt}/move", base(&env));
    let preview = env.app.get(&path, &env.owner.cookie).await;
    assert_eq!(preview.body["adds_replacement"], true, "{:?}", preview.body);
    let moved = env
        .app
        .post(&path, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.body);
    assert!(model_id(&env, "gpt-4.1").await.starts_with("model_"));

    let mini = model_id(&env, "gpt-4o-mini").await;
    let r = env
        .app
        .get(
            &format!("{}/deprecations/{mini}/move", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "No model of this project is retired or has a shutdown date with that id."
    );
}

#[tokio::test]
async fn the_lifecycle_job_announces_warns_while_the_model_is_called_and_moves_on_shutdown() {
    let env = env().await;
    let hook = env
        .app
        .post(
            &format!("/orgs/{}/webhooks", env.owner.org_id),
            json!({ "name": "ops", "url": "http://127.0.0.1:9/hook", "kinds": ["model_deprecated", "model_shutdown_soon"] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(hook.status, StatusCode::CREATED, "{:?}", hook.body);
    let gpt = model_id(&env, "gpt-4o").await;
    let alias = env
        .app
        .post(
            &format!("{}/aliases", base(&env)),
            json!({ "name": "main", "route": { "type": "model", "model": gpt } }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(alias.status, StatusCode::CREATED);
    // The admin's session ends after a week without use, so the settings are set first.
    let s = settings(
        &env,
        json!({ "move_on_shutdown": true, "serve_replacement": false }),
    )
    .await;
    assert_eq!(s.status, StatusCode::OK);
    let start = today(&env);
    let shutdown = start + Duration::days(40);
    retire(&env, "gpt-4o", start, shutdown, "gpt-4o-mini").await;
    assert_eq!(
        run_job(&env).await,
        Ran::worked("1 announced, 0 warned, 0 moved, 0 replacements added")
    );
    assert_eq!(
        run_job(&env).await,
        Ran::idle("0 announced, 0 warned, 0 moved, 0 replacements added")
    );

    // 30 days before: only a project that still calls the model hears about it.
    env.app.clock.advance(Duration::days(10));
    assert_eq!(
        run_job(&env).await,
        Ran::idle("0 announced, 0 warned, 0 moved, 0 replacements added")
    );
    used(&env, "gpt-4o", today(&env), 2).await;
    assert_eq!(
        run_job(&env).await,
        Ran::worked("0 announced, 1 warned, 0 moved, 0 replacements added")
    );
    assert_eq!(
        run_job(&env).await,
        Ran::idle("0 announced, 0 warned, 0 moved, 0 replacements added")
    );

    // A week before, and again the day before: one warning each, none for the steps skipped.
    env.app.clock.advance(Duration::days(24));
    used(&env, "gpt-4o", today(&env), 1).await;
    assert_eq!(
        run_job(&env).await,
        Ran::worked("0 announced, 1 warned, 0 moved, 0 replacements added")
    );
    env.app.clock.advance(Duration::days(5));
    used(&env, "gpt-4o", today(&env), 1).await;
    assert_eq!(
        run_job(&env).await,
        Ran::worked("0 announced, 1 warned, 0 moved, 0 replacements added")
    );

    env.app.clock.advance(Duration::days(1));
    retire(&env, "gpt-4o", start, shutdown, "gpt-4o-mini").await;
    assert_eq!(
        run_job(&env).await,
        Ran::worked("0 announced, 0 warned, 1 moved, 0 replacements added")
    );
    let route: Value = sqlx::query_scalar("SELECT route FROM aliases WHERE name = 'main'")
        .fetch_one(&env.app.state.pg)
        .await
        .unwrap();
    let mini: String =
        sqlx::query_scalar("SELECT id::text FROM catalogue_models WHERE model_id = 'gpt-4o-mini'")
            .fetch_one(&env.app.state.pg)
            .await
            .unwrap();
    assert_eq!(
        common::uuid(route["model"].as_str().unwrap()).to_string(),
        mini
    );
    let audited: Vec<(String, Option<uuid::Uuid>)> = sqlx::query_as(
        "SELECT actor_kind, project_id FROM audit_events WHERE action = 'catalogue_model.move'",
    )
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap();
    assert_eq!(
        audited,
        [("system".to_owned(), Some(common::uuid(&env.project)))]
    );

    let events: Vec<String> = queued(&env).await.into_iter().map(|(t, _)| t).collect();
    assert_eq!(
        events,
        [
            "model.deprecated",
            "model.shutdown_soon",
            "model.shutdown_soon",
            "model.shutdown_soon",
            "model.moved"
        ]
    );
    let (_, body) = &queued(&env).await[1];
    let data = &body["data"];
    assert_eq!(data["model"], "gpt-4o");
    assert_eq!(data["replacement"], "gpt-4o-mini");
    assert_eq!(data["shutdown_on"], day(shutdown));
    assert_eq!(data["requests_7d"], 2);
    assert_eq!(data["catalogue_model_id"], gpt);
    assert_eq!(data["provider_id"], "openai");
}

#[tokio::test]
async fn the_settings_are_off_by_default_audited_and_need_the_right_to_manage_models() {
    let env = env().await;
    let r = env
        .app
        .get(
            &format!("{}/model-lifecycle", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        r.body,
        json!({ "move_on_shutdown": false, "serve_replacement": false })
    );
    let r = settings(
        &env,
        json!({ "move_on_shutdown": true, "serve_replacement": false }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let audit = env
        .app
        .get(
            &format!("{}/audit-log?action=project.model_lifecycle", base(&env)),
            &env.owner.cookie,
        )
        .await;
    let entry = &audit.body["data"][0];
    assert_eq!(
        entry["before"]["move_on_shutdown"], false,
        "{:?}",
        audit.body
    );
    assert_eq!(entry["after"]["move_on_shutdown"], true);
    let r = settings(&env, json!({ "move_on_shutdown": true, "later": true })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}
