#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat};
use kaeso_kpp::{DialectId, RequestKind};
use serde_json::{Value, json};

const SECRET: &str = "al-voyage-test-credential-0001";
const EMBED: &str = "voyage-4-lite";
const RERANK: &str = "rerank-3";

fn fixture(name: &str) -> String {
    format!("[mock:fixture=../providers/voyage/responses/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/voyage/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "voyage", "name": "voyage", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [EMBED, RERANK] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "voyage", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_voyage(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

fn documents() -> Value {
    json!([
        "The Mediterranean diet emphasizes fish, olive oil, and vegetables, believed to reduce chronic diseases.",
        "Photosynthesis in plants converts light energy into glucose and produces essential oxygen.",
        "Apple's conference call to discuss fourth fiscal quarter results and business updates is scheduled for Thursday, November 2, 2023 at 2:00 p.m. PT / 5:00 p.m. ET."
    ])
}

#[tokio::test]
async fn rerank_apps_reach_voyage_with_top_k_and_get_its_scores_and_documents_in_cohere_shape() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "When is Apple's conference call scheduled? [mock:fixture=top-k-documents]",
            "documents": documents(),
            "top_n": 2,
            "return_documents": true,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("voyage"));
    let sent = sent_to_voyage(&env, "/v1/rerank");
    assert_eq!(
        sent,
        json!({
            "query": "When is Apple's conference call scheduled? [mock:fixture=top-k-documents]",
            "documents": documents(),
            "model": RERANK,
            "top_k": 2,
            "return_documents": true,
        })
    );
    let reply = r.json();
    assert_eq!(reply["results"][0]["index"], 2, "{reply}");
    assert_eq!(reply["results"][0]["relevance_score"], 0.9140625);
    assert_eq!(reply["results"][0]["document"]["text"], documents()[2]);
    assert_eq!(reply["results"][1]["index"], 0);
    assert_eq!(reply["meta"]["billed_units"]["input_tokens"], 94);
    assert!(reply.get("data").is_none() && reply.get("usage").is_none());
}

#[tokio::test]
async fn a_voyage_rerank_is_billed_by_its_total_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK, "query": "q", "documents": ["a", "b"] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_voyage(&env, "/v1/rerank");
    assert_eq!(
        sent,
        json!({ "query": "q", "documents": ["a", "b"], "model": RERANK })
    );
    let reply = r.json();
    assert_eq!(reply["results"][0]["index"], 1, "{reply}");
    assert!(reply["results"][0].get("document").is_none());
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    env.app.state.in_flight.settled().await;
    let o = env.capture.recorded(&id).await;
    assert_eq!(
        (o.ingress, o.kind, o.egress),
        (
            DialectId::CohereRerank,
            RequestKind::Rerank,
            Some(DialectId::VoyageRerank)
        )
    );
    assert_eq!(o.usage.clone().unwrap().input_tokens, 4);
}

#[tokio::test]
async fn what_voyage_cant_rerank_is_refused_before_anything_is_sent() {
    let env = env().await;
    let cases = [
        (
            json!({ "model": RERANK, "query": "q", "documents": ["a"], "max_tokens_per_doc": 512 }),
            "`max_tokens_per_doc` is not supported by Voyage AI, which cuts documents at the model's limit; remove it.",
        ),
        (
            json!({ "model": RERANK, "query": "q", "documents": [{ "text": "a", "title": "A" }] }),
            "Voyage AI takes each document as text only; remove `title` from `documents[0]`.",
        ),
    ];
    for (body, message) in cases {
        let r = call(&env, "/v1/rerank", Some(&env.key), false, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["message"], message);
    }
    // Cohere's own fields have no place at Voyage AI, which refuses fields it doesn't know.
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "q",
            "documents": ["a"],
            "relevant_spans": { "mode": "enabled" },
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["message"]
            .as_str()
            .unwrap()
            .contains("relevant_spans"),
        "{}",
        r.text()
    );
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
async fn voyage_rerank_errors_reach_rerank_apps_with_voyages_text() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "q [mock:error=recorded-object-documents-400]",
            "documents": ["a"],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let message = r.json()["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("supplied for argument 'documents' is not a valid string"),
        "{message}"
    );
}

#[tokio::test]
async fn embeddings_apps_reach_voyage_with_its_field_names_and_get_prompt_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": format!("When is the conference call scheduled? {}", fixture("query-base64")),
            "dimensions": 256,
            "encoding_format": "base64",
            "input_type": "query",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("voyage"));
    let sent = sent_to_voyage(&env, "/v1/embeddings");
    assert_eq!(sent["output_dimension"], 256);
    assert_eq!(sent["encoding_format"], "base64");
    assert_eq!(sent["input_type"], "query");
    assert!(sent.get("dimensions").is_none(), "{sent}");
    let reply = r.json();
    assert_eq!(reply["data"][0]["embedding"], "AACAPwAAAMAAAEBA");
    assert_eq!(
        reply["usage"],
        json!({ "prompt_tokens": 9, "total_tokens": 9 })
    );

    // Floats are Voyage AI's default, which it takes only as no `encoding_format`.
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": [format!("Sample text 1 {}", fixture("published-documents")), "Sample text 2"],
            "encoding_format": "float",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_voyage(&env, "/v1/embeddings");
    assert!(sent.get("encoding_format").is_none(), "{sent}");
    assert_eq!(r.json()["usage"]["prompt_tokens"], 10);
}

#[tokio::test]
async fn openai_fields_voyage_refuses_are_refused_before_anything_is_sent() {
    let env = env().await;
    let cases = [
        (
            json!({ "model": EMBED, "input": "a", "dimensions": 300 }),
            "Voyage AI takes `dimensions` of 256, 512, 1024, 2048 only, not 300.",
        ),
        (
            json!({ "model": EMBED, "input": "a", "user": "user-0001" }),
            "`user` is not supported by Voyage AI; remove it from the request.",
        ),
        (
            json!({ "model": EMBED, "input": [1, 2, 3] }),
            "Voyage AI takes `input` as text only; send strings instead of token ids.",
        ),
    ];
    for (body, message) in cases {
        let r = call(&env, "/v1/embeddings", Some(&env.key), false, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["message"], message);
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(EMBED, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn voyage_embedding_errors_reach_openai_apps_with_voyages_text() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": format!("a {}", error("recorded-openai-fields-400")),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("Argument 'user' is not supported by our API"),
        "{message}"
    );
}
