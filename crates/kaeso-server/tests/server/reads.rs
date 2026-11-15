use crate::common;

use axum::http::StatusCode;
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

#[tokio::test]
async fn resources_are_readable_by_id() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    s.clickhouse.migrate(&s.pg).await.unwrap();
    let owner = app.setup().await;
    let org = &owner.org_id;
    let project = app
        .get(&format!("/orgs/{org}/projects"), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let r = app
        .get(
            &format!("/orgs/{org}/projects/{project}/members/{}", owner.user_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["user"]["id"], owner.user_id);
    let inv = app
        .post(
            &format!("/orgs/{org}/invites"),
            json!({ "email": "new@example.com", "preset": "member" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(inv.status, StatusCode::CREATED, "{:?}", inv.body);
    let r = app
        .get(
            &format!("/orgs/{org}/invites/{}", inv.body["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.body["email"], "new@example.com");
    let k = app
        .post(
            &format!("/orgs/{org}/platform-keys"),
            json!({ "name": "ci" }),
            Some(&owner.cookie),
        )
        .await;
    let r = app
        .get(
            &format!(
                "/orgs/{org}/platform-keys/{}",
                k.body["id"].as_str().unwrap()
            ),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.body["name"], "ci");
    let k = app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": "mine" }),
            Some(&owner.cookie),
        )
        .await;
    let r = app
        .get(
            &format!("/me/keys/{}", k.body["id"].as_str().unwrap()),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.body["name"], "mine");
    let r = app.get("/me/sessions", &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let list = app
        .get(&format!("/orgs/{org}/audit-log"), &owner.cookie)
        .await;
    let event = list.body["data"][0]["id"].as_str().unwrap();
    let r = app
        .get(&format!("/orgs/{org}/audit-log/{event}"), &owner.cookie)
        .await;
    assert_eq!(r.body["id"], event);
    let list = app.get("/instance/audit-log", &owner.cookie).await;
    let event = list.body["data"][0]["id"].as_str().unwrap();
    assert_eq!(
        app.get(&format!("/instance/audit-log/{event}"), &owner.cookie)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}/audit-log/{event}"), &owner.cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let r = app
        .get(&format!("/instance/orgs/{org}/projects"), &owner.cookie)
        .await;
    assert_eq!(r.body["data"][0]["id"], project);
    let r = app
        .get(&format!("/instance/orgs/{org}/usage"), &owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["requests_30d"], 0);
    let r = app.get("/instance/system", &owner.cookie).await;
    assert_eq!(r.body["log_buffer"]["rows"], 0);
}

#[tokio::test]
async fn a_filter_with_a_nul_character_is_refused_naming_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    s.clickhouse.migrate(&s.pg).await.unwrap();
    let owner = app.setup().await;
    let org = &owner.org_id;
    let project = common::first_project(&app, &owner).await;
    for (path, param) in [
        ("/instance/audit-log?action=%00".to_owned(), "action"),
        ("/instance/audit-log?target=%00".to_owned(), "target"),
        ("/instance/users?state=%00".to_owned(), "state"),
        (format!("/orgs/{org}/audit-log?action=%00"), "action"),
        (format!("/orgs/{org}/audit-log?target=%00"), "target"),
        (format!("/orgs/{org}/audit-log/export?action=%00"), "action"),
        (format!("/orgs/{org}/members?role=%00"), "role"),
        (format!("/orgs/{org}/members?preset=%00"), "preset"),
        (
            format!("/orgs/{org}/projects/{project}/audit-log?action=%00"),
            "action",
        ),
        (format!("/orgs/{org}/usage?model=%00"), "model"),
        (
            format!("/orgs/{org}/projects/{project}/usage?model=%00"),
            "model",
        ),
    ] {
        let r = app.get(&path, &owner.cookie).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{path}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{path}");
        assert_eq!(
            r.body["error"]["message"],
            format!("`{param}` must be text without a NUL character."),
            "{path}"
        );
    }
    s.cleanup().await;
}
