mod common;

use axum::http::{Method, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::App;
use kaeso_testkit::services::TestServices;
use serde_json::json;

const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0,
    0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn upload(bytes: &[u8], content_type: &str) -> serde_json::Value {
    json!({ "content_type": content_type, "data": STANDARD.encode(bytes) })
}

#[tokio::test]
async fn people_and_organizations_have_pictures() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());

    let r = app
        .call(
            Method::PUT,
            "/me/avatar",
            Some(upload(PNG, "image/png")),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let url = r.body["avatar_url"].as_str().unwrap().to_owned();
    assert!(url.starts_with("/platform/v1/me/avatars/"), "{url}");
    let session = app.get("/auth/session", &owner.cookie).await;
    assert_eq!(session.body["user"]["avatar_url"], url.as_str());
    let picture = app
        .get(url.trim_start_matches("/platform/v1"), &owner.cookie)
        .await;
    assert_eq!(picture.status, StatusCode::OK);
    assert_eq!(picture.headers["content-type"], "image/png");
    assert!(
        picture.headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );

    let r = app
        .call(
            Method::PUT,
            "/me/avatar",
            Some(upload(b"<svg/>", "image/png")),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = app
        .call(
            Method::PUT,
            "/me/avatar",
            Some(upload(PNG, "image/jpeg")),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let mut big = PNG.to_vec();
    big.resize(600 * 1024, 0);
    let r = app
        .call(
            Method::PUT,
            "/me/avatar",
            Some(upload(&big, "image/png")),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let org = format!("/orgs/{}/avatar", owner.org_id);
    let r = app
        .call(Method::PUT, &org, Some(upload(PNG, "image/png")), cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["avatar_url"],
        url.as_str(),
        "the same bytes are stored once"
    );
    let mine = app.get("/me/memberships", &owner.cookie).await;
    assert_eq!(mine.body["data"][0]["org_avatar_url"], url.as_str());

    let r = app.call(Method::DELETE, "/me/avatar", None, cookie).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body["avatar_url"].is_null());
    assert_eq!(
        app.get(url.trim_start_matches("/platform/v1"), &owner.cookie)
            .await
            .status,
        StatusCode::OK
    );
    let r = app.call(Method::DELETE, &org, None, cookie).await;
    assert!(r.body["avatar_url"].is_null());
    assert_eq!(
        app.get(url.trim_start_matches("/platform/v1"), &owner.cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let anonymous = app
        .call(
            Method::GET,
            url.trim_start_matches("/platform/v1"),
            None,
            None,
        )
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00,
];

#[tokio::test]
async fn projects_have_pictures_where_they_are_named() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let org = owner.org_id.clone();

    let r = app
        .post(
            &format!("/orgs/{org}/invites"),
            json!({ "email": "dev@example.com", "preset": "developer", "project_roles": [] }),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    app.wait_for_mail("dev@example.com", 1).await;
    let dev = app
        .post(
            "/auth/invites/accept",
            json!({ "token": app.last_token("dev@example.com"), "name": "Dev", "password": "correct horse battery" }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let created = app
        .post(
            &format!("/orgs/{org}/projects"),
            json!({ "name": "Support bot" }),
            Some(&dev),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert!(created.body["avatar_url"].is_null());
    let project = created.body["id"].as_str().unwrap().to_owned();
    let picture = format!("/orgs/{org}/projects/{project}/avatar");
    let r = app
        .call(
            Method::PUT,
            &picture,
            Some(upload(PNG, "image/png")),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let url = r.body["avatar_url"].as_str().unwrap().to_owned();
    assert!(url.starts_with("/platform/v1/me/avatars/"), "{url}");
    let served = app.get(url.trim_start_matches("/platform/v1"), &dev).await;
    assert_eq!(served.status, StatusCode::OK);
    assert_eq!(served.headers["content-type"], "image/png");

    let default = app
        .get(&format!("/orgs/{org}/projects"), &owner.cookie)
        .await
        .body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["is_default"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{org}/projects/{default}/avatar"),
            Some(upload(PNG, "image/png")),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let r = app
        .call(
            Method::PUT,
            &format!("/orgs/{org}/projects/{default}/avatar"),
            Some(upload(JPEG, "image/jpeg")),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let default_url = r.body["avatar_url"].as_str().unwrap().to_owned();
    assert_ne!(default_url, url);

    for (bytes, kind) in [(b"<svg/>".as_slice(), "image/png"), (PNG, "image/webp")] {
        let r = app
            .call(Method::PUT, &picture, Some(upload(bytes, kind)), Some(&dev))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    }
    let mut big = PNG.to_vec();
    big.resize(600 * 1024, 0);
    let r = app
        .call(
            Method::PUT,
            &picture,
            Some(upload(&big, "image/png")),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let one = app
        .get(&format!("/orgs/{org}/projects/{project}"), &dev)
        .await;
    assert_eq!(one.body["avatar_url"], url.as_str());
    let listed = app
        .get(&format!("/orgs/{org}/projects"), &owner.cookie)
        .await;
    let row = |id: &str| {
        listed.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(row(&project)["avatar_url"], url.as_str());
    assert_eq!(row(&default)["avatar_url"], default_url.as_str());
    let mine = app.get("/me/memberships", &dev).await;
    let projects = mine.body["data"][0]["projects"].as_array().unwrap();
    assert!(
        projects
            .iter()
            .any(|p| p["id"] == project.as_str() && p["avatar_url"] == url.as_str()),
        "{projects:?}"
    );
    let key = app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "openai", "name": "Team", "secret": "sk-test-org-secret-0123456789WXYZ" }),
            cookie,
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let assigned = app
        .call(
            Method::PUT,
            &format!(
                "/orgs/{org}/credentials/{}/projects",
                key.body["id"].as_str().unwrap()
            ),
            Some(json!({ "projects": [project] })),
            cookie,
        )
        .await;
    assert_eq!(assigned.status, StatusCode::OK, "{:?}", assigned.body);
    assert_eq!(assigned.body["projects"][0]["avatar_url"], url.as_str());

    let audit = app
        .get(
            &format!("/orgs/{org}/audit-log?action=project.update"),
            &owner.cookie,
        )
        .await;
    assert!(
        audit.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["after"]["avatar_url"] == url.as_str()),
        "{:?}",
        audit.body
    );

    let r = app.call(Method::DELETE, &picture, None, Some(&dev)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(r.body["avatar_url"].is_null());
    assert_eq!(
        app.get(url.trim_start_matches("/platform/v1"), &dev)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let r = app
        .post(
            &format!("/orgs/{org}/projects/{project}/archive"),
            json!({}),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = app
        .call(
            Method::PUT,
            &picture,
            Some(upload(PNG, "image/png")),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn deleting_an_account_removes_its_picture_and_language() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let pg = &app.state.pg;
    let user = kaeso_core::id::UserId::new();
    kaeso_store::pg::users::insert(
        pg,
        &kaeso_store::pg::users::NewUser {
            id: user.uuid(),
            email: "leaving@example.com",
            name: "Leaving",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: false,
        },
    )
    .await
    .unwrap();
    kaeso_store::pg::avatars::put(pg, "leaving-picture", "image/png", PNG)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET avatar = 'leaving-picture', locale = 'de' WHERE id = $1")
        .bind(user.uuid())
        .execute(pg)
        .await
        .unwrap();

    let mut tx = pg.begin().await.unwrap();
    kaeso_server::services::users::delete(&app.state, &mut tx, user, app.state.clock.now())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (state, avatar, locale): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT state, avatar, locale FROM users WHERE id = $1")
            .bind(user.uuid())
            .fetch_one(pg)
            .await
            .unwrap();
    assert_eq!((state.as_str(), avatar, locale), ("deleted", None, None));
    assert!(
        kaeso_store::pg::avatars::get(pg, "leaving-picture")
            .await
            .unwrap()
            .is_none()
    );
    s.cleanup().await;
}
