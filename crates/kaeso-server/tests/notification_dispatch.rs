#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_core::notifications::Kind;
use kaeso_mail::notices::{Because, Fact};
use kaeso_server::services::notifications::dispatch::{Recipients, Send, deliver};
use kaeso_server::services::notifications::system;
use kaeso_testkit::services::TestServices;
use serde_json::json;
use time::macros::time;
use uuid::Uuid;

const OWNER: &str = "owner@example.com";

fn kinds(app: &App, to: &str) -> Vec<String> {
    app.mail
        .to(to)
        .iter()
        .map(|m| m.kind.as_str().to_owned())
        .collect()
}

async fn settle(app: &App) {
    app.state.in_flight.settled().await;
    app.state.mailer.flush().await;
}

async fn choose(app: &App, cookie: &str, kinds: serde_json::Value) {
    app.call(
        Method::PUT,
        "/me/notifications",
        Some(json!({ "kinds": kinds })),
        Some(cookie),
    )
    .await
    .expect_status(StatusCode::OK);
}

fn to(user: Uuid) -> Recipients {
    Recipients::from([(user, Because::Right("tools.manage"))])
}

fn send(kind: Kind, subject: &str, org: Option<Uuid>, actor: Option<Uuid>) -> Send<'static> {
    Send {
        kind,
        org,
        project: None,
        place: Some("Acme"),
        subject: subject.to_owned(),
        open: "orgs".to_owned(),
        actor,
    }
}

async fn run_notices(app: &App) -> String {
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "notifications")
        .unwrap();
    kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap()
}

fn next_morning(app: &App) {
    let now = app.state.clock.now();
    let tomorrow = now.date().next_day().unwrap();
    app.clock.set(tomorrow.with_time(time!(8:32)).assume_utc());
}

#[tokio::test]
async fn the_actor_never_hears_about_their_own_change_but_security_emails_always_arrive() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let me = common::uuid(&owner.user_id);
    let fact = Fact::PortalClient {
        client: "ChatGPT",
        by: kaeso_mail::Actor::Person("Ada"),
    };

    let n = deliver(
        &app.state,
        &send(Kind::PortalClients, "a", None, Some(me)),
        &fact,
        to(me),
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "the person who connected the client is not told");
    let app_fact = Fact::NewApp { app: "Claude" };
    let n = deliver(
        &app.state,
        &send(Kind::NewApp, "b", None, Some(me)),
        &app_fact,
        to(me),
    )
    .await
    .unwrap();
    assert_eq!(n, 1, "a security email reaches the person who caused it");
    settle(&app).await;
    let mails = app.mail.to(OWNER);
    let last = mails.last().unwrap();
    assert_eq!(last.kind.as_str(), "new_app");
    assert_eq!(
        last.list_unsubscribe, None,
        "security emails have no unsubscribe link"
    );
}

