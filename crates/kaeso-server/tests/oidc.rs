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
            display_name: name.map(str::to_owned),
            auto_redirect,
        },
    )
    .await
    .unwrap();
    assert_eq!(stored, oidc::Upserted::Saved);
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
            expires_at: app.clock.now() + time::Duration::days(21),
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
            expires_at: app.clock.now() + time::Duration::days(21),
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

const SSO_ONLY: [(&str, &str); 2] = [
    ("KAESO_AUTH_PASSWORD", "false"),
    ("KAESO_AUTH_MAGIC_LINK", "false"),
];

async fn follow(app: &App, query: &str) -> common::Resp {
    app.raw(
        Request::get(format!("/platform/v1/auth/oidc/start{query}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
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

#[tokio::test]
async fn an_sso_only_install_offers_its_only_provider_without_an_address() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &SSO_ONLY).await;
    let owner = app.setup().await;
    let page = methods_page(&app, "").await;
    assert_eq!(
        (&page["password"], &page["magic_link"], &page["sso"]),
        (&json!(false), &json!(false), &json!(true))
    );
    assert_eq!(page["provider"], serde_json::Value::Null);

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
    assert_eq!(
        methods_page(&app, "").await["provider"],
        json!({ "org": owner.org_id, "name": "Acme ID", "auto_redirect": true })
    );

    let beta = another_org(&app, "Beta").await;
    connect_org(&app, &beta, &idp, "beta.example", None, false).await;
    assert_eq!(
        methods_page(&app, "").await["provider"],
        serde_json::Value::Null,
        "with two providers the page asks for the address"
    );
    assert_eq!(
        methods_page(&app, &format!("?org={beta}")).await["provider"],
        json!({ "org": beta, "name": null, "auto_redirect": false })
    );
    let by_address = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@acme.example" }),
            None,
        )
        .await;
    assert_eq!(by_address.body["sso"], true);
    assert_eq!(by_address.body["provider"]["name"], "Acme ID");
    assert_eq!(by_address.body["provider"]["org"], owner.org_id.as_str());
    let elsewhere = app
        .post(
            "/auth/methods",
            json!({ "email": "ada@else.example" }),
            None,
        )
        .await;
    assert_eq!(
        (&elsewhere.body["sso"], &elsewhere.body["provider"]),
        (&json!(false), &serde_json::Value::Null)
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_org_sign_in_link_names_its_provider_on_any_install() {
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
    assert_eq!(
        methods_page(&app, "").await["provider"],
        serde_json::Value::Null,
        "with passwords on, the page asks for the address first"
    );
    assert_eq!(
        methods_page(&app, &format!("?org={}", owner.org_id)).await["provider"]["name"],
        "Acme ID"
    );
    let unknown = OrgId::new();
    assert_eq!(
        methods_page(&app, &format!("?org={unknown}")).await["provider"],
        serde_json::Value::Null
    );

    let off = App::start_with(&s, &[("KAESO_AUTH_OIDC", "false")]).await;
    assert_eq!(
        methods_page(&off, &format!("?org={}", owner.org_id)).await["provider"],
        serde_json::Value::Null
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_start_link_goes_to_the_provider_and_back_to_the_page_asked_for() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &SSO_ONLY).await;
    let owner = app.setup().await;
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;

    let r = follow(&app, "?next=%2Fsettings%2Faccount%3Ftab%3Dsessions").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.headers);
    let to = location(&r);
    assert!(to.starts_with(idp.issuer()), "{to}");
    assert!(to.contains("code_challenge_method=S256"), "{to}");
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    let binding = r.set_cookie("__Host-kaeso_oidc").unwrap();
    let auth = idp.authorize(&to, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&binding)).await;
    assert_eq!(
        location(&back),
        format!("{ORIGIN}/settings/account?tab=sessions")
    );
    let cookie = back.cookie().expect("signed in");
    assert_eq!(
        app.get("/auth/session", &cookie).await.body["user"]["email"],
        "ada@acme.example"
    );

    let flow = start(
        &app,
        json!({ "org": owner.org_id, "next": "/orgs/x/projects/y/logs#top" }),
    )
    .await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(
        location(&back),
        format!("{ORIGIN}/orgs/x/projects/y/logs#top")
    );

    let flow = start(&app, json!({ "email": "ada@acme.example" })).await;
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(location(&back), format!("{ORIGIN}/"));
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
                json!({ "org": owner.org_id, "next": next }),
                None,
            )
            .await;
        assert_eq!(
            (r.status, r.body["error"]["param"].as_str()),
            (StatusCode::BAD_REQUEST, Some("next")),
            "{next:?}"
        );
        let r = follow(
            &app,
            &format!("?org={}&next={}", owner.org_id, urlencode(next)),
        )
        .await;
        let to = location(&r);
        assert!(
            to.starts_with(&format!("{ORIGIN}/sign-in#sso_error=")),
            "{next:?}: {to}"
        );
        assert!(r.set_cookie("__Host-kaeso_oidc").is_none(), "{next:?}");
    }

    let flow = start(&app, json!({ "org": owner.org_id, "next": "/orgs" })).await;
    sqlx::query(
        "UPDATE sign_in_tokens SET data = jsonb_set(data, '{next}', '\"//evil.example\"') WHERE kind = 'oidc_state'",
    )
    .execute(&app.state.pg)
    .await
    .unwrap();
    let auth = idp.authorize(&flow.url, "ada@acme.example", "Ada Lovelace");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    assert_eq!(location(&back), format!("{ORIGIN}/"));

    let flow = start(&app, json!({ "org": owner.org_id, "next": "/orgs?a=1" })).await;
    let auth = idp.authorize(&flow.url, "eve@else.example", "Eve");
    let back = callback(&app, &auth.code, &auth.state, Some(&flow.binding)).await;
    let to = location(&back);
    assert!(
        to.starts_with(&format!("{ORIGIN}/sign-in?next=%2Forgs%3Fa%3D1#sso_error=")),
        "a refused sign-in keeps the page to come back to: {to}"
    );
    assert!(back.cookie().is_none());
    s.cleanup().await;
}

