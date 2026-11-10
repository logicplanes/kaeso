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
            "custom": false,
            "takes_key": true,
            "hides_built_in": null,
            "key_page": { "url": "https://console.groq.com/keys", "label": "API Keys" },
            "key_prefix": "gsk_",
            "dialects": ["openai_chat"],
            "count_tokens": null,
            "embeddings": null,
            "account": null,
            "service_account": null,
            "model_routes": [],
            "server": null,
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
    let vercel = listed.iter().find(|p| p["id"] == "vercel").unwrap();
    assert_eq!(
        vercel["dialects"],
        json!([
            "openai_chat",
            "openai_responses",
            "anthropic_messages",
            "openai_embeddings"
        ])
    );
    assert_eq!(vercel["count_tokens"], Value::Null);
    assert_eq!(vercel["key_prefix"], "vck_");
    let cohere = listed.iter().find(|p| p["id"] == "cohere").unwrap();
    assert_eq!(cohere["name"], "Cohere");
    assert_eq!(
        cohere["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(cohere["count_tokens"], Value::Null);
    assert_eq!(cohere["key_prefix"], Value::Null);
    let alibaba = listed.iter().find(|p| p["id"] == "alibaba").unwrap();
    assert_eq!(alibaba["name"], "QwenCloud");
    assert_eq!(
        alibaba["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(alibaba["embeddings"], "openai_embeddings");
    assert_eq!(alibaba["key_prefix"], "sk-");
    let cerebras = listed.iter().find(|p| p["id"] == "cerebras").unwrap();
    assert_eq!(cerebras["name"], "Cerebras");
    assert_eq!(cerebras["dialects"], json!(["openai_chat"]));
    assert_eq!(cerebras["count_tokens"], Value::Null);
    assert_eq!(cerebras["embeddings"], Value::Null);
    assert_eq!(cerebras["key_prefix"], "csk-");
    let zai = listed.iter().find(|p| p["id"] == "zai").unwrap();
    assert_eq!(zai["name"], "Z.ai");
    assert_eq!(zai["dialects"], json!(["anthropic_messages"]));
    assert_eq!(zai["count_tokens"], Value::Null);
    assert_eq!(zai["embeddings"], Value::Null);
    assert_eq!(zai["key_prefix"], Value::Null);
    assert_eq!(
        zai["key_page"],
        json!({ "url": "https://z.ai/manage-apikey/apikey-list", "label": "API Keys" })
    );
    let minimax = listed.iter().find(|p| p["id"] == "minimax").unwrap();
    assert_eq!(minimax["name"], "MiniMax");
    assert_eq!(minimax["dialects"], json!(["anthropic_messages"]));
    assert_eq!(minimax["count_tokens"], Value::Null);
    assert_eq!(minimax["embeddings"], Value::Null);
    assert_eq!(minimax["key_prefix"], "sk-api-");
    let google = listed.iter().find(|p| p["id"] == "google").unwrap();
    assert_eq!(google["name"], "Google AI Studio");
    assert_eq!(google["dialects"], json!(["gemini", "gemini_embeddings"]));
    assert_eq!(google["count_tokens"], "gemini");
    assert_eq!(google["embeddings"], "gemini_embeddings");
    assert_eq!(google["key_prefix"], Value::Null);
    assert_eq!(
        google["key_page"],
        json!({ "url": "https://aistudio.google.com/apikey", "label": "API keys" })
    );
    let fireworks = listed.iter().find(|p| p["id"] == "fireworks-ai").unwrap();
    assert_eq!(fireworks["name"], "Fireworks AI");
    assert_eq!(
        fireworks["dialects"],
        json!(["openai_chat", "anthropic_messages", "openai_embeddings"])
    );
    assert_eq!(fireworks["count_tokens"], Value::Null);
    assert_eq!(fireworks["embeddings"], "openai_embeddings");
    assert_eq!(fireworks["key_prefix"], "fw_");
    let nebius = listed.iter().find(|p| p["id"] == "nebius").unwrap();
    assert_eq!(nebius["name"], "Nebius Token Factory");
    assert_eq!(
        nebius["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(nebius["embeddings"], "openai_embeddings");
    assert_eq!(nebius["key_prefix"], Value::Null);
    let sambanova = listed.iter().find(|p| p["id"] == "sambanova").unwrap();
    assert_eq!(sambanova["name"], "SambaNova");
    assert_eq!(
        sambanova["dialects"],
        json!(["openai_chat", "anthropic_messages"])
    );
    assert_eq!(sambanova["count_tokens"], "anthropic_messages");
    assert_eq!(sambanova["embeddings"], Value::Null);
    let scaleway = listed.iter().find(|p| p["id"] == "scaleway").unwrap();
    assert_eq!(scaleway["name"], "Scaleway");
    assert_eq!(
        scaleway["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(
        scaleway["key_page"],
        json!({ "url": "https://console.scaleway.com/iam/api-keys", "label": "IAM → API keys" })
    );
    let nvidia = listed.iter().find(|p| p["id"] == "nvidia").unwrap();
    assert_eq!(nvidia["name"], "NVIDIA");
    assert_eq!(
        nvidia["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(nvidia["count_tokens"], Value::Null);
    assert_eq!(nvidia["embeddings"], "openai_embeddings");
    assert_eq!(nvidia["key_prefix"], "nvapi-");
    assert_eq!(
        nvidia["key_page"],
        json!({ "url": "https://build.nvidia.com/settings/api-keys", "label": "API Keys" })
    );

    let anonymous = app
        .call(Method::GET, &format!("/orgs/{org}/providers"), None, None)
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}
