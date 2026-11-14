#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

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

async fn project_path(app: &App, owner: &common::Owner) -> String {
    let r = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Support bot" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    format!(
        "/orgs/{}/projects/{}",
        owner.org_id,
        r.body["id"].as_str().unwrap()
    )
}

async fn send_if_match(
    app: &App,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    cookie: &str,
    tag: &str,
) -> common::Resp {
    let req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/platform/v1{path}"))
        .header(
            axum::http::header::COOKIE,
            format!("__Host-kaeso_session={cookie}"),
        )
        .header(axum::http::header::ORIGIN, common::ORIGIN)
        .header(axum::http::header::IF_MATCH, tag)
        .header(axum::http::header::CONTENT_TYPE, "application/json");
    let body = body.map_or_else(axum::body::Body::empty, |b| {
        axum::body::Body::from(serde_json::to_vec(&b).unwrap())
    });
    app.raw(req.body(body).unwrap()).await
}

#[tokio::test]
async fn model_overrides_changed_at_once_both_stay() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let project = project_path(&app, &owner).await;
    let r = app
        .post(
            &format!("{project}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-sonnet-4-5" }),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.headers["etag"], "\"1\"");
    let model = format!("{project}/models/{}", r.body["id"].as_str().unwrap());
    for round in 0..ROUNDS {
        let name = format!("Sonnet {round}");
        let price = format!("{}.5", round + 1);
        let (named, priced) = tokio::join!(
            app.call(
                Method::PATCH,
                &model,
                Some(json!({ "overrides": { "display_name": name } })),
                cookie
            ),
            app.call(
                Method::PATCH,
                &model,
                Some(json!({ "overrides": { "input_price": price } })),
                cookie
            ),
        );
        assert_eq!(named.status, StatusCode::OK, "{:?}", named.body);
        assert_eq!(priced.status, StatusCode::OK, "{:?}", priced.body);
        let now = app.get(&model, &owner.cookie).await;
        assert_eq!(
            now.body["overrides"]["display_name"],
            name.as_str(),
            "round {round}"
        );
        assert_eq!(
            now.body["overrides"]["input_price"],
            price.as_str(),
            "round {round}"
        );
        assert_eq!(now.body["revision"], json!(2 * round + 3), "round {round}");
    }
}

