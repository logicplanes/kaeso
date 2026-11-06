#![allow(clippy::unwrap_used)]

mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use common::App;
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const READ_ONLY: &str = "This demo is read-only; install Kaeso to change it.";
const NO_PROVIDERS: &str = "This demo calls no providers; install Kaeso to connect your own.";
const DEMO_USER: (&str, &str) = ("KAESO_AUTH_DEMO_USER", "owner@example.com");
const LOOPBACK: (&str, &str) = (
    "KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS",
    "127.0.0.0/8,::1/128",
);

async fn demo() -> (TestServices, App, common::Owner, App) {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let demo = App::start_with(&s, &[DEMO_USER]).await;
    (s, app, owner, demo)
}

async fn visit(demo: &App) -> String {
    let r = demo
        .post("/auth/demo", json!({}), None)
        .await
        .expect_status(StatusCode::OK);
    r.cookie().unwrap()
}

async fn visit_from(demo: &App, peer: [u8; 4], browser: &str) -> String {
    let r = demo
        .raw(
            Request::post("/platform/v1/auth/demo")
                .header(header::ORIGIN, common::ORIGIN)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::USER_AGENT, browser)
                .extension(ConnectInfo(SocketAddr::from((peer, 40000))))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::OK);
    r.cookie().unwrap()
}

#[tokio::test]
async fn a_visitor_signs_in_as_the_demo_user_with_one_request() {
    let (_s, app, _owner, demo) = demo().await;
    let methods = demo.call(Method::GET, "/auth/methods", None, None).await;
    assert_eq!(methods.body["demo"], json!(true), "{:?}", methods.body);
    let elsewhere = app.call(Method::GET, "/auth/methods", None, None).await;
    assert_eq!(elsewhere.body["demo"], json!(false));

    let cookie = visit(&demo).await;
    let session = demo
        .get("/auth/session", &cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        session.body["user"]["email"],
        json!("owner@example.com"),
        "{:?}",
        session.body
    );
    let org = session.body["memberships"][0]["org_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let projects = demo
        .get(&format!("/orgs/{org}/projects"), &cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(projects.body["data"][0]["name"], json!("Checkout"));
}

#[tokio::test]
async fn the_demo_refuses_every_change_but_signing_out() {
    let (_s, _app, owner, demo) = demo().await;
    let cookie = visit(&demo).await;
    let refused = |r: common::Resp| {
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
        assert_eq!(
            r.body.pointer("/error/message").and_then(Value::as_str),
            Some(READ_ONLY)
        );
    };
    refused(
        demo.post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Search" }),
            Some(&cookie),
        )
        .await,
    );
    refused(
        demo.call(
            Method::PATCH,
            "/me",
            Some(json!({ "name": "Visitor" })),
            Some(&cookie),
        )
        .await,
    );
    refused(
        demo.post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": common::PASSWORD }),
            None,
        )
        .await,
    );
    let still = demo
        .get(&format!("/orgs/{}/projects", owner.org_id), &cookie)
        .await;
    assert_eq!(
        still.body["data"].as_array().unwrap().len(),
        1,
        "nothing was created"
    );

    demo.post("/auth/logout", json!({}), Some(&cookie))
        .await
        .expect_status(StatusCode::NO_CONTENT);
    demo.get("/auth/session", &cookie)
        .await
        .expect_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_demo_has_no_model_api_and_an_install_without_one_has_no_demo_sign_in() {
    let (_s, app, _owner, demo) = demo().await;
    let models = demo
        .raw(
            Request::builder()
                .method(Method::GET)
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(models.status, StatusCode::NOT_FOUND);
    let r = app.post("/auth/demo", json!({}), None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert!(r.cookie().is_none());
}

#[tokio::test]
async fn each_visitor_sees_only_their_own_session() {
    let (_s, app, owner, demo) = demo().await;
    let visitors = [
        ("203.0.113.7", "Firefox/144.0"),
        ("198.51.100.23", "Safari/26.1"),
    ];
    let first = visit_from(&demo, [203, 0, 113, 7], visitors[0].1).await;
    let second = visit_from(&demo, [198, 51, 100, 23], visitors[1].1).await;

    for (cookie, (ip, browser)) in [first, second].iter().zip(visitors) {
        for path in ["/me/sessions", "/auth/sessions"] {
            let listed = demo.get(path, cookie).await.expect_status(StatusCode::OK);
            let data = listed.body["data"].as_array().unwrap();
            assert_eq!(data.len(), 1, "{path} lists one session: {:?}", listed.body);
            assert_eq!(data[0]["current"], json!(true), "{path}");
            assert_eq!(data[0]["ip"], json!(ip), "{path}");
            assert_eq!(data[0]["user_agent"], json!(browser), "{path}");
        }
    }

    let elsewhere = app
        .get("/auth/sessions", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    let ips: Vec<&Value> = elsewhere.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["ip"])
        .collect();
    for (ip, _) in visitors {
        assert!(
            ips.contains(&&json!(ip)),
            "an install without a demo lists every session: {ips:?}"
        );
    }
}

#[tokio::test]
async fn the_demo_asks_no_provider_which_models_it_lists() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let demo = App::start_with(&s, &[LOOPBACK, DEMO_USER]).await;
    let mock = MockProvider::start().await;
    app.post(
        &format!("/orgs/{}/custom-providers", owner.org_id),
        json!({ "id": "lab", "name": "Lab", "base_url": mock.base_url, "auth": "bearer", "generation": "openai_chat" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    app.post(
        &format!("/orgs/{}/credentials", owner.org_id),
        json!({ "provider": "lab", "name": "lab", "secret": "lab-test-key-0000000001" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let project = common::first_project(&app, &owner).await;
    let offered = format!(
        "/orgs/{}/projects/{project}/models/offered?provider=lab",
        owner.org_id
    );
    app.get(&offered, &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(mock.received().len(), 1, "an install without a demo asks");
    mock.clear();

    let cookie = visit(&demo).await;
    let r = demo.get(&offered, &cookie).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body.pointer("/error/message").and_then(Value::as_str),
        Some(NO_PROVIDERS)
    );
    assert!(mock.received().is_empty(), "the demo called the provider");
}
