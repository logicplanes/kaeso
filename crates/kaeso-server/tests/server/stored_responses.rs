#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::{Answer, Env, call};
use serde_json::{Value, json};
use tower::ServiceExt;

// The id of the mock's stored reply (`openai_responses/responses/text-input-string.json`).
const STORED: &str = "resp_67ccd2bed1ec8190b14f964abc0542670bb6a6b452d3795b";
const SETTING: &str = "Responses stored at the provider are off for this project; an admin can turn on \"Allow responses stored at the provider\" in the organization's and the project's settings, or send `store: false` and the whole conversation in `input`.";

async fn patch(env: &Env, path: &str, body: Value) -> common::Resp {
    env.app
        .call(Method::PATCH, path, Some(body), Some(&env.owner.cookie))
        .await
}

async fn allow(env: &Env, project: &str) {
    let org = format!("/orgs/{}/stored-responses", env.owner.org_id);
    let r = patch(env, &org, json!({ "allowed": true })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = patch(
        env,
        &format!(
            "/orgs/{}/projects/{project}/stored-responses",
            env.owner.org_id
        ),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["effective"], true);
}

async fn respond(env: &Env, key: &str, body: Value) -> Answer {
    call(env, "/v1/responses", Some(key), false, Some(body)).await
}

async fn stored(env: &Env, key: &str, method: &str, id: &str) -> Answer {
    let req = Request::builder()
        .uri(format!("/v1/responses/{id}"))
        .method(method)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

fn sent(env: &Env) -> Value {
    env.mock.last().unwrap().json()
}

#[tokio::test]
async fn both_switches_start_off_and_refuse_asking_for_stored_state_by_the_setting() {
    let env = common::v1::env(None, None).await;
    let r = env
        .app
        .get(
            &format!("/orgs/{}/stored-responses", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.body, json!({ "allowed": false }));
    let project = format!(
        "/orgs/{}/projects/{}/stored-responses",
        env.owner.org_id, env.project
    );
    let r = env.app.get(&project, &env.owner.cookie).await;
    assert_eq!(
        r.body,
        json!({ "enabled": false, "org_allowed": false, "effective": false })
    );
    let r = patch(&env, &project, json!({ "enabled": true })).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);

    for body in [
        json!({ "model": "gpt-4o", "input": "Hi", "store": true }),
        json!({ "model": "gpt-4o", "input": "Hi", "previous_response_id": STORED }),
    ] {
        let before = env.mock.received().len();
        let r = respond(&env, &env.key, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["message"], SETTING);
        assert_eq!(env.mock.received().len(), before, "refused before sending");
    }
    let r = respond(&env, &env.key, json!({ "model": "gpt-4o", "input": "Hi" })).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "a request that asks for nothing goes as before"
    );
    assert_eq!(
        stored(&env, &env.key, "GET", STORED).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_project_stores_continues_reads_and_deletes_a_response_at_openai() {
    let env = common::v1::env(None, None).await;
    allow(&env, &env.project).await;

    let r = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Tell me a joke." }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["id"], STORED);
    assert_eq!(
        sent(&env).get("store"),
        None,
        "OpenAI stores unless told not to"
    );

    let r = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Explain it.", "previous_response_id": STORED, "store": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let continued = sent(&env);
    assert_eq!(continued["previous_response_id"], STORED);
    assert_eq!(continued["store"], true);

    let r = stored(&env, &env.key, "GET", STORED).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["id"], STORED);
    assert_eq!(r.json()["object"], "response");
    let asked = env.mock.last().unwrap();
    assert_eq!(asked.path, format!("/v1/responses/{STORED}"));
    assert_eq!(
        asked.header("authorization"),
        Some(format!("Bearer {}", common::v1::OPENAI_SECRET).as_str())
    );

    let r = stored(&env, &env.key, "DELETE", STORED).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json(),
        json!({ "id": STORED, "object": "response.deleted", "deleted": true })
    );
    let r = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Again?", "previous_response_id": STORED }),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("Previous response with id '{STORED}' not found.")
    );
    assert_eq!(r.json()["error"]["param"], "previous_response_id");

    let r = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Not kept.", "store": false }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(sent(&env)["store"], false);
    assert_eq!(
        stored(&env, &env.key, "GET", STORED).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn another_project_on_the_same_provider_key_cant_continue_or_read_it() {
    let env = common::v1::env(None, None).await;
    allow(&env, &env.project).await;
    let r = respond(&env, &env.key, json!({ "model": "gpt-4o", "input": "Hi" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    let other =
        common::create_project(&env.app, &env.owner.org_id, &env.owner.cookie, "Other").await;
    allow(&env, &other).await;
    let base = format!("/orgs/{}/projects/{other}", env.owner.org_id);
    let r = env
        .app
        .post(
            &format!("{base}/models"),
            json!({ "provider": "openai", "model_id": "gpt-4o" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = env.mock.received().len();
    let r = respond(
        &env,
        &key,
        json!({ "model": "gpt-4o", "input": "Go on.", "previous_response_id": STORED }),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    for method in ["GET", "DELETE"] {
        let r = stored(&env, &key, method, STORED).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{method}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!("No response found with id '{STORED}'.")
        );
    }
    assert_eq!(env.mock.received().len(), before, "nothing reached OpenAI");
    assert_eq!(
        stored(&env, &env.key, "GET", STORED).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_model_in_another_format_is_refused_naming_the_field() {
    let env = common::v1::env(None, None).await;
    allow(&env, &env.project).await;
    let r = respond(
        &env,
        &env.key,
        json!({ "model": "claude-haiku-4-5", "input": "Hi", "previous_response_id": STORED }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "previous_response_id");
    assert_eq!(
        r.json()["error"]["message"],
        "`previous_response_id` needs a provider that keeps responses, called in the Responses format; Anthropic serves `claude-haiku-4-5` in another way. Send the whole conversation in `input` instead."
    );
    let r = respond(
        &env,
        &env.key,
        json!({ "model": "claude-haiku-4-5", "input": "Hi", "store": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "store");
    let r = respond(
        &env,
        &env.key,
        json!({ "model": "claude-haiku-4-5", "input": "Hi" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

async fn upload_batch(env: &Env, key: &str, lines: &[Value]) -> Answer {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let form = kaeso_codecs::multipart::FormWriter::write(&[
        kaeso_codecs::multipart::FormField::Text("purpose", "batch"),
        kaeso_codecs::multipart::FormField::File(
            "file",
            "input.jsonl",
            "application/jsonl",
            text.as_bytes(),
        ),
    ]);
    let req = Request::builder()
        .uri("/v1/files")
        .method(Method::POST)
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", form.content_type())
        .body(Body::from(form.into_body()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

#[tokio::test]
async fn item_references_omitted_store_and_batches_follow_stored_response_rules() {
    let env = common::v1::env(None, None).await;

    let plain = respond(&env, &env.key, json!({ "model": "gpt-4o", "input": "Hi" })).await;
    assert_eq!(plain.status, StatusCode::OK, "{}", plain.text());
    assert_eq!(
        sent(&env)["store"],
        false,
        "when stored responses are off, OpenAI is told not to store"
    );

    let before = env.mock.received().len();
    let item_ref = respond(
        &env,
        &env.key,
        json!({
            "model": "gpt-4o",
            "input": [{ "type": "item_reference", "id": "msg_other_project_1" }]
        }),
    )
    .await;
    assert_eq!(
        item_ref.status,
        StatusCode::BAD_REQUEST,
        "{}",
        item_ref.text()
    );
    assert_eq!(item_ref.json()["error"]["param"], "input");
    assert_eq!(env.mock.received().len(), before);

    let dotted_store = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Hi", "a.b": 1, "store": true }),
    )
    .await;
    assert_eq!(
        dotted_store.status,
        StatusCode::BAD_REQUEST,
        "{}",
        dotted_store.text()
    );
    assert_eq!(dotted_store.json()["error"]["message"], SETTING);
    assert_eq!(env.mock.received().len(), before);

    let count_item_ref = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({
            "model": "gpt-4o",
            "input": [{ "type": "item_reference", "id": "msg_other_project_1" }]
        })),
    )
    .await;
    assert_eq!(
        count_item_ref.status,
        StatusCode::BAD_REQUEST,
        "{}",
        count_item_ref.text()
    );
    assert_eq!(count_item_ref.json()["error"]["param"], "input");
    assert_eq!(env.mock.received().len(), before);

    for bad_body in [
        json!({ "model": "gpt-4o", "input": "Hi", "store": true }),
        json!({ "model": "gpt-4o", "input": "Hi", "previous_response_id": STORED }),
        json!({ "model": "gpt-4o", "input": [{ "type": "item_reference", "id": "msg_1" }] }),
        json!({ "model": "gpt-4o", "input": [{ "type": "item_reference", "id": "rs_1" }] }),
    ] {
        let before = env.mock.received().len();
        let line = json!({
            "custom_id": "req-1",
            "method": "POST",
            "url": "/v1/responses",
            "body": bad_body
        });
        let r = upload_batch(&env, &env.key, &[line]).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(env.mock.received().len(), before);
    }

    let ok_batch = upload_batch(
        &env,
        &env.key,
        &[json!({
            "custom_id": "req-ok",
            "method": "POST",
            "url": "/v1/responses",
            "body": { "model": "gpt-4o", "input": "Plain batch line." }
        })],
    )
    .await;
    assert_eq!(ok_batch.status, StatusCode::OK, "{}", ok_batch.text());
    let uploaded_raw = String::from_utf8_lossy(&env.mock.last().unwrap().body).into_owned();
    assert!(uploaded_raw.contains("\"store\":false"), "{uploaded_raw}");
    let ok_file_id = ok_batch.json()["id"].as_str().unwrap().to_owned();
    let del_ok = Request::builder()
        .uri(format!("/v1/files/{ok_file_id}"))
        .method(Method::DELETE)
        .header("authorization", format!("Bearer {}", env.key))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        env.app
            .router
            .clone()
            .oneshot(del_ok)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    allow(&env, &env.project).await;
    let raw_escaped = Request::builder()
        .uri("/v1/responses")
        .method(Method::POST)
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gpt-4o","input":[{"type":"item_\u0072eference","id":"msg_other_project_1"}]}"#,
        ))
        .unwrap();
    let raw_resp = env.app.router.clone().oneshot(raw_escaped).await.unwrap();
    assert_eq!(raw_resp.status(), StatusCode::BAD_REQUEST);
    let raw_body: Value = serde_json::from_slice(
        &http_body_util::BodyExt::collect(raw_resp.into_body())
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(raw_body["error"]["param"], "input");

    let stored_batch_upload = upload_batch(
        &env,
        &env.key,
        &[json!({
            "custom_id": "req-store",
            "method": "POST",
            "url": "/v1/responses",
            "body": { "model": "gpt-4o", "input": "Keep from batch.", "store": true }
        })],
    )
    .await;
    assert_eq!(
        stored_batch_upload.status,
        StatusCode::OK,
        "{}",
        stored_batch_upload.text()
    );
    let stored_file_id = stored_batch_upload.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created_batch = call(
        &env,
        "/v1/batches",
        Some(&env.key),
        false,
        Some(json!({
            "input_file_id": stored_file_id,
            "endpoint": "/v1/responses",
            "completion_window": "24h"
        })),
    )
    .await;
    assert_eq!(
        created_batch.status,
        StatusCode::OK,
        "{}",
        created_batch.text()
    );
    let batch_id = created_batch.json()["id"].as_str().unwrap().to_owned();
    let poll_req = Request::builder()
        .uri(format!("/v1/batches/{batch_id}"))
        .method(Method::GET)
        .header("authorization", format!("Bearer {}", env.key))
        .body(Body::empty())
        .unwrap();
    let poll_resp = env.app.router.clone().oneshot(poll_req).await.unwrap();
    assert_eq!(poll_resp.status(), StatusCode::OK);
    assert_eq!(
        stored(&env, &env.key, "GET", STORED).await.status,
        StatusCode::OK
    );
    assert_eq!(
        stored(&env, &env.key, "DELETE", STORED).await.status,
        StatusCode::OK
    );

    let disable_org = patch(
        &env,
        &format!("/orgs/{}/stored-responses", env.owner.org_id),
        json!({ "allowed": false }),
    )
    .await;
    assert_eq!(disable_org.status, StatusCode::OK);
    let stale_batch = call(
        &env,
        "/v1/batches",
        Some(&env.key),
        false,
        Some(json!({
            "input_file_id": stored_file_id,
            "endpoint": "/v1/responses",
            "completion_window": "24h"
        })),
    )
    .await;
    assert_eq!(
        stale_batch.status,
        StatusCode::BAD_REQUEST,
        "{}",
        stale_batch.text()
    );
    assert_eq!(stale_batch.json()["error"]["param"], "input_file_id");
    allow(&env, &env.project).await;

    let stored_reply = respond(
        &env,
        &env.key,
        json!({ "model": "gpt-4o", "input": "Keep this." }),
    )
    .await;
    assert_eq!(stored_reply.status, StatusCode::OK);

    let other =
        common::create_project(&env.app, &env.owner.org_id, &env.owner.cookie, "Other").await;
    allow(&env, &other).await;
    let base = format!("/orgs/{}/projects/{other}", env.owner.org_id);
    env.app
        .post(
            &format!("{base}/models"),
            json!({ "provider": "openai", "model_id": "gpt-4o" }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let other_key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();

    let cross_project_batch = upload_batch(
        &env,
        &other_key,
        &[json!({
            "custom_id": "req-cross",
            "method": "POST",
            "url": "/v1/responses",
            "body": { "model": "gpt-4o", "input": "Continue.", "previous_response_id": STORED }
        })],
    )
    .await;
    assert_eq!(
        cross_project_batch.status,
        StatusCode::BAD_REQUEST,
        "{}",
        cross_project_batch.text()
    );
    assert!(
        cross_project_batch
            .text()
            .contains(&format!("Previous response with id '{STORED}' not found.")),
        "{}",
        cross_project_batch.text()
    );

    let own_project_batch = upload_batch(
        &env,
        &env.key,
        &[json!({
            "custom_id": "req-own",
            "method": "POST",
            "url": "/v1/responses",
            "body": { "model": "gpt-4o", "input": "Continue.", "previous_response_id": STORED }
        })],
    )
    .await;
    assert_eq!(
        own_project_batch.status,
        StatusCode::OK,
        "{}",
        own_project_batch.text()
    );
}
