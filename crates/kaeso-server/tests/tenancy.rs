#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const PASSWORD: &str = "correct horse battery";

async fn join(
    app: &App,
    owner: &Owner,
    email: &str,
    preset: &str,
    roles: Value,
) -> (String, String) {
    let r = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": email, "preset": preset, "project_roles": roles }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let before = app.mail.to(email).len();
    app.wait_for_mail(email, before.max(1)).await;
    let token = app.last_token(email);
    let accepted = app
        .post(
            "/auth/invites/accept",
            json!({ "token": token, "name": email, "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.body);
    (
        accepted.cookie().unwrap(),
        accepted.body["user"]["id"].as_str().unwrap().to_owned(),
    )
}

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

#[tokio::test]
async fn invites_add_members_with_their_presets_and_project_roles() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Bot" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(project.status, StatusCode::CREATED);
    let project_id = project.body["id"].as_str().unwrap().to_owned();

    let (dev, dev_id) = join(
        &app,
        &owner,
        "dev@example.com",
        "developer",
        json!([{ "project_id": project_id, "preset": "admin" }]),
    )
    .await;
    let member = app
        .get(
            &format!("/orgs/{}/members/{dev_id}", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(member.body["grant"]["preset"], "developer");
    let mine = app.get("/me/memberships", &dev).await;
    let projects = mine.body["data"][0]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 2, "default project and the invited project");
    assert!(
        projects
            .iter()
            .any(|p| p["is_default"] == true && p["preset"] == "developer"),
        "{projects:?}"
    );
    assert!(
        projects
            .iter()
            .any(|p| p["id"] == project_id && p["preset"] == "admin")
    );

    let again = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": "dev@example.com", "preset": "member" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    let invite = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": "late@example.com", "preset": "member" }),
            Some(&owner.cookie),
        )
        .await;
    let invite_id = invite.body["id"].as_str().unwrap().to_owned();
    app.wait_for_mail("late@example.com", 1).await;
    let token = app.last_token("late@example.com");
    let revoke = app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/invites/{invite_id}", owner.org_id),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(revoke.status, StatusCode::NO_CONTENT);
    let r = app
        .post(
            "/auth/invites/accept",
            json!({ "token": token, "name": "Late", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    let invite = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": "slow@example.com", "preset": "member" }),
            Some(&owner.cookie),
        )
        .await;
    let invite_id = invite.body["id"].as_str().unwrap().to_owned();
    app.wait_for_mail("slow@example.com", 1).await;
    let old_token = app.last_token("slow@example.com");
    app.clock.advance(time::Duration::days(22));
    let owner_cookie = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let r = app
        .post("/auth/invites/lookup", json!({ "token": old_token }), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let resent = app
        .post(
            &format!("/orgs/{}/invites/{invite_id}/resend", owner.org_id),
            json!({}),
            Some(&owner_cookie),
        )
        .await;
    assert_eq!(
        (resent.status, resent.body["status"].as_str()),
        (StatusCode::OK, Some("pending")),
        "{:?}",
        resent.body
    );
    app.wait_for_mail("slow@example.com", 2).await;
    let lookup = app
        .post(
            "/auth/invites/lookup",
            json!({ "token": app.last_token("slow@example.com") }),
            None,
        )
        .await;
    assert_eq!(
        (
            lookup.body["org"]["name"].as_str(),
            lookup.body["account_exists"].as_bool()
        ),
        (Some("Acme"), Some(false))
    );
    s.cleanup().await;
}

#[tokio::test]
async fn rights_can_only_be_granted_by_holders_and_owners_stay() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (admin, admin_id) = join(&app, &owner, "admin@example.com", "admin", json!([])).await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let org = &owner.org_id;

    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "preset": "admin" })),
            Some(&dev),
        )
        .await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::FORBIDDEN, "permission_denied")
    );
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("members.rights.update")
    );
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "preset": "developer", "rights_added": ["audit_log.view"] })),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(
        r.body["grant"]["rights"]
            .as_array()
            .unwrap()
            .contains(&json!("audit_log.view"))
    );
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "preset": "owner" })),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "preset": "member", "rights_added": ["org.delete"] })),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("org.delete")
    );
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "preset": "member", "rights_added": ["fly"] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["error"]["param"], "rights_added");

    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{}", owner.user_id),
            Some(json!({ "preset": "admin" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = app
        .call(
            Method::DELETE,
            &format!("/me/memberships/{org}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = app
        .call(Method::DELETE, "/me", None, Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{org}/members/{}", owner.user_id),
            None,
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let r = app
        .post(
            &format!("/orgs/{org}/ownership-transfer"),
            json!({ "user_id": admin_id }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let me = app
        .get(
            &format!("/orgs/{org}/members/{}", owner.user_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(me.body["grant"]["preset"], "admin");
    app.wait_for_mail("admin@example.com", 2).await;
    assert!(
        app.mail
            .to("admin@example.com")
            .iter()
            .any(|m| m.kind.as_str() == "ownership_transferred")
    );
    s.cleanup().await;
}

#[tokio::test]
async fn removing_a_member_archives_their_keys_and_ends_sessions_when_no_org_is_left() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let project = default_project(&app, &owner).await;
    let dev_uuid = dev_id.parse::<kaeso_core::id::UserId>().unwrap().uuid();
    let org_uuid = owner
        .org_id
        .parse::<kaeso_core::id::OrgId>()
        .unwrap()
        .uuid();
    let project_uuid = project.parse::<kaeso_core::id::ProjectId>().unwrap().uuid();
    sqlx::query(
        "INSERT INTO api_keys (id, org_id, project_id, user_id, name, secret_hash, secret_prefix, secret_last4, inference, platform, created_by_kind) \
         VALUES (gen_random_uuid(), $1, $2, $3, 'k', '\\x01', 'abc', 'wxyz', true, false, 'user')",
    )
    .bind(org_uuid)
    .bind(project_uuid)
    .bind(dev_uuid)
    .execute(&app.state.pg)
    .await
    .unwrap();
    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/members/{dev_id}", owner.org_id),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let (archived,): (bool,) =
        sqlx::query_as("SELECT archived_at IS NOT NULL FROM api_keys WHERE user_id = $1")
            .bind(dev_uuid)
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert!(archived);
    assert_eq!(
        app.get("/auth/session", &dev).await.status,
        StatusCode::UNAUTHORIZED,
        "removal from every org ends every session"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn projects_are_created_listed_by_visibility_and_archived() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (dev, _) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let (billing, _) = join(&app, &owner, "billing@example.com", "billing", json!([])).await;
    let (member, _) = join(&app, &owner, "member@example.com", "member", json!([])).await;

    let created = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Dev project" }),
            Some(&dev),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let dev_project = created.body["id"].as_str().unwrap().to_owned();
    let mine = app
        .get(&format!("/orgs/{org}/projects/{dev_project}/members"), &dev)
        .await;
    assert_eq!(mine.body["data"][0]["grant"]["preset"], "admin");
    assert_eq!(
        app.post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "x" }),
            Some(&member)
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );

    let names = |r: &common::Resp| {
        r.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&app.get(&format!("/orgs/{org}/projects"), &member).await),
        ["Default project"]
    );
    assert_eq!(
        names(&app.get(&format!("/orgs/{org}/projects"), &billing).await).len(),
        2
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}/projects/{dev_project}"), &billing)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}/projects/{dev_project}"), &member)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    app.call(
        Method::PUT,
        &format!("/orgs/{org}/security/key-policy"),
        Some(json!({ "max_key_lifetime_days": 30 })),
        Some(&owner.cookie),
    )
    .await;
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{org}/projects/{dev_project}/key-policy"),
            Some(json!({ "max_key_lifetime_days": 90 })),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let default = default_project(&app, &owner).await;
    let r = app
        .post(
            &format!("/orgs/{org}/projects/{default}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = app
        .post(
            &format!("/orgs/{org}/projects/{dev_project}/archive"),
            json!({}),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/projects/{dev_project}"),
            Some(json!({ "name": "again" })),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        names(
            &app.get(&format!("/orgs/{org}/projects"), &owner.cookie)
                .await
        ),
        ["Default project"]
    );
    s.cleanup().await;
}

#[tokio::test]
async fn service_accounts_get_only_rights_the_creator_holds() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}/service-accounts");
    let r = app
        .post(
            &base,
            json!({ "name": "ci", "preset": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let id = r.body["id"].as_str().unwrap().to_owned();
    let (dev, _) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    assert_eq!(
        app.post(
            &base,
            json!({ "name": "x", "preset": "viewer" }),
            Some(&dev)
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let dev_id = app
        .get(&format!("/orgs/{org}/members"), &owner.cookie)
        .await
        .body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user"]["email"] == "dev@example.com")
        .unwrap()["user"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/projects/{project}/members/{dev_id}"),
            Some(json!({ "preset": "admin", "rights_removed": ["logs.export"] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .post(&base, json!({ "name": "x", "preset": "admin" }), Some(&dev))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("logs.export")
    );
    let r = app
        .post(
            &base,
            json!({ "name": "y", "preset": "admin", "rights_removed": ["logs.export"] }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = app
        .post(
            &format!("{base}/{id}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert!(r.body["archived_at"].is_string());
    assert_eq!(
        app.get(&base, &owner.cookie).await.body["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    s.cleanup().await;
}

#[tokio::test]
async fn every_change_is_audited_and_the_log_can_be_filtered_and_exported() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    app.post(
        &format!("/orgs/{org}/projects"),
        json!({ "name": "One" }),
        Some(&owner.cookie),
    )
    .await;
    app.call(
        Method::PATCH,
        &format!("/orgs/{org}"),
        Some(json!({ "name": "Acme Inc." })),
        Some(&owner.cookie),
    )
    .await;
    let log = app
        .get(&format!("/orgs/{org}/audit-log"), &owner.cookie)
        .await;
    let actions: Vec<&str> = log.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        ["org.update", "project.create", "org.create"],
        "newest first"
    );
    assert_eq!(log.body["data"][0]["actor"]["id"], owner.user_id);
    assert_eq!(log.body["data"][0]["before"]["name"], "Acme");
    let filtered = app
        .get(
            &format!("/orgs/{org}/audit-log?action=project."),
            &owner.cookie,
        )
        .await;
    assert_eq!(filtered.body["data"].as_array().unwrap().len(), 1);
    let page = app
        .get(&format!("/orgs/{org}/audit-log?limit=1"), &owner.cookie)
        .await;
    assert_eq!(page.body["has_more"], true);
    let next = app
        .get(
            &format!(
                "/orgs/{org}/audit-log?limit=1&after={}",
                page.body["next_cursor"].as_str().unwrap()
            ),
            &owner.cookie,
        )
        .await;
    assert_eq!(next.body["data"][0]["action"], "project.create");
    let req = axum::http::Request::get(format!("/platform/v1/orgs/{org}/audit-log/export"))
        .header("cookie", format!("__Host-kaeso_session={}", owner.cookie))
        .body(axum::body::Body::empty())
        .unwrap();
    let res = tower::ServiceExt::oneshot(app.router.clone(), req)
        .await
        .unwrap();
    assert_eq!(res.headers()["content-type"], "application/x-ndjson");
    let text = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0]["action"], "audit_log.export");
    assert!(
        sqlx::query("UPDATE audit_events SET action = 'x'")
            .execute(&app.state.pg)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_events")
            .execute(&app.state.pg)
            .await
            .is_err()
    );
    let instance = app.get("/instance/audit-log", &owner.cookie).await;
    assert!(
        instance.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["org_id"].is_null())
    );
    s.cleanup().await;
}

#[tokio::test]
async fn deleting_an_org_waits_its_grace_period_and_can_be_undone() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (_admin, _) = join(&app, &owner, "admin@example.com", "admin", json!([])).await;
    app.clock.advance(time::Duration::minutes(20));
    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{org}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.error_type(), "recent_sign_in_required");
    let cookie = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let r = app
        .call(Method::DELETE, &format!("/orgs/{org}"), None, Some(&cookie))
        .await;
    assert_eq!(
        (r.status, r.body["state"].as_str()),
        (StatusCode::OK, Some("scheduled_for_deletion")),
        "{:?}",
        r.body
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}/members"), &cookie)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.call(
            Method::PATCH,
            &format!("/orgs/{org}"),
            Some(json!({ "name": "Renamed" })),
            Some(&cookie)
        )
        .await
        .error_type(),
        "org_scheduled_for_deletion"
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}"), &cookie).await.status,
        StatusCode::OK
    );
    app.wait_for_mail("admin@example.com", 2).await;
    assert!(
        app.mail
            .to("admin@example.com")
            .iter()
            .any(|m| m.kind.as_str() == "org_scheduled_for_deletion")
    );
    let r = app
        .post(&format!("/orgs/{org}/restore"), json!({}), Some(&cookie))
        .await;
    assert_eq!(r.body["state"], "active");
    assert_eq!(
        app.get(&format!("/orgs/{org}/members"), &cookie)
            .await
            .status,
        StatusCode::OK
    );

    let picture = |bytes: &[u8], kind: &str| {
        use base64::Engine;
        json!({ "content_type": kind, "data": base64::engine::general_purpose::STANDARD.encode(bytes) })
    };
    let default = default_project(&app, &owner).await;
    for (path, body) in [
        (
            format!("/orgs/{org}/avatar"),
            picture(b"\x89PNG\r\n\x1a\n-org", "image/png"),
        ),
        (
            format!("/orgs/{org}/projects/{default}/avatar"),
            picture(&[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3], "image/jpeg"),
        ),
    ] {
        let r = app
            .call(Method::PUT, &path, Some(body), Some(&cookie))
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }

    app.call(Method::DELETE, &format!("/orgs/{org}"), None, Some(&cookie))
        .await;
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "org_deletion")
        .unwrap();
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "deleted 0 organizations"
    );
    app.clock.advance(time::Duration::days(8));
    s.clickhouse.migrate(&s.pg).await.unwrap();
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "deleted 1 organizations"
    );
    let (orgs, events): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM organizations), (SELECT count(*) FROM audit_events WHERE org_id IS NOT NULL)",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!((orgs, events), (0, 0));
    let (pictures,): (i64,) = sqlx::query_as("SELECT count(*) FROM avatars")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(pictures, 0, "the org's and its projects' pictures are gone");
    let (finals,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM audit_events WHERE action = 'org.delete_final'")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(finals, 1);
    s.cleanup().await;
}

#[tokio::test]
async fn instance_admins_cannot_read_org_content_unless_appointed() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[("KAESO_INSTANCE_ORG_CREATION", "everyone")]).await;
    let admin = app.setup().await;
    app.post(
        "/instance/users/invites",
        json!({ "email": "bob@example.com" }),
        Some(&admin.cookie),
    )
    .await;
    app.wait_for_mail("bob@example.com", 1).await;
    let bob = app.post("/auth/invites/accept", json!({ "token": app.last_token("bob@example.com"), "name": "Bob", "password": PASSWORD }), None).await.cookie().unwrap();
    let created = app
        .post("/me/orgs", json!({ "name": "Bob's org" }), Some(&bob))
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let bob_org = created.body["id"].as_str().unwrap().to_owned();
    assert_eq!(
        app.get(&format!("/instance/orgs/{bob_org}"), &admin.cookie)
            .await
            .body["members"],
        1
    );
    assert_eq!(
        app.get(&format!("/orgs/{bob_org}/members"), &admin.cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&format!("/orgs/{bob_org}/audit-log"), &admin.cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/instance/users", &bob).await.status,
        StatusCode::FORBIDDEN
    );
    let admin_id = admin.user_id.clone();
    let r = app
        .post(
            &format!("/instance/orgs/{bob_org}/owner"),
            json!({ "user_id": admin_id }),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["owners"], 2);
    let log = app
        .get(&format!("/orgs/{bob_org}/audit-log"), &admin.cookie)
        .await;
    assert_eq!(
        log.body["data"][0]["action"],
        "org.owner_appointed_by_instance_admin"
    );
    let again = app
        .post(
            &format!("/instance/orgs/{bob_org}/owner"),
            json!({ "user_id": admin_id }),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body["owners"], 2);
    let r = app
        .call(
            Method::PATCH,
            &format!("/instance/orgs/{bob_org}"),
            Some(json!({ "name": "Bob Labs" })),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["name"], "Bob Labs");
    let log = app
        .get(&format!("/orgs/{bob_org}/audit-log"), &admin.cookie)
        .await;
    assert_eq!(log.body["data"][0]["action"], "org.update");
    let r = app
        .call(
            Method::PATCH,
            &format!("/instance/orgs/{bob_org}"),
            Some(json!({ "name": "" })),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = app
        .post(
            &format!("/instance/orgs/{bob_org}/schedule-deletion"),
            json!({}),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["state"], "scheduled_for_deletion");
    assert!(r.body["deletion_due_at"].is_string());
    let r = app
        .post(
            &format!("/instance/orgs/{bob_org}/schedule-deletion"),
            json!({}),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = app
        .post(
            &format!("/instance/orgs/{bob_org}/owner"),
            json!({ "user_id": admin_id }),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = app
        .post(
            &format!("/instance/orgs/{bob_org}/restore"),
            json!({}),
            Some(&admin.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &format!("/instance/orgs/{bob_org}"),
            Some(json!({ "name": "Mine" })),
            Some(&bob),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    s.cleanup().await;
}

#[tokio::test]
async fn org_creation_follows_the_instance_policy() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (dev, _) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let can_create = async |cookie: &str| {
        let r = app.get("/auth/session", cookie).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body["can_create_orgs"].as_bool()
    };
    assert_eq!(can_create(&dev).await, Some(false));
    assert_eq!(can_create(&owner.cookie).await, Some(true));
    assert_eq!(
        app.post("/me/orgs", json!({ "name": "Mine" }), Some(&dev))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "org_creation": "everyone" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.body["org_creation"].as_str()),
        (StatusCode::OK, Some("everyone"))
    );
    assert_eq!(can_create(&dev).await, Some(true));
    assert_eq!(can_create(&owner.cookie).await, Some(true));
    assert_eq!(
        app.post("/me/orgs", json!({ "name": "Mine" }), Some(&dev))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(
        app.get("/me/memberships", &dev).await.body["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "content_retention_days": 400 })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    s.cleanup().await;
}

#[tokio::test]
async fn deactivation_disables_keys_and_reactivation_restores_them() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let project = default_project(&app, &owner).await;
    let dev_uuid = dev_id.parse::<kaeso_core::id::UserId>().unwrap().uuid();
    for (name, disabled) in [("on", false), ("off", true)] {
        sqlx::query(
            "INSERT INTO api_keys (id, org_id, project_id, user_id, name, secret_hash, secret_prefix, secret_last4, inference, platform, created_by_kind, disabled_at, disabled_reason) \
             VALUES (gen_random_uuid(), $1, $2, $3, $4, gen_random_uuid()::text::bytea, 'abc', 'wxyz', true, false, 'user', \
             CASE WHEN $5 THEN now() END, CASE WHEN $5 THEN 'manual' END)",
        )
        .bind(owner.org_id.parse::<kaeso_core::id::OrgId>().unwrap().uuid())
        .bind(project.parse::<kaeso_core::id::ProjectId>().unwrap().uuid())
        .bind(dev_uuid)
        .bind(name)
        .bind(disabled)
        .execute(&app.state.pg)
        .await
        .unwrap();
    }
    let r = app
        .post(
            &format!("/instance/users/{dev_id}/deactivate"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.body["state"].as_str()),
        (StatusCode::OK, Some("deactivated")),
        "{:?}",
        r.body
    );
    assert_eq!(
        app.get("/auth/session", &dev).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.post(
            "/auth/password",
            json!({ "email": "dev@example.com", "password": PASSWORD }),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let rows: Vec<(String, bool)> =
        sqlx::query_as("SELECT name, disabled_at IS NOT NULL FROM api_keys ORDER BY name")
            .fetch_all(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(rows, [("off".to_owned(), true), ("on".to_owned(), true)]);
    app.post(
        &format!("/instance/users/{dev_id}/reactivate"),
        json!({}),
        Some(&owner.cookie),
    )
    .await;
    let rows: Vec<(String, bool)> =
        sqlx::query_as("SELECT name, disabled_at IS NOT NULL FROM api_keys ORDER BY name")
            .fetch_all(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [("off".to_owned(), true), ("on".to_owned(), false)],
        "only the keys deactivation disabled come back"
    );
    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/users/{dev_id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let u = app
        .get(&format!("/instance/users/{dev_id}"), &owner.cookie)
        .await;
    assert_eq!(
        (
            u.body["state"].as_str(),
            u.body["email"].as_str(),
            u.body["name"].as_str()
        ),
        (Some("deleted"), None, Some("Deleted user"))
    );
    let r = app
        .call(
            Method::PUT,
            &format!("/instance/users/{}/instance-admin", owner.user_id),
            Some(json!({ "is_instance_admin": false })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    s.cleanup().await;
}

#[tokio::test]
async fn orgs_requiring_two_factor_close_their_routes_to_password_sessions_without_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/require-2fa", owner.org_id),
            Some(json!({ "enabled": true })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let password = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": PASSWORD }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let r = app.get(&format!("/orgs/{}", owner.org_id), &password).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("two-factor")
    );
    assert_eq!(
        app.post("/me/totp", json!({}), Some(&password))
            .await
            .status,
        StatusCode::OK
    );
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/require-sso", owner.org_id),
            Some(json!({ "enabled": true })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    s.cleanup().await;
}

async fn defaults(app: &App, owner: &Owner) -> Vec<(String, bool, bool)> {
    app.get(
        &format!("/orgs/{}/projects?include_archived=true", owner.org_id),
        &owner.cookie,
    )
    .await
    .body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap().to_owned(),
                p["is_default"] == true,
                !p["archived_at"].is_null(),
            )
        })
        .collect()
}

async fn make_default(app: &App, org: &str, project: &str, cookie: &str) -> common::Resp {
    app.call(
        Method::PUT,
        &format!("/orgs/{org}/default-project"),
        Some(json!({ "project_id": project })),
        Some(cookie),
    )
    .await
}

#[tokio::test]
async fn org_admins_choose_the_default_project_and_members_follow() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let first = default_project(&app, &owner).await;
    let (dev, _) = join(&app, &owner, "dev@example.com", "developer", json!([])).await;
    let (member, _) = join(&app, &owner, "member@example.com", "member", json!([])).await;
    let created = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Production" }),
            Some(&dev),
        )
        .await;
    let next = created.body["id"].as_str().unwrap().to_owned();

    let r = make_default(&app, &org, &next, &dev).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        make_default(&app, &org, &next, &member).await.status,
        StatusCode::FORBIDDEN
    );

    let r = make_default(&app, &org, &next, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["id"], next.as_str());
    assert_eq!(r.body["is_default"], true);
    let now = defaults(&app, &owner).await;
    assert_eq!(
        now.iter()
            .filter(|(_, d, _)| *d)
            .map(|(id, _, _)| id.as_str())
            .collect::<Vec<_>>(),
        [next.as_str()]
    );
    let again = make_default(&app, &org, &next, &owner.cookie).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body["is_default"], true);

    let mine = app.get("/me/memberships", &member).await;
    let projects = mine.body["data"][0]["projects"].as_array().unwrap();
    assert!(
        projects.iter().any(|p| p["id"] == next.as_str()
            && p["is_default"] == true
            && p["preset"] == "viewer"),
        "{projects:?}"
    );
    assert!(
        projects
            .iter()
            .any(|p| p["id"] == first.as_str() && p["is_default"] == false),
        "{projects:?}"
    );
    let mine = app.get("/me/memberships", &dev).await;
    assert!(
        mine.body["data"][0]["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == next.as_str() && p["preset"] == "admin"),
        "the creator keeps the admin preset: {:?}",
        mine.body
    );
    let (billing, _) = join(&app, &owner, "billing@example.com", "billing", json!([])).await;
    let mine = app.get("/me/memberships", &billing).await;
    let projects = mine.body["data"][0]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1, "{projects:?}");
    assert_eq!(projects[0]["id"], next.as_str());

    let audit = app
        .get(
            &format!("/orgs/{org}/audit-log?action=project.default"),
            &owner.cookie,
        )
        .await;
    let events = audit.body["data"].as_array().unwrap();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["before"]["default_project"], first.as_str());
    assert_eq!(events[0]["after"]["default_project"], next.as_str());

    let r = app
        .post(
            &format!("/orgs/{org}/projects/{next}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("default"),
        "{:?}",
        r.body
    );
    let r = app
        .post(
            &format!("/orgs/{org}/projects/{first}/archive"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = make_default(&app, &org, &first, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = make_default(
        &app,
        &org,
        &kaeso_core::id::ProjectId::new().to_string(),
        &owner.cookie,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    let other = app
        .post(
            "/me/orgs",
            json!({ "name": "Other org" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.body);
    let other_org = other.body["id"].as_str().unwrap().to_owned();
    let r = make_default(&app, &other_org, &next, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn the_org_always_has_exactly_one_default_project() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let mut ids = Vec::new();
    for n in 0..6 {
        let r = app
            .post(
                &format!("/orgs/{org}/projects"),
                json!({ "name": format!("Project {n}") }),
                Some(&owner.cookie),
            )
            .await;
        ids.push(r.body["id"].as_str().unwrap().to_owned());
    }

    for _ in 0..3 {
        let calls = ids
            .iter()
            .map(|id| make_default(&app, &org, id, &owner.cookie));
        for r in futures::future::join_all(calls).await {
            assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        }
        let now = defaults(&app, &owner).await;
        assert_eq!(now.iter().filter(|(_, d, _)| *d).count(), 1, "{now:?}");
    }

    for id in &ids {
        let current = defaults(&app, &owner).await;
        if current.iter().any(|(p, d, a)| p == id && (*d || *a)) {
            continue;
        }
        let path = format!("/orgs/{org}/projects/{id}/archive");
        let archive = app.post(&path, json!({}), Some(&owner.cookie));
        let promote = make_default(&app, &org, id, &owner.cookie);
        let (archived, promoted) = tokio::join!(archive, promote);
        assert!(
            (archived.status == StatusCode::OK) != (promoted.status == StatusCode::OK),
            "archive {:?}, default {:?}",
            archived.body,
            promoted.body
        );
        let now = defaults(&app, &owner).await;
        assert_eq!(now.iter().filter(|(_, d, _)| *d).count(), 1, "{now:?}");
        assert!(!now.iter().any(|(_, d, a)| *d && *a), "{now:?}");
    }
    s.cleanup().await;
}
