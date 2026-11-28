#![allow(clippy::unwrap_used)]

use crate::common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::App;
use kaeso_testkit::services::TestServices;

const APP_ORIGIN: &str = "https://app.example.com";

fn request(method: Method, path: &str) -> axum::http::request::Builder {
    Request::builder().method(method).uri(path)
}

async fn answers(app: &App, cookie: &str) -> Vec<(String, StatusCode, Option<String>)> {
    let session = format!("__Host-kaeso_session={cookie}");
    let requests = [
        request(Method::GET, "/healthz"),
        request(Method::GET, "/readyz"),
        request(Method::GET, "/robots.txt"),
        request(Method::GET, "/platform/v1/auth/methods"),
        request(Method::GET, "/platform/v1/instance/settings")
            .header(header::COOKIE, session.as_str()),
        request(Method::GET, "/platform/v1/auth/session"),
        request(Method::POST, "/v1/chat/completions"),
        request(Method::GET, "/.well-known/oauth-authorization-server"),
        request(Method::GET, "/sign-in"),
        request(Method::OPTIONS, "/v1/chat/completions")
            .header(header::ORIGIN, APP_ORIGIN)
            .header("access-control-request-method", "POST"),
    ];
    let mut out = Vec::new();
    for builder in requests {
        let req = builder.body(Body::empty()).unwrap();
        let what = format!("{} {}", req.method(), req.uri());
        let (status, headers, _) = app.raw_bytes(req).await;
        let tag = headers
            .get("x-robots-tag")
            .map(|v| v.to_str().unwrap().to_owned());
        out.push((what, status, tag));
    }
    out
}

#[tokio::test]
async fn every_answer_of_the_gateway_asks_crawlers_to_leave_it_out_whatever_the_setting() {
    let s = TestServices::start().await;
    let off = App::start_with(&s, &[("KAESO_CORS_ALLOWED_ORIGINS", APP_ORIGIN)]).await;
    let owner = off.setup().await;
    let on = off
        .second_instance_with(
            &s,
            &[
                ("KAESO_SEARCH_ENGINES", "true"),
                ("KAESO_CORS_ALLOWED_ORIGINS", APP_ORIGIN),
            ],
        )
        .await;
    for (setting, app) in [("false", &off), ("true", &on)] {
        let seen = answers(app, &owner.cookie).await;
        let statuses: Vec<(&str, u16)> = seen
            .iter()
            .map(|(what, status, _)| (what.as_str(), status.as_u16()))
            .collect();
        assert_eq!(
            statuses,
            [
                ("GET /healthz", 200),
                ("GET /readyz", 200),
                ("GET /robots.txt", 200),
                ("GET /platform/v1/auth/methods", 200),
                ("GET /platform/v1/instance/settings", 200),
                ("GET /platform/v1/auth/session", 401),
                ("POST /v1/chat/completions", 401),
                ("GET /.well-known/oauth-authorization-server", 200),
                ("GET /sign-in", 404),
                ("OPTIONS /v1/chat/completions", 204),
            ],
            "search_engines = {setting}"
        );
        for (what, _, tag) in &seen {
            assert_eq!(
                tag.as_deref(),
                Some("noindex, nofollow"),
                "{what} with search_engines = {setting}"
            );
        }
    }
    s.cleanup().await;
}

#[tokio::test]
async fn the_gateways_robots_txt_keeps_every_crawler_out_whatever_the_setting() {
    let s = TestServices::start().await;
    for setting in ["false", "true"] {
        let app = App::start_with(&s, &[("KAESO_SEARCH_ENGINES", setting)]).await;
        let (status, headers, body) = app
            .raw_bytes(
                request(Method::GET, "/robots.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "User-agent: *\nDisallow: /\n",
            "search_engines = {setting}"
        );
    }
    s.cleanup().await;
}

#[tokio::test]
async fn the_instance_settings_show_whether_search_engines_may_index_the_sign_in_pages() {
    let s = TestServices::start().await;
    let off = App::start(&s).await;
    let owner = off.setup().await;
    let r = off.get("/instance/settings", &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["search_engines"], false, "off unless set");

    let on = off
        .second_instance_with(&s, &[("KAESO_SEARCH_ENGINES", "on")])
        .await;
    let r = on.get("/instance/settings", &owner.cookie).await;
    assert_eq!(r.body["search_engines"], true);
    s.cleanup().await;
}

#[test]
fn a_value_that_isnt_true_or_false_stops_the_gateway() {
    let source = kaeso_server::config::MapSource::new()
        .set("KAESO_PUBLIC_URL", "https://kaeso.example.com")
        .set(
            "KAESO_MASTER_KEYS",
            "k1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        )
        .set(
            "KAESO_POSTGRES_URL",
            "postgres://kaeso:secret@postgres:5432/kaeso",
        )
        .set("KAESO_VALKEY_URL", "redis://valkey:6379")
        .set("KAESO_MAIL_TRANSPORT", "log")
        .set("KAESO_SEARCH_ENGINES", "sometimes");
    let error = kaeso_server::config::load(&source).unwrap_err().to_string();
    assert!(
        error.contains("KAESO_SEARCH_ENGINES") && error.contains("must be true or false"),
        "{error}"
    );
}
