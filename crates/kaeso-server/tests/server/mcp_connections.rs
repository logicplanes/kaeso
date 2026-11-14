#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{App, ORIGIN, Owner, uuid};
use kaeso_server::services::mcp_connections::{self, ConnectionError, SignInError};
use kaeso_store::pg::mcp_connections as connections;
use kaeso_store::pg::mcp_servers::{self as servers, NewServer, ServerRow};
use kaeso_testkit::mcp::{MockMcp, OAuth, Options};
use kaeso_testkit::services::TestServices;
use kaeso_upstream::mcp::{Client, Limits, SecretHeader};
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;
use url::Url;

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn server(
    app: &App,
    owner: &Owner,
    url: &str,
    auth: &str,
    mode: &str,
    registration: Option<&str>,
) -> ServerRow {
    let id = uuid::Uuid::now_v7();
    let org = uuid(&owner.org_id);
    let handle = format!("x{}", &id.simple().to_string()[24..]);
    assert!(
        servers::insert(
            &app.state.pg,
            &NewServer {
                id,
                org_id: org,
                handle: &handle,
                name: "X",
                url,
                auth_method: auth,
                credential_mode: mode,
                oauth_registration: registration,
                scopes: &[],
                change_policy: "hold",
                created_by_kind: "system",
                created_by_id: None,
            },
        )
        .await
        .unwrap()
    );
    servers::get(&app.state.pg, org, id).await.unwrap().unwrap()
}

async fn reload(app: &App, row: &ServerRow) -> ServerRow {
    servers::get(&app.state.pg, row.org_id, row.id)
        .await
        .unwrap()
        .unwrap()
}

async fn follow(app: &App, back: &str, cookie: Option<&str>) -> String {
    let url = Url::parse(back).unwrap();
    assert_eq!(url.origin().ascii_serialization(), ORIGIN);
    let path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
    let mut req = Request::builder().uri(path);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, format!("__Host-kaeso_session={c}"));
    }
    let r = app.raw(req.body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    r.headers[header::LOCATION].to_str().unwrap().to_owned()
}

async fn connect(app: &App, owner: &Owner, mock: &MockMcp, row: &ServerRow) -> String {
    let begun = mcp_connections::begin(&app.state, row, None, uuid(&owner.user_id), None)
        .await
        .unwrap();
    follow(app, &mock.authorize(&begun.url), Some(&owner.cookie)).await
}

fn connected_page(owner: &Owner, row: &ServerRow, result: &str) -> String {
    format!(
        "{ORIGIN}/orgs/{}/mcp-servers?server={}&result={result}",
        owner.org_id,
        kaeso_core::id::McpServerId::from_uuid(row.id)
    )
}

#[tokio::test]
async fn kaeso_publishes_its_client_metadata_document() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let r = app
        .raw(
            Request::builder()
                .uri("/oauth/client.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["client_id"], format!("{ORIGIN}/oauth/client.json"));
    assert_eq!(
        r.body["redirect_uris"][0],
        format!("{ORIGIN}/oauth/upstream/callback")
    );
    assert_eq!(r.body["token_endpoint_auth_method"], "none");
    assert_eq!(r.body["application_type"], "web");
    assert_eq!(r.headers[header::CACHE_CONTROL], "public, max-age=3600");
}

#[tokio::test]
async fn a_shared_oauth_connection_signs_in_with_the_client_metadata_document() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth::default()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;

    let begun = mcp_connections::begin(&app.state, &row, None, uuid(&owner.user_id), None)
        .await
        .unwrap();
    let asked = Url::parse(&begun.url).unwrap();
    let q: std::collections::HashMap<_, _> = asked.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], format!("{ORIGIN}/oauth/client.json"));
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["resource"], mock.url());
    assert_eq!(q["scope"], "posts.read posts.write");
    assert_eq!(
        q["redirect_uri"],
        format!("{ORIGIN}/oauth/upstream/callback")
    );

    let back = mock.authorize(&begun.url);
    assert_eq!(
        follow(&app, &back, Some(&owner.cookie)).await,
        connected_page(&owner, &row, "connected")
    );
    let token = &mock.token_requests()[0];
    assert_eq!(token["grant_type"], "authorization_code");
    assert_eq!(token["client_id"], format!("{ORIGIN}/oauth/client.json"));
    assert!(token.contains_key("code_verifier"));

    let stored = connections::of_server(&app.state.pg, row.id).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].account_label.as_deref(), Some("kaesoapp"));
    assert_eq!(stored[0].issuer.as_deref(), Some(mock.base().as_str()));
    assert_eq!(stored[0].last4, "at-1");
    let reloaded = reload(&app, &row).await;
    assert_eq!(reloaded.oauth_registration.as_deref(), Some("cimd"));
    assert_eq!(reloaded.oauth_issuer.as_deref(), Some(mock.base().as_str()));

    let credential = mcp_connections::credential(&app.state, &reloaded, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(credential.header, "authorization");
    assert_eq!(credential.value.expose_secret(), "Bearer at-1");
    let url = Url::parse(&mock.url()).unwrap();
    let client = Client {
        upstream: &app.state.mcp,
        url: &url,
        secret: Some(SecretHeader {
            name: &credential.header,
            value: &credential.value,
        }),
        limits: Limits {
            first_byte: Duration::from_secs(5),
            idle: Duration::from_secs(5),
            max_body: 1 << 20,
        },
    };
    let hs = client.handshake().await.unwrap();
    assert_eq!(client.list_tools(&hs).await.unwrap().len(), 3);

    assert_eq!(
        follow(&app, &back, Some(&owner.cookie)).await,
        format!("{ORIGIN}/?mcp_sign_in=expired")
    );
}

