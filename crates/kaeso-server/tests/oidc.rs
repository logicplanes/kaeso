#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{App, ORIGIN, Owner};
use kaeso_core::clock::Clock;
use kaeso_core::id::OrgId;
use kaeso_store::pg::oidc;
use kaeso_testkit::oidc::{Fault, MockIdp};
use kaeso_testkit::services::TestServices;
use serde_json::json;

async fn connect(app: &App, owner: &Owner, idp: &MockIdp, default_preset: Option<&str>) {
    connect_org(app, &owner.org_id, idp, "acme.example", None, false).await;
    sqlx::query("UPDATE organizations SET sso_default_preset = $1")
        .bind(default_preset)
        .execute(&app.state.pg)
        .await
        .unwrap();
}

async fn connect_org(
    app: &App,
    org: &str,
    idp: &MockIdp,
    domain: &str,
    name: Option<&str>,
    auto_redirect: bool,
) {
    connect_org_with(app, org, idp, domain, name, auto_redirect, true).await;
}

async fn connect_org_with(
    app: &App,
    org: &str,
    idp: &MockIdp,
    domain: &str,
    name: Option<&str>,
    auto_redirect: bool,
    verified: bool,
) {
    let org: OrgId = org.parse().unwrap();
    let sealed = kaeso_crypto::seal(
        &app.state.keyring,
        b"client-secret",
        &kaeso_server::platform::auth::oidc::secret_aad(org),
    )
    .unwrap();
    let mut conn = app.state.pg.acquire().await.unwrap();
    let stored = oidc::upsert(
        &mut conn,
        &oidc::Connection {
            org_id: org.uuid(),
            issuer: idp.issuer().to_owned(),
            client_id: "kaeso".into(),
            secret_ciphertext: sealed.ciphertext,
            secret_nonce: sealed.nonce,
            wrapped_key: sealed.wrapped_key,
            key_id: sealed.key_id,
            domains: vec![domain.into()],
            verified_domains: Vec::new(),
            verification_token: "token".into(),
            display_name: name.map(str::to_owned),
            auto_redirect,
            require_email_verified: true,
            confirms_sign_in: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(stored, oidc::Upserted::Saved);
    if verified {
        assert!(
            oidc::mark_verified(&mut *conn, org.uuid(), domain, app.clock.now())
                .await
                .unwrap()
        );
    }
}

struct Flow {
    url: String,
    binding: String,
}

async fn start(app: &App, body: serde_json::Value) -> Flow {
    let r = app.post("/auth/oidc/start", body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    Flow {
        url: r.body["authorization_url"].as_str().unwrap().to_owned(),
        binding: r.set_cookie("__Host-kaeso_oidc").unwrap(),
    }
}

async fn callback(app: &App, code: &str, state: &str, binding: Option<&str>) -> common::Resp {
    let mut req = Request::get(format!(
        "/platform/v1/auth/oidc/callback?code={code}&state={state}"
    ));
    if let Some(b) = binding {
        req = req.header(header::COOKIE, format!("__Host-kaeso_oidc={b}"));
    }
    app.raw(req.body(Body::empty()).unwrap()).await
}

async fn provider_error(app: &App, state: &str, binding: &str) -> common::Resp {
    app.raw(
        Request::get(format!(
            "/platform/v1/auth/oidc/callback?error=access_denied&error_description=Denied&state={state}"
        ))
        .header(header::COOKIE, format!("__Host-kaeso_oidc={binding}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
}

fn location(r: &common::Resp) -> String {
    r.headers[header::LOCATION].to_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_first_sso_sign_in_creates_the_user_with_the_default_role() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("developer")).await;

    let flow = start(&app, json!({ "email": "ada@ACME.example" })).await;
    assert!(flow.url.starts_with(idp.issuer()));
    assert!(flow.url.contains("code_challenge_method=S256"));
    assert!(flow.url.contains(&urlencode(&format!(
        "{ORIGIN}/platform/v1/auth/oidc/callback"
    ))));
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.headers);
    assert_eq!(location(&r), format!("{ORIGIN}/"));
    let cleared = r
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("__Host-kaeso_oidc=;"))
        .expect("the flow cookie is cleared");
    assert!(
        cleared.contains("; Secure") && cleared.contains("Max-Age=0"),
        "{cleared}"
    );
    let cookie = r.cookie().unwrap();
    let who = app.get("/auth/session", &cookie).await;
    assert_eq!(who.body["user"]["email"], "ada@acme.example");
    assert_eq!(who.body["user"]["name"], "Ada Lovelace");
    assert_eq!(who.body["session"]["method"], "oidc");
    assert_eq!(who.body["memberships"][0]["preset"], "developer");
    sqlx::query("UPDATE organizations SET require_sso = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        app.get("/auth/session", &cookie).await.body["memberships"][0]["access"],
        "ok"
    );
    let again = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert!(location(&again).contains("sso_error="));
    s.cleanup().await;
}

#[tokio::test]
async fn an_org_admits_its_sso_users_under_every_account_creation_setting() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    for (setting, email) in [
        ("invitation_only", "ada@acme.example"),
        ("allowed_domains", "grace@acme.example"),
    ] {
        sqlx::query(
            "UPDATE instance_settings SET account_creation = $1, allowed_domains = '{example.com}'",
        )
        .bind(setting)
        .execute(&app.state.pg)
        .await
        .unwrap();
        let flow = start(&app, json!({ "email": email })).await;
        let auth = idp.authorize(&flow.url, email, "Member");
        let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
        assert_eq!(location(&r), format!("{ORIGIN}/"), "{setting}");
        let who = app.get("/auth/session", &r.cookie().unwrap()).await;
        assert_eq!(who.body["memberships"][0]["preset"], "member", "{setting}");
    }
    s.cleanup().await;
}

#[tokio::test]
async fn the_methods_for_an_address_follow_its_domain_not_its_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, None).await;
    let methods = |email: &'static str| {
        let app = &app;
        async move {
            let r = app
                .post("/auth/methods", json!({ "email": email }), None)
                .await;
            assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
            r.body
        }
    };

    let page = app
        .call(axum::http::Method::GET, "/auth/methods", None, None)
        .await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.body["password"], true);
    assert_eq!(page.body["sign_up"], false);

    let known = methods("owner@example.com").await;
    let unknown = methods("nobody@example.com").await;
    assert_eq!(known, unknown);
    assert_eq!(known["provider"], serde_json::Value::Null);
    assert_eq!(known["password"], true);
    assert_eq!(known["magic_link"], true);

    let sso = methods("ada@ACME.example").await;
    assert_eq!(sso["provider"]["org"], owner.org_id.as_str());
    assert_eq!(sso["sso_required"], false);
    assert_eq!(sso["password"], true);

    sqlx::query("UPDATE organizations SET require_sso = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let required = methods("ada@acme.example").await;
    assert_eq!(required["sso_required"], true);
    assert_eq!(required["password"], false);
    assert_eq!(required["magic_link"], false);

    sqlx::query("UPDATE instance_settings SET account_creation = 'everyone'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(methods("ada@acme.example").await["sign_up"], false);
    assert_eq!(methods("nobody@example.com").await["sign_up"], true);

    let r = app
        .post("/auth/methods", json!({ "email": "not an address" }), None)
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
}

#[tokio::test]
async fn the_mock_provider_signs_a_browser_in_through_its_page() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let flow = start(&app, json!({ "email": "lin@acme.example" })).await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let page = client.get(&flow.url).send().await.unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    assert!(page.text().await.unwrap().contains(r#"name="login_email""#));
    let url = url::Url::parse(&flow.url).unwrap();
    let mut form: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    form.push(("login_email".into(), "lin@acme.example".into()));
    form.push(("login_name".into(), "Lin Park".into()));
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&form)
        .finish();
    let back = client
        .post(format!("{}/authorize", idp.issuer()))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(back.status(), StatusCode::SEE_OTHER);
    let to = url::Url::parse(back.headers()[header::LOCATION].to_str().unwrap()).unwrap();
    assert!(to.path().ends_with("/platform/v1/auth/oidc/callback"));
    let q: std::collections::HashMap<String, String> = to.query_pairs().into_owned().collect();
    let r = callback(&app, &q["code"], &q["state"], Some(&flow.binding)).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.headers);
    let who = app.get("/auth/session", &r.cookie().unwrap()).await;
    assert_eq!(who.body["user"]["name"], "Lin Park");
    s.cleanup().await;
}