#[tokio::test]
async fn a_change_meant_for_an_older_revision_is_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let project = project_path(&app, &owner).await;
    let r = app
        .post(
            &format!("{project}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-sonnet-4-5" }),
            cookie,
        )
        .await;
    let model = format!("{project}/models/{}", r.body["id"].as_str().unwrap());
    let got = app.get(&model, &owner.cookie).await;
    let tag = got.headers["etag"].to_str().unwrap().to_owned();
    assert_eq!(tag, format!("\"{}\"", got.body["revision"]));
    let body = Some(json!({ "overrides": { "display_name": "Mine" } }));
    let first = send_if_match(
        &app,
        Method::PATCH,
        &model,
        body.clone(),
        &owner.cookie,
        &tag,
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.body);
    assert_eq!(first.headers["etag"], "\"2\"");
    let late = send_if_match(&app, Method::PATCH, &model, body, &owner.cookie, &tag).await;
    assert_eq!(
        late.status,
        StatusCode::PRECONDITION_FAILED,
        "{:?}",
        late.body
    );
    assert_eq!(late.error_type(), "conflict");
    assert_eq!(
        late.body["error"]["message"],
        "This catalogue model was changed since you loaded it; reload it and try again."
    );
    let gone = send_if_match(&app, Method::DELETE, &model, None, &owner.cookie, &tag).await;
    assert_eq!(gone.status, StatusCode::PRECONDITION_FAILED);
    let bad = send_if_match(&app, Method::DELETE, &model, None, &owner.cookie, "2").await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.body["error"]["param"], "If-Match");
    let removed = send_if_match(&app, Method::DELETE, &model, None, &owner.cookie, "\"2\"").await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{:?}", removed.body);

    let limit = app
        .post(
            &format!("{project}/limits"),
            json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": "50" }),
            cookie,
        )
        .await;
    assert_eq!(limit.status, StatusCode::CREATED, "{:?}", limit.body);
    assert_eq!(limit.body["revision"], 1);
    assert_eq!(limit.body["alert_thresholds"], json!([50, 80, 100]));
    let path = format!("{project}/limits/{}", limit.body["id"].as_str().unwrap());
    let raised = send_if_match(
        &app,
        Method::PATCH,
        &path,
        Some(json!({ "amount": "60" })),
        &owner.cookie,
        "\"1\"",
    )
    .await;
    assert_eq!(raised.status, StatusCode::OK, "{:?}", raised.body);
    assert_eq!(raised.body["revision"], 2);
    let stale = send_if_match(
        &app,
        Method::PATCH,
        &path,
        Some(json!({ "amount": "70" })),
        &owner.cookie,
        "\"1\"",
    )
    .await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(
        stale.body["error"]["message"],
        "This limit was changed since you loaded it; reload it and try again."
    );
    let now = app.get(&path, &owner.cookie).await;
    assert_eq!(now.body["amount"], "60");
    assert_eq!(now.headers["etag"], "\"2\"");
    let removed = send_if_match(&app, Method::DELETE, &path, None, &owner.cookie, "*").await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn limits_changed_at_once_keep_the_hierarchy() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let project = project_path(&app, &owner).await;
    let parent = app
        .post(
            &format!("{project}/limits"),
            json!({ "level": "project", "kind": "token_budget", "period": "month", "amount": 1000 }),
            cookie,
        )
        .await;
    let child = app
        .post(
            &format!("{project}/limits"),
            json!({ "level": "member", "user": owner.user_id, "kind": "token_budget", "period": "month", "amount": 500 }),
            cookie,
        )
        .await;
    assert_eq!(child.status, StatusCode::CREATED, "{:?}", child.body);
    assert_eq!(child.body["alert_thresholds"], json!([50, 80, 100]));
    let parent = format!("{project}/limits/{}", parent.body["id"].as_str().unwrap());
    let child = format!("{project}/limits/{}", child.body["id"].as_str().unwrap());
    for round in 0..ROUNDS {
        let (lowered, raised) = tokio::join!(
            app.call(
                Method::PATCH,
                &parent,
                Some(json!({ "amount": 600 })),
                cookie
            ),
            app.call(
                Method::PATCH,
                &child,
                Some(json!({ "amount": 800 })),
                cookie
            ),
        );
        let ok = [lowered.status, raised.status]
            .iter()
            .filter(|s| **s == StatusCode::OK)
            .count();
        assert_eq!(ok, 1, "round {round}: {:?} {:?}", lowered.body, raised.body);
        let p = app.get(&parent, &owner.cookie).await.body["amount"].clone();
        let c = app.get(&child, &owner.cookie).await.body["amount"].clone();
        let (p, c): (u64, u64) = (
            p.as_str().unwrap().parse().unwrap(),
            c.as_str().unwrap().parse().unwrap(),
        );
        assert!(c <= p, "round {round}: member {c} above project {p}");
        app.call(
            Method::PATCH,
            &child,
            Some(json!({ "amount": 500 })),
            cookie,
        )
        .await;
        app.call(
            Method::PATCH,
            &parent,
            Some(json!({ "amount": 1000 })),
            cookie,
        )
        .await;
    }
}

