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
