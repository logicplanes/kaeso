#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "azure-test-credential-0001";
const ENDPOINT: &str = "https://example.openai.azure.com";
const GPT: &str = "gpt-5";
const MINI: &str = "gpt-5-mini";
const EMBED: &str = "text-embedding-3-large";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/azure/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/azure/errors/{name}]")
}

fn credentials(env: &Env) -> String {
    format!("/orgs/{}/credentials", env.owner.org_id)
}

fn models(env: &Env) -> String {
    format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project)
}

async fn add_key(env: &Env, body: Value) -> common::Resp {
    env.app
        .post(&credentials(env), body, Some(&env.owner.cookie))
        .await
}

fn key(extra: Value) -> Value {
    let mut k = json!({ "provider": "azure", "name": "Azure", "secret": SECRET });
    for (name, value) in extra.as_object().unwrap() {
        k[name] = value.clone();
    }
    k
}

// Returns the entry id of each model, in order.
async fn env() -> (Env, Vec<String>) {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = add_key(&env, key(json!({ "endpoint": ENDPOINT }))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let mut ids = Vec::new();
    for model in [GPT, MINI, EMBED] {
        let r = env
            .app
            .post(
                &models(&env),
                json!({ "provider": "azure", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
        ids.push(r.body["id"].as_str().unwrap().to_owned());
    }
    (env, ids)
}

async fn set_values(env: &Env, entry: &str, overrides: Value) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &format!("{}/{entry}", models(env)),
            Some(json!({ "overrides": overrides })),
            Some(&env.owner.cookie),
        )
        .await
}

fn sent_to_azure(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, format!("/openai/v1/{path}"));
    assert_eq!(sent.header("api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    sent.json()
}

#[tokio::test]
async fn an_azure_key_carries_its_resource_endpoint_and_other_keys_take_none() {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    for (endpoint, why) in [
        (None, "an Azure key names its resource"),
        (Some("https://example.com"), "the host is a resource's"),
        (
            Some("http://example.openai.azure.com"),
            "the endpoint is https",
        ),
        (
            Some("https://example.openai.azure.com/anthropic"),
            "the path is the v1 API's or none",
        ),
    ] {
        let extra = endpoint.map_or_else(|| json!({}), |e| json!({ "endpoint": e }));
        let r = add_key(&env, key(extra)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{why}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "endpoint", "{why}");
    }
    let r = add_key(
        &env,
        json!({ "provider": "openai", "name": "OpenAI", "secret": "sk-test-credential-0001", "endpoint": ENDPOINT }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "OpenAI takes no endpoint; leave `endpoint` out."
    );

    let r = add_key(
        &env,
        key(json!({ "endpoint": "https://Example.OpenAI.Azure.com/openai/v1/" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["endpoint"], ENDPOINT);
    let path = format!("{}/{}", credentials(&env), r.body["id"].as_str().unwrap());
    let other = "https://other.services.ai.azure.com";
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Azure", "endpoint": other })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["endpoint"], other);
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": "azure-test-credential-0002" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["endpoint"], other, "a new secret keeps the endpoint");

    let r = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let azure = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "azure")
        .unwrap()
        .clone();
    assert_eq!(azure["name"], "Azure");
    assert_eq!(
        azure["key_endpoint"]["hosts"],
        json!([
            ".openai.azure.com",
            ".services.ai.azure.com",
            ".cognitiveservices.azure.com"
        ])
    );
}

#[tokio::test]
async fn chat_apps_reach_the_v1_api_with_the_key_as_api_key_and_openai_field_names() {
    let (env, _) = env().await;
    let mut body = chat(
        GPT,
        &format!("Hello! {}", fixture("responses", "greeting-published")),
    );
    body["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({ "role": "developer", "content": "Be kind." }));
    body["max_completion_tokens"] = json!(64);
    body["safety_identifier"] = json!("user-0001");
    body["verbosity"] = json!("low");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("azure"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hi there! How can I assist you today?"
    );
    assert_eq!(
        reply["prompt_annotations"][0]["content_filter_results"]["hate"]["severity"], "safe",
        "Azure's filter results reach Chat apps"
    );
    assert_eq!(reply["usage"]["completion_tokens"], 11);
    let sent = sent_to_azure(&env, "chat/completions");
    assert_eq!(sent["model"], GPT);
    assert_eq!(sent["messages"][0]["role"], "developer");
    assert_eq!(
        (
            &sent["max_completion_tokens"],
            &sent["safety_identifier"],
            &sent["verbosity"]
        ),
        (&json!(64), &json!("user-0001"), &json!("low"))
    );
}

#[tokio::test]
async fn a_models_provider_model_id_goes_to_azure_in_place_of_its_id() {
    let (env, ids) = env().await;
    let r = set_values(&env, &ids[0], json!({ "upstream_model": "gpt5 prod" })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "upstream_model");
    let r = set_values(&env, &ids[0], json!({ "upstream_model": "gpt5-prod" })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["effective"]["upstream_model"], "gpt5-prod");

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT,
            &format!("Hello! {}", fixture("responses", "greeting-published")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_azure(&env, "chat/completions")["model"],
        "gpt5-prod"
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.model_id.as_deref(), Some(GPT), "logs keep the model's id");

    let r = set_values(&env, &ids[2], json!({ "upstream_model": "embed-small" })).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": "The quick brown fox." })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_azure(&env, "embeddings")["model"], "embed-small");

    env.mock.clear();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(MINI, "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_azure(&env, "chat/completions")["model"],
        MINI,
        "a model without one sends its own id"
    );
}

#[tokio::test]
async fn azures_filtered_stream_ends_as_a_content_filter_stop_in_every_format() {
    let (env, _) = env().await;
    let mut body = chat(
        GPT,
        &format!(
            "Tell me the lyrics of Hey Jude. {}",
            fixture("streams", "filter-blocked-doc")
        ),
    );
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let chunks: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let text: String = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(text, "Hey Jude, better");
    assert!(
        chunks
            .iter()
            .any(|c| c["choices"][0]["finish_reason"] == "content_filter"),
        "{}",
        r.text()
    );

    let mut body = messages(
        GPT,
        &format!("What is color? {}", fixture("streams", "filter-pass-doc")),
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
    assert_eq!(text, "Color is a");
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["delta"]["stop_reason"], "end_turn");
    assert!(
        events.iter().any(|e| e["type"] == "message_stop"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn responses_and_embeddings_go_to_the_v1_api_and_token_counts_are_refused() {
    let (env, ids) = env().await;
    let r = set_values(
        &env,
        &ids[1],
        json!({ "egress_dialect": "openai_responses" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            MINI,
            &format!(
                "Explain what the bitter lesson is? {}",
                fixture("responses", "bitter-lesson-doc")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.json()["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .starts_with("The bitter lesson refers to"),
        "{}",
        r.text()
    );
    assert_eq!(sent_to_azure(&env, "responses")["model"], MINI);

    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": ["The quick brown fox."] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_azure(&env, "embeddings")["model"], EMBED);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GPT, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_content_filter_refusal_reaches_apps_as_one_and_a_wrong_key_as_a_refused_key() {
    let (env, _) = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT,
            &format!("Pizza? {}", error("content-filter-prompt-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    // A Chat app reads Azure's own error, as for every provider of its format.
    let e = r.json();
    assert_eq!(e["error"]["code"], "content_filter", "{e}");
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.error_kind.as_deref(), Some("content_filtered"));
    assert!(
        e["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("The response was filtered due to the prompt"),
        "{e}"
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            GPT,
            &format!("Pizza? {}", error("content-filter-simple-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());

    for name in ["recorded-invalid-api-key", "recorded-entra-token"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GPT, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("azure"));
    assert!(!format!("{o:?}").contains(SECRET));
}

#[tokio::test]
async fn a_model_check_only_tells_whether_the_key_and_its_endpoint_work() {
    let (env, _) = env().await;
    let r = env
        .app
        .post(
            &format!("{}/check", models(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let reasons: Vec<(&str, &str)> = r.body["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["model_id"].as_str().unwrap(),
                f["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        reasons,
        [
            (GPT, "inconclusive"),
            (MINI, "inconclusive"),
            (EMBED, "inconclusive")
        ],
        "{:?}",
        r.body
    );
    // The project's OpenAI and Anthropic models are checked against those providers, under
    // `/v1/` on the same mock; Azure's v1 API is under `/openai/v1/`.
    let asked: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .filter(|s| !s.path.starts_with("/v1/"))
        .map(|s| {
            assert_eq!(s.header("api-key"), Some(SECRET));
            format!("{} {}", s.method, s.path)
        })
        .collect();
    assert!(
        asked.iter().all(|a| a == "GET /openai/v1/models"),
        "only Azure's model listing is read: {asked:?}"
    );
}

const TENANT: &str = "aaaabbbb-0000-cccc-1111-dddd2222eeee";
const CLIENT: &str = "00001111-aaaa-2222-bbbb-3333cccc4444";
const CLIENT_SECRET: &str = "A1bC2dE3fH4iJ5kL6mN7oP8qR9sT0u";

fn principal(secret: &str, scope: Option<&str>) -> String {
    let mut p = json!({ "tenant_id": TENANT, "client_id": CLIENT, "client_secret": secret });
    if let Some(s) = scope {
        p["scope"] = json!(s);
    }
    p.to_string()
}

async fn entra_env(secret: &str, scope: Option<&str>) -> (Env, String) {
    let env = common::v1::env_with_google_token(Some(Arc::new(Capture::default()))).await;
    let r = add_key(
        &env,
        json!({ "provider": "azure", "name": "Entra", "auth_method": "azure_service_principal", "secret": principal(secret, scope), "endpoint": ENDPOINT }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let id = r.body["id"].as_str().unwrap().to_owned();
    env.app
        .post(
            &models(&env),
            json!({ "provider": "azure", "model_id": GPT }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    (env, id)
}

fn token_requests(env: &Env) -> Vec<kaeso_testkit::mock_provider::Received> {
    env.mock
        .received()
        .into_iter()
        .filter(|r| {
            r.path
                .ends_with(kaeso_testkit::mock_provider::AZURE_TOKEN_PATH)
        })
        .collect()
}

async fn ask(env: &Env) -> common::v1::Answer {
    call(
        env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT,
            "Hello [mock:fixture=recorded-response-format-text]",
        )),
    )
    .await
}

#[tokio::test]
async fn a_service_principal_signs_in_once_and_sends_its_token_as_a_bearer_renewed_before_it_expires()
 {
    let (env, _) = entra_env(CLIENT_SECRET, None).await;
    let listed = env.app.get(&credentials(&env), &env.owner.cookie).await;
    let key = listed.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["auth_method"] == "azure_service_principal")
        .unwrap()
        .clone();
    assert_eq!(
        (&key["account"], &key["last4"], &key["endpoint"]),
        (&json!(TENANT), &json!("4444"), &json!(ENDPOINT))
    );

    let now = time::OffsetDateTime::now_utc();
    env.app.clock.set(now);
    for _ in 0..3 {
        let r = ask(&env).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        let sent = env.mock.last().unwrap();
        let bearer = format!(
            "Bearer {}1",
            kaeso_testkit::mock_provider::AZURE_TOKEN_PREFIX
        );
        assert_eq!(sent.header("authorization"), Some(bearer.as_str()));
        assert_eq!(sent.header("api-key"), None);
    }
    let tokens = token_requests(&env);
    assert_eq!(tokens.len(), 1, "one exchange for three requests");
    assert_eq!(tokens[0].path, format!("/{TENANT}/oauth2/v2.0/token"));
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&tokens[0].body)
            .into_owned()
            .collect();
    assert_eq!(form["grant_type"], "client_credentials");
    assert_eq!(form["client_id"], CLIENT);
    assert_eq!(form["client_secret"], CLIENT_SECRET);
    assert_eq!(
        form["scope"],
        "https://cognitiveservices.azure.com/.default"
    );

    env.app.clock.advance(time::Duration::minutes(56));
    let r = ask(&env).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        token_requests(&env).len(),
        2,
        "renewed five minutes before it expires"
    );
    assert!(
        env.mock
            .last()
            .unwrap()
            .header("authorization")
            .unwrap()
            .ends_with("mock-2")
    );
}

#[tokio::test]
async fn a_refused_secret_signs_the_key_out_until_it_is_replaced() {
    let (env, id) = entra_env("refused-secret", Some("https://ai.azure.com/.default")).await;
    let r = ask(&env).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Microsoft Entra ID refused the service principal of this project's Azure key, so it is signed out. Ask an organization admin to replace it under Providers."
    );
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&token_requests(&env)[0].body)
            .into_owned()
            .collect();
    assert_eq!(form["scope"], "https://ai.azure.com/.default");
    let shown = env
        .app
        .get(&format!("{}/{id}", credentials(&env)), &env.owner.cookie)
        .await;
    assert_eq!(shown.body["auth_error"], "signed_out", "{:?}", shown.body);

    let before = token_requests(&env).len();
    let r = ask(&env).await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert!(r.text().contains("so it is signed out"), "{}", r.text());
    assert_eq!(
        token_requests(&env).len(),
        before,
        "a signed-out key asks for no token"
    );

    let r = env
        .app
        .post(
            &format!("{}/{id}/replace", credentials(&env)),
            json!({ "secret": principal(CLIENT_SECRET, None) }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["auth_error"], Value::Null);
    assert_eq!(r.body["endpoint"], ENDPOINT);
    let r = ask(&env).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

#[tokio::test]
async fn a_resource_refusing_the_token_names_the_role_and_bad_principals_are_refused_when_added() {
    let (env, _) = entra_env(CLIENT_SECRET, None).await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT,
            &format!("Hello {}", error("recorded-entra-token")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Azure refused this project's Microsoft Entra ID service principal. It needs the Cognitive Services OpenAI User role on the resource; ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT,
            "Hello [mock:fixture=recorded-response-format-text]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    for (body, message) in [
        (
            json!({ "provider": "azure", "name": "x", "auth_method": "azure_service_principal", "secret": "nope", "endpoint": ENDPOINT }),
            "Give the service principal as a JSON object.",
        ),
        (
            json!({ "provider": "azure", "name": "x", "auth_method": "azure_service_principal", "secret": json!({ "tenant_id": TENANT, "client_id": CLIENT }).to_string(), "endpoint": ENDPOINT }),
            "The service principal has no `client_secret`.",
        ),
        (
            json!({ "provider": "azure", "name": "x", "auth_method": "azure_service_principal", "secret": principal("s", Some("https://graph.microsoft.com/.default")), "endpoint": ENDPOINT }),
            "The service principal's `scope` isn't valid.",
        ),
        (
            json!({ "provider": "openai", "name": "x", "auth_method": "azure_service_principal", "secret": principal("s", None) }),
            "OpenAI takes no Microsoft Entra ID service principal; add an API key.",
        ),
        (
            json!({ "provider": "azure", "name": "x", "auth_method": "azure_service_principal", "secret": principal("s", None) }),
            "Azure keys belong to a resource; give its `endpoint`.",
        ),
    ] {
        let r = add_key(&env, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{message}: {:?}", r.body);
        assert_eq!(r.body["error"]["message"], message);
    }
    let providers = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let azure = providers.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "azure")
        .unwrap();
    assert_eq!(azure["entra_id"]["role"], "Cognitive Services OpenAI User");
    assert_eq!(
        azure["entra_id"]["scopes"],
        json!([
            "https://cognitiveservices.azure.com/.default",
            "https://ai.azure.com/.default"
        ])
    );
}
