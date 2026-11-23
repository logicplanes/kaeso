#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// Organizations' webhooks: what they receive, signed, and how deliveries are tried again.

use crate::common;

use axum::http::{Method, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::v1::{Answer, Env, call, messages};
use kaeso_core::clock::Clock;
use kaeso_testkit::webhooks::{Delivered, MockReceiver};
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = ("KAESO_WEBHOOKS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn env() -> Env {
    common::v1::env(None, Some(&[LOOPBACK])).await
}

fn hooks(env: &Env) -> String {
    format!("/orgs/{}/webhooks", env.owner.org_id)
}

async fn add(env: &Env, body: Value) -> Value {
    let r = env
        .app
        .post(&hooks(env), body, Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn change(env: &Env, id: &str, body: Value) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &format!("{}/{id}", hooks(env)),
            Some(body),
            Some(&env.owner.cookie),
        )
        .await
}

// Runs the `webhooks` job once, as every gateway does every few seconds.
async fn deliver(env: &Env) -> String {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::webhooks::run(env.app.state.clone())
        .await
        .unwrap()
        .summary
}

async fn ask(env: &Env) -> Answer {
    call(
        env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await
}

// A token budget the first request takes past 80 percent: 28 of 30 tokens.
async fn budget(env: &Env) -> String {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project),
            json!({ "level": "project", "kind": "token_budget", "period": "month", "amount": 30, "hard": false, "alert_thresholds": [80] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

fn verify(d: &Delivered, secret: &str) {
    let id = d.header("webhook-id").unwrap();
    let timestamp = d.header("webhook-timestamp").unwrap();
    let key = STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap())
        .unwrap();
    let signed = format!("{id}.{timestamp}.{}", d.body);
    let expected = format!(
        "v1,{}",
        STANDARD.encode(kaeso_crypto::hmac_sha256(&key, signed.as_bytes()))
    );
    assert_eq!(d.header("webhook-signature"), Some(expected.as_str()));
    assert_eq!(d.header("content-type"), Some("application/json"));
}

async fn deliveries(env: &Env, id: &str) -> Vec<Value> {
    let r = env
        .app
        .get(
            &format!("{}/{id}/deliveries", hooks(env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].as_array().unwrap().clone()
}

#[tokio::test]
async fn a_budget_past_80_percent_posts_a_signed_event() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    let created = add(
        &env,
        json!({ "name": "On-call", "url": receiver.url, "kinds": ["budget_alerts"] }),
    )
    .await;
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("whsec_"), "{secret}");
    assert_eq!(created["format"], "json");
    assert_eq!(created["enabled"], true);
    let limit = budget(&env).await;
    assert_eq!(ask(&env).await.status, StatusCode::OK);
    env.app.wait_for_mail("owner@example.com", 1).await;
    deliver(&env).await;

    let [got] = &receiver.got()[..] else {
        panic!("{:?}", receiver.got());
    };
    verify(got, &secret);
    let event = got.json();
    assert_eq!(event["type"], "budget.alert");
    assert_eq!(
        Some(event["id"].as_str().unwrap()),
        got.header("webhook-id")
    );
    assert!(event["id"].as_str().unwrap().starts_with("msg_"));
    assert_eq!(event["organization"]["id"], env.owner.org_id.to_string());
    assert_eq!(event["project"]["id"], env.project);
    assert_eq!(
        event["data"],
        json!({
            "limit_id": limit,
            "kind": "token_budget",
            "period": "month",
            "amount": "30",
            "used": "28",
            "percent": 80,
            "applies_to": { "type": "project" },
            "resets_at": event["data"]["resets_at"],
        })
    );
    assert!(event["data"]["resets_at"].as_str().unwrap().ends_with('Z'));
    assert!(
        event["summary"].as_str().unwrap().contains("80%"),
        "{event}"
    );
    assert!(
        event["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/projects/{}/limits", env.project)),
        "{event}"
    );
    let timestamp: i64 = got.header("webhook-timestamp").unwrap().parse().unwrap();
    assert_eq!(timestamp, env.app.clock.now().unix_timestamp());

    let list = deliveries(&env, created["id"].as_str().unwrap()).await;
    assert_eq!(list[0]["type"], "budget.alert");
    assert_eq!(list[0]["status"], "delivered");
    assert_eq!(list[0]["attempts"], 1);
    assert_eq!(list[0]["last_status"], 204);
    assert_eq!(list[0]["id"], event["id"]);
    deliver(&env).await;
    assert_eq!(receiver.got().len(), 1, "sent once");
}

#[tokio::test]
async fn a_receiver_that_was_down_for_ten_minutes_still_gets_the_event() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    let created = add(
        &env,
        json!({ "name": "Ops", "url": receiver.url, "kinds": ["budget_alerts"] }),
    )
    .await;
    let id = created["id"].as_str().unwrap();
    budget(&env).await;
    receiver.answer(503);
    assert_eq!(ask(&env).await.status, StatusCode::OK);
    env.app.wait_for_mail("owner@example.com", 1).await;
    deliver(&env).await;
    assert_eq!(receiver.got().len(), 1);
    let [pending] = &deliveries(&env, id).await[..] else {
        panic!()
    };
    assert_eq!(
        (
            &pending["status"],
            &pending["attempts"],
            &pending["last_status"]
        ),
        (&json!("pending"), &json!(1), &json!(503))
    );
    assert!(pending["next_attempt_at"].is_string());
    deliver(&env).await;
    assert_eq!(receiver.got().len(), 1, "not before its next try");

    // Down for the tries after 1 and 6 minutes; back before the one after 21.
    for wait in [61, 5 * 60] {
        env.app.clock.advance(time::Duration::seconds(wait));
        deliver(&env).await;
    }
    assert_eq!(receiver.got().len(), 3);
    receiver.answer(200);
    env.app.clock.advance(time::Duration::minutes(10));
    deliver(&env).await;
    assert_eq!(
        receiver.got().len(),
        3,
        "the next try is 15 minutes after the last"
    );
    env.app.clock.advance(time::Duration::minutes(5));
    deliver(&env).await;
    let got = receiver.got();
    assert_eq!(got.len(), 4);
    let ids: Vec<_> = got.iter().map(|d| d.header("webhook-id")).collect();
    assert!(
        ids.iter().all(|i| *i == ids[0]),
        "one event, one id: {ids:?}"
    );
    let [done] = &deliveries(&env, id).await[..] else {
        panic!()
    };
    assert_eq!(
        (&done["status"], &done["attempts"], &done["last_status"]),
        (&json!("delivered"), &json!(4), &json!(200))
    );
    let hook = env
        .app
        .get(&format!("{}/{id}", hooks(&env)), &env.owner.cookie)
        .await;
    assert_eq!(hook.body["consecutive_failures"], 0);
    assert_eq!(hook.body["last_status"], 200);
}

#[tokio::test]
async fn after_five_tries_a_delivery_fails_for_good() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    let created = add(
        &env,
        json!({ "name": "Ops", "url": receiver.url, "kinds": ["budget_alerts"] }),
    )
    .await;
    budget(&env).await;
    receiver.answer(500);
    assert_eq!(ask(&env).await.status, StatusCode::OK);
    env.app.wait_for_mail("owner@example.com", 1).await;
    for _ in 0..8 {
        deliver(&env).await;
        env.app.clock.advance(time::Duration::hours(1));
    }
    assert_eq!(receiver.got().len(), 5);
    let [failed] = &deliveries(&env, created["id"].as_str().unwrap()).await[..] else {
        panic!()
    };
    assert_eq!(
        (&failed["status"], &failed["attempts"]),
        (&json!("failed"), &json!(5))
    );
    assert_eq!(
        failed["last_error"],
        "The receiver answered 500 Internal Server Error."
    );
    assert!(failed["next_attempt_at"].is_null());
}

