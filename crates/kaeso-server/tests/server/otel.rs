#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// Model requests as OpenTelemetry traces, sent to a collector over OTLP/HTTP.

use crate::common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::{Answer, Env, messages};
use http_body_util::BodyExt;
use kaeso_testkit::otel::MockCollector;
use serde_json::{Value, json};
use tower::ServiceExt;

const TRACE: &str = "0af7651916cd43dd8448eb211c80319c";
const PARENT: &str = "b7ad6b7169203331";
const HAIKU: &str = "claude-haiku-4-5";

async fn env_at(endpoint: &str, extra: &[(&str, &str)]) -> Env {
    let mut settings = vec![("KAESO_OTEL_ENDPOINT", endpoint)];
    settings.extend_from_slice(extra);
    let e = common::v1::env(None, Some(&settings)).await;
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

async fn env(collector: &MockCollector, extra: &[(&str, &str)]) -> Env {
    env_at(&collector.base_url, extra).await
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

// Sends what the gateway buffered, as it does every `otel.flush_interval`.
async fn export(env: &Env) -> usize {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::otel::flush_all(&env.app.state, Duration::from_secs(10)).await
}

async fn send(env: &Env, path: &str, body: Value, headers: &[(&str, &str)]) -> Answer {
    let mut req = Request::builder()
        .uri(path)
        .method("POST")
        .header("content-type", "application/json")
        .header("x-api-key", &env.key)
        .header("authorization", format!("Bearer {}", env.key))
        .header("anthropic-version", "2023-06-01");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = req
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

async fn ask(env: &Env, text: &str, headers: &[(&str, &str)]) -> Answer {
    send(env, "/v1/messages", messages(HAIKU, text), headers).await
}

fn traceparent(flags: &str) -> String {
    format!("00-{TRACE}-{PARENT}-{flags}")
}

fn value<'a>(span: &'a Value, key: &str) -> Option<&'a Value> {
    span["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["key"] == key)
        .map(|a| &a["value"])
}

fn text<'a>(span: &'a Value, key: &str) -> Option<&'a str> {
    value(span, key).and_then(|v| v["stringValue"].as_str())
}

fn int(span: &Value, key: &str) -> Option<i64> {
    value(span, key).and_then(|v| v["intValue"].as_str()?.parse().ok())
}

fn flag(span: &Value, key: &str) -> Option<bool> {
    value(span, key).and_then(|v| v["boolValue"].as_bool())
}

fn named<'a>(spans: &'a [Value], name: &str) -> Vec<&'a Value> {
    spans.iter().filter(|s| s["name"] == name).collect()
}

