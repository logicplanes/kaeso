#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, messages};
use kaeso_testkit::claude::{self, Fault, MockClaude};
use serde_json::{Value, json};

const MODEL: &str = "claude-haiku-4-5";

struct Linked {
    env: Env,
    claude: MockClaude,
}

impl Linked {
    fn credentials(&self) -> String {
        format!("/orgs/{}/credentials", self.env.owner.org_id)
    }

    async fn get(&self, path: &str) -> common::Resp {
        self.env.app.get(path, &self.env.owner.cookie).await
    }

    async fn post(&self, path: &str, body: Value) -> common::Resp {
        self.env
            .app
            .post(path, body, Some(&self.env.owner.cookie))
            .await
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> common::Resp {
        self.env
            .app
            .call(method, path, body, Some(&self.env.owner.cookie))
            .await
    }

    async fn set_app(&self) -> common::Resp {
        self.call(
            Method::PUT,
            "/instance/linked-billing/anthropic",
            Some(json!({
                "client_id": claude::CLIENT_ID,
                "audience": claude::AUDIENCE,
                "organization_id": claude::ORGANIZATION,
                "federation_rule_id": claude::RULE,
            })),
        )
        .await
    }

    async fn begin(&self, name: &str) -> Value {
        let r = self
            .post(
                &format!("{}/oauth", self.credentials()),
                json!({ "provider": "anthropic", "name": name, "linked_billing": true, "make_default": true }),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body
    }

    async fn finish(&self, attempt: &Value, back: &str) -> common::Resp {
        self.post(
            &format!(
                "{}/oauth/{}",
                self.credentials(),
                attempt["attempt"].as_str().unwrap()
            ),
            json!({ "callback_url": back }),
        )
        .await
    }

    // Links a Claude organization as the project's Anthropic key and returns the key's view.
    async fn link(&self) -> Value {
        let attempt = self.begin("Acme Claude").await;
        let back = self
            .claude
            .approve(attempt["authorize_url"].as_str().unwrap());
        let r = self.finish(&attempt, &back).await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        r.body
    }

    async fn key(&self, id: &str) -> Value {
        self.get(&format!("{}/{id}", self.credentials())).await.body
    }

    async fn api_key(&self) -> String {
        let list = self.get(&self.credentials()).await.body;
        list["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["provider"] == "anthropic" && k["auth_method"] == "api_key")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn ask(&self, text: &str) -> common::v1::Answer {
        let a = call(
            &self.env,
            "/v1/messages",
            Some(&self.env.key),
            true,
            Some(messages(MODEL, text)),
        )
        .await;
        self.env.app.state.in_flight.settled().await;
        a
    }
}

async fn linked_env() -> Linked {
    linked_env_with(Some(Arc::new(Capture::default()))).await
}

// A captured run records no usage: the capture takes the place of the sink that counts it.
async fn linked_env_with(capture: Option<Arc<Capture>>) -> Linked {
    let claude = MockClaude::start().await;
    let env = common::v1::env_linked(capture, &claude.base_url).await;
    let view = env
        .app
        .get("/instance/linked-billing", &env.owner.cookie)
        .await
        .body;
    let anthropic = &view[0];
    claude.register(
        anthropic["redirect_uri"].as_str().unwrap(),
        anthropic["issuer"].as_str().unwrap(),
        anthropic["jwks"].clone(),
    );
    let linked = Linked { env, claude };
    let r = linked.set_app().await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    linked
}

#[tokio::test]
async fn the_instance_shows_what_to_register_and_publishes_its_keys() {
    let claude = MockClaude::start().await;
    let env = common::v1::env_linked(None, &claude.base_url).await;
    let r = env
        .app
        .get("/instance/linked-billing", &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let a = &r.body[0];
    assert_eq!(a["provider"], "anthropic");
    assert_eq!(a["organization"], "Claude organization");
    assert_eq!(a["app"], Value::Null);
    let redirect = a["redirect_uri"].as_str().unwrap();
    assert!(redirect.ends_with("/linked-billing/callback"), "{redirect}");
    let issuer = a["issuer"].as_str().unwrap();
    assert!(
        issuer.ends_with("/platform/v1/linked-billing/oidc"),
        "{issuer}"
    );
    assert_eq!(a["subject"], "kaeso:linked-billing:anthropic");
    assert_eq!(a["jwks"]["keys"][0]["alg"], "ES256");

    let discovery = env
        .app
        .call(
            Method::GET,
            "/linked-billing/oidc/.well-known/openid-configuration",
            None,
            None,
        )
        .await;
    assert_eq!(discovery.status, StatusCode::OK, "{:?}", discovery.body);
    assert_eq!(discovery.body["issuer"], issuer);
    assert_eq!(discovery.body["jwks_uri"], format!("{issuer}/jwks.json"));
    let jwks = env
        .app
        .call(Method::GET, "/linked-billing/oidc/jwks.json", None, None)
        .await;
    assert_eq!(jwks.status, StatusCode::OK);
    assert_eq!(jwks.body, a["jwks"], "one key, kept across reads");
}

#[tokio::test]
async fn check_tests_the_app_before_anyone_links() {
    let l = linked_env().await;
    let r = l
        .post("/instance/linked-billing/anthropic/check", json!({}))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let app = &r.body["app"];
    assert_eq!(app["check_problem"], Value::Null, "{app}");
    assert!(app["checked_at"].is_string());
    assert_eq!(
        l.claude.profiles().len(),
        1,
        "one profile of its own for checks"
    );
    let par = l
        .claude
        .asked()
        .into_iter()
        .find(|(p, _)| p == "/v1/oauth/par")
        .unwrap()
        .1;
    assert_eq!(par["scope"], "billing:attribution offline_access");
    assert_eq!(par["connection_type"], "organization_shared");
    assert_eq!(par["connection_auth_target"], "platform");
    assert_eq!(par["code_challenge_method"], "S256");

    for (fault, problem) in [
        (Fault::InvalidClient, "client"),
        (Fault::FederationRefused, "federation"),
    ] {
        l.claude.fail(fault);
        let r = l
            .post("/instance/linked-billing/anthropic/check", json!({}))
            .await;
        assert_eq!(r.body["app"]["check_problem"], problem, "{fault:?}");
    }
    assert_eq!(l.claude.profiles().len(), 1, "the check's profile is kept");
}

#[tokio::test]
async fn an_organization_links_its_claude_organization_once() {
    let l = linked_env().await;
    let attempt = l.begin("Acme Claude").await;
    let url = attempt["authorize_url"].as_str().unwrap();
    assert!(
        url.starts_with(&format!("{}/oauth/authorize?", l.claude.base_url)),
        "{url}"
    );
    let back = l.claude.approve(url);
    let r = l.finish(&attempt, &back).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["auth_method"], "linked_billing");
    assert_eq!(r.body["name"], "Acme Claude");
    assert!(r.body["account"].as_str().unwrap().starts_with("ocon_"));
    assert_eq!(r.body["auth_error"], Value::Null);
    assert_eq!(r.body["is_default"], true);

    let again = l.finish(&attempt, &back).await;
    assert_eq!(
        again.status,
        StatusCode::BAD_REQUEST,
        "an attempt is used once"
    );

    let second = l
        .post(
            &format!("{}/oauth", l.credentials()),
            json!({ "provider": "anthropic", "name": "Second", "linked_billing": true }),
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT, "{:?}", second.body);
    assert_eq!(
        second.body["error"]["message"],
        "This organization already has a linked Anthropic key; link it again or unlink it."
    );
    let profiles = l.claude.profiles();
    assert_eq!(profiles.len(), 1, "{profiles:?}");
    let providers = l
        .get(&format!("/orgs/{}/providers", l.env.owner.org_id))
        .await
        .body;
    let anthropic = providers["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "anthropic")
        .unwrap()
        .clone();
    assert_eq!(anthropic["linked_billing"]["ready"], true);
    assert_eq!(
        anthropic["linked_billing"]["roles"],
        json!(["Admin", "Billing"])
    );
    let openai = providers["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "openai")
        .unwrap()
        .clone();
    assert_eq!(openai["linked_billing"], Value::Null);
}

#[tokio::test]
async fn a_link_that_doesnt_complete_says_why() {
    let l = linked_env().await;
    let attempt = l.begin("Claude").await;
    let back = l
        .claude
        .refuse(attempt["authorize_url"].as_str().unwrap(), "access_denied");
    let r = l.finish(&attempt, &back).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "Linking didn't complete; sign in with an account that has the Admin or Billing role."
    );

    let attempt = l.begin("Claude").await;
    let back = l.claude.refuse(
        attempt["authorize_url"].as_str().unwrap(),
        "user_profile_already_linked",
    );
    let r = l.finish(&attempt, &back).await;
    assert_eq!(
        r.body["error"]["message"],
        "That Claude organization is already linked to another organization on this install."
    );

    l.claude.fail(Fault::AlreadyLinked);
    let attempt = l.begin("Claude").await;
    let back = l.claude.approve(attempt["authorize_url"].as_str().unwrap());
    let r = l.finish(&attempt, &back).await;
    assert_eq!(
        r.body["error"]["message"],
        "That Claude organization is already linked to another organization on this install."
    );

    l.claude.fail(Fault::NoRefreshToken);
    let attempt = l.begin("Claude").await;
    let back = l.claude.approve(attempt["authorize_url"].as_str().unwrap());
    let r = l.finish(&attempt, &back).await;
    assert_eq!(
        r.body["error"]["message"],
        "Anthropic issued no refresh token; ask an instance admin to turn on Offline access."
    );
    assert_eq!(
        l.claude.revoked().len(),
        1,
        "a connection Kaeso can't keep is ended at once"
    );

    l.claude.fail(Fault::None);
    let r = l
        .finish(
            &attempt,
            "https://kaeso.example.com/linked-billing/elsewhere?code=x&state=y",
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let list = l.get(&l.credentials()).await.body;
    assert!(
        !list["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["auth_method"] == "linked_billing"),
        "no key was added"
    );
}

#[tokio::test]
async fn linking_needs_the_install_app_first() {
    let claude = MockClaude::start().await;
    let env = common::v1::env_linked(None, &claude.base_url).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials/oauth", env.owner.org_id),
            json!({ "provider": "anthropic", "name": "Claude", "linked_billing": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "Linked billing for Anthropic isn't set up on this install; ask an instance admin."
    );
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials/oauth", env.owner.org_id),
            json!({ "provider": "openai", "name": "OpenAI", "linked_billing": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(
        r.body["error"]["message"],
        "OpenAI offers no linked billing; add an API key for it."
    );
}

#[tokio::test]
async fn requests_send_the_connection_token_and_no_api_key() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let sent = l.env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert!(
        sent.header("authorization")
            .is_some_and(|v| v.starts_with("Bearer lbtok-")),
        "{:?}",
        sent.header("authorization")
    );
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.header("anthropic-user-profile-id"), None);
    assert!(sent.json().get("billing_policy").is_none());
    let outcomes = l.env.outcomes().await;
    assert_eq!(
        outcomes.last().unwrap().credential_id,
        Some(id.parse::<kaeso_core::id::CredentialId>().unwrap().uuid()),
        "the request log names the key that paid"
    );

    let r = call(
        &l.env,
        "/v1/embeddings",
        Some(&l.env.key),
        false,
        Some(json!({ "model": MODEL, "input": "x" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
}

#[tokio::test]
async fn an_app_that_must_name_the_profile_sends_billing_policy() {
    let l = linked_env().await;
    l.link().await;
    let r = l
        .call(
            Method::PUT,
            "/instance/linked-billing/anthropic",
            Some(json!({
                "client_id": claude::CLIENT_ID,
                "audience": claude::AUDIENCE,
                "organization_id": claude::ORGANIZATION,
                "federation_rule_id": claude::RULE,
                "send_billing_policy": true,
            })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let sent = l.env.mock.last().unwrap();
    assert!(
        sent.header("anthropic-user-profile-id")
            .is_some_and(|p| p.starts_with("uprof_"))
    );
    assert!(
        sent.header("anthropic-beta")
            .is_some_and(|b| b.contains("user-profiles-billing-2026-10-01"))
    );
    assert_eq!(
        sent.json()["billing_policy"],
        json!({ "type": "billing_policy", "bill_to": "user_profile" })
    );
}

#[tokio::test]
async fn a_spend_limit_refuses_with_where_to_raise_it_and_lifts_after_a_paid_request() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let a = l
        .ask("[mock:key-fail=lbtok linked-spend-limit] hello")
        .await;
    assert_eq!(a.status, StatusCode::PAYMENT_REQUIRED, "{}", a.text());
    let message = a.json()["error"]["message"].as_str().unwrap().to_owned();
    assert_eq!(
        message,
        "The linked Claude organization of this project's Anthropic key reached its spend limit. Its admins can raise it in Claude Console."
    );
    let view = l.key(id).await;
    assert!(view["spend_limited_until"].is_string(), "{view}");
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let view = l.key(id).await;
    assert_eq!(view["spend_limited_until"], Value::Null, "{view}");
}

#[tokio::test]
async fn a_project_falls_back_to_another_key_when_its_link_cant_pay() {
    let l = linked_env().await;
    let key = l.link().await;
    let linked = key["id"].as_str().unwrap().to_owned();
    let fallback = l.api_key().await;
    let path = format!("/orgs/{}/linked-billing/fallbacks", l.env.owner.org_id);
    let r = l
        .call(
            Method::PUT,
            &path,
            Some(
                json!({ "project": l.env.project, "provider": "anthropic", "credential": linked }),
            ),
        )
        .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "a link can't be its own fallback"
    );
    let r = l
        .call(
            Method::PUT,
            &path,
            Some(json!({ "project": l.env.project, "provider": "anthropic", "credential": fallback })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body[0]["credential"], fallback);

    let a = l
        .ask("[mock:key-fail=lbtok linked-spend-limit] hello")
        .await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let sent = l.env.mock.last().unwrap();
    assert_eq!(
        sent.header("x-api-key"),
        Some(common::v1::ANTHROPIC_SECRET),
        "the fallback key paid"
    );
    let audit = l
        .get(&format!(
            "/orgs/{}/audit-log?action=credential.fallback",
            l.env.owner.org_id
        ))
        .await;
    assert_eq!(audit.status, StatusCode::OK);

    let r = l
        .call(
            Method::PUT,
            &path,
            Some(json!({ "project": l.env.project, "provider": "anthropic", "credential": null })),
        )
        .await;
    assert_eq!(r.body, json!([]));
    let a = l
        .ask("[mock:key-fail=lbtok linked-spend-limit] hello")
        .await;
    assert_eq!(a.status, StatusCode::PAYMENT_REQUIRED, "{}", a.text());
}

#[tokio::test]
async fn an_ended_link_shows_unlinked_and_stops_paying() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let a = l
        .ask("[mock:key-fail=lbtok linked-not-billable] hello")
        .await;
    assert_eq!(a.status, StatusCode::BAD_GATEWAY, "{}", a.text());
    assert_eq!(
        l.key(id).await["auth_error"],
        Value::Null,
        "a refusal right after linking is the link settling"
    );
    // Anthropic takes a fresh link's refusal as settling for 10 seconds; the key's `updated_at`
    // comes from the database's clock, which runs ahead of the test's frozen one.
    l.env.app.clock.advance(time::Duration::minutes(1));
    let a = l
        .ask("[mock:key-fail=lbtok linked-not-billable] hello")
        .await;
    assert_eq!(a.status, StatusCode::BAD_GATEWAY, "{}", a.text());
    let view = l.key(id).await;
    assert_eq!(view["auth_error"], "unlinked", "{view}");
    let before = l.env.mock.received().len();
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::BAD_GATEWAY, "{}", a.text());
    assert_eq!(
        a.json()["error"]["message"],
        "The Claude organization of this project's Anthropic key is no longer linked. Ask an organization admin to link it again under Providers."
    );
    assert_eq!(
        l.env.mock.received().len(),
        before,
        "nothing is sent for a link that ended"
    );
}

#[tokio::test]
async fn tokens_are_renewed_before_they_expire_and_an_ended_connection_unlinks() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    l.env.app.clock.advance(time::Duration::minutes(51));
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert_eq!(l.claude.refreshes(), 1);
    let first = l.env.mock.last().unwrap();
    assert_ne!(first.header("authorization"), Some("Bearer lbtok-2"));

    l.claude.fail(Fault::RefreshLostOnce);
    l.env.app.clock.advance(time::Duration::minutes(55));
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert_eq!(
        l.claude.refreshes(),
        3,
        "a lost answer is asked again once, at once"
    );
    assert_eq!(l.key(id).await["auth_error"], Value::Null);

    l.claude.fail(Fault::RefreshPaused);
    l.env.app.clock.advance(time::Duration::minutes(55));
    let _ = l.ask("hello").await;
    assert_eq!(
        l.key(id).await["auth_error"],
        Value::Null,
        "a paused refresh keeps the refresh token for later"
    );

    l.claude.fail(Fault::Ended);
    l.env.app.clock.advance(time::Duration::minutes(10));
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::BAD_GATEWAY, "{}", a.text());
    assert_eq!(l.key(id).await["auth_error"], "unlinked");
}

#[tokio::test]
async fn unlinking_ends_the_connection_and_removes_the_key() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    l.claude.fail(Fault::RevokeDown);
    let r = l
        .call(Method::DELETE, &format!("{}/{id}", l.credentials()), None)
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    assert_eq!(
        l.key(id).await["id"],
        id,
        "the key stays while Anthropic is down"
    );
    l.claude.fail(Fault::None);
    let r = l
        .call(Method::DELETE, &format!("{}/{id}", l.credentials()), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    let revoked = l.claude.revoked();
    assert_eq!(revoked.last().unwrap()["connection_id"], key["account"]);
}

#[tokio::test]
async fn linking_again_ends_the_old_connection_and_keeps_the_key() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let r = l
        .post(&format!("{}/{id}/oauth", l.credentials()), json!({}))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert!(
        l.claude.revoked().last().unwrap()["user_profile_id"]
            .as_str()
            .is_some_and(|p| p.starts_with("uprof_"))
    );
    let back = l.claude.approve(r.body["authorize_url"].as_str().unwrap());
    let done = l
        .post(
            &format!(
                "{}/{id}/oauth/{}",
                l.credentials(),
                r.body["attempt"].as_str().unwrap()
            ),
            json!({ "callback_url": back }),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.body);
    assert_eq!(done.body["id"], id);
    assert_ne!(done.body["account"], key["account"], "a new connection");
    assert_eq!(l.claude.profiles().len(), 1, "the same profile");
}

#[tokio::test]
async fn removing_the_app_holds_every_link_until_it_is_back() {
    let l = linked_env().await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let r = l
        .call(Method::DELETE, "/instance/linked-billing/anthropic", None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{:?}", r.body);
    assert_eq!(l.key(id).await["auth_error"], "needs_setup");
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::BAD_GATEWAY, "{}", a.text());
    assert_eq!(
        a.json()["error"]["message"],
        "Linked billing for this project's Anthropic key needs the install's OAuth app. Ask an instance admin to set it up again."
    );
    let r = l.set_app().await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(l.key(id).await["auth_error"], Value::Null);
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
}

#[tokio::test]
async fn usage_and_costs_say_which_key_paid() {
    let l = linked_env_with(None).await;
    let key = l.link().await;
    let id = key["id"].as_str().unwrap();
    let a = l.ask("hello").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    kaeso_server::services::usage::flush(&l.env.app.state)
        .await
        .unwrap();
    let usage = l
        .get(&format!(
            "/orgs/{}/usage?group_by=billed_to",
            l.env.owner.org_id
        ))
        .await;
    assert_eq!(usage.status, StatusCode::OK, "{:?}", usage.body);
    let keys: Vec<&str> = usage.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, [id], "{:?}", usage.body);
    let costs = l
        .get(&format!(
            "/orgs/{}/linked-billing/costs",
            l.env.owner.org_id
        ))
        .await;
    assert_eq!(costs.status, StatusCode::OK, "{:?}", costs.body);
    assert_eq!(costs.body["keys"][0]["credential"], id);
    assert_eq!(costs.body["keys"][0]["requests"], 1);
    let refused = l
        .get(&format!(
            "/orgs/{}/usage?group_by=nonsense",
            l.env.owner.org_id
        ))
        .await;
    assert_eq!(
        refused.body["error"]["message"],
        "Use `project`, `billed_to` or what a project's usage groups by, such as `day`."
    );
}