#[tokio::test]
async fn a_start_link_without_an_org_needs_exactly_one_provider() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let to = location(&follow(&app, "?next=%2Forgs").await);
    assert!(
        to.starts_with(&format!(
            "{ORIGIN}/sign-in?next=%2Forgs#sso_error=No%20organization"
        )),
        "{to}"
    );
    let idp = MockIdp::start("kaeso", "client-secret").await;
    connect(&app, &owner, &idp, Some("member")).await;
    assert!(location(&follow(&app, "").await).starts_with(idp.issuer()));
    let beta = another_org(&app, "Beta").await;
    connect_org(&app, &beta, &idp, "beta.example", None, false).await;
    let to = location(&follow(&app, "").await);
    assert!(to.contains("sso_error=Name%20the%20organization"), "{to}");
    assert!(location(&follow(&app, &format!("?org={beta}")).await).starts_with(idp.issuer()));
    let to = location(&follow(&app, "?org=not-an-org").await);
    assert!(
        to.starts_with(&format!("{ORIGIN}/sign-in#sso_error=")),
        "{to}"
    );

    let off = App::start_with(&s, &[("KAESO_AUTH_OIDC", "false")]).await;
    let to = location(&follow(&off, &format!("?org={beta}")).await);
    assert!(to.contains("turned%20off"), "{to}");
    s.cleanup().await;
}

#[tokio::test]
async fn sign_in_reads_and_start_links_need_no_origin_but_changes_do() {
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
    let expected = format!("Cross-site request refused; send the header `Origin: {ORIGIN}`.");
    for (path, body) in [
        ("/auth/methods", json!({ "email": "ada@acme.example" })),
        ("/auth/oidc/start", json!({ "org": owner.org_id })),
    ] {
        let r = bare(axum::http::Method::POST, path, Some(body)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(r.body["error"]["message"], expected.as_str(), "{path}");
    }
    let r = follow(&app, &format!("?org={}", owner.org_id)).await;
    assert!(location(&r).starts_with(idp.issuer()));
    s.cleanup().await;
}