#[tokio::test]
async fn only_the_person_who_started_a_sign_in_can_finish_it() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth::default()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;
    let begun = mcp_connections::begin(&app.state, &row, None, uuid(&owner.user_id), None)
        .await
        .unwrap();
    let back = mock.authorize(&begun.url);
    assert_eq!(
        follow(&app, &back, None).await,
        connected_page(&owner, &row, "expired")
    );
    assert!(
        connections::of_server(&app.state.pg, row.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(mock.token_requests().is_empty());
}

#[tokio::test]
async fn an_answer_from_another_issuer_is_never_redeemed() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth::default()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;
    let begun = mcp_connections::begin(&app.state, &row, None, uuid(&owner.user_id), None)
        .await
        .unwrap();
    let back = mock.authorize(&begun.url).replace(
        &format!(
            "iss={}",
            url::form_urlencoded::byte_serialize(mock.base().as_bytes()).collect::<String>()
        ),
        "iss=https%3A%2F%2Fevil.example.com",
    );
    assert_eq!(
        follow(&app, &back, Some(&owner.cookie)).await,
        connected_page(&owner, &row, "failed")
    );
    assert!(
        connections::of_server(&app.state.pg, row.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(mock.token_requests().is_empty());
}

#[tokio::test]
async fn without_metadata_documents_kaeso_registers_and_signs_in_as_a_confidential_client() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: false,
            confidential: true,
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;
    let back = connect(&app, &owner, &mock, &row).await;
    assert_eq!(back, connected_page(&owner, &row, "connected"));
    let reloaded = reload(&app, &row).await;
    assert_eq!(reloaded.oauth_registration.as_deref(), Some("dcr"));
    let client_id = reloaded.oauth_client_id.clone().unwrap();
    assert_eq!(mock.registered_clients(), std::slice::from_ref(&client_id));
    assert!(reloaded.has_client_secret);
    assert!(!mock.token_requests()[0].contains_key("client_secret"));

    let again = connect(&app, &owner, &mock, &reloaded).await;
    assert_eq!(again, connected_page(&owner, &row, "connected"));
    assert_eq!(
        mock.registered_clients().len(),
        1,
        "the client is reused for the same issuer"
    );
}

#[tokio::test]
async fn a_server_without_registration_takes_a_client_entered_by_hand() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: false,
            dcr: false,
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;
    let refused = mcp_connections::begin(&app.state, &row, None, uuid(&owner.user_id), None).await;
    assert!(
        matches!(refused, Err(SignInError::NoRegistration)),
        "{refused:?}"
    );

    mcp_connections::set_manual_client(&app.state, row.id, "kaeso-by-hand", None)
        .await
        .unwrap();
    let manual = reload(&app, &row).await;
    assert_eq!(manual.oauth_issuer, None);
    assert_eq!(
        connect(&app, &owner, &mock, &manual).await,
        connected_page(&owner, &row, "connected")
    );
    assert_eq!(mock.token_requests()[0]["client_id"], "kaeso-by-hand");
    assert_eq!(
        reload(&app, &row).await.oauth_issuer.as_deref(),
        Some(mock.base().as_str())
    );
}

