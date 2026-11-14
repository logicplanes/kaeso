#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

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
        "../../../../apps/console/src/test/fixtures/providers.json"
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
            "account_scope": null,
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
    let hugging_face = listed.iter().find(|p| p["id"] == "huggingface").unwrap();
    assert_eq!(hugging_face["name"], "Hugging Face");
    assert_eq!(hugging_face["dialects"], json!(["openai_chat"]));
    assert_eq!(hugging_face["count_tokens"], Value::Null);
    assert_eq!(hugging_face["embeddings"], Value::Null);
    assert_eq!(hugging_face["key_prefix"], "hf_");
    assert_eq!(
        hugging_face["key_page"],
        json!({ "url": "https://huggingface.co/settings/tokens", "label": "Settings → Access Tokens" })
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
    let baseten = listed.iter().find(|p| p["id"] == "baseten").unwrap();
    assert_eq!(baseten["name"], "Baseten");
    assert_eq!(baseten["dialects"], json!(["openai_chat"]));
    assert_eq!(baseten["count_tokens"], Value::Null);
    assert_eq!(baseten["embeddings"], Value::Null);
    assert_eq!(baseten["key_prefix"], Value::Null);
    assert_eq!(
        baseten["key_page"],
        json!({ "url": "https://app.baseten.co/settings/api_keys", "label": "Organization settings → API keys" })
    );
    let siliconflow = listed.iter().find(|p| p["id"] == "siliconflow").unwrap();
    assert_eq!(siliconflow["name"], "SiliconFlow");
    assert_eq!(
        siliconflow["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(siliconflow["count_tokens"], Value::Null);
    assert_eq!(siliconflow["embeddings"], "openai_embeddings");
    assert_eq!(siliconflow["key_prefix"], "sk-");
    assert_eq!(
        siliconflow["key_page"],
        json!({ "url": "https://cloud.siliconflow.com/account/ak", "label": "API Keys" })
    );
    let cloudflare = listed
        .iter()
        .find(|p| p["id"] == "cloudflare-ai-gateway")
        .unwrap();
    assert_eq!(cloudflare["name"], "Cloudflare AI Gateway");
    assert_eq!(
        cloudflare["dialects"],
        json!([
            "openai_chat",
            "openai_responses",
            "anthropic_messages",
            "openai_embeddings"
        ])
    );
    assert_eq!(
        (
            &cloudflare["count_tokens"],
            &cloudflare["embeddings"],
            &cloudflare["key_prefix"]
        ),
        (&Value::Null, &json!("openai_embeddings"), &Value::Null)
    );
    assert_eq!(
        cloudflare["account_scope"],
        json!({
            "account_page": { "url": "https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/", "label": "Find account and zone IDs" },
            "gateway_header": "cf-aig-gateway-id"
        })
    );
    assert_eq!(cloudflare["service_account"], Value::Null);
    let standardcompute = listed
        .iter()
        .find(|p| p["id"] == "standardcompute")
        .unwrap();
    assert_eq!(standardcompute["name"], "Standard Compute");
    assert_eq!(standardcompute["dialects"], json!(["openai_chat"]));
    assert_eq!(
        (
            &standardcompute["count_tokens"],
            &standardcompute["embeddings"],
            &standardcompute["key_prefix"]
        ),
        (&Value::Null, &Value::Null, &Value::Null)
    );
    assert_eq!(
        standardcompute["key_page"],
        json!({ "url": "https://standardcompute.com/dashboard/api-keys", "label": "API Keys" })
    );
    let perplexity = listed.iter().find(|p| p["id"] == "perplexity").unwrap();
    assert_eq!(perplexity["name"], "Perplexity");
    assert_eq!(perplexity["dialects"], json!(["openai_chat"]));
    assert_eq!(
        (
            &perplexity["count_tokens"],
            &perplexity["embeddings"],
            &perplexity["key_prefix"]
        ),
        (&Value::Null, &Value::Null, &json!("pplx-"))
    );
    assert_eq!(
        perplexity["key_page"],
        json!({ "url": "https://console.perplexity.ai/project/keys", "label": "API Keys" })
    );
    let typesafe = listed.iter().find(|p| p["id"] == "typesafe").unwrap();
    assert_eq!(typesafe["name"], "TypeSafe");
    assert_eq!(typesafe["dialects"], json!(["system_one"]));
    assert_eq!(
        (
            &typesafe["count_tokens"],
            &typesafe["embeddings"],
            &typesafe["key_prefix"]
        ),
        (&Value::Null, &Value::Null, &Value::Null)
    );
    assert_eq!(
        typesafe["key_page"],
        json!({ "url": "https://console.typesafe.ai/keys", "label": "API keys" })
    );
    let deepinfra = listed.iter().find(|p| p["id"] == "deepinfra").unwrap();
    assert_eq!(deepinfra["name"], "DeepInfra");
    assert_eq!(
        deepinfra["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(
        (
            &deepinfra["count_tokens"],
            &deepinfra["embeddings"],
            &deepinfra["key_prefix"]
        ),
        (&Value::Null, &json!("openai_embeddings"), &Value::Null)
    );
    assert_eq!(
        deepinfra["key_page"],
        json!({ "url": "https://deepinfra.com/dash/keys", "label": "API keys" })
    );
    let upstage = listed.iter().find(|p| p["id"] == "upstage").unwrap();
    assert_eq!(upstage["name"], "Upstage");
    assert_eq!(
        upstage["dialects"],
        json!(["openai_chat", "openai_embeddings"])
    );
    assert_eq!(
        (
            &upstage["count_tokens"],
            &upstage["embeddings"],
            &upstage["key_prefix"]
        ),
        (&Value::Null, &json!("openai_embeddings"), &Value::Null)
    );
    assert_eq!(
        upstage["key_page"],
        json!({ "url": "https://console.upstage.ai/api-keys", "label": "API keys" })
    );

    let anonymous = app
        .call(Method::GET, &format!("/orgs/{org}/providers"), None, None)
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}
