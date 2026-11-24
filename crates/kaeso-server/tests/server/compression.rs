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
    send_from(app, method, path, cookie, body, gzip, Some("same-origin")).await
}

async fn send_from(
    app: &App,
    method: Method,
    path: &str,
    cookie: &str,
    body: Option<Value>,
    gzip: bool,
    site: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .header(header::ORIGIN, ORIGIN);
    if gzip {
        req = req.header(header::ACCEPT_ENCODING, "gzip, deflate, br, zstd");
    }
    if let Some(site) = site {
        req = req.header("sec-fetch-site", site);
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
async fn platform_api_reads_come_compressed_to_the_console_and_to_platform_keys() {
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
    // A platform key goes without a cookie, which no browser sends by itself.
    let key = app
        .post(
            &format!("/orgs/{}/platform-keys", owner.org_id),
            json!({ "name": "CI", "expires_in_days": 30 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let secret = key.body["secret"].as_str().unwrap();
    let req = Request::builder()
        .uri(format!("/platform/v1/orgs/{}/projects", owner.org_id))
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::ACCEPT_ENCODING, "gzip")
        .body(Body::empty())
        .unwrap();
    let (status, headers, packed) = app.raw_bytes(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    let projects: Value = serde_json::from_slice(&gunzip(&packed)).unwrap();
    assert_eq!(projects["data"].as_array().unwrap().len(), 1);
}

// Compression-length attacks (BREACH) load a page from another site again and again with the
// victim's cookie; only the console's own reads, and changes never, are compressed.
#[tokio::test]
async fn reads_from_another_site_changes_and_the_model_api_stay_uncompressed() {
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

    let rights = format!("/platform/v1/orgs/{}/rights", owner.org_id);
    for site in [Some("cross-site"), Some("same-site"), Some("none"), None] {
        let (status, headers, _) =
            send_from(&app, Method::GET, &rights, &owner.cookie, None, true, site).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(header::CONTENT_ENCODING).is_none(), "{site:?}");
    }

    let (_, headers, _) = send(&app, Method::GET, "/v1/models", &owner.cookie, None, true).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
}
