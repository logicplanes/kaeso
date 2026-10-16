mod common;

use axum::http::StatusCode;
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

#[tokio::test]
async fn every_job_runs_once_on_a_fresh_install() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    for job in kaeso_server::jobs::all() {
        let outcome = kaeso_server::jobs::run_once(&app.state, job).await;
        assert!(outcome.is_ok(), "{}: {outcome:?}", job.name);
    }
    s.cleanup().await;
}

#[tokio::test]
async fn service_account_keys_warn_the_project_admins() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Pipelines" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(project.status, StatusCode::CREATED, "{:?}", project.body);
    let base = format!(
        "/orgs/{}/projects/{}",
        owner.org_id,
        project.body["id"].as_str().unwrap()
    );

    let (_, viewer) = common::join(
        &app,
        &owner,
        json!({ "email": "viewer@example.com", "role": "member", "project_roles": [] }),
        "Viewer",
    )
    .await;
    let r = app
        .post(
            &format!("{base}/members"),
            json!({ "user_id": viewer, "role": "viewer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);

    let account = app
        .post(
            &format!("{base}/service-accounts"),
            json!({ "name": "deployer", "role": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(account.status, StatusCode::CREATED, "{:?}", account.body);
    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "deploy", "service_account_id": account.body["id"], "expires_in_days": 1 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);

    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    let outcome = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome, "sent 1 key expiry warnings");
    app.state.mailer.flush().await;
    let warned = |to: &str| {
        app.mail
            .to(to)
            .iter()
            .filter(|m| m.kind.as_str() == "key_expiry_warning")
            .count()
    };
    assert_eq!(warned("owner@example.com"), 1);
    assert_eq!(warned("viewer@example.com"), 0);
    s.cleanup().await;
}
