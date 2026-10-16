#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{App, ORIGIN, Owner};
use kaeso_core::id::OrgId;
use kaeso_store::pg::oidc;
use kaeso_testkit::oidc::{Fault, MockIdp};
use kaeso_testkit::services::TestServices;
use serde_json::json;

async fn connect(app: &App, owner: &Owner, idp: &MockIdp, default_preset: Option<&str>) {
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
        },
    )
    .await
    .unwrap()
    .unwrap();
    sqlx::query("UPDATE organizations SET sso_default_preset = $1")
        .bind(default_preset)
        .execute(&app.state.pg)
        .await
        .unwrap();
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
    assert_eq!(known["sso"], false);
    assert_eq!(known["password"], true);
    assert_eq!(known["magic_link"], true);

    let sso = methods("ada@ACME.example").await;
    assert_eq!(sso["sso"], true);
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
    let flow = start(&app, json!({ "org": owner.org_id })).await;
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
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(21),
        },
    )
    .await
    .unwrap();
    let flow = start(&app, json!({ "org": owner.org_id })).await;
    let auth = idp.authorize(&flow.url, "bob@acme.example", "Bob");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let who = app
        .get("/auth/session", &r.cookie().expect("signed in"))
        .await;
    assert_eq!(who.body["memberships"][0]["preset"], "admin");
    s.cleanup().await;
}

#[tokio::test]
async fn sso_signs_in_existing_accounts_only_where_they_are_members() {
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
        let flow = start(&app, json!({ "org": owner.org_id })).await;
        let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
        callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await
    };

    let r = sign_in().await;
    assert!(location(&r).contains("sso_error="), "{}", location(&r));
    assert!(r.cookie().is_none());

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
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(21),
        },
    )
    .await
    .unwrap();
    let r = sign_in().await;
    assert!(
        location(&r).contains("sign%20in%20to%20accept"),
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
    for fault in [
        Fault::WrongNonce,
        Fault::WrongAudience,
        Fault::Expired,
        Fault::WrongIssuer,
        Fault::UnverifiedEmail,
    ] {
        idp.set_fault(fault.clone());
        let flow = start(&app, json!({ "org": owner.org_id })).await;
        let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
        let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
        assert!(
            location(&r).contains("sso_error="),
            "{fault:?}: {}",
            location(&r)
        );
        assert!(r.cookie().is_none(), "{fault:?}");
    }
    idp.set_fault(Fault::None);
    let flow = start(&app, json!({ "org": owner.org_id })).await;
    let auth = idp.authorize(&flow.url, "owner@example.com", "Owner");
    let r = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert!(location(&r).contains("acme.example"), "{}", location(&r));
    assert!(r.cookie().is_none());
    let flow = start(&app, json!({ "org": owner.org_id })).await;
    let auth = idp.authorize(&flow.url, "eve@acme.example", "Eve");
    let r = callback(&app, &auth.code, &auth.state, None).await;
    assert!(location(&r).contains("sso_error="));
    let r = callback(&app, &auth.code, &auth.state, Some("other-browser")).await;
    assert!(location(&r).contains("sso_error="));
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
        .post("/auth/oidc/start", json!({ "org": owner.org_id }), None)
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("private network"),
        "{:?}",
        r.body
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
