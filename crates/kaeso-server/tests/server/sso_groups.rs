#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_core::clock::Clock;
use kaeso_core::id::OrgId;
use kaeso_store::pg::oidc;
use kaeso_testkit::oidc::MockIdp;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

async fn connect(app: &App, owner: &Owner, idp: &MockIdp) {
    let org: OrgId = owner.org_id.parse().unwrap();
    let sealed = kaeso_crypto::seal(
        &app.state.keyring,
        b"client-secret",
        &kaeso_server::platform::auth::oidc::secret_aad(org),
    )
    .unwrap();
    let mut conn = app.state.pg.acquire().await.unwrap();
    oidc::upsert(
        &mut conn,
        &oidc::Connection {
            org_id: org.uuid(),
            issuer: idp.issuer().to_owned(),
            client_id: "kaeso".into(),
            secret_ciphertext: sealed.ciphertext,
            secret_nonce: sealed.nonce,
            wrapped_key: sealed.wrapped_key,
            key_id: sealed.key_id,
            domains: vec!["acme.example".into()],
            verified_domains: Vec::new(),
            verification_token: "token".into(),
            display_name: None,
            auto_redirect: false,
            require_email_verified: true,
            confirms_sign_in: None,
        },
    )
    .await
    .unwrap();
    assert!(
        oidc::mark_verified(&mut *conn, org.uuid(), "acme.example", app.clock.now())
            .await
            .unwrap()
    );
    sqlx::query("UPDATE organizations SET sso_default_preset = 'member'")
        .execute(&app.state.pg)
        .await
        .unwrap();
}

