use crate::common;

use axum::http::{Method, StatusCode};
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const XAI_SECRET: &str = "xai-test-xai-credential-0001";

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "xai", "name": "xai", "secret": XAI_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        json!({ "provider": "xai", "model_id": "grok-4.3" }),
        json!({
            "provider": "xai",
            "model_id": "grok-4.20-0309-non-reasoning",
            "overrides": { "egress_dialect": "openai_chat" }
        }),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                model,
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent(env: &Env) -> Option<(String, Option<String>, Value)> {
    let last = env.mock.last()?;
    Some((
        last.path.clone(),
        last.header("authorization").map(str::to_owned),
        last.json(),
    ))
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn an_openai_responses_app_reaches_xai_and_keeps_its_citations_and_usage() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": "xai/grok-4.3",
            "input": "Opening hours? [mock:fixture=providers/xai/recorded-web-search]",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("xai"));
    let (path, auth, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {XAI_SECRET}").as_str())
    );
    assert_eq!(body["model"], "grok-4.3");
    let reply = r.json();
    let message = reply["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "message")
        .unwrap();
    let citation = &message["content"][0]["annotations"][0];
    assert_eq!(citation["type"], "url_citation");
    assert_eq!(citation["url"], "https://oceanario.pt/en/");
    assert_eq!(reply["usage"]["num_server_side_tools_used"], 2);
    assert_eq!(reply["usage"]["cost_in_usd_ticks"], 232_046_000);
    assert_eq!(
        reply["usage"]["server_side_tool_usage_details"]["web_search_calls"],
        2
    );
}

#[tokio::test]
async fn an_openai_chat_app_keeps_xai_reasoning_content_on_the_chat_dialect() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "grok-4.20-0309-non-reasoning",
            "17 * 24? [mock:fixture=providers/xai/recorded-reasoning-content]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(body["model"], "grok-4.20-0309-non-reasoning");
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["content"], "408");
    assert_eq!(
        message["reasoning_content"],
        "Calculating 17 multiplied by 24."
    );
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        178
    );
    assert_eq!(
        reply["usage"]["prompt_tokens_details"]["cached_tokens"],
        128
    );
    assert_eq!(reply["usage"]["cost_in_usd_ticks"], 5_706_000);
}

#[tokio::test]
async fn codex_desktops_union_tool_reaches_xai_as_one_object_and_says_so() {
    let env = env().await;
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kaeso-codecs/fixtures/openai_responses/requests/codex-desktop-root-union.json"
    ))
    .unwrap();
    let mut body: Value = serde_json::from_str(&fixture).unwrap();
    body["model"] = json!("grok-4.3");
    body["stream"] = json!(false);
    body["input"][2]["content"][0]["text"] =
        json!("Weather in Boston? [mock:fixture=providers/xai/recorded-tools-function]");
    let r = call(&env, "/v1/responses", Some(&env.key), false, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("tools[0].tools[0]"));
    assert_eq!(r.header("kaeso-left-out"), None);
    let (_, _, sent) = sent(&env).unwrap();
    let tools: Vec<(&str, bool)> = sent["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap(),
                t["parameters"].get("oneOf").is_some(),
            )
        })
        .collect();
    assert_eq!(
        tools,
        [
            ("mcp__codex_app__automation_update", false),
            ("mcp__codex_app__list_threads", false)
        ]
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "grok-4.3",
            "Hi [mock:fixture=providers/xai/recorded-text-input-string]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), None);
}

#[tokio::test]
async fn an_anthropic_app_gets_xai_tool_calls_and_streams_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "grok-4.3",
            "Weather in Boston? [mock:fixture=providers/xai/recorded-tools-function]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    assert_eq!(body["model"], "grok-4.3");
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    let tool = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(tool["name"], "get_current_weather");
    assert_eq!(
        tool["input"],
        json!({ "location": "Boston, MA", "unit": "fahrenheit" })
    );
    assert_eq!(reply["usage"]["output_tokens"], 200);

    let mut stream = messages(
        "grok-4.3",
        "Hello! [mock:fixture=providers/xai/recorded-text-basic-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let text: String = events
        .iter()
        .filter(|e| e["type"] == "content_block_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Hello! How can I help you today?");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["usage"]["output_tokens"], 116);
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
}

#[tokio::test]
async fn xai_errors_reach_each_app_in_its_own_error_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "grok-4.20-0309-non-reasoning",
            "Say hi. [mock:error=providers/xai/recorded-presence-penalty-reasoning-model]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(
        error["message"],
        "Model grok-4.3 does not support parameter presencePenalty."
    );
    assert_eq!(error["type"], "invalid_request_error");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "grok-4.3",
            "Weather? [mock:error=providers/xai/recorded-reasoning-encrypted]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "Could not decrypt the provided encrypted_content. Ensure the value is the unmodified encrypted_content from a previous response."
    );
}

