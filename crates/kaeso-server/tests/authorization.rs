#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const RIGHTS_MIGRATION: &str = include_str!(
    "../../kaeso-store/migrations/postgres/20261019120000_owner_and_content_logging_rights.sql"
);

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await;
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["is_default"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn join(app: &App, owner: &Owner, email: &str, role: &str) -> (String, String) {
    common::join(app, owner, json!({ "email": email, "role": role }), email).await
}

fn rights(v: &Value) -> Vec<&str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn only_owners_transfer_ownership_and_no_grant_gives_the_right() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (admin, admin_id) = join(&app, &owner, "admin@example.com", "admin").await;
    let (_, dev_id) = join(&app, &owner, "dev@example.com", "developer").await;

    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{admin_id}"),
            Some(json!({ "role": "admin", "rights_added": ["org.ownership.transfer"] })),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "rights_added");
    let r = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Successor", "rights": ["org.view", "members.view", "org.ownership.transfer"] }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "rights");

    let role = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Successor", "rights": ["org.view", "members.view"] }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.call(
        Method::PATCH,
        &format!("/orgs/{org}/members/{admin_id}"),
        Some(json!({ "role": role })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    sqlx::query(
        "UPDATE org_roles SET rights = rights || '{org.ownership.transfer}'::text[] WHERE org_id IS NOT NULL",
    )
    .execute(&app.state.pg)
    .await
    .unwrap();
    sqlx::query("UPDATE org_memberships SET rights_added = '{org.ownership.transfer}' WHERE preset = 'admin'")
        .execute(&app.state.pg)
        .await
        .unwrap();

    let held = app
        .get(&format!("/orgs/{org}/members/{admin_id}"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert!(!rights(&held.body["grant"]["rights"]).contains(&"org.ownership.transfer"));
    let r = app
        .post(
            &format!("/orgs/{org}/ownership-transfer"),
            json!({ "user_id": dev_id }),
            Some(&admin),
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["right"], "org.ownership.transfer");
    let dev = app
        .get(&format!("/orgs/{org}/members/{dev_id}"), &owner.cookie)
        .await;
    assert_eq!(dev.body["grant"]["role"]["id"], "developer");

    sqlx::raw_sql(RIGHTS_MIGRATION)
        .execute(&app.state.pg)
        .await
        .unwrap();
    let stored = app
        .get(&format!("/orgs/{org}/roles/{role}"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(rights(&stored.body["rights"]), ["org.view", "members.view"]);
    app.call(
        Method::PATCH,
        &format!("/orgs/{org}/roles/{role}"),
        Some(json!({ "name": "Deputy" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let catalogue = app.get(&format!("/orgs/{org}/rights"), &owner.cookie).await;
    let transfer = catalogue.body["org"]["categories"][0]["rights"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "org.ownership.transfer")
        .unwrap()
        .clone();
    assert_eq!(transfer["owner_only"], true);

    app.post(
        &format!("/orgs/{org}/ownership-transfer"),
        json!({ "user_id": dev_id }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let me = app
        .get(
            &format!("/orgs/{org}/members/{}", owner.user_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(me.body["grant"]["role"]["id"], "admin");
    s.cleanup().await;
}

#[tokio::test]
async fn content_logging_rights_move_to_the_new_right() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "admin").await;
    sqlx::query(
        "UPDATE project_memberships SET preset = 'admin', rights_removed = '{logs.content.disable}' \
         WHERE user_id = (SELECT id FROM users WHERE email = 'dev@example.com')",
    )
    .execute(&app.state.pg)
    .await
    .unwrap();
    app.call(
        Method::PATCH,
        &format!("/orgs/{org}/members/{dev_id}"),
        Some(json!({ "role": "member" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    sqlx::raw_sql(RIGHTS_MIGRATION)
        .execute(&app.state.pg)
        .await
        .unwrap();
    let member = app
        .get(
            &format!("/orgs/{org}/projects/{project}/members/{dev_id}"),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        rights(&member.body["grant"]["rights_removed"]),
        ["logs.content.manage"]
    );
    for on in [false, true] {
        let r = app
            .call(
                Method::PATCH,
                &format!("/orgs/{org}/projects/{project}/logs/settings"),
                Some(json!({ "content_logging": on })),
                Some(&dev),
            )
            .await
            .expect_status(StatusCode::FORBIDDEN);
        assert_eq!(r.body["error"]["right"], "logs.content.manage");
    }
    s.cleanup().await;
}

#[tokio::test]
async fn service_accounts_change_only_for_callers_holding_their_rights() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}/service-accounts");
    let account = |role: &'static str| {
        let app = &app;
        let base = base.clone();
        let cookie = owner.cookie.clone();
        async move {
            app.post(&base, json!({ "name": role, "role": role }), Some(&cookie))
                .await
                .expect_status(StatusCode::CREATED)
                .body["id"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    let strong = account("admin").await;
    let weak = account("viewer").await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "member").await;
    app.call(
        Method::PATCH,
        &format!("/orgs/{org}/projects/{project}/members/{dev_id}"),
        Some(json!({
            "role": "developer",
            "rights_added": ["service_accounts.rights.update", "service_accounts.archive"],
        })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let r = app
        .call(
            Method::PATCH,
            &format!("{base}/{strong}"),
            Some(json!({ "name": "renamed" })),
            Some(&dev),
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "You can't change a service account that holds rights you don't hold."
    );
    app.call(
        Method::PATCH,
        &format!("{base}/{strong}"),
        Some(json!({ "grant": { "role": "viewer" } })),
        Some(&dev),
    )
    .await
    .expect_status(StatusCode::FORBIDDEN);
    app.post(&format!("{base}/{strong}/archive"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::FORBIDDEN);
    let still = app
        .get(&format!("{base}/{strong}"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(still.body["name"], "admin");
    assert_eq!(still.body["grant"]["role"]["id"], "admin");
    assert!(still.body["archived_at"].is_null());

    app.call(
        Method::PATCH,
        &format!("{base}/{weak}"),
        Some(json!({ "name": "reader" })),
        Some(&dev),
    )
    .await
    .expect_status(StatusCode::OK);
    app.post(&format!("{base}/{weak}/archive"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn project_member_changes_count_the_rights_of_the_org_role() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let members = format!("/orgs/{org}/projects/{project}/members");
    let (lead, lead_id) = join(&app, &owner, "lead@example.com", "developer").await;
    let (_, auditor_id) = join(&app, &owner, "auditor@example.com", "auditor").await;
    let (_, member_id) = join(&app, &owner, "member@example.com", "member").await;
    app.call(
        Method::PATCH,
        &format!("{members}/{lead_id}"),
        Some(json!({ "role": "admin", "rights_removed": ["project.audit_log.view"] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let auditor = app
        .get(&format!("{members}/{auditor_id}"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(auditor.body["grant"]["role"]["id"], "viewer");
    assert!(!rights(&auditor.body["grant"]["rights"]).contains(&"project.audit_log.view"));

    for (method, body) in [
        (Method::PATCH, Some(json!({ "role": "viewer" }))),
        (Method::DELETE, None),
    ] {
        let r = app
            .call(
                method,
                &format!("{members}/{auditor_id}"),
                body,
                Some(&lead),
            )
            .await
            .expect_status(StatusCode::FORBIDDEN);
        assert_eq!(
            r.body["error"]["message"],
            "You can't change a member who holds rights you don't hold."
        );
    }
    app.get(&format!("{members}/{auditor_id}"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);

    app.call(
        Method::PATCH,
        &format!("{members}/{member_id}"),
        Some(json!({ "role": "developer" })),
        Some(&lead),
    )
    .await
    .expect_status(StatusCode::OK);
    app.call(
        Method::DELETE,
        &format!("{members}/{member_id}"),
        None,
        Some(&lead),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    s.cleanup().await;
}

#[tokio::test]
async fn only_owners_change_or_remove_owners_in_projects() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let members = format!("/orgs/{org}/projects/{project}/members");
    let (lead, lead_id) = join(&app, &owner, "lead@example.com", "developer").await;
    let (_, heir_id) = join(&app, &owner, "heir@example.com", "owner").await;
    let (_, member_id) = join(&app, &owner, "member@example.com", "member").await;
    app.call(
        Method::PATCH,
        &format!("{members}/{lead_id}"),
        Some(json!({ "role": "admin" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let key = app
        .post(
            &format!("/orgs/{org}/platform-keys"),
            json!({ "name": "CI", "expires_in_days": 30 }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();

    for target in [&owner.user_id, &heir_id] {
        for (method, body) in [
            (Method::PATCH, Some(json!({ "role": "viewer" }))),
            (Method::DELETE, None),
        ] {
            let path = format!("{members}/{target}");
            let r = app
                .call(method.clone(), &path, body.clone(), Some(&lead))
                .await
                .expect_status(StatusCode::FORBIDDEN);
            assert_eq!(
                r.body["error"]["message"],
                "You can't change a member who holds rights you don't hold."
            );
            let r = app
                .with_key(method, &path, body, &key)
                .await
                .expect_status(StatusCode::FORBIDDEN);
            assert_eq!(
                r.body["error"]["message"],
                "You can't change a member who holds rights you don't hold."
            );
        }
        let still = app
            .get(&format!("{members}/{target}"), &owner.cookie)
            .await
            .expect_status(StatusCode::OK);
        assert_eq!(still.body["grant"]["role"]["id"], "admin");
    }

    app.call(
        Method::PATCH,
        &format!("{members}/{heir_id}"),
        Some(json!({ "role": "developer" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.with_key(
        Method::DELETE,
        &format!("{members}/{member_id}"),
        None,
        &key,
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    s.cleanup().await;
}

#[tokio::test]
async fn invitations_are_resent_only_by_people_who_could_send_them() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (admin, _) = join(&app, &owner, "admin@example.com", "admin").await;
    let invite = |email: &'static str, role: &'static str| {
        let app = &app;
        let cookie = owner.cookie.clone();
        let org = org.clone();
        async move {
            app.post(
                &format!("/orgs/{org}/invites"),
                json!({ "email": email, "role": role }),
                Some(&cookie),
            )
            .await
            .expect_status(StatusCode::CREATED)
            .body["id"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    let to_owner = invite("heir@example.com", "owner").await;
    let to_dev = invite("dev@example.com", "developer").await;
    app.wait_for_mail("heir@example.com", 1).await;

    let r = app
        .post(
            &format!("/orgs/{org}/invites/{to_owner}/resend"),
            json!({}),
            Some(&admin),
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["message"], "Only owners can invite owners.");
    app.post(
        &format!("/orgs/{org}/invites/{to_dev}/resend"),
        json!({}),
        Some(&admin),
    )
    .await
    .expect_status(StatusCode::OK);
    app.post(
        &format!("/orgs/{org}/invites/{to_owner}/resend"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.wait_for_mail("heir@example.com", 2).await;
    assert_eq!(app.mail.to("heir@example.com").len(), 2);
    s.cleanup().await;
}

#[tokio::test]
async fn project_rights_are_shown_only_for_project_members() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Research" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, outsider) = join(&app, &owner, "outsider@example.com", "developer").await;
    let (admin, admin_id) = join(&app, &owner, "admin@example.com", "admin").await;

    let r = app
        .get(
            &format!("/orgs/{org}/projects/{project}/members/{outsider}/rights"),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::NOT_FOUND);
    assert_eq!(
        r.body["error"]["message"],
        "No such project member, or you cannot see it."
    );
    app.get(
        &format!(
            "/orgs/{org}/projects/{project}/members/{}/rights",
            owner.user_id
        ),
        &owner.cookie,
    )
    .await
    .expect_status(StatusCode::OK);
    let mine = app
        .get(&format!("/orgs/{org}/projects/{project}/my-rights"), &admin)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(mine.body["user_id"], admin_id);
    assert!(mine.body["role"].is_null());
    assert!(!mine.body["rights"].as_array().unwrap().is_empty());
    s.cleanup().await;
}
