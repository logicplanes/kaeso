#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat};
use kaeso_kpp::{DialectId, RequestKind};
use serde_json::{Value, json};

const COHERE_SECRET: &str = "cohere-test-credential-0007";
const RERANK: &str = "rerank-v5.0-fast";
const COMMAND: &str = "command-a-03-2025";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "cohere", "name": "cohere", "secret": COHERE_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [RERANK, COMMAND] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "cohere", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn documents() -> Value {
    json!([
        "The Starter plan includes email support.",
        "The Business plan includes priority support by phone and chat.",
        "All plans include the community forum."
    ])
}

fn sent_to_cohere(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path, "/v2/rerank",
        "Cohere reranks at the root of its host"
    );
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {COHERE_SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn rerank_apps_reach_cohere_at_the_root_of_its_host_and_get_its_scores() {
    let env = env().await;
    for path in ["/v1/rerank", "/v2/rerank"] {
        env.mock.clear();
        let r = call(
            &env,
            path,
            Some(&env.key),
            false,
            Some(json!({
                "model": RERANK,
                "query": "Which plan includes priority support?",
                "documents": documents(),
                "top_n": 2,
                "max_tokens_per_doc": 512,
            })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{path}: {}", r.text());
        assert_eq!(r.header("kaeso-provider"), Some("cohere"));
        let sent = sent_to_cohere(&env);
        assert_eq!(
            sent,
            json!({
                "model": RERANK,
                "query": "Which plan includes priority support?",
                "documents": documents(),
                "top_n": 2,
                "max_tokens_per_doc": 512,
            })
        );
        let reply = r.json();
        assert_eq!(reply["results"][0]["index"], 1, "{reply}");
        assert_eq!(reply["results"][0]["relevance_score"], 0.9312);
        assert_eq!(reply["meta"]["billed_units"]["input_tokens"], 61);
    }
}

#[tokio::test]
async fn documents_as_objects_reach_cohere_as_text_and_come_back_when_asked() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "Which plan includes priority support?",
            "documents": [
                { "text": "The Starter plan includes email support." },
                { "text": "The Business plan includes priority support by phone and chat." },
            ],
            "return_documents": true,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cohere(&env);
    assert!(
        sent["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_string)
    );
    assert!(sent.get("return_documents").is_none(), "{sent}");
    let reply = r.json();
    assert_eq!(
        reply["results"][0]["document"],
        json!({ "text": "The Business plan includes priority support by phone and chat." })
    );
    assert_eq!(
        reply["results"][1]["document"],
        json!({ "text": "The Starter plan includes email support." })
    );

    env.mock.clear();
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({
            "model": RERANK,
            "query": "q",
            "documents": [{ "text": "a", "title": "A" }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["message"],
        "Cohere's Rerank API takes each document as text only; remove `title` from `documents[0]`."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn rerank_and_other_models_refuse_each_others_endpoints_before_anything_is_sent() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": COMMAND, "query": "q", "documents": ["a"] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        body["message"],
        format!("The model `{COMMAND}` is a generation model; it cannot rerank documents.")
    );
    assert_eq!(body["id"], r.header("kaeso-request-id").unwrap());

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(RERANK, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{RERANK}` is a rerank model; send it to `/v1/rerank`.")
    );

    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK, "query": "q", "documents": [] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["message"], "`documents` must not be empty.");
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn cohere_errors_reach_rerank_apps_as_cohere_sent_them() {
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
        r.json(),
        json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "message": "Too many requests; try again in a minute."
        })
    );
}

#[tokio::test]
async fn a_rerank_is_billed_by_the_tokens_cohere_reports_and_logged_under_its_kind() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK, "query": "q", "documents": documents() })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.header("kaeso-request-id").unwrap().to_owned();
    env.app.state.in_flight.settled().await;
    let o = env.capture.recorded(&id).await;
    assert_eq!(
        (o.ingress, o.kind, o.egress),
        (
            DialectId::CohereRerank,
            RequestKind::Rerank,
            Some(DialectId::CohereRerank)
        )
    );
    let usage = o.usage.clone().unwrap();
    assert_eq!((usage.input_tokens, usage.output_tokens), (61, 0));
    assert!(!o.usage_estimated);
    assert_eq!(o.response_model.as_deref(), Some(RERANK));
    assert_eq!(
        kaeso_server::services::logs::endpoint(&o),
        "rerank",
        "the request log names the endpoint"
    );
}

#[tokio::test]
async fn a_repeated_rerank_is_answered_from_the_response_cache() {
    let env = env().await;
    let r = env
        .app
        .call(
            axum::http::Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/response-cache",
                env.owner.org_id, env.project
            ),
            Some(json!({ "enabled": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let body = json!({ "model": RERANK, "query": "Which plan includes priority support?", "documents": documents() });
    let first = call(
        &env,
        "/v2/rerank",
        Some(&env.key),
        false,
        Some(body.clone()),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(first.header("kaeso-cache"), Some("miss"));
    env.app.state.in_flight.settled().await;
    let second = call(&env, "/v2/rerank", Some(&env.key), false, Some(body)).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert_eq!(second.header("kaeso-cache"), Some("hit"));
    assert_eq!(second.json()["results"], first.json()["results"]);
    assert_eq!(env.mock.received().len(), 1);
}

#[tokio::test]
async fn a_top_n_above_the_number_of_documents_reaches_cohere_as_written() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/rerank",
        Some(&env.key),
        false,
        Some(json!({ "model": RERANK, "query": "q", "documents": documents(), "top_n": 10 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_cohere(&env)["top_n"], 10);
}
