#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner, PASSWORD};
use kaeso_testkit::services::TestServices;
use serde_json::json;

async fn join(app: &App, owner: &Owner, email: &str, name: &str) -> (String, String) {
    common::join(
        app,
        owner,
        json!({ "email": email, "role": "member", "project_roles": [] }),
        name,
    )
    .await
}

#[tokio::test]
async fn instance_admins_find_users_by_name_email_and_state() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, grace) = join(&app, &owner, "grace@example.com", "Grace Hopper").await;
    join(&app, &owner, "ada@example.com", "Ada Lovelace").await;
    let (_, mia) = join(&app, &owner, "mia@BÜCHER.example", "Mia").await;

    let all = app.get("/instance/users", &owner.cookie).await;
    assert_eq!(all.status, StatusCode::OK, "{:?}", all.body);
    assert_eq!(all.body["data"].as_array().unwrap().len(), 4);
    for search in [
        "b%C3%BCcher.example",
        "mia%40b%C3%BCcher.example",
        "xn--bcher",
    ] {
        let found = app
            .get(&format!("/instance/users?search={search}"), &owner.cookie)
            .await;
        assert_eq!(found.body["data"].as_array().unwrap().len(), 1, "{search}");
        assert_eq!(found.body["data"][0]["id"], mia.as_str());
        assert_eq!(found.body["data"][0]["email"], "mia@xn--bcher-kva.example");
    }
    let found = app
        .get("/instance/users?search=hopper", &owner.cookie)
        .await;
    assert_eq!(
        found.body["data"].as_array().unwrap().len(),
        1,
        "{:?}",
        found.body
    );
    assert_eq!(found.body["data"][0]["id"], grace.as_str());
    let found = app
        .get("/instance/users?search=ADA%40", &owner.cookie)
        .await;
    assert_eq!(
        found.body["data"].as_array().unwrap().len(),
        1,
        "{:?}",
        found.body
    );
    let paged = app.get("/instance/users?limit=2", &owner.cookie).await;
    assert_eq!(paged.body["data"].as_array().unwrap().len(), 2);
    assert_eq!(paged.body["has_more"], true);

    let r = app
        .post(
            &format!("/instance/users/{grace}/deactivate"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .post(
            &format!("/instance/users/{grace}/deactivate"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let deactivated = app
        .get("/instance/users?state=deactivated", &owner.cookie)
        .await;
    assert_eq!(
        deactivated.body["data"].as_array().unwrap().len(),
        1,
        "{:?}",
        deactivated.body
    );
    let r = app
        .call(
            Method::PUT,
            &format!("/instance/users/{grace}/instance-admin"),
            Some(json!({ "is_instance_admin": true })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .post(
            &format!("/instance/users/{grace}/reactivate"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .post(
            &format!("/instance/users/{grace}/reactivate"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn instance_admins_end_sessions_reset_2fa_and_share_administration() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (grace_cookie, grace) = join(&app, &owner, "grace@example.com", "Grace Hopper").await;

    let r = app
        .post(
            "/instance/users/invites",
            json!({ "email": "grace@example.com" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    let r = app
        .post(
            &format!("/instance/users/{}/deactivate", owner.user_id),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = app
        .call(
            Method::DELETE,
            &format!("/instance/users/{}", owner.user_id),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = app
        .get(
            "/instance/users/usr_000000000000000000000000",
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    assert_eq!(
        app.get("/auth/session", &grace_cookie).await.status,
        StatusCode::OK
    );
    let r = app
        .post(
            &format!("/instance/users/{grace}/sessions/revoke"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    assert_eq!(
        app.get("/auth/session", &grace_cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    app.wait_for_mail("grace@example.com", 2).await;

    let r = app
        .post(
            &format!("/instance/users/{grace}/reset-2fa"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);

    let r = app
        .call(
            Method::PUT,
            &format!("/instance/users/{grace}/instance-admin"),
            Some(json!({ "is_instance_admin": true })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["is_instance_admin"], true);
    let signed_in = app
        .post(
            "/auth/password",
            json!({ "email": "grace@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{:?}", signed_in.body);
    let grace_admin = signed_in.cookie().unwrap();
    assert_eq!(
        app.get("/instance/users", &grace_admin).await.status,
        StatusCode::OK
    );
    let r = app
        .call(
            Method::PUT,
            &format!("/instance/users/{grace}/instance-admin"),
            Some(json!({ "is_instance_admin": false })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        app.get("/instance/users", &grace_admin).await.status,
        StatusCode::FORBIDDEN
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_last_instance_admin_is_never_deactivated_or_deleted() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let pg = &app.state.pg;
    let solo = kaeso_core::id::UserId::new();
    kaeso_store::pg::users::insert(
        pg,
        &kaeso_store::pg::users::NewUser {
            id: solo.uuid(),
            email: "solo@example.com",
            name: "Solo",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: true,
        },
    )
    .await
    .unwrap();
    let owner_id: kaeso_core::id::UserId = owner.user_id.parse().unwrap();
    let r = app
        .call(
            Method::PUT,
            &format!("/instance/users/{}/instance-admin", owner.user_id),
            Some(json!({ "is_instance_admin": false })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(
        !kaeso_store::pg::users::get(pg, owner_id.uuid())
            .await
            .unwrap()
            .unwrap()
            .is_instance_admin
    );

    let now = app.state.clock.now();
    let mut tx = pg.begin().await.unwrap();
    let deactivated =
        kaeso_server::services::users::deactivate(&app.state, &mut tx, solo, now, None).await;
    assert!(
        deactivated
            .unwrap_err()
            .to_string()
            .contains("last instance admin")
    );
    drop(tx);
    let mut tx = pg.begin().await.unwrap();
    let deleted = kaeso_server::services::users::delete(&app.state, &mut tx, solo, now).await;
    assert!(
        deleted
            .unwrap_err()
            .to_string()
            .contains("last instance admin")
    );
    drop(tx);
    let still = kaeso_store::pg::users::get(pg, solo.uuid())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (still.state.as_str(), still.is_instance_admin),
        ("active", true)
    );
    s.cleanup().await;
}