fn nanos(span: &Value, field: &str) -> u128 {
    span[field].as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn a_request_continues_the_apps_trace_with_a_span_for_its_model() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[]).await;
    let parent = traceparent("01");
    let r = ask(
        &env,
        "Hi",
        &[("traceparent", &parent), ("tracestate", "app=1")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        env.mock
            .received()
            .iter()
            .all(|m| m.header("traceparent").is_none() && m.header("tracestate").is_none()),
        "the provider never gets the app's trace ids"
    );
    assert_eq!(export(&env).await, 2);
    let spans = collector.spans();
    let [server] = named(&spans, "kaeso.request")[..] else {
        panic!("{spans:#?}");
    };
    let [step] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    assert_eq!(
        (server["traceId"].as_str(), server["parentSpanId"].as_str()),
        (Some(TRACE), Some(PARENT))
    );
    assert_eq!(server["traceState"], "app=1");
    assert_eq!(
        (server["kind"].as_u64(), step["kind"].as_u64()),
        (Some(2), Some(3))
    );
    assert_eq!(step["traceId"], TRACE);
    assert_eq!(step["parentSpanId"], server["spanId"]);
    assert!(nanos(server, "startTimeUnixNano") <= nanos(step, "startTimeUnixNano"));
    assert!(nanos(step, "endTimeUnixNano") <= nanos(server, "endTimeUnixNano"));
    assert!(server.get("status").is_none(), "{server:#}");

    for (key, want) in [
        ("gen_ai.operation.name", "chat"),
        ("gen_ai.request.model", HAIKU),
        ("gen_ai.provider.name", "anthropic"),
        ("kaeso.endpoint", "messages"),
        ("kaeso.source", "api"),
    ] {
        assert_eq!(text(server, key), Some(want), "{key}");
    }
    assert_eq!(
        text(server, "kaeso.request_id"),
        r.header("kaeso-request-id")
    );
    assert_eq!(
        text(server, "kaeso.organization.id"),
        Some(env.owner.org_id.to_string().as_str())
    );
    assert_eq!(text(server, "kaeso.project.id"), Some(env.project.as_str()));
    assert!(text(server, "kaeso.key.id").is_some());
    assert_eq!(int(server, "http.response.status_code"), Some(200));
    assert!(int(server, "kaeso.cost_microusd").unwrap() > 0);
    assert!(int(server, "kaeso.overhead_ms").is_some());
    assert_eq!(flag(server, "gen_ai.request.stream"), Some(false));
    assert_eq!(
        int(server, "gen_ai.usage.input_tokens"),
        None,
        "the step carries the tokens"
    );

    assert_eq!(text(step, "gen_ai.request.model"), Some(HAIKU));
    assert_eq!(text(step, "gen_ai.provider.name"), Some("anthropic"));
    assert!(text(step, "gen_ai.response.model").is_some());
    assert!(int(step, "gen_ai.usage.input_tokens").unwrap() > 0);
    assert!(int(step, "gen_ai.usage.output_tokens").unwrap() > 0);
    for span in [server, step] {
        assert!(
            value(span, "gen_ai.input.messages").is_none()
                && value(span, "gen_ai.output.messages").is_none(),
            "no content unless asked: {span:#}"
        );
        let all = span.to_string();
        assert!(!all.contains(&env.key), "never the key's secret");
    }

    let [export] = &collector.exports()[..] else {
        panic!("one export");
    };
    assert_eq!(export.headers["content-type"], "application/json");
    let resource = &export.body["resourceSpans"][0]["resource"];
    assert_eq!(text(resource, "service.name"), Some("kaeso-gateway"));
    assert_eq!(
        text(resource, "service.version"),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[tokio::test]
async fn a_request_without_a_traceparent_starts_a_trace_and_a_failed_one_is_an_error() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[("KAESO_OTEL_SERVICE_NAME", "gateway-eu")]).await;
    let r = ask(&env, "Hi [mock:error=overloaded]", &[]).await;
    assert_eq!(r.status.as_u16(), 529, "{}", r.text());
    export(&env).await;
    let spans = collector.spans();
    let [server] = named(&spans, "kaeso.request")[..] else {
        panic!("{spans:#?}");
    };
    let [step] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    assert!(server.get("parentSpanId").is_none());
    assert_eq!(server["traceId"].as_str().unwrap().len(), 32);
    assert_eq!(step["traceId"], server["traceId"]);
    assert_eq!(int(server, "http.response.status_code"), Some(529));
    assert_eq!(text(server, "error.type"), Some("overloaded"));
    assert_eq!(server["status"]["code"], 2);
    assert_eq!(step["status"]["code"], 2);
    assert_eq!(text(step, "error.type"), Some("overloaded"));
    let resource = &collector.exports()[0].body["resourceSpans"][0]["resource"];
    assert_eq!(text(resource, "service.name"), Some("gateway-eu"));
}

