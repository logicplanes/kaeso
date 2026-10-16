mod common;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

const ROUNDS: usize = 20;

#[tokio::test]
async fn project_rename_and_content_logging_both_stay() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let r = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Support bot" }),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let project = format!(
        "/orgs/{}/projects/{}",
        owner.org_id,
        r.body["id"].as_str().unwrap()
    );
    let settings = format!("{project}/logs/settings");

    for round in 0..ROUNDS {
        let name = format!("Support bot {round}");
        let logging = round % 2 == 1;
        let (renamed, switched) = tokio::join!(
            app.call(
                Method::PATCH,
                &project,
                Some(json!({ "name": name })),
                cookie
            ),
            app.call(
                Method::PATCH,
                &settings,
                Some(json!({ "content_logging": logging })),
                cookie
            ),
        );
        assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.body);
        assert_eq!(switched.status, StatusCode::OK, "{:?}", switched.body);
        let now = app.get(&project, &owner.cookie).await;
        assert_eq!(now.body["name"], name.as_str(), "round {round}");
        assert_eq!(now.body["content_logging"], logging, "round {round}");
    }
}

#[tokio::test]
async fn org_rename_and_security_setting_both_stay() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let org = format!("/orgs/{}", owner.org_id);
    let key_policy = format!("{org}/security/key-policy");

    for round in 0..ROUNDS {
        let name = format!("Acme {round}");
        let days = i32::try_from(round).unwrap() + 30;
        let (renamed, limited) = tokio::join!(
            app.call(Method::PATCH, &org, Some(json!({ "name": name })), cookie),
            app.call(
                Method::PUT,
                &key_policy,
                Some(json!({ "max_key_lifetime_days": days })),
                cookie
            ),
        );
        assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.body);
        assert_eq!(limited.status, StatusCode::OK, "{:?}", limited.body);
        let now = app.get(&org, &owner.cookie).await;
        assert_eq!(now.body["name"], name.as_str(), "round {round}");
        assert_eq!(now.body["max_key_lifetime_days"], days, "round {round}");
    }
}