#[tokio::test]
async fn without_an_invite_or_default_role_there_is_no_access() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, None).await;
    let flow = start(&app, json!({ "email": "bob@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "bob@acme.example", "Bob");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert!(location(&r).contains("sso_error="), "{}", location(&r));
    assert!(r.cookie().is_none());

    let token_hash = kaeso_core::secret::hash("invite-token");
    kaeso_store::pg::invites::insert(
        &app.state.pg,
        &kaeso_store::pg::invites::NewInvite {
            id: uuid::Uuid::now_v7(),
            org_id: Some(owner.org_id.parse::<OrgId>().unwrap().uuid()),
            email: "bob@acme.example",
            org_preset: Some("admin"),
            org_role_id: None,
            project_roles: json!([]),
            token_hash: &token_hash,
            invited_by: None,
            expires_at: app.clock.now() + time::Duration::days(21),
        },
    )
    .await
    .unwrap();
    let flow = start(&app, json!({ "email": "bob@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "bob@acme.example", "Bob");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let who = app
        .get("/auth/session", &r.cookie().expect("signed in"))
        .await;
    assert_eq!(who.body["memberships"][0]["preset"], "admin");
    s.cleanup().await;
}

#[tokio::test]
async fn joining_by_sso_takes_one_invitation_and_retires_the_others() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, None).await;
    let org = owner.org_id.parse::<OrgId>().unwrap().uuid();
    let mut ids = Vec::new();
    for (preset, token) in [("admin", "older-invite"), ("member", "newer-invite")] {
        let id = uuid::Uuid::now_v7();
        kaeso_store::pg::invites::insert(
            &app.state.pg,
            &kaeso_store::pg::invites::NewInvite {
                id,
                org_id: Some(org),
                email: "bob@acme.example",
                org_preset: Some(preset),
                org_role_id: None,
                project_roles: json!([]),
                token_hash: &kaeso_core::secret::hash(token),
                invited_by: None,
                expires_at: app.clock.now() + time::Duration::days(21),
            },
        )
        .await
        .unwrap();
        ids.push(kaeso_core::id::InviteId::from_uuid(id).to_string());
    }

    let flow = start(&app, json!({ "email": "bob@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "bob@acme.example", "Bob");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let who = app
        .get("/auth/session", &r.cookie().expect("signed in"))
        .await;
    assert_eq!(who.body["memberships"][0]["preset"], "member");
    let status = |id: &str| {
        let path = format!("/orgs/{}/invites/{id}", owner.org_id);
        let app = &app;
        let cookie = owner.cookie.clone();
        async move { app.get(&path, &cookie).await.body["status"].clone() }
    };
    assert_eq!(status(&ids[0]).await, "revoked");
    assert_eq!(status(&ids[1]).await, "accepted");
    let r = app
        .post(
            "/auth/invites/lookup",
            json!({ "token": "older-invite" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn sso_signs_in_existing_accounts_once_linked_and_only_where_they_are_members() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("admin")).await;
    let org: OrgId = owner.org_id.parse().unwrap();
    let eve = kaeso_core::id::UserId::new();
    kaeso_store::pg::users::insert(
        &app.state.pg,
        &kaeso_store::pg::users::NewUser {
            id: eve.uuid(),
            email: "eve@acme.example",
            name: "Eve",
            password_hash: None,
            email_verified_at: None,
            is_instance_admin: false,
        },
    )
    .await
    .unwrap();
    let sign_in = || async {
        let flow = start(&app, json!({ "email": "eve@acme.example" })).await;
        let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
        callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await
    };

    let r = sign_in().await;
    refused_with(&r, LINK_REQUIRED);

    let token_hash = kaeso_core::secret::hash("eve-invite");
    kaeso_store::pg::invites::insert(
        &app.state.pg,
        &kaeso_store::pg::invites::NewInvite {
            id: uuid::Uuid::now_v7(),
            org_id: Some(org.uuid()),
            email: "eve@acme.example",
            org_preset: Some("member"),
            org_role_id: None,
            project_roles: json!([]),
            token_hash: &token_hash,
            invited_by: None,
            expires_at: app.clock.now() + time::Duration::days(21),
        },
    )
    .await
    .unwrap();
    refused_with(&sign_in().await, LINK_REQUIRED);
    kaeso_store::pg::sso_identities::link(
        &app.state.pg,
        org.uuid(),
        eve.uuid(),
        idp.issuer(),
        "sub-eve@acme.example",
        app.clock.now(),
    )
    .await
    .unwrap();
    let r = sign_in().await;
    assert!(
        location(&r).ends_with("#sso_error=accept_invite"),
        "{}",
        location(&r)
    );
    assert!(r.cookie().is_none());
    let members: i64 =
        sqlx::query_scalar("SELECT count(*) FROM org_memberships WHERE user_id = $1")
            .bind(eve.uuid())
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(members, 0);

    let mut tx = app.state.pg.begin().await.unwrap();
    kaeso_server::services::orgs::add_member(
        &mut tx,
        org,
        eve,
        &kaeso_store::pg::members::Grant {
            preset: "member".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let r = sign_in().await;
    assert_eq!(location(&r), format!("{ORIGIN}/"));
    let who = app.get("/auth/session", &r.cookie().unwrap()).await;
    assert_eq!(who.body["user"]["email"], "eve@acme.example");
    assert_eq!(who.body["memberships"][0]["preset"], "member");
    s.cleanup().await;
}

#[tokio::test]
async fn forged_or_misdirected_answers_are_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    for (fault, code) in [
        (Fault::WrongNonce, "answer_unverified"),
        (Fault::WrongAudience, "answer_unverified"),
        (Fault::Expired, "answer_unverified"),
        (Fault::WrongIssuer, "answer_unverified"),
        (Fault::UnverifiedEmail, "email_unverified"),
    ] {
        idp.set_fault(fault.clone());
        let flow = start(&app, json!({ "email": "eve@acme.example" })).await;
        let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
        let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
        refused_with(&r, code);
    }
    idp.set_fault(Fault::None);
    let flow = start(&app, json!({ "email": "eve@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "owner@example.com", "Owner");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert!(
        location(&r).ends_with("#sso_error=domain_not_allowed"),
        "{}",
        location(&r)
    );
    assert!(r.cookie().is_none());
    let flow = start(&app, json!({ "email": "eve@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
    let r = callback(&app, &auth.code, &auth.state, None).await;
    assert!(location(&r).contains("sso_error="));
    let r = callback(&app, &auth.code, &auth.state, Some("other-browser")).await;
    assert!(location(&r).contains("sso_error="));
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(
        location(&r),
        format!("{ORIGIN}/"),
        "a misdirected answer leaves the browser that began the sign-in able to finish it"
    );
    assert!(r.cookie().is_some());
    s.cleanup().await;
}

#[tokio::test]
async fn providers_on_private_networks_are_refused_when_not_allowed() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[("KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS", "false")]).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let r = app
        .post(
            "/auth/oidc/start",
            json!({ "email": "ada@acme.example" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"], "The identity provider could not be reached.",
        "a private address answers like an unreachable one"
    );
    let r = app
        .post(
            "/auth/oidc/start",
            json!({ "email": "x@nowhere.example" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    s.cleanup().await;
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[tokio::test]
async fn oversized_identity_provider_answers_are_refused() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = axum::Router::new()
        .route("/big", axum::routing::get(|| async { vec![b'x'; 2 << 20] }))
        .route(
            "/small",
            axum::routing::get(|| async { vec![b'x'; 1 << 10] }),
        );
    tokio::spawn(async move { axum::serve(listener, router).await });
    let fetch = |path: &str| {
        let request = Request::get(format!("http://{addr}{path}"))
            .body(Vec::new())
            .unwrap();
        kaeso_server::auth::oidc::fetch(true, request)
    };
    let err = fetch("/big").await.unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");
    let ok = fetch("/small").await.unwrap();
    assert_eq!(ok.body().len(), 1 << 10);
}

const WITHOUT_PASSWORDS: [(&str, &str); 2] = [
    ("KAESO_AUTH_PASSWORD", "false"),
    ("KAESO_AUTH_MAGIC_LINK", "false"),
];

async fn start_as_member(app: &App, cookie: &str, body: serde_json::Value) -> Flow {
    let r = app.post("/auth/oidc/start", body, Some(cookie)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    Flow {
        url: r.body["authorization_url"].as_str().unwrap().to_owned(),
        binding: r.set_cookie("__Host-kaeso_oidc").unwrap(),
    }
}

async fn join_org(app: &App, org: &str, user: &str) {
    let grant = kaeso_store::pg::members::Grant {
        preset: "member".into(),
        ..Default::default()
    };
    assert!(
        kaeso_store::pg::members::insert_org(
            &app.state.pg,
            common::uuid(org),
            common::uuid(user),
            &grant
        )
        .await
        .unwrap()
    );
}

async fn methods_page(app: &App, query: &str) -> serde_json::Value {
    let r = app
        .call_with(
            axum::http::Method::GET,
            &format!("/auth/methods{query}"),
            None,
            None,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

async fn another_org(app: &App, name: &str) -> String {
    let org = OrgId::new();
    kaeso_store::pg::orgs::insert(&app.state.pg, org.uuid(), name, 30, 30)
        .await
        .unwrap();
    org.to_string()
}

async fn start_without_a_click(app: &App, org: &str) {
    sqlx::query("UPDATE org_oidc SET auto_redirect = true WHERE org_id = $1")
        .bind(common::uuid(org))
        .execute(&app.state.pg)
        .await
        .unwrap();
}

#[tokio::test]
async fn with_passwords_and_links_off_the_page_still_asks_for_the_address_first() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &WITHOUT_PASSWORDS).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(
        &app,
        &owner.org_id,
        &idp,
        "acme.example",
        Some("Acme ID"),
        true,
    )
    .await;
    let page = methods_page(&app, "").await;
    assert_eq!(
        (&page["password"], &page["magic_link"]),
        (&json!(false), &json!(false))
    );
    assert_eq!(
        page["provider"],
        serde_json::Value::Null,
        "even the install's only provider waits for an address"
    );

    let by_address = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@acme.example" }),
            None,
        )
        .await;
    assert_eq!(
        by_address.body["provider"],
        json!({ "org": owner.org_id, "org_name": "Acme", "name": "Acme ID", "auto_redirect": true })
    );
    let elsewhere = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@else.example" }),
            None,
        )
        .await;
    assert_eq!(elsewhere.body["provider"], serde_json::Value::Null);
    s.cleanup().await;
}

#[tokio::test]
async fn the_address_offers_only_the_provider_of_its_verified_domain() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, true).await;
    let beta = another_org(&app, "Beta").await;
    connect_org_with(&app, &beta, &idp, "beta.example", None, true, false).await;
    let offered = |email: &'static str| {
        let app = &app;
        async move {
            let r = app
                .post("/auth/methods", json!({ "email": email }), None)
                .await;
            r.body["provider"]["org"].clone()
        }
    };
    assert_eq!(offered("ada@acme.example").await, json!(owner.org_id));
    assert_eq!(
        offered("ada@beta.example").await,
        serde_json::Value::Null,
        "a domain the org hasn't verified offers no SSO"
    );
    assert_eq!(offered("ada@else.example").await, serde_json::Value::Null);
    let r = app
        .post(
            "/auth/oidc/start",
            json!({ "email": "ada@beta.example" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn a_link_from_another_site_never_signs_a_signed_out_browser_in_to_its_org() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let evil = another_org(&app, "Evil").await;
    connect_org(&app, &evil, &idp, "evil.example", Some("Evil ID"), true).await;
    set_default_role(&app, &evil, "admin").await;

    let r = app
        .raw(
            Request::get(format!("/platform/v1/auth/oidc/start?org={evil}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        r.status,
        StatusCode::METHOD_NOT_ALLOWED,
        "no start link sends a browser on to an org's provider"
    );
    assert!(r.headers.get(header::LOCATION).is_none());
    assert_eq!(
        methods_page(&app, &format!("?org={evil}")).await["provider"],
        serde_json::Value::Null,
        "the sign-in page names no org without an address"
    );

    let r = app
        .post("/auth/oidc/start", json!({ "org": evil }), None)
        .await;
    assert_eq!(
        (
            r.status,
            r.body["error"]["param"].as_str(),
            r.body["error"]["message"].as_str()
        ),
        (
            StatusCode::BAD_REQUEST,
            Some("email"),
            Some("Enter your email to sign in with single sign-on.")
        ),
        "a signed-out browser can't start an org's SSO by its id, even without a click"
    );
    assert!(r.set_cookie("__Host-kaeso_oidc").is_none());
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(accounts, 1, "no account was made for the other org");
    s.cleanup().await;
}

#[tokio::test]
async fn only_a_signed_in_member_names_its_org_to_start_sso() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    start_without_a_click(&app, &owner.org_id).await;
    let beta = another_org(&app, "Beta").await;
    connect_org(&app, &beta, &idp, "beta.example", None, true).await;

    let r = app
        .post("/auth/oidc/start", json!({ "org": owner.org_id }), None)
        .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("email"))
    );
    let flow = start_as_member(&app, &owner.cookie, json!({ "org": owner.org_id })).await;
    assert!(
        flow.url.contains("code_challenge_method=S256"),
        "a signed-in member opens its org's SSO: {}",
        flow.url
    );
    let r = app
        .post(
            "/auth/oidc/start",
            json!({ "org": beta }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("email")),
        "{:?}",
        r.body
    );
    let unconnected = another_org(&app, "Gamma").await;
    let r = app
        .post("/auth/oidc/start", json!({ "org": unconnected }), None)
        .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "a signed-out browser learns nothing about an org's SSO: {:?}",
        r.body
    );

    let flow = start(
        &app,
        json!({ "email": "ada@acme.example", "next": "/orgs/x/projects/y/logs#top" }),
    )
    .await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(
        location(&back),
        format!("{ORIGIN}/orgs/x/projects/y/logs#top")
    );
    s.cleanup().await;
}

#[tokio::test]
async fn next_never_sends_the_browser_off_the_install() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    for next in [
        "//evil.example",
        "https://evil.example/",
        "/\\evil.example",
        "/orgs\r\nSet-Cookie: x=1",
        "/platform/v1/auth/oidc/start",
        "/../v1/models",
    ] {
        let r = app
            .post(
                "/auth/oidc/start",
                json!({ "email": "ada@acme.example", "next": next }),
                None,
            )
            .await;
        assert_eq!(
            (r.status, r.body["error"]["param"].as_str()),
            (StatusCode::BAD_REQUEST, Some("next")),
            "{next:?}"
        );
        assert!(r.set_cookie("__Host-kaeso_oidc").is_none(), "{next:?}");
    }

    let flow = start(
        &app,
        json!({ "email": "ada@acme.example", "next": "/orgs" }),
    )
    .await;
    sqlx::query(
        "UPDATE sign_in_tokens SET data = jsonb_set(data, '{next}', '\"//evil.example\"') WHERE kind = 'oidc_state'",
    )
    .execute(&app.state.pg)
    .await
    .unwrap();
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(location(&back), format!("{ORIGIN}/"));

    let flow = start(
        &app,
        json!({ "email": "ada@acme.example", "next": "/orgs?a=1" }),
    )
    .await;
    let auth = idp.authorize(&flow.url, "eve@else.example", "Eve");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let to = location(&back);
    assert!(
        to.starts_with(&format!(
            "{ORIGIN}/sign-in?next=%2Forgs%3Fa%3D1#sso_error=domain_not_allowed"
        )),
        "a refused sign-in keeps the page to come back to: {to}"
    );
    assert!(back.cookie().is_none());
    s.cleanup().await;
}

#[tokio::test]
async fn sign_in_reads_need_no_origin_but_changes_do() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let bare = |method: axum::http::Method, path: &'static str, body: Option<serde_json::Value>| {
        let app = &app;
        async move { app.call_with(method, path, body, None, None).await }
    };
    assert_eq!(
        bare(axum::http::Method::GET, "/auth/methods", None)
            .await
            .status,
        StatusCode::OK
    );
    let expected =
        format!("Open Kaeso at {ORIGIN}; it refuses requests from pages at other addresses.");
    for (path, body) in [
        ("/auth/methods", json!({ "email": "ada@acme.example" })),
        ("/auth/oidc/start", json!({ "email": "ada@acme.example" })),
    ] {
        let r = bare(axum::http::Method::POST, path, Some(body)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(r.body["error"]["message"], expected.as_str(), "{path}");
    }
    s.cleanup().await;
}

#[tokio::test]
async fn a_refusal_at_the_provider_ends_the_flow_and_keeps_the_way_back() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let refused = "#sso_error=provider_refused";

    let flow = start(
        &app,
        json!({ "email": "ada@acme.example", "next": "/settings/account" }),
    )
    .await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let r = provider_error(&app, &auth.state, &flow.binding).await;
    assert_eq!(
        location(&r),
        format!("{ORIGIN}/sign-in?next=%2Fsettings%2Faccount{refused}")
    );
    assert!(r.cookie().is_none());
    let late = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert!(
        location(&late).ends_with("#sso_error=expired"),
        "the refused flow can't be finished afterwards: {}",
        location(&late)
    );

    let flow = start(
        &app,
        json!({ "email": "ada@acme.example", "next": "/orgs" }),
    )
    .await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let r = provider_error(&app, &auth.state, "other-browser").await;
    assert_eq!(location(&r), format!("{ORIGIN}/sign-in{refused}"));
    let r = provider_error(&app, "unknown-state", &flow.binding).await;
    assert_eq!(location(&r), format!("{ORIGIN}/sign-in{refused}"));
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(
        location(&back),
        format!("{ORIGIN}/orgs"),
        "a refusal from another browser leaves the flow to the browser that began it"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_refusal_after_the_provider_keeps_the_page_to_come_back_to() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, true).await;

    let by_org = start_as_member(
        &app,
        &owner.cookie,
        json!({ "org": owner.org_id, "next": "/orgs" }),
    )
    .await;
    let by_email = start(
        &app,
        json!({ "email": "bob@acme.example", "next": "/orgs" }),
    )
    .await;
    for flow in [by_org, by_email] {
        let auth = idp.authorize(&flow.url, "bob@acme.example", "Bob");
        let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
        assert_eq!(
            location(&back),
            format!("{ORIGIN}/sign-in?next=%2Forgs#sso_error=no_access"),
            "{}",
            flow.url
        );
        assert!(back.cookie().is_none());
    }
    s.cleanup().await;
}

async fn callback_in(
    app: &App,
    auth: &kaeso_testkit::oidc::Authorized,
    binding: &str,
    session: &str,
) -> common::Resp {
    app.raw(
        Request::get(format!(
            "/platform/v1/auth/oidc/callback?code={}&state={}",
            auth.code, auth.state
        ))
        .header(
            header::COOKIE,
            format!("__Host-kaeso_session={session}; __Host-kaeso_oidc={binding}"),
        )
        .body(Body::empty())
        .unwrap(),
    )
    .await
}

async fn sessions_of(app: &App, email: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE u.email = $1 AND s.revoked_at IS NULL",
    )
    .bind(email)
    .fetch_one(&app.state.pg)
    .await
    .unwrap()
}

#[tokio::test]
async fn an_sso_answer_never_signs_a_signed_in_browser_in_as_someone_else() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let evil = another_org(&app, "Evil").await;
    connect_org(&app, &evil, &idp, "evil.example", Some("Evil ID"), true).await;
    set_default_role(&app, &evil, "admin").await;

    let flow = start(&app, json!({ "email": "eve@evil.example" })).await;
    let auth = idp.authorize_as(&flow.url, "sub-eve", "eve@evil.example", "Eve");
    let r = callback_in(&app, &auth, &flow.binding, &owner.cookie).await;
    assert_eq!(
        location(&r),
        format!("{ORIGIN}/sign-in#sso_error=signed_in_as_other")
    );
    assert!(r.cookie().is_none(), "no session for the other account");
    let who = app
        .get("/auth/session", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        who.body["user"]["email"], "owner@example.com",
        "the browser's own session stays signed in"
    );
    let eve: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE email = 'eve@evil.example'")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(eve, 0, "the refused answer creates no account");

    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    let begun = app
        .post(
            "/auth/oidc/start",
            json!({ "org": owner.org_id }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let binding = begun.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize_as(
        begun.body["authorization_url"].as_str().unwrap(),
        "sub-owner",
        "owner@acme.example",
        "Owner",
    );
    let r = callback_in(&app, &auth, &binding, &owner.cookie).await;
    let sso = r.cookie().expect("the same account signs in again");
    assert_eq!(
        app.get("/auth/session", &sso).await.body["session"]["sso_org"],
        owner.org_id.as_str()
    );
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED,
        "and replaces the session it had in this browser"
    );
    assert_eq!(sessions_of(&app, "owner@example.com").await, 1);
    s.cleanup().await;
}

async fn sso_as(app: &App, idp: &MockIdp, subject: &str, email: &str) -> common::Resp {
    let flow = start(app, json!({ "email": email })).await;
    answer_as(app, idp, &flow, subject, email).await
}

async fn answer_as(
    app: &App,
    idp: &MockIdp,
    flow: &Flow,
    subject: &str,
    email: &str,
) -> common::Resp {
    let auth = idp.authorize_as(&flow.url, subject, email, "Someone");
    callback(app, &auth.code, &auth.state, Some(&flow.binding)).await
}

async fn link_account(
    app: &App,
    idp: &MockIdp,
    cookie: &str,
    org: &str,
    subject: &str,
    email: &str,
) -> common::Resp {
    let r = app
        .post(
            "/me/sso-links",
            json!({ "org": org, "next": "/settings/account/security" }),
            Some(cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let binding = r.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize_as(
        r.body["authorization_url"].as_str().unwrap(),
        subject,
        email,
        "Someone",
    );
    callback(app, &auth.code, &auth.state, Some(&binding)).await
}

async fn set_default_role(app: &App, org: &str, preset: &str) {
    sqlx::query("UPDATE organizations SET sso_default_preset = $1 WHERE id = $2")
        .bind(preset)
        .bind(common::uuid(org))
        .execute(&app.state.pg)
        .await
        .unwrap();
}

async fn links(app: &App) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sso_identities")
        .fetch_one(&app.state.pg)
        .await
        .unwrap()
}

fn refused_with(r: &common::Resp, code: &str) {
    let to = location(r);
    assert!(to.ends_with(&format!("#sso_error={code}")), "{to}");
    assert!(r.cookie().is_none(), "{to}");
}

const LINK_REQUIRED: &str = "link_required";

#[tokio::test]
async fn an_orgs_sso_never_signs_in_an_account_by_its_address_alone() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let evil = another_org(&app, "Evil").await;
    connect_org(&app, &evil, &idp, "example.com", None, false).await;
    set_default_role(&app, &evil, "admin").await;

    let r = sso_as(&app, &idp, "sub-anyone", "owner@example.com").await;
    refused_with(&r, LINK_REQUIRED);
    let joined: i64 = sqlx::query_scalar("SELECT count(*) FROM org_memberships WHERE org_id = $1")
        .bind(common::uuid(&evil))
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(joined, 0);

    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    sqlx::query("UPDATE users SET email = 'owner@acme.example'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let r = sso_as(&app, &idp, "sub-anyone", "owner@acme.example").await;
    refused_with(&r, LINK_REQUIRED);
    assert_eq!(links(&app).await, 0);
    s.cleanup().await;
}

#[tokio::test]
async fn a_linked_account_signs_in_through_its_identity_and_only_its_own_org() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;

    let r = link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    assert_eq!(location(&r), format!("{ORIGIN}/settings/account/security"));
    assert!(
        r.cookie().is_none(),
        "linking keeps the session it began in"
    );
    let listed = app.get("/me/sso-links", &owner.cookie).await;
    assert_eq!(listed.body["data"][0]["org_name"], "Acme");
    assert_eq!(listed.body["data"][0]["linked"], true);

    let r = sso_as(&app, &idp, "sub-owner", "owner@acme.example").await;
    let sso = r.cookie().expect("signed in through the link");
    let who = app.get("/auth/session", &sso).await;
    assert_eq!(who.body["user"]["email"], "owner@example.com");
    assert_eq!(who.body["session"]["sso_org"], owner.org_id.as_str());
    assert_eq!(who.body["user"]["is_instance_admin"], true);
    assert_eq!(who.body["can_administer_instance"], false);
    assert_eq!(who.body["can_create_orgs"], false);
    let r = app.get("/instance/users", &sso).await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::FORBIDDEN, "other_sign_in_required")
    );
    let r = app.get(&format!("/orgs/{}", owner.org_id), &sso).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let who = app.get("/auth/session", &owner.cookie).await;
    assert_eq!(who.body["can_administer_instance"], true);
    assert_eq!(who.body["session"]["sso_org"], serde_json::Value::Null);

    let flow = start(&app, json!({ "email": "owner@acme.example" })).await;
    let r = answer_as(&app, &idp, &flow, "sub-impostor", "owner@example.com").await;
    assert!(location(&r).contains("sso_error="), "{}", location(&r));
    assert!(r.cookie().is_none());

    let (eve, _) = common::join(
        &app,
        &owner,
        json!({ "email": "eve@example.com", "role": "member" }),
        "Eve",
    )
    .await;
    let r = link_account(
        &app,
        &idp,
        &eve,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    assert!(
        location(&r).starts_with(&format!(
            "{ORIGIN}/settings/account/security#sso_error=linked_elsewhere"
        )),
        "{}",
        location(&r)
    );
    let r = app
        .post("/me/sso-links", json!({ "org": owner.org_id }), Some(&sso))
        .await;
    assert_eq!(r.error_type(), "other_sign_in_required");
    app.clock.advance(time::Duration::minutes(16));
    let r = app
        .post("/me/sso-links", json!({ "org": owner.org_id }), Some(&eve))
        .await;
    assert_eq!(r.error_type(), "recent_sign_in_required");

    let unlink = |cookie: String| {
        let app = &app;
        let path = format!("/me/sso-links/{}", owner.org_id);
        async move {
            app.call(axum::http::Method::DELETE, &path, None, Some(&cookie))
                .await
        }
    };
    let r = unlink(owner.cookie.clone()).await;
    assert_eq!(
        r.error_type(),
        "recent_sign_in_required",
        "unlinking is a security change"
    );
    let fresh = app
        .post(
            "/auth/password",
            json!({ "email": "owner@example.com", "password": common::PASSWORD }),
            None,
        )
        .await
        .expect_status(StatusCode::OK)
        .cookie()
        .unwrap();
    let r = unlink(fresh.clone()).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    assert_eq!(
        app.get("/auth/session", &sso).await.status,
        StatusCode::UNAUTHORIZED,
        "the session the unlinked identity signed in ends"
    );
    assert_eq!(
        app.get("/auth/session", &fresh).await.status,
        StatusCode::OK,
        "a session from another sign-in stays"
    );
    let r = sso_as(&app, &idp, "sub-owner", "owner@acme.example").await;
    assert!(r.cookie().is_none(), "{}", location(&r));
    s.cleanup().await;
}

#[tokio::test]
async fn removing_or_repointing_the_connection_ends_the_sessions_it_signed_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let org = format!("/orgs/{}", owner.org_id);
    let sso = format!("/orgs/{}/security/sso", owner.org_id);
    let connection = |idp: &MockIdp, name: &str| {
        json!({
            "issuer": idp.issuer(),
            "client_id": "kaeso",
            "client_secret": "client-secret",
            "domains": ["acme.example"],
            "name": name,
            "default_role": "member",
        })
    };
    let signed_in = |cookie: String| {
        let app = &app;
        async move { app.get("/auth/session", &cookie).await.status }
    };
    let alice = sso_as(&app, &idp, "sub-alice", "alice@acme.example")
        .await
        .cookie()
        .expect("signed in");
    app.get(&org, &alice).await.expect_status(StatusCode::OK);

    app.call(
        axum::http::Method::PUT,
        &sso,
        Some(connection(&idp, "Acme ID")),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(
        signed_in(alice.clone()).await,
        StatusCode::OK,
        "a change that keeps the provider keeps its sessions"
    );

    let other = MockIdp::start("kaeso", "client-secret").await;
    app.call(
        axum::http::Method::PUT,
        &sso,
        Some(connection(&other, "Acme ID")),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(
        signed_in(alice.clone()).await,
        StatusCode::UNAUTHORIZED,
        "the replaced provider's sessions end"
    );
    assert_eq!(signed_in(owner.cookie.clone()).await, StatusCode::OK);

    let bob = sso_as(&app, &other, "sub-bob", "bob@acme.example")
        .await
        .cookie()
        .expect("signed in through the new provider");
    app.get(&org, &bob).await.expect_status(StatusCode::OK);
    app.call(axum::http::Method::DELETE, &sso, None, Some(&owner.cookie))
        .await
        .expect_status(StatusCode::NO_CONTENT);
    app.clock.advance(time::Duration::days(6));
    assert_eq!(
        app.get(&org, &bob).await.status,
        StatusCode::UNAUTHORIZED,
        "a removed connection's sessions end"
    );
    assert_eq!(
        signed_in(owner.cookie.clone()).await,
        StatusCode::OK,
        "sessions from other sign-ins stay"
    );
    s.cleanup().await;
}

async fn answered_during(
    app: &App,
    idp: &MockIdp,
    url: &str,
    binding: &str,
    email: &str,
    change: impl std::future::Future<Output = ()>,
) -> common::Resp {
    let auth = idp.authorize_as(url, &format!("sub-{email}"), email, "Someone");
    let mut held = idp.hold_next_token();
    let answer = callback(app, &auth.code, &auth.state, Some(binding));
    let change = async move {
        held.reached().await;
        change.await;
        held.release();
    };
    tokio::join!(answer, change).0
}

#[tokio::test]
async fn an_answer_in_flight_while_the_connection_changes_signs_nobody_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let other = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let sso = format!("/orgs/{}/security/sso", owner.org_id);
    let repoint = |to: &MockIdp| {
        let body = json!({
            "issuer": to.issuer(),
            "client_id": "kaeso",
            "client_secret": "client-secret",
            "domains": ["acme.example"],
            "default_role": "member",
        });
        let (app, sso, cookie) = (&app, &sso, &owner.cookie);
        async move {
            app.call(axum::http::Method::PUT, sso, Some(body), Some(cookie))
                .await
                .expect_status(StatusCode::OK);
        }
    };
    let accounts = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&app.state.pg)
            .await
            .unwrap()
    };
    let before = accounts().await;

    let flow = start(&app, json!({ "email": "alice@acme.example" })).await;
    let removed = async {
        app.call(axum::http::Method::DELETE, &sso, None, Some(&owner.cookie))
            .await
            .expect_status(StatusCode::NO_CONTENT);
    };
    let r = answered_during(
        &app,
        &idp,
        &flow.url,
        &flow.binding,
        "alice@acme.example",
        removed,
    )
    .await;
    refused_with(&r, "expired");
    assert_eq!(
        accounts().await,
        before,
        "a removed connection creates no account"
    );

    connect(&app, &owner, &idp, Some("member")).await;
    let flow = start(&app, json!({ "email": "alice@acme.example" })).await;
    let r = answered_during(
        &app,
        &idp,
        &flow.url,
        &flow.binding,
        "alice@acme.example",
        repoint(&other),
    )
    .await;
    refused_with(&r, "expired");
    assert_eq!(
        accounts().await,
        before,
        "a replaced provider creates no account"
    );

    let begun = app
        .post(
            "/me/sso-links",
            json!({ "org": owner.org_id }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let r = answered_during(
        &app,
        &other,
        begun.body["authorization_url"].as_str().unwrap(),
        &begun.set_cookie("__Host-kaeso_oidc").unwrap(),
        "owner@acme.example",
        repoint(&idp),
    )
    .await;
    refused_with(&r, "expired");
    assert_eq!(
        links(&app).await,
        0,
        "a replaced provider links no identity"
    );

    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner@acme.example",
        "owner@acme.example",
    )
    .await;
    let begun = app
        .post(
            "/me/sign-in-again/sso",
            json!({ "org": owner.org_id }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let r = answered_during(
        &app,
        &idp,
        begun.body["authorization_url"].as_str().unwrap(),
        &begun.set_cookie("__Host-kaeso_oidc").unwrap(),
        "owner@acme.example",
        repoint(&other),
    )
    .await;
    refused_with(&r, "expired");
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::OK,
        "a replaced provider replaces no session"
    );

    let r = sso_as(&app, &other, "sub-alice", "alice@acme.example").await;
    assert!(r.cookie().is_some(), "the connection now in place signs in");
    s.cleanup().await;
}

struct InTwoOrgs {
    acme: String,
    beta: String,
    ada: String,
}

async fn ada_in_two_orgs(app: &App, idp: &MockIdp) -> InTwoOrgs {
    let owner = app.setup().await;
    connect_org(app, &owner.org_id, idp, "acme.example", None, false).await;
    set_default_role(app, &owner.org_id, "member").await;
    let beta = another_org(app, "Beta").await;
    let r = sso_as(app, idp, "sub-ada", "ada@acme.example").await;
    let ada = r.cookie().unwrap();
    let who = app.get("/auth/session", &ada).await;
    let ada_id: kaeso_core::id::UserId = who.body["user"]["id"].as_str().unwrap().parse().unwrap();
    let mut tx = app.state.pg.begin().await.unwrap();
    kaeso_server::services::orgs::add_member(
        &mut tx,
        beta.parse().unwrap(),
        ada_id,
        &kaeso_store::pg::members::Grant {
            preset: "member".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    InTwoOrgs {
        acme: owner.org_id,
        beta,
        ada,
    }
}

async fn access_of(app: &App, cookie: &str) -> std::collections::HashMap<String, String> {
    let who = app.get("/auth/session", cookie).await;
    who.body["memberships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["org_id"].as_str().unwrap().to_owned(),
                m["access"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_managed_accounts_sso_session_opens_every_org_and_the_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let InTwoOrgs { acme, beta, ada } = ada_in_two_orgs(&app, &idp).await;

    let who = app.get("/auth/session", &ada).await;
    assert_eq!(who.body["managed_by"]["org"], acme.as_str());
    assert_eq!(who.body["managed_by"]["name"], "Acme");
    let access = access_of(&app, &ada).await;
    assert_eq!((&*access[&acme], &*access[&beta]), ("ok", "ok"));
    app.get(&format!("/orgs/{beta}"), &ada)
        .await
        .expect_status(StatusCode::OK);
    sqlx::query("UPDATE organizations SET require_2fa = true WHERE id = $1")
        .bind(common::uuid(&beta))
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        access_of(&app, &ada).await[&beta],
        "ok",
        "the provider stands in for the second factor"
    );

    app.clock.advance(time::Duration::minutes(16));
    let r = app
        .post(
            "/me/password",
            json!({ "new_password": "another long password" }),
            Some(&ada),
        )
        .await;
    assert_eq!(
        r.error_type(),
        "recent_sign_in_required",
        "an account change still needs a recent sign-in"
    );
    let r = app
        .post("/me/orgs", json!({ "name": "Mine" }), Some(&ada))
        .await;
    assert_eq!(
        r.body["error"]["message"],
        "Only instance admins can create organizations on this install.",
        "the instance's own rule decides, not the way Ada signed in"
    );
    let r = app
        .call(
            axum::http::Method::DELETE,
            &format!("/me/memberships/{beta}"),
            None,
            Some(&ada),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    s.cleanup().await;
}

#[tokio::test]
async fn an_sso_session_reads_the_account_but_changes_it_only_after_another_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let InTwoOrgs {
        acme: owner_org,
        beta,
        ada,
    } = ada_in_two_orgs(&app, &idp).await;
    sqlx::query("UPDATE users SET email = 'ada@else.example' WHERE email = 'ada@acme.example'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let who = app.get("/auth/session", &ada).await;
    assert_eq!(
        who.body["managed_by"],
        serde_json::Value::Null,
        "an address at another domain is no longer the org's"
    );

    let access = access_of(&app, &ada).await;
    assert_eq!(access[&owner_org], "ok");
    assert_eq!(access[&beta], "needs_other_sign_in");
    assert_eq!(
        app.get(&format!("/orgs/{owner_org}"), &ada).await.status,
        StatusCode::OK
    );
    let r = app.get(&format!("/orgs/{beta}"), &ada).await;
    assert_eq!(
        (r.status, r.error_type()),
        (StatusCode::FORBIDDEN, "other_sign_in_required")
    );
    for path in ["/me", "/me/sso-links", "/me/keys", "/auth/sessions"] {
        assert_eq!(app.get(path, &ada).await.status, StatusCode::OK, "{path}");
    }
    for (method, path, body) in [
        (
            axum::http::Method::POST,
            "/me/email".to_owned(),
            Some(json!({ "email": "ada@else.example" })),
        ),
        (
            axum::http::Method::POST,
            "/me/password".to_owned(),
            Some(json!({ "new_password": "another long password" })),
        ),
        (axum::http::Method::POST, "/me/totp".to_owned(), None),
        (
            axum::http::Method::POST,
            "/me/orgs".to_owned(),
            Some(json!({ "name": "Mine" })),
        ),
        (
            axum::http::Method::DELETE,
            format!("/me/memberships/{beta}"),
            None,
        ),
        (axum::http::Method::DELETE, "/me".to_owned(), None),
    ] {
        let r = app.call(method.clone(), &path, body, Some(&ada)).await;
        assert_eq!(
            (r.status, r.error_type()),
            (StatusCode::FORBIDDEN, "other_sign_in_required"),
            "{method} {path}"
        );
    }
    let r = app
        .call(
            axum::http::Method::PATCH,
            "/me",
            Some(json!({ "name": "Ada L." })),
            Some(&ada),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    s.cleanup().await;
}

async fn own_key(app: &App, cookie: &str, org: &str, name: &str) -> String {
    let projects = app.get(&format!("/orgs/{org}/projects"), cookie).await;
    let project = projects.body["data"][0]["id"].as_str().unwrap().to_owned();
    let r = app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": name }),
            Some(cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    r.body["id"].as_str().unwrap().to_owned()
}

fn key_names(r: &common::Resp) -> Vec<String> {
    let mut names: Vec<String> = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn an_sso_session_lists_only_the_keys_of_the_org_it_may_open() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    own_key(&app, &owner.cookie, &owner.org_id, "acme-bot").await;
    let beta = app
        .post("/me/orgs", json!({ "name": "Beta" }), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::CREATED);
    let beta = beta.body["id"].as_str().unwrap().to_owned();
    let beta_key = own_key(&app, &owner.cookie, &beta, "beta-bot").await;

    let sso = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    let mine = app
        .get("/me/keys", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        key_names(&mine),
        ["acme-bot"],
        "a key in an org the SSO session may not open stays hidden"
    );
    let r = app.get(&format!("/me/keys/{beta_key}"), &sso).await;
    assert_eq!(r.error_type(), "other_sign_in_required");
    let r = app
        .get("/me/keys?include_archived=true", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(key_names(&r), ["acme-bot"]);

    let mine = app.get("/me/keys", &owner.cookie).await;
    assert_eq!(key_names(&mine), ["acme-bot", "beta-bot"]);
    sqlx::query("UPDATE organizations SET require_2fa = true WHERE id = $1")
        .bind(common::uuid(&beta))
        .execute(&app.state.pg)
        .await
        .unwrap();
    let mine = app.get("/me/keys", &owner.cookie).await;
    assert_eq!(
        key_names(&mine),
        ["acme-bot"],
        "a session without the second factor an org requires doesn't list its keys"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn accounts_an_orgs_sso_created_before_links_existed_link_at_their_next_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    set_default_role(&app, &owner.org_id, "member").await;

    let r = sso_as(&app, &idp, "sub-lin", "lin@acme.example").await;
    assert!(r.cookie().is_some());
    sqlx::query("DELETE FROM sso_identities")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let migration =
        include_str!("../../kaeso-store/migrations/postgres/20261020200000_sso_identities.sql");
    let backfill = &migration[migration.find("INSERT INTO sso_identities").unwrap()..];
    sqlx::raw_sql(sqlx::AssertSqlSafe(backfill.to_owned()))
        .execute(&app.state.pg)
        .await
        .unwrap();
    let pending: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT issuer, subject FROM sso_identities")
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert_eq!(pending, (None, None));

    let r = sso_as(&app, &idp, "sub-lin", "lin@acme.example").await;
    assert!(r.cookie().is_some(), "the first sign-in completes the link");
    let r = sso_as(&app, &idp, "sub-other", "lin@acme.example").await;
    refused_with(&r, LINK_REQUIRED);
    let r = sso_as(&app, &idp, "sub-lin", "lin@acme.example").await;
    assert!(r.cookie().is_some());
    s.cleanup().await;
}

#[tokio::test]
async fn an_unverified_domain_neither_signs_in_nor_creates_accounts() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    let evil = another_org(&app, "Evil").await;
    connect_org_with(&app, &evil, &idp, "acme.example", None, true, false).await;
    set_default_role(&app, &evil, "member").await;

    let r = app
        .post(
            "/auth/oidc/start",
            json!({ "email": "ada@acme.example" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    join_org(&app, &evil, &owner.user_id).await;
    let flow = start_as_member(&app, &owner.cookie, json!({ "org": evil })).await;
    let r = answer_as(&app, &idp, &flow, "sub-ada", "ada@acme.example").await;
    refused_with(&r, "no_verified_domain");
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(accounts, 1);

    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    set_default_role(&app, &owner.org_id, "member").await;
    let flow = start(&app, json!({ "email": "ada@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let who = app
        .get("/auth/session", &r.cookie().expect("signed in"))
        .await;
    assert_eq!(
        who.body["session"]["sso_org"],
        owner.org_id.as_str(),
        "the address routes to the org that verified its domain"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_token_without_email_verified_signs_in_only_where_the_connection_allows_it() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    idp.set_fault(Fault::NoEmailVerified);
    let r = sso_as(&app, &idp, "sub-ada", "ada@acme.example").await;
    refused_with(&r, "email_unverified");
    sqlx::query("UPDATE org_oidc SET require_email_verified = false")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let r = sso_as(&app, &idp, "sub-ada", "ada@acme.example").await;
    assert!(r.cookie().is_some(), "{}", location(&r));
    idp.set_fault(Fault::UnverifiedEmail);
    let r = sso_as(&app, &idp, "sub-ada", "ada@acme.example").await;
    refused_with(&r, "email_unverified");
    s.cleanup().await;
}

#[tokio::test]
async fn link_local_and_cloud_metadata_addresses_are_never_fetched() {
    for url in [
        "http://169.254.169.254/latest/meta-data/",
        "http://[fe80::1]/.well-known/openid-configuration",
        "http://[fd00:ec2::254]/latest/meta-data/",
    ] {
        let request = Request::get(url).body(Vec::new()).unwrap();
        let refused = kaeso_server::auth::oidc::fetch(true, request)
            .await
            .unwrap_err();
        assert!(
            refused.to_string().contains("private network"),
            "{url}: {refused}"
        );
    }
}

async fn confirm_as(
    app: &App,
    idp: &MockIdp,
    cookie: &str,
    org: &str,
    subject: &str,
    email: &str,
) -> (String, common::Resp) {
    let r = app
        .post(
            "/me/sign-in-again/sso",
            json!({ "org": org, "next": "/orgs/x/providers" }),
            Some(cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let url = r.body["authorization_url"].as_str().unwrap().to_owned();
    let binding = r.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize_as(&url, subject, email, "Someone");
    (
        url,
        callback(app, &auth.code, &auth.state, Some(&binding)).await,
    )
}

async fn add_provider_key(app: &App, org: &str, cookie: &str) -> common::Resp {
    app.post(
        &format!("/orgs/{org}/credentials"),
        json!({ "provider": "openai", "name": "Production", "secret": "sk-test-confirmed-000000000000" }),
        Some(cookie),
    )
    .await
}

#[tokio::test]
async fn an_sso_session_confirms_a_recent_sign_in_at_its_provider_and_keeps_its_account() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(
        &app,
        &owner.org_id,
        &idp,
        "acme.example",
        Some("Acme ID"),
        false,
    )
    .await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    let sso = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();

    let ways = app
        .get("/me/sign-in-again", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(ways.body["password"], true);
    assert_eq!(
        ways.body["set_password"], false,
        "the owner has a password already"
    );
    assert_eq!(
        ways.body["sso"],
        json!([]),
        "with a password, single sign-on isn't offered"
    );
    sqlx::query("UPDATE organizations SET require_sso = true")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let guest = app
        .get("/me/sign-in-again", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        (&guest.body["password"], &guest.body["sso"]),
        (&json!(true), &json!([])),
        "requiring SSO leaves a guest's own sign-in alone"
    );
    common::manage_domain(&app, &owner.org_id, "example.com").await;
    let managed = app
        .get("/me/sign-in-again", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        (&managed.body["password"], &managed.body["magic_link"]),
        (&json!(false), &json!(false)),
        "an account whose org requires SSO is confirmed only through it"
    );
    assert_eq!(
        managed.body["sso"],
        json!([{ "org_id": owner.org_id, "org_name": "Acme", "provider_name": "Acme ID" }])
    );
    sqlx::query("UPDATE organizations SET require_sso = false")
        .execute(&app.state.pg)
        .await
        .unwrap();
    sqlx::query("DELETE FROM org_oidc_domains WHERE domain = 'example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();

    app.clock.advance(time::Duration::minutes(16));
    assert_eq!(
        add_provider_key(&app, &owner.org_id, &sso)
            .await
            .error_type(),
        "recent_sign_in_required"
    );

    let (url, r) = confirm_as(
        &app,
        &idp,
        &sso,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    let query: std::collections::HashMap<String, String> = url::Url::parse(&url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    assert_eq!(
        query.get("prompt").map(String::as_str),
        Some("login"),
        "the provider asks the person again"
    );
    assert_eq!(query.get("max_age").map(String::as_str), Some("0"));
    assert_eq!(
        location(&r),
        format!("{ORIGIN}/orgs/x/providers"),
        "back to the page that asked"
    );
    let confirmed = r.cookie().expect("a new session");
    assert_ne!(confirmed, sso);
    let who = app.get("/auth/session", &confirmed).await;
    assert_eq!(who.body["user"]["email"], "owner@example.com");
    assert_eq!(who.body["session"]["sso_org"], owner.org_id.as_str());
    let added = add_provider_key(&app, &owner.org_id, &confirmed).await;
    assert_eq!(added.status, StatusCode::CREATED, "{:?}", added.body);
    let old = app.get("/auth/session", &sso).await;
    assert_eq!(
        old.status,
        StatusCode::UNAUTHORIZED,
        "the confirmed session replaces the old one"
    );
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action = 'user.sign_in_confirm'",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    assert_eq!(
        confirms_sign_in(&app, &owner.org_id, &confirmed).await,
        json!(true),
        "a fresh sign-in shows the provider confirms"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_confirmation_never_switches_accounts_and_comes_back_to_the_page_with_the_reason() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    let back = |r: &common::Resp, code: &str| {
        assert_eq!(
            location(r),
            format!("{ORIGIN}/orgs/x/providers#sso_error={code}")
        );
        assert!(
            r.cookie().is_none(),
            "no session for a refused confirmation"
        );
    };

    let (_, r) = confirm_as(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@example.com",
    )
    .await;
    back(&r, "domain_not_allowed");
    let (_, r) = confirm_as(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    back(&r, "other_account");

    let (eve, _) = common::join(
        &app,
        &owner,
        json!({ "email": "eve@acme.example", "role": "member" }),
        "Eve",
    )
    .await;
    link_account(
        &app,
        &idp,
        &eve,
        &owner.org_id,
        "sub-eve",
        "eve@acme.example",
    )
    .await;
    let (_, r) = confirm_as(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-eve",
        "eve@acme.example",
    )
    .await;
    back(&r, "other_account");
    sqlx::query("UPDATE users SET email = 'owner@acme.example' WHERE email = 'owner@example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let (_, r) = confirm_as(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    back(&r, LINK_REQUIRED);

    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    idp.set_fault(Fault::StaleAuthTime);
    let (_, r) = confirm_as(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    back(&r, "cannot_confirm");
    idp.set_fault(Fault::None);
    let who = app.get("/auth/session", &owner.cookie).await;
    assert_eq!(
        who.status,
        StatusCode::OK,
        "a refused confirmation leaves the session as it was"
    );

    let other = another_org(&app, "Other").await;
    let r = app
        .post(
            "/me/sign-in-again/sso",
            json!({ "org": other }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    s.cleanup().await;
}

async fn signed_in_ago(app: &App, cookie: &str) -> time::Duration {
    let who = app.get("/auth/session", cookie).await;
    let at = time::OffsetDateTime::parse(
        who.body["session"]["signed_in_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    app.clock.now() - at
}

#[track_caller]
fn about(ago: time::Duration, expected: time::Duration) {
    assert!(
        (ago - expected).abs() < time::Duration::seconds(1),
        "{ago} is not {expected}"
    );
}

#[tokio::test]
async fn an_sso_sign_in_is_only_as_recent_as_the_sign_in_at_the_provider() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;

    idp.set_fault(Fault::StaleAuthTime);
    let reused = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    about(signed_in_ago(&app, &reused).await, time::Duration::hours(1));
    assert_eq!(
        add_provider_key(&app, &owner.org_id, &reused)
            .await
            .error_type(),
        "recent_sign_in_required",
        "a session the provider signed in an hour ago is not a fresh sign-in"
    );

    idp.set_fault(Fault::NoAuthTime);
    let unknown = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    about(
        signed_in_ago(&app, &unknown).await,
        time::Duration::minutes(15) + time::Duration::seconds(1),
    );
    assert_eq!(
        add_provider_key(&app, &owner.org_id, &unknown)
            .await
            .error_type(),
        "recent_sign_in_required"
    );
    let (_, r) = confirm_as(
        &app,
        &idp,
        &unknown,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    assert_eq!(
        location(&r),
        format!("{ORIGIN}/orgs/x/providers#sso_error=cannot_confirm"),
        "a confirmation without auth_time can't prove a fresh sign-in"
    );
    assert!(r.cookie().is_none());
    assert_eq!(
        app.get("/auth/session", &unknown).await.status,
        StatusCode::OK
    );

    idp.set_fault(Fault::None);
    let fresh = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    about(signed_in_ago(&app, &fresh).await, time::Duration::ZERO);
    let added = add_provider_key(&app, &owner.org_id, &fresh).await;
    assert_eq!(added.status, StatusCode::CREATED, "{:?}", added.body);
    s.cleanup().await;
}

#[tokio::test]
async fn an_account_without_a_password_is_offered_one_by_email_to_confirm_with() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    let sso = sso_as(&app, &idp, "sub-new", "new@acme.example")
        .await
        .cookie()
        .unwrap();
    let ways = app
        .get("/me/sign-in-again", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(ways.body["password"], false);
    assert_eq!(ways.body["set_password"], true);
    assert_eq!(
        ways.body["sso"],
        json!([]),
        "a password set by email confirms, so single sign-on isn't offered"
    );
    s.cleanup().await;
}

async fn confirms_sign_in(app: &App, org: &str, cookie: &str) -> serde_json::Value {
    app.get(&format!("/orgs/{org}/security"), cookie)
        .await
        .expect_status(StatusCode::OK)
        .body["sso"]["confirms_sign_in"]
        .clone()
}

async fn confirming_providers(app: &App, cookie: &str) -> Vec<String> {
    app.get("/me/sign-in-again", cookie)
        .await
        .expect_status(StatusCode::OK)
        .body["sso"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["provider_name"].as_str().unwrap().to_owned())
        .collect()
}

async fn a_provider_that_cant_confirm_is_no_longer_offered(fault: Fault) {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &WITHOUT_PASSWORDS).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(
        &app,
        &owner.org_id,
        &idp,
        "acme.example",
        Some("Acme ID"),
        false,
    )
    .await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    idp.set_fault(fault);
    let sso = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    assert_eq!(
        confirms_sign_in(&app, &owner.org_id, &sso).await,
        json!(null),
        "a sign-in without max_age=0 proves nothing about a confirmation"
    );
    assert_eq!(
        confirming_providers(&app, &sso).await,
        ["Acme ID"],
        "with nothing else to confirm with, single sign-on is offered"
    );

    let (_, r) = confirm_as(
        &app,
        &idp,
        &sso,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    refused_with(&r, "cannot_confirm");
    assert_eq!(app.get("/auth/session", &sso).await.status, StatusCode::OK);
    assert_eq!(
        confirms_sign_in(&app, &owner.org_id, &sso).await,
        json!(false)
    );
    assert_eq!(
        confirming_providers(&app, &sso).await,
        Vec::<String>::new(),
        "a provider that can't confirm isn't offered again"
    );
    let linked = app
        .get("/me/sso-links", &sso)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        linked.body["data"][0]["linked"], true,
        "the account stays linked and signs in through it"
    );

    idp.set_fault(Fault::None);
    let later = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    assert_eq!(
        confirms_sign_in(&app, &owner.org_id, &later).await,
        json!(false),
        "a later sign-in with auth_time doesn't show that the provider honours max_age=0"
    );
    app.clock.advance(time::Duration::minutes(16));
    let (_, r) = confirm_as(
        &app,
        &idp,
        &later,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    assert!(r.cookie().is_some(), "a fresh sign-in still confirms");
    let confirmed = r.cookie().unwrap();
    assert_eq!(
        confirms_sign_in(&app, &owner.org_id, &confirmed).await,
        json!(true)
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_provider_that_leaves_out_auth_time_is_no_longer_offered_to_confirm_a_sign_in() {
    a_provider_that_cant_confirm_is_no_longer_offered(Fault::NoAuthTime).await;
}

#[tokio::test]
async fn a_provider_that_reuses_its_own_sign_in_is_no_longer_offered_to_confirm_a_sign_in() {
    a_provider_that_cant_confirm_is_no_longer_offered(Fault::StaleAuthTime).await;
}

#[tokio::test]
async fn a_new_issuer_or_client_id_forgets_whether_the_provider_confirms_a_sign_in() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    let sso = format!("/orgs/{}/security/sso", owner.org_id);
    let save = |client_id: &'static str, secret: Option<&'static str>| {
        let mut body =
            json!({ "issuer": idp.issuer(), "client_id": client_id, "domains": ["acme.example"] });
        if let Some(secret) = secret {
            body["client_secret"] = json!(secret);
        }
        app.call(
            axum::http::Method::PUT,
            &sso,
            Some(body),
            Some(&owner.cookie),
        )
    };
    oidc::record_confirms_sign_in(
        &app.state.pg,
        common::uuid(&owner.org_id),
        idp.issuer(),
        false,
    )
    .await
    .unwrap();

    let kept = save("kaeso", None).await.expect_status(StatusCode::OK);
    assert_eq!(
        kept.body["sso"]["confirms_sign_in"],
        json!(false),
        "the same provider and app keep what their confirmations showed"
    );
    let changed = save("kaeso-new", Some("client-secret"))
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(changed.body["sso"]["confirms_sign_in"], json!(null));
    s.cleanup().await;
}

#[tokio::test]
async fn instance_admins_administer_through_single_sign_on_only_from_the_chosen_org() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "example.com", None, false).await;
    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@example.com",
    )
    .await;
    let sso = sso_as(&app, &idp, "sub-owner", "owner@example.com")
        .await
        .cookie()
        .unwrap();
    let admin_sso = |cookie: String, org: serde_json::Value| {
        let app = &app;
        async move {
            app.call(
                axum::http::Method::PUT,
                "/instance/settings/admin-sso",
                Some(json!({ "org": org })),
                Some(&cookie),
            )
            .await
        }
    };

    let who = app.get("/auth/session", &sso).await;
    assert_eq!(who.body["managed_by"]["org"], owner.org_id.as_str());
    assert_eq!(
        who.body["can_administer_instance"], false,
        "no org is chosen"
    );
    assert_eq!(
        app.get("/instance/users", &sso).await.error_type(),
        "other_sign_in_required"
    );

    let other = another_org(&app, "Elsewhere").await;
    let connected = app.get("/instance/orgs?sso=true", &owner.cookie).await;
    assert_eq!(
        connected.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Acme"]
    );
    let r = admin_sso(owner.cookie.clone(), json!(other)).await;
    assert_eq!(
        (r.status, r.body["error"]["param"].as_str()),
        (StatusCode::BAD_REQUEST, Some("org")),
        "an org without a connection manages nobody"
    );
    let r = admin_sso(owner.cookie.clone(), json!(owner.org_id)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["admin_sso"],
        json!({ "org": owner.org_id, "name": "Acme", "connected": true })
    );
    let audited = app
        .get(
            "/instance/audit-log?action=instance.admin_sso_update",
            &owner.cookie,
        )
        .await;
    assert_eq!(
        audited.body["data"][0]["after"]["org"],
        owner.org_id.as_str()
    );

    let who = app.get("/auth/session", &sso).await;
    assert_eq!(who.body["can_administer_instance"], true);
    app.get("/instance/users", &sso)
        .await
        .expect_status(StatusCode::OK);
    let r = admin_sso(sso.clone(), serde_json::Value::Null).await;
    assert_eq!(
        r.error_type(),
        "other_sign_in_required",
        "an org's provider can't choose who administers the instance"
    );

    sqlx::query("UPDATE users SET email = 'owner@else.example'")
        .execute(&app.state.pg)
        .await
        .unwrap();
    assert_eq!(
        app.get("/instance/users", &sso).await.error_type(),
        "other_sign_in_required",
        "the org no longer manages the account"
    );
    sqlx::query("UPDATE users SET email = 'owner@example.com'")
        .execute(&app.state.pg)
        .await
        .unwrap();

    app.clock.advance(time::Duration::minutes(16));
    let r = admin_sso(owner.cookie.clone(), serde_json::Value::Null).await;
    assert_eq!(r.error_type(), "recent_sign_in_required");

    sqlx::query("DELETE FROM org_oidc")
        .execute(&app.state.pg)
        .await
        .unwrap();
    let settings = app.get("/instance/settings", &owner.cookie).await;
    assert_eq!(
        settings.body["admin_sso"]["connected"], false,
        "without its connection the setting has no effect"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_operator_chooses_the_instance_admins_sso_org_from_the_server() {
    use kaeso_server::services::admin_sso::{OperatorAdminSso, operator_set};

    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "example.com", None, false).await;
    let elsewhere = another_org(&app, "Elsewhere").await;
    let set = |org: String| {
        let app = &app;
        async move { operator_set(&app.state, &org).await.unwrap() }
    };
    let audited = || async {
        app.get(
            "/instance/audit-log?action=instance.admin_sso_update",
            &owner.cookie,
        )
        .await
        .body["data"]
            .as_array()
            .unwrap()
            .clone()
    };

    assert_eq!(set("acme".into()).await, OperatorAdminSso::NotAnOrg);
    assert_eq!(
        set(OrgId::new().to_string()).await,
        OperatorAdminSso::UnknownOrg
    );
    assert_eq!(
        set(elsewhere.clone()).await,
        OperatorAdminSso::NotConnected,
        "the same rule as the console's form"
    );
    assert!(audited().await.is_empty(), "a refused change isn't audited");

    assert_eq!(
        set(owner.org_id.clone()).await,
        OperatorAdminSso::Chosen {
            org: owner.org_id.parse().unwrap(),
            name: "Acme".into()
        }
    );
    let settings = app.get("/instance/settings", &owner.cookie).await;
    assert_eq!(
        settings.body["admin_sso"],
        json!({ "org": owner.org_id, "name": "Acme", "connected": true })
    );
    let events = audited().await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["actor"]["kind"], "system");
    assert_eq!(events[0]["before"], json!({ "org": null, "name": null }));
    assert_eq!(
        events[0]["after"],
        json!({ "org": owner.org_id, "name": "Acme", "by": "operator" })
    );

    assert_eq!(set("none".into()).await, OperatorAdminSso::Cleared);
    let settings = app.get("/instance/settings", &owner.cookie).await;
    assert_eq!(settings.body["admin_sso"], json!(null));
    let events = audited().await;
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0]["before"],
        json!({ "org": owner.org_id, "name": "Acme" })
    );
    assert_eq!(
        events[0]["after"],
        json!({ "org": null, "name": null, "by": "operator" })
    );
    s.cleanup().await;
}

const MANAGED_ADDRESS: &str =
    "Your organization manages this address; change it at its identity provider.";

#[tokio::test]
async fn the_identity_provider_owns_a_managed_accounts_address() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect_org(&app, &owner.org_id, &idp, "acme.example", None, false).await;
    set_default_role(&app, &owner.org_id, "member").await;
    let ada = sso_as(&app, &idp, "sub-ada", "ada@acme.example")
        .await
        .cookie()
        .unwrap();
    sso_as(&app, &idp, "sub-bob", "bob@acme.example")
        .await
        .cookie()
        .unwrap();
    let email_of = |cookie: String| {
        let app = &app;
        async move {
            app.get("/auth/session", &cookie).await.body["user"]["email"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };

    let r = app
        .post(
            "/me/email",
            json!({ "email": "ada@else.example" }),
            Some(&ada),
        )
        .await;
    assert_eq!(
        (r.status, r.body["error"]["message"].as_str().unwrap()),
        (StatusCode::FORBIDDEN, MANAGED_ADDRESS)
    );

    common::manage_domain(&app, &owner.org_id, "eu.acme.example").await;
    let renamed = sso_as(&app, &idp, "sub-ada", "Ada.Lovelace@eu.acme.example")
        .await
        .cookie()
        .expect("signed in through the identity");
    assert_eq!(email_of(renamed).await, "Ada.Lovelace@eu.acme.example");
    let audited = app
        .get(
            &format!("/orgs/{}/audit-log?action=user.email_change", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(
        audited.body["data"][0]["before"]["email"],
        "ada@acme.example"
    );
    assert_eq!(audited.body["data"][0]["after"]["by"], "identity_provider");

    let kept = sso_as(&app, &idp, "sub-ada", "bob@acme.example")
        .await
        .cookie()
        .expect("still signed in");
    assert_eq!(
        email_of(kept).await,
        "Ada.Lovelace@eu.acme.example",
        "another account's address is never taken"
    );

    link_account(
        &app,
        &idp,
        &owner.cookie,
        &owner.org_id,
        "sub-owner",
        "owner@acme.example",
    )
    .await;
    let guest = sso_as(&app, &idp, "sub-owner", "owner@acme.example")
        .await
        .cookie()
        .unwrap();
    assert_eq!(
        email_of(guest).await,
        "owner@example.com",
        "an address the org doesn't manage stays the account's own"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn signing_in_again_offers_single_sign_on_only_when_nothing_else_confirms() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let acme_idp = MockIdp::start("kaeso", "client-secret").await;
    let beta_idp = MockIdp::start("kaeso", "client-secret").await;
    let beta = app
        .post("/me/orgs", json!({ "name": "Beta" }), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::CREATED);
    let beta = beta.body["id"].as_str().unwrap().to_owned();
    connect_org(&app, &owner.org_id, &acme_idp, "acme.example", None, false).await;
    connect_org(
        &app,
        &beta,
        &beta_idp,
        "example.com",
        Some("Beta ID"),
        false,
    )
    .await;
    link_account(
        &app,
        &acme_idp,
        &owner.cookie,
        &owner.org_id,
        "sub-acme",
        "owner@acme.example",
    )
    .await;
    link_account(
        &app,
        &beta_idp,
        &owner.cookie,
        &beta,
        "sub-beta",
        "owner@example.com",
    )
    .await;
    let orgs_offered = |r: &common::Resp| -> Vec<String> {
        r.body["sso"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["org_name"].as_str().unwrap().to_owned())
            .collect()
    };

    let ways = app
        .get("/me/sign-in-again", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        (&ways.body["password"], &ways.body["magic_link"]),
        (&json!(true), &json!(true))
    );
    assert_eq!(
        ways.body["sso"],
        json!([]),
        "with a password, not even the managing org's provider is offered"
    );

    let without_passwords = App::start_with(&s, &WITHOUT_PASSWORDS).await;
    let ways = without_passwords
        .get("/me/sign-in-again", &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(
        orgs_offered(&ways),
        ["Beta", "Acme"],
        "Beta manages the account, so it comes first"
    );

    sqlx::query("UPDATE org_oidc SET confirms_sign_in = false WHERE org_id = $1")
        .bind(common::uuid(&beta))
        .execute(&app.state.pg)
        .await
        .unwrap();
    let ways = without_passwords
        .get("/me/sign-in-again", &owner.cookie)
        .await;
    assert_eq!(
        orgs_offered(&ways),
        ["Acme"],
        "a provider that couldn't confirm isn't offered"
    );
    s.cleanup().await;
}
