#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

// The request bodies follow the identity providers' own documentation: Okta's SCIM 2.0 guide
// (https://developer.okta.com/docs/api/openapi/okta-scim/guides/scim-20) and Microsoft's
// (https://learn.microsoft.com/en-us/entra/identity/app-provisioning/use-scim-to-provision-users-and-groups,
// https://learn.microsoft.com/en-us/entra/identity/app-provisioning/application-provisioning-config-problem-scim-compatibility).

use crate::common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, Owner};
use kaeso_core::clock::Clock;
use kaeso_core::id::OrgId;
use kaeso_store::pg::oidc;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const DOMAIN: &str = "acme.example";

async fn connect(app: &App, org: &str, domain: &str) {
    let org: OrgId = org.parse().unwrap();
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
            issuer: "https://idp.acme.example".into(),
            client_id: "kaeso".into(),
            secret_ciphertext: sealed.ciphertext,
            secret_nonce: sealed.nonce,
            wrapped_key: sealed.wrapped_key,
            key_id: sealed.key_id,
            domains: vec![domain.into()],
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
        oidc::mark_verified(&mut *conn, org.uuid(), domain, app.clock.now())
            .await
            .unwrap()
    );
    sqlx::query("UPDATE organizations SET sso_default_preset = 'developer' WHERE id = $1")
        .bind(org.uuid())
        .execute(&app.state.pg)
        .await
        .unwrap();
}

async fn token(app: &App, owner: &Owner) -> String {
    let r = app
        .post(
            &format!("/orgs/{}/scim/token", owner.org_id),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let t = r.body["token"].as_str().unwrap().to_owned();
    assert!(t.starts_with("sk-kso-scim-"));
    t
}

struct Scim<'a> {
    app: &'a App,
    org: String,
    token: String,
}

