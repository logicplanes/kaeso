#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::v1::{Answer, Capture, Env, call, chat, messages};
use kaeso_codecs::{EncodeCtx, dispatch};
use kaeso_kpp::{DialectId, RequestKind, StreamMode};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

fn keepalive_of(dialect: DialectId) -> String {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
    let mut encoder = dispatch::stream_encoder(dialect, &ctx).unwrap();
    String::from_utf8(encoder.keepalive()).unwrap()
}

async fn env(extra: &[(&str, &str)]) -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), Some(extra)).await
}

fn streaming(mut body: Value) -> Value {
    body["stream"] = json!(true);
    body
}

fn cases(text: &str) -> [(&'static str, bool, Value, String); 3] {
    [
        (
            "/v1/chat/completions",
            false,
            streaming(chat("claude-haiku-4-5", text)),
            keepalive_of(DialectId::OpenAiChat),
        ),
        (
            "/v1/responses",
            false,
            json!({ "model": "claude-haiku-4-5", "input": text, "stream": true }),
            keepalive_of(DialectId::OpenAiResponses),
        ),
        (
            "/v1/messages",
            true,
            streaming(messages("gpt-4o", text)),
            keepalive_of(DialectId::AnthropicMessages),
        ),
    ]
}

async fn send(env: &Env, path: &str, anthropic: bool, body: Value) -> Answer {
    call(env, path, Some(&env.key), anthropic, Some(body)).await
}

#[tokio::test]
async fn keepalives_flow_while_the_provider_is_quiet_and_leave_the_stream_intact() {
    let env = env(&[("KAESO_STREAMS_KEEPALIVE_INTERVAL", "1s")]).await;
    let quiet = cases("Hi [mock:silence=2300]");
    let loud = cases("Hi");
    let runs = quiet.iter().zip(loud.iter()).map(|(q, l)| {
        let env = &env;
        async move {
            let (a, b) = tokio::join!(
                send(env, q.0, q.1, q.2.clone()),
                send(env, l.0, l.1, l.2.clone())
            );
            (q.0, q.3.clone(), a, b)
        }
    });
    for (path, keepalive, quiet, loud) in futures::future::join_all(runs).await {
        assert_eq!(quiet.status, 200, "{path}: {}", quiet.text());
        assert!(!keepalive.is_empty(), "{path}");
        let split = |text: &str| -> (usize, String) {
            let frames: Vec<&str> = text.split_inclusive("\n\n").collect();
            let n = frames.iter().filter(|f| **f == keepalive).count();
            (n, frames.into_iter().filter(|f| *f != keepalive).collect())
        };
        let (quiet_n, quiet_rest) = split(&quiet.text());
        let (loud_n, loud_rest) = split(&loud.text());
        assert!(quiet_n > loud_n, "{path}: {}", quiet.text());
        assert_eq!(quiet_rest, loud_rest, "{path}");
    }
}

#[tokio::test]
async fn events_the_apps_format_leaves_out_do_not_count_as_writes() {
    let env = env(&[("KAESO_STREAMS_KEEPALIVE_INTERVAL", "1s")]).await;
    let body = streaming(chat(
        "claude-haiku-4-5",
        "Think [mock:fixture=thinking-enabled-budget] [mock:slow=200]",
    ));
    let r = send(&env, "/v1/chat/completions", false, body).await;
    assert_eq!(r.status, 200, "{}", r.text());
    let text = r.text();
    let keepalive = keepalive_of(DialectId::OpenAiChat);
    let frames: Vec<&str> = text.split_inclusive("\n\n").collect();
    let answer = frames
        .iter()
        .position(|f| f.contains("\"content\":\"") && !f.contains("\"content\":\"\""))
        .unwrap();
    let before = frames[..answer].iter().filter(|f| **f == keepalive).count();
    assert!(before >= 2, "{text}");
    assert!(text.trim_end().ends_with("data: [DONE]"), "{text}");
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert!(o.stream_progress.unwrap().keepalives >= 1);
}

#[tokio::test]
async fn keepalives_can_be_turned_off() {
    let env = env(&[("KAESO_STREAMS_KEEPALIVE_INTERVAL", "0s")]).await;
    let [(path, anthropic, body, _), ..] = cases("Hi [mock:silence=1500]");
    let r = send(&env, path, anthropic, body).await;
    assert_eq!(r.status, 200, "{}", r.text());
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert_eq!(o.stream_progress.unwrap().keepalives, 0, "{}", r.text());
    assert!(
        r.text().trim_end().ends_with("data: [DONE]"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn headers_and_keepalives_reach_a_waiting_chat_app_at_once() {
    let env = env(&[("KAESO_STREAMS_KEEPALIVE_INTERVAL", "1s")]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(kaeso_server::serve::serve_until(
        listener,
        env.app.router.clone(),
        shutdown.clone(),
        Duration::from_secs(1),
        kaeso_server::serve::Connections {
            header_read_timeout: Duration::from_secs(30),
            max: 4096,
        },
        CancellationToken::new(),
    ));
    let started = Instant::now();
    let mut resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .bearer_auth(&env.key)
        .json(&streaming(chat(
            "claude-haiku-4-5",
            "Hi [mock:silence=3000]",
        )))
        .send()
        .await
        .unwrap();
    let headers_after = started.elapsed();
    assert_eq!(resp.status(), 200);
    assert!(
        headers_after < Duration::from_millis(1500),
        "headers took {headers_after:?}"
    );
    let keepalive = keepalive_of(DialectId::OpenAiChat);
    let mut seen = String::new();
    let mut keepalive_at = Vec::new();
    while let Some(chunk) = resp.chunk().await.unwrap() {
        let text = String::from_utf8_lossy(&chunk).into_owned();
        if text.contains(&keepalive) && !seen.contains("Hello") {
            keepalive_at.push(started.elapsed());
        }
        seen.push_str(&text);
    }
    assert!(
        seen.starts_with("data: {") && seen.contains("\"role\":\"assistant\""),
        "{seen}"
    );
    assert!(keepalive_at.len() >= 2, "{keepalive_at:?}: {seen}");
    assert!(
        keepalive_at[0] < Duration::from_millis(2000),
        "the first keepalive took {:?}",
        keepalive_at[0]
    );
    assert!(seen.trim_end().ends_with("data: [DONE]"), "{seen}");
    shutdown.cancel();
}

async fn outcome(
    env: &Env,
    matches: impl Fn(&kaeso_server::model_api::hooks::Outcome) -> bool,
) -> kaeso_server::model_api::hooks::Outcome {
    for _ in 0..300 {
        if let Some(o) = env.capture.all().into_iter().rev().find(|o| matches(o)) {
            return o;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no matching outcome in {:#?}", env.capture.all());
}

#[tokio::test]
async fn a_stream_error_stops_the_provider_request() {
    let env = env(&[]).await;
    let gate = env.mock.gate();
    let marker = format!(
        "Hi [mock:fixture=error-overloaded-mid-stream] {}",
        gate.steps_marker()
    );
    for _ in 0..7 {
        gate.step();
    }
    let r = send(
        &env,
        "/v1/messages",
        true,
        streaming(messages("claude-haiku-4-5", &marker)),
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.text().contains("overloaded_error"), "{}", r.text());
    env.mock.wait_for_dropped_streams(1).await;
    assert_eq!(env.mock.stream_counts(), (1, 0, 1));
}

#[tokio::test]
async fn streams_that_fail_after_their_headers_are_recorded_as_failures() {
    let env = env(&[("KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT", "1s")]).await;
    let cases = [
        (
            "[mock:fixture=error-overloaded-mid-stream]",
            "overloaded",
            None,
        ),
        ("[mock:cut]", "stream_interrupted", Some(502)),
        ("[mock:stall]", "timeout", Some(504)),
    ];
    for (marker, kind, status) in cases {
        let body = streaming(chat("claude-haiku-4-5", &format!("Hi {marker}")));
        let r = send(&env, "/v1/chat/completions", false, body).await;
        assert_eq!(r.status, 200, "{marker}");
        assert!(r.text().contains("\"error\""), "{marker}: {}", r.text());
        assert!(!r.text().contains("[DONE]"), "{marker}: {}", r.text());
        let id = r.header("kaeso-request-id").unwrap().to_owned();
        let o = outcome(&env, |o| o.request_id.to_string() == id).await;
        assert_eq!(o.error_kind.as_deref(), Some(kind), "{marker}");
        assert!(o.status >= 500, "{marker}: {}", o.status);
        if let Some(s) = status {
            assert_eq!(o.status, s, "{marker}");
        }
        assert!(!o.cancelled);
        let p = o.stream_progress.unwrap();
        assert!(p.events > 0 && p.bytes > 0, "{marker}: {p:?}");
        assert!(p.first_byte.is_some(), "{marker}");
        assert_eq!(p.bytes, r.body.len() as u64, "{marker}");
    }
    let r = send(
        &env,
        "/v1/chat/completions",
        false,
        streaming(chat("claude-haiku-4-5", "Hi")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert_eq!(o.status, 200);
    assert!(o.error_kind.is_none());
    assert_eq!(o.stream_progress.unwrap().bytes, r.body.len() as u64);
}

async fn disconnect_after_first_text(env: &Env, model: &str) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let body = streaming(messages(model, "Tell me a story [mock:slow]"));
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let id = resp
        .headers()
        .get("kaeso-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let mut body = resp.into_body();
    let mut seen = String::new();
    while !seen.contains("text_delta") {
        let frame = body.frame().await.unwrap().unwrap();
        if let Ok(data) = frame.into_data() {
            seen.push_str(&String::from_utf8_lossy(&data));
        }
    }
    drop(body);
    id
}

#[tokio::test]
async fn disconnects_are_charged_an_estimate() {
    let env = env(&[]).await;
    let id = disconnect_after_first_text(&env, "claude-haiku-4-5").await;
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert_eq!((o.status, o.cancelled), (499, true));
    assert!(o.usage_estimated);
    let u = o.usage.clone().unwrap();
    assert_eq!(u.input_tokens, 10);
    assert!((3..18).contains(&u.output_tokens), "{u:?}");
    assert!(
        env.mock
            .received()
            .iter()
            .all(|r| r.path != "/v1/messages/count_tokens"),
        "no count needed"
    );
    for _ in 0..200 {
        if env.mock.stream_counts().2 >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        env.mock.stream_counts().2 >= 1,
        "{:?}",
        env.mock.stream_counts()
    );

    let id = disconnect_after_first_text(&env, "gpt-4o").await;
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert_eq!(o.status, 499);
    assert!(o.usage_estimated);
    let u = o.usage.clone().unwrap();
    assert_eq!(u.input_tokens, 11, "{u:?}");
    assert!(u.output_tokens >= 1, "{u:?}");
    let count = env
        .mock
        .received()
        .into_iter()
        .find(|r| r.path == "/v1/responses/input_tokens")
        .expect("the input was counted");
    let generate = env
        .mock
        .received()
        .into_iter()
        .find(|r| r.path == "/v1/responses")
        .unwrap();
    assert_eq!(count.json()["input"], generate.json()["input"]);
    assert_eq!(count.json()["model"], "gpt-4o");
    assert!(count.json().get("stream").is_none(), "{}", count.json());

    let r = send(
        &env,
        "/v1/chat/completions",
        false,
        streaming(chat("claude-haiku-4-5", "Hi [mock:cut]")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert_eq!(o.error_kind.as_deref(), Some("stream_interrupted"));
    assert!(o.usage_estimated);
    assert_eq!(o.usage.unwrap().input_tokens, 10);

    let r = send(
        &env,
        "/v1/chat/completions",
        false,
        streaming(chat("claude-haiku-4-5", "Hi")),
    )
    .await;
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert!(!o.usage_estimated);
    let u = o.usage.unwrap();
    assert_eq!((u.input_tokens, u.output_tokens), (10, 18));
}

#[tokio::test]
async fn requests_open_at_shutdown_answer_and_are_recorded() {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let env = env(&[]).await;
    let mut gate = env.mock.gate();
    let held = send(
        &env,
        "/v1/messages",
        true,
        messages("claude-haiku-4-5", &format!("Hi {}", gate.marker())),
    );
    let stream = async {
        let body = streaming(chat(
            "claude-haiku-4-5",
            "Hi [mock:fixture=thinking-long-tool-loop] [mock:slow=200]",
        ));
        let req = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .header("authorization", format!("Bearer {}", env.key))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = env.app.router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let id = resp.headers()["kaeso-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let mut body = resp.into_body();
        let mut text = Vec::new();
        let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
        text.extend_from_slice(&first);
        gate.wait_for(1).await;
        let open = env.app.state.in_flight.end(Duration::from_secs(5)).await;
        while let Some(frame) = body.frame().await {
            if let Ok(data) = frame.unwrap().into_data() {
                text.extend_from_slice(&data);
            }
        }
        (id, String::from_utf8(text).unwrap(), open)
    };
    let (h, (stream_id, text, open)) = tokio::join!(held, stream);
    assert_eq!(open, 0);
    assert!(text.contains("restarting"), "{text}");
    assert!(!text.contains("[DONE]"), "{text}");
    assert_eq!(h.status, 503, "{}", h.text());
    assert!(h.text().contains("restarting"), "{}", h.text());
    let held_id = h.header("kaeso-request-id").unwrap().to_owned();
    for (id, streamed) in [(stream_id, true), (held_id, false)] {
        let o = outcome(&env, |o| o.request_id.to_string() == id).await;
        assert_eq!(o.error_kind.as_deref(), Some("shutdown"));
        assert_eq!(o.status, 503);
        if streamed {
            assert!(o.usage_estimated);
            assert!(o.stream_progress.unwrap().bytes > 0);
        }
    }
}

async fn set_content_logging(env: &Env, on: bool) {
    let r = env
        .app
        .call(
            axum::http::Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/logs/settings",
                env.owner.org_id, env.project
            ),
            Some(json!({ "content_logging": on })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.body["effective_content_logging"], on, "{:?}", r.body);
}

#[tokio::test]
async fn without_content_logging_a_stream_keeps_only_what_usage_needs() {
    let env = env(&[]).await;
    for on in [false, true] {
        set_content_logging(&env, on).await;
        let before = env.capture.all().len();
        let body = streaming(messages("claude-haiku-4-5", "kept or not"));
        let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
        assert_eq!(r.status, 200, "{}", r.text());
        assert!(r.text().contains("message_stop"));
        common::eventually("the stream is recorded", || {
            let recorded = env.capture.all().len();
            async move { recorded > before }
        })
        .await;
        let o = env.capture.all().pop().unwrap();
        assert!(o.stream);
        let reply = o.reply.as_ref().unwrap();
        assert!(o.usage.as_ref().unwrap().output_tokens > 0);
        assert!(!reply.model.is_empty());
        assert_eq!(o.request_body.is_some(), on);
        assert_eq!(!reply.blocks.is_empty(), on);
    }
}

async fn open_stream(env: &Env, text: &str) -> (String, axum::body::Body) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&streaming(messages("claude-haiku-4-5", text))).unwrap(),
        ))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let id = resp.headers()["kaeso-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    (id, resp.into_body())
}

#[tokio::test]
async fn streams_end_at_the_installs_limits() {
    for (setting, value, text) in [
        (
            "KAESO_STREAMS_MAX_BYTES",
            "300",
            "Hi [mock:fixture=thinking-long-tool-loop]",
        ),
        ("KAESO_STREAMS_MAX_DURATION", "1s", "Hi [mock:stall]"),
    ] {
        let env = env(&[(setting, value)]).await;
        let r = send(
            &env,
            "/v1/messages",
            true,
            streaming(messages("claude-haiku-4-5", text)),
        )
        .await;
        assert_eq!(r.status, 200);
        let text = r.text();
        assert!(
            text.contains("The reply reached this install's limit for a stream; Kaeso ended it."),
            "{setting}: {text}"
        );
        assert!(!text.contains("message_stop"), "{setting}: {text}");
        let id = r.header("kaeso-request-id").unwrap().to_owned();
        let o = outcome(&env, |o| o.request_id.to_string() == id).await;
        assert_eq!(o.error_kind.as_deref(), Some("stream_limit"), "{setting}");
        assert_eq!(o.status, 502);
        assert!(o.usage_estimated, "{setting}");
    }
}

#[tokio::test]
async fn an_app_that_stops_reading_ends_its_stream() {
    use http_body_util::BodyExt;
    let env = env(&[("KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT", "1s")]).await;
    let (id, mut body) = open_stream(
        &env,
        "Hi [mock:fixture=thinking-long-tool-loop] [mock:slow=10]",
    )
    .await;
    assert!(body.frame().await.unwrap().unwrap().is_data());
    let o = outcome(&env, |o| o.request_id.to_string() == id).await;
    assert!(o.cancelled);
    assert_eq!(o.status, 499);
    assert_eq!(o.error_kind.as_deref(), Some("client_disconnected"));
    drop(body);
}

#[tokio::test]
async fn a_request_body_that_stops_arriving_is_refused() {
    use axum::body::{Body, Bytes};
    use axum::http::Request;
    use futures::StreamExt;
    use tower::ServiceExt;
    let env = env(&[("KAESO_SERVER_BODY_READ_TIMEOUT", "1s")]).await;
    let stalled =
        futures::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"{\"model\":"))])
            .chain(futures::stream::pending());
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from_stream(stalled))
        .unwrap();
    let started = Instant::now();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["error"]["message"],
        "The request body did not arrive in time; send it again."
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn a_request_body_dripped_in_time_for_each_part_is_refused_at_its_deadline() {
    use axum::body::{Body, Bytes};
    use axum::http::Request;
    use tower::ServiceExt;
    let env = env(&[
        ("KAESO_SERVER_BODY_READ_TIMEOUT", "1s"),
        ("KAESO_SERVER_BODY_READ_DEADLINE", "2s"),
    ])
    .await;
    assert_eq!(
        env.app.state.config.server.body_read_deadline,
        Duration::from_secs(2)
    );
    let body = serde_json::to_vec(&chat("gpt-4o", "Hi")).unwrap();
    let dripped = futures::stream::unfold(body.into_iter(), |mut bytes| async move {
        let byte = bytes.next()?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        Some((Ok::<_, std::io::Error>(Bytes::from(vec![byte])), bytes))
    });
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "application/json")
        .body(Body::from_stream(dripped))
        .unwrap();
    let started = Instant::now();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let waited = started.elapsed();
    assert_eq!(resp.status(), 400);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["error"]["message"],
        "The request body did not arrive in time; send it again."
    );
    assert!(
        (Duration::from_secs(2)..Duration::from_secs(4)).contains(&waited),
        "{waited:?}"
    );
    assert!(env.mock.received().is_empty());
}
