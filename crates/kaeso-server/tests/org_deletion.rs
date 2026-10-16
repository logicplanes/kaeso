#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_core::clock::Clock;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const PASSWORD: &str = "correct horse battery";

async fn join(app: &App, owner: &Owner, email: &str, preset: &str) -> String {
    let r = app
        .post(
            &format!("/orgs/{}/invites", owner.org_id),
            json!({ "email": email, "preset": preset }),
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
    accepted.cookie().unwrap()
}

async fn sign_in(app: &App, email: &str) -> String {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await
    .cookie()
    .unwrap()
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

fn due(body: &Value) -> OffsetDateTime {
    OffsetDateTime::parse(body["deletion_due_at"].as_str().unwrap(), &Rfc3339).unwrap()
}

async fn schedule(app: &App, org: &str, cookie: &str) -> Value {
    let r = app
        .call(Method::DELETE, &format!("/orgs/{org}"), None, Some(cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

fn read_only(r: &common::Resp) {
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::CONFLICT, "org_scheduled_for_deletion"),
        "{:?}",
        r.body
    );
    assert_eq!(
        r.body["error"]["message"],
        "This organization is scheduled for deletion; nothing in it can be changed."
    );
}

#[tokio::test]
async fn a_scheduled_org_stays_readable_refuses_changes_and_can_be_restored() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let dev = join(&app, &owner, "developer@example.com", "developer").await;
    let key = app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": "ci" }),
            Some(&dev),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let key_id = key.body["id"].as_str().unwrap().to_owned();
    let secret = key.body["secret"].as_str().unwrap().to_owned();

    let cookie = sign_in(&app, "owner@example.com").await;
    let scheduled = schedule(&app, &org, &cookie).await;
    assert_eq!(scheduled["state"], "scheduled_for_deletion");
    assert_eq!(
        due(&scheduled),
        app.clock.now() + time::Duration::days(7),
        "the default grace period is 7 days"
    );

    for who in [&cookie, &dev] {
        for path in [
            format!("/orgs/{org}"),
            format!("/orgs/{org}/members"),
            format!("/orgs/{org}/projects"),
            format!("/orgs/{org}/projects/{project}"),
            format!("/orgs/{org}/projects/{project}/keys"),
            format!("/orgs/{org}/projects/{project}/models"),
            format!("/orgs/{org}/logs/settings"),
            format!("/orgs/{org}/audit-log"),
        ] {
            let r = app.get(&path, who).await;
            if r.status == StatusCode::FORBIDDEN {
                continue;
            }
            assert_eq!(r.status, StatusCode::OK, "{path}: {:?}", r.body);
        }
    }
    let who = app.get("/auth/session", &dev).await;
    let membership = &who.body["memberships"][0];
    assert_eq!(membership["org_state"], "scheduled_for_deletion");
    assert_eq!(
        membership["org_deletion_due_at"],
        scheduled["deletion_due_at"]
    );
    assert_eq!(
        who.body["org_deletion"],
        json!({ "allowed": true, "grace_days": 7 })
    );
    let rights = |body: &Value| -> Vec<String> {
        body["memberships"][0]["rights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().to_owned())
            .collect()
    };
    let dev_rights = rights(&who.body);
    assert!(!dev_rights.is_empty());
    assert!(
        dev_rights
            .iter()
            .all(|r| r.contains(".view") || r.ends_with(".export")),
        "{dev_rights:?}"
    );
    let owner_rights = rights(&app.get("/auth/session", &cookie).await.body);
    assert!(owner_rights.contains(&"org.delete".to_owned()));
    assert!(!owner_rights.contains(&"org.update".to_owned()));
    let mine = app.get("/me/memberships", &dev).await;
    assert_eq!(
        mine.body["data"][0]["org_deletion_due_at"],
        scheduled["deletion_due_at"]
    );

    for (method, path, body) in [
        (
            Method::PATCH,
            format!("/orgs/{org}"),
            json!({ "name": "Renamed" }),
        ),
        (
            Method::POST,
            format!("/orgs/{org}/projects"),
            json!({ "name": "New" }),
        ),
        (
            Method::PATCH,
            format!("/orgs/{org}/projects/{project}"),
            json!({ "name": "Renamed" }),
        ),
        (
            Method::POST,
            format!("/orgs/{org}/invites"),
            json!({ "email": "member@example.com", "preset": "member" }),
        ),
        (Method::DELETE, format!("/orgs/{org}"), json!({})),
    ] {
        read_only(&app.call(method, &path, Some(body), Some(&cookie)).await);
    }
    read_only(
        &app.post(&format!("/me/keys/{key_id}/disable"), json!({}), Some(&dev))
            .await,
    );
    read_only(
        &app.call(
            Method::DELETE,
            &format!("/me/memberships/{org}"),
            None,
            Some(&dev),
        )
        .await,
    );
    assert_eq!(
        app.get("/auth/session", &dev).await.body["memberships"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the member is still in the org"
    );

    let models = app
        .raw(
            Request::builder()
                .uri("/v1/models")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(models.status, StatusCode::UNAUTHORIZED, "{:?}", models.body);

    assert_eq!(
        app.post(&format!("/orgs/{org}/restore"), json!({}), Some(&dev))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let restored = app
        .post(&format!("/orgs/{org}/restore"), json!({}), Some(&cookie))
        .await;
    assert_eq!(restored.body["state"], "active", "{:?}", restored.body);
    assert!(restored.body["deletion_due_at"].is_null());
    let renamed = app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}"),
            Some(json!({ "name": "Renamed" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.body);
    s.cleanup().await;
}

#[tokio::test]
async fn instance_admins_set_the_grace_period_and_whether_orgs_can_be_deleted() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let admin = join(&app, &owner, "admin@example.com", "admin").await;

    let settings = app.get("/instance/settings", &owner.cookie).await;
    assert_eq!(
        (
            &settings.body["org_deletion"],
            &settings.body["org_deletion_grace_days"]
        ),
        (&json!(true), &json!(7))
    );
    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "org_deletion_grace_days": 3 })),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    for days in [0, 91] {
        let r = app
            .call(
                Method::PATCH,
                "/instance/settings",
                Some(json!({ "org_deletion_grace_days": days })),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(
            (r.status, r.body["error"]["param"].as_str()),
            (StatusCode::BAD_REQUEST, Some("org_deletion_grace_days"))
        );
    }
    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "org_deletion_grace_days": 3 })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["org_deletion_grace_days"], 3, "{:?}", r.body);
    let audit = app
        .get(
            "/instance/audit-log?action=instance.settings_update",
            &owner.cookie,
        )
        .await;
    assert_eq!(
        audit.body["data"][0]["after"]["org_deletion_grace_days"], 3,
        "{:?}",
        audit.body
    );

    let cookie = sign_in(&app, "owner@example.com").await;
    let scheduled = schedule(&app, &org, &cookie).await;
    assert_eq!(due(&scheduled), app.clock.now() + time::Duration::days(3));
    app.call(
        Method::PATCH,
        "/instance/settings",
        Some(json!({ "org_deletion_grace_days": 30 })),
        Some(&cookie),
    )
    .await;
    assert_eq!(
        app.get(&format!("/orgs/{org}"), &cookie).await.body["deletion_due_at"],
        scheduled["deletion_due_at"]
    );

    let r = app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "org_deletion": false })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.body["org_deletion"], false);
    assert_eq!(
        app.get("/auth/session", &admin).await.body["org_deletion"],
        json!({ "allowed": false, "grace_days": 30 })
    );
    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/orgs/{org}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str()),
        (
            StatusCode::FORBIDDEN,
            Some("This instance doesn't allow deleting organizations.")
        )
    );
    let r = app
        .post(
            &format!("/instance/orgs/{org}/restore"),
            json!({}),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.body["state"], "active", "{:?}", r.body);
    let r = app
        .call(Method::DELETE, &format!("/orgs/{org}"), None, Some(&cookie))
        .await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str()),
        (
            StatusCode::FORBIDDEN,
            Some("This instance doesn't allow deleting organizations.")
        )
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}"), &cookie).await.body["state"],
        "active"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_org_is_deleted_for_good_when_its_grace_period_ends() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let dev = join(&app, &owner, "developer@example.com", "developer").await;
    let project = default_project(&app, &owner).await;
    let limit = app
        .post(
            &format!("/orgs/{org}/projects/{project}/limits"),
            json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": 50 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(limit.status, StatusCode::CREATED, "{:?}", limit.body);
    let member = app
        .post(
            &format!("/orgs/{org}/projects/{project}/limits"),
            json!({ "level": "member", "user": owner.user_id, "kind": "token_budget", "period": "total", "amount": 50 }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(member.status, StatusCode::CREATED, "{:?}", member.body);
    let cookie = sign_in(&app, "owner@example.com").await;
    schedule(&app, &org, &cookie).await;
    s.clickhouse.migrate(&s.pg).await.unwrap();
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "org_deletion")
        .unwrap();

    app.clock.advance(time::Duration::days(6));
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "deleted 0 organizations"
    );
    assert_eq!(
        app.get(&format!("/orgs/{org}/members"), &dev).await.status,
        StatusCode::OK
    );

    app.clock
        .advance(time::Duration::days(1) + time::Duration::minutes(1));
    let mut conn = app.state.valkey.conn();
    let mut counters = Vec::new();
    for row in kaeso_store::pg::limits::list_org(
        &app.state.pg,
        org.parse::<kaeso_core::id::OrgId>().unwrap().uuid(),
    )
    .await
    .unwrap()
    {
        let key = app
            .state
            .valkey
            .key(&row.limit().unwrap().counter(app.clock.now()));
        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("v")
            .arg(1)
            .query_async(&mut conn)
            .await
            .unwrap();
        let _: () = redis::cmd("ZADD")
            .arg(format!("{key}:f"))
            .arg(1)
            .arg("req")
            .query_async(&mut conn)
            .await
            .unwrap();
        counters.extend([format!("{key}:f"), key]);
    }
    assert_eq!(counters.len(), 4);
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "deleted 1 organizations"
    );
    let left: i64 = redis::cmd("EXISTS")
        .arg(&counters)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(
        app.get(&format!("/orgs/{org}"), &dev).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(
        app.get("/auth/session", &dev).await.body["memberships"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        app.get("/me/memberships", &dev).await.body["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let admin = sign_in(&app, "owner@example.com").await;
    assert!(
        app.get("/instance/orgs", &admin).await.body["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let left: Vec<(i64,)> = sqlx::query_as(
        "SELECT count(*) FROM projects UNION ALL SELECT count(*) FROM api_keys \
         UNION ALL SELECT count(*) FROM limits UNION ALL SELECT count(*) FROM org_memberships \
         UNION ALL SELECT count(*) FROM audit_events WHERE org_id IS NOT NULL",
    )
    .fetch_all(&app.state.pg)
    .await
    .unwrap();
    assert!(left.iter().all(|(n,)| *n == 0), "{left:?}");
    let audit = app
        .get("/instance/audit-log?action=org.delete_final", &admin)
        .await;
    let event = &audit.body["data"][0];
    assert_eq!(event["target_id"], org.as_str(), "{:?}", audit.body);
    assert_eq!(event["before"]["name"], "Acme");
    s.cleanup().await;
}

#[tokio::test]
async fn instance_admins_sort_by_deletion_date_restore_and_delete_now() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let mut cookie = sign_in(&app, "owner@example.com").await;
    let mut ids = vec![owner.org_id.clone()];
    for name in ["Northwind", "Juniper Studio", "Halcyon"] {
        let r = app
            .post("/instance/orgs", json!({ "name": name }), Some(&cookie))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        ids.push(r.body["id"].as_str().unwrap().to_owned());
    }
    schedule(&app, &ids[1], &cookie).await;
    app.clock.advance(time::Duration::hours(1));
    cookie = sign_in(&app, "owner@example.com").await;
    app.call(
        Method::PATCH,
        "/instance/settings",
        Some(json!({ "org_deletion_grace_days": 2 })),
        Some(&cookie),
    )
    .await;
    schedule(&app, &ids[3], &cookie).await;

    let names = |r: &common::Resp| -> Vec<String> {
        r.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let sorted = app.get("/instance/orgs?sort=deletion", &cookie).await;
    assert_eq!(
        names(&sorted),
        ["Halcyon", "Northwind", "Acme", "Juniper Studio"]
    );
    assert!(sorted.body["data"][0]["deletion_due_at"].is_string());
    let mut paged = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let path = match &after {
            Some(a) => format!("/instance/orgs?sort=deletion&limit=1&after={a}"),
            None => "/instance/orgs?sort=deletion&limit=1".to_owned(),
        };
        let r = app.get(&path, &cookie).await;
        paged.extend(names(&r));
        match r.body["next_cursor"].as_str() {
            Some(c) => after = Some(c.to_owned()),
            None => break,
        }
    }
    assert_eq!(paged, names(&sorted));
    assert_eq!(
        names(&app.get("/instance/orgs", &cookie).await),
        ["Acme", "Northwind", "Juniper Studio", "Halcyon"]
    );

    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/orgs/{}", ids[2]),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    app.clock.advance(time::Duration::minutes(20));
    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/orgs/{}", ids[3]),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(r.error_type(), "recent_sign_in_required");
    cookie = sign_in(&app, "owner@example.com").await;
    s.clickhouse.migrate(&s.pg).await.unwrap();
    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/orgs/{}", ids[3]),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    assert_eq!(
        app.get(&format!("/instance/orgs/{}", ids[3]), &cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let audit = app.get("/instance/audit-log", &cookie).await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(&actions[..2], ["org.delete_final", "org.delete_now"]);

    let r = app
        .post(
            &format!("/instance/orgs/{}/restore", ids[1]),
            json!({}),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.body["state"], "active", "{:?}", r.body);
    assert_eq!(
        names(&app.get("/instance/orgs?sort=deletion", &cookie).await),
        ["Acme", "Northwind", "Juniper Studio"]
    );
    s.cleanup().await;
}
