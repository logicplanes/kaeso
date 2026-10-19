#![allow(clippy::unwrap_used)]

mod common;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use common::v1::{Env, messages};
use serde_json::{Value, json};

const LIMIT: u32 = 5;

async fn env(limit: u32) -> Env {
    let limit = limit.to_string();
    common::v1::env(
        None,
        Some(&[("KAESO_KEY_FAILURES_PER_ADDRESS", limit.as_str())]),
    )
    .await
}

fn made_up(n: u32) -> String {
    format!("sk-kso-{n:0>48}")
}

struct Answer {
    status: StatusCode,
    retry_after: Option<u64>,
    body: Value,
}

async fn send(env: &Env, peer: &str, forwarded: Option<&str>, path: &str, key: &str) -> Answer {
    let mut req = if path.starts_with("/platform/") {
        Request::builder()
            .method("GET")
            .uri(path)
            .header("authorization", format!("Bearer {key}"))
            .body(Body::empty())
            .unwrap()
    } else {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&messages("claude-haiku-4-5", "hi")).unwrap(),
            ))
            .unwrap()
    };
    let addr = SocketAddr::new(peer.parse::<IpAddr>().unwrap(), 40000);
    req.extensions_mut().insert(ConnectInfo(addr));
    if let Some(f) = forwarded {
        req.headers_mut()
            .insert("x-forwarded-for", f.parse().unwrap());
    }
    let resp = env.app.raw(req).await;
    let retry_after = resp
        .headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    Answer {
        status: resp.status,
        retry_after,
        body: resp.body,
    }
}

async fn v1(env: &Env, peer: &str, key: &str) -> Answer {
    send(env, peer, None, "/v1/messages", key).await
}