#[tokio::test]
async fn webhooks_get_only_their_kinds_their_project_and_nothing_while_off() {
    let env = env().await;
    let all = MockReceiver::start().await;
    let other_kind = MockReceiver::start().await;
    let other_project = MockReceiver::start().await;
    let off = MockReceiver::start().await;
    add(
        &env,
        json!({ "name": "All", "url": all.url, "kinds": ["budget_alerts"], "project": env.project }),
    )
    .await;
    add(
        &env,
        json!({ "name": "Keys", "url": other_kind.url, "kinds": ["provider_keys"] }),
    )
    .await;
    let second = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": "Second" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(second.status, StatusCode::CREATED, "{:?}", second.body);
    add(
        &env,
        json!({ "name": "Second", "url": other_project.url, "kinds": ["budget_alerts"], "project": second.body["id"] }),
    )
    .await;
    let turned_off = add(
        &env,
        json!({ "name": "Off", "url": off.url, "kinds": ["budget_alerts"] }),
    )
    .await;
    let r = change(
        &env,
        turned_off["id"].as_str().unwrap(),
        json!({ "enabled": false }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (r.body["enabled"].clone(), r.body["disabled_reason"].clone()),
        (json!(false), Value::Null)
    );
    budget(&env).await;
    assert_eq!(ask(&env).await.status, StatusCode::OK);
    env.app.wait_for_mail("owner@example.com", 1).await;
    deliver(&env).await;
    assert_eq!(all.got().len(), 1);
    assert!(other_kind.got().is_empty());
    assert!(other_project.got().is_empty());
    assert!(off.got().is_empty());
}

#[tokio::test]
async fn webhooks_get_their_event_whatever_people_chose_for_their_emails() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    add(
        &env,
        json!({ "name": "On-call", "url": receiver.url, "kinds": ["budget_alerts"], "format": "slack" }),
    )
    .await;
    let r = env
        .app
        .call(
            Method::PUT,
            "/me/notifications",
            Some(json!({ "kinds": { "budget_alerts": "off" } })),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    budget(&env).await;
    assert_eq!(ask(&env).await.status, StatusCode::OK);
    // No email to wait for: the alert's event is queued as the request is charged.
    for _ in 0..200 {
        deliver(&env).await;
        if !receiver.got().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        env.app
            .mail
            .to("owner@example.com")
            .iter()
            .all(|m| !m.subject.contains("budget"))
    );
    let [got] = &receiver.got()[..] else {
        panic!("{:?}", receiver.got());
    };
    let text = got.json()["text"].as_str().unwrap().to_owned();
    assert!(text.starts_with("[Acme"), "{text}");
    assert!(text.contains("80%") && text.contains("/limits"), "{text}");
    assert_eq!(got.json().as_object().unwrap().len(), 1, "only `text`");
}

#[tokio::test]
async fn the_test_event_is_sent_at_once_and_a_rotated_secret_signs_the_next() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    let created = add(
        &env,
        json!({ "name": "On-call", "url": receiver.url, "kinds": ["provider_keys"] }),
    )
    .await;
    let id = created["id"].as_str().unwrap();
    let test = format!("{}/{id}/test", hooks(&env));
    let r = env
        .app
        .post(&test, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        (&r.body["type"], &r.body["status"], &r.body["last_status"]),
        (&json!("webhook.test"), &json!("delivered"), &json!(204))
    );
    let [got] = &receiver.got()[..] else { panic!() };
    verify(got, created["secret"].as_str().unwrap());
    assert_eq!(got.json()["data"]["webhook_id"], id);

    let rotated = env
        .app
        .post(
            &format!("{}/{id}/secret", hooks(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(rotated.status, StatusCode::OK, "{:?}", rotated.body);
    let secret = rotated.body["secret"].as_str().unwrap();
    assert_ne!(secret, created["secret"]);
    receiver.answer(500);
    let r = env
        .app
        .post(&test, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(
        (&r.body["status"], &r.body["last_status"]),
        (&json!("failed"), &json!(500)),
        "a test is tried once"
    );
    verify(&receiver.got()[1], secret);

    let r = change(&env, id, json!({ "enabled": false })).await;
    assert_eq!(r.status, StatusCode::OK);
    let r = env
        .app
        .post(&test, json!({}), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
}

#[tokio::test]
async fn twenty_failures_in_a_row_turn_a_webhook_off_and_tell_its_managers() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    receiver.answer(502);
    let created = add(
        &env,
        json!({ "name": "Flaky", "url": receiver.url, "kinds": ["provider_keys"] }),
    )
    .await;
    let id = created["id"].as_str().unwrap();
    let test = format!("{}/{id}/test", hooks(&env));
    for _ in 0..20 {
        let r = env
            .app
            .post(&test, json!({}), Some(&env.owner.cookie))
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }
    let hook = env
        .app
        .get(&format!("{}/{id}", hooks(&env)), &env.owner.cookie)
        .await;
    assert_eq!(
        (
            &hook.body["enabled"],
            &hook.body["disabled_reason"],
            &hook.body["consecutive_failures"]
        ),
        (&json!(false), &json!("failures"), &json!(20))
    );
    env.app.wait_for_mail("owner@example.com", 1).await;
    let mail = env.app.mail.to("owner@example.com");
    assert!(
        mail.iter()
            .any(|m| m.subject.contains("The webhook Flaky was turned off")),
        "{mail:?}"
    );
    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log?action=webhook.update", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        audit.body["data"][0]["actor"]["kind"], "system",
        "{:?}",
        audit.body
    );

    let r = change(&env, id, json!({ "enabled": true })).await;
    assert_eq!(
        (
            &r.body["enabled"],
            &r.body["disabled_reason"],
            &r.body["consecutive_failures"]
        ),
        (&json!(true), &Value::Null, &json!(0))
    );
}

#[tokio::test]
async fn addresses_a_webhook_may_not_reach_are_refused() {
    let env = common::v1::env(None, None).await;
    for (url, says) in [
        (
            "http://127.0.0.1:9/hook",
            "an address this install does not allow",
        ),
        (
            "https://10.1.2.3/hook",
            "an address this install does not allow",
        ),
        (
            "https://169.254.169.254/latest",
            "an address this install does not allow",
        ),
        (
            "ftp://hooks.example.com/kaeso",
            "must start with `https://`",
        ),
        (
            "https://user:pass@hooks.example.com/kaeso",
            "without a user",
        ),
    ] {
        let r = env
            .app
            .post(
                &hooks(&env),
                json!({ "name": "X", "url": url, "kinds": ["budget_alerts"] }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{url}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "url");
        assert!(
            r.body["error"]["message"].as_str().unwrap().contains(says),
            "{url}: {:?}",
            r.body
        );
    }
}

#[tokio::test]
async fn kinds_projects_and_the_limit_of_ten_are_checked() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    for (body, param) in [
        (
            json!({ "name": "X", "url": receiver.url, "kinds": ["account_security"] }),
            "kinds",
        ),
        (
            json!({ "name": "X", "url": receiver.url, "kinds": [] }),
            "kinds",
        ),
        (
            json!({ "name": "", "url": receiver.url, "kinds": ["budget_alerts"] }),
            "name",
        ),
        (
            json!({ "name": "X", "url": receiver.url, "kinds": ["budget_alerts"], "project": "proj_01k7a3f8m2xq4v9c5r6t8w0yz1" }),
            "project",
        ),
    ] {
        let r = env
            .app
            .post(&hooks(&env), body.clone(), Some(&env.owner.cookie))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{body}");
    }
    let created = add(
        &env,
        json!({ "name": "A", "url": receiver.url, "kinds": ["budget_alerts", "budget_alerts", "limit_reached"] }),
    )
    .await;
    assert_eq!(created["kinds"], json!(["budget_alerts", "limit_reached"]));
    for n in 1..10 {
        add(
            &env,
            json!({ "name": format!("H{n}"), "url": receiver.url, "kinds": ["budget_alerts"] }),
        )
        .await;
    }
    let r = env
        .app
        .post(
            &hooks(&env),
            json!({ "name": "Eleventh", "url": receiver.url, "kinds": ["budget_alerts"] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let list = env.app.get(&hooks(&env), &env.owner.cookie).await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 10);
    assert!(
        !list.body.to_string().contains("whsec_"),
        "the secret is shown only once"
    );
}

#[tokio::test]
async fn managing_webhooks_takes_its_right_and_every_change_is_audited() {
    let env = env().await;
    let receiver = MockReceiver::start().await;
    let (dev, _) = common::join(
        &env.app,
        &env.owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let r = env.app.get(&hooks(&env), &dev).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["right"], "webhooks.view");
    let r = env
        .app
        .post(
            &hooks(&env),
            json!({ "name": "X", "url": receiver.url, "kinds": ["budget_alerts"] }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["right"], "webhooks.manage");

    let created = add(
        &env,
        json!({ "name": "A", "url": receiver.url, "kinds": ["budget_alerts"] }),
    )
    .await;
    let id = created["id"].as_str().unwrap();
    assert_eq!(
        change(&env, id, json!({ "name": "B", "project": env.project }))
            .await
            .status,
        StatusCode::OK
    );
    let r = change(&env, id, json!({ "project": null })).await;
    assert_eq!(r.body["project"], Value::Null);
    env.app
        .post(
            &format!("{}/{id}/secret", hooks(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("{}/{id}", hooks(&env)),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = env
        .app
        .get(&format!("{}/{id}", hooks(&env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .filter(|a| a.starts_with("webhook."))
        .collect();
    assert_eq!(
        actions,
        [
            "webhook.delete",
            "webhook.secret_rotate",
            "webhook.update",
            "webhook.update",
            "webhook.create"
        ]
    );
    assert!(
        !audit.body.to_string().contains("whsec_"),
        "the audit log never holds a secret"
    );
}
