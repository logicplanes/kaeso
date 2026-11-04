#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn new_key(app: &App, base: &str, cookie: &str, body: Value) -> (String, String) {
    let r = app
        .post(&format!("{base}/keys"), body, Some(cookie))
        .await
        .expect_status(StatusCode::CREATED);
    (
        r.body["id"].as_str().unwrap().to_owned(),
        r.body["secret"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn a_key_someone_else_disabled_stays_off_for_its_owner() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let (dev, _) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let (id, _) = new_key(&app, &base, &dev, json!({ "name": "laptop" })).await;

    app.post(
        &format!("{base}/keys/{id}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let r = app
        .post(&format!("/me/keys/{id}/enable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "Someone else disabled this key; ask a holder of `keys.all.manage` to enable it."
    );
    app.post(
        &format!("{base}/keys/{id}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    app.post(&format!("/me/keys/{id}/disable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::OK);
    let r = app
        .post(&format!("/me/keys/{id}/enable"), json!({}), Some(&dev))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["state"], "active");
    s.cleanup().await;
}

#[tokio::test]
async fn keys_cannot_undo_what_a_person_did_or_mint_platform_keys() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{org}/projects/{project}");
    let (_, script) = new_key(
        &app,
        &base,
        &owner.cookie,
        json!({ "name": "script", "access": "platform" }),
    )
    .await;
    let (target, _) = new_key(&app, &base, &owner.cookie, json!({ "name": "target" })).await;

    let r = app
        .with_key(
            Method::POST,
            &format!("{base}/keys"),
            Some(json!({ "name": "copy", "access": "platform" })),
            &script,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["error"]["message"],
        "Keys can't create keys with platform access; use a console session."
    );
    app.with_key(
        Method::POST,
        &format!("{base}/keys"),
        Some(json!({ "name": "inference only" })),
        &script,
    )
    .await
    .expect_status(StatusCode::CREATED);

    app.post(
        &format!("{base}/keys/{target}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let refused = app
        .with_key(
            Method::POST,
            &format!("{base}/keys/{target}/enable"),
            Some(json!({})),
            &script,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        refused.body["error"]["message"],
        "A person disabled this key, so only a person can enable it; use a console session."
    );
    app.post(
        &format!("{base}/keys/{target}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    app.with_key(
        Method::POST,
        &format!("{base}/keys/{target}/disable"),
        Some(json!({})),
        &script,
    )
    .await
    .expect_status(StatusCode::OK);
    app.with_key(
        Method::POST,
        &format!("{base}/keys/{target}/enable"),
        Some(json!({})),
        &script,
    )
    .await
    .expect_status(StatusCode::OK);

    let platform = format!("/orgs/{org}/platform-keys");
    let org_key = |name: &'static str| {
        let app = &app;
        let platform = platform.clone();
        let cookie = owner.cookie.clone();
        async move {
            let r = app
                .post(&platform, json!({ "name": name }), Some(&cookie))
                .await
                .expect_status(StatusCode::CREATED);
            (
                r.body["id"].as_str().unwrap().to_owned(),
                r.body["secret"].as_str().unwrap().to_owned(),
            )
        }
    };
    let (_, automation) = org_key("automation").await;
    let (other, _) = org_key("other").await;
    app.post(
        &format!("{platform}/{other}/disable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let refused = app
        .with_key(
            Method::POST,
            &format!("{platform}/{other}/enable"),
            Some(json!({})),
            &automation,
        )
        .await
        .expect_status(StatusCode::FORBIDDEN);
    assert_eq!(
        refused.body["error"]["message"],
        "A person disabled this key, so only a person can enable it; use a console session."
    );
    app.post(
        &format!("{platform}/{other}/enable"),
        json!({}),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn instance_changes_to_people_need_a_recent_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, grace) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "grace@example.com", "role": "member" }),
        "Grace",
    )
    .await;
    app.clock.advance(time::Duration::minutes(16));
    for (method, path, body) in [
        (
            Method::PUT,
            format!("/instance/users/{grace}/instance-admin"),
            Some(json!({ "is_instance_admin": true })),
        ),
        (
            Method::POST,
            format!("/instance/users/{grace}/reset-2fa"),
            Some(json!({})),
        ),
        (Method::DELETE, format!("/instance/users/{grace}"), None),
        (
            Method::POST,
            format!("/instance/orgs/{}/owner", owner.org_id),
            Some(json!({ "user_id": grace })),
        ),
    ] {
        let r = app.call(method, &path, body, Some(&owner.cookie)).await;
        assert_eq!(
            (r.status, r.error_type()),
            (StatusCode::FORBIDDEN, "recent_sign_in_required"),
            "{path}: {:?}",
            r.body
        );
    }
    let member = app
        .get(
            &format!("/orgs/{}/members/{grace}", owner.org_id),
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(member.body["grant"]["role"]["id"], "member");
    s.cleanup().await;
}

async fn until_a_lock_is_awaited(pool: &sqlx::PgPool) {
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting > 0 {
            return;
        }
        tokio::task::yield_now().await;
    }
}

async fn once_it_waits(
    app: &App,
    held: sqlx::Transaction<'static, sqlx::Postgres>,
    request: impl std::future::Future<Output = common::Resp>,
) -> common::Resp {
    tokio::pin!(request);
    tokio::select! {
        r = &mut request => panic!("the key didn't wait for the change under way: {:?}", r.body),
        () = until_a_lock_is_awaited(&app.state.pg) => {}
    }
    held.commit().await.unwrap();
    request.await
}

async fn keys_of(app: &App, owner: &Owner, base: &str, filter: &str) -> Value {
    app.get(
        &format!("{base}/keys?include_archived=true&{filter}"),
        &owner.cookie,
    )
    .await
    .expect_status(StatusCode::OK)
    .body["data"]
        .clone()
}

#[tokio::test]
async fn a_key_asked_for_while_its_member_is_removed_is_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let (dev, dev_id) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let org = common::uuid(&owner.org_id);
    let mut removal = app.state.pg.begin().await.unwrap();
    kaeso_store::pg::members::delete_org(&mut removal, org, common::uuid(&dev_id))
        .await
        .unwrap();

    let r = once_it_waits(
        &app,
        removal,
        app.post(
            &format!("{base}/keys"),
            json!({ "name": "late" }),
            Some(&dev),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "No such organization, or you cannot see it."
    );
    assert_eq!(
        keys_of(&app, &owner, &base, &format!("user={dev_id}")).await,
        json!([])
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_key_asked_for_while_its_member_leaves_the_project_is_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let (member, member_id) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "member@example.com", "role": "member" }),
        "Member",
    )
    .await;
    app.call(
        Method::PATCH,
        &format!("{base}/members/{member_id}"),
        Some(json!({ "role": "developer" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    let mut removal = app.state.pg.begin().await.unwrap();
    kaeso_store::pg::members::delete_project(
        &mut *removal,
        common::uuid(&project),
        common::uuid(&member_id),
    )
    .await
    .unwrap();

    let r = once_it_waits(
        &app,
        removal,
        app.post(
            &format!("{base}/keys"),
            json!({ "name": "late" }),
            Some(&member),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "No such project, or you cannot see it."
    );
    assert_eq!(
        keys_of(&app, &owner, &base, &format!("user={member_id}")).await,
        json!([])
    );
    s.cleanup().await;
}

#[tokio::test]
async fn deleting_an_account_archives_a_key_created_meanwhile() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let (dev, dev_id) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let key = uuid::Uuid::now_v7();
    let mut creating = app.state.pg.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM org_memberships WHERE org_id = $1 AND user_id = $2 FOR SHARE")
        .bind(common::uuid(&owner.org_id))
        .bind(common::uuid(&dev_id))
        .fetch_one(&mut *creating)
        .await
        .unwrap();
    kaeso_store::pg::keys::insert(
        &mut *creating,
        &kaeso_store::pg::keys::NewKey {
            id: key,
            org_id: common::uuid(&owner.org_id),
            project_id: common::uuid(&project),
            user_id: Some(common::uuid(&dev_id)),
            service_account_id: None,
            name: "late",
            secret_hash: &[7; 32],
            secret_prefix: "sk-kso-late",
            secret_last4: "late",
            inference: true,
            platform: false,
            expires_at: None,
            created_by_kind: "user",
            created_by_id: Some(common::uuid(&dev_id)),
            rotated_from: None,
        },
    )
    .await
    .unwrap();

    let r = once_it_waits(
        &app,
        creating,
        app.call(Method::DELETE, "/me", None, Some(&dev)),
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    let keys = keys_of(&app, &owner, &base, &format!("user={dev_id}")).await;
    assert_eq!(keys.as_array().unwrap().len(), 1, "{keys:?}");
    assert_eq!(keys[0]["state"], "archived");
    s.cleanup().await;
}

#[tokio::test]
async fn a_key_asked_for_while_its_project_is_archived_is_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let created = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Support bot" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let project = created.body["id"].as_str().unwrap().to_owned();
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let mut archive = app.state.pg.begin().await.unwrap();
    let mut row = kaeso_store::pg::projects::get_for_update(
        &mut *archive,
        common::uuid(&owner.org_id),
        common::uuid(&project),
    )
    .await
    .unwrap()
    .unwrap();
    row.archived_at = Some(time::OffsetDateTime::now_utc());
    kaeso_store::pg::projects::update(&mut *archive, &row)
        .await
        .unwrap();

    let r = once_it_waits(
        &app,
        archive,
        app.post(
            &format!("{base}/keys"),
            json!({ "name": "late" }),
            Some(&owner.cookie),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "The project is archived; its keys cannot change."
    );
    assert_eq!(keys_of(&app, &owner, &base, "").await, json!([]));
    s.cleanup().await;
}

#[tokio::test]
async fn a_key_asked_for_while_its_service_account_is_archived_is_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    let created = app
        .post(
            &format!("{base}/service-accounts"),
            json!({ "name": "deployer", "role": "developer" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let account = created.body["id"].as_str().unwrap().to_owned();
    let mut archive = app.state.pg.begin().await.unwrap();
    let mut row = kaeso_store::pg::service_accounts::get_for_update(
        &mut *archive,
        common::uuid(&project),
        common::uuid(&account),
    )
    .await
    .unwrap()
    .unwrap();
    row.archived_at = Some(time::OffsetDateTime::now_utc());
    kaeso_store::pg::service_accounts::update(&mut *archive, &row)
        .await
        .unwrap();

    let r = once_it_waits(
        &app,
        archive,
        app.post(
            &format!("{base}/service-accounts/{account}/keys"),
            json!({ "name": "late" }),
            Some(&owner.cookie),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "No such active service account, or you cannot see it."
    );
    assert_eq!(
        keys_of(&app, &owner, &base, &format!("service_account={account}")).await,
        json!([])
    );
    s.cleanup().await;
}
