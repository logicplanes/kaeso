use crate::common;

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
            &format!(
                "{base}/service-accounts/{}/keys",
                account.body["id"].as_str().unwrap()
            ),
            json!({ "name": "deploy", "expires_in_days": 1 }),
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

#[tokio::test]
async fn expiring_platform_keys_warn_who_may_rotate_them_once() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = format!("/orgs/{}/platform-keys", owner.org_id);
    let join = |email: &'static str, role: &'static str| {
        common::join(
            &app,
            &owner,
            json!({ "email": email, "role": role, "project_roles": [] }),
            "Member",
        )
    };
    join("admin@example.com", "admin").await;
    join("member@example.com", "member").await;
    let key = |name: &'static str, days: i64| {
        let app = &app;
        let base = base.clone();
        let cookie = owner.cookie.clone();
        async move {
            let r = app
                .post(
                    &base,
                    json!({ "name": name, "expires_in_days": days }),
                    Some(&cookie),
                )
                .await;
            assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
            r.body["key"]["id"]
                .as_str()
                .or_else(|| r.body["id"].as_str())
                .unwrap()
                .to_owned()
        }
    };
    key("terraform", 1).await;
    key("later", 30).await;
    let disabled = key("disabled", 1).await;
    let archived = key("archived", 7).await;
    for (id, action) in [(disabled, "disable"), (archived, "archive")] {
        let r = app
            .post(
                &format!("{base}/{id}/{action}"),
                json!({}),
                Some(&owner.cookie),
            )
            .await;
        assert!(r.status.is_success(), "{action}: {:?}", r.body);
    }

    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    let outcome = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome, "sent 2 key expiry warnings");
    app.state.mailer.flush().await;
    let warned = |to: &str| {
        app.mail
            .to(to)
            .iter()
            .filter(|m| m.kind.as_str() == "platform_key_expiry_warning")
            .map(|m| m.subject.clone())
            .collect::<Vec<_>>()
    };
    let owner_mail = warned("owner@example.com");
    assert_eq!(owner_mail.len(), 1, "{owner_mail:?}");
    assert!(owner_mail[0].contains("terraform"), "{owner_mail:?}");
    assert_eq!(warned("admin@example.com").len(), 1);
    assert!(warned("member@example.com").is_empty());

    // The 1-day warning also stands for the 7-day one, and nothing is sent twice.
    let again = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again, "sent 0 key expiry warnings");
    s.cleanup().await;
}
