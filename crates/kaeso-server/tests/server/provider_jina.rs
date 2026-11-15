#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat};
use kaeso_kpp::{DialectId, RequestKind};
use serde_json::{Value, json};

// Jina AI's keys start with `jina_`, which the mock reads to answer `/v1/rerank` as Jina.
const SECRET: &str = "jina_test-credential-0001";
const EMBED: &str = "jina-embeddings-v5-text-nano";
const RERANK: &str = "jina-reranker-v3";
const RERANK_V2: &str = "jina-reranker-v2-base-multilingual";

fn fixture(name: &str) -> String {
    format!("[mock:fixture=../providers/jina/responses/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/jina/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "jina", "name": "jina", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [EMBED, RERANK, RERANK_V2] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "jina", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_jina(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn rerank_apps_reach_jina_with_return_documents_written_and_get_its_scores() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "Climate change impact on renewable energy adoption",
            "documents": ["a", "b", "c", "d"],
            "top_n": 4,
            "max_tokens_per_doc": 1024,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("jina"));
    let sent = sent_to_jina(&env, "/v1/rerank");
    assert_eq!(
        sent,
        json!({
            "model": RERANK,
            "query": "Climate change impact on renewable energy adoption",
            "documents": ["a", "b", "c", "d"],
            "top_n": 4,
            "return_documents": false,
            "max_doc_length": 1024,
        })
    );
    let reply = r.json();
    assert_eq!(reply["results"][0]["index"], 0, "{reply}");
    assert_eq!(reply["results"][0]["relevance_score"], 0.6875);
    assert!(reply["results"][0].get("document").is_none());
    assert_eq!(reply["meta"]["billed_units"]["input_tokens"], 512);
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    env.app.state.in_flight.settled().await;
    let o = env.capture.recorded(&id).await;
    assert_eq!(
        (o.ingress, o.kind, o.egress),
        (
            DialectId::CohereRerank,
            RequestKind::Rerank,
            Some(DialectId::JinaRerank)
        )
    );
    assert_eq!(o.usage.clone().unwrap().input_tokens, 512);
}

#[tokio::test]
async fn documents_jina_echoes_reach_rerank_apps_as_text_objects() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "Which plan includes priority support? [mock:fixture=text-objects]",
            "documents": [
                { "text": "The Starter plan includes email support." },
                "The Business plan includes priority support by phone and chat."
            ],
            "return_documents": true,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_jina(&env, "/v1/rerank");
    assert_eq!(sent["return_documents"], true);
    assert!(
        sent["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_string),
        "{sent}"
    );
    let reply = r.json();
    assert_eq!(
        reply["results"][0]["document"],
        json!({ "text": "The Business plan includes priority support by phone and chat." })
    );
    assert_eq!(
        reply["results"][1]["document"],
        json!({ "text": "The Starter plan includes email support." })
    );
}

#[tokio::test]
async fn what_jina_cant_rerank_is_refused_before_anything_is_sent() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK_V2, "query": "q", "documents": ["a"], "max_tokens_per_doc": 512 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["message"],
        format!(
            "`max_tokens_per_doc` is supported by Jina AI's `jina-reranker-v3` and `jina-reranker-v3.5` only, not `{RERANK_V2}`; remove it."
        )
    );
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK, "query": "q", "documents": ["a"], "priority": 1 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(RERANK, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn jina_rerank_errors_reach_rerank_apps_with_jinas_text_and_kind() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "q [mock:error=rate-limit-429]",
            "documents": ["a"],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.header("retry-after"), Some("60"));
    assert_eq!(
        r.json()["message"],
        "Too many requests; try again in a minute."
    );
}

#[tokio::test]
async fn embeddings_apps_reach_jina_with_embedding_type_and_get_its_vectors() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": [format!("When is the conference call scheduled? {}", fixture("query-base64"))],
            "encoding_format": "base64",
            "dimensions": 256,
            "task": "retrieval.query",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("jina"));
    let sent = sent_to_jina(&env, "/v1/embeddings");
    assert_eq!(
        (&sent["embedding_type"], &sent["dimensions"], &sent["task"]),
        (&json!("base64"), &json!(256), &json!("retrieval.query"))
    );
    assert!(sent.get("encoding_format").is_none(), "{sent}");
    let reply = r.json();
    assert_eq!(reply["data"][0]["embedding"], "AACAPwAAAMAAAEBA");
    assert_eq!(
        reply["usage"],
        json!({ "prompt_tokens": 9, "total_tokens": 9 })
    );
}

#[tokio::test]
async fn openai_fields_jina_has_no_place_for_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (body, message) in [
        (
            json!({ "model": EMBED, "input": "a", "user": "user-0001" }),
            "`user` is not supported by Jina AI; remove it from the request.",
        ),
        (
            json!({ "model": EMBED, "input": [1, 2, 3] }),
            "Jina AI takes `input` as text only; send strings instead of token ids.",
        ),
    ] {
        let r = call(&env, "/v1/embeddings", Some(&env.key), false, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["message"], message);
    }
    // Jina's own inputs as objects have no place in OpenAI's embeddings format.
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": [{ "image": "https://example.com/a.png" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_key_jina_refuses_reaches_openai_apps_as_a_bad_gateway() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": format!("a {}", error("recorded-invalid-key-401")),
        })),
    )
    .await;
    // A key Jina refuses is the organization's provider key, not the app's: a bad gateway.
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
}
