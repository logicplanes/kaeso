#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "pplx-0123456789abcdef0123456789abcdef";
const SONAR_PRO: &str = "sonar-pro";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/perplexity/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/perplexity/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "perplexity", "name": "perplexity", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "perplexity", "model_id": SONAR_PRO }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn sent_to_perplexity(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/sonar");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_send_search_fields_as_written_and_get_citations_back() {
    let env = env().await;
    let mut body = chat(
        SONAR_PRO,
        &format!(
            "What did the IPCC conclude about sea level rise? {}",
            fixture("responses", "citations-doc")
        ),
    );
    body["max_completion_tokens"] = json!(600);
    body["search_mode"] = json!("academic");
    body["search_domain_filter"] = json!(["ipcc.ch"]);
    body["web_search_options"] = json!({ "search_context_size": "high" });
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("perplexity"));
    let reply = r.json();
    assert_eq!(
        reply["citations"],
        json!([
            "https://www.ipcc.ch/report/ar6/syr/",
            "https://www.ipcc.ch/srocc/chapter/summary-for-policymakers/"
        ])
    );
    assert_eq!(
        reply["search_results"][0]["title"],
        "Climate Change 2023: Synthesis Report"
    );
    assert_eq!(reply["usage"]["num_search_queries"], 2);
    assert!(
        reply["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .ends_with("in recent decades [2].")
    );

    let sent = sent_to_perplexity(&env);
    assert_eq!(sent["model"], SONAR_PRO);
    assert_eq!(sent["max_tokens"], 600);
    assert_eq!(sent["search_mode"], "academic");
    assert_eq!(sent["search_domain_filter"], json!(["ipcc.ch"]));
    assert_eq!(
        sent["web_search_options"],
        json!({ "search_context_size": "high" })
    );
    assert!(sent.get("max_completion_tokens").is_none(), "{sent}");

    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("perplexity"));
    let usage = o.usage.clone().unwrap();
    assert_eq!((usage.input_tokens, usage.output_tokens), (31, 42));
}

#[tokio::test]
async fn anthropic_apps_stream_the_answer_once_with_its_last_usage() {
    let env = env().await;
    let mut body = messages(
        SONAR_PRO,
        &format!(
            "What did the IPCC conclude? {}",
            fixture("streams", "full-mode-doc")
        ),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(
        text,
        "The IPCC's AR6 Synthesis Report concludes that sea level rise is unavoidable for centuries to millennia [1]."
    );
    let delta = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(delta["delta"]["stop_reason"], "end_turn");
    assert_eq!(delta["usage"]["output_tokens"], 24);

    let sent = sent_to_perplexity(&env);
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["max_tokens"], 256);
    assert!(sent.get("stream_options").is_none(), "{sent}");
}

#[tokio::test]
async fn what_perplexity_does_not_take_is_refused_before_anything_is_sent() {
    let env = env().await;
    let mut tools = chat(SONAR_PRO, "What is the weather in Paris?");
    tools["tools"] = json!([{
        "type": "function",
        "function": { "name": "get_weather", "parameters": { "type": "object" } }
    }]);
    let mut concise = chat(SONAR_PRO, "Hello");
    concise["stream_mode"] = json!("concise");
    for (body, message) in [
        (
            tools,
            "The model `sonar-pro` does not support tool calling.",
        ),
        (
            concise,
            "`stream_mode` is not supported by Perplexity; remove it from the request.",
        ),
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(body),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["message"], message);
    }
    let mut search = messages(SONAR_PRO, "Hello");
    search["search_mode"] = json!("academic");
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(search)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn perplexity_errors_reach_apps_with_their_status() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            SONAR_PRO,
            &format!("Hello {}", error("request-rate-limit-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            SONAR_PRO,
            &format!("Hello {}", error("validation-error-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("perplexity"));
    assert!(!format!("{o:?}").contains(SECRET));
}
