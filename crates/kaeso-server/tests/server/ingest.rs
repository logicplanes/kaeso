#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::io::Write;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::Env;
use serde_json::{Value, json};

async fn env() -> Env {
    let e = common::v1::env(None, None).await;
    e.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&e.app.state.pg)
        .await
        .unwrap();
    e
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn source(env: &Env, kind: &str) -> (String, String) {
    let r = env
        .app
        .post(
            &format!("{}/sources", base(env)),
            json!({ "kind": kind, "name": kind, "content": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (
        r.body["source"]["id"].as_str().unwrap().to_owned(),
        r.body["key"].as_str().unwrap().to_owned(),
    )
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/server/fixtures/sources/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

async fn post(
    env: &Env,
    path: &str,
    key: Option<&str>,
    content_type: &str,
    gzip: bool,
    body: Vec<u8>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", content_type);
    if let Some(k) = key {
        req = req.header("authorization", format!("Bearer {k}"));
    }
    let body = if gzip {
        req = req.header("content-encoding", "gzip");
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&body).unwrap();
        z.finish().unwrap()
    } else {
        body
    };
    env.app.raw_bytes(req.body(Body::from(body)).unwrap()).await
}

async fn flush(env: &Env) {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
}

async fn logs(env: &Env, query: &str) -> Vec<Value> {
    let r = env
        .app
        .get(&format!("{}/logs{query}", base(env)), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].as_array().unwrap().clone()
}

// The fixtures as if their records had just been made: Kaeso refuses records older than 7 days,
// and the log forgets old ones. Every time in them moves by the same amount.
fn fresh(env: &Env, name: &str) -> Vec<u8> {
    let raw = fixture(name);
    let now = kaeso_core::clock::Clock::now(&*env.app.state.clock).unix_timestamp_nanos() as u64;
    if name.ends_with(".pb") {
        let groups = kaeso_server::ingest::otlp::logs_from_protobuf(&raw, usize::MAX).unwrap();
        let times: Vec<u64> = groups
            .iter()
            .flat_map(|g| g.items.iter())
            .flat_map(|r| [r.time_unix_nano, r.observed_time_unix_nano])
            .filter(|t| *t > 0)
            .collect();
        let delta = now - times.iter().max().copied().unwrap_or(now);
        let mut out = raw.clone();
        for t in times {
            let (from, to) = (t.to_le_bytes(), (t + delta).to_le_bytes());
            for at in 0..out.len().saturating_sub(7) {
                if out[at..at + 8] == from {
                    out[at..at + 8].copy_from_slice(&to);
                }
            }
        }
        return out;
    }
    let mut v: Value = serde_json::from_slice(&raw).unwrap();
    fn times(v: &Value, out: &mut Vec<u64>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if k.ends_with("UnixNano") {
                        if let Some(t) = x.as_str().and_then(|s| s.parse().ok()).or(x.as_u64()) {
                            out.push(t);
                        }
                    } else {
                        times(x, out);
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| times(x, out)),
            _ => {}
        }
    }
    fn shift(v: &mut Value, delta: u64) {
        match v {
            Value::Object(m) => {
                let event_time = m.get("key").and_then(Value::as_str) == Some("event.timestamp");
                for (k, x) in m.iter_mut() {
                    if k.ends_with("UnixNano") {
                        let t = x
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                            .or(x.as_u64());
                        if let Some(t) = t.filter(|t| *t > 0) {
                            *x = Value::String((t + delta).to_string());
                        }
                    } else if event_time && k == "value" {
                        *x = json!({ "stringValue": "" });
                    } else {
                        shift(x, delta);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| shift(x, delta)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    times(&v, &mut all);
    let delta = now - all.iter().max().copied().unwrap_or(now);
    shift(&mut v, delta);
    serde_json::to_vec(&v).unwrap()
}

#[tokio::test]
async fn claude_codes_otlp_export_lands_in_the_log_as_observed_requests_and_tool_calls() {
    let env = env().await;
    let (id, key) = source(&env, "claude_code").await;
    let (status, headers, body) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        fresh(&env, "claude-code-logs-1.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(headers["content-type"], "application/json");
    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), json!({}));
    let (status, headers, body) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/x-protobuf",
        true,
        fresh(&env, "claude-code-logs-2.pb"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "application/x-protobuf");
    assert!(
        body.is_empty(),
        "an export kept whole answers with an empty message"
    );
    for name in ["claude-code-mcp-named.json", "claude-code-traces.json"] {
        let path = if name.contains("traces") {
            "/ingest/otlp/v1/traces"
        } else {
            "/ingest/otlp/v1/logs"
        };
        let (status, _, _) = post(
            &env,
            path,
            Some(&key),
            "application/json",
            false,
            fresh(&env, name),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{name}");
    }
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/metrics",
        Some(&key),
        "application/json",
        false,
        fresh(&env, "claude-code-metrics.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    flush(&env).await;

    let rows = logs(&env, "?source=observed").await;
    assert_eq!(
        rows.len(),
        4,
        "two from the first exports, two from the MCP turn: {rows:#?}"
    );
    assert!(rows.iter().all(|r| r["model"] == "claude-haiku-5-5"));
    let first = rows
        .iter()
        .find(|r| r["upstream_request_id"] == "req_01EXAMPLE0C4AA1B023B5AA98FFB4")
        .unwrap();
    assert_eq!(first["usage"]["cache_read_tokens"], 11_873);
    assert_eq!(first["client"], "claude-code");

    let calls = env
        .app
        .get(
            &format!("{}/logs/tool-calls", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(calls.body["data"][0]["tool"], "get_me");
    assert_eq!(calls.body["data"][0]["source"], "observed");

    let view = env
        .app
        .get(&format!("{}/sources/{id}", base(&env)), &env.owner.cookie)
        .await
        .body;
    assert_eq!(view["status"], "connected");
    assert_eq!(view["activity"][0]["requests"], 4);
    assert_eq!(view["activity"][0]["tool_calls"], 1);

    let usage = env
        .app
        .get(
            &format!("{}/usage?group_by=day", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(usage.status, StatusCode::OK, "{:?}", usage.body);
    let days = usage.body["data"].as_array().unwrap();
    let total = |k: &str| days.iter().map(|d| d[k].as_u64().unwrap_or(0)).sum::<u64>();
    assert_eq!(
        total("tool_calls"),
        1,
        "Analytics counts the reported MCP call: {days:?}"
    );
    assert!(
        total("input_tokens") + total("cache_read_tokens") > 0,
        "and the reported requests' tokens"
    );
}

#[tokio::test]
async fn codex_and_gemini_cli_exports_count_their_requests_once() {
    let env = env().await;
    let (_, key) = source(&env, "codex").await;
    for _ in 0..2 {
        let (status, _, _) = post(
            &env,
            "/ingest/otlp/v1/logs",
            Some(&key),
            "application/json",
            false,
            fresh(&env, "codex-logs.json"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, gemini) = source(&env, "gemini_cli").await;
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&gemini),
        "application/json",
        true,
        fresh(&env, "gemini-cli-logs.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    flush(&env).await;
    let rows = logs(&env, "?source=observed").await;
    let models: Vec<&str> = rows.iter().map(|r| r["model"].as_str().unwrap()).collect();
    assert_eq!(models.len(), 2, "{rows:#?}");
    assert!(models.contains(&"gpt-5.4-mini") && models.contains(&"gemini-3.6-flash"));
}

#[tokio::test]
async fn the_json_endpoints_take_kaeso_litellm_and_antigravity_batches() {
    let env = env().await;
    let now = kaeso_core::clock::Clock::now(&*env.app.state.clock);
    let at = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let (_, key) = source(&env, "http").await;
    let (status, _, body) = post(
        &env,
        "/ingest/v1/events",
        Some(&key),
        "application/json",
        false,
        json!({
            "requests": [
                { "id": "run-1", "started_at": at, "duration_ms": 800, "provider": "anthropic",
                  "model": "claude-haiku-4-5", "usage": { "input_tokens": 100, "output_tokens": 20 },
                  "prompt": "Hi", "reply": "Hello" },
                { "id": "run-2", "model": "claude-haiku-4-5", "unknown": true }
            ],
            "tool_calls": [{ "id": "t-1", "server": "linear", "tool": "list_issues", "arguments": { "team": "ENG" } }]
        })
        .to_string()
        .into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let answer: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        answer,
        json!({ "requests": 1, "tool_calls": 1, "duplicates": 0, "refused": 1, "refusal": "unreadable" })
    );

    let (_, litellm) = source(&env, "litellm").await;
    let ndjson = [
        json!({ "id": "chatcmpl-9", "model": "openai/gpt-5.4-mini", "status": "success",
            "prompt_tokens": 50, "completion_tokens": 5, "startTime": now.unix_timestamp() as f64,
            "endTime": now.unix_timestamp() as f64 + 0.5, "response_cost": 0.0001 }),
        json!({ "id": "chatcmpl-10", "model": "openai/gpt-5.4-mini", "status": "success",
            "prompt_tokens": 60, "completion_tokens": 6 }),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let (status, _, body) = post(
        &env,
        "/ingest/v1/litellm",
        Some(&litellm),
        "application/json",
        false,
        ndjson.into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["requests"],
        2
    );

    let (_, agy) = source(&env, "antigravity").await;
    let (status, _, body) = post(
        &env,
        "/ingest/v1/antigravity",
        Some(&agy),
        "application/json",
        false,
        json!({
            "hook": { "conversationId": "c-1", "modelName": "gemini-pro-agent" },
            "steps": [{ "step_index": 3, "type": "PLANNER_RESPONSE", "created_at": at,
                "model": "gemini-4-neon-high", "input_tokens": 900, "output_tokens": 30 }]
        })
        .to_string()
        .into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    flush(&env).await;
    let rows = logs(&env, "?source=observed").await;
    assert_eq!(rows.len(), 4, "{rows:#?}");
    let kept = rows
        .iter()
        .find(|r| r["model"] == "claude-haiku-4-5")
        .unwrap();
    let entry = env
        .app
        .get(
            &format!("{}/logs/{}", base(&env), kept["id"].as_str().unwrap()),
            &env.owner.cookie,
        )
        .await
        .body;
    assert_eq!(
        entry["content"]["request"]["messages"][0]["content"], "Hi",
        "{entry}"
    );
}

#[tokio::test]
async fn ingest_refuses_missing_wrong_and_disabled_keys_and_unreadable_bodies() {
    let env = env().await;
    let (id, key) = source(&env, "opentelemetry").await;
    let body = || b"{\"resourceLogs\": []}".to_vec();
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        None,
        "application/json",
        false,
        body(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&env.key),
        "application/json",
        false,
        body(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an API key is no source key"
    );
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        b"not json".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let r = env
        .app
        .get(&format!("{}/sources/{id}", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(r.body["last_error"], "unreadable");

    let models = env
        .app
        .raw_bytes(
            Request::builder()
                .uri("/v1/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        models.0,
        StatusCode::UNAUTHORIZED,
        "a source key calls no model"
    );
    for (method, path) in [(Method::GET, "/platform/v1/me"), (Method::POST, "/mcp")] {
        let (status, _, _) = env
            .app
            .raw_bytes(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", format!("Bearer {key}"))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a source key is no key for {path}"
        );
    }
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "text/plain",
        false,
        body(),
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let (status, headers, refusal) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some("sk-kso-src-000000000000000000000000000000000000000000000000"),
        "application/x-protobuf",
        false,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        headers["content-type"], "application/x-protobuf",
        "a protobuf export hears back in protobuf"
    );
    assert!(!refusal.is_empty());

    env.app
        .call(
            Method::PATCH,
            &format!("{}/sources/{id}", base(&env)),
            Some(json!({ "disabled": true })),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        body(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_, other) = source(&env, "http").await;
    let huge = vec![b' '; 9 * 1024 * 1024];
    let (status, _, _) = post(
        &env,
        "/ingest/v1/events",
        Some(&other),
        "application/json",
        false,
        huge,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, _, _) = post(
        &env,
        "/ingest/v1/events",
        Some(&other),
        "application/json",
        false,
        json!({ "requests": [], "spans": [] })
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an unknown field of the batch is refused"
    );
}

#[tokio::test]
async fn a_source_that_sends_too_many_batches_is_asked_to_wait() {
    let env = env().await;
    let (_, key) = source(&env, "http").await;
    let empty = || json!({ "requests": [] }).to_string().into_bytes();
    let mut last = StatusCode::OK;
    let mut headers = axum::http::HeaderMap::new();
    for _ in 0..601 {
        let (s, h, _) = post(
            &env,
            "/ingest/v1/events",
            Some(&key),
            "application/json",
            false,
            empty(),
        )
        .await;
        last = s;
        headers = h;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    assert!(headers.contains_key("retry-after"));
}

#[tokio::test]
async fn a_source_that_sends_too_many_records_is_asked_to_wait() {
    let env = env().await;
    let (_, key) = source(&env, "http").await;
    // Records from the future are refused without being kept, so the test writes nothing.
    let calls: Vec<Value> = (0..1_000)
        .map(|n| json!({ "id": format!("t-{n}"), "tool": "x", "started_at": "2999-01-01T00:00:00Z" }))
        .collect();
    let batch = || json!({ "tool_calls": calls }).to_string().into_bytes();
    for n in 0..6 {
        let (s, _, body) = post(
            &env,
            "/ingest/v1/events",
            Some(&key),
            "application/json",
            false,
            batch(),
        )
        .await;
        assert_eq!(
            s,
            StatusCode::OK,
            "batch {n}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    let (s, h, body) = post(
        &env,
        "/ingest/v1/events",
        Some(&key),
        "application/json",
        false,
        batch(),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert!(h.contains_key("retry-after"));
    assert!(
        String::from_utf8_lossy(&body).contains("more than 6,000 records this minute"),
        "{}",
        String::from_utf8_lossy(&body)
    );
    let one_too_many: Vec<Value> = (0..1_001)
        .map(|n| json!({ "id": format!("u-{n}"), "tool": "x" }))
        .collect();
    let (_, other) = source(&env, "http").await;
    let (s, _, _) = post(
        &env,
        "/ingest/v1/events",
        Some(&other),
        "application/json",
        false,
        json!({ "tool_calls": one_too_many })
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::PAYLOAD_TOO_LARGE,
        "a batch holds at most 1,000 records"
    );
}

#[tokio::test]
async fn the_consoles_http_example_is_taken_as_it_is() {
    let env = env().await;
    let (_, key) = source(&env, "http").await;
    let (status, _, body) = post(
        &env,
        "/ingest/v1/events",
        Some(&key),
        "application/json",
        false,
        fixture("http-example.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let answer: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        answer,
        json!({ "requests": 1, "tool_calls": 1, "duplicates": 0, "refused": 0, "refusal": null }),
        "the setup snippet's body must stay one that Kaeso takes"
    );
}

fn record_now(env: &Env, offset_ms: i64, attrs: Value) -> Value {
    let at = env.app.state.clock.now() + time::Duration::milliseconds(offset_ms);
    let attributes: Vec<Value> = attrs
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| json!({ "key": k, "value": { "stringValue": v.as_str().unwrap() } }))
        .collect();
    json!({ "timeUnixNano": at.unix_timestamp_nanos().to_string(), "attributes": attributes })
}

fn logs_body(service: &str, records: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({ "resourceLogs": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": service } }] },
        "scopeLogs": [{ "scope": { "name": service }, "logRecords": records }]
    }]}))
    .unwrap()
}

async fn view(env: &Env, id: &str) -> Value {
    env.app
        .get(&format!("{}/sources/{id}", base(env)), &env.owner.cookie)
        .await
        .body
}

#[tokio::test]
async fn a_source_says_why_it_records_nothing_and_metrics_alone_dont_connect_it() {
    let env = env().await;
    let (id, key) = source(&env, "opentelemetry").await;
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/metrics",
        Some(&key),
        "application/json",
        false,
        fresh(&env, "claude-code-metrics.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view(&env, &id).await["status"], "waiting");

    let unknown = logs_body(
        "some-cli",
        vec![
            record_now(
                &env,
                -2_000,
                json!({ "event.name": "some_cli.api_call", "model": "m" }),
            ),
            record_now(
                &env,
                -1_000,
                json!({ "event.name": "some_cli.session_start" }),
            ),
        ],
    );
    let (status, _, body) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        unknown,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let answer: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(answer["partialSuccess"]["rejectedLogRecords"], "2");
    assert_eq!(answer["partialSuccess"]["errorMessage"], "unrecognized");
    let v = view(&env, &id).await;
    assert_eq!(v["status"], "waiting", "{v:#?}");
    assert_eq!(v["last_error"], "unrecognized");
    let (status, _, _) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        fresh(&env, "claude-code-logs-1.json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view(&env, &id).await["status"], "connected");
    let again = logs_body(
        "my-app",
        vec![record_now(
            &env,
            -500,
            json!({ "event.name": "http.request", "url.path": "/" }),
        )],
    );
    let (_, _, body) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        again,
    )
    .await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({}),
        "once a source records, the other records its client exports are left alone"
    );
}

#[tokio::test]
async fn codex_times_a_request_whose_start_came_in_the_export_before() {
    let env = env().await;
    let (_, key) = source(&env, "codex").await;
    for body in [
        logs_body(
            "codex_exec",
            vec![
                record_now(
                    &env,
                    -5_000,
                    json!({ "event.name": "codex.user_prompt", "prompt": "Fix the build",
                    "conversation.id": "c1", "app.version": "0.171.0" }),
                ),
                record_now(
                    &env,
                    -4_000,
                    json!({ "event.name": "codex.api_request", "duration_ms": "300",
                    "http.response.status_code": "200", "model": "gpt-5.4-mini", "conversation.id": "c1" }),
                ),
            ],
        ),
        logs_body(
            "codex_exec",
            vec![record_now(
                &env,
                -1_700,
                json!({ "event.name": "codex.sse_event", "event.kind": "response.completed",
                "model": "gpt-5.4-mini", "conversation.id": "c1", "input_token_count": "40", "output_token_count": "8" }),
            )],
        ),
    ] {
        let (status, _, _) = post(
            &env,
            "/ingest/otlp/v1/logs",
            Some(&key),
            "application/json",
            false,
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    flush(&env).await;
    let rows = logs(&env, "?source=observed").await;
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(
        rows[0]["duration_ms"], 2_600,
        "from its start in the first export"
    );
    assert_eq!(rows[0]["ttfb_ms"], 300);
    assert!(
        rows[0]["has_content"].as_bool().unwrap(),
        "its prompt came along"
    );
}

async fn project_named(env: &Env, org: &str, name: &str) -> String {
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

async fn source_at(env: &Env, org: &str, project: &str, kind: &str) -> String {
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{project}/sources"),
            json!({ "kind": kind, "name": kind }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body["key"].as_str().unwrap().to_owned()
}

async fn events(env: &Env, key: &str, requests: Value) -> Value {
    let (status, _, body) = post(
        env,
        "/ingest/v1/events",
        Some(key),
        "application/json",
        false,
        json!({ "requests": requests }).to_string().into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

fn now_text(env: &Env) -> String {
    env.app
        .state
        .clock
        .now()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

// Kaeso's JSON counts input as Anthropic does: without what came from or went into the cache.
fn usage_of(u: &Value) -> Value {
    json!({
        "input_tokens": u["input_tokens"],
        "output_tokens": u["output_tokens"],
        "cache_read_tokens": u["cache_read_input_tokens"].as_u64().unwrap_or(0),
        "cache_write_tokens": u["cache_creation_input_tokens"].as_u64().unwrap_or(0),
    })
}

#[tokio::test]
async fn a_gateway_requests_copy_counts_once_from_any_project_of_its_organization() {
    let env = env().await;
    let org = env.owner.org_id.to_string();
    let second = project_named(&env, &org, "Laptops").await;
    let key = source_at(&env, &org, &second, "http").await;

    let at = now_text(&env);
    let r = common::v1::call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(common::v1::messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let usage = usage_of(&r.json()["usage"]);
    // The client reports the request the moment it has the reply, from another project.
    let copy = json!([{ "id": "copy-1", "started_at": at, "duration_ms": 40, "provider": "anthropic",
        "model": "claude-haiku-4-5", "usage": usage }]);
    let answer = events(&env, &key, copy).await;
    assert_eq!(
        (&answer["requests"], &answer["duplicates"]),
        (&json!(0), &json!(1)),
        "{answer}"
    );
    // The gateway request is taken: the same tokens again are another request.
    let again = json!([{ "id": "copy-2", "started_at": at, "duration_ms": 40, "provider": "anthropic",
        "model": "claude-haiku-4-5", "usage": usage }]);
    assert_eq!(events(&env, &key, again).await["requests"], 1);

    let at = now_text(&env);
    let r = common::v1::call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(common::v1::messages(
            "claude-haiku-4-5",
            "Hi [mock:fail=claude-haiku-4-5 overloaded]",
        )),
    )
    .await;
    assert!(r.status.as_u16() >= 400, "{}", r.text());
    let failed = json!([{ "id": "failed-1", "started_at": at, "duration_ms": 30, "provider": "anthropic",
        "model": "claude-haiku-4-5", "status": r.status.as_u16(), "error": "Overloaded" }]);
    let answer = events(&env, &key, failed).await;
    assert_eq!(
        answer["duplicates"], 1,
        "a failed request is its copy too: {answer}"
    );

    let at = now_text(&env);
    let mut streamed = common::v1::messages("claude-haiku-4-5", "Hi");
    streamed["stream"] = json!(true);
    let r = common::v1::call(&env, "/v1/messages", Some(&env.key), true, Some(streamed)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events_of: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let mut u = events_of
        .iter()
        .find(|e| e["type"] == "message_start")
        .unwrap()["message"]["usage"]
        .clone();
    u["output_tokens"] = events_of
        .iter()
        .rev()
        .find(|e| e["type"] == "message_delta")
        .unwrap()["usage"]["output_tokens"]
        .clone();
    let copy = json!([{ "id": "streamed-1", "started_at": at, "duration_ms": 60, "provider": "anthropic",
        "model": "claude-haiku-4-5", "usage": usage_of(&u), "stream": true }]);
    let answer = events(&env, &key, copy).await;
    assert_eq!(
        answer["duplicates"], 1,
        "a stream is noted before its end reaches the app: {answer}"
    );

    flush(&env).await;
    let observed: Vec<Value> = logs(&env, "?source=observed").await;
    assert!(observed.is_empty(), "{observed:#?}");
}

#[tokio::test]
async fn another_organizations_source_never_takes_a_gateway_request_for_its_copy() {
    let env = env().await;
    let r = env
        .app
        .post(
            "/me/orgs",
            json!({ "name": "Elsewhere" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let other = r.body["id"].as_str().unwrap().to_owned();
    let project = project_named(&env, &other, "Laptops").await;
    let key = source_at(&env, &other, &project, "http").await;
    // The gateway's own organization has a source too, so its requests are noted.
    source(&env, "http").await;

    let at = now_text(&env);
    let r = common::v1::call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(common::v1::messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let copy = json!([{ "id": "copy-1", "started_at": at, "duration_ms": 40, "provider": "anthropic",
        "model": "claude-haiku-4-5", "usage": usage_of(&r.json()["usage"]) }]);
    let answer = events(&env, &key, copy).await;
    assert_eq!(
        (&answer["requests"], &answer["duplicates"]),
        (&json!(1), &json!(0)),
        "{answer}"
    );
}

#[tokio::test]
async fn a_model_named_with_its_effort_is_one_model_and_a_request_without_timing_has_none() {
    let env = env().await;
    let (_, key) = source(&env, "http").await;
    let now = env.app.state.clock.now();
    let rfc = |t: time::OffsetDateTime| {
        t.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let at = rfc(now);
    let answer = events(
        &env,
        &key,
        json!([
            { "id": "effort-1", "started_at": at, "provider": "anthropic", "model": "claude-opus-4-6-high",
              "usage": { "input_tokens": 100, "output_tokens": 20 } },
            { "id": "effort-2", "started_at": at, "duration_ms": 900, "provider": "anthropic",
              "model": "claude-opus-4-6", "reasoning_effort": "Medium",
              "usage": { "input_tokens": 100, "output_tokens": 20 } },
            { "id": "effort-3", "started_at": at, "duration_ms": 900, "provider": "anthropic",
              "model": "made-up-model-high", "usage": { "input_tokens": 1, "output_tokens": 1 } },
            { "id": "effort-4", "started_at": at, "duration_ms": 900, "provider": "anthropic",
              "model": "claude-opus-4-6-xhigh", "usage": { "input_tokens": 1, "output_tokens": 1 } },
            { "id": "effort-5", "started_at": at, "duration_ms": 900, "provider": "anthropic",
              "model": "claude-haiku-4-5-high", "usage": { "input_tokens": 1, "output_tokens": 1 } },
            { "id": "effort-6", "started_at": at, "duration_ms": 900, "provider": "anthropic",
              "model": "claude-sonnet-4-6-thinking", "usage": { "input_tokens": 1, "output_tokens": 1 } }
        ]),
    )
    .await;
    assert_eq!(answer["requests"], 6, "{answer}");
    flush(&env).await;
    let rows = logs(&env, "?source=observed").await;
    let by_effort = |e: Option<&str>, model: &str| {
        rows.iter()
            .find(|r| r["reasoning_effort"].as_str() == e && r["model"] == model)
            .unwrap_or_else(|| panic!("{e:?} {model}: {rows:#?}"))
            .clone()
    };
    let high = by_effort(Some("high"), "claude-opus-4-6");
    assert_eq!(high["requested_model"], "claude-opus-4-6");
    assert_eq!(high["cost_known"], true);
    assert_eq!(high["duration_ms"], Value::Null, "{high}");
    assert_eq!(
        by_effort(Some("medium"), "claude-opus-4-6")["duration_ms"],
        900
    );
    let thinking = by_effort(None, "claude-sonnet-4-6");
    assert_eq!(
        (&thinking["requested_model"], &thinking["cost_known"]),
        (&json!("claude-sonnet-4-6"), &json!(true)),
        "thinking on names no level: {thinking}"
    );
    for (model, why) in [
        (
            "made-up-model-high",
            "neither the catalogue nor KPMR knows the model",
        ),
        (
            "claude-opus-4-6-xhigh",
            "KPMR lists no such level for the model",
        ),
        (
            "claude-haiku-4-5-high",
            "KPMR lists no levels for the model",
        ),
    ] {
        assert_eq!(
            by_effort(None, model)["requested_model"],
            model,
            "keeps its whole name: {why}"
        );
    }

    let timeline = env
        .app
        .get(
            &format!(
                "{}/usage/timeline?step=hour&group_by=model&from={}&to={}",
                base(&env),
                rfc(now - time::Duration::hours(1)),
                rfc(now + time::Duration::hours(1)),
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(timeline.status, StatusCode::OK, "{:?}", timeline.body);
    let buckets = timeline.body["data"].as_array().unwrap();
    let opus = buckets
        .iter()
        .find(|b| {
            b["key"]
                .as_str()
                .is_some_and(|k| k.ends_with("/claude-opus-4-6"))
        })
        .unwrap_or_else(|| panic!("{buckets:#?}"));
    assert_eq!(
        (opus["requests"].as_u64(), opus["duration_max_ms"].as_u64()),
        (Some(2), Some(900)),
        "the untimed request leaves the latency alone: {opus}"
    );
}

#[tokio::test]
async fn antigravity_models_log_as_the_model_at_their_setting_and_usage_names_them() {
    let env = env().await;
    let (_, agy) = source(&env, "antigravity").await;
    let at = now_text(&env);
    let steps: Vec<Value> = [
        "gemini-4-carbon-high",
        "gemini-3.8-flash-low",
        "claude-opus-4-6-thinking",
        "gpt-oss-120b-medium",
    ]
    .iter()
    .enumerate()
    .map(|(i, model)| {
        json!({ "step_index": i * 2, "type": "PLANNER_RESPONSE", "created_at": at,
            "model": model, "input_tokens": 900, "cache_read_tokens": 100, "output_tokens": 30 })
    })
    .collect();
    let (status, _, body) = post(
        &env,
        "/ingest/v1/antigravity",
        Some(&agy),
        "application/json",
        false,
        json!({ "hook": { "conversationId": "c-7" }, "steps": steps })
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    flush(&env).await;

    let mut logged: Vec<(String, String, Value, Value)> = logs(&env, "?source=observed")
        .await
        .iter()
        .map(|r| {
            (
                r["requested_model"].as_str().unwrap().to_owned(),
                r["model"].as_str().unwrap().to_owned(),
                r["reasoning_effort"].clone(),
                r["cost_known"].clone(),
            )
        })
        .collect();
    logged.sort_by(|a, b| a.0.cmp(&b.0));
    let row = |m: &str, e: Value, priced: bool| (m.to_owned(), m.to_owned(), e, json!(priced));
    assert_eq!(
        logged,
        [
            row("claude-opus-4-6", Value::Null, true),
            row("gemini-3.8-flash", json!("low"), true),
            row("gemini-4-carbon", json!("high"), true),
            // Only resellers list it, so no price is its own, but it is still one model.
            row("gpt-oss-120b", json!("medium"), false),
        ]
    );

    let r = env
        .app
        .get(
            &format!("{}/usage?group_by=model", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let names: Vec<(&str, &str)> = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            let key = u["key"].as_str().unwrap();
            (
                key.rsplit('/').next().unwrap(),
                u["name"].as_str().unwrap_or(key),
            )
        })
        .collect();
    for (model, name) in [
        ("claude-opus-4-6", "Claude Opus 4.6"),
        ("gemini-3.8-flash", "Gemini 3.8 Flash"),
        ("gemini-4-carbon", "Gemini 4 Carbon"),
        ("gpt-oss-120b", "GPT OSS 120B"),
    ] {
        assert!(names.contains(&(model, name)), "{model}: {names:?}");
    }
    let providers: Vec<&str> = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["key"].as_str()?.split_once('/').map(|(p, _)| p))
        .collect();
    assert!(
        providers.contains(&"anthropic") && providers.contains(&"google"),
        "Claude is Anthropic's model even from a client set up for Google: {providers:?}"
    );
}

#[tokio::test]
async fn a_client_set_up_for_one_provider_reports_another_makers_model_as_the_makers() {
    let env = env().await;
    let (_, key) = source(&env, "gemini_cli").await;
    let at = env.app.state.clock.now();
    let int = |n: u64| json!({ "intValue": n.to_string() });
    let text = |s: &str| json!({ "stringValue": s });
    let record = |model: &str, prompt: &str| {
        json!({
            "timeUnixNano": at.unix_timestamp_nanos().to_string(),
            "attributes": [
                { "key": "event.name", "value": text("gemini_cli.api_response") },
                { "key": "model", "value": text(model) },
                { "key": "auth_type", "value": text("gemini-api-key") },
                { "key": "prompt_id", "value": text(prompt) },
                { "key": "session.id", "value": text("s-1") },
                { "key": "duration_ms", "value": int(700) },
                { "key": "input_token_count", "value": int(120) },
                { "key": "output_token_count", "value": int(12) },
            ]
        })
    };
    let (status, _, body) = post(
        &env,
        "/ingest/otlp/v1/logs",
        Some(&key),
        "application/json",
        false,
        logs_body(
            "gemini-cli",
            vec![
                record("claude-haiku-4-5", "p-1"),
                record("gemini-2.5-flash", "p-2"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    flush(&env).await;
    let rows = logs(&env, "?source=observed").await;
    let provider = |model: &str| {
        rows.iter()
            .find(|r| r["model"] == model)
            .unwrap_or_else(|| panic!("{model}: {rows:#?}"))["provider"]
            .clone()
    };
    assert_eq!(provider("claude-haiku-4-5"), "anthropic");
    assert_eq!(provider("gemini-2.5-flash"), "google");
}
