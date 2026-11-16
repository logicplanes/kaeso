#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::http::{Method, StatusCode};
use common::v1::{Env, OPENAI_SECRET, call, chat};
use serde_json::{Value, json};

const FIRST: &str = "sk-test-pool-first-key-00001111";
const SECOND: &str = "sk-test-pool-second-key-0002222";

async fn add_key(env: &Env, name: &str, secret: &str) -> String {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "openai", "name": name, "secret": secret }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

async fn assign(env: &Env, key: &str, body: Value) -> common::Resp {
    env.app
        .call(
            Method::PUT,
            &format!("/orgs/{}/credentials/{key}/projects", env.owner.org_id),
            Some(body),
            Some(&env.owner.cookie),
        )
        .await
}

// Two keys of OpenAI in the project's pool, besides the org's default, which the project no longer
// uses once keys are assigned to it.
async fn pool() -> (Env, String, String) {
    let env = common::v1::env(None, None).await;
    let first = add_key(&env, "First", FIRST).await;
    let second = add_key(&env, "Second", SECOND).await;
    for key in [&first, &second] {
        let r = assign(&env, key, json!({ "projects": [env.project] })).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }
    (env, first, second)
}

fn used(env: &Env) -> String {
    let auth = env.mock.last().unwrap();
    let auth = auth.header("authorization").unwrap();
    auth.trim_start_matches("Bearer ").to_owned()
}

async fn ask(env: &Env, text: &str) -> common::v1::Answer {
    call(
        env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", text)),
    )
    .await
}

#[tokio::test]
async fn a_caller_stays_on_its_key_and_a_rate_limited_key_gives_way_to_the_next_then_waits() {
    let (env, _, _) = pool().await;
    let mut drawn = Vec::new();
    for _ in 0..4 {
        let r = ask(&env, "Hi").await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        drawn.push(used(&env));
    }
    let mine = drawn[0].clone();
    assert!(
        drawn.iter().all(|k| *k == mine),
        "one caller, one key: {drawn:?}"
    );
    assert!(
        [FIRST, SECOND].contains(&mine.as_str()),
        "the default is out of the pool"
    );
    let other = if mine == FIRST { SECOND } else { FIRST };

    let before = env.mock.received().len();
    let r = ask(
        &env,
        &format!("Hi [mock:key-fail={mine} rate-limit-tokens-429]"),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "the pool's other key answers: {}",
        r.text()
    );
    assert_eq!(used(&env), other);
    assert_eq!(
        env.mock.received().len(),
        before + 2,
        "both keys were asked"
    );

    let before = env.mock.received().len();
    let r = ask(
        &env,
        &format!("Hi [mock:key-fail={mine} rate-limit-tokens-429]"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.received().len(),
        before + 1,
        "the held key is skipped"
    );
    assert_eq!(used(&env), other);

    env.app.clock.advance(time::Duration::seconds(3));
    let r = ask(&env, "Hi").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        used(&env),
        mine,
        "after its retry-after the caller's key is back"
    );
}

#[tokio::test]
async fn a_refused_key_gives_way_but_a_failure_of_the_request_does_not() {
    let (env, _, _) = pool().await;
    ask(&env, "Hi").await;
    let mine = used(&env);
    let r = ask(
        &env,
        &format!("Hi [mock:key-fail={mine} invalid-api-key-401]"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_ne!(used(&env), mine);

    let before = env.mock.received().len();
    let r = ask(&env, "Hi [mock:fail=gpt-4o context-length-exceeded-400]").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        env.mock.received().len(),
        before + 1,
        "no other key is tried"
    );

    let before = env.mock.received().len();
    let r = ask(
        &env,
        &format!("Hi [mock:key-fail={FIRST} rate-limit-tokens-429] [mock:key-fail={SECOND} rate-limit-tokens-429]"),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(env.mock.received().len() <= before + 2);
}

#[tokio::test]
async fn an_alias_step_says_how_many_keys_it_tried_and_falls_back_only_when_all_are_limited() {
    let (env, _, _) = pool().await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let models = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let id = |name: &str| {
        models.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == name)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let r = env
        .app
        .post(
            &format!("{base}/aliases"),
            json!({ "name": "sturdy", "route": { "type": "model", "model": id("gpt-4o"), "fallback": { "type": "model", "model": id("claude-haiku-4-5") } } }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let alias = |text: &str| {
        call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat("sturdy", text)),
        )
    };
    alias("Hi").await;
    let mine = used(&env);
    let r = alias(&format!("Hi [mock:key-fail={mine} rate-limit-tokens-429]")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"), "no fallback");
    let route = r.header("kaeso-route").unwrap().to_owned();
    assert!(
        route.contains(";status=200") && route.contains(";tries=2"),
        "{route}"
    );

    let r = alias(&format!(
        "Hi [mock:key-fail={FIRST} rate-limit-tokens-429] [mock:key-fail={SECOND} rate-limit-tokens-429]"
    ))
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.header("kaeso-model"),
        Some("claude-haiku-4-5"),
        "every key was limited"
    );
}

#[tokio::test]
async fn weights_are_listed_checked_and_kept_per_project() {
    let (env, first, _) = pool().await;
    let r = assign(
        &env,
        &first,
        json!({ "projects": [env.project], "weights": { env.project.clone(): 30 } }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["projects"][0]["weight"], 30);
    for (weights, message) in [
        (json!({ env.project.clone(): 0 }), "A weight is 1 to 100."),
        (json!({ env.project.clone(): 101 }), "A weight is 1 to 100."),
        (
            json!({ "proj_00000000000000000000000000": 2 }),
            "`proj_00000000000000000000000000` is not one of the key's `projects`.",
        ),
    ] {
        let r = assign(
            &env,
            &first,
            json!({ "projects": [env.project], "weights": weights }),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{message}: {:?}", r.body);
        assert_eq!(r.body["error"]["message"], message);
        assert_eq!(r.body["error"]["param"], "weights");
    }
    let listed = env
        .app
        .get(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let mut weights: Vec<(String, Value)> = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|k| !k["projects"].as_array().unwrap().is_empty())
        .map(|k| {
            (
                k["name"].as_str().unwrap().to_owned(),
                k["projects"][0]["weight"].clone(),
            )
        })
        .collect();
    weights.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        weights,
        [
            ("First".to_owned(), json!(30)),
            ("Second".to_owned(), json!(1))
        ]
    );

    let r = assign(&env, &first, json!({ "projects": [] })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    for _ in 0..3 {
        ask(&env, "Hi").await;
        assert_eq!(used(&env), SECOND, "a pool of one");
    }
    let second = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Second")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assign(&env, &second, json!({ "projects": [] })).await;
    ask(&env, "Hi").await;
    assert_eq!(used(&env), OPENAI_SECRET, "no key assigned: the default");
}
