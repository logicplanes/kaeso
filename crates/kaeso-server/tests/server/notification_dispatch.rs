#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use crate::common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_core::notifications::Kind;
use kaeso_mail::notices::{Because, Fact};
use kaeso_server::services::notifications::dispatch::{Recipients, Send, deliver};
use kaeso_server::services::notifications::system::{self, Refusal};
use kaeso_store::pg::limits::UsageRow;
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
            .contains("You get this email because you hold the right “Manage tools”."),
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
    let support = common::create_project(&app, &owner.org_id, &owner.cookie, "Support").await;
    let in_support = Send {
        org: Some(common::uuid(&owner.org_id)),
        project: Some(common::uuid(&support)),
        place: Some("Support"),
        ..send(Kind::PortalClients, "daily", None, None)
    };
    deliver(&app.state, &in_support, &other, to(me))
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
        digest
            .text
            .contains("Acme\n- ChatGPT was connected\nAcme / Support\n- Cursor was connected\n"),
        "{}",
        digest.text
    );
    assert!(!digest.text.contains('['), "{}", digest.text);
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
            json!({ "provider": "openai", "name": "Production", "secret": "sk-test-org-secret-0123456789WXYZ" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let credential = common::uuid(key.body["id"].as_str().unwrap());
    let refused = Refusal {
        status: 401,
        message: "Incorrect API key provided: sk-te***WXYZ.",
        billing: false,
    };
    system::key_refused(&app.state, org, credential, refused).await;
    system::key_refused(&app.state, org, credential, refused).await;
    system::key_answered(credential);
    system::key_refused(&app.state, org, credential, refused).await;
    system::key_refused(&app.state, org, credential, refused).await;
    settle(&app).await;
    assert!(
        !kinds(&app, OWNER).contains(&"provider_keys".to_owned()),
        "an answer in between resets the count"
    );
    app.clock.advance(time::Duration::minutes(2));
    system::key_refused(&app.state, org, credential, refused).await;
    system::key_refused(&app.state, org, credential, refused).await;
    settle(&app).await;
    let mails: Vec<_> = app
        .mail
        .to(OWNER)
        .into_iter()
        .filter(|m| m.kind.as_str() == "provider_keys")
        .collect();
    assert_eq!(mails.len(), 1);
    assert_eq!(
        mails[0].subject,
        "[Acme] OpenAI key Production keeps being refused"
    );
    let text = &mails[0].text;
    for line in [
        "OpenAI refused the key Production (…WXYZ) three times in a row; replace it.",
        "Organization: Acme\n",
        "Provider: OpenAI\n",
        "Key: Production (…WXYZ)\n",
        "Used by: Every project without a key of its own\n",
        "Provider's answer: HTTP 401: Incorrect API key provided: sk-te***WXYZ.\n",
        "Last used: never\n",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
    let at = |label: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(label))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert!(at("First refused: ").ends_with(" UTC"), "{text}");
    assert_ne!(
        at("First refused: "),
        at("Last refused: "),
        "the third refusal came two minutes after the first\n{text}"
    );
    assert!(
        text.contains(&format!("{}/provider-keys\n", owner.org_id)),
        "{text}"
    );
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

fn usage(day: time::Date, org: Uuid, project: Uuid, requests: i64, cost: &str) -> UsageRow {
    UsageRow {
        day,
        org_id: org,
        project_id: project,
        user_id: None,
        service_account_id: None,
        api_key_id: None,
        catalogue_model_id: None,
        model_id: "claude-haiku-4-5".into(),
        provider: "anthropic".into(),
        ingress_dialect: "anthropic_messages".into(),
        egress_dialect: "anthropic_messages".into(),
        requests,
        errors: 0,
        input_tokens: 10,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        cost: cost.parse().unwrap(),
        media: Default::default(),
    }
}

#[tokio::test]
async fn the_weekly_summary_names_each_organization_and_the_week_and_links_to_usage() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup_without_projects().await;
    let checkout = common::create_project(&app, &owner.org_id, &owner.cookie, "Checkout").await;
    let globex = app
        .post("/me/orgs", json!({ "name": "Globex" }), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let research = common::create_project(&app, &globex, &owner.cookie, "Research").await;
    choose(&app, &owner.cookie, json!({ "weekly_summary": "now" })).await;
    let mut monday = app.state.clock.now().date().next_day().unwrap();
    while monday.weekday() != time::Weekday::Monday {
        monday = monday.next_day().unwrap();
    }
    let (from, to) = (
        monday - time::Duration::days(7),
        monday.previous_day().unwrap(),
    );
    let day = monday - time::Duration::days(3);
    kaeso_store::pg::limits::add_usage(
        &app.state.pg,
        &[
            usage(
                day,
                common::uuid(&owner.org_id),
                common::uuid(&checkout),
                9120,
                "30.10",
            ),
            usage(
                day,
                common::uuid(&globex),
                common::uuid(&research),
                2400,
                "8.60",
            ),
        ],
    )
    .await
    .unwrap();
    app.clock.set(monday.with_time(time!(8:31)).assume_utc());
    assert!(
        run_notices(&app).await.contains("1 weekly summaries"),
        "one summary for the person"
    );
    settle(&app).await;
    let week = app
        .mail
        .to(OWNER)
        .into_iter()
        .find(|m| m.kind.as_str() == "weekly_summary")
        .unwrap();
    let range = kaeso_mail::Locale::En.range(from, to);
    let base = app.state.config.public_url.as_str();
    for line in [
        format!("Here is how your projects used Kaeso, {range}.\n"),
        "Acme\n- Checkout: 9,120 requests, $30.10\n".to_owned(),
        "Globex\n- Research: 2,400 requests, $8.60\n".to_owned(),
        format!(
            "Open usage:\n{base}orgs/{}/analytics?from={from}&to={to}&by=project\n",
            owner.org_id
        ),
    ] {
        assert!(week.text.contains(&line), "{line}\n{}", week.text);
    }
    s.cleanup().await;
}