#[tokio::test]
async fn rate_limits_have_no_alerts() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let cookie = Some(owner.cookie.as_str());
    let project = project_path(&app, &owner).await;
    let rate = app
        .post(
            &format!("{project}/limits"),
            json!({ "level": "project", "kind": "request_rate", "period": "minute", "amount": 10 }),
            cookie,
        )
        .await;
    assert_eq!(rate.status, StatusCode::CREATED, "{:?}", rate.body);
    assert_eq!(rate.body["alert_thresholds"], json!([]));
    let path = format!("{project}/limits/{}", rate.body["id"].as_str().unwrap());
    let r = app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "alert_thresholds": [50] })),
            cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "Rate limits have no alerts; leave `alert_thresholds` out."
    );
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
        r = &mut request => panic!("the change did not wait for the owner lock: {:?}", r.body),
        () = until_a_lock_is_awaited(&app.state.pg) => {}
    }
    held.commit().await.unwrap();
    request.await
}

struct Transfer {
    owner: common::Owner,
    admin: String,
    heir: (String, String),
}

async fn mid_transfer(app: &App) -> (Transfer, sqlx::Transaction<'static, sqlx::Postgres>) {
    let owner = app.setup().await;
    let (admin, _) = common::join(
        app,
        &owner,
        json!({ "email": "admin@example.com", "role": "admin" }),
        "Ada",
    )
    .await;
    let heir = common::join(
        app,
        &owner,
        json!({ "email": "heir@example.com", "role": "developer" }),
        "Grace",
    )
    .await;
    let org = common::uuid(&owner.org_id);
    let preset = |p: &str| kaeso_store::pg::members::Grant {
        preset: p.to_owned(),
        ..Default::default()
    };
    let mut held = app.state.pg.begin().await.unwrap();
    kaeso_store::pg::members::lock_owners(&mut held, org)
        .await
        .unwrap();
    kaeso_store::pg::members::update_org(&mut *held, org, common::uuid(&heir.1), &preset("owner"))
        .await
        .unwrap();
    kaeso_store::pg::members::update_org(
        &mut *held,
        org,
        common::uuid(&owner.user_id),
        &preset("admin"),
    )
    .await
    .unwrap();
    (Transfer { owner, admin, heir }, held)
}

async fn owner_ids(app: &App, t: &Transfer) -> Vec<String> {
    let listed = app
        .get(
            &format!("/orgs/{}/members?role=owner", t.owner.org_id),
            &t.heir.0,
        )
        .await
        .expect_status(StatusCode::OK);
    listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["user"]["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn an_admin_cant_demote_someone_who_became_an_owner_meanwhile() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let (t, held) = mid_transfer(&app).await;
    let path = format!("/orgs/{}/members/{}", t.owner.org_id, t.heir.1);
    let r = once_it_waits(
        &app,
        held,
        app.call(
            Method::PATCH,
            &path,
            Some(json!({ "role": "member" })),
            Some(&t.admin),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "Only owners can make someone an owner or change an owner's role."
    );
    assert_eq!(owner_ids(&app, &t).await, std::slice::from_ref(&t.heir.1));
    s.cleanup().await;
}

#[tokio::test]
async fn an_admin_cant_remove_someone_who_became_an_owner_meanwhile() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let (t, held) = mid_transfer(&app).await;
    let path = format!("/orgs/{}/members/{}", t.owner.org_id, t.heir.1);
    let r = once_it_waits(
        &app,
        held,
        app.call(Method::DELETE, &path, None, Some(&t.admin)),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "Only owners can remove an owner."
    );
    assert_eq!(owner_ids(&app, &t).await, std::slice::from_ref(&t.heir.1));
    s.cleanup().await;
}

#[tokio::test]
async fn someone_who_became_the_last_owner_meanwhile_cant_leave() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let (t, held) = mid_transfer(&app).await;
    let path = format!("/me/memberships/{}", t.owner.org_id);
    let r = once_it_waits(
        &app,
        held,
        app.call(Method::DELETE, &path, None, Some(&t.heir.0)),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "You are this organization's last owner; transfer ownership before leaving."
    );
    assert_eq!(owner_ids(&app, &t).await, std::slice::from_ref(&t.heir.1));
    s.cleanup().await;
}
