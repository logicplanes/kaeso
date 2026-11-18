#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// Model lines: a project adds `claude-opus-latest`-style names that KPMR keeps pointed at a
// provider's newest stable model, calls them like a model, and follows them by its policy.

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat};
use kaeso_core::clock::Clock;
use kaeso_core::kpmr;
use serde_json::{Value, json};
use time::Duration;

const LOOPBACK: (&str, &str) = ("KAESO_WEBHOOKS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), Some(&[LOOPBACK])).await
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

// Publishes the built-in catalogue with `lines` for a provider, as a KPMR sync would.
async fn publish(env: &Env, provider: &str, lines: Value) {
    let mut doc = common::kpmr::built_in();
    doc["providers"][provider]["lines"] = lines;
    let bytes = doc.to_string();
    let parsed = kpmr::parse(bytes.as_bytes(), &kaeso_core::provider::known_ids())
        .unwrap()
        .catalogue;
    kaeso_server::services::kpmr::apply(
        &env.app.state,
        parsed,
        env.app.clock.now(),
        "https://kpmr.example.com/v1/catalogue",
        b"lines",
        None,
    )
    .await
    .unwrap();
}

fn at(model: &str) -> Value {
    json!({ "model": model, "since": "2026-09-22", "previous": null })
}

async fn add(env: &Env, provider: &str, line: &str, policy: Value) -> Value {
    let mut body = json!({ "provider": provider, "line": line });
    if let Value::Object(p) = policy {
        body.as_object_mut().unwrap().extend(p);
    }
    let r = env
        .app
        .post(
            &format!("{}/lines", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn action(env: &Env, id: &str, what: &str) -> common::Resp {
    env.app
        .post(
            &format!("{}/lines/{id}/{what}", base(env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await
}

async fn line(env: &Env, id: &str) -> Value {
    let r = env
        .app
        .get(&format!("{}/lines", base(env)), &env.owner.cookie)
        .await;
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id)
        .unwrap()
        .clone()
}

async fn run_job(env: &Env) -> String {
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "model_lines")
        .unwrap();
    kaeso_server::jobs::run_once(&env.app.state, job)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn a_line_is_called_like_a_model_and_the_log_names_the_model_and_the_line() {
    let env = env().await;
    publish(
        &env,
        "openai",
        json!({ "gpt-small-latest": at("gpt-4o-mini") }),
    )
    .await;
    let offered = env
        .app
        .get(
            &format!("{}/lines/offered?provider=openai", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        offered.body["data"],
        json!([{ "provider": "openai", "line": "gpt-small-latest", "model": "gpt-4o-mini", "since": "2026-09-22", "previous": null, "added": null }])
    );
    let added = add(&env, "openai", "gpt-small-latest", json!({})).await;
    assert_eq!(
        (&added["model"], &added["policy"], &added["kpmr_model"]),
        (
            &json!("gpt-4o-mini"),
            &json!("follow"),
            &json!("gpt-4o-mini")
        )
    );
    let id = added["id"].as_str().unwrap();
    let entry = env
        .app
        .get(&format!("{}/models/{id}", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(entry.body["model_id"], "gpt-small-latest");
    assert_eq!(entry.body["line_target"], "gpt-4o-mini");

    for name in ["gpt-small-latest", "openai/gpt-small-latest"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(name, "Hi")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.header("kaeso-model"), Some("gpt-4o-mini"));
    }
    env.app.state.in_flight.settled().await;
    let last = env.capture.all().pop().unwrap();
    assert_eq!(last.model_id.as_deref(), Some("gpt-4o-mini"));
    assert_eq!(last.line.as_deref(), Some("gpt-small-latest"));
    let again = env
        .app
        .post(
            &format!("{}/lines", base(&env)),
            json!({ "provider": "openai", "line": "gpt-small-latest" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(
        again.body["error"]["message"],
        "The catalogue already has `openai/gpt-small-latest`."
    );
}

#[tokio::test]
async fn follow_moves_ask_waits_delay_moves_later_and_pin_stays() {
    let env = env().await;
    let hook = env
        .app
        .post(
            &format!("/orgs/{}/webhooks", env.owner.org_id),
            json!({ "name": "ops", "url": "http://127.0.0.1:9/hook", "kinds": ["model_lines"] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(hook.status, StatusCode::CREATED, "{:?}", hook.body);
    let lines = |model: &str| json!({ "a-latest": at(model), "b-latest": at(model), "c-latest": at(model), "d-latest": at(model) });
    publish(&env, "openai", lines("gpt-4o-mini")).await;
    let follow = add(&env, "openai", "a-latest", json!({})).await;
    let ask = add(&env, "openai", "b-latest", json!({ "policy": "ask" })).await;
    let delay = add(
        &env,
        "openai",
        "c-latest",
        json!({ "policy": "delay", "delay_days": 3 }),
    )
    .await;
    let pin = add(&env, "openai", "d-latest", json!({ "policy": "pin" })).await;
    let id = |v: &Value| v["id"].as_str().unwrap().to_owned();

    publish(&env, "openai", lines("gpt-4o")).await;
    assert_eq!(run_job(&env).await, "1 moved, 2 waiting, 0 held");
    assert_eq!(run_job(&env).await, "0 moved, 0 waiting, 0 held");
    assert_eq!(line(&env, &id(&follow)).await["model"], "gpt-4o");
    assert_eq!(line(&env, &id(&follow)).await["previous"], "gpt-4o-mini");
    let waiting = line(&env, &id(&ask)).await;
    assert_eq!(
        (&waiting["model"], &waiting["pending"]["model"]),
        (&json!("gpt-4o-mini"), &json!("gpt-4o"))
    );
    assert_eq!(waiting["pending"]["moves_at"], Value::Null);
    let delayed = line(&env, &id(&delay)).await;
    assert!(delayed["pending"]["moves_at"].is_string(), "{delayed:?}");
    assert_eq!(line(&env, &id(&pin)).await["model"], "gpt-4o-mini");

    env.app.clock.advance(Duration::days(3));
    assert_eq!(run_job(&env).await, "1 moved, 0 waiting, 0 held");
    assert_eq!(line(&env, &id(&delay)).await["model"], "gpt-4o");

    let kept = action(&env, &id(&ask), "keep").await;
    assert_eq!(
        (kept.status, &kept.body["kept"]),
        (StatusCode::OK, &json!("gpt-4o"))
    );
    assert_eq!(run_job(&env).await, "0 moved, 0 waiting, 0 held");
    let moved = action(&env, &id(&ask), "move").await;
    assert_eq!(
        (moved.status, &moved.body["model"]),
        (StatusCode::OK, &json!("gpt-4o"))
    );
    let nothing = action(&env, &id(&ask), "move").await;
    assert_eq!(nothing.status, StatusCode::CONFLICT);
    assert_eq!(
        nothing.body["error"]["message"],
        "The line points where KPMR does; there is nothing to move to."
    );

    let back = action(&env, &id(&follow), "back").await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.body);
    assert_eq!(
        (
            &back.body["model"],
            &back.body["previous"],
            &back.body["policy"]
        ),
        (&json!("gpt-4o-mini"), &json!("gpt-4o"), &json!("pin"))
    );
    let no_previous = action(&env, &id(&pin), "back").await;
    assert_eq!(
        no_previous.body["error"]["message"],
        "The line has no earlier model to go back to."
    );
    let no_wait = action(&env, &id(&pin), "keep").await;
    assert_eq!(
        no_wait.body["error"]["message"],
        "No move waits for this line."
    );

    env.app.state.in_flight.settled().await;
    let events: Vec<(String, Value)> = sqlx::query_as::<_, (String, Value)>(
        "SELECT event_type, body::jsonb FROM webhook_deliveries ORDER BY created_at",
    )
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap();
    let types: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(
        types,
        [
            "line.moved",
            "line.move_available",
            "line.moved",
            "line.moved",
            "line.moved"
        ]
    );
    let data = &events[0].1["data"];
    assert_eq!(
        (&data["line"], &data["from"], &data["to"], &data["policy"]),
        (
            &json!("a-latest"),
            &json!("gpt-4o-mini"),
            &json!("gpt-4o"),
            &json!("follow")
        )
    );
    assert!(
        data["prices"].as_array().is_some_and(|p| !p.is_empty()),
        "{data:?}"
    );
    let audit = env
        .app
        .get(
            &format!("{}/audit-log?action=catalogue_model.line_move", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        audit.body["data"].as_array().unwrap().len(),
        4,
        "{:?}",
        audit.body
    );
}

#[tokio::test]
async fn a_line_without_a_provider_key_stays_and_says_why() {
    let env = env().await;
    let models = &common::kpmr::built_in()["providers"]["mistral"]["models"];
    let ids: Vec<String> = models
        .as_object()
        .unwrap()
        .keys()
        .take(2)
        .cloned()
        .collect();
    publish(&env, "mistral", json!({ "m-latest": at(&ids[0]) })).await;
    let added = add(&env, "mistral", "m-latest", json!({})).await;
    publish(&env, "mistral", json!({ "m-latest": at(&ids[1]) })).await;
    assert_eq!(run_job(&env).await, "0 moved, 0 waiting, 1 held");
    assert_eq!(run_job(&env).await, "0 moved, 0 waiting, 0 held");
    let id = added["id"].as_str().unwrap();
    let held = format!(
        "No provider key for `mistral` is set up for this project, so the line stays on `{}`.",
        ids[0]
    );
    assert_eq!(line(&env, id).await["held"], held);
    let r = action(&env, id, "move").await;
    assert_eq!(
        (r.status, &r.body["error"]["message"]),
        (StatusCode::CONFLICT, &json!(held))
    );
}

#[tokio::test]
async fn lines_are_refused_when_kpmr_has_none_and_a_policy_is_checked() {
    let env = env().await;
    publish(
        &env,
        "openai",
        json!({ "gpt-small-latest": at("gpt-4o-mini") }),
    )
    .await;
    let e = &env;
    let refuse = |body: Value| async move {
        e.app
            .post(&format!("{}/lines", base(e)), body, Some(&e.owner.cookie))
            .await
    };
    let r = refuse(json!({ "provider": "openai", "line": "nope-latest" })).await;
    assert_eq!(
        r.body["error"]["message"],
        "KPMR has no line `nope-latest` for `openai`."
    );
    for bad in [
        json!({ "provider": "openai", "line": "gpt-small-latest", "policy": "delay" }),
        json!({ "provider": "openai", "line": "gpt-small-latest", "policy": "delay", "delay_days": 31 }),
        json!({ "provider": "openai", "line": "gpt-small-latest", "policy": "ask", "delay_days": 2 }),
    ] {
        let r = refuse(bad).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            r.body["error"]["message"],
            "`delay_days` must be 1 to 30 with the `delay` policy, and absent otherwise."
        );
    }
    let added = add(&env, "openai", "gpt-small-latest", json!({})).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/lines/{}", base(&env), added["id"].as_str().unwrap()),
            Some(json!({ "policy": "delay", "delay_days": 5 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(
        (r.body["policy"].as_str(), r.body["delay_days"].as_i64()),
        (Some("delay"), Some(5))
    );
    let missing = action(&env, "model_00000000000000000000000000", "move").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}
