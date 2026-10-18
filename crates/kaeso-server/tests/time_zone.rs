#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, PASSWORD};
use kaeso_testkit::services::TestServices;
use serde_json::json;

fn zone_of(r: &common::Resp) -> &str {
    r.headers["kaeso-time-zone"].to_str().unwrap()
}

#[tokio::test]
async fn setup_takes_the_first_users_time_zone_and_admins_change_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let url = kaeso_server::setup::issue_link(&app.state)
        .await
        .unwrap()
        .unwrap();
    let token = url.split("#token=").nth(1).unwrap().to_owned();
    let body = |zone: &str| json!({ "token": token, "email": "owner@example.com", "name": "Owner", "password": PASSWORD, "org_name": "Acme", "timezone": zone });
    let unknown = app.post("/auth/setup", body("Berlin"), None).await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.body["error"]["param"], "timezone");
    assert_eq!(
        unknown.body["error"]["message"],
        "`timezone` isn't a known time zone; choose one such as `Europe/Berlin`."
    );
    let r = app.post("/auth/setup", body("Europe/Berlin"), None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let cookie = r.cookie().unwrap();

    let who = app.get("/auth/session", &cookie).await;
    assert_eq!(who.body["timezone"], "Europe/Berlin");
    assert_eq!(zone_of(&who), "Europe/Berlin");
    let settings = app.get("/instance/settings", &cookie).await;
    assert_eq!(settings.body["timezone"], "Europe/Berlin");

    let wrong = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "timezone": "Mars/Olympus" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong.body["error"]["param"], "timezone");

    let changed = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "timezone": " Asia/Tokyo " })),
            Some(&cookie),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    assert_eq!(changed.body["timezone"], "Asia/Tokyo");
    let who = app.get("/auth/session", &cookie).await;
    assert_eq!(who.body["timezone"], "Asia/Tokyo");
    assert_eq!(zone_of(&who), "Asia/Tokyo", "the header follows at once");

    let updates = app.get("/instance/updates", &cookie).await;
    assert_eq!(
        updates.body["settings"]["schedule"]["timezone"],
        "Asia/Tokyo"
    );

    let audit = app
        .get(
            "/instance/audit-log?action=instance.settings_update",
            &cookie,
        )
        .await;
    let event = &audit.body["data"][0];
    assert_eq!(event["before"]["timezone"], "Europe/Berlin");
    assert_eq!(event["after"]["timezone"], "Asia/Tokyo");
    s.cleanup().await;
}

#[tokio::test]
async fn a_new_install_starts_in_the_configured_time_zone() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[("KAESO_INSTANCE_TIMEZONE", "America/New_York")]).await;
    let owner = app.setup().await;
    let who = app.get("/auth/session", &owner.cookie).await;
    assert_eq!(who.body["timezone"], "America/New_York");
    let anonymous = app.call(Method::GET, "/auth/methods", None, None).await;
    assert_eq!(zone_of(&anonymous), "America/New_York");
    s.cleanup().await;
}