/// Signs `email` in through the provider; the session's cookie and the user's id.
async fn sign_in(app: &App, idp: &MockIdp, email: &str) -> (String, String) {
    let r = app
        .post("/auth/oidc/start", json!({ "email": email }), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let url = r.body["authorization_url"].as_str().unwrap().to_owned();
    let binding = r.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize(&url, email, "Someone");
    let r = app
        .raw(
            Request::get(format!(
                "/platform/v1/auth/oidc/callback?code={}&state={}",
                auth.code, auth.state
            ))
            .header(header::COOKIE, format!("__Host-kaeso_oidc={binding}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.headers);
    let cookie = r.cookie().unwrap();
    let who = app.get("/auth/session", &cookie).await;
    (cookie, who.body["user"]["id"].as_str().unwrap().to_owned())
}

async fn rules(app: &App, owner: &Owner, body: Value) -> common::Resp {
    app.call(
        Method::PUT,
        &format!("/orgs/{}/security/sso/groups", owner.org_id),
        Some(body),
        Some(&owner.cookie),
    )
    .await
}

async fn role(app: &App, owner: &Owner, user: &str) -> String {
    let r = app
        .get(
            &format!("/orgs/{}/members/{user}", owner.org_id),
            &owner.cookie,
        )
        .await;
    r.body["grant"]["role"]["id"].as_str().unwrap().to_owned()
}

async fn in_project(app: &App, owner: &Owner, project: &str, user: &str) -> Option<String> {
    let r = app
        .get(
            &format!("/orgs/{}/projects/{project}/members/{user}", owner.org_id),
            &owner.cookie,
        )
        .await;
    (r.status == StatusCode::OK).then(|| r.body["grant"]["role"]["id"].as_str().unwrap().to_owned())
}

struct Setup {
    s: TestServices,
    app: App,
    owner: Owner,
    idp: MockIdp,
    mapped: String,
    by_hand: String,
}

async fn setup(mode: &str) -> Setup {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp).await;
    let mapped = common::create_project(&app, &owner.org_id, &owner.cookie, "Checkout").await;
    let by_hand = common::create_project(&app, &owner.org_id, &owner.cookie, "Support").await;
    let r = rules(
        &app,
        &owner,
        json!({
            "claim": "groups",
            "scopes": ["groups"],
            "mode": mode,
            "rules": [
                { "group": "kaeso-admins", "role": "admin", "projects": [{ "project": mapped, "role": "developer" }] },
                { "group": "everyone", "role": "developer" }
            ]
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["rules"][0]["role"], "admin");
    assert_eq!(r.body["rules"][0]["projects"][0]["project"], mapped);
    Setup {
        s,
        app,
        owner,
        idp,
        mapped,
        by_hand,
    }
}

#[tokio::test]
async fn a_group_gives_its_role_and_project_and_keep_in_sync_takes_back_only_what_it_gave() {
    let t = setup("sync").await;
    let (app, owner, idp) = (&t.app, &t.owner, &t.idp);
    idp.set_groups("ada@acme.example", Some(&["kaeso-admins", "everyone"]));
    let (_, ada) = sign_in(app, idp, "ada@acme.example").await;
    assert_eq!(
        role(app, owner, &ada).await,
        "admin",
        "the first match sets the role"
    );
    assert_eq!(
        in_project(app, owner, &t.mapped, &ada).await.as_deref(),
        Some("developer")
    );

    // An admin adds her to another project by hand.
    let r = app
        .post(
            &format!("/orgs/{}/projects/{}/members", owner.org_id, t.by_hand),
            json!({ "user_id": ada, "role": "viewer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let groups = app
        .get(
            &format!("/orgs/{}/security/sso/groups", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(
        groups.body["your_last_groups"],
        Value::Null,
        "the owner never signed in through it"
    );

    idp.set_groups("ada@acme.example", Some(&["everyone"]));
    sign_in(app, idp, "ada@acme.example").await;
    assert_eq!(
        role(app, owner, &ada).await,
        "developer",
        "the next rule's role"
    );
    assert_eq!(
        in_project(app, owner, &t.mapped, &ada).await,
        None,
        "what the rule gave is gone"
    );
    assert_eq!(
        in_project(app, owner, &t.by_hand, &ada).await.as_deref(),
        Some("viewer"),
        "a hand-given membership stays"
    );

    idp.set_groups("ada@acme.example", Some(&[]));
    sign_in(app, idp, "ada@acme.example").await;
    assert_eq!(
        role(app, owner, &ada).await,
        "member",
        "no match: the default role"
    );

    let audit = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .collect();
    assert!(actions.contains(&"project.member_remove"), "{actions:?}");
    assert!(
        actions.contains(&"security.sso_groups_update"),
        "{actions:?}"
    );
    t.s.cleanup().await;
}

#[tokio::test]
async fn add_only_keeps_what_a_rule_gave_and_a_missing_claim_changes_nothing() {
    let t = setup("add").await;
    let (app, owner, idp) = (&t.app, &t.owner, &t.idp);
    idp.set_groups("bob@acme.example", Some(&["kaeso-admins"]));
    let (_, bob) = sign_in(app, idp, "bob@acme.example").await;
    assert_eq!(role(app, owner, &bob).await, "admin");
    idp.set_groups("bob@acme.example", Some(&[]));
    sign_in(app, idp, "bob@acme.example").await;
    assert_eq!(
        role(app, owner, &bob).await,
        "admin",
        "add only takes nothing away"
    );
    assert!(in_project(app, owner, &t.mapped, &bob).await.is_some());

    let t2 = rules(
        app,
        owner,
        json!({ "mode": "sync", "rules": [{ "group": "everyone", "role": "developer" }] }),
    )
    .await;
    assert_eq!(t2.status, StatusCode::OK, "{:?}", t2.body);
    idp.set_groups("bob@acme.example", None);
    sign_in(app, idp, "bob@acme.example").await;
    assert_eq!(
        role(app, owner, &bob).await,
        "admin",
        "a token without the claim changes nothing"
    );
    assert!(in_project(app, owner, &t.mapped, &bob).await.is_some());
    let r = app
        .get(
            &format!("/orgs/{}/security/sso/groups", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert!(r.body["claim_missing_at"].is_string(), "{:?}", r.body);
    t.s.cleanup().await;
}

#[tokio::test]
async fn owners_are_untouched_and_rules_never_make_owners() {
    let t = setup("sync").await;
    let (app, owner, idp) = (&t.app, &t.owner, &t.idp);
    idp.set_groups("cy@acme.example", Some(&["everyone"]));
    let (_, cy) = sign_in(app, idp, "cy@acme.example").await;
    assert_eq!(role(app, owner, &cy).await, "developer");
    sqlx::query("UPDATE org_memberships SET preset = 'owner', from_sso_group = false WHERE user_id = (SELECT id FROM users WHERE email = 'cy@acme.example')")
        .execute(&app.state.pg)
        .await
        .unwrap();
    idp.set_groups("cy@acme.example", Some(&["kaeso-admins"]));
    let (cookie, _) = sign_in(app, idp, "cy@acme.example").await;
    assert_eq!(role(app, owner, &cy).await, "owner");
    let r = app
        .get(
            &format!("/orgs/{}/security/sso/groups", owner.org_id),
            &cookie,
        )
        .await;
    assert_eq!(r.body["your_last_groups"], json!(["kaeso-admins"]));

    for (body, param) in [
        (
            json!({ "rules": [{ "group": "x", "role": "owner" }] }),
            "rules",
        ),
        (
            json!({ "rules": [{ "group": "x" }, { "group": "x" }] }),
            "rules",
        ),
        (json!({ "mode": "sometimes" }), "mode"),
        (json!({ "claim": "has space" }), "claim"),
        (
            json!({ "rules": [{ "group": "x", "projects": [{ "project": "proj_00000000000000000000000000", "role": "viewer" }] }] }),
            "rules",
        ),
    ] {
        let r = rules(app, owner, body.clone()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{body}");
    }
    t.s.cleanup().await;
}

#[tokio::test]
async fn rules_need_a_connection() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = rules(&app, &owner, json!({ "rules": [] })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "Connect an identity provider first; group rules apply to its sign-ins."
    );
    let r = app
        .get(
            &format!("/orgs/{}/security/sso/groups", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.body["claim"], "groups");
    assert_eq!(r.body["mode"], "add");
    s.cleanup().await;
}
