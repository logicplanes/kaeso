#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::{Method, StatusCode};
use common::v1::{self, Capture, Env, call, call_app, chat, messages};
use common::{App, Owner, PASSWORD};
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = (
    "KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS",
    "127.0.0.0/8,::1/128",
);
const LAB_SECRET: &str = "lab-test-key-0000000001";

async fn add_provider(app: &App, owner: &Owner, body: Value) -> common::Resp {
    app.post(
        &format!("/orgs/{}/custom-providers", owner.org_id),
        body,
        Some(&owner.cookie),
    )
    .await
}

async fn provider(
    app: &App,
    owner: &Owner,
    id: &str,
    base_url: &str,
    generation: &str,
    auth: &str,
) {
    let r = add_provider(
        app,
        owner,
        json!({ "id": id, "name": format!("Lab {id}"), "base_url": base_url, "auth": auth, "generation": generation }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn key(app: &App, owner: &Owner, provider: &str, secret: &str) {
    let r = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            json!({ "provider": provider, "name": provider, "secret": secret }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

fn custom_model(provider: &str, model: &str, dialect: &str) -> Value {
    json!({
        "provider": provider,
        "model_id": model,
        "custom": true,
        "overrides": {
            "input_modalities": ["text"],
            "output_modalities": ["text"],
            "context_limit": 32768,
            "output_limit": 4096,
            "input_price": "0.10",
            "output_price": "0.20",
            "egress_dialect": dialect,
        },
    })
}

async fn add_model(app: &App, owner: &Owner, project: &str, body: Value) -> common::Resp {
    app.post(
        &format!("/orgs/{}/projects/{project}/models", owner.org_id),
        body,
        Some(&owner.cookie),
    )
    .await
}

async fn audit_actions(app: &App, owner: &Owner) -> Vec<Value> {
    app.get(&format!("/orgs/{}/audit-log", owner.org_id), &owner.cookie)
        .await
        .body["data"]
        .as_array()
        .unwrap()
        .clone()
}

async fn patch_if_match(
    app: &App,
    path: &str,
    body: Value,
    cookie: &str,
    tag: &str,
) -> common::Resp {
    let req = axum::http::Request::builder()
        .method(Method::PATCH)
        .uri(format!("/platform/v1{path}"))
        .header(
            axum::http::header::COOKIE,
            format!("__Host-kaeso_session={cookie}"),
        )
        .header(axum::http::header::ORIGIN, common::ORIGIN)
        .header(axum::http::header::IF_MATCH, tag)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    app.raw(req).await
}

async fn env() -> Env {
    v1::env(Some(Arc::new(Capture::default())), Some(&[LOOPBACK])).await
}

async fn serve(response: String) -> (String, Arc<AtomicUsize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            let mut buf = vec![0u8; 1 << 16];
            let _ = socket.read(&mut buf).await;
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (format!("http://{addr}"), hits)
}

fn http_json(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn error_message(answer: &v1::Answer) -> String {
    let body = answer.json();
    body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn admins_add_change_and_delete_custom_providers_with_their_own_rights_and_every_change_audited()
 {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let base = format!("/orgs/{org}/custom-providers");

    let created = add_provider(
        &app,
        &owner,
        json!({
            "id": "acme-vllm",
            "name": "  Acme vLLM ",
            "base_url": "http://127.0.0.1:9/serving",
            "auth": "bearer",
            "generation": "openai_chat",
            "embeddings": "openai_embeddings",
        }),
    )
    .await
    .expect_status(StatusCode::CREATED);
    assert_eq!(created.headers["etag"], "\"1\"");
    assert_eq!(
        created.body,
        json!({
            "id": "acme-vllm",
            "name": "Acme vLLM",
            "base_url": "http://127.0.0.1:9/serving",
            "auth": "bearer",
            "generation": "openai_chat",
            "embeddings": "openai_embeddings",
            "count_tokens": null,
            "hides_built_in": null,
            "in_use": { "keys": 0, "models": 0, "projects": 0 },
            "created_by": created.body["created_by"],
            "created_at": created.body["created_at"],
            "updated_at": created.body["updated_at"],
            "revision": 1,
        })
    );
    assert_eq!(created.body["created_by"]["id"], owner.user_id);

    let listed = app
        .get(&base, &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    assert_eq!(listed.body["data"][0]["id"], "acme-vllm");
    let providers = app
        .get(&format!("/orgs/{org}/providers"), &owner.cookie)
        .await;
    let shown = providers.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "acme-vllm")
        .unwrap()
        .clone();
    assert_eq!(
        shown,
        json!({
            "id": "acme-vllm",
            "name": "Acme vLLM",
            "custom": true,
            "takes_key": true,
            "hides_built_in": null,
            "key_page": null,
            "key_prefix": null,
            "dialects": ["openai_chat", "openai_embeddings"],
            "count_tokens": null,
            "embeddings": "openai_embeddings",
            "account": null,
        })
    );
    assert!(
        !providers.body.to_string().contains("127.0.0.1"),
        "the list never shows addresses"
    );

    let stale = patch_if_match(
        &app,
        &format!("{base}/acme-vllm"),
        json!({ "name": "Renamed" }),
        &owner.cookie,
        "\"7\"",
    )
    .await;
    assert_eq!(
        stale.status,
        StatusCode::PRECONDITION_FAILED,
        "{:?}",
        stale.body
    );
    let changed = patch_if_match(
        &app,
        &format!("{base}/acme-vllm"),
        json!({ "name": "Acme GPUs", "embeddings": null, "generation": "anthropic_messages", "count_tokens": "anthropic_messages", "auth": "x_api_key" }),
        &owner.cookie,
        "\"1\"",
    )
    .await
    .expect_status(StatusCode::OK);
    assert_eq!(changed.body["name"], "Acme GPUs");
    assert_eq!(changed.body["embeddings"], Value::Null);
    assert_eq!(changed.body["count_tokens"], "anthropic_messages");
    assert_eq!(changed.body["revision"], 2);
    assert_eq!(changed.headers["etag"], "\"2\"");

    let (dev, _) = common::join(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "preset": "developer" }),
        "Dev",
    )
    .await;
    let (member, _) = common::join(
        &app,
        &owner,
        json!({ "email": "lin@example.com", "role": "member" }),
        "Lin",
    )
    .await;
    assert_eq!(app.get(&base, &dev).await.status, StatusCode::OK);
    let refused = app
        .post(&base, json!({ "id": "dev-own", "name": "Dev", "base_url": "https://llm.example.com", "auth": "bearer", "generation": "openai_chat" }), Some(&dev))
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.body);
    assert!(
        refused.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("providers.custom.manage")
    );
    assert_eq!(app.get(&base, &member).await.status, StatusCode::FORBIDDEN);
    assert_eq!(
        app.call(
            Method::DELETE,
            &format!("{base}/acme-vllm"),
            None,
            Some(&dev)
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );

    app.call(
        Method::DELETE,
        &format!("{base}/acme-vllm"),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    assert_eq!(
        app.get(&format!("{base}/acme-vllm"), &owner.cookie)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let actions: Vec<String> = audit_actions(&app, &owner)
        .await
        .iter()
        .map(|e| e["action"].as_str().unwrap().to_owned())
        .filter(|a| a.starts_with("custom_provider."))
        .collect();
    assert_eq!(
        actions,
        [
            "custom_provider.delete",
            "custom_provider.update",
            "custom_provider.create"
        ]
    );
    let events = audit_actions(&app, &owner).await;
    let update = events
        .iter()
        .find(|e| e["action"] == "custom_provider.update")
        .unwrap();
    assert_eq!(update["target_kind"], "custom_provider");
    assert_eq!(update["target_id"], "acme-vllm");
    assert_eq!(update["before"]["name"], "Acme vLLM");
    assert_eq!(update["after"]["name"], "Acme GPUs");
    s.cleanup().await;
}

#[tokio::test]
async fn ids_and_addresses_are_checked_before_a_custom_provider_is_added() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let ok = json!({ "id": "lab", "name": "Lab", "base_url": "https://llm.example.com", "auth": "bearer", "generation": "openai_chat" });
    for (change, param, message) in [
        (
            json!({ "id": "groq" }),
            "id",
            "`groq` is a built-in provider; choose another id.",
        ),
        (
            json!({ "id": "lmstudio" }),
            "id",
            "`lmstudio` is a provider listed on models.dev that Kaeso may add; choose another id.",
        ),
        (
            json!({ "id": "My_Lab" }),
            "id",
            "`id` must be lowercase letters and digits with single hyphens between them.",
        ),
        (
            json!({ "id": "a".repeat(41) }),
            "id",
            "`id` can have at most 40 characters.",
        ),
        (
            json!({ "name": " " }),
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        ),
        (
            json!({ "base_url": "ftp://llm.example.com" }),
            "base_url",
            "`base_url` must start with `https://`, or `http://` for an allowed network.",
        ),
        (
            json!({ "base_url": "https://user:secret@llm.example.com" }),
            "base_url",
            "`base_url` can't hold a user name or password; add a provider key instead.",
        ),
        (
            json!({ "base_url": "https://llm.example.com/v1?key=1" }),
            "base_url",
            "`base_url` can't have a query or a fragment.",
        ),
        (
            json!({ "base_url": "http://1.1.1.1" }),
            "base_url",
            "`base_url` uses `http` for a public address; use `https`.",
        ),
        (
            json!({ "base_url": "https://169.254.169.254/latest" }),
            "base_url",
            "`base_url` points to an address this install does not allow.",
        ),
        (
            json!({ "base_url": "http://[fd00:ec2::254]" }),
            "base_url",
            "`base_url` points to an address this install does not allow.",
        ),
        (
            json!({ "count_tokens": "openai_responses" }),
            "count_tokens",
            "`count_tokens` must be `null` or the `generation` format, which must count tokens.",
        ),
        (
            json!({ "generation": "openai_embeddings" }),
            "generation",
            "`generation` has a value of the wrong type or form.",
        ),
    ] {
        let mut body = ok.clone();
        for (k, v) in change.as_object().unwrap() {
            body[k] = v.clone();
        }
        let r = add_provider(&app, &owner, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{change}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{change}");
        assert_eq!(r.body["error"]["message"], message, "{change}");
    }
    add_provider(&app, &owner, ok.clone())
        .await
        .expect_status(StatusCode::CREATED);
    let twice = add_provider(&app, &owner, ok).await;
    assert_eq!(twice.status, StatusCode::CONFLICT);
    assert_eq!(
        twice.body["error"]["message"],
        "This organization already has a custom provider `lab`."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_name_that_resolves_to_a_private_address_is_refused_unless_the_instance_allows_it() {
    let s = TestServices::start().await;
    let closed = App::start(&s).await;
    let owner = closed.setup().await;
    let mock = MockProvider::start().await;
    let port = mock.base_url.rsplit(':').next().unwrap().to_owned();
    for base_url in [
        format!("https://localhost:{port}"),
        format!("http://localhost:{port}"),
        mock.base_url.clone(),
        "https://10.0.0.8".to_owned(),
    ] {
        let r = add_provider(
            &closed,
            &owner,
            json!({ "id": "lab", "name": "Lab", "base_url": base_url, "auth": "bearer", "generation": "openai_chat" }),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::BAD_REQUEST,
            "{base_url}: {:?}",
            r.body
        );
        assert_eq!(
            r.body["error"]["message"],
            "`base_url` points to an address this install does not allow."
        );
    }
    let everything = App::start_with(
        &s,
        &[("KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS", "0.0.0.0/0,::/0")],
    )
    .await;
    let r = add_provider(
        &everything,
        &owner,
        json!({ "id": "metadata", "name": "Metadata", "base_url": "http://169.254.169.254", "auth": "none", "generation": "openai_chat" }),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "cloud metadata stays refused: {:?}",
        r.body
    );
    assert!(mock.received().is_empty());
    s.cleanup().await;
}

#[tokio::test]
async fn every_connection_is_checked_so_an_address_allowed_once_is_refused_once_the_instance_stops_allowing_it()
 {
    let env = env().await;
    let named = env.mock.base_url.replace("127.0.0.1", "localhost");
    provider(&env.app, &env.owner, "lab", &named, "openai_chat", "bearer").await;
    key(&env.app, &env.owner, "lab", LAB_SECRET).await;
    add_model(
        &env.app,
        &env.owner,
        &env.project,
        custom_model("lab", "llama", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("lab/llama", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reached = env.mock.received().len();

    let strict = env.app.second_instance(&env.services).await;
    let r = call_app(
        &strict,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("lab/llama", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        error_message(&r),
        "The custom provider `lab` is at an address this install does not allow; ask its operator."
    );
    assert_eq!(
        env.mock.received().len(),
        reached,
        "nothing reached the provider"
    );
    assert_eq!(r.header("kaeso-provider"), Some("lab"));
}

#[tokio::test]
async fn a_custom_providers_redirect_is_never_followed() {
    let env = env().await;
    let target = format!("{}/v1/chat/completions", env.mock.base_url);
    let (redirecting, hits) = serve(format!(
        "HTTP/1.1 307 Temporary Redirect\r\nlocation: {target}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    ))
    .await;
    provider(
        &env.app,
        &env.owner,
        "hop",
        &redirecting,
        "openai_chat",
        "bearer",
    )
    .await;
    key(&env.app, &env.owner, "hop", LAB_SECRET).await;
    add_model(
        &env.app,
        &env.owner,
        &env.project,
        custom_model("hop", "llama", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);
    env.mock.clear();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("hop/llama", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.header("location"), None);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(
        env.mock.received().is_empty(),
        "the redirect's target was never asked"
    );
}

#[tokio::test]
async fn a_request_reaches_a_custom_provider_in_each_generation_format_and_for_embeddings_and_counts()
 {
    let env = env().await;
    let (app, owner) = (&env.app, &env.owner);
    let base = env.mock.base_url.clone();
    for (id, generation, auth) in [
        ("lab-chat", "openai_chat", "bearer"),
        ("lab-responses", "openai_responses", "bearer"),
        ("lab-messages", "anthropic_messages", "x_api_key"),
    ] {
        let mut body = json!({ "id": id, "name": id, "base_url": base, "auth": auth, "generation": generation });
        if id == "lab-chat" {
            body["embeddings"] = json!("openai_embeddings");
        }
        if id == "lab-messages" {
            body["count_tokens"] = json!("anthropic_messages");
        }
        add_provider(app, owner, body)
            .await
            .expect_status(StatusCode::CREATED);
        key(app, owner, id, &format!("{LAB_SECRET}-{id}")).await;
        add_model(
            app,
            owner,
            &env.project,
            custom_model(id, "house-model", generation),
        )
        .await
        .expect_status(StatusCode::CREATED);
    }
    add_model(
        app,
        owner,
        &env.project,
        custom_model("lab-chat", "house-embed", "openai_embeddings"),
    )
    .await
    .expect_status(StatusCode::CREATED);

    for (model, path, anthropic, sent_to, header, secret) in [
        (
            "lab-chat/house-model",
            "/v1/chat/completions",
            false,
            "/v1/chat/completions",
            "authorization",
            "Bearer lab-test-key-0000000001-lab-chat",
        ),
        (
            "lab-chat/house-model",
            "/v1/messages",
            true,
            "/v1/chat/completions",
            "authorization",
            "Bearer lab-test-key-0000000001-lab-chat",
        ),
        (
            "lab-responses/house-model",
            "/v1/responses",
            false,
            "/v1/responses",
            "authorization",
            "Bearer lab-test-key-0000000001-lab-responses",
        ),
        (
            "lab-messages/house-model",
            "/v1/messages",
            true,
            "/v1/messages",
            "x-api-key",
            "lab-test-key-0000000001-lab-messages",
        ),
        (
            "lab-messages/house-model",
            "/v1/chat/completions",
            false,
            "/v1/messages",
            "x-api-key",
            "lab-test-key-0000000001-lab-messages",
        ),
    ] {
        env.mock.clear();
        let body = match path {
            "/v1/messages" => messages(model, "hi"),
            "/v1/responses" => json!({ "model": model, "input": "hi" }),
            _ => chat(model, "hi"),
        };
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert_eq!(r.status, StatusCode::OK, "{model} via {path}: {}", r.text());
        assert_eq!(
            r.header("kaeso-provider"),
            Some(model.split('/').next().unwrap())
        );
        let sent = env.mock.last().unwrap();
        assert_eq!(sent.path, sent_to, "{model} via {path}");
        assert_eq!(sent.header(header), Some(secret), "{model} via {path}");
        assert_eq!(sent.json()["model"], "house-model");
    }

    env.mock.clear();
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "lab-chat/house-embed", "input": "hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/embeddings");

    let r = call(&env, "/v1/messages/count_tokens", Some(&env.key), true, Some(json!({ "model": "lab-messages/house-model", "messages": [{ "role": "user", "content": "hi" }] }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/messages/count_tokens");
    let r = call(&env, "/v1/messages/count_tokens", Some(&env.key), true, Some(json!({ "model": "lab-chat/house-model", "messages": [{ "role": "user", "content": "hi" }] }))).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        error_message(&r),
        "The model `house-model` cannot count tokens."
    );

    let outcomes = env.outcomes().await;
    let logged: Vec<&str> = outcomes
        .iter()
        .filter_map(|o| o.custom_provider.as_deref())
        .collect();
    assert!(
        logged.contains(&"lab-chat") && logged.contains(&"lab-messages"),
        "{logged:?}"
    );
    assert!(
        outcomes
            .iter()
            .filter(|o| o.custom_provider.is_some())
            .all(|o| o.provider == Some("custom"))
    );

    let listed = call(&env, "/v1/models", Some(&env.key), false, None).await;
    let ids: Vec<String> = listed.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&"lab-chat/house-model".to_owned()), "{ids:?}");
    assert!(
        ids.contains(&"lab-chat/house-embed".to_owned()) || ids.contains(&"house-embed".to_owned()),
        "{ids:?}"
    );
}

#[tokio::test]
async fn a_custom_provider_that_cannot_be_reached_refuses_the_key_or_answers_unreadably_says_so() {
    let env = env().await;
    let (app, owner) = (&env.app, &env.owner);
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        format!("http://{addr}")
    };
    let (garbled, _) = serve(http_json(
        "200 OK",
        r#"{"answer": "not a chat completion"}"#,
    ))
    .await;
    provider(app, owner, "gone", &closed, "openai_chat", "bearer").await;
    provider(app, owner, "garbled", &garbled, "openai_chat", "bearer").await;
    provider(
        app,
        owner,
        "open-lab",
        &env.mock.base_url,
        "openai_chat",
        "none",
    )
    .await;
    provider(
        app,
        owner,
        "picky",
        &env.mock.base_url,
        "anthropic_messages",
        "x_api_key",
    )
    .await;
    for id in ["gone", "garbled"] {
        key(app, owner, id, LAB_SECRET).await;
    }
    let refused_key = app
        .post(
            &format!("/orgs/{}/credentials", owner.org_id),
            json!({ "provider": "open-lab", "name": "x", "secret": LAB_SECRET }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(refused_key.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        refused_key.body["error"]["message"],
        "The custom provider `open-lab` takes no key; Kaeso calls it without one."
    );
    for (id, dialect) in [
        ("gone", "openai_chat"),
        ("garbled", "openai_chat"),
        ("open-lab", "openai_chat"),
        ("picky", "anthropic_messages"),
    ] {
        add_model(app, owner, &env.project, custom_model(id, "m", dialect))
            .await
            .expect_status(StatusCode::CREATED);
    }

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gone/m", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        error_message(&r),
        "Kaeso cannot reach the custom provider `gone`; check its base URL."
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("garbled/m", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["type"], "error", "the app gets its own format");
    assert_eq!(
        error_message(&r),
        "The custom provider `garbled` answered in a form `openai_chat` cannot read; check its API format."
    );

    env.mock.clear();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("open-lab/m", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        error_message(&r),
        "The custom provider `open-lab` asks for a key; an organization admin sets how it signs in under Providers."
    );
    assert_eq!(
        env.mock.last().unwrap().header("authorization"),
        None,
        "no key was sent"
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("picky/m", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        error_message(&r).starts_with("No credential for the provider `picky`"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn another_organization_can_neither_see_nor_call_a_custom_provider_and_may_use_the_same_id() {
    let s = TestServices::start().await;
    let mock = MockProvider::start().await;
    let other_mock = MockProvider::start().await;
    let app = App::start_with(&s, &[LOOPBACK, ("KAESO_INSTANCE_ORG_CREATION", "everyone")]).await;
    let owner = app.setup().await;
    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    provider(
        &app,
        &owner,
        "acme-vllm",
        &mock.base_url,
        "openai_chat",
        "bearer",
    )
    .await;
    key(&app, &owner, "acme-vllm", LAB_SECRET).await;
    add_model(
        &app,
        &owner,
        &project,
        custom_model("acme-vllm", "llama", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);

    app.post(
        "/instance/users/invites",
        json!({ "email": "bob@example.com" }),
        Some(&owner.cookie),
    )
    .await;
    app.wait_for_mail("bob@example.com", 1).await;
    let bob = app
        .post("/auth/invites/accept", json!({ "token": app.last_token("bob@example.com"), "name": "Bob", "password": PASSWORD }), None)
        .await
        .cookie()
        .unwrap();
    let bob_org = app
        .post("/me/orgs", json!({ "name": "Bob's org" }), Some(&bob))
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bob_project = app
        .get(&format!("/orgs/{bob_org}/projects"), &bob)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bob_owner = Owner {
        cookie: bob.clone(),
        user_id: String::new(),
        org_id: bob_org.clone(),
    };

    assert_eq!(
        app.get(&format!("/orgs/{}/custom-providers", owner.org_id), &bob)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&format!("/orgs/{bob_org}/custom-providers"), &bob)
            .await
            .body["data"],
        json!([])
    );
    let providers = app.get(&format!("/orgs/{bob_org}/providers"), &bob).await;
    assert!(!providers.body.to_string().contains("acme-vllm"));
    let r = add_model(
        &app,
        &bob_owner,
        &bob_project,
        custom_model("acme-vllm", "llama", "openai_chat"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "provider");
    let r = app
        .post(
            &format!("/orgs/{bob_org}/credentials"),
            json!({ "provider": "acme-vllm", "name": "x", "secret": LAB_SECRET }),
            Some(&bob),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let bob_key = app
        .post(
            &format!("/orgs/{bob_org}/projects/{bob_project}/keys"),
            json!({ "name": "bob" }),
            Some(&bob),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = call_app(
        &app,
        "/v1/chat/completions",
        Some(&bob_key),
        false,
        Some(chat("acme-vllm/llama", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());

    provider(
        &app,
        &bob_owner,
        "acme-vllm",
        &other_mock.base_url,
        "openai_chat",
        "bearer",
    )
    .await;
    key(&app, &bob_owner, "acme-vllm", "bob-test-key-00000000001").await;
    add_model(
        &app,
        &bob_owner,
        &bob_project,
        custom_model("acme-vllm", "llama", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let owner_key = app
        .post(
            &format!("/orgs/{}/projects/{project}/keys", owner.org_id),
            json!({ "name": "owner" }),
            Some(&owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    for (k, expected, other) in [
        (&bob_key, &other_mock, &mock),
        (&owner_key, &mock, &other_mock),
    ] {
        expected.clear();
        other.clear();
        let r = call_app(
            &app,
            "/v1/chat/completions",
            Some(k),
            false,
            Some(chat("acme-vllm/llama", "hi")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(expected.received().len(), 1);
        assert!(
            other.received().is_empty(),
            "each organization reaches only its own endpoint"
        );
        let sent = expected.last().unwrap();
        let secret = if k == &bob_key {
            "Bearer bob-test-key-00000000001".to_owned()
        } else {
            format!("Bearer {LAB_SECRET}")
        };
        assert_eq!(sent.header("authorization"), Some(secret.as_str()));
    }
    s.cleanup().await;
}

#[tokio::test]
async fn deleting_a_custom_provider_removes_its_keys_and_its_models_in_every_project() {
    let env = env().await;
    let (app, owner) = (&env.app, &env.owner);
    provider(
        app,
        owner,
        "lab",
        &env.mock.base_url,
        "openai_chat",
        "bearer",
    )
    .await;
    key(app, owner, "lab", LAB_SECRET).await;
    let second = app
        .post(
            &format!("/orgs/{}/projects", owner.org_id),
            json!({ "name": "Second" }),
            Some(&owner.cookie),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut entries = Vec::new();
    for p in [&env.project, &second] {
        let added = add_model(app, owner, p, custom_model("lab", "llama", "openai_chat"))
            .await
            .expect_status(StatusCode::CREATED);
        entries.push(added.body["id"].as_str().unwrap().to_owned());
    }
    let alias = app
        .post(
            &format!("/orgs/{}/projects/{}/aliases", owner.org_id, env.project),
            json!({ "name": "house", "catalogue_model_id": entries[0] }),
            Some(&owner.cookie),
        )
        .await;
    assert!(alias.status.is_success(), "{:?}", alias.body);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("house", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let one = app
        .get(
            &format!("/orgs/{}/custom-providers/lab", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(
        one.body["in_use"],
        json!({ "keys": 1, "models": 2, "projects": 2 })
    );

    app.call(
        Method::DELETE,
        &format!("/orgs/{}/custom-providers/lab", owner.org_id),
        None,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::NO_CONTENT);
    let keys = app
        .get(
            &format!("/orgs/{}/credentials", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert!(
        !keys.body.to_string().contains("\"lab\""),
        "{:?}",
        keys.body
    );
    for p in [&env.project, &second] {
        let catalogue = app
            .get(
                &format!("/orgs/{}/projects/{p}/models", owner.org_id),
                &owner.cookie,
            )
            .await;
        assert!(!catalogue.body.to_string().contains("\"lab\""));
    }
    for model in ["house", "lab/llama"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(model, "hi")),
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{model}: {}", r.text());
    }
    let events = audit_actions(app, owner).await;
    let deleted = events
        .iter()
        .find(|e| e["action"] == "custom_provider.delete")
        .unwrap();
    assert_eq!(deleted["after"]["removed_keys"], 1);
    assert_eq!(deleted["after"]["removed_models"], 2);
    assert_eq!(
        deleted["before"]["base_url"],
        format!("{}/", env.mock.base_url)
    );
}

#[tokio::test]
async fn a_built_in_provider_a_later_release_adds_under_an_organizations_id_stays_hidden_there() {
    let env = env().await;
    let (app, owner) = (&env.app, &env.owner);
    let own = MockProvider::start().await;
    sqlx::query(
        "INSERT INTO custom_providers (id, org_id, provider, name, base_url, auth, generation, created_by_kind) \
         VALUES (gen_random_uuid(), $1, 'groq', 'Our Groq proxy', $2, 'bearer', 'openai_chat', 'system')",
    )
    .bind(common::uuid(&owner.org_id))
    .bind(&own.base_url)
    .execute(&app.state.pg)
    .await
    .unwrap();
    for p in env
        .app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"]
        .as_array()
        .unwrap()
    {
        kaeso_server::services::catalogue::forget(
            &app.state,
            common::uuid(p["id"].as_str().unwrap()),
        )
        .await;
    }

    let providers = app
        .get(&format!("/orgs/{}/providers", owner.org_id), &owner.cookie)
        .await;
    let groq: Vec<&Value> = providers.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"] == "groq")
        .collect();
    assert_eq!(groq.len(), 1);
    assert_eq!(groq[0]["custom"], true);
    assert_eq!(groq[0]["hides_built_in"], "Groq");
    let listed = app
        .get(
            &format!("/orgs/{}/custom-providers", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(listed.body["data"][0]["hides_built_in"], "Groq");

    key(app, owner, "groq", LAB_SECRET).await;
    add_model(
        app,
        owner,
        &env.project,
        custom_model("groq", "llama-3.3-70b-versatile", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);
    env.mock.clear();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("groq/llama-3.3-70b-versatile", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        own.received().len(),
        1,
        "the organization's own endpoint answers"
    );
    assert!(
        env.mock.received().is_empty(),
        "the built-in Groq never sees the key"
    );

    let blocked = add_provider(app, owner, json!({ "id": "groq", "name": "x", "base_url": "https://x.example.com", "auth": "bearer", "generation": "openai_chat" })).await;
    assert_eq!(blocked.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_install_that_turns_custom_providers_off_refuses_them_without_losing_them() {
    let env = env().await;
    provider(
        &env.app,
        &env.owner,
        "lab",
        &env.mock.base_url,
        "openai_chat",
        "bearer",
    )
    .await;
    key(&env.app, &env.owner, "lab", LAB_SECRET).await;
    add_model(
        &env.app,
        &env.owner,
        &env.project,
        custom_model("lab", "llama", "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);

    let off = App::start_with(
        &env.services,
        &[("KAESO_CUSTOM_PROVIDERS_ENABLED", "false")],
    )
    .await;
    let r = add_provider(&off, &env.owner, json!({ "id": "other", "name": "x", "base_url": "https://x.example.com", "auth": "bearer", "generation": "openai_chat" })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "Custom providers are turned off on this install; ask its operator."
    );
    let providers = off
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert!(!providers.body.to_string().contains("\"lab\""));
    assert_eq!(
        off.get(
            &format!("/orgs/{}/custom-providers", env.owner.org_id),
            &env.owner.cookie
        )
        .await
        .body["data"][0]["id"],
        "lab"
    );
    let catalogue = off
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(catalogue.status, StatusCode::OK, "{:?}", catalogue.body);
    env.mock.clear();
    let r = call_app(
        &off,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("lab/llama", "hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR, "{}", r.text());
    assert_eq!(
        error_message(&r),
        "Custom providers are turned off on this install, so `lab` cannot be called."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_custom_providers_model_listing_offers_model_ids_to_add() {
    let env = env().await;
    provider(
        &env.app,
        &env.owner,
        "lab",
        &env.mock.base_url,
        "openai_chat",
        "bearer",
    )
    .await;
    let base = format!(
        "/orgs/{}/projects/{}/models/offered",
        env.owner.org_id, env.project
    );
    let r = env
        .app
        .get(&format!("{base}?provider=lab"), &env.owner.cookie)
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "No provider key for Lab lab is set up for this project; an org admin can add one."
    );
    key(&env.app, &env.owner, "lab", LAB_SECRET).await;
    let r = env
        .app
        .get(&format!("{base}?provider=lab"), &env.owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    let ids: Vec<&str> = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["model_id"].as_str().unwrap())
        .collect();
    assert!(!ids.is_empty() && ids.is_sorted(), "{ids:?}");
    assert_eq!(env.mock.last().unwrap().path, "/v1/models");
    let first = ids[0].to_owned();
    add_model(
        &env.app,
        &env.owner,
        &env.project,
        custom_model("lab", &first, "openai_chat"),
    )
    .await
    .expect_status(StatusCode::CREATED);
    let again = env
        .app
        .get(&format!("{base}?provider=lab"), &env.owner.cookie)
        .await;
    assert!(again.body["data"][0]["catalogue_model_id"].is_string());
    let builtin = env
        .app
        .get(&format!("{base}?provider=openai"), &env.owner.cookie)
        .await;
    assert_eq!(builtin.status, StatusCode::BAD_REQUEST);

    let r = add_model(
        &env.app,
        &env.owner,
        &env.project,
        json!({ "provider": "lab", "model_id": "plain" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        "`lab` is a custom provider; send `custom: true` with the model's values."
    );
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/models/batch",
                env.owner.org_id, env.project
            ),
            json!({ "provider": "lab", "all_current": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}
