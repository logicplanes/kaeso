#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::io::Read;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use common::{App, ORIGIN};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn send(
    app: &App,
    method: Method,
    path: &str,
    cookie: &str,
    body: Option<Value>,
    gzip: bool,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .header(header::ORIGIN, ORIGIN);
    if gzip {
        req = req.header(header::ACCEPT_ENCODING, "gzip, deflate, br, zstd");
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    app.raw_bytes(req).await
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .unwrap();
    out
}

#[tokio::test]
async fn platform_api_reads_come_compressed_to_a_client_that_takes_gzip() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let rights = format!("/platform/v1/orgs/{}/rights", owner.org_id);

    let (status, headers, plain) =
        send(&app, Method::GET, &rights, &owner.cookie, None, false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert!(
        plain.len() > 4096,
        "the rights list is {} bytes",
        plain.len()
    );

    let (status, headers, packed) =
        send(&app, Method::GET, &rights, &owner.cookie, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    assert!(
        headers[header::VARY]
            .to_str()
            .unwrap()
            .to_ascii_lowercase()
            .contains("accept-encoding")
    );
    assert!(
        packed.len() * 4 < plain.len(),
        "{} of {} bytes",
        packed.len(),
        plain.len()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&gunzip(&packed)).unwrap(),
        serde_json::from_slice::<Value>(&plain).unwrap()
    );
}

// A change can answer with a new secret beside text the request chose, which is what
// compression-length attacks (BREACH) measure; only reads are compressed.
#[tokio::test]
async fn platform_api_changes_and_the_model_api_stay_uncompressed() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;

    let (status, headers, body) = send(
        &app,
        Method::POST,
        &format!("/platform/v1/orgs/{}/projects/{project}/keys", owner.org_id),
        &owner.cookie,
        Some(json!({ "name": "deploy ".repeat(14).trim() })),
        true,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    let created: Value = serde_json::from_slice(&body).unwrap();
    assert!(created["secret"].as_str().unwrap().starts_with("sk-kso-"));

    let (_, headers, _) = send(&app, Method::GET, "/v1/models", &owner.cookie, None, true).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
}