impl Scim<'_> {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> common::Resp {
        let mut req = Request::builder()
            .method(method)
            .uri(format!("/platform/v1/orgs/{}/scim/v2{path}", self.org))
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        if body.is_some() {
            req = req.header(header::CONTENT_TYPE, "application/scim+json");
        }
        let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
        self.app.raw(req.body(body).unwrap()).await
    }

    async fn find(&self, filter: &str) -> Value {
        let r = self
            .call(
                Method::GET,
                &format!("/Users?filter={}&startIndex=1&count=100", urlencode(filter)),
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

async fn setup(s: &TestServices) -> (App, Owner) {
    let app = App::start(s).await;
    let owner = app.setup().await;
    connect(&app, &owner.org_id, DOMAIN).await;
    (app, owner)
}

async fn member_preset(app: &App, owner: &Owner, user: &str) -> Option<String> {
    let r = app
        .get(
            &format!("/orgs/{}/members/{user}", owner.org_id),
            &owner.cookie,
        )
        .await;
    (r.status == StatusCode::OK).then(|| {
        r.body["grant"]["preset"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    })
}

async fn audit_actions(app: &App, owner: &Owner) -> Vec<String> {
    let r = app
        .get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await;
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn okta_creates_renames_deactivates_reactivates_and_deletes_a_user() {
    let s = TestServices::start().await;
    let (app, owner) = setup(&s).await;
    let scim = Scim {
        app: &app,
        token: token(&app, &owner).await,
        org: owner.org_id.clone(),
    };

    let r = scim.call(Method::GET, "/ServiceProviderConfig", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["patch"]["supported"], true);
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/scim+json");
    let r = scim.call(Method::GET, "/ResourceTypes", None).await;
    assert_eq!(r.body["totalResults"], 2);
    let r = scim.call(Method::GET, "/Schemas", None).await;
    assert_eq!(
        r.body["Resources"][0]["id"],
        "urn:ietf:params:scim:schemas:core:2.0:User"
    );

    // Okta looks the user up before it creates them.
    let none = scim.find("userName eq \"ada@acme.example\"").await;
    assert_eq!(none["totalResults"], 0);
    assert_eq!(none["Resources"], json!([]));
    let r = scim
        .call(
            Method::POST,
            "/Users",
            Some(json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
                "userName": "ada@acme.example",
                "name": { "givenName": "Ada", "familyName": "Lovelace" },
                "emails": [{ "primary": true, "value": "ada@acme.example", "type": "work" }],
                "displayName": "Ada Lovelace",
                "locale": "en-US",
                "externalId": "00u1okta",
                "groups": [],
                "password": "ignored-1234",
                "active": true
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let id = r.body["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("usr_"));
    assert_eq!(r.body["active"], true);
    assert_eq!(r.body["externalId"], "00u1okta");
    assert_eq!(r.body["userName"], "ada@acme.example");
    assert_eq!(
        member_preset(&app, &owner, &id).await.as_deref(),
        Some("developer")
    );
    let pending: (Option<String>,) = sqlx::query_as(
        "SELECT subject FROM sso_identities WHERE user_id = (SELECT id FROM users WHERE email = 'ada@acme.example')",
    )
    .fetch_one(&app.state.pg)
    .await
    .unwrap();
    assert_eq!(pending.0, None, "the first sign-in binds the identity");

    let again = scim
        .call(
            Method::POST,
            "/Users",
            Some(json!({ "userName": "ada@acme.example", "active": true })),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.body["scimType"], "uniqueness");
    assert_eq!(again.body["status"], "409");

    let found = scim.find("externalId eq \"00u1okta\"").await;
    assert_eq!(found["Resources"][0]["id"], id);
    let listed = scim.call(Method::GET, "/Users", None).await;
    assert_eq!(
        listed.body["totalResults"], 1,
        "the owner is no account SCIM manages"
    );

    // Okta renames with a PUT of the whole user.
    let r = scim
        .call(
            Method::PUT,
            &format!("/Users/{id}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
                "id": id,
                "userName": "ada@acme.example",
                "name": { "givenName": "Ada", "familyName": "King" },
                "emails": [{ "primary": true, "value": "ada@acme.example", "type": "work" }],
                "active": true
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["displayName"], "Ada King");

    // Okta deactivates with a PATCH that replaces `active`.
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Users/{id}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [{ "op": "replace", "value": { "active": false } }]
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["active"], false);
    assert_eq!(
        member_preset(&app, &owner, &id).await,
        None,
        "no longer a member"
    );
    let r = scim.call(Method::GET, &format!("/Users/{id}"), None).await;
    assert_eq!(
        r.body["active"], false,
        "a deprovisioned user is still listed"
    );

    let r = scim
        .call(
            Method::PATCH,
            &format!("/Users/{id}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [{ "op": "replace", "value": { "active": true } }]
            })),
        )
        .await;
    assert_eq!(r.body["active"], true);
    assert_eq!(
        member_preset(&app, &owner, &id).await.as_deref(),
        Some("developer")
    );

    let r = scim
        .call(Method::DELETE, &format!("/Users/{id}"), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(member_preset(&app, &owner, &id).await, None);
    assert_eq!(
        scim.call(Method::GET, &format!("/Users/{id}"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let actions = audit_actions(&app, &owner).await;
    assert_eq!(
        actions.iter().filter(|a| *a == "member.provision").count(),
        2,
        "{actions:?}"
    );
    assert_eq!(
        actions
            .iter()
            .filter(|a| *a == "member.deprovision")
            .count(),
        2,
        "{actions:?}"
    );

    let status = app
        .get(&format!("/orgs/{}/scim", owner.org_id), &owner.cookie)
        .await;
    assert_eq!(status.body["enabled"], true);
    assert_eq!(status.body["provisioned"], 0);
    assert!(status.body["last_used_at"].is_string());
    assert!(
        status.body["base_url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/platform/v1/orgs/{}/scim/v2", owner.org_id))
    );
    s.cleanup().await;
}

#[tokio::test]
async fn entra_id_disables_with_the_string_false_and_its_keys_stop() {
    let s = TestServices::start().await;
    let (app, owner) = setup(&s).await;
    let scim = Scim {
        app: &app,
        token: token(&app, &owner).await,
        org: owner.org_id.clone(),
    };
    // Someone who joined before provisioning: Entra ID finds them by userName and takes them on.
    let (cookie, user) = common::join(
        &app,
        &owner,
        json!({ "email": "grace@acme.example", "role": "admin" }),
        "Grace",
    )
    .await;
    let project = common::first_project(&app, &owner).await;
    let key = app
        .post(
            &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
            json!({ "name": "laptop" }),
            Some(&cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let key_id = key.body["id"].as_str().unwrap().to_owned();

    let found = scim.find("userName eq \"Grace@acme.example\"").await;
    assert_eq!(found["totalResults"], 1);
    assert_eq!(found["Resources"][0]["id"], user);
    assert_eq!(found["Resources"][0]["active"], true);

    // Entra ID's PATCH without the SCIM-compliance flag: `Replace`, paths, and strings.
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Users/{user}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [
                    { "op": "Replace", "path": "externalId", "value": "e1a2b3" },
                    { "op": "Replace", "path": "name.familyName", "value": "Hopper" },
                    { "op": "Replace", "path": "urn:ietf:params:scim:schemas:extension:enterprise:2.0:User:employeeNumber", "value": "42" }
                ]
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["externalId"], "e1a2b3");
    assert_eq!(r.body["displayName"], "Hopper");
    // With the flag, several attributes in one value without a path.
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Users/{user}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [{ "op": "replace", "value": { "displayName": "Grace Hopper", "name.givenName": "Grace" } }]
            })),
        )
        .await;
    assert_eq!(r.body["displayName"], "Grace Hopper");

    let r = scim
        .call(
            Method::PATCH,
            &format!("/Users/{user}"),
            Some(json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [{ "op": "Replace", "path": "active", "value": "False" }]
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["active"], false);
    let archived: (Option<time::OffsetDateTime>,) =
        sqlx::query_as("SELECT archived_at FROM api_keys WHERE id = $1")
            .bind(key_id.parse::<kaeso_core::id::ApiKeyId>().unwrap().uuid())
            .fetch_one(&app.state.pg)
            .await
            .unwrap();
    assert!(archived.0.is_some(), "the member's key is archived");
    assert_eq!(member_preset(&app, &owner, &user).await, None);
    let r = app.get("/auth/session", &cookie).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "in no organization, signed out"
    );

    let bad = scim
        .call(
            Method::PATCH,
            &format!("/Users/{user}"),
            Some(
                json!({ "Operations": [{ "op": "Replace", "path": "active", "value": "maybe" }] }),
            ),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.body["scimType"], "invalidValue");
    let status = app
        .get(&format!("/orgs/{}/scim", owner.org_id), &owner.cookie)
        .await;
    assert_eq!(status.body["last_error"], "`active` must be true or false.");
    s.cleanup().await;
}

#[tokio::test]
async fn guests_the_last_owner_and_unknown_filters_are_refused() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    // The owner's own domain is verified, so SCIM manages the owner's account.
    connect(&app, &owner.org_id, "example.com").await;
    let scim = Scim {
        app: &app,
        token: token(&app, &owner).await,
        org: owner.org_id.clone(),
    };
    let r = scim
        .call(
            Method::POST,
            "/Users",
            Some(json!({ "userName": "guest@elsewhere.example" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["scimType"], "invalidValue");

    let me = scim.find("userName eq \"owner@example.com\"").await;
    let id = me["Resources"][0]["id"].as_str().unwrap().to_owned();
    let r = scim
        .call(Method::DELETE, &format!("/Users/{id}"), None)
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert!(r.body["detail"].as_str().unwrap().contains("last owner"));
    assert!(member_preset(&app, &owner, &id).await.is_some());

    let r = scim
        .call(
            Method::GET,
            "/Users?filter=name.familyName%20co%20%22x%22",
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["scimType"], "invalidFilter");
    s.cleanup().await;
}

#[tokio::test]
async fn tokens_belong_to_one_org_and_a_replaced_token_works_for_its_grace_period() {
    let s = TestServices::start().await;
    let (app, owner) = setup(&s).await;
    let r = app
        .post(
            &format!("/orgs/{}/scim/token", owner.org_id),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    let first = r.body["token"].as_str().unwrap().to_owned();
    let second = token(&app, &owner).await;
    let old = Scim {
        app: &app,
        token: first.clone(),
        org: owner.org_id.clone(),
    };
    assert_eq!(
        old.call(Method::GET, "/Users", None).await.status,
        StatusCode::OK
    );
    app.clock.advance(time::Duration::hours(25));
    assert_eq!(
        old.call(Method::GET, "/Users", None).await.status,
        StatusCode::UNAUTHORIZED
    );
    let new = Scim {
        app: &app,
        token: second.clone(),
        org: owner.org_id.clone(),
    };
    assert_eq!(
        new.call(Method::GET, "/Users", None).await.status,
        StatusCode::OK
    );

    let other_org = OrgId::new();
    kaeso_store::pg::orgs::insert(&app.state.pg, other_org.uuid(), "Other", 30, 30)
        .await
        .unwrap();
    let other_org = other_org.to_string();
    let wrong = Scim {
        app: &app,
        token: second.clone(),
        org: other_org,
    };
    let r = wrong.call(Method::GET, "/Users", None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{:?}", r.body);
    assert_eq!(
        r.body["schemas"][0],
        "urn:ietf:params:scim:api:messages:2.0:Error"
    );

    s.cleanup().await;
}

#[tokio::test]
async fn deleting_the_token_ends_provisioning() {
    let s = TestServices::start().await;
    let (app, owner) = setup(&s).await;
    let scim = Scim {
        app: &app,
        token: token(&app, &owner).await,
        org: owner.org_id.clone(),
    };
    assert_eq!(
        scim.call(Method::GET, "/Users", None).await.status,
        StatusCode::OK
    );
    let r = app
        .call(
            Method::DELETE,
            &format!("/orgs/{}/scim/token", owner.org_id),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        scim.call(Method::GET, "/Users", None).await.status,
        StatusCode::UNAUTHORIZED
    );
    let status = app
        .get(&format!("/orgs/{}/scim", owner.org_id), &owner.cookie)
        .await;
    assert_eq!(status.body["enabled"], false);
    s.cleanup().await;
}

#[tokio::test]
async fn a_token_needs_single_sign_on() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let r = app
        .post(
            &format!("/orgs/{}/scim/token", owner.org_id),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "Connect single sign-on first; SCIM manages the accounts it signs in."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn groups_are_kept_as_the_identity_provider_pushes_them() {
    let s = TestServices::start().await;
    let (app, owner) = setup(&s).await;
    let scim = Scim {
        app: &app,
        token: token(&app, &owner).await,
        org: owner.org_id.clone(),
    };
    let ada = scim
        .call(
            Method::POST,
            "/Users",
            Some(json!({ "userName": "ada@acme.example", "externalId": "a" })),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bob = scim
        .call(
            Method::POST,
            "/Users",
            Some(json!({ "userName": "bob@acme.example", "externalId": "b" })),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = scim
        .call(
            Method::POST,
            "/Groups",
            Some(json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:Group"],
                "displayName": "Engineering",
                "externalId": "grp1",
                "members": [{ "value": ada }]
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let group = r.body["id"].as_str().unwrap().to_owned();
    let dup = scim
        .call(
            Method::POST,
            "/Groups",
            Some(json!({ "displayName": "Engineering" })),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);

    // Okta adds members; Entra ID removes one by its value.
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Groups/{group}"),
            Some(json!({ "Operations": [{ "op": "add", "path": "members", "value": [{ "value": bob, "display": "bob@acme.example" }] }] })),
        )
        .await;
    assert_eq!(r.body["members"].as_array().unwrap().len(), 2);
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Groups/{group}"),
            Some(json!({ "Operations": [{ "op": "Remove", "path": "members", "value": [{ "value": ada }] }] })),
        )
        .await;
    assert_eq!(r.body["members"], json!([{ "value": bob }]));
    let r = scim
        .call(
            Method::PATCH,
            &format!("/Groups/{group}"),
            Some(json!({ "Operations": [
                { "op": "remove", "path": format!("members[value eq \"{bob}\"]") },
                { "op": "Replace", "path": "displayName", "value": "Platform" }
            ] })),
        )
        .await;
    assert_eq!(r.body["members"], json!([]));
    assert_eq!(r.body["displayName"], "Platform");
    let r = scim
        .call(
            Method::GET,
            "/Groups?filter=displayName%20eq%20%22platform%22",
            None,
        )
        .await;
    assert_eq!(r.body["totalResults"], 1);
    let r = scim
        .call(Method::DELETE, &format!("/Groups/{group}"), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        scim.call(Method::GET, &format!("/Groups/{group}"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    s.cleanup().await;
}