#[tokio::test]
async fn settings_mutes_and_dedupe_decide_what_goes_out() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let me = common::uuid(&owner.user_id);
    let org = common::uuid(&owner.org_id);
    let fact = Fact::PortalClient {
        client: "ChatGPT",
        by: kaeso_mail::Actor::Person("Ada"),
    };
    let portal = |subject: &'static str| send(Kind::PortalClients, subject, Some(org), None);

    choose(&app, &owner.cookie, json!({ "portal_clients": "now" })).await;
    assert_eq!(
        deliver(&app.state, &portal("one"), &fact, to(me))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        deliver(&app.state, &portal("one"), &fact, to(me))
            .await
            .unwrap(),
        0,
        "one email per subject and day"
    );
    settle(&app).await;
    let mail = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "portal_client")
        .unwrap();
    assert_eq!(mail.subject, "[Acme] ChatGPT was connected");
    let one_click = mail.list_unsubscribe.clone().unwrap();
    assert!(
        one_click.starts_with("https://kaeso.test/platform/v1/notifications/unsubscribe?token="),
        "{one_click}"
    );
    assert!(
        mail.text
            .contains("You get this email because you hold tools.manage."),
        "{}",
        mail.text
    );
    assert!(
        mail.text.contains("https://kaeso.test/unsubscribe#token="),
        "{}",
        mail.text
    );

    choose(&app, &owner.cookie, json!({ "portal_clients": "off" })).await;
    assert_eq!(
        deliver(&app.state, &portal("two"), &fact, to(me))
            .await
            .unwrap(),
        0
    );

    choose(&app, &owner.cookie, json!({ "portal_clients": "now" })).await;
    app.call(
        Method::PUT,
        &format!("/me/memberships/{}/notifications", owner.org_id),
        Some(json!({ "muted": true })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(
        deliver(&app.state, &portal("three"), &fact, to(me))
            .await
            .unwrap(),
        0,
        "a muted org sends nothing"
    );
    let security = send(Kind::NewApp, "four", Some(org), None);
    assert_eq!(
        deliver(
            &app.state,
            &security,
            &Fact::NewApp { app: "Claude" },
            to(me)
        )
        .await
        .unwrap(),
        1,
        "security emails ignore the mute"
    );
}

#[tokio::test]
async fn daily_emails_and_those_over_the_hourly_cap_wait_for_the_morning_digest() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let me = common::uuid(&owner.user_id);
    let fact = Fact::PortalClient {
        client: "ChatGPT",
        by: kaeso_mail::Actor::Person("Ada"),
    };
    choose(&app, &owner.cookie, json!({ "portal_clients": "now" })).await;
    for i in 0..21 {
        let subject: &'static str = Box::leak(format!("burst-{i}").into_boxed_str());
        deliver(
            &app.state,
            &send(Kind::PortalClients, subject, None, None),
            &fact,
            to(me),
        )
        .await
        .unwrap();
    }
    settle(&app).await;
    assert_eq!(
        kinds(&app, OWNER)
            .iter()
            .filter(|k| *k == "portal_client")
            .count(),
        20
    );

    choose(&app, &owner.cookie, json!({ "portal_clients": "daily" })).await;
    let other = Fact::PortalClient {
        client: "Cursor",
        by: kaeso_mail::Actor::Person("Ada"),
    };
    deliver(
        &app.state,
        &send(Kind::PortalClients, "daily", None, None),
        &other,
        to(me),
    )
    .await
    .unwrap();
    settle(&app).await;
    assert_eq!(
        kinds(&app, OWNER).iter().filter(|k| *k == "digest").count(),
        0
    );

    next_morning(&app);
    assert!(run_notices(&app).await.starts_with("1 digests"));
    assert!(
        run_notices(&app).await.starts_with("0 digests"),
        "a digest goes out once"
    );
    settle(&app).await;
    let digest = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "digest")
        .unwrap();
    assert_eq!(digest.subject, "2 updates from Kaeso");
    assert!(
        digest.text.contains("- [Acme] ChatGPT was connected"),
        "{}",
        digest.text
    );
    assert!(
        digest.text.contains("- [Acme] Cursor was connected"),
        "{}",
        digest.text
    );
}

async fn second(app: &App, owner: &Owner, email: &str, role: &str) -> (String, Uuid) {
    let (cookie, id) = common::join_first_project(
        app,
        owner,
        json!({ "email": email, "role": role }),
        "Grace Hopper",
    )
    .await;
    (cookie, common::uuid(&id))
}

#[tokio::test]
async fn joining_tells_the_inviter_at_once_and_admins_in_the_digest() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    second(&app, &owner, "grace@example.com", "admin").await;
    settle(&app).await;
    let accepted = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "invite_accepted")
        .unwrap();
    assert_eq!(accepted.subject, "[Acme] Grace Hopper accepted");
    assert!(
        !kinds(&app, OWNER).contains(&"member_changes".to_owned()),
        "member changes default to daily"
    );
    next_morning(&app);
    run_notices(&app).await;
    settle(&app).await;
    let digest = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "digest")
        .unwrap();
    assert!(
        digest.text.contains("Grace Hopper joined"),
        "{}",
        digest.text
    );
    assert!(
        app.mail
            .to("grace@example.com")
            .iter()
            .all(|m| m.kind.as_str() != "member_changes"),
        "the person who joined isn't told"
    );
}

