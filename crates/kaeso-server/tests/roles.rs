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
    role: &str,
    projects: Value,
) -> (String, String) {
    let r = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": email, "role": role, "project_roles": projects }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    app.wait_for_mail(email, 1).await;
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

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn held(rights: &Value, right: &str) -> Option<Value> {
    rights
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["right"] == right)
        .map(|r| r["sources"].clone())
}

#[tokio::test]
async fn the_catalogue_and_the_built_in_roles_are_served() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = &owner.org_id;
    let cat = app.get(&format!("/orgs/{org}/rights"), &owner.cookie).await;
    assert_eq!(cat.status, StatusCode::OK, "{:?}", cat.body);
    let members = cat.body["org"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "members")
        .unwrap();
    assert_eq!(members["name"], "Members & access");
    assert!(
        !members["rights"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "members.2fa.reset")
    );
    let remove = members["rights"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "members.remove")
        .unwrap();
    assert_eq!(remove["needs"], json!(["org.view", "members.view"]));
    assert_eq!(remove["sensitive"], true);
    let project_ids: Vec<&str> = cat.body["project"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        project_ids,
        [
            "project",
            "members",
            "model_use",
            "keys",
            "models",
            "limits",
            "logs",
            "usage",
            "audit"
        ]
    );

    let roles = app.get(&format!("/orgs/{org}/roles"), &owner.cookie).await;
    assert_eq!(roles.status, StatusCode::OK, "{:?}", roles.body);
    let ids: Vec<(String, String)> = roles.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["scope"].as_str().unwrap().into(),
                r["id"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        ids,
        [
            ("org", "owner"),
            ("org", "admin"),
            ("org", "billing"),
            ("org", "developer"),
            ("org", "auditor"),
            ("org", "member"),
            ("project", "admin"),
            ("project", "developer"),
            ("project", "viewer"),
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    let owner_role = &roles.body["data"][0];
    assert_eq!(owner_role["holders"], 1);
    assert_eq!(owner_role["kind"], "builtin");
    let auditor = app
        .get(&format!("/orgs/{org}/roles/auditor"), &owner.cookie)
        .await;
    assert!(names(&auditor.body["project_rights"]).contains(&"project.audit_log.view".into()));
    assert!(!names(&auditor.body["project_rights"]).contains(&"logs.content.view".into()));
    let viewer = app
        .get(
            &format!("/orgs/{org}/roles/viewer?scope=project"),
            &owner.cookie,
        )
        .await;
    assert_eq!(viewer.body["scope"], "project");
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/roles/admin"),
            Some(json!({ "name": "Boss" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);

    let who = app.get("/auth/session", &owner.cookie).await;
    let m = &who.body["memberships"][0];
    assert_eq!(
        m["role"],
        json!({ "id": "owner", "kind": "builtin", "name": "Owner" })
    );
    assert_eq!(m["preset"], "owner");
    assert!(names(&m["project_rights"]).contains(&"project.audit_log.view".into()));
}

#[tokio::test]
async fn custom_roles_are_created_assigned_changed_and_deleted() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let (dev, dev_id) = join(&app, &owner, "dev@example.com", "member", json!([])).await;

    let r = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Reader", "rights": ["org.view"], "project_rights": ["project.view", "models.manage"] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "`models.manage` needs `models.view`."
    );
    let r = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Admin", "rights": ["org.view"] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let created = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({
                "scope": "org",
                "name": "Support lead",
                "description": "Reads usage and logs everywhere.",
                "rights": ["org.view", "members.view", "usage.org.view"],
                "project_rights": ["project.view", "logs.metadata.view", "usage.view"],
            }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let role = created.body["id"].as_str().unwrap().to_owned();
    assert!(role.starts_with("role_"));
    assert_eq!(created.body["kind"], "custom");
    assert_eq!(created.body["holders"], 0);
    let dup = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "support LEAD", "rights": ["org.view"] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT, "{:?}", dup.body);

    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "role": role })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["grant"]["role"]["name"], "Support lead");
    assert_eq!(r.body["grant"]["role"]["kind"], "custom");
    assert_eq!(r.body["grant"]["preset"], Value::Null);
    let who = app.get("/auth/session", &dev).await;
    let m = &who.body["memberships"][0];
    assert_eq!(m["role"]["id"], role.as_str());
    assert_eq!(
        names(&m["project_rights"]),
        ["project.view", "logs.metadata.view", "usage.view"]
    );
    let list = app
        .get(&format!("/orgs/{org}/members?role={role}"), &owner.cookie)
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);

    let mine = app
        .get(&format!("/orgs/{org}/projects/{project}/my-rights"), &dev)
        .await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.body);
    assert_eq!(mine.body["role"]["id"], "viewer");
    let sources = held(&mine.body["rights"], "logs.metadata.view").unwrap();
    let kinds: Vec<&str> = sources
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["project_role", "org_role"]);
    assert_eq!(sources[1]["role"]["name"], "Support lead");

    let rights = app
        .get(
            &format!("/orgs/{org}/members/{dev_id}/rights"),
            &owner.cookie,
        )
        .await;
    assert_eq!(rights.status, StatusCode::OK, "{:?}", rights.body);
    assert_eq!(rights.body["org"]["role"]["name"], "Support lead");
    assert_eq!(
        held(&rights.body["org"]["rights"], "usage.org.view").unwrap()[0]["kind"],
        "role"
    );
    assert_eq!(
        rights.body["projects"][0]["project"]["id"],
        project.as_str()
    );

    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/roles/{role}"),
            Some(json!({ "project_rights": ["project.view", "logs.metadata.view", "usage.view", "costs.view"] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["holders"], 1);
    let mine = app
        .get(&format!("/orgs/{org}/projects/{project}/my-rights"), &dev)
        .await;
    assert!(held(&mine.body["rights"], "costs.view").is_some());

    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{org}/roles/{role}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{dev_id}"),
            Some(json!({ "role": "member" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{org}/roles/{role}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);

    let log = app
        .get(
            &format!("/orgs/{org}/audit-log?action=role."),
            &owner.cookie,
        )
        .await;
    let actions: Vec<&str> = log.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["role.delete", "role.update", "role.create"]);
}

#[tokio::test]
async fn custom_project_roles_work_for_members_service_accounts_and_invites() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let created = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({
                "scope": "project",
                "name": "Log reader",
                "rights": ["project.view", "logs.metadata.view", "logs.content.view"],
            }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let role = created.body["id"].as_str().unwrap().to_owned();
    let org_role = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Viewer plus", "rights": ["org.view"] }),
            Some(&owner.cookie),
        )
        .await;
    let org_role = org_role.body["id"].as_str().unwrap().to_owned();
    let other = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Bot" }),
            Some(&owner.cookie),
        )
        .await;
    let other = other.body["id"].as_str().unwrap().to_owned();
    let r = app
        .post(
            &format!("/orgs/{org}/invites"),
            json!({ "email": "x@example.com", "role": "member", "project_roles": [{ "project_id": other, "role": org_role }] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let (reader, reader_id) = join(
        &app,
        &owner,
        "reader@example.com",
        &org_role,
        json!([{ "project_id": other, "role": role }]),
    )
    .await;
    let membership = app
        .get(
            &format!("/orgs/{org}/projects/{other}/members/{reader_id}"),
            &owner.cookie,
        )
        .await;
    assert_eq!(membership.body["grant"]["role"]["name"], "Log reader");
    let mine = app
        .get(&format!("/orgs/{org}/projects/{other}/my-rights"), &reader)
        .await;
    assert_eq!(
        mine.body["rights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["right"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["project.view", "logs.metadata.view", "logs.content.view"]
    );
    let me = app.get("/me/memberships", &reader).await;
    let projects = me.body["data"][0]["projects"].as_array().unwrap();
    assert!(
        projects
            .iter()
            .any(|p| p["id"] == project.as_str() && p["role"]["id"] == "viewer")
    );

    let sa = app
        .post(
            &format!("/orgs/{org}/projects/{project}/service-accounts"),
            json!({ "name": "log-shipper", "role": role }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(sa.status, StatusCode::CREATED, "{:?}", sa.body);
    assert_eq!(sa.body["grant"]["role"]["id"], role.as_str());
    assert_eq!(
        names(&sa.body["grant"]["rights"]),
        ["project.view", "logs.metadata.view", "logs.content.view"]
    );
    let listed = app
        .get(&format!("/orgs/{org}/roles?scope=project"), &owner.cookie)
        .await;
    let custom = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == role.as_str())
        .unwrap();
    assert_eq!(custom["holders"], 2);

    let log = app
        .get(
            &format!("/orgs/{org}/projects/{project}/audit-log"),
            &owner.cookie,
        )
        .await;
    assert_eq!(log.status, StatusCode::OK, "{:?}", log.body);
    let first = &log.body["data"][0];
    assert_eq!(first["action"], "service_account.create");
    assert_eq!(first["project_id"], project.as_str());
    let denied = app
        .get(
            &format!("/orgs/{org}/projects/{project}/audit-log"),
            &reader,
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN, "{:?}", denied.body);
    assert_eq!(denied.body["error"]["right"], "project.audit_log.view");
    let grantors = app
        .get(
            &format!("/orgs/{org}/projects/{project}/rights/project.audit_log.view/grantors"),
            &reader,
        )
        .await;
    assert_eq!(grantors.status, StatusCode::OK, "{:?}", grantors.body);
    assert_eq!(grantors.body["data"][0]["email"], "owner@example.com");
    let org_grantors = app
        .get(
            &format!("/orgs/{org}/rights/audit_log.view/grantors"),
            &owner.cookie,
        )
        .await;
    assert_eq!(org_grantors.body["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn nobody_grants_what_they_lack_or_changes_someone_above_them() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (_admin, admin_id) = join(&app, &owner, "admin@example.com", "admin", json!([])).await;
    let (_member, member_id) = join(&app, &owner, "member@example.com", "member", json!([])).await;
    let manager_role = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({
                "scope": "org",
                "name": "People manager",
                "rights": ["org.view", "members.view", "members.rights.update", "members.remove", "roles.manage"],
            }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        manager_role.status,
        StatusCode::CREATED,
        "{:?}",
        manager_role.body
    );
    let manager_role = manager_role.body["id"].as_str().unwrap().to_owned();
    let (manager, _) = join(&app, &owner, "people@example.com", &manager_role, json!([])).await;

    let r = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Auditing", "rights": ["org.view", "audit_log.view"] }),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "You can't grant `audit_log.view`: you don't hold it yourself."
    );
    let r = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "project", "name": "Anything", "rights": ["project.view"] }),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{member_id}"),
            Some(json!({ "role": "developer" })),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/members/{member_id}"),
            Some(json!({ "role": "member", "rights_added": ["members.remove"] })),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    for (method, body) in [
        (Method::PATCH, Some(json!({ "role": "member" }))),
        (Method::DELETE, None),
    ] {
        let r = app
            .call(
                method,
                &format!("/orgs/{org}/members/{admin_id}"),
                body,
                Some(&manager),
            )
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
        assert_eq!(
            r.body["error"]["message"],
            "You can't change a member who holds rights you don't hold."
        );
    }
    let auditing = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Auditing", "rights": ["org.view", "audit_log.view"] }),
            Some(&owner.cookie),
        )
        .await;
    let auditing = auditing.body["id"].as_str().unwrap().to_owned();
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/roles/{auditing}"),
            Some(json!({ "name": "Renamed" })),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let r = app
        .post(
            &format!("/orgs/{org}/invites"),
            json!({ "email": "o2@example.com", "role": "owner" }),
            Some(&manager),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
}

#[tokio::test]
async fn auditors_read_everything_and_change_nothing() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let other = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Bot" }),
            Some(&owner.cookie),
        )
        .await;
    let other = other.body["id"].as_str().unwrap().to_owned();
    let (auditor, _) = join(&app, &owner, "audit@example.com", "auditor", json!([])).await;
    let projects = app.get(&format!("/orgs/{org}/projects"), &auditor).await;
    assert_eq!(projects.body["data"].as_array().unwrap().len(), 2);
    for path in [
        format!("/orgs/{org}/audit-log"),
        format!("/orgs/{org}/members"),
        format!("/orgs/{org}/projects/{other}/audit-log"),
        format!("/orgs/{org}/projects/{other}/members"),
        format!("/orgs/{org}/projects/{other}/keys"),
    ] {
        let r = app.get(&path, &auditor).await;
        assert_eq!(r.status, StatusCode::OK, "{path}: {:?}", r.body);
    }
    let mine = app
        .get(&format!("/orgs/{org}/projects/{other}/my-rights"), &auditor)
        .await;
    assert_eq!(mine.body["role"], Value::Null);
    assert!(held(&mine.body["rights"], "logs.content.view").is_none());
    assert_eq!(
        held(&mine.body["rights"], "keys.all.view").unwrap()[0]["kind"],
        "org_role"
    );
    let r = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/projects/{other}"),
            Some(json!({ "name": "Renamed" })),
            Some(&auditor),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(r.body["error"]["right"], "project.update");
}