async fn exhaust_rest(env: &Env, peer: &str, forwarded: Option<&str>, used: u32, from: u32) {
    for n in from..from + LIMIT - used {
        let r = send(env, peer, forwarded, "/v1/messages", &made_up(n)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{n}: {:?}", r.body);
    }
    let r = send(env, peer, forwarded, "/v1/messages", &made_up(from + LIMIT)).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", r.body);
}

async fn exhaust(env: &Env, peer: &str, forwarded: Option<&str>, from: u32) {
    exhaust_rest(env, peer, forwarded, 0, from).await;
}

#[tokio::test]
async fn scanning_keys_is_refused_without_postgres_and_valid_keys_keep_working() {
    let env = env(LIMIT).await;
    let scanner = "203.0.113.7";
    assert_eq!(v1(&env, scanner, &env.key).await.status, StatusCode::OK);
    for n in 0..LIMIT {
        let r = v1(&env, scanner, &made_up(n)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{n}: {:?}", r.body);
        assert_eq!(v1(&env, scanner, &env.key).await.status, StatusCode::OK);
    }
    let r = v1(&env, scanner, &made_up(LIMIT)).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", r.body);
    assert_eq!(r.body["type"], "error");
    assert_eq!(r.body["error"]["type"], "rate_limit_error");
    let message = r.body["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Too many requests with unknown API keys"),
        "{message}"
    );
    let retry = r.retry_after.expect("Retry-After");
    assert!((1..=60).contains(&retry), "{retry}");

    let mut blocker = env.app.state.pg.begin().await.unwrap();
    sqlx::query("LOCK TABLE api_keys, org_platform_keys IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    for n in 100..110 {
        let r = tokio::time::timeout(Duration::from_secs(2), v1(&env, scanner, &made_up(n)))
            .await
            .expect("a blocked address waited for PostgreSQL");
        assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    }
    let org_key = format!("sk-kso-org-{:0>48}", 1);
    let r = tokio::time::timeout(Duration::from_secs(2), v1(&env, scanner, &org_key))
        .await
        .expect("a blocked address waited for PostgreSQL");
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    blocker.rollback().await.unwrap();

    assert_eq!(v1(&env, scanner, &env.key).await.status, StatusCode::OK);
    let r = v1(&env, "198.51.100.4", &made_up(200)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let fresh = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "fresh" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        v1(&env, scanner, &fresh).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
    env.app.clock.advance(time::Duration::seconds(61));
    assert_eq!(v1(&env, scanner, &fresh).await.status, StatusCode::OK);
}

#[tokio::test]
async fn only_lookups_that_find_nothing_count() {
    let env = env(LIMIT).await;
    for _ in 0..(LIMIT * 3) {
        let r = v1(&env, "203.0.113.20", &made_up(1)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    }
    for n in 0..(LIMIT * 3) {
        let r = v1(&env, "203.0.113.21", &format!("sk-kso-short{n}")).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    }
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let created = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "off" }),
            Some(&env.owner.cookie),
        )
        .await;
    let (id, secret) = (
        created.body["id"].as_str().unwrap().to_owned(),
        created.body["secret"].as_str().unwrap().to_owned(),
    );
    let r = env
        .app
        .post(
            &format!("{base}/keys/{id}/disable"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert!(r.status.is_success(), "{:?}", r.body);
    for _ in 0..(LIMIT * 3) {
        let r = v1(&env, "203.0.113.22", &secret).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    }
    exhaust_rest(&env, "203.0.113.20", None, 1, 1000).await;
    exhaust(&env, "203.0.113.22", None, 2000).await;
}

#[tokio::test]
async fn the_client_address_follows_the_trusted_proxies() {
    let env = env(LIMIT).await;
    exhaust(&env, "127.0.0.1", Some("198.51.100.20"), 0).await;
    let blocked = send(
        &env,
        "127.0.0.1",
        Some("198.51.100.20"),
        "/v1/messages",
        &made_up(50),
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    let other = send(
        &env,
        "127.0.0.1",
        Some("198.51.100.21"),
        "/v1/messages",
        &made_up(51),
    )
    .await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
    exhaust(&env, "203.0.113.30", Some("198.51.100.99"), 100).await;
    let spoofed = send(
        &env,
        "203.0.113.30",
        Some("198.51.100.77"),
        "/v1/messages",
        &made_up(200),
    )
    .await;
    assert_eq!(spoofed.status, StatusCode::TOO_MANY_REQUESTS);
    let victim = send(
        &env,
        "127.0.0.1",
        Some("198.51.100.99"),
        "/v1/messages",
        &made_up(201),
    )
    .await;
    assert_eq!(victim.status, StatusCode::UNAUTHORIZED);
    for n in 0..LIMIT {
        let peer = format!("2001:db8:5:6::{}", n + 1);
        let r = send(&env, &peer, None, "/v1/messages", &made_up(300 + n)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    let r = send(
        &env,
        "2001:db8:5:6:ffff::1",
        None,
        "/v1/messages",
        &made_up(400),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    let r = send(&env, "2001:db8:5:7::1", None, "/v1/messages", &made_up(401)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn private_networks_are_not_trusted_unless_configured() {
    let env = env(LIMIT).await;
    exhaust(&env, "10.0.0.2", Some("198.51.100.20"), 0).await;
    let spoofed = send(
        &env,
        "10.0.0.2",
        Some("198.51.100.21"),
        "/v1/messages",
        &made_up(50),
    )
    .await;
    assert_eq!(
        spoofed.status,
        StatusCode::TOO_MANY_REQUESTS,
        "a forwarded address from an untrusted peer is ignored"
    );

    let trusting = common::v1::env(
        None,
        Some(&[
            ("KAESO_KEY_FAILURES_PER_ADDRESS", "5"),
            ("KAESO_TRUSTED_PROXIES", "10.0.0.0/8"),
        ]),
    )
    .await;
    exhaust(&trusting, "10.0.0.2", Some("198.51.100.20"), 0).await;
    let other = send(
        &trusting,
        "10.0.0.2",
        Some("198.51.100.21"),
        "/v1/messages",
        &made_up(51),
    )
    .await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_platform_api_shares_the_count_and_answers_in_its_envelope() {
    let env = env(LIMIT).await;
    let peer = "203.0.113.40";
    let path = format!("/platform/v1/orgs/{}/projects", env.owner.org_id);
    for n in 0..3 {
        assert_eq!(
            v1(&env, peer, &made_up(n)).await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    for n in 3..LIMIT {
        let r = send(&env, peer, None, &path, &made_up(n)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
        assert_eq!(r.body["error"]["type"], "authentication");
    }
    let org_key = format!("sk-kso-org-{:0>48}", 7);
    let r = send(&env, peer, None, &path, &org_key).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", r.body);
    assert_eq!(r.body["error"]["type"], "rate_limited");
    let message = r.body["error"]["message"].as_str().unwrap();
    assert!(common::copy::sent_problem(message).is_none(), "{message}");
    assert!(r.retry_after.is_some_and(|s| (1..=60).contains(&s)));
    assert_eq!(
        v1(&env, peer, &made_up(99)).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
    let r = env
        .app
        .get(
            &format!("/orgs/{}/projects", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
}

#[tokio::test]
async fn the_throttle_can_be_turned_off() {
    let env = env(0).await;
    for n in 0..(LIMIT * 4) {
        let r = v1(&env, "203.0.113.50", &made_up(n)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{n}");
    }
}