#[tokio::test]
async fn changes_by_someone_else_reach_the_person_they_are_about() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let (grace, _) = second(&app, &owner, "grace@example.com", "admin").await;
    let at = format!("/orgs/{}/projects/{project}", owner.org_id);
    let key = app
        .post(
            &format!("{at}/keys"),
            json!({ "name": "ci-deploy" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    app.post(
        &format!("{at}/keys/{}/rotate", key.body["id"].as_str().unwrap()),
        json!({ "grace_hours": 0 }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    settle(&app).await;
    assert!(
        !kinds(&app, OWNER).contains(&"key_changed".to_owned()),
        "rotating your own key tells nobody"
    );

    let other = app
        .post(
            &format!("{at}/keys"),
            json!({ "name": "nightly" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    app.post(
        &format!("{at}/keys/{}/disable", other.body["id"].as_str().unwrap()),
        json!({}),
        Some(&grace),
    )
    .await
    .expect_status(StatusCode::OK);
    settle(&app).await;
    let changed = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "key_changed")
        .unwrap();
    assert!(
        changed
            .text
            .contains("Grace Hopper disabled your key nightly."),
        "{}",
        changed.text
    );
}

#[tokio::test]
async fn the_cost_report_goes_out_on_the_first_at_831() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let _owner = app.setup().await;
    let now = app.state.clock.now();
    let first = now.date().replace_day(1).unwrap();
    let next = if now.date().day() == 1 && now.time() < time!(8:31) {
        first
    } else {
        let in_next_month = first + time::Duration::days(32);
        in_next_month.replace_day(1).unwrap()
    };
    app.clock.set(next.with_time(time!(8:30)).assume_utc());
    assert!(!run_notices(&app).await.contains("cost reports"));
    app.clock.set(next.with_time(time!(8:31)).assume_utc());
    assert!(
        run_notices(&app).await.contains("0 cost reports"),
        "no usage, no report"
    );
    assert!(
        !run_notices(&app).await.contains("cost reports"),
        "once a day"
    );
}

#[tokio::test]
async fn being_added_to_a_project_tells_the_person_added() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (_, grace) = second(&app, &owner, "grace@example.com", "member").await;
    let other = common::create_project(&app, &owner.org_id, &owner.cookie, "Support").await;
    app.post(
        &format!("/orgs/{}/projects/{other}/members", owner.org_id),
        json!({ "user_id": kaeso_core::id::UserId::from_uuid(grace).to_string(), "role": "developer" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    settle(&app).await;
    let added = app
        .mail
        .to("grace@example.com")
        .into_iter()
        .find(|m| m.kind.as_str() == "access_changed")
        .unwrap();
    assert_eq!(added.subject, "[Support] You were added");
    assert!(
        added.text.contains("Ada Lovelace added you as Developer.")
            || added.text.contains("added you as Developer."),
        "{}",
        added.text
    );
}

#[tokio::test]
async fn a_provider_key_refused_three_times_in_a_row_needs_attention_once() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = common::uuid(&owner.org_id);
    let key = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            json!({ "provider": "openai", "name": "OpenAI prod", "secret": "sk-test-org-secret-0123456789WXYZ" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let credential = common::uuid(key.body["id"].as_str().unwrap());
    system::key_refused(&app.state, org, credential).await;
    system::key_refused(&app.state, org, credential).await;
    system::key_answered(credential);
    system::key_refused(&app.state, org, credential).await;
    system::key_refused(&app.state, org, credential).await;
    settle(&app).await;
    assert!(
        !kinds(&app, OWNER).contains(&"provider_keys".to_owned()),
        "an answer in between resets the count"
    );
    system::key_refused(&app.state, org, credential).await;
    system::key_refused(&app.state, org, credential).await;
    settle(&app).await;
    let mails: Vec<_> = app
        .mail
        .to(OWNER)
        .into_iter()
        .filter(|m| m.kind.as_str() == "provider_keys")
        .collect();
    assert_eq!(mails.len(), 1);
    assert_eq!(mails[0].subject, "[Acme] OpenAI prod keeps being refused");
}

#[tokio::test]
async fn a_signed_out_mcp_connection_reaches_its_admins_or_its_person() {
    let s = TestServices::start().await;
    let mock = kaeso_testkit::mcp::MockMcp::start(kaeso_testkit::mcp::Options::default()).await;
    let app = App::start_with(&s, &[("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8")]).await;
    let owner = app.setup().await;
    let org = common::uuid(&owner.org_id);
    let created = app
        .post(
            &format!("/orgs/{}/mcp-servers", owner.org_id),
            json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let server = kaeso_store::pg::mcp_servers::get(
        &app.state.pg,
        org,
        common::uuid(created.body["id"].as_str().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    system::server_signed_out(&app.state, &server, None).await;
    system::server_signed_out(&app.state, &server, Some(common::uuid(&owner.user_id))).await;
    system::tools_held(&app.state, &server, &["create_post".to_owned()]).await;
    settle(&app).await;
    let subjects: Vec<String> = app
        .mail
        .to(OWNER)
        .iter()
        .map(|m| m.subject.clone())
        .collect();
    assert!(
        subjects.contains(&"[Acme] X signed Kaeso out".to_owned()),
        "{subjects:?}"
    );
    assert!(
        subjects.contains(&"[Acme] X signed you out".to_owned()),
        "{subjects:?}"
    );
    assert!(
        subjects.contains(&"[Acme] create_post changed".to_owned()),
        "{subjects:?}"
    );
}

#[tokio::test]
async fn a_finished_audit_log_export_is_confirmed_to_whoever_ran_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = app
        .get(
            &format!("/orgs/{}/audit-log/export", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    settle(&app).await;
    let done = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "export_ready")
        .unwrap();
    assert_eq!(done.subject, "[Acme] Your export finished");
}