#[tokio::test]
async fn an_oauth_token_is_renewed_in_its_last_minute_and_signs_out_once() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            expires_in: 120,
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "oauth", "shared", Some("cimd")).await;
    connect(&app, &owner, &mock, &row).await;
    let row = reload(&app, &row).await;
    let bearer =
        |c: Option<mcp_connections::Credential>| c.unwrap().value.expose_secret().to_owned();

    app.clock.advance(time::Duration::seconds(59));
    assert_eq!(
        bearer(
            mcp_connections::credential(&app.state, &row, None)
                .await
                .unwrap()
        ),
        "Bearer at-1"
    );
    app.clock.advance(time::Duration::seconds(2));
    assert_eq!(
        bearer(
            mcp_connections::credential(&app.state, &row, None)
                .await
                .unwrap()
        ),
        "Bearer at-2"
    );
    let refresh = mock.token_requests().last().cloned().unwrap();
    assert_eq!(refresh["grant_type"], "refresh_token");
    assert_eq!(refresh["refresh_token"], "rt-1");
    assert_eq!(refresh["resource"], mock.url());
    assert_eq!(
        bearer(
            mcp_connections::credential(&app.state, &row, None)
                .await
                .unwrap()
        ),
        "Bearer at-2"
    );

    mock.revoke_refresh_tokens();
    app.clock.advance(time::Duration::seconds(61));
    let asked = mock.token_requests().len();
    let out = mcp_connections::credential(&app.state, &row, None).await;
    assert!(matches!(out, Err(ConnectionError::SignedOut)), "{out:?}");
    assert_eq!(mock.token_requests().len(), asked + 1);
    let stored = connections::sealed(&app.state.pg, row.id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.auth_error.as_deref(), Some("signed_out"));
    let again = mcp_connections::credential(&app.state, &row, None).await;
    assert!(matches!(again, Err(ConnectionError::SignedOut)));
    assert_eq!(
        mock.token_requests().len(),
        asked + 1,
        "a signed-out connection asks no more"
    );
}

#[tokio::test]
async fn a_secret_header_is_sent_as_given_and_kept_per_person() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("upstream-token".to_owned()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let shared = server(&app, &owner, &mock.url(), "header", "shared", None).await;
    for (bad, value) in [
        ("Host", "x"),
        ("mcp-session-id", "x"),
        ("x y", "x"),
        ("X-Api-Key", "line\nbreak"),
    ] {
        let r =
            mcp_connections::set_header(&app.state, &shared, None, bad, &SecretString::from(value))
                .await;
        assert!(matches!(r, Err(SignInError::BadHeader(_))), "{bad}");
    }
    assert!(matches!(
        mcp_connections::credential(&app.state, &shared, None).await,
        Err(ConnectionError::NotConnected)
    ));
    mcp_connections::set_header(
        &app.state,
        &shared,
        None,
        "Authorization",
        &SecretString::from("Bearer upstream-token"),
    )
    .await
    .unwrap();
    let c = mcp_connections::credential(&app.state, &shared, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (c.header.as_str(), c.value.expose_secret()),
        ("Authorization", "Bearer upstream-token")
    );
    assert_eq!(
        connections::of_server(&app.state.pg, shared.id)
            .await
            .unwrap()[0]
            .last4,
        "oken"
    );

    servers::delete(&app.state.pg, shared.org_id, shared.id)
        .await
        .unwrap();
    let personal = server(&app, &owner, &mock.url(), "header", "per_user", None).await;
    let me = uuid(&owner.user_id);
    assert!(matches!(
        mcp_connections::credential(&app.state, &personal, None).await,
        Err(ConnectionError::NotConnected)
    ));
    mcp_connections::set_header(
        &app.state,
        &personal,
        Some(me),
        "X-Api-Key",
        &SecretString::from("mine"),
    )
    .await
    .unwrap();
    let mine = mcp_connections::credential(&app.state, &personal, Some(me))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(mine.value.expose_secret(), "mine");
    assert!(matches!(
        mcp_connections::credential(&app.state, &personal, Some(uuid::Uuid::now_v7())).await,
        Err(ConnectionError::NotConnected)
    ));
    let none = server(&app, &owner, &mock.url(), "none", "shared", None).await;
    assert!(
        mcp_connections::credential(&app.state, &none, None)
            .await
            .unwrap()
            .is_none()
    );
}
