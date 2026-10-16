#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const PASSWORD: &str = "correct horse battery";
const INVITATION_ONLY: &str =
    "Accounts on this install are by invitation only; ask an administrator to invite you.";
const NOT_ALLOWED: &str =
    "This email address can't create an account here; ask an administrator to invite you.";

async fn settings(app: &App, owner: &Owner, body: Value) -> common::Resp {
    app.call(
        Method::PATCH,
        "/instance/settings",
        Some(body),
        Some(&owner.cookie),
    )
    .await
}

async fn set(app: &App, owner: &Owner, body: Value) -> Value {
    let r = settings(app, owner, body).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

async fn offers_sign_up(app: &App, email: &str) -> bool {
    let r = app
        .post("/auth/methods", json!({ "email": email }), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["sign_up"].as_bool().unwrap()
}

async fn install_offers_sign_up(app: &App) -> bool {
    let r = app.call(Method::GET, "/auth/methods", None, None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["sign_up"].as_bool().unwrap()
}

async fn sign_up(app: &App, email: &str) -> (StatusCode, Option<String>) {
    let r = app
        .post("/auth/signup", json!({ "email": email }), None)
        .await;
    let message = r.body["error"]["message"].as_str().map(str::to_owned);
    (r.status, message)
}

async fn finish(app: &App, email: &str) -> common::Resp {
    let token = app.last_token(email);
    app.post(
        "/auth/signup/verify",
        json!({ "token": token, "name": "Ada", "password": PASSWORD }),
        None,
    )
    .await
}

async fn fresh_sign_in(app: &App, email: &str) -> String {
    let r = app
        .post(
            "/auth/password",
            json!({ "email": email, "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.cookie().unwrap()
}

#[tokio::test]
async fn invitation_only_is_the_default_and_refuses_every_sign_up() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = app.get("/instance/settings", &owner.cookie).await;
    assert_eq!(
        (
            r.body["account_creation"].as_str(),
            r.body["org_creation"].as_str(),
            r.body["allowed_domains"].as_array().map(Vec::len),
        ),
        (Some("invitation_only"), Some("instance_admins"), Some(0))
    );
    assert!(r.body["sign_in"].get("public_signup").is_none());
    assert!(!install_offers_sign_up(&app).await);
    assert!(!offers_sign_up(&app, "ada@example.com").await);
    assert!(!offers_sign_up(&app, "owner@example.com").await);
    assert_eq!(
        sign_up(&app, "ada@example.com").await,
        (StatusCode::FORBIDDEN, Some(INVITATION_ONLY.to_owned()))
    );
    assert_eq!(
        sign_up(&app, "owner@example.com").await,
        (StatusCode::FORBIDDEN, Some(INVITATION_ONLY.to_owned()))
    );
    assert!(app.mail.to("ada@example.com").is_empty());
    s.cleanup().await;
}

#[tokio::test]
async fn everyone_lets_any_address_create_an_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let after = set(&app, &owner, json!({ "account_creation": "everyone" })).await;
    assert_eq!(after["account_creation"], "everyone");
    assert!(install_offers_sign_up(&app).await);
    assert!(offers_sign_up(&app, "ada@anywhere.example").await);
    assert_eq!(
        sign_up(&app, "ada@anywhere.example").await,
        (StatusCode::OK, None)
    );
    app.wait_for_mail("ada@anywhere.example", 1).await;
    let r = finish(&app, "ada@anywhere.example").await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["session"]["method"], "signup");
    let verified: bool = sqlx::query_scalar(
        "SELECT email_verified_at IS NOT NULL FROM users WHERE email = 'ada@anywhere.example'",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert!(verified);
    s.cleanup().await;
}

#[tokio::test]
async fn allowed_domains_decide_by_the_address_domain_alone() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let after = set(
        &app,
        &owner,
        json!({
            "account_creation": "allowed_domains",
            "allowed_domains": ["Example.COM", "*.corp.example", "BÜCHER.example", "example.com."],
        }),
    )
    .await;
    assert_eq!(
        after["allowed_domains"],
        json!(["*.corp.example", "example.com", "bücher.example"])
    );
    assert!(install_offers_sign_up(&app).await);
    for (email, allowed) in [
        ("ada@example.com", true),
        ("Ada@EXAMPLE.com", true),
        ("ada@eu.example.com", false),
        ("ada@badexample.com", false),
        ("ada@example.com.attacker.example", false),
        ("ada@eu.corp.example", true),
        ("ada@a.b.corp.example", true),
        ("ada@corp.example", false),
        ("ada@bücher.example", true),
        ("ada@BÜCHER.example", true),
        ("ada@xn--bcher-kva.example", true),
        ("ada@bucher.example", false),
    ] {
        assert_eq!(offers_sign_up(&app, email).await, allowed, "{email}");
    }
    let r = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@other.example" }),
            None,
        )
        .await;
    assert!(!r.body.to_string().contains("corp.example"), "{:?}", r.body);

    assert_eq!(
        sign_up(&app, "ada@other.example").await,
        (StatusCode::FORBIDDEN, Some(NOT_ALLOWED.to_owned()))
    );
    assert_eq!(
        sign_up(&app, "grace@EU.corp.example").await,
        (StatusCode::OK, None)
    );
    app.wait_for_mail("grace@eu.corp.example", 1).await;
    let r = finish(&app, "grace@eu.corp.example").await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["user"]["email"], "grace@eu.corp.example");
    assert_eq!(
        sign_up(&app, "ada@bücher.example").await,
        (StatusCode::OK, None)
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_sign_up_link_follows_the_setting_when_it_is_used() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    set(&app, &owner, json!({ "account_creation": "everyone" })).await;
    assert_eq!(
        sign_up(&app, "ada@example.com").await,
        (StatusCode::OK, None)
    );
    app.wait_for_mail("ada@example.com", 1).await;
    set(
        &app,
        &owner,
        json!({ "account_creation": "invitation_only" }),
    )
    .await;
    let r = finish(&app, "ada@example.com").await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str()),
        (StatusCode::FORBIDDEN, Some(INVITATION_ONLY))
    );
    set(
        &app,
        &owner,
        json!({ "account_creation": "allowed_domains", "allowed_domains": ["example.com"] }),
    )
    .await;
    let r = finish(&app, "ada@example.com").await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn invitations_create_accounts_under_every_setting() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let accept = async |email: &str| {
        app.wait_for_mail(email, 1).await;
        let token = app.last_token(email);
        app.post(
            "/auth/invites/accept",
            json!({ "token": token, "name": "Invited", "password": PASSWORD }),
            None,
        )
        .await
    };
    for (setting, email) in [
        (
            json!({ "account_creation": "invitation_only" }),
            "org@elsewhere.example",
        ),
        (
            json!({ "account_creation": "allowed_domains", "allowed_domains": ["example.com"] }),
            "org@other.example",
        ),
    ] {
        set(&app, &owner, setting).await;
        assert!(!offers_sign_up(&app, email).await);
        let r = app
            .post(
                &format!("/orgs/{}/invites", owner.org_id),
                json!({ "email": email, "preset": "member" }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        let r = accept(email).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(r.body["user"]["email"], email);
    }
    let r = app
        .post(
            "/instance/users/invites",
            json!({ "email": "solo@elsewhere.example" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = accept("solo@elsewhere.example").await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn org_creation_can_follow_the_allowed_domains() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    set(
        &app,
        &owner,
        json!({ "account_creation": "everyone", "org_creation": "allowed_domains", "allowed_domains": ["*.example.com"] }),
    )
    .await;
    let mut cookies = Vec::new();
    for email in ["ada@eu.example.com", "bob@elsewhere.example"] {
        assert_eq!(sign_up(&app, email).await, (StatusCode::OK, None));
        app.wait_for_mail(email, 1).await;
        let r = finish(&app, email).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        cookies.push(r.cookie().unwrap());
    }
    let can = async |cookie: &str| {
        app.get("/auth/session", cookie).await.body["can_create_orgs"]
            .as_bool()
            .unwrap()
    };
    assert!(can(&cookies[0]).await);
    assert!(!can(&cookies[1]).await);
    assert!(can(&owner.cookie).await);
    let r = app
        .post("/me/orgs", json!({ "name": "Ada's" }), Some(&cookies[0]))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = app
        .post("/me/orgs", json!({ "name": "Bob's" }), Some(&cookies[1]))
        .await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str()),
        (
            StatusCode::FORBIDDEN,
            Some("Your email address can't create organizations here; ask an instance admin.")
        )
    );
    set(&app, &owner, json!({ "org_creation": "instance_admins" })).await;
    assert!(!can(&cookies[0]).await);
    let r = app
        .post("/me/orgs", json!({ "name": "Again" }), Some(&cookies[0]))
        .await;
    assert_eq!(
        r.body["error"]["message"],
        "Only instance admins can create organizations on this install."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_allowed_domains_are_checked_before_they_are_kept() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let refused = async |body: Value| {
        let r = settings(&app, &owner, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        (
            r.body["error"]["param"].as_str().map(str::to_owned),
            r.body["error"]["message"].as_str().unwrap().to_owned(),
        )
    };
    assert_eq!(
        refused(json!({ "account_creation": "allowed_domains" })).await,
        (
            Some("allowed_domains".into()),
            "Add at least one allowed domain to use it.".into()
        )
    );
    assert_eq!(
        refused(json!({ "org_creation": "allowed_domains", "allowed_domains": [] })).await,
        (
            Some("allowed_domains".into()),
            "Add at least one allowed domain to use it.".into()
        )
    );
    assert_eq!(
        refused(json!({ "allowed_domains": ["example.com", "https://example.org"] })).await,
        (
            Some("allowed_domains[1]".into()),
            "`https://example.org` isn't a domain; use example.com or *.example.com.".into()
        )
    );
    let many: Vec<String> = (0..101).map(|i| format!("d{i}.example")).collect();
    assert_eq!(
        refused(json!({ "allowed_domains": many })).await.1,
        "Allow at most 100 domains."
    );
    set(
        &app,
        &owner,
        json!({ "account_creation": "allowed_domains", "allowed_domains": ["example.com"] }),
    )
    .await;
    assert_eq!(
        refused(json!({ "allowed_domains": [] })).await.0,
        Some("allowed_domains".into())
    );
    let r = set(
        &app,
        &owner,
        json!({ "account_creation": "everyone", "allowed_domains": [] }),
    )
    .await;
    assert_eq!(r["allowed_domains"], json!([]));
    s.cleanup().await;
}

#[tokio::test]
async fn changes_need_a_recent_sign_in_and_are_audited() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let mut owner = app.setup().await;
    app.clock.advance(time::Duration::minutes(20));
    for body in [
        json!({ "account_creation": "everyone" }),
        json!({ "allowed_domains": ["example.com"] }),
        json!({ "org_creation": "everyone" }),
    ] {
        let r = settings(&app, &owner, body).await;
        assert_eq!(r.error_type(), "recent_sign_in_required", "{:?}", r.body);
    }
    set(&app, &owner, json!({ "org_deletion_grace_days": 9 })).await;

    owner.cookie = fresh_sign_in(&app, "owner@example.com").await;
    set(
        &app,
        &owner,
        json!({ "account_creation": "allowed_domains", "allowed_domains": ["bücher.example"], "org_creation": "allowed_domains" }),
    )
    .await;
    let log = app
        .get(
            "/instance/audit-log?action=instance.settings_update",
            &owner.cookie,
        )
        .await;
    assert_eq!(log.status, StatusCode::OK, "{:?}", log.body);
    let event = &log.body["data"][0];
    assert_eq!(event["action"], "instance.settings_update");
    assert_eq!(event["actor"]["kind"], "user");
    assert_eq!(event["before"]["account_creation"], "invitation_only");
    assert_eq!(event["before"]["org_creation"], "instance_admins");
    assert_eq!(event["before"]["allowed_domains"], json!([]));
    assert_eq!(event["after"]["account_creation"], "allowed_domains");
    assert_eq!(event["after"]["org_creation"], "allowed_domains");
    assert_eq!(event["after"]["allowed_domains"], json!(["bücher.example"]));
    let stored: Vec<String> = sqlx::query_scalar("SELECT allowed_domains FROM instance_settings")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(stored, ["xn--bcher-kva.example"]);
    s.cleanup().await;
}

#[tokio::test]
async fn an_upgraded_install_keeps_who_could_sign_up() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let stored = async |app: &App| -> Option<String> {
        sqlx::query_scalar("SELECT account_creation FROM instance_settings")
            .fetch_one(&app.state.pg)
            .await
            .unwrap()
    };
    let forget = async |app: &App| {
        sqlx::query("UPDATE instance_settings SET account_creation = NULL")
            .execute(&app.state.pg)
            .await
            .unwrap();
    };
    forget(&app).await;
    assert!(!install_offers_sign_up(&app).await);
    let open = App::start_with(&s, &[("KAESO_INSTANCE_ACCOUNT_CREATION", "everyone")]).await;
    assert_eq!(stored(&open).await.as_deref(), Some("everyone"));
    assert!(offers_sign_up(&open, "ada@example.com").await);
    let again = App::start(&s).await;
    assert_eq!(stored(&again).await.as_deref(), Some("everyone"));
    forget(&again).await;
    let plain = App::start(&s).await;
    assert_eq!(stored(&plain).await.as_deref(), Some("invitation_only"));
    s.cleanup().await;
}
