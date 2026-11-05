#![allow(clippy::unwrap_used)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const READ_ONLY: &str = "This demo is read-only; install Kaeso to change it.";

async fn demo() -> (TestServices, App, common::Owner, App) {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let demo = App::start_with(&s, &[("KAESO_AUTH_DEMO_USER", "owner@example.com")]).await;
    (s, app, owner, demo)
}

async fn visit(demo: &App) -> String {
    let r = demo
        .post("/auth/demo", json!({}), None)
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