#[tokio::test]
async fn a_wrong_xai_key_is_reported_as_a_refused_key() {
    let env = env().await;
    let key_refused = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";
    for (path, anthropic, body, kind) in [
        (
            "/v1/messages",
            true,
            messages(
                "grok-4.3",
                "Hi. [mock:error=providers/xai/recorded-invalid-api-key]",
            ),
            "api_error",
        ),
        (
            "/v1/responses",
            false,
            json!({
                "model": "grok-4.3",
                "input": "Hi. [mock:error=providers/xai/recorded-invalid-api-key]",
            }),
            "server_error",
        ),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{path}: {}", r.text());
        let error = &r.json()["error"];
        assert_eq!(error["message"], key_refused, "{path}");
        assert_eq!(error["type"], kind, "{path}");
    }
}

#[tokio::test]
async fn token_counts_for_xai_models_are_estimated_in_each_apps_format() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": "grok-4.3",
            "messages": [{ "role": "user", "content": "Hello" }],
        })),
    )
    .await;
    assert!(r.estimated_count() > 0);

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "grok-4.3", "input": "Hello" })),
    )
    .await;
    assert!(r.estimated_count() > 0);

    // Stop sequences and a limit under 16 tokens, which Kaeso fits to OpenAI Responses when
    // it generates, are fine in a count too.
    let r = call(
        &env,
        "/v1beta/models/grok-4.3:countTokens",
        Some(&env.key),
        false,
        Some(json!({ "generateContentRequest": {
            "model": "models/grok-4.3",
            "contents": [{ "role": "user", "parts": [{ "text": "Hello" }] }],
            "generationConfig": { "stopSequences": ["END"], "maxOutputTokens": 5, "seed": 7 },
        } })),
    )
    .await;
    assert!(r.estimated_count() > 0);
    assert_eq!(
        r.header("kaeso-left-out"),
        Some("generationConfig.seed"),
        "as a generation would"
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn nextchats_neutral_penalties_reach_xais_responses_api_left_out_and_named() {
    let env = env().await;
    let mut body: Value = serde_json::from_str(include_str!(
        "../../../kaeso-codecs/fixtures/openai_chat/requests/nextchat-defaults.json"
    ))
    .unwrap();
    body["model"] = json!("grok-4.3");
    body["stream"] = json!(false);
    body["store"] = json!(false);
    body["service_tier"] = json!("auto");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.header("kaeso-left-out"),
        Some("frequency_penalty, presence_penalty, service_tier")
    );
    let (path, _, sent) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    for key in ["frequency_penalty", "presence_penalty", "service_tier"] {
        assert!(sent.get(key).is_none(), "{key}: {sent}");
    }
    assert_eq!(sent["store"], json!(false));
}

fn text_of(events: &[Value], pick: impl Fn(&Value) -> Option<&str>) -> String {
    events.iter().filter_map(pick).collect()
}

