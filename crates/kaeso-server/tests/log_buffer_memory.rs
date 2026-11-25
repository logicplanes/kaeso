#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

// The only test in its binary: the allocator counts every thread of the process, the mock
// provider's included.

#[path = "server/common/mod.rs"]
mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use http_body_util::BodyExt;
use peak_alloc::PeakAlloc;
use serde_json::json;
use tower::ServiceExt;

#[global_allocator]
static ALLOC: PeakAlloc = PeakAlloc;

const BUFFER_BYTES: usize = 1 << 20;
const READ_BUFFER_BYTES: usize = 8 * 1024;

async fn chat(env: &common::v1::Env) {
    // As a connection hands it over: a short body at the start of a read buffer.
    let mut body = Vec::with_capacity(READ_BUFFER_BYTES);
    serde_json::to_writer(
        &mut body,
        &json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Hello"}],
        }),
    )
    .unwrap();
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", env.key))
        .body(Body::from(Bytes::from(body)))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let reply = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{reply:?}");
    env.app.state.in_flight.settled().await;
    env.mock.clear();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_rows_that_wait_for_clickhouse_hold_no_more_memory_than_the_buffer_allows() {
    let env = common::v1::env(None, Some(&[("KAESO_LOGS_BUFFER_SIZE", "1MiB")])).await;
    // One request fills the caches every later one reads; its own row is what the buffer
    // holds before the measurement, about a quarter of a percent of the limit.
    chat(&env).await;
    assert_eq!(env.app.state.logs.len(), 1, "nothing writes the rows");
    let before = ALLOC.current_usage();
    // Enough requests to fill the buffer several times over, so it holds as much as it ever will.
    while env.app.state.logs.dropped() < 2000 {
        chat(&env).await;
    }
    let held = ALLOC.current_usage().saturating_sub(before);
    assert!(
        held <= BUFFER_BYTES * 5 / 4,
        "{held} bytes held for {} waiting rows in a buffer of {BUFFER_BYTES}",
        env.app.state.logs.len()
    );
}
