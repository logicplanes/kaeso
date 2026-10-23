#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::v1::{Answer, call_app, chat, messages};
use common::{App, Owner};
use kaeso_testkit::grok::{Fault, MockGrok, Reply};
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const STORY: &str = "Once upon a time, in a shimmering meadow under the silver moon, a gentle unicorn named Luna pranced through fields of glowing flowers. She collected starlight in her horn to light the way for sleepy fireflies returning home. As the night grew quiet, Luna curled up on a cloud of soft moss and drifted into peaceful dreams.";

struct Plan {
    app: App,
    grok: MockGrok,
    api: MockProvider,
    owner: Owner,
    credential: String,
    key: String,
    services: TestServices,
}

impl Plan {
    async fn call(&self, path: &str, anthropic: bool, body: Value) -> Answer {
        call_app(&self.app, path, Some(&self.key), anthropic, Some(body)).await
    }

    async fn access_token(&self) -> String {
        common::account_tokens(&self.app, &self.owner, &self.credential)
            .await
            .access_token
    }

    fn sent(&self) -> Vec<kaeso_testkit::mock_provider::Received> {
        self.grok
            .received()
            .into_iter()
            .filter(|r| r.method == "POST" && r.path.starts_with("/v1/"))
            .collect()
    }
}

async fn plan() -> Plan {
    let s = TestServices::start().await;
    let grok = MockGrok::start().await;
    let api = MockProvider::start().await;
    let app = App::start_full(
        &s,
        &[("KAESO_PROVIDERS_XAI_BASE_URL", api.base_url.as_str())],
        None,
        None,
        Some(common::accounts_at(grok.base_url())),
    )
    .await;
    let owner = app.setup().await;
    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created = common::sign_in_grok(&app, &owner, &grok, "Ada's Grok", "ada@example.com").await;
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    app.post(
        &format!("{base}/models"),
        json!({ "provider": "xai", "model_id": "grok-4.3" }),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "app" }),
            Some(&owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    Plan {
        app,
        grok,
        api,
        owner,
        credential: created["id"].as_str().unwrap().to_owned(),
        key,
        services: s,
    }
}

#[tokio::test]
async fn requests_go_to_xai_with_the_accounts_bearer_token_as_the_app_asked() {
    let p = plan().await;
    let r = p
        .call("/v1/chat/completions", false, chat("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["choices"][0]["message"]["content"], STORY);
    assert_eq!(r.header("kaeso-provider"), Some("xai"));
    assert!(
        p.api.received().is_empty(),
        "the configured base URL is never used"
    );
    let sent = p.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "/v1/responses");
    assert_eq!(
        sent[0].header("authorization"),
        Some(format!("Bearer {}", p.access_token().await).as_str())
    );
    let body = sent[0].json();
    assert_eq!(body["model"], "grok-4.3");
    assert_ne!(body["stream"], true, "a whole reply is asked for as one");
    for header in ["x-grok-conv-id", "x-xai-token-source", "x-grok-client"] {
        assert!(sent[0].header(header).is_none(), "{header}");
    }

    let r = p
        .call("/v1/messages", true, messages("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let answer = r.json();
    let text = answer["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "text")
        .unwrap();
    assert_eq!(text["text"], STORY);
    assert_eq!(p.sent().len(), 2);
    p.services.cleanup().await;
}

#[tokio::test]
async fn xai_errors_reach_the_app_in_its_format_without_a_retry() {
    let p = plan().await;
    p.grok.reply_with(Reply::RateLimited);
    let r = p
        .call("/v1/chat/completions", false, chat("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Your team has reached its monthly spending limit."
    );
    let r = p
        .call("/v1/messages", true, messages("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");
    assert_eq!(p.sent().len(), 2, "no request is retried");

    p.grok.reply_with(Reply::Text);
    p.grok.end_sessions();
    let r = p
        .call("/v1/chat/completions", false, chat("xai/grok-4.3", "Hi"))
        .await;
    assert!(r.status.is_client_error() || r.status.is_server_error());
    assert!(!r.text().contains(&p.access_token().await));
    p.services.cleanup().await;
}

#[tokio::test]
async fn a_plan_without_api_access_stops_requests_until_someone_signs_in_again() {
    let p = plan().await;
    p.app.clock.advance(time::Duration::minutes(29));
    p.grok.set_fault(Fault::NoApiAccess);
    let r = p
        .call("/v1/chat/completions", false, chat("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The Grok plan of this project's SpaceX key has no API access. Ask an organization admin to add an API key under Providers."
    );
    p.grok.set_fault(Fault::SignedOut);
    let r = p
        .call("/v1/messages", true, messages("xai/grok-4.3", "Hi"))
        .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(p.grok.refreshes(), 1);
    assert!(p.sent().is_empty());
    p.services.cleanup().await;
}
