#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::{App, Owner, PASSWORD};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const SSO_ONLY: &str =
    "Your organization requires single sign-on; continue with its identity provider.";

struct Team {
    owner: Owner,
    ada: String,
    bob: String,
}

async fn team(app: &App) -> Team {
    let owner = app.setup().await;
    let (ada, _) = common::join(
        app,
        &owner,
        json!({ "email": "ada@acme.example", "role": "member" }),
        "Ada",
    )
    .await;
    let (bob, _) = common::join(
        app,
        &owner,
        json!({ "email": "bob@else.example", "role": "member" }),
        "Bob",
    )
    .await;
    common::manage_domain(app, &owner.org_id, "acme.example").await;
    Team { owner, ada, bob }
}

async fn require_sso(app: &App, team: &Team, on: bool) -> common::Resp {
    app.call(
        Method::PUT,
        &format!("/orgs/{}/security/require-sso", team.owner.org_id),
        Some(json!({ "enabled": on })),
        Some(&team.owner.cookie),
    )
    .await
}

async fn password(app: &App, email: &str) -> common::Resp {
    app.post(
        "/auth/password",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await
}

#[tokio::test]
async fn the_org_lists_whom_it_manages_and_who_is_a_guest() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let team = team(&app).await;
    let members = app
        .get(
            &format!("/orgs/{}/members", team.owner.org_id),
            &team.owner.cookie,
        )
        .await
        .expect_status(StatusCode::OK);
    let managed: std::collections::BTreeMap<String, bool> = members.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["user"]["email"].as_str().unwrap().to_owned(),
                m["managed"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        managed.into_iter().collect::<Vec<_>>(),
        [
            ("ada@acme.example".to_owned(), true),
            ("bob@else.example".to_owned(), false),
            ("owner@example.com".to_owned(), false),
        ]
    );

    let who = app.get("/auth/session", &team.ada).await;
    assert_eq!(who.body["managed_by"]["org"], team.owner.org_id.as_str());
    assert_eq!(who.body["managed_by"]["name"], "Acme");
    assert_eq!(who.body["memberships"][0]["sso"], true);
    let who = app.get("/auth/session", &team.bob).await;
    assert_eq!(who.body["managed_by"], Value::Null);

    let users = app
        .get("/instance/users", &team.owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    for u in users.body["data"].as_array().unwrap() {
        let expected = if u["email"] == "ada@acme.example" {
            json!({ "org": team.owner.org_id, "name": "Acme" })
        } else {
            Value::Null
        };
        assert_eq!(u["managed_by"], expected, "{}", u["email"]);
    }

    sqlx::query("DELETE FROM org_oidc")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let who = app.get("/auth/session", &team.ada).await;
    assert_eq!(
        who.body["managed_by"],
        Value::Null,
        "without a connection the org manages nobody"
    );
    assert_eq!(who.body["memberships"][0]["sso"], false);
    s.cleanup().await;
}

#[tokio::test]
async fn requiring_sso_takes_password_and_link_sign_in_from_managed_accounts_only() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let team = team(&app).await;
    let r = require_sso(&app, &team, true).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    assert_eq!(
        app.get("/auth/session", &team.ada).await.status,
        StatusCode::UNAUTHORIZED,
        "a managed account's password session ends"
    );
    let ended: Value = sqlx::query_scalar(
        "SELECT after FROM audit_events WHERE action = 'security.require_sso' ORDER BY at DESC LIMIT 1",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(ended["sessions_ended"], 1);

    let r = password(&app, "ada@acme.example").await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str().unwrap()),
        (StatusCode::FORBIDDEN, SSO_ONLY)
    );
    let r = app
        .post(
            "/auth/magic-link",
            json!({ "email": "Ada@Acme.example" }),
            None,
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str().unwrap()),
        (StatusCode::FORBIDDEN, SSO_ONLY)
    );
    let methods = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@acme.example" }),
            None,
        )
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(methods.body["sso_required"], true);
    assert_eq!(methods.body["password"], false);
    assert_eq!(methods.body["provider"]["org_name"], "Acme");

    let bob = password(&app, "bob@else.example")
        .await
        .expect_status(StatusCode::OK)
        .cookie()
        .unwrap();
    app.get(&format!("/orgs/{}", team.owner.org_id), &bob)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        app.get(&format!("/orgs/{}", team.owner.org_id), &team.bob)
            .await
            .status,
        StatusCode::OK,
        "a guest's session stays and opens the org"
    );
    app.get(&format!("/orgs/{}", team.owner.org_id), &team.owner.cookie)
        .await
        .expect_status(StatusCode::OK);

    let r = require_sso(&app, &team, false).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    password(&app, "ada@acme.example")
        .await
        .expect_status(StatusCode::OK);
    s.cleanup().await;
}

#[tokio::test]
async fn the_requiring_admins_own_session_stays_and_the_org_closes_to_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let team = team(&app).await;
    common::manage_domain(&app, &team.owner.org_id, "example.com").await;
    let r = require_sso(&app, &team, true).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let who = app.get("/auth/session", &team.owner.cookie).await;
    assert_eq!(who.status, StatusCode::OK);
    assert_eq!(who.body["memberships"][0]["access"], "needs_sso");
    assert_eq!(
        who.body["can_administer_instance"], true,
        "the account itself stays usable, like a sign-in link from the server"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_sign_in_link_from_the_server_still_signs_a_managed_account_in() {
    use kaeso_server::services::users::{OperatorSignIn, operator_sign_in_link};
    use secrecy::ExposeSecret;

    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let team = team(&app).await;
    require_sso(&app, &team, true)
        .await
        .expect_status(StatusCode::OK);
    let OperatorSignIn::Link(url) = operator_sign_in_link(&app.state, "ada@acme.example")
        .await
        .unwrap()
    else {
        panic!("no link for Ada");
    };
    let token = url
        .expose_secret()
        .split("#token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let r = app
        .post("/auth/magic-link/verify", json!({ "token": token }), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let who = app.get("/auth/session", &r.cookie().unwrap()).await;
    assert_eq!(
        who.body["memberships"][0]["access"], "needs_sso",
        "the org that requires SSO stays closed to it"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_domain_verified_while_sso_is_required_ends_its_accounts_other_sessions() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (cy, _) = common::join(
        &app,
        &owner,
        json!({ "email": "cy@eu.acme.example", "role": "member" }),
        "Cy",
    )
    .await;
    common::manage_domain(&app, &owner.org_id, "acme.example").await;
    sqlx::query("UPDATE organizations SET require_sso = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        app.get("/auth/session", &cy).await.status,
        StatusCode::OK,
        "eu.acme.example isn't verified yet"
    );
    let r =
        kaeso_server::services::sso::operator_verify_domain(&app.state, "eu.acme.example", None)
            .await
            .unwrap();
    assert!(
        matches!(r, kaeso_server::services::sso::OperatorVerify::NotListed),
        "{r:?}"
    );
    sqlx::query("INSERT INTO org_oidc_domains (org_id, domain) VALUES ($1, 'eu.acme.example')")
        .bind(common::uuid(&owner.org_id))
        .execute(&app.state.pg)
        .await
        .unwrap();
    let r =
        kaeso_server::services::sso::operator_verify_domain(&app.state, "eu.acme.example", None)
            .await
            .unwrap();
    assert!(
        matches!(r, kaeso_server::services::sso::OperatorVerify::Verified(_)),
        "{r:?}"
    );
    assert_eq!(
        app.get("/auth/session", &cy).await.status,
        StatusCode::UNAUTHORIZED
    );
    s.cleanup().await;
}
