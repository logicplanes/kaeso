#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::App;
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::notifications::Kind;
use kaeso_server::services::notifications::unsubscribe_token;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

fn areas(body: &Value) -> Vec<&str> {
    body["areas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["area"].as_str().unwrap())
        .collect()
}

fn kinds(body: &Value) -> Vec<&str> {
    body["areas"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|a| a["kinds"].as_array().unwrap())
        .map(|k| k["kind"].as_str().unwrap())
        .collect()
}

fn kind<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["areas"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|a| a["kinds"].as_array().unwrap())
        .find(|k| k["kind"] == name)
        .unwrap_or_else(|| panic!("{name} missing"))
}

#[tokio::test]
async fn each_person_sees_the_emails_their_rights_let_them_receive() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;

    let mine = app
        .get("/me/notifications", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        mine.body["sends_email"], false,
        "tests write emails to memory, not SMTP"
    );
    assert_eq!(
        areas(&mine.body),
        [
            "security",
            "personal",
            "project",
            "tools",
            "organization",
            "instance"
        ]
    );
    assert_eq!(
        kinds(&mine.body).len(),
        23,
        "an owner and instance admin gets every kind"
    );
    let security = kind(&mine.body, "account_security");
    assert_eq!(
        (
            &security["always"],
            &security["delivery"],
            &security["reason"]
        ),
        (&json!(true), &json!("now"), &Value::Null)
    );
    let joins = kind(&mine.body, "org_members");
    assert_eq!(
        (&joins["default"], &joins["delivery"], &joins["reason"]),
        (&json!("daily"), &json!("daily"), &json!("members.invite"))
    );
    assert_eq!(
        (&joins["scheduled"], &joins["deliveries"]),
        (&json!(false), &json!(["now", "daily", "off"]))
    );
    let weekly = kind(&mine.body, "weekly_summary");
    assert_eq!(
        (
            &weekly["delivery"],
            &weekly["scheduled"],
            &weekly["deliveries"]
        ),
        (&json!("off"), &json!(true), &json!(["now", "off"])),
        "a scheduled email is on or off; the digest would only delay it"
    );
    assert_eq!(
        kind(&mine.body, "cost_report")["deliveries"],
        json!(["now", "off"])
    );
    assert_eq!(security["deliveries"], json!([]));

    let (developer, _) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let theirs = app
        .get("/me/notifications", &developer)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        kinds(&theirs.body),
        [
            "account_security",
            "new_app",
            "weekly_summary",
            "access_changed",
            "budget_alerts",
            "limit_reached",
            "key_expiry",
            "key_changed",
            "content_logging",
            "connection_expired",
            "portal_clients",
            "cost_report",
        ]
    );
    assert_eq!(
        kind(&theirs.body, "budget_alerts")["reason"],
        "keys.own.create",
        "a developer gets budget alerts for their own keys"
    );
}

#[tokio::test]
async fn deliveries_change_one_kind_at_a_time_and_security_emails_have_none() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let put = |body: Value| {
        app.call(
            Method::PUT,
            "/me/notifications",
            Some(body),
            Some(&owner.cookie),
        )
    };

    let r = put(json!({ "kinds": { "org_members": "off", "weekly_summary": "now" } }))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(kind(&r.body, "org_members")["delivery"], "off");
    assert_eq!(kind(&r.body, "weekly_summary")["delivery"], "now");
    assert_eq!(
        kind(&r.body, "budget_alerts")["delivery"],
        "now",
        "others keep theirs"
    );

    let r = put(json!({ "kinds": { "org_members": "daily" } }))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(kind(&r.body, "org_members")["delivery"], "daily");
    assert_eq!(kind(&r.body, "weekly_summary")["delivery"], "now");

    for (body, param, message) in [
        (
            json!({ "kinds": { "account_security": "off" } }),
            "kinds.account_security",
            "Security emails are always sent; they have no setting.",
        ),
        (
            json!({ "kinds": { "gossip": "off" } }),
            "kinds.gossip",
            "`gossip` is not an email kind.",
        ),
        (
            json!({ "kinds": { "weekly_summary": "daily" } }),
            "kinds.weekly_summary",
            "`weekly_summary` goes out on a schedule; choose `now` or `off`.",
        ),
        (
            json!({ "kinds": { "cost_report": "daily" } }),
            "kinds.cost_report",
            "`cost_report` goes out on a schedule; choose `now` or `off`.",
        ),
    ] {
        let r = put(body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        assert_eq!(
            (
                r.body["error"]["param"].as_str(),
                r.body["error"]["message"].as_str()
            ),
            (Some(param), Some(message))
        );
    }
    let r = put(json!({ "kinds": { "org_members": "weekly" } })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);

    let audit = app
        .get(
            "/instance/audit-log?action=user.notifications",
            &owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        audit.body["data"][0]["after"],
        json!({ "org_members": "daily" })
    );
}

