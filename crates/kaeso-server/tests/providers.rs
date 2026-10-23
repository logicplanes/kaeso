#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::{Method, StatusCode};
use common::App;
use kaeso_core::provider::PROVIDERS;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

#[tokio::test]
async fn every_member_reads_each_provider_with_its_name_key_page_and_formats() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let (member, _) = common::join(
        &app,
        &owner,
        json!({ "email": "lin@example.com", "role": "member" }),
        "Lin",
    )
    .await;

    let r = app
        .get(&format!("/orgs/{org}/providers"), &member)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(r.body["has_more"], false);
    let console: Value = serde_json::from_str(include_str!(
        "../../../apps/console/src/test/fixtures/providers.json"
    ))
    .unwrap();
    assert_eq!(
        r.body, console,
        "the console's tests read the list the gateway serves"
    );
    let listed = r.body["data"].as_array().unwrap();
    assert_eq!(
        listed.iter().map(|p| p["id"].clone()).collect::<Vec<_>>(),
        PROVIDERS.iter().map(|p| json!(p.id)).collect::<Vec<_>>()
    );
    let groq = listed.iter().find(|p| p["id"] == "groq").unwrap();
    assert_eq!(
        groq,
        &json!({
            "id": "groq",
            "name": "Groq",
            "key_page": { "url": "https://console.groq.com/keys", "label": "API Keys" },
            "key_prefix": "gsk_",
            "dialects": ["openai_chat"],
            "count_tokens": null,
            "embeddings": null,
            "account": null,
        })
    );
    let mistral = listed.iter().find(|p| p["id"] == "mistral").unwrap();
    assert_eq!(mistral["key_prefix"], Value::Null);
    assert_eq!(mistral["embeddings"], "openai_embeddings");
    let openai = listed.iter().find(|p| p["id"] == "openai").unwrap();
    assert_eq!(
        openai["dialects"],
        json!(["openai_responses", "openai_chat", "openai_embeddings"])
    );
    assert_eq!(openai["count_tokens"], "openai_responses");

    let anonymous = app
        .call(Method::GET, &format!("/orgs/{org}/providers"), None, None)
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}