#[tokio::test]
async fn stop_sequences_small_limits_and_seed_reach_grok_from_other_formats() {
    let env = env().await;
    let hello = "Hello! [mock:fixture=providers/xai/recorded-text-basic-stream]";

    let mut asked = chat("grok-4.3", hello);
    asked["stop"] = json!([" help", "never"]);
    asked["seed"] = json!(7);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(asked),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("stop"));
    assert_eq!(r.header("kaeso-left-out"), Some("seed"));
    let (path, _, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    assert!(
        body.get("stop").is_none() && body.get("seed").is_none(),
        "{body}"
    );
    assert_eq!(
        body["stream"], true,
        "Kaeso reads a reply it ends itself as a stream"
    );
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello! How can I"
    );
    assert_eq!(reply["choices"][0]["finish_reason"], "stop");
    for count in ["prompt_tokens", "completion_tokens"] {
        assert!(
            reply["usage"][count].as_u64().unwrap() > 0,
            "the reply ended before xAI's usage, so it gets the estimate: {reply}"
        );
    }

    let mut streamed = messages("grok-4.3", hello);
    streamed["stream"] = json!(true);
    streamed["stop_sequences"] = json!(["you"]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(streamed)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("stop_sequences"));
    let events = sse_data(&r.text());
    let text = text_of(&events, |e| e["delta"]["text"].as_str());
    assert_eq!(text, "Hello! How can I help ");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["delta"]["stop_reason"], "stop_sequence");
    assert_eq!(end["delta"]["stop_sequence"], "you");
    assert_eq!(events.last().unwrap()["type"], "message_stop");

    let mut small = chat("grok-4.3", hello);
    small["max_tokens"] = json!(3);
    small["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(small),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("max_tokens"));
    let (_, _, body) = sent(&env).unwrap();
    assert_eq!(body["max_output_tokens"], 16);
    let events = sse_data(&r.text());
    let text = text_of(&events, |e| e["choices"][0]["delta"]["content"].as_str());
    assert_eq!(
        text, "Hello! How",
        "xAI's limit leaves out reasoning, and so does Kaeso's"
    );
    let finished = text_of(&events, |e| e["choices"][0]["finish_reason"].as_str());
    assert_eq!(finished, "length");

    env.mock.clear();
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": "grok-4.3", "input": hello, "max_output_tokens": 5 })),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "a Responses app's own limit goes as it wrote it: {}",
        r.text()
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_crewai_tool_messages_name_is_left_out_for_xais_responses_api() {
    let env = env().await;
    let mut body: Value = serde_json::from_str(include_str!(
        "../../../kaeso-codecs/fixtures/openai_chat/requests/crewai-tool-name.json"
    ))
    .unwrap();
    body["model"] = json!("grok-4.3");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("messages[3].name"));
    let (path, _, sent) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    let output = sent["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "function_call_output")
        .unwrap();
    assert_eq!(output["call_id"], "call_1");
    assert!(output.get("name").is_none(), "{output}");
}

#[tokio::test]
async fn a_cached_reply_keeps_to_the_stop_sequence_of_its_own_request() {
    let env = env().await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/response-cache",
                env.owner.org_id, env.project
            ),
            Some(json!({ "enabled": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let hello = "Hello! [mock:fixture=providers/xai/recorded-text-basic-stream]";
    let mut texts = Vec::new();
    for (stop, cache) in [(" help", "miss"), (" today", "miss"), (" help", "hit")] {
        let mut asked = chat("grok-4.3", hello);
        asked["stop"] = json!([stop]);
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(asked),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.header("kaeso-cache"), Some(cache), "{stop}");
        let reply = r.json();
        texts.push(reply["choices"][0]["message"]["content"].clone());
    }
    assert_eq!(
        texts,
        [
            "Hello! How can I",
            "Hello! How can I help you",
            "Hello! How can I"
        ]
    );
}

#[allow(clippy::unwrap_used)]
async fn count_with(env: &Env, body: Value, cache_control: Option<&str>) -> common::v1::Answer {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let mut req = axum::http::Request::builder()
        .uri("/v1/messages/count_tokens")
        .method("POST")
        .header("x-api-key", env.key.as_str())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json");
    if let Some(c) = cache_control {
        req = req.header("cache-control", c);
    }
    let req = req
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (resp.status(), resp.headers().clone());
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    env.app.state.in_flight.settled().await;
    common::v1::Answer {
        status,
        headers,
        body,
    }
}

#[tokio::test]
async fn an_estimated_count_gets_the_checks_a_sent_request_gets() {
    let env = env().await;
    env.mock.clear();
    let hello =
        json!({ "model": "grok-4.3", "messages": [{ "role": "user", "content": "Hello" }] });

    let r = count_with(&env, hello.clone(), Some("only-if-cached")).await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "No cached reply answers this request, and `only-if-cached` keeps it from the provider."
    );

    let notes = json!({
        "model": "grok-4.3",
        "messages": [{ "role": "user", "content": [
            { "type": "document", "source": { "type": "text", "media_type": "text/plain", "data": "Notes" } },
            { "type": "text", "text": "Summarise." },
        ] }],
    });
    let r = count_with(&env, notes, None).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("text document"),
        "{}",
        r.text()
    );

    sqlx::query("UPDATE catalogue_models SET availability = 'unavailable', availability_checked_at = $1 WHERE model_id = 'grok-4.3'")
        .bind(env.app.state.clock.now())
        .execute(&env.app.state.pg)
        .await
        .unwrap();
    kaeso_server::services::catalogue::forget(&env.app.state, common::uuid(env.project.as_str()))
        .await;
    env.app
        .clock
        .advance(kaeso_server::services::availability::RECHECK_EVERY);
    let r = count_with(&env, hello, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "SpaceX does not serve `grok-4.3` to this project's key; check the key's access."
    );
    assert!(
        env.mock.received().is_empty(),
        "an estimate asks the provider nothing, not even to recheck the model"
    );
}