#[tokio::test]
async fn a_digest_setting_kept_from_before_reads_as_on_for_a_scheduled_email() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let user: uuid::Uuid = owner.user_id.parse::<UserId>().unwrap().uuid();
    for kind in ["weekly_summary", "cost_report"] {
        sqlx::query(
            "INSERT INTO notification_settings (user_id, kind, delivery) VALUES ($1, $2, 'daily')",
        )
        .bind(user)
        .bind(kind)
        .execute(&app.state.pg)
        .await
        .unwrap();
    }
    let mine = app
        .get("/me/notifications", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(kind(&mine.body, "weekly_summary")["delivery"], "now");
    assert_eq!(kind(&mine.body, "cost_report")["delivery"], "now");
    let token = unsubscribe_token(
        &app.state,
        owner.user_id.parse().unwrap(),
        Kind::WeeklySummary,
    );
    let shown = app
        .call_with(
            Method::GET,
            &format!("/notifications/unsubscribe?token={token}"),
            None,
            None,
            None,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(shown.body["delivery"], "now");
}

async fn mute(app: &App, owner: &common::Owner, org: &str, muted: bool) -> common::Resp {
    app.call(
        Method::PUT,
        &format!("/me/memberships/{org}/notifications"),
        Some(json!({ "muted": muted })),
        Some(&owner.cookie),
    )
    .await
}

#[tokio::test]
async fn a_muted_org_sends_no_alerts_and_shows_on_memberships() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;

    let r = mute(&app, &owner, &owner.org_id, true)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body, json!({ "muted": true }));
    let memberships = app
        .get("/me/memberships", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(memberships.body["data"][0]["notifications_muted"], true);

    app.post(
        &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
        json!({ "name": "short", "expires_in_days": 1 }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "key_expiry_warnings")
        .unwrap();
    assert_eq!(
        kaeso_server::jobs::run_once(&app.state, job)
            .await
            .unwrap()
            .unwrap(),
        "sent 0 key expiry warnings"
    );

    let elsewhere = OrgId::from_uuid(uuid::Uuid::from_u128(1)).to_string();
    let r = mute(&app, &owner, &elsewhere, true).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);

    mute(&app, &owner, &owner.org_id, false)
        .await
        .expect_status(StatusCode::OK);
    let memberships = app.get("/me/memberships", &owner.cookie).await;
    assert_eq!(memberships.body["data"][0]["notifications_muted"], false);
}

#[tokio::test]
async fn a_test_email_goes_out_at_most_once_a_minute() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let before = app.mail.to("owner@example.com").len();

    let r = app
        .post("/me/notifications/test", json!({}), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        r.body,
        json!({ "status": "sent", "to": "owner@example.com" })
    );
    app.wait_for_mail("owner@example.com", before + 1).await;
    assert_eq!(
        app.mail
            .to("owner@example.com")
            .last()
            .unwrap()
            .kind
            .as_str(),
        "test"
    );

    let again = app
        .post("/me/notifications/test", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(
        again.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{:?}",
        again.body
    );
    assert!(
        again.body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("You sent a test email a moment ago; try again in ")
    );
}

#[tokio::test]
async fn an_unsubscribe_link_turns_one_kind_off_without_a_session() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let user: UserId = owner.user_id.parse().unwrap();
    let token = unsubscribe_token(&app.state, user, Kind::OrgMembers);
    let link = format!("/notifications/unsubscribe?token={token}");

    let shown = app
        .call_with(Method::GET, &link, None, None, None)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        shown.body,
        json!({ "kind": "org_members", "name": "Member changes", "email": "owner@example.com", "delivery": "daily" })
    );

    let one_click = Request::builder()
        .method(Method::POST)
        .uri(format!("/platform/v1{link}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("List-Unsubscribe=One-Click"))
        .unwrap();
    let r = app.raw(one_click).await.expect_status(StatusCode::OK);
    assert_eq!(r.body["delivery"], "off");
    let mine = app.get("/me/notifications", &owner.cookie).await;
    assert_eq!(kind(&mine.body, "org_members")["delivery"], "off");
    assert_eq!(kind(&mine.body, "invite_accepted")["delivery"], "now");

    let mut forged = token.clone();
    forged.pop();
    forged.push(if token.ends_with('A') { 'B' } else { 'A' });
    let security = unsubscribe_token(&app.state, user, Kind::AccountSecurity);
    for bad in [forged, security, "nonsense".to_owned()] {
        let r = app
            .call_with(
                Method::POST,
                &format!("/notifications/unsubscribe?token={bad}"),
                None,
                None,
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}: {:?}", r.body);
        assert_eq!(
            r.body["error"]["message"],
            "This unsubscribe link is invalid; choose your emails under Account › Notifications."
        );
    }
}
