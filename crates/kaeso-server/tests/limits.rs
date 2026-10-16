#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::v1::{Answer, Env, call, call_app, messages};
use serde_json::{Value, json};

const TOKENS: i64 = 28;

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn limit(env: &Env, body: Value) -> Value {
    let r = env
        .app
        .post(
            &format!("{}/limits", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn ask(env: &Env, key: &str) -> Answer {
    call(
        env,
        "/v1/messages",
        Some(key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await
}

async fn used(env: &Env, id: &str, expect: &str) {
    used_in(env, &base(env), id, expect).await;
}

async fn used_in(env: &Env, base: &str, id: &str, expect: &str) {
    for _ in 0..200 {
        let r = env
            .app
            .get(&format!("{base}/limits/{id}"), &env.owner.cookie)
            .await;
        if r.body["used"] == expect {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let r = env
        .app
        .get(&format!("{base}/limits/{id}"), &env.owner.cookie)
        .await;
    panic!("limit {id} never reached {expect}: {:?}", r.body);
}

async fn key_of(env: &Env, name: &str, extra: Value) -> (String, String) {
    let mut body = json!({ "name": name });
    if let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object()) {
        b.extend(e.clone());
    }
    let r = env
        .app
        .post(
            &format!("{}/keys", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (
        r.body["secret"].as_str().unwrap().to_owned(),
        r.body["id"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn request_rates_slide_over_sixty_seconds() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "rated", json!({})).await;
    limit(&env, json!({ "level": "key", "key": id, "kind": "request_rate", "period": "minute", "amount": 3 })).await;
    for _ in 0..3 {
        assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    }
    let r = ask(&env, &secret).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("this key's request rate of 3 requests per minute"),
        "{}",
        r.text()
    );
    let retry: u64 = r.header("retry-after").unwrap().parse().unwrap();
    assert!((1..=60).contains(&retry), "{retry}");
    assert_eq!(ask(&env, &env.key).await.status, StatusCode::OK);
    env.app.clock.advance(time::Duration::seconds(61));
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
}

#[tokio::test]
async fn budgets_refuse_after_the_crossing_request_and_reset_at_utc_midnight() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "budgeted", json!({})).await;
    use kaeso_core::clock::Clock;
    let now = env.app.clock.now();
    let tomorrow = now.date().next_day().unwrap().midnight().assume_utc();
    env.app
        .clock
        .advance(tomorrow - now - time::Duration::minutes(1));
    let lim = limit(
        &env,
        json!({ "level": "key", "key": id, "kind": "token_budget", "period": "day", "amount": 50 }),
    )
    .await;
    let lim_id = lim["id"].as_str().unwrap();
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    used(&env, lim_id, "28").await;
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    used(&env, lim_id, "56").await;
    let r = ask(&env, &secret).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let msg = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(
        msg.contains("daily token budget of 50 tokens") && msg.contains("resets at"),
        "{msg}"
    );
    assert!(r.header("retry-after").is_none());
    env.app.clock.advance(time::Duration::minutes(2));
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
}

#[tokio::test]
async fn three_levels_and_model_scopes() {
    let env = common::v1::env(None, None).await;
    let models = env
        .app
        .get(&format!("{}/models", base(&env)), &env.owner.cookie)
        .await;
    let gpt = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "gpt-4o")
        .unwrap()["id"]
        .clone();
    limit(&env, json!({ "level": "project", "kind": "request_rate", "model": gpt, "period": "minute", "amount": 1 })).await;
    let gpt_call = || {
        call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("gpt-4o", "Hi")),
        )
    };
    assert_eq!(gpt_call().await.status, StatusCode::OK);
    let r = gpt_call().await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        r.text().contains("the project's request rate") && r.text().contains("for this model"),
        "{}",
        r.text()
    );
    let other = ask(&env, &env.key).await;
    assert_eq!(
        other.status,
        StatusCode::OK,
        "other models pass: {}",
        other.text()
    );
}

#[tokio::test]
async fn member_budgets_cover_all_their_keys() {
    let env = common::v1::env(None, None).await;
    let lim = limit(&env, json!({ "level": "member", "user": env.owner.user_id, "kind": "spend_budget", "period": "month", "amount": "0.0002" })).await;
    let (second, _) = key_of(&env, "second", json!({})).await;
    assert_eq!(ask(&env, &env.key).await.status, StatusCode::OK);
    assert_eq!(ask(&env, &second).await.status, StatusCode::OK);
    used(&env, lim["id"].as_str().unwrap(), "0.0002").await;
    let r = ask(&env, &env.key).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(
        r.text()
            .contains("the member's monthly spend budget of 0.0002 USD"),
        "{}",
        r.text()
    );
}

struct Place {
    base: String,
    key: String,
}

impl Place {
    async fn limit(&self, env: &Env, body: Value) -> String {
        let r = env
            .app
            .post(
                &format!("{}/limits", self.base),
                body,
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        r.body["id"].as_str().unwrap().to_owned()
    }

    async fn member_limit(&self, env: &Env, body: Value) -> String {
        let mut body = body;
        body["level"] = json!("member");
        body["user"] = json!(env.owner.user_id);
        self.limit(env, body).await
    }

    async fn ask(&self, env: &Env) -> Answer {
        ask(env, &self.key).await
    }

    async fn used(&self, env: &Env, id: &str, expect: &str) {
        used_in(env, &self.base, id, expect).await;
    }
}

async fn place(env: &Env, org: &str, name: Option<&str>) -> Place {
    let project = match name {
        Some(name) => {
            let r = env
                .app
                .post(
                    &format!("/orgs/{org}/projects"),
                    json!({ "name": name }),
                    Some(&env.owner.cookie),
                )
                .await;
            assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
            r.body["id"].as_str().unwrap().to_owned()
        }
        None => env
            .app
            .get(&format!("/orgs/{org}/projects"), &env.owner.cookie)
            .await
            .body["data"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    };
    let base = format!("/orgs/{org}/projects/{project}");
    let r = env
        .app
        .post(
            &format!("{base}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "app" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    Place {
        base,
        key: r.body["secret"].as_str().unwrap().to_owned(),
    }
}

async fn three_places(env: &Env) -> [Place; 3] {
    let first = Place {
        base: base(env),
        key: env.key.clone(),
    };
    let second = place(env, &env.owner.org_id, Some("Second")).await;
    let r = env
        .app
        .post(
            "/me/orgs",
            json!({ "name": "Elsewhere" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let org = r.body["id"].as_str().unwrap().to_owned();
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "anthropic", "name": "anthropic", "secret": common::v1::ANTHROPIC_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let elsewhere = place(env, &org, None).await;
    [first, second, elsewhere]
}

fn refused_by(r: &Answer, what: &str) {
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(r.text().contains(what), "{}", r.text());
}

#[tokio::test]
async fn a_members_rates_count_in_each_project_and_org_on_their_own() {
    let env = common::v1::env(None, None).await;
    let places = three_places(&env).await;
    let mut rates = Vec::new();
    for p in &places {
        rates.push(
            p.member_limit(
                &env,
                json!({ "kind": "request_rate", "period": "minute", "amount": 2 }),
            )
            .await,
        );
    }
    for p in &places {
        assert_eq!(p.ask(&env).await.status, StatusCode::OK);
        assert_eq!(p.ask(&env).await.status, StatusCode::OK);
        refused_by(
            &p.ask(&env).await,
            "the member's request rate of 2 requests per minute",
        );
    }

    env.app.clock.advance(time::Duration::seconds(61));
    for (p, id) in places.iter().zip(&rates) {
        let r = env
            .app
            .call(
                Method::PATCH,
                &format!("{}/limits/{id}", p.base),
                Some(json!({ "amount": 100 })),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        p.member_limit(
            &env,
            json!({ "kind": "token_rate", "period": "minute", "amount": 50 }),
        )
        .await;
    }
    let [first, second, elsewhere] = &places;
    assert_eq!(first.ask(&env).await.status, StatusCode::OK);
    assert_eq!(first.ask(&env).await.status, StatusCode::OK);
    refused_by(
        &first.ask(&env).await,
        "the member's token rate of 50 tokens per minute",
    );
    for p in [second, elsewhere] {
        assert_eq!(p.ask(&env).await.status, StatusCode::OK);
        assert_eq!(p.ask(&env).await.status, StatusCode::OK);
        refused_by(&p.ask(&env).await, "the member's token rate");
    }
}

#[tokio::test]
async fn a_members_budgets_count_in_each_project_and_org_on_their_own() {
    let env = common::v1::env(None, None).await;
    let places = three_places(&env).await;
    let mut tokens = Vec::new();
    let mut spend = Vec::new();
    for p in &places {
        tokens.push(
            p.member_limit(
                &env,
                json!({ "kind": "token_budget", "period": "day", "amount": 50 }),
            )
            .await,
        );
        spend.push(
            p.member_limit(
                &env,
                json!({ "kind": "spend_budget", "period": "month", "amount": "0.001" }),
            )
            .await,
        );
    }
    let [first, second, elsewhere] = &places;
    assert_eq!(first.ask(&env).await.status, StatusCode::OK);
    assert_eq!(first.ask(&env).await.status, StatusCode::OK);
    first.used(&env, &tokens[0], "56").await;
    first.used(&env, &spend[0], "0.0002").await;
    refused_by(
        &first.ask(&env).await,
        "the member's daily token budget of 50 tokens",
    );
    for (i, p) in [(1, second), (2, elsewhere)] {
        p.used(&env, &tokens[i], "0").await;
        p.used(&env, &spend[i], "0").await;
    }
    assert_eq!(second.ask(&env).await.status, StatusCode::OK);
    second.used(&env, &tokens[1], "28").await;
    second.used(&env, &spend[1], "0.0001").await;
    first.used(&env, &tokens[0], "56").await;
    elsewhere.used(&env, &tokens[2], "0").await;

    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    lose_valkey(&env).await;
    refused_by(&first.ask(&env).await, "the member's daily token budget");
    let r = second.ask(&env).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    second.used(&env, &tokens[1], "56").await;
    refused_by(&second.ask(&env).await, "the member's daily token budget");
    assert_eq!(elsewhere.ask(&env).await.status, StatusCode::OK);
    elsewhere.used(&env, &tokens[2], "28").await;
    elsewhere.used(&env, &spend[2], "0.0001").await;
    first.used(&env, &spend[0], "0.0002").await;
}

#[tokio::test]
async fn service_account_and_key_limits_count_only_their_own_requests() {
    let env = common::v1::env(None, None).await;
    let here = Place {
        base: base(&env),
        key: env.key.clone(),
    };
    let (other, other_id) = key_of(&env, "other", json!({})).await;
    let svc = env
        .app
        .post(
            &format!("{}/service-accounts", here.base),
            json!({ "name": "worker", "preset": "developer" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(svc.status, StatusCode::CREATED, "{:?}", svc.body);
    let svc_id = svc.body["id"].as_str().unwrap().to_owned();
    let r = env
        .app
        .post(
            &format!("{}/service-accounts/{svc_id}/keys", here.base),
            json!({ "name": "worker" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let svc_key = r.body["secret"].as_str().unwrap().to_owned();
    let member = here
        .member_limit(
            &env,
            json!({ "kind": "token_budget", "period": "day", "amount": 100 }),
        )
        .await;
    let key_budget = here.limit(&env, json!({ "level": "key", "key": other_id, "kind": "token_budget", "period": "day", "amount": 50 })).await;
    let svc_budget = here.limit(&env, json!({ "level": "member", "service_account": svc_id, "kind": "token_budget", "period": "day", "amount": 50 })).await;
    let svc_rate = here.limit(&env, json!({ "level": "member", "service_account": svc_id, "kind": "request_rate", "period": "minute", "amount": 3 })).await;

    assert_eq!(ask(&env, &other).await.status, StatusCode::OK);
    assert_eq!(ask(&env, &other).await.status, StatusCode::OK);
    refused_by(
        &ask(&env, &other).await,
        "this key's daily token budget of 50 tokens",
    );
    here.used(&env, &key_budget, "56").await;
    here.used(&env, &member, "56").await;
    here.used(&env, &svc_budget, "0").await;
    here.used(&env, &svc_rate, "0").await;
    assert_eq!(ask(&env, &svc_key).await.status, StatusCode::OK);
    assert_eq!(ask(&env, &svc_key).await.status, StatusCode::OK);
    refused_by(
        &ask(&env, &svc_key).await,
        "the member's daily token budget of 50 tokens",
    );
    here.used(&env, &svc_rate, "2").await;
    here.used(&env, &svc_budget, "56").await;
    here.used(&env, &member, "56").await;
    here.used(&env, &key_budget, "56").await;
    assert_eq!(here.ask(&env).await.status, StatusCode::OK);
    here.used(&env, &member, "84").await;
    here.used(&env, &key_budget, "56").await;
    assert_eq!(here.ask(&env).await.status, StatusCode::OK);
    refused_by(
        &here.ask(&env).await,
        "the member's daily token budget of 100 tokens",
    );
    here.used(&env, &svc_budget, "56").await;
    here.used(&env, &svc_rate, "2").await;
}

#[tokio::test]
async fn children_never_exceed_parents() {
    let env = common::v1::env(None, None).await;
    let (_, id) = key_of(&env, "k", json!({})).await;
    let project = limit(
        &env,
        json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": 50 }),
    )
    .await;
    let over = env.app.post(&format!("{}/limits", base(&env)), json!({ "level": "key", "key": id, "kind": "spend_budget", "period": "month", "amount": 60 }), Some(&env.owner.cookie)).await;
    assert_eq!(over.status, StatusCode::BAD_REQUEST, "{:?}", over.body);
    assert!(
        over.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("monthly spend budget of 50 USD")
    );
    limit(&env, json!({ "level": "key", "key": id, "kind": "spend_budget", "period": "month", "amount": 40 })).await;
    limit(
        &env,
        json!({ "level": "key", "key": id, "kind": "spend_budget", "period": "day", "amount": 60 }),
    )
    .await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/limits/{}", base(&env), project["id"].as_str().unwrap()),
            Some(json!({ "amount": 30 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert!(r.body["error"]["message"].as_str().unwrap().contains(&id));
    let bad = env.app.post(&format!("{}/keys", base(&env)), json!({ "name": "x", "limits": [{ "kind": "spend_budget", "period": "month", "amount": 70 }] }), Some(&env.owner.cookie)).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST, "{:?}", bad.body);
    let (_, ok) = key_of(
        &env,
        "tight",
        json!({ "limits": [{ "kind": "request_rate", "period": "minute", "amount": 5 }] }),
    )
    .await;
    let list = env
        .app
        .get(
            &format!("{}/limits?key={ok}", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);
    let dup = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": 10 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    let bad = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "request_rate", "period": "day", "amount": 10 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(bad.body["error"]["param"], "period");
}

#[tokio::test]
async fn soft_budgets_alert_once_per_threshold() {
    let env = common::v1::env(None, None).await;
    let lim = limit(&env, json!({ "level": "project", "kind": "token_budget", "period": "month", "amount": 50, "hard": false, "alert_thresholds": [50, 100] })).await;
    let id = lim["id"].as_str().unwrap();
    assert_eq!(ask(&env, &env.key).await.status, StatusCode::OK);
    used(&env, id, "28").await;
    env.app.wait_for_mail("owner@example.com", 1).await;
    assert_eq!(ask(&env, &env.key).await.status, StatusCode::OK);
    used(&env, id, "56").await;
    env.app.wait_for_mail("owner@example.com", 2).await;
    assert_eq!(ask(&env, &env.key).await.status, StatusCode::OK);
    used(&env, id, "84").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let alerts: Vec<_> = env
        .app
        .mail
        .to("owner@example.com")
        .into_iter()
        .filter(|m| m.subject.contains("monthly token budget"))
        .collect();
    assert_eq!(alerts.len(), 2, "{alerts:?}");
    assert!(alerts[0].subject.contains("50%") && alerts[1].subject.contains("100%"));
    let mut recorded = 0;
    for _ in 0..200 {
        let audit = env
            .app
            .get(
                &format!(
                    "/orgs/{}/audit-log?action=limit.threshold_reached",
                    env.owner.org_id
                ),
                &env.owner.cookie,
            )
            .await;
        recorded = audit.body["data"].as_array().unwrap().len();
        if recorded >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(recorded, 2);
    let r = env
        .app
        .call(
            Method::PUT,
            "/me/notifications",
            Some(json!({ "budget_alerts": false, "key_expiry": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
}

#[tokio::test]
async fn counters_are_rebuilt_from_postgres_after_valkey_loses_them() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "k", json!({})).await;
    let lim = limit(&env, json!({ "level": "key", "key": id, "kind": "token_budget", "period": "total", "amount": 60 })).await;
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    used(&env, lim["id"].as_str().unwrap(), "56").await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    let mut conn = env.services.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{}*", env.services.valkey_prefix))
        .query_async(&mut conn)
        .await
        .unwrap();
    for k in keys {
        let _: () = redis::cmd("DEL")
            .arg(k)
            .query_async(&mut conn)
            .await
            .unwrap();
    }
    let r = ask(&env, &secret).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    used(&env, lim["id"].as_str().unwrap(), "84").await;
    let id: kaeso_core::id::LimitId = lim["id"].as_str().unwrap().parse().unwrap();
    let ttl: i64 = redis::cmd("TTL")
        .arg(
            env.app
                .state
                .valkey
                .key(&format!("lim:{}:total", id.uuid())),
        )
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(ttl > 30 * 86_400, "{ttl}");
    assert_eq!(
        ask(&env, &secret).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn parallel_requests_stay_within_one_request_of_a_budget() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "k", json!({})).await;
    limit(&env, json!({ "level": "key", "key": id, "kind": "request_rate", "period": "minute", "amount": 1000 })).await;
    let lim = limit(&env, json!({ "level": "key", "key": id, "kind": "token_budget", "period": "day", "amount": 200 })).await;
    let mut passed = 0i64;
    loop {
        let wave: Vec<_> = (0..20)
            .map(|_| {
                call(
                    &env,
                    "/v1/messages",
                    Some(&secret),
                    true,
                    Some(messages("claude-haiku-4-5", "Hi [mock:delay=50]")),
                )
            })
            .collect();
        let answers = futures::future::join_all(wave).await;
        passed += answers
            .iter()
            .filter(|a| a.status == StatusCode::OK)
            .count() as i64;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let exhausted = answers.iter().any(|a| {
            a.status == StatusCode::TOO_MANY_REQUESTS && a.header("retry-after").is_none()
        });
        if exhausted {
            break;
        }
    }
    let used_tokens = passed * TOKENS;
    assert!(
        (200..=200 + TOKENS).contains(&used_tokens),
        "{passed} requests passed: {used_tokens} tokens"
    );
    used(&env, lim["id"].as_str().unwrap(), &used_tokens.to_string()).await;

    let (secret, id) = key_of(&env, "rate", json!({})).await;
    limit(&env, json!({ "level": "key", "key": id, "kind": "request_rate", "period": "minute", "amount": 5 })).await;
    let wave: Vec<_> = (0..20).map(|_| ask(&env, &secret)).collect();
    let ok = futures::future::join_all(wave)
        .await
        .iter()
        .filter(|a| a.status == StatusCode::OK)
        .count();
    assert_eq!(ok, 5);
}

async fn wave(env: &Env, key: &str, n: usize) -> Vec<Answer> {
    let calls: Vec<_> = (0..n)
        .map(|_| {
            call(
                env,
                "/v1/messages",
                Some(key),
                true,
                Some(messages("claude-haiku-4-5", "Hi [mock:delay=300]")),
            )
        })
        .collect();
    futures::future::join_all(calls).await
}

fn passed(answers: &[Answer]) -> usize {
    answers
        .iter()
        .filter(|a| a.status == StatusCode::OK)
        .count()
}

async fn lose_valkey(env: &Env) {
    let mut conn = env.services.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{}*", env.services.valkey_prefix))
        .query_async(&mut conn)
        .await
        .unwrap();
    for k in keys {
        let _: () = redis::cmd("DEL")
            .arg(k)
            .query_async(&mut conn)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_rebuilt_budget_projects_requests_in_flight_at_the_period_average() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "k", json!({})).await;
    let lim = limit(&env, json!({ "level": "key", "key": id, "kind": "token_budget", "period": "day", "amount": 400 })).await;
    for _ in 0..5 {
        assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    }
    used(&env, lim["id"].as_str().unwrap(), &(5 * TOKENS).to_string()).await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    lose_valkey(&env).await;
    let answers = wave(&env, &secret, 4).await;
    assert_eq!(
        passed(&answers),
        4,
        "{:?}",
        answers.iter().map(Answer::text).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_new_period_projects_requests_in_flight_at_the_recent_average() {
    let env = common::v1::env(None, None).await;
    let id = |s: &str| -> uuid::Uuid {
        let raw = s.split_once('_').unwrap().1;
        let v = kaeso_core::id::decode_base32(raw).unwrap();
        uuid::Uuid::from_u128(v)
    };
    let (secret, key) = key_of(&env, "k", json!({})).await;
    let yesterday = time::OffsetDateTime::now_utc().date() - time::Duration::days(1);
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[kaeso_store::pg::limits::UsageRow {
            day: yesterday,
            org_id: id(&env.owner.org_id),
            project_id: id(&env.project),
            user_id: Some(id(&env.owner.user_id)),
            service_account_id: None,
            api_key_id: Some(id(&key)),
            catalogue_model_id: None,
            model_id: "claude-haiku-4-5".into(),
            provider: "anthropic".into(),
            requests: 40,
            errors: 30,
            input_tokens: 100,
            output_tokens: 180,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost: rust_decimal::Decimal::ZERO,
        }],
    )
    .await
    .unwrap();
    limit(&env, json!({ "level": "key", "key": key, "kind": "token_budget", "period": "day", "amount": 200 })).await;
    let answers = wave(&env, &secret, 9).await;
    assert_eq!(
        passed(&answers),
        8,
        "{:?}",
        answers.iter().map(Answer::text).collect::<Vec<_>>()
    );
    let (fresh, fresh_id) = key_of(&env, "fresh", json!({})).await;
    limit(&env, json!({ "level": "key", "key": fresh_id, "kind": "token_budget", "period": "day", "amount": 200 })).await;
    let answers = wave(&env, &fresh, 4).await;
    assert_eq!(passed(&answers), 1);
    assert!(
        answers
            .iter()
            .filter(|a| a.status == StatusCode::TOO_MANY_REQUESTS)
            .all(|a| a.header("retry-after") == Some("1"))
    );
}

#[tokio::test]
async fn token_rates_count_a_request_when_it_ends() {
    let env = common::v1::env(None, None).await;
    let (secret, id) = key_of(&env, "k", json!({})).await;
    limit(&env, json!({ "level": "key", "key": id, "kind": "token_rate", "period": "minute", "amount": 30 })).await;
    let long = call(
        &env,
        "/v1/messages",
        Some(&secret),
        true,
        Some(messages("claude-haiku-4-5", "Long [mock:delay=500]")),
    );
    let advance = async {
        for _ in 0..200 {
            if env
                .mock
                .received()
                .iter()
                .any(|r| String::from_utf8_lossy(&r.body).contains("Long"))
            {
                env.app.clock.advance(time::Duration::seconds(61));
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the request never reached the provider");
    };
    let (answer, ()) = tokio::join!(long, advance);
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(ask(&env, &secret).await.status, StatusCode::OK);
    assert_eq!(
        ask(&env, &secret).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn without_valkey_hard_budgets_fail_closed_and_the_rest_open() {
    let env = common::v1::env(None, None).await;
    let (rated, rated_id) = key_of(&env, "rated", json!({})).await;
    let (budgeted, budgeted_id) = key_of(&env, "budgeted", json!({})).await;
    limit(&env, json!({ "level": "key", "key": rated_id, "kind": "request_rate", "period": "minute", "amount": 100 })).await;
    limit(&env, json!({ "level": "key", "key": budgeted_id, "kind": "spend_budget", "period": "month", "amount": 10 })).await;
    let soft = key_of(&env, "soft", json!({})).await;
    limit(&env, json!({ "level": "key", "key": soft.1, "kind": "token_budget", "period": "month", "amount": 1000, "hard": false })).await;
    let down = common::App::start_with(
        &env.services,
        &[
            ("KAESO_VALKEY_URL", "redis://127.0.0.1:1"),
            (
                "KAESO_PROVIDERS_OPENAI_BASE_URL",
                env.mock.base_url.as_str(),
            ),
            (
                "KAESO_PROVIDERS_ANTHROPIC_BASE_URL",
                env.mock.base_url.as_str(),
            ),
        ],
    )
    .await;
    let ask_down = |key: String| {
        let down = &down;
        async move {
            call_app(
                down,
                "/v1/messages",
                Some(&key),
                true,
                Some(messages("claude-haiku-4-5", "Hi")),
            )
            .await
        }
    };
    assert_eq!(
        ask_down(rated).await.status,
        StatusCode::OK,
        "rates fail open"
    );
    assert_eq!(
        ask_down(soft.0).await.status,
        StatusCode::OK,
        "soft budgets fail open"
    );
    let r = ask_down(budgeted).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text());
    assert!(r.text().contains("hard budget"));
}