#[tokio::test]
async fn a_streamed_request_and_a_cancelled_one_are_traced() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[]).await;
    let mut body = messages(HAIKU, "Hi");
    body["stream"] = json!(true);
    let r = send(&env, "/v1/messages", body, &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    export(&env).await;
    let spans = collector.spans();
    let [server] = named(&spans, "kaeso.request")[..] else {
        panic!("{spans:#?}");
    };
    assert_eq!(flag(server, "gen_ai.request.stream"), Some(true));
    assert!(int(server, "kaeso.ttfb_ms").is_some());
    let [step] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    assert!(int(step, "gen_ai.usage.output_tokens").unwrap() > 0);

    let mut body = messages(HAIKU, "Tell me a story [mock:slow]");
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
    // The gateway learns that the app left when it next writes to it.
    let mut spans = Vec::new();
    for _ in 0..300 {
        export(&env).await;
        spans = collector.spans();
        if spans
            .iter()
            .any(|s| int(s, "http.response.status_code") == Some(499))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let cancelled = named(&spans, "kaeso.request")
        .into_iter()
        .find(|s| int(s, "http.response.status_code") == Some(499))
        .unwrap_or_else(|| panic!("{spans:#?}"));
    assert_eq!(flag(cancelled, "kaeso.cancelled"), Some(true));
    assert_eq!(text(cancelled, "error.type"), Some("client_disconnected"));
    assert!(
        cancelled.get("status").is_none(),
        "the app left; Kaeso didn't fail"
    );
}

async fn model_id(env: &Env, model: &str) -> String {
    let models = env
        .app
        .get(&format!("{}/models", base(env)), &env.owner.cookie)
        .await;
    models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == model)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn an_alias_that_fell_back_has_a_span_for_each_step_it_tried() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[]).await;
    let claude = model_id(&env, HAIKU).await;
    let gpt = model_id(&env, "gpt-4o").await;
    let r = env
        .app
        .post(
            &format!("{}/aliases", base(&env)),
            json!({ "name": "sturdy", "route": { "type": "model", "model": claude, "fallback": { "type": "model", "model": gpt } } }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = send(
        &env,
        "/v1/messages",
        messages("sturdy", &format!("Hi [mock:fail={HAIKU} overloaded]")),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    export(&env).await;
    let spans = collector.spans();
    let [server] = named(&spans, "kaeso.request")[..] else {
        panic!("{spans:#?}");
    };
    assert_eq!(text(server, "kaeso.alias"), Some("sturdy"));
    assert_eq!(text(server, "gen_ai.request.model"), Some("sturdy"));
    assert_eq!(text(server, "gen_ai.provider.name"), Some("openai"));
    let [failed] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    let [answered] = named(&spans, "chat gpt-4o")[..] else {
        panic!("{spans:#?}");
    };
    assert_eq!(text(failed, "gen_ai.provider.name"), Some("anthropic"));
    assert_eq!(text(failed, "kaeso.route.path"), Some("route"));
    assert_eq!(int(failed, "http.response.status_code"), Some(529));
    assert_eq!(text(failed, "error.type"), Some("overloaded"));
    assert_eq!(failed["status"]["code"], 2);
    assert_eq!(int(failed, "gen_ai.usage.input_tokens"), None);
    assert!(answered.get("status").is_none());
    assert_eq!(text(answered, "gen_ai.provider.name"), Some("openai"));
    assert!(int(answered, "gen_ai.usage.output_tokens").unwrap() > 0);
    for step in [failed, answered] {
        assert_eq!(step["parentSpanId"], server["spanId"]);
    }
    assert!(
        nanos(failed, "endTimeUnixNano") <= nanos(answered, "startTimeUnixNano"),
        "in the order they were tried"
    );
}

#[tokio::test]
async fn a_try_made_again_is_a_span_of_its_own() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[]).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &base(&env),
            Some(json!({ "retries": 1 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = ask(&env, "Hi [mock:flaky=once 1 overloaded]", &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    export(&env).await;
    let spans = collector.spans();
    let [first, second] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    assert_eq!(flag(first, "kaeso.retried"), Some(true));
    assert_eq!(int(first, "http.response.status_code"), Some(529));
    assert_eq!(flag(second, "kaeso.retried"), None);
    assert!(int(second, "gen_ai.usage.output_tokens").unwrap() > 0);
}

#[tokio::test]
async fn a_cached_reply_has_no_span_for_a_provider() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[]).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/response-cache", base(&env)),
            Some(json!({ "enabled": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    for _ in 0..2 {
        assert_eq!(ask(&env, "Same question", &[]).await.status, StatusCode::OK);
        env.app.state.in_flight.settled().await;
    }
    export(&env).await;
    let spans = collector.spans();
    let servers = named(&spans, "kaeso.request");
    let hit = servers
        .iter()
        .find(|s| text(s, "kaeso.response_cache") == Some("hit"))
        .unwrap_or_else(|| panic!("{spans:#?}"));
    assert!(
        spans
            .iter()
            .filter(|s| s["traceId"] == hit["traceId"])
            .all(|s| s["name"] == "kaeso.request"),
        "nothing called a provider"
    );
    assert!(int(hit, "gen_ai.usage.output_tokens").unwrap() > 0);
    assert_eq!(int(hit, "kaeso.cost_microusd"), Some(0));
}

#[tokio::test]
async fn content_is_exported_only_when_asked_and_when_the_log_keeps_it() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[("KAESO_OTEL_CONTENT", "on")]).await;
    let mut body = messages(HAIKU, "What is the capital of France?");
    body["system"] = json!("Answer in one word.");
    assert_eq!(
        send(&env, "/v1/messages", body, &[]).await.status,
        StatusCode::OK
    );
    export(&env).await;
    let spans = collector.spans();
    let [step] = named(&spans, &format!("chat {HAIKU}"))[..] else {
        panic!("{spans:#?}");
    };
    let read = |key: &str| serde_json::from_str::<Value>(text(step, key).unwrap()).unwrap();
    assert_eq!(
        read("gen_ai.system_instructions"),
        json!([{ "type": "text", "content": "Answer in one word." }])
    );
    assert_eq!(
        read("gen_ai.input.messages"),
        json!([{ "role": "user", "parts": [{ "type": "text", "content": "What is the capital of France?" }] }])
    );
    let output = read("gen_ai.output.messages");
    assert_eq!(output[0]["role"], "assistant");
    assert_eq!(output[0]["finish_reason"], "stop");
    assert!(
        !output[0]["parts"][0]["content"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let [server] = named(&spans, "kaeso.request")[..] else {
        panic!("{spans:#?}");
    };
    assert!(
        value(server, "gen_ai.input.messages").is_none(),
        "only once, on the step"
    );

    let r = ask(&env, "A secret question", &[("kaeso-log-content", "off")]).await;
    assert_eq!(r.status, StatusCode::OK);
    export(&env).await;
    let spans = collector.spans();
    let unkept = spans
        .iter()
        .filter(|s| s["name"] == format!("chat {HAIKU}"))
        .nth(1)
        .unwrap_or_else(|| panic!("{spans:#?}"));
    assert!(value(unkept, "gen_ai.input.messages").is_none());
    assert!(
        !spans
            .iter()
            .any(|s| s.to_string().contains("A secret question"))
    );
}

#[tokio::test]
async fn sampling_follows_the_apps_traceparent_and_otherwise_the_ratio() {
    let collector = MockCollector::start().await;
    let env = env(&collector, &[("KAESO_OTEL_SAMPLE_RATIO", "0")]).await;
    ask(&env, "Hi", &[]).await;
    ask(&env, "Hi", &[("traceparent", &traceparent("00"))]).await;
    assert_eq!(export(&env).await, 0, "neither is sampled");
    ask(&env, "Hi", &[("traceparent", &traceparent("01"))]).await;
    assert_eq!(export(&env).await, 2, "the app sampled it");
    assert!(collector.spans().iter().all(|s| s["traceId"] == TRACE));
}

#[tokio::test]
async fn headers_go_to_the_collector() {
    let collector = MockCollector::start().await;
    let env = env(
        &collector,
        &[(
            "KAESO_OTEL_HEADERS",
            "authorization=Bearer%20t0ken,x-tenant=kaeso",
        )],
    )
    .await;
    ask(&env, "Hi", &[]).await;
    export(&env).await;
    let exports = collector.exports();
    assert_eq!(exports[0].headers["authorization"], "Bearer t0ken");
    assert_eq!(exports[0].headers["x-tenant"], "kaeso");
}

#[tokio::test]
async fn a_collector_that_is_down_or_refuses_never_holds_up_a_request() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    let _recording = metrics::set_default_local_recorder(&recorder);
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let env = env_at(&format!("http://127.0.0.1:{closed}"), &[]).await;
    let r = ask(&env, "Hi", &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(export(&env).await, 0);
    assert_eq!(env.app.state.otel.as_ref().unwrap().waiting(), 0);
    let text = handle.render();
    assert!(
        text.contains("kaeso_otel_spans_total{outcome=\"failed\"} 2")
            && text.contains("kaeso_otel_spans_total{outcome=\"dropped\"} 2"),
        "{text}"
    );

    let collector = MockCollector::start().await;
    collector.answer(400);
    let env = self::env(&collector, &[]).await;
    assert_eq!(ask(&env, "Hi", &[]).await.status, StatusCode::OK);
    export(&env).await;
    assert!(collector.spans().is_empty());
    let text = handle.render();
    assert!(
        text.contains("kaeso_otel_spans_total{outcome=\"dropped\"} 4"),
        "a refusal loses the spans: {text}"
    );
    collector.answer(200);
    assert_eq!(ask(&env, "Hi", &[]).await.status, StatusCode::OK);
    export(&env).await;
    assert_eq!(collector.spans().len(), 2);
    assert!(
        handle
            .render()
            .contains("kaeso_otel_spans_total{outcome=\"exported\"} 2")
    );
}

#[tokio::test]
async fn without_an_endpoint_nothing_is_exported() {
    let env = common::v1::env(None, None).await;
    assert!(env.app.state.otel.is_none());
    assert_eq!(ask(&env, "Hi", &[]).await.status, StatusCode::OK);
    assert_eq!(export(&env).await, 0);
}

#[tokio::test]
async fn a_browser_may_send_trace_context() {
    let collector = MockCollector::start().await;
    let gateway = env(
        &collector,
        &[("KAESO_CORS_ALLOWED_ORIGINS", "https://app.example.com")],
    )
    .await;
    let req = Request::builder()
        .uri("/v1/messages")
        .method("OPTIONS")
        .header("origin", "https://app.example.com")
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "content-type, traceparent, tracestate",
        )
        .body(Body::empty())
        .unwrap();
    let resp = gateway.app.router.clone().oneshot(req).await.unwrap();
    let allowed = resp.headers()["access-control-allow-headers"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        allowed.contains("traceparent") && allowed.contains("tracestate"),
        "{allowed}"
    );
}
