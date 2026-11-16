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
