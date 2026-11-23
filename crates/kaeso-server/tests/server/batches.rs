#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

async fn send(
    env: &Env,
    key: &str,
    method: &str,
    path: &str,
    content_type: Option<String>,
    body: Vec<u8>,
) -> Answer {
    let mut req = Request::builder()
        .uri(path)
        .method(method)
        .header("authorization", format!("Bearer {key}"));
    if let Some(ct) = content_type {
        req = req.header("content-type", ct);
    }
    let resp = env
        .app
        .router
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
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

fn line(id: &str, url: &str, model: &str, text: &str) -> String {
    let body = if url == "/v1/embeddings" {
        json!({ "model": model, "input": text })
    } else {
        json!({ "model": model, "messages": [{ "role": "user", "content": text }], "max_tokens": 50 })
    };
    json!({ "custom_id": id, "method": "POST", "url": url, "body": body }).to_string()
}

async fn upload(env: &Env, key: &str, lines: &[String]) -> Answer {
    let file = lines.join("\n") + "\n";
    let form = kaeso_codecs::multipart::FormWriter::write(&[
        kaeso_codecs::multipart::FormField::Text("purpose", "batch"),
        kaeso_codecs::multipart::FormField::File(
            "file",
            "input.jsonl",
            "application/jsonl",
            file.as_bytes(),
        ),
    ]);
    let ct = form.content_type();
    send(env, key, "POST", "/v1/files", Some(ct), form.into_body()).await
}

async fn create(env: &Env, key: &str, file: &str) -> Answer {
    let body = json!({ "input_file_id": file, "endpoint": "/v1/chat/completions", "completion_window": "24h", "metadata": { "job": "nightly" } });
    send(
        env,
        key,
        "POST",
        "/v1/batches",
        Some("application/json".into()),
        body.to_string().into_bytes(),
    )
    .await
}

#[tokio::test]
async fn batches_respect_hard_budgets_and_unpriced_features() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let files = Arc::new(AtomicUsize::new(0));
    let batches = Arc::new(AtomicUsize::new(0));
    let files_post = Arc::clone(&files);
    let batches_post = Arc::clone(&batches);
    let out_line = |completion_tokens: u64| {
        format!(
            "{}\n",
            json!({
                "id": "batch_req_1",
                "custom_id": "fit-1",
                "response": {
                    "status_code": 200,
                    "request_id": "req_1",
                    "body": {
                        "id": "chatcmpl-1",
                        "object": "chat.completion",
                        "created": 1_700_000_000,
                        "model": "gpt-4o",
                        "choices": [{
                            "index": 0,
                            "message": { "role": "assistant", "content": "blue" },
                            "finish_reason": "stop"
                        }],
                        "usage": {
                            "prompt_tokens": 10,
                            "completion_tokens": completion_tokens,
                            "total_tokens": 10 + completion_tokens
                        }
                    }
                },
                "error": null
            })
        )
    };
    let out_one = out_line(18);
    let out_two = out_line(100);
    let upstream = axum::Router::new()
        .route(
            "/v1/files",
            axum::routing::post(move || {
                let n = files_post.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    axum::Json(json!({
                        "id": format!("file-up-{n}"),
                        "object": "file",
                        "purpose": "batch"
                    }))
                }
            }),
        )
        .route(
            "/v1/batches",
            axum::routing::post(move || {
                let n = batches_post.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    axum::Json(json!({
                        "id": format!("batch-up-{n}"),
                        "object": "batch",
                        "endpoint": "/v1/chat/completions",
                        "status": "validating",
                        "completion_window": "24h"
                    }))
                }
            }),
        )
        .route(
            "/v1/batches/batch-up-1",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "id": "batch-up-1",
                    "object": "batch",
                    "endpoint": "/v1/chat/completions",
                    "status": "completed",
                    "output_file_id": "file-out-1"
                }))
            }),
        )
        .route(
            "/v1/batches/batch-up-2",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "id": "batch-up-2",
                    "object": "batch",
                    "endpoint": "/v1/chat/completions",
                    "status": "completed",
                    "output_file_id": "file-out-2"
                }))
            }),
        )
        .route(
            "/v1/files/file-out-1/content",
            axum::routing::get(move || {
                let body = out_one.clone();
                async move { body }
            }),
        )
        .route(
            "/v1/files/file-out-2/content",
            axum::routing::get(move || {
                let body = out_two.clone();
                async move { body }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, upstream).await;
    });

    let env = common::v1::env(None, Some(&[("KAESO_PROVIDERS_OPENAI_BASE_URL", &up_url)])).await;
    let limits_path = format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project);
    for body in [
        json!({ "level": "project", "kind": "token_rate", "period": "minute", "amount": 20 }),
        json!({ "level": "project", "kind": "token_budget", "period": "day", "amount": 250, "hard": true }),
    ] {
        env.app
            .post(&limits_path, body, Some(&env.owner.cookie))
            .await
            .expect_status(StatusCode::CREATED);
    }

    let big_line = json!({
        "custom_id": "big-1",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": { "model": "gpt-4o", "messages": [{ "role": "user", "content": "Hi" }], "max_tokens": 300 }
    })
    .to_string();
    let big_file = upload(&env, &env.key, &[big_line]).await.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let over_tokens = create(&env, &env.key, &big_file).await;
    assert_eq!(
        over_tokens.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        over_tokens.text()
    );

    let fit_line = json!({
        "custom_id": "fit-1",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": { "model": "gpt-4o", "messages": [{ "role": "user", "content": "Say blue." }], "max_tokens": 140 }
    })
    .to_string();
    let fit_file = upload(&env, &env.key, &[fit_line]).await.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let nul_meta = send(
        &env,
        &env.key,
        "POST",
        "/v1/batches",
        Some("application/json".into()),
        json!({
            "input_file_id": fit_file,
            "endpoint": "/v1/chat/completions",
            "completion_window": "24h",
            "metadata": { "note": "a\u{0000}b" }
        })
        .to_string()
        .into_bytes(),
    )
    .await;
    assert_eq!(
        nul_meta.status,
        StatusCode::BAD_REQUEST,
        "{}",
        nul_meta.text()
    );

    let first = create(&env, &env.key, &fit_file).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let first_id = first.json()["id"].as_str().unwrap().to_owned();

    let held_busy = create(&env, &env.key, &fit_file).await;
    assert_eq!(
        held_busy.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        held_busy.text()
    );

    let settled = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{first_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(settled.status, StatusCode::OK, "{}", settled.text());
    assert_eq!(settled.json()["status"], "completed");

    let second = create(&env, &env.key, &fit_file).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    let second_id = second.json()["id"].as_str().unwrap().to_owned();
    let second_settled = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{second_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(second_settled.status, StatusCode::OK);

    let after_settled_usage = create(&env, &env.key, &fit_file).await;
    assert_eq!(
        after_settled_usage.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        after_settled_usage.text()
    );

    env.app
        .post(
            &limits_path,
            json!({ "level": "project", "kind": "spend_budget", "period": "day", "amount": "10", "hard": true }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);

    let priority_line = json!({
        "custom_id": "prio-1",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": {
            "model": "gpt-4o",
            "messages": [{ "role": "user", "content": "Hi" }],
            "max_tokens": 20,
            "service_tier": "priority"
        }
    })
    .to_string();
    let priority_file = upload(&env, &env.key, &[priority_line]).await.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let unpriced = create(&env, &env.key, &priority_file).await;
    assert_eq!(
        unpriced.status,
        StatusCode::FORBIDDEN,
        "{}",
        unpriced.text()
    );
    assert!(
        unpriced.text().contains("service_tier: priority"),
        "{}",
        unpriced.text()
    );

    let small_line = json!({
        "custom_id": "small-1",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": { "model": "gpt-4o", "messages": [{ "role": "user", "content": "Hi" }], "max_tokens": 20 }
    })
    .to_string();
    let small_file = upload(&env, &env.key, &[small_line]).await.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let key_resp = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
            json!({ "name": "tight-spend" }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let tight_key = key_resp.body["secret"].as_str().unwrap().to_owned();
    let tight_id = key_resp.body["id"].as_str().unwrap().to_owned();
    env.app
        .post(
            &limits_path,
            json!({
                "level": "key",
                "key": tight_id,
                "kind": "spend_budget",
                "period": "day",
                "amount": "0.00001",
                "hard": true
            }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    let over_spend = create(&env, &tight_key, &small_file).await;
    assert_eq!(
        over_spend.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        over_spend.text()
    );
}

#[tokio::test]
async fn a_batch_runs_at_openai_and_its_lines_are_charged_at_the_batch_price() {
    let env = env().await;
    let lines = [
        line(
            "request-1",
            "/v1/chat/completions",
            "openai/gpt-4o",
            "Say blue.",
        ),
        line(
            "request-2",
            "/v1/chat/completions",
            "openai/gpt-4o",
            "Say green. [mock:batch-fail]",
        ),
    ];
    let r = upload(&env, &env.key, &lines).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let file = r.json();
    assert!(file["id"].as_str().unwrap().starts_with("file_"), "{file}");
    assert_eq!(
        (&file["purpose"], &file["object"]),
        (&json!("batch"), &json!("file"))
    );
    let uploaded = env.mock.last().unwrap();
    assert_eq!(uploaded.path, "/v1/files");
    assert!(
        String::from_utf8_lossy(&uploaded.body).contains("\"model\":\"gpt-4o\""),
        "the provider's model id goes to OpenAI"
    );
    assert!(!String::from_utf8_lossy(&uploaded.body).contains("openai/gpt-4o"));
    let file_id = file["id"].as_str().unwrap().to_owned();

    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/files/{file_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.json()["id"], file_id);

    let r = create(&env, &env.key, &file_id).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let batch = r.json();
    assert_eq!(batch["status"], "validating");
    assert_eq!(batch["input_file_id"], file_id);
    assert!(batch["id"].as_str().unwrap().starts_with("batch_"));
    let sent = env.mock.last().unwrap().json();
    assert_eq!(
        sent["input_file_id"], "file-mockin1",
        "the provider's own file id"
    );
    assert_eq!(sent["metadata"]["job"], "nightly");
    let batch_id = batch["id"].as_str().unwrap().to_owned();

    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{batch_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let done = r.json();
    assert_eq!(done["status"], "completed");
    assert_eq!(
        done["request_counts"],
        json!({ "total": 2, "completed": 1, "failed": 1 })
    );
    let output = done["output_file_id"].as_str().unwrap().to_owned();
    assert!(output.starts_with("file_"), "{done}");

    let lines: Vec<_> = env
        .outcomes()
        .await
        .into_iter()
        .filter(|o| {
            o.model_id.as_deref() == Some("gpt-4o")
                && o.upstream_request_id
                    .as_deref()
                    .is_some_and(|i| i.starts_with("request-"))
        })
        .collect();
    assert_eq!(lines.len(), 2, "one recorded request per line");
    let ok = lines.iter().find(|o| o.status == 200).unwrap();
    assert_eq!(
        ok.usage.as_ref().map(|u| (u.input_tokens, u.output_tokens)),
        Some((19, 10))
    );
    let prices = ok.prices.as_ref().unwrap();
    assert_eq!(
        (prices.input_price, prices.output_price),
        (Some("1.25".parse().unwrap()), Some("5".parse().unwrap())),
        "half of gpt-4o's price"
    );
    assert!(lines.iter().any(|o| o.status == 400 && o.usage.is_none()));

    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{batch_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let again = env
        .outcomes()
        .await
        .into_iter()
        .filter(|o| {
            o.upstream_request_id
                .as_deref()
                .is_some_and(|i| i.starts_with("request-"))
        })
        .count();
    assert_eq!(again, 2, "a batch is charged once");

    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/files/{output}/content"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(
        text.contains("\"custom_id\":\"request-1\"")
            && text.contains("\"custom_id\":\"request-2\""),
        "{text}"
    );

    let r = send(&env, &env.key, "GET", "/v1/batches?limit=10", None, vec![]).await;
    assert_eq!(r.json()["data"].as_array().unwrap().len(), 1);
    assert_eq!(r.json()["data"][0]["id"], batch_id);
    assert_eq!(r.json()["has_more"], false);

    let r = send(
        &env,
        &env.key,
        "DELETE",
        &format!("/v1/files/{file_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(
        r.json(),
        json!({ "id": file_id, "object": "file", "deleted": true })
    );
    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/files/{file_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_uploads_hold_the_translation_budget_and_check_line_depth() {
    let env = common::v1::env(None, Some(&[("KAESO_BODY_LIMITS_IN_FLIGHT", "64MiB")])).await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let make_key = |name: &'static str| {
        let app = &env.app;
        let path = format!("{base}/keys");
        let cookie = env.owner.cookie.clone();
        async move {
            let r = app
                .post(&path, json!({ "name": name }), Some(&cookie))
                .await
                .expect_status(StatusCode::CREATED);
            let secret = r.body["secret"].as_str().unwrap().to_owned();
            let id: kaeso_core::id::ApiKeyId = r.body["id"].as_str().unwrap().parse().unwrap();
            (secret, id.uuid())
        }
    };
    let (greedy, greedy_id) = make_key("greedy-batch").await;
    let (other, _) = make_key("other-batch").await;

    let deep = format!("{}1{}", "[".repeat(150), "]".repeat(150));
    let deep_line = format!(
        r#"{{"custom_id":"deep-1","method":"POST","url":"/v1/chat/completions","body":{{"model":"openai/gpt-4o","messages":[{{"role":"user","content":"Hi"}}],"extra":{deep}}}}}"#
    );
    let deep_res = upload(&env, &other, &[deep_line]).await;
    assert_eq!(
        deep_res.status,
        StatusCode::BAD_REQUEST,
        "{}",
        deep_res.text()
    );
    assert!(
        deep_res.text().contains("more than 127 levels deep"),
        "{}",
        deep_res.text()
    );

    let budget = &env.app.state.translating;
    let share = budget.hold(greedy_id, u64::MAX).await.unwrap();
    let one_line = [line(
        "req-1",
        "/v1/chat/completions",
        "openai/gpt-4o",
        "Say blue.",
    )];
    let refused = upload(&env, &greedy, &one_line);
    let served = async {
        common::eventually("the greedy key's batch upload waits", || async {
            budget.waiting() == 1
        })
        .await;
        upload(&env, &other, &one_line).await
    };
    let (refused, served) = tokio::join!(refused, served);
    assert_eq!(served.status, StatusCode::OK, "{}", served.text());
    assert_eq!(
        refused.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        refused.text()
    );
    assert_eq!(refused.header("retry-after"), Some("10"));
    assert_eq!(
        refused.json()["error"]["message"],
        "Kaeso is busy translating other requests; try again shortly."
    );
    drop(share);
    assert_eq!(budget.free(), 64 << 20);
}

#[tokio::test]
async fn a_file_is_refused_by_the_line_that_breaks_the_rules() {
    let env = env().await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let models = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let gpt = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "gpt-4o")
        .unwrap()["id"]
        .clone();
    let limited = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "one model", "models": [gpt] }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let chat = |id: &str, model: &str| line(id, "/v1/chat/completions", model, "Hi");
    for (key, lines, message) in [
        (
            &limited,
            vec![chat("a", "claude-haiku-4-5")],
            "Line 1 of the file: no model `claude-haiku-4-5` is available to this key.",
        ),
        (
            &env.key,
            vec![chat("a", "gpt-4o"), chat("b", "claude-haiku-4-5")],
            "Line 2 of the file: it names `claude-haiku-4-5`, but the file's first line names `gpt-4o`; a batch runs one model.",
        ),
        (
            &env.key,
            vec![chat("a", "gpt-4o"), chat("a", "gpt-4o")],
            "Line 2 of the file: `custom_id` `a` is used before.",
        ),
        (
            &env.key,
            vec![
                chat("a", "gpt-4o"),
                line("b", "/v1/embeddings", "gpt-4o", "Hi"),
            ],
            "Line 2 of the file: it asks for `/v1/embeddings`, but the file's first line asks for `/v1/chat/completions`; a batch runs one endpoint.",
        ),
        (
            &env.key,
            vec![chat("a", "claude-haiku-4-5")],
            "Line 1 of the file: Anthropic runs no batches Kaeso can send.",
        ),
        (
            &env.key,
            vec![line("a", "/v1/embeddings", "gpt-4o", "Hi")],
            "Line 1 of the file: `gpt-4o` isn't served in this endpoint's format; Kaeso doesn't translate batches yet.",
        ),
        (
            &env.key,
            vec!["{".to_owned()],
            "Line 1 of the file: it isn't a JSON object.",
        ),
        (&env.key, vec![], "The file holds no requests."),
    ] {
        let before = env.mock.received().len();
        let r = upload(&env, key, &lines).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{message}: {}", r.text());
        assert_eq!(r.json()["error"]["message"], message);
        assert_eq!(env.mock.received().len(), before, "nothing reached OpenAI");
    }
    let r = upload(&env, &limited, &[chat("a", "gpt-4o")]).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "the key's one model runs: {}",
        r.text()
    );
}

#[tokio::test]
async fn another_projects_files_and_batches_answer_404_and_a_running_batch_can_be_cancelled() {
    let env = env().await;
    let r = upload(
        &env,
        &env.key,
        &[line(
            "a",
            "/v1/chat/completions",
            "gpt-4o",
            "Hi [mock:batch-running]",
        )],
    )
    .await;
    let file = r.json()["id"].as_str().unwrap().to_owned();
    let batch = create(&env, &env.key, &file).await.json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{batch}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.json()["status"], "in_progress");
    let r = send(
        &env,
        &env.key,
        "POST",
        &format!("/v1/batches/{batch}/cancel"),
        None,
        vec![],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["status"], "cancelling");
    assert_eq!(
        env.mock.last().unwrap().path,
        "/v1/batches/batch_mock1/cancel"
    );

    let other =
        common::create_project(&env.app, &env.owner.org_id, &env.owner.cookie, "Other").await;
    let key = env
        .app
        .post(
            &format!("/orgs/{}/projects/{other}/keys", env.owner.org_id),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    for (method, path) in [
        ("GET", format!("/v1/batches/{batch}")),
        ("POST", format!("/v1/batches/{batch}/cancel")),
        ("GET", format!("/v1/files/{file}")),
        ("GET", format!("/v1/files/{file}/content")),
        ("DELETE", format!("/v1/files/{file}")),
    ] {
        let r = send(&env, &key, method, &path, None, vec![]).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            r.text()
        );
    }
    let r = create(&env, &key, &file).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["error"]["param"], "input_file_id");
    let r = send(&env, &key, "GET", "/v1/batches", None, vec![]).await;
    assert_eq!(r.json()["data"], json!([]));
    let _: Value = r.json();
}

#[tokio::test]
async fn follow_up_routes_and_jobs_leave_key_fragments_out_of_provider_errors() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let tries = Arc::new(AtomicUsize::new(0));
    let tries_post = Arc::clone(&tries);
    let upstream = axum::Router::new()
        .route(
            "/v1/files",
            axum::routing::post(|| async {
                axum::Json(json!({ "id": "file-up1", "object": "file", "purpose": "batch" }))
            }),
        )
        .route(
            "/v1/batches",
            axum::routing::post(move || {
                let n = tries_post.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        (
                            StatusCode::TOO_MANY_REQUESTS,
                            axum::Json(json!({
                                "error": {
                                    "message": "Rate limit for key sk-proj-SecretFragment999 in org-AbCdEf123456.",
                                    "type": "rate_limit_error"
                                }
                            })),
                        )
                    } else {
                        (
                            StatusCode::OK,
                            axum::Json(json!({
                                "id": "batch_up1",
                                "object": "batch",
                                "status": "failed",
                                "output_file_id": "file-out1",
                                "error_file_id": "file-err1",
                                "errors": {
                                    "object": "list",
                                    "data": [{
                                        "code": "rate_limit",
                                        "message": "Key sk-proj-SecretFragment999 in org-AbCdEf123456 hit RPM."
                                    }]
                                },
                                "organization_id": "org-AbCdEf123456"
                            })),
                        )
                    }
                }
            }),
        )
        .route(
            "/v1/batches/batch_up1",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "id": "batch_up1",
                    "object": "batch",
                    "status": "failed",
                    "output_file_id": "file-out1",
                    "error_file_id": "file-err1",
                    "errors": {
                        "object": "list",
                        "data": [{
                            "code": "rate_limit",
                            "message": "Key sk-proj-SecretFragment999 in org-AbCdEf123456 hit RPM."
                        }]
                    },
                    "organization_id": "org-AbCdEf123456"
                }))
            }),
        )
        .route(
            "/v1/files/file-err1/content",
            axum::routing::get(|| async {
                format!(
                    "{}\n",
                    json!({
                        "id": "batch_req_1",
                        "custom_id": "request-1",
                        "response": null,
                        "error": {
                            "code": "invalid_api_key",
                            "message": "Rejected sk-proj-SecretFragment999 for org-AbCdEf123456."
                        }
                    })
                )
            }),
        )
        .route(
            "/v1/files/file-out1/content",
            axum::routing::get(|| async {
                format!(
                    "{}\n{}\n{}\n",
                    json!({
                        "id": "batch_req_2",
                        "custom_id": "request-2",
                        "response": {
                            "status_code": 200,
                            "body": {
                                "id": "chatcmpl-1",
                                "object": "chat.completion",
                                "model": "gpt-4o",
                                "choices": [{
                                    "index": 0,
                                    "message": { "role": "assistant", "content": "Literal sk-proj-ExampleLiteral123456 stays as written." },
                                    "finish_reason": "stop"
                                }]
                            }
                        },
                        "error": null
                    }),
                    json!({
                        "id": "batch_req_3",
                        "custom_id": "request-3",
                        "response": {
                            "status_code": 401,
                            "body": {
                                "error": {
                                    "message": "Incorrect key sk-proj-SecretFragment999 in org-AbCdEf123456."
                                }
                            }
                        },
                        "error": null
                    }),
                    json!({
                        "id": "batch_req_4",
                        "custom_id": "request-4",
                        "response": {
                            "status_code": 200,
                            "body": {
                                "id": "resp_failed_1",
                                "object": "response",
                                "status": "failed",
                                "error": {
                                    "code": "server_error",
                                    "message": "Response failed for sk-proj-SecretFragment999 in org-AbCdEf123456."
                                }
                            }
                        },
                        "error": null
                    })
                )
            }),
        )
        .route(
            "/v1/voices",
            axum::routing::get(|| async {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    axum::Json(json!({
                        "detail": {
                            "status": "quota_exceeded",
                            "message": "Quota exceeded for sk-proj-SecretFragment999 in org-AbCdEf123456."
                        }
                    })),
                )
            }),
        )
        .route(
            "/v2/videos",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "id": "vid_up1",
                    "object": "video",
                    "status": "in_progress",
                    "progress": 10
                }))
            }),
        )
        .route(
            "/v2/videos/vid_up1",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "id": "vid_up1",
                    "object": "video",
                    "status": "failed",
                    "error": {
                        "code": "server_error",
                        "message": "Video failed for sk-proj-SecretFragment999 in org-AbCdEf123456."
                    }
                }))
            }),
        )
        .route(
            "/v1/flows/image",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "id": "gen_up1",
                    "status": "pending"
                }))
            }),
        )
        .route(
            "/v1/flows/image/gen_up1",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "id": "gen_up1",
                    "status": "failed",
                    "failure_reason": "timeout",
                    "error_message": "Image failed for sk-proj-SecretFragment999 in org-AbCdEf123456."
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, upstream).await;
    });

    let env = common::v1::env(
        None,
        Some(&[
            ("KAESO_PROVIDERS_OPENAI_BASE_URL", &base),
            ("KAESO_PROVIDERS_ELEVENLABS_BASE_URL", &base),
            ("KAESO_PROVIDERS_TOGETHERAI_BASE_URL", &base),
        ]),
    )
    .await;
    for (provider, secret) in [
        ("elevenlabs", "sk_test_elevenlabs_000000000000"),
        ("togetherai", "tgp_v1_test_togetherai_000000000000"),
    ] {
        env.app
            .post(
                &format!("/orgs/{}/credentials", env.owner.org_id),
                json!({ "provider": provider, "name": provider, "secret": secret }),
                Some(&env.owner.cookie),
            )
            .await
            .expect_status(StatusCode::CREATED);
    }
    for (provider, model) in [
        ("togetherai", "ByteDance/Seedance-1.0-lite"),
        ("elevenlabs", "bytedance-seedream-5-lite"),
    ] {
        env.app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await
            .expect_status(StatusCode::CREATED);
    }

    let file = upload(
        &env,
        &env.key,
        &[line("request-1", "/v1/chat/completions", "gpt-4o", "Hi")],
    )
    .await
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let first = create(&env, &env.key, &file).await;
    assert_eq!(
        first.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        first.text()
    );
    assert!(
        !first.text().contains("SecretFragment999"),
        "{}",
        first.text()
    );
    assert!(!first.text().contains("AbCdEf123456"), "{}", first.text());

    let second = create(&env, &env.key, &file).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert!(
        !second.text().contains("SecretFragment999"),
        "{}",
        second.text()
    );
    assert!(!second.text().contains("AbCdEf123456"), "{}", second.text());
    let second_json = second.json();
    let batch_id = second_json["id"].as_str().unwrap().to_owned();

    let fetched = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/batches/{batch_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert!(
        !fetched.text().contains("SecretFragment999"),
        "{}",
        fetched.text()
    );
    assert!(
        !fetched.text().contains("AbCdEf123456"),
        "{}",
        fetched.text()
    );
    let fetched_json = fetched.json();
    let out_file = fetched_json["output_file_id"].as_str().unwrap().to_owned();
    let err_file = fetched_json["error_file_id"].as_str().unwrap().to_owned();

    let err_content = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/files/{err_file}/content"),
        None,
        vec![],
    )
    .await;
    assert_eq!(err_content.status, StatusCode::OK);
    assert!(
        !err_content.text().contains("SecretFragment999"),
        "{}",
        err_content.text()
    );
    assert!(
        !err_content.text().contains("AbCdEf123456"),
        "{}",
        err_content.text()
    );

    let out_content = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/files/{out_file}/content"),
        None,
        vec![],
    )
    .await;
    assert_eq!(out_content.status, StatusCode::OK);
    assert!(
        !out_content.text().contains("SecretFragment999"),
        "{}",
        out_content.text()
    );
    assert!(
        !out_content.text().contains("AbCdEf123456"),
        "{}",
        out_content.text()
    );
    assert!(
        out_content.text().contains("sk-proj-ExampleLiteral123456"),
        "{}",
        out_content.text()
    );

    let voices = send(&env, &env.key, "GET", "/v1/voices", None, vec![]).await;
    assert_eq!(
        voices.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        voices.text()
    );
    assert!(
        !voices.text().contains("SecretFragment999"),
        "{}",
        voices.text()
    );
    assert!(!voices.text().contains("AbCdEf123456"), "{}", voices.text());

    let started = send(
        &env,
        &env.key,
        "POST",
        "/v1/videos",
        Some("application/json".into()),
        json!({ "model": "ByteDance/Seedance-1.0-lite", "prompt": "A calm ocean wave." })
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.text());
    let video_id = started.json()["id"].as_str().unwrap().to_owned();
    let polled = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/videos/{video_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(polled.status, StatusCode::OK, "{}", polled.text());
    assert!(
        !polled.text().contains("SecretFragment999"),
        "{}",
        polled.text()
    );
    assert!(!polled.text().contains("AbCdEf123456"), "{}", polled.text());

    let gen_started = send(
        &env,
        &env.key,
        "POST",
        "/v1/flows/image",
        Some("application/json".into()),
        json!({ "model_id": "bytedance-seedream-5-lite", "prompt": "A calm ocean wave." })
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(gen_started.status, StatusCode::OK, "{}", gen_started.text());
    let gen_id = gen_started.json()["id"].as_str().unwrap().to_owned();
    let gen_polled = send(
        &env,
        &env.key,
        "GET",
        &format!("/v1/flows/image/{gen_id}"),
        None,
        vec![],
    )
    .await;
    assert_eq!(gen_polled.status, StatusCode::OK, "{}", gen_polled.text());
    assert!(
        !gen_polled.text().contains("SecretFragment999"),
        "{}",
        gen_polled.text()
    );
    assert!(
        !gen_polled.text().contains("AbCdEf123456"),
        "{}",
        gen_polled.text()
    );
}
