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
