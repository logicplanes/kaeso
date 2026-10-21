#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::v1::{Env, call, chat, messages};
use kaeso_core::clock::Clock;
use kaeso_store::clickhouse::logs::{self as ch, ContentRow, LogRow};
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

async fn env(extra: &[(&str, &str)]) -> Env {
    let e = common::v1::env(None, Some(extra)).await;
    if extra.iter().all(|(k, _)| *k != "KAESO_CLICKHOUSE_URL") {
        e.app
            .state
            .clickhouse
            .migrate(&e.app.state.pg)
            .await
            .unwrap();
    }
    e
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn flush(env: &Env) {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    assert_eq!(env.app.state.logs.len(), 0, "the buffer drained");
}

async fn logs(env: &Env, query: &str) -> Value {
    let r = env
        .app
        .get(&format!("{}/logs{query}", base(env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

#[tokio::test]
async fn requests_are_logged_with_metadata_and_content() {
    let env = env(&[]).await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hello")),
    )
    .await;
    let request_id = r.header("kaeso-request-id").unwrap().to_owned();
    env.app.clock.advance(time::Duration::seconds(1));
    let failed = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("claude-haiku-4-5", "[mock:error=rate-limit]")),
    )
    .await;
    assert_eq!(failed.status, StatusCode::TOO_MANY_REQUESTS);
    flush(&env).await;

    let list = logs(&env, "").await;
    let data = list["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["status"], 429);
    assert_eq!(data[0]["error_kind"], "rate_limited");
    assert_eq!(data[0]["provider"], "anthropic");
    assert_eq!(data[0]["ingress_dialect"], "openai_chat");
    let ok = &data[1];
    assert_eq!(ok["id"], request_id);
    assert_eq!(ok["endpoint"], "messages");
    assert_eq!(ok["ingress_dialect"], "anthropic_messages");
    assert_eq!(ok["egress_dialect"], "anthropic_messages");
    assert_eq!(ok["model"], "claude-haiku-4-5");
    assert_eq!(ok["usage"]["input_tokens"], 10);
    assert_eq!(ok["usage"]["output_tokens"], 18);
    assert_eq!(ok["cost"], "0.0001");
    assert_eq!(ok["has_content"], true);
    assert!(ok.get("content").is_none(), "lists carry metadata only");

    let one = env
        .app
        .get(
            &format!("{}/logs/{request_id}", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(one.status, StatusCode::OK, "{:?}", one.body);
    assert_eq!(
        one.body["content"]["request"]["messages"][0]["content"],
        "Hello"
    );
    assert!(
        one.body["content"]["reply"]["blocks"].is_array(),
        "{:?}",
        one.body["content"]
    );
    let all = one.body.to_string();
    assert!(!all.contains(&env.key) && !all.contains("sk-ant-test"));

    assert_eq!(
        logs(&env, "?failed=true").await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        logs(&env, "?provider=anthropic").await["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        logs(&env, "?status=200").await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        logs(&env, "?provider=openai").await["data"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    for bad in ["OpenAI", "open%20ai", "a--b", "x%27%20OR%201%3D1"] {
        let r = env
            .app
            .get(
                &format!("{}/logs?provider={bad}", base(&env)),
                &env.owner.cookie,
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(r.body["error"]["param"], "provider", "{bad}");
    }
    assert_eq!(
        logs(&env, "?error_kind=rate_limited").await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let page = logs(&env, "?limit=1").await;
    assert_eq!(page["has_more"], true);
    let next = logs(
        &env,
        &format!("?limit=1&after={}", page["next_cursor"].as_str().unwrap()),
    )
    .await;
    assert_eq!(next["data"][0]["id"], request_id);
    assert_eq!(next["has_more"], false);

    let usage = env
        .app
        .get(
            &format!("{}/usage?group_by=model", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(usage.status, StatusCode::OK, "{:?}", usage.body);
    let rows = usage.body["data"].as_array().unwrap();
    let haiku = rows
        .iter()
        .find(|r| r["key"] == "anthropic/claude-haiku-4-5")
        .unwrap();
    assert_eq!(haiku["requests"], 2);
    assert_eq!(haiku["errors"], 1);
    assert_eq!(haiku["cost"], "0.0001");
    let org = env
        .app
        .get(
            &format!("/orgs/{}/usage?group_by=project", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(org.body["data"][0]["requests"], 2);
    assert_eq!(org.body["data"][0]["errors"], 1);
    assert_eq!(org.body["data"][0]["key"], env.project);
    for (group, prefix) in [("key", "key_"), ("member", "user:usr_")] {
        let r = env
            .app
            .get(
                &format!("{}/usage?group_by={group}", base(&env)),
                &env.owner.cookie,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        let key = r.body["data"][0]["key"].as_str().unwrap();
        assert!(key.starts_with(prefix), "{group}: {key}");
    }

    let storage = env
        .app
        .get(
            &format!("/orgs/{}/logs/storage", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(storage.status, StatusCode::OK, "{:?}", storage.body);
    assert_eq!(storage.body["entries"], 2);
    assert!(storage.body["logical_bytes"].as_u64().unwrap() > 0);
    assert_eq!(storage.body["projects"][0]["project_id"], env.project);
}

#[tokio::test]
async fn cursors_from_elsewhere_are_refused_as_invalid() {
    use base64::Engine;
    let env = env(&[]).await;
    let id = Uuid::now_v7();
    for text in [
        format!("{}.{id}", i128::MAX),
        format!("{}.{id}", i128::MIN),
        format!("99999999999999999999.{id}"),
        "1760000000000.not-a-uuid".to_owned(),
    ] {
        let cursor = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&text);
        let r = env
            .app
            .get(
                &format!("{}/logs?after={cursor}", base(&env)),
                &env.owner.cookie,
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{text}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "after");
        assert_eq!(
            r.body["error"]["message"],
            "`after` is not a cursor from a previous page."
        );
    }
}

#[tokio::test]
async fn content_follows_the_switches_and_the_rights() {
    let env = env(&[]).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/logs/settings", base(&env)),
            Some(json!({ "content_logging": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.body["effective_content_logging"], false, "{:?}", r.body);
    call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "secret prompt")),
    )
    .await;
    flush(&env).await;
    let entry = &logs(&env, "").await["data"][0];
    assert_eq!(entry["has_content"], false);
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "content_logging": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/logs/settings", base(&env)),
            Some(json!({ "content_logging": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    env.app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "content_logging": true })),
            Some(&env.owner.cookie),
        )
        .await;
    env.app
        .call(
            Method::PATCH,
            &format!("{}/logs/settings", base(&env)),
            Some(json!({ "content_logging": true })),
            Some(&env.owner.cookie),
        )
        .await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "visible prompt")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    flush(&env).await;

    let svc = env
        .app
        .post(
            &format!("{}/service-accounts", base(&env)),
            json!({ "name": "reader", "preset": "viewer" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(svc.status, StatusCode::CREATED, "{:?}", svc.body);
    let key = env
        .app
        .post(
            &format!(
                "{}/service-accounts/{}/keys",
                base(&env),
                svc.body["id"].as_str().unwrap()
            ),
            json!({ "name": "reader", "access": "platform" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let secret = key.body["secret"].as_str().unwrap();
    let r = env
        .app
        .with_key(
            Method::GET,
            &format!("{}/logs/{id}", base(&env)),
            None,
            secret,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["has_content"], true);
    assert_eq!(r.body["content"], Value::Null);
    let r = env
        .app
        .with_key(
            Method::GET,
            &format!("{}/logs/export?include_content=true", base(&env)),
            None,
            secret,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = env
        .app
        .get(&format!("{}/logs/{id}", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(
        r.body["content"]["request"]["messages"][0]["content"],
        "visible prompt"
    );
}

#[tokio::test]
async fn an_archived_projects_content_switch_stays_as_it_was() {
    let env = env(&[]).await;
    let org = env.owner.org_id.clone();
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Old experiments" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let project = format!("/orgs/{org}/projects/{}", r.body["id"].as_str().unwrap());
    let r = env
        .app
        .post(
            &format!("{project}/archive"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{project}/logs/settings"),
            Some(json!({ "content_logging": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "The project is archived and cannot be changed."
    );
    let r = env
        .app
        .get(&format!("{project}/logs/settings"), &env.owner.cookie)
        .await;
    assert_eq!(r.body["content_logging"], true, "{:?}", r.body);
}

#[tokio::test]
async fn streams_that_fail_after_their_headers_are_logged_as_failures() {
    let env = env(&[]).await;
    let mut ok = messages("claude-haiku-4-5", "Hello");
    ok["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(ok)).await;
    assert_eq!(r.status, StatusCode::OK);
    let received = r.body.len() as u64;
    env.app.clock.advance(time::Duration::seconds(1));
    let mut cut = chat("claude-haiku-4-5", "[mock:cut]");
    cut["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(cut),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text().contains("\"error\""), "{}", r.text());
    flush(&env).await;

    let data = logs(&env, "").await["data"].as_array().unwrap().clone();
    assert_eq!(data.len(), 2);
    let broken = &data[0];
    assert_eq!(broken["status"], 502);
    assert_eq!(broken["error_kind"], "stream_interrupted");
    let p = &broken["stream_progress"];
    assert!(p["events"].as_u64().unwrap() > 0, "{broken}");
    assert!(p["bytes"].as_u64().unwrap() > 0, "{broken}");
    assert!(p["first_byte_ms"].is_u64(), "{broken}");
    assert_eq!(p["keepalives"], 0);
    let one = env
        .app
        .get(
            &format!("{}/logs/{}", base(&env), broken["id"].as_str().unwrap()),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        one.body["content"]["reply"]["stop"],
        json!({ "other": "incomplete" }),
        "{:?}",
        one.body
    );
    let fine = &data[1];
    assert_eq!(fine["status"], 200);
    assert!(fine["error_kind"].is_null());
    assert_eq!(fine["stream_progress"]["bytes"], received);
    assert_eq!(
        logs(&env, "?failed=true&error_kind=stream_interrupted").await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let usage = env
        .app
        .get(
            &format!("{}/usage?group_by=model", base(&env)),
            &env.owner.cookie,
        )
        .await;
    let haiku = usage.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"] == "anthropic/claude-haiku-4-5")
        .unwrap()
        .clone();
    assert_eq!(haiku["errors"], 1, "{haiku}");
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    flush(&env).await;
    let one = env
        .app
        .get(&format!("{}/logs/{id}", base(&env)), &env.owner.cookie)
        .await;
    assert!(one.body["stream_progress"].is_null(), "{:?}", one.body);
}

#[tokio::test]
async fn disconnects_charge_limits_with_the_logged_estimate() {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let env = env(&[]).await;
    let r = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "token_budget", "period": "day", "amount": 1_000_000 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let limit = r.body["id"].as_str().unwrap().to_owned();
    let mut body = messages("gpt-4o", "Tell me a story [mock:slow]");
    body["stream"] = json!(true);
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let mut stream = resp.into_body();
    let mut seen = String::new();
    while !seen.contains("text_delta") {
        if let Ok(data) = stream.frame().await.unwrap().unwrap().into_data() {
            seen.push_str(&String::from_utf8_lossy(&data));
        }
    }
    drop(stream);
    let mut entry = Value::Null;
    for _ in 0..300 {
        flush(&env).await;
        if let Some(e) = logs(&env, "?status=499").await["data"].get(0) {
            entry = e.clone();
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(entry["error_kind"], "client_disconnected", "{entry}");
    let usage = &entry["usage"];
    assert_eq!(usage["estimated"], true, "{entry}");
    assert_eq!(usage["input_tokens"], 11, "{entry}");
    assert!(usage["output_tokens"].as_u64().unwrap() >= 1, "{entry}");
    assert_ne!(entry["cost"], "0", "{entry}");
    let total = [
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
    ]
    .iter()
    .map(|k| usage[*k].as_u64().unwrap())
    .sum::<u64>();
    let l = env
        .app
        .get(&format!("{}/limits/{limit}", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(l.body["used"], total.to_string(), "{:?}", l.body);
}

fn row(org: Uuid, project: Uuid, at: OffsetDateTime, i: u64) -> (LogRow, ContentRow) {
    let id = Uuid::now_v7();
    (
        LogRow {
            request_id: id,
            org_id: org,
            project_id: project,
            key_id: Uuid::nil(),
            principal_kind: "user".into(),
            principal_id: Uuid::nil(),
            started_at: at,
            finished_at: at,
            duration_ms: 5,
            ttfb_ms: Some(2),
            endpoint: "messages".into(),
            ingress_dialect: "anthropic_messages".into(),
            egress_dialect: "anthropic_messages".into(),
            provider: "anthropic".into(),
            requested_model: "m".into(),
            resolved_model: "m".into(),
            stream: false,
            status: 200,
            error_kind: String::new(),
            error_message: String::new(),
            input_tokens: i,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost: 0,
            user_agent: "ua, with comma".into(),
            client_ip: String::new(),
            upstream_request_id: String::new(),
            has_content: true,
            logical_bytes: 10,
            expires_at: at + time::Duration::days(90),
            stream_events: 0,
            stream_bytes: 0,
            stream_first_byte_ms: None,
            stream_keepalives: 0,
            usage_estimated: false,
            source: "api".into(),
        },
        ContentRow {
            request_id: id,
            org_id: org,
            project_id: project,
            started_at: at,
            request_body: format!("{{\"n\":{i}}}"),
            reply: "{}".into(),
            expires_at: at + time::Duration::days(30),
        },
    )
}

async fn seed(env: &Env, n: u64, at: OffsetDateTime) {
    let org: kaeso_core::id::OrgId = env.owner.org_id.parse().unwrap();
    let project: kaeso_core::id::ProjectId = env.project.parse().unwrap();
    let (logs, contents): (Vec<_>, Vec<_>) = (0..n)
        .map(|i| {
            row(
                org.uuid(),
                project.uuid(),
                at + time::Duration::milliseconds(i as i64),
                i,
            )
        })
        .unzip();
    ch::insert_logs(&env.app.state.clickhouse, &logs)
        .await
        .unwrap();
    ch::insert_contents(&env.app.state.clickhouse, &contents)
        .await
        .unwrap();
}

#[tokio::test]
async fn exports_stream_large_ranges_in_both_formats() {
    let env = env(&[]).await;
    let now = env.app.clock.now().replace_nanosecond(0).unwrap();
    seed(&env, 2500, now - time::Duration::hours(1)).await;
    let req = axum::http::Request::builder()
        .uri(format!(
            "/platform/v1{}/logs/export?include_content=true",
            base(&env)
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "application/x-ndjson");
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let lines: Vec<Value> = std::str::from_utf8(&body)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2500);
    assert!(lines[0]["content"]["request"]["n"].is_u64());
    let ids: std::collections::HashSet<_> = lines
        .iter()
        .map(|l| l["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 2500);

    let req = axum::http::Request::builder()
        .uri(format!(
            "/platform/v1{}/logs/export?format=csv&from={}",
            base(&env),
            (now - time::Duration::minutes(61))
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.starts_with("id,started_at,"));
    assert_eq!(text.lines().count(), 2501);
    assert!(text.contains("\"ua, with comma\""), "CSV fields are quoted");
}

#[tokio::test]
async fn retention_shrinks_and_deletions_remove_rows() {
    let env = env(&[]).await;
    let now = env.app.clock.now().replace_nanosecond(0).unwrap();
    seed(&env, 3, now - time::Duration::days(50)).await;
    seed(&env, 2, now - time::Duration::days(10)).await;
    seed(&env, 1, now - time::Duration::hours(1)).await;
    let count = |q: &'static str| {
        let env = &env;
        async move { logs(env, q).await["data"].as_array().unwrap().len() }
    };
    assert_eq!(count("?limit=100").await, 6);
    call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "x")),
    )
    .await;
    flush(&env).await;
    let org: kaeso_core::id::OrgId = env.owner.org_id.parse().unwrap();
    let project: kaeso_core::id::ProjectId = env.project.parse().unwrap();
    let rows = ch::search(
        &env.app.state.clickhouse,
        org.uuid(),
        project.uuid(),
        &ch::Filter::default(),
        None,
        1,
    )
    .await
    .unwrap();
    let days = (rows[0].expires_at - rows[0].started_at).whole_days();
    assert!((89..=90).contains(&days), "{days}");

    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "metadata_retention_days": 30, "content_retention_days": 7 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let bad = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "content_retention_days": 40 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    kaeso_server::services::logs::run_jobs(&env.app.state)
        .await
        .unwrap();
    let mut left = 0;
    for _ in 0..100 {
        left = count("?limit=100").await;
        if left == 4 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(left, 4);

    let from = (now - time::Duration::days(11))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let to = (now - time::Duration::days(9))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let r = env
        .app
        .post(
            &format!("/orgs/{}/logs/deletions", env.owner.org_id),
            json!({ "from": from, "to": to }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{:?}", r.body);
    for _ in 0..100 {
        left = count("?limit=100").await;
        if left == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(left, 2);
    let jobs = env
        .app
        .get(
            &format!("/orgs/{}/logs/deletions", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert!(
        jobs.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|j| j["state"] == "done"),
        "{:?}",
        jobs.body
    );
    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log?action=logs.", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(
        actions.contains(&"logs.delete") && actions.contains(&"logs.settings_update"),
        "{actions:?}"
    );
}

#[tokio::test]
async fn no_query_reaches_another_orgs_logs() {
    let env = env(&[]).await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "x")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    flush(&env).await;
    let other = env
        .app
        .post(
            "/me/orgs",
            json!({ "name": "Other" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.body);
    let other_org = other.body["id"].as_str().unwrap();
    let projects = env
        .app
        .get(&format!("/orgs/{other_org}/projects"), &env.owner.cookie)
        .await;
    let other_project = projects.body["data"][0]["id"].as_str().unwrap();
    let list = env
        .app
        .get(
            &format!("/orgs/{other_org}/projects/{other_project}/logs"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 0);
    let one = env
        .app
        .get(
            &format!("/orgs/{other_org}/projects/{other_project}/logs/{id}"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(one.status, StatusCode::NOT_FOUND);
    let storage = env
        .app
        .get(
            &format!("/orgs/{other_org}/logs/storage"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(storage.body["entries"], 0);
    let usage = env
        .app
        .get(&format!("/orgs/{other_org}/usage"), &env.owner.cookie)
        .await;
    assert_eq!(usage.body["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn requests_never_wait_for_clickhouse_and_drops_are_counted() {
    let env = env(&[
        ("KAESO_CLICKHOUSE_URL", "http://127.0.0.1:1"),
        ("KAESO_LOGS_BUFFER_SIZE", "2KiB"),
    ])
    .await;
    for _ in 0..10 {
        let r = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "Hello")),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::OK,
            "requests do not depend on ClickHouse"
        );
    }
    let state = &env.app.state;
    state.in_flight.settled().await;
    assert!(!state.logs.is_empty());
    assert!(
        state.logs.dropped() > 0,
        "the oldest rows were dropped beyond the buffer size"
    );
    let before = state.logs.len();
    assert!(
        kaeso_server::services::logs::flush_once(state)
            .await
            .is_err()
    );
    assert_eq!(state.logs.len(), before, "failed writes keep the rows");
    let r = env
        .app
        .get(&format!("{}/logs", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_shutdown_flush_keeps_trying_and_keeps_what_it_could_not_write() {
    let env = env(&[("KAESO_CLICKHOUSE_URL", "http://127.0.0.1:1")]).await;
    for _ in 0..3 {
        let r = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "Hello")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK);
    }
    let state = &env.app.state;
    common::eventually("the requests are buffered", || async {
        state.logs.len() == 3
    })
    .await;
    let started = std::time::Instant::now();
    let written = kaeso_server::services::logs::flush_all(state, Duration::from_millis(2500)).await;
    assert_eq!(written, 0);
    assert_eq!(state.logs.len(), 3, "unwritten rows stay in the buffer");
    let waited = started.elapsed();
    assert!(waited >= Duration::from_secs(2), "it retried: {waited:?}");
    assert!(waited < Duration::from_secs(10), "{waited:?}");
}

#[tokio::test]
async fn long_metadata_counts_toward_the_log_buffer() {
    let env = env(&[
        ("KAESO_CLICKHOUSE_URL", "http://127.0.0.1:1"),
        ("KAESO_LOGS_BUFFER_SIZE", "64KiB"),
    ])
    .await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "content_logging": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let long = "m".repeat(100_000);
    for _ in 0..3 {
        let r = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages(&long, "Hello")),
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
    }
    let state = &env.app.state;
    state.in_flight.settled().await;
    assert_eq!(
        state.logs.len(),
        0,
        "rows larger than the buffer are not kept"
    );
    assert_eq!(state.logs.dropped(), 3);
}

#[tokio::test]
async fn content_exports_fetch_large_bodies_a_few_at_a_time() {
    let env = env(&[]).await;
    let org: kaeso_core::id::OrgId = env.owner.org_id.parse().unwrap();
    let project: kaeso_core::id::ProjectId = env.project.parse().unwrap();
    let at = env.app.clock.now() - time::Duration::hours(1);
    let (logs, contents): (Vec<_>, Vec<_>) = (0..3u64)
        .map(|i| {
            let (mut log, content) = row(
                org.uuid(),
                project.uuid(),
                at + time::Duration::milliseconds(i as i64),
                i,
            );
            log.logical_bytes = 40 << 20;
            (log, content)
        })
        .unzip();
    ch::insert_logs(&env.app.state.clickhouse, &logs)
        .await
        .unwrap();
    ch::insert_contents(&env.app.state.clickhouse, &contents)
        .await
        .unwrap();
    let req = axum::http::Request::builder()
        .uri(format!(
            "/platform/v1{}/logs/export?include_content=true",
            base(&env)
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = resp.into_body();
    let mut chunks = Vec::new();
    while let Some(frame) = http_body_util::BodyExt::frame(&mut body).await {
        if let Ok(data) = frame.unwrap().into_data()
            && !data.is_empty()
        {
            chunks.push(data);
        }
    }
    assert_eq!(chunks.len(), 3, "one body of 40 MiB per batch");
    for chunk in &chunks {
        let line: Value = serde_json::from_slice(chunk.trim_ascii_end()).unwrap();
        assert!(line["content"]["request"]["n"].is_u64(), "{line}");
    }
}

#[tokio::test]
async fn an_org_queues_only_a_few_log_deletions_at_once() {
    let env = env(&[("KAESO_CLICKHOUSE_URL", "http://127.0.0.1:1")]).await;
    let path = format!("/orgs/{}/logs/deletions", env.owner.org_id);
    let ask = || {
        env.app.post(
            &path,
            json!({ "from": "2026-01-01T00:00:00Z", "to": "2026-02-01T00:00:00Z" }),
            Some(&env.owner.cookie),
        )
    };
    for _ in 0..10 {
        let r = ask().await;
        assert_eq!(r.status, StatusCode::ACCEPTED, "{:?}", r.body);
    }
    let r = ask().await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "10 log deletions are still waiting; try again once they have run."
    );
}

#[tokio::test]
async fn a_request_keeps_the_content_decision_it_started_with() {
    let env = env(&[]).await;
    let switch = |on: bool| {
        let path = format!("{}/logs/settings", base(&env));
        let cookie = env.owner.cookie.clone();
        let app = &env.app;
        async move {
            let r = app
                .call(
                    Method::PATCH,
                    &path,
                    Some(json!({ "content_logging": on })),
                    Some(&cookie),
                )
                .await;
            assert_eq!(r.body["effective_content_logging"], on, "{:?}", r.body);
        }
    };
    switch(false).await;
    let mut gate = env.mock.gate();
    let held = messages("claude-haiku-4-5", &format!("held {}", gate.marker()));
    let sent = call(&env, "/v1/messages", Some(&env.key), true, Some(held));
    let reply = async {
        gate.wait_for(1).await;
        switch(true).await;
        gate.open();
    };
    let (r, ()) = tokio::join!(sent, reply);
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    flush(&env).await;
    let entry = &logs(&env, "").await["data"][0];
    assert_eq!(entry["has_content"], false, "{entry}");
}
