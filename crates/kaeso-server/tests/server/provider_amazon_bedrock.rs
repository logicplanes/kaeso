#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "ABSKa2Flc28tdGVzdC1jcmVkZW50aWFsLTAwMDE=";
const REGION: &str = "eu-central-1";
const DEEPSEEK: &str = "deepseek.v3.2";
const CLAUDE: &str = "us.anthropic.claude-sonnet-5";
const NOVA: &str = "amazon.nova-pro-v1:0";
const GPT: &str = "openai.gpt-6-sol";

fn error(name: &str) -> String {
    format!("[mock:error=../providers/amazon-bedrock/errors/{name}]")
}

fn credentials(env: &Env) -> String {
    format!("/orgs/{}/credentials", env.owner.org_id)
}

async fn add_key(env: &Env, body: Value) -> common::Resp {
    env.app
        .post(&credentials(env), body, Some(&env.owner.cookie))
        .await
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = add_key(
        &env,
        json!({ "provider": "amazon-bedrock", "name": "Bedrock", "secret": SECRET, "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["location"], REGION);
    for model in [DEEPSEEK, CLAUDE, NOVA, GPT] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "amazon-bedrock", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

#[tokio::test]
async fn chat_models_go_to_the_openai_route_with_a_bearer_key_and_claude_to_messages_with_x_api_key()
 {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            DEEPSEEK,
            "Hello [mock:fixture=recorded-response-format-text]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("amazon-bedrock"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/openai/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.json()["model"], DEEPSEEK);

    let mut body = messages(
        CLAUDE,
        "Hello [mock:fixture=recorded-thinking-enabled-budget]",
    );
    body["max_tokens"] = json!(2048);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 1024 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][0]["type"], "thinking", "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/anthropic/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(sent.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(sent.json()["model"], CLAUDE);

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(CLAUDE, "Hello [mock:fixture=recorded-text-simple]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/anthropic/v1/messages");
}

#[tokio::test]
async fn a_model_bedrock_serves_only_through_its_own_api_is_refused_before_sending() {
    let env = env().await;
    for model in [GPT] {
        let before = env.mock.received().len();
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(
                model,
                "Hello [mock:fixture=recorded-response-format-text]",
            )),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{model}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!(
                "Amazon Bedrock serves `{model}` only through its own API, which Kaeso doesn't call yet."
            )
        );
        assert_eq!(r.json()["error"]["param"], "model");
        assert_eq!(env.mock.received().len(), before, "{model}: nothing sent");
    }
}

#[tokio::test]
async fn bedrock_refusals_of_the_key_are_refused_keys_on_both_routes_and_token_counts_are_refused()
{
    let env = env().await;
    for name in [
        "recorded-invalid-api-key",
        "recorded-long-term-key-format",
        "recorded-short-term-key-format",
        "recorded-refused-api-key",
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(DEEPSEEK, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains("refused"), "{name}: {message}");

        let mut body = messages(CLAUDE, &format!("Hello {}", error(name)));
        body["max_tokens"] = json!(64);
        let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        assert_eq!(
            r.json()["error"]["type"],
            "api_error",
            "{name}: {}",
            r.text()
        );
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains("refused"), "{name}: {message}");
    }
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": CLAUDE, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.estimated_count() > 0);
}

#[tokio::test]
async fn a_key_calls_one_of_bedrocks_regions_which_may_change_and_other_keys_call_none() {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let key = |extra: Value| {
        let mut k = json!({ "provider": "amazon-bedrock", "name": "Bedrock", "secret": SECRET });
        for (name, value) in extra.as_object().unwrap() {
            k[name] = value.clone();
        }
        k
    };
    for (extra, message) in [
        (
            json!({}),
            "Amazon Bedrock keys call one region; give it as `location`.",
        ),
        (
            json!({ "location": "us-west-1" }),
            "Kaeso doesn't call that region for this provider; choose one from the list.",
        ),
        (
            json!({ "location": "global" }),
            "Kaeso doesn't call that region for this provider; choose one from the list.",
        ),
    ] {
        let r = add_key(&env, key(extra.clone())).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{extra}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "location", "{extra}");
        assert_eq!(r.body["error"]["message"], message, "{extra}");
    }
    let r = add_key(
        &env,
        json!({ "provider": "openai", "name": "OpenAI", "secret": "sk-test-credential-0001", "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This key calls no location; leave `location` out."
    );

    let r = add_key(&env, key(json!({ "location": "us-gov-west-1" }))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(
        (&r.body["auth_method"], &r.body["location"]),
        (&json!("api_key"), &json!("us-gov-west-1"))
    );
    let id = r.body["id"].as_str().unwrap().to_owned();
    let path = format!("{}/{id}", credentials(&env));
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Bedrock", "location": "ap-northeast-1" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["location"], "ap-northeast-1");
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Bedrock", "location": "mars-north-1" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": "bedrock-api-key-dGVzdC1jcmVkZW50aWFsLTAwMDI=" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["location"], "ap-northeast-1",
        "a replaced key keeps its region"
    );
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": SECRET, "location": "eu-west-3" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["location"], "eu-west-3");

    let listed = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let providers = listed.body["data"].as_array().unwrap();
    let bedrock = providers
        .iter()
        .find(|p| p["id"] == "amazon-bedrock")
        .unwrap();
    assert_eq!(bedrock["name"], "Amazon Bedrock");
    assert_eq!(bedrock["key_prefix"], Value::Null);
    assert_eq!(
        bedrock["dialects"],
        json!(["openai_chat", "anthropic_messages", "bedrock_converse"])
    );
    assert_eq!(bedrock["count_tokens"], Value::Null);
    let regions = bedrock["regions"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 22);
    assert!(regions.contains(&json!(REGION)));
    assert_eq!(
        bedrock["regions"]["regions_page"]["url"],
        "https://docs.aws.amazon.com/general/latest/gr/bedrock.html"
    );
    assert!(
        providers
            .iter()
            .filter(|p| p["id"] != "amazon-bedrock")
            .all(|p| p["regions"].is_null())
    );
}

#[tokio::test]
async fn a_model_check_is_inconclusive_and_sends_nothing() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/models/check",
                env.owner.org_id, env.project
            ),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let mut failed: Vec<(&str, &str)> = r.body["failed"]
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
    failed.sort_unstable();
    assert_eq!(
        failed,
        [
            (NOVA, "inconclusive"),
            (DEEPSEEK, "inconclusive"),
            (GPT, "inconclusive"),
            (CLAUDE, "inconclusive"),
        ],
        "{:?}",
        r.body
    );
    let bedrock: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .map(|r| r.path)
        .filter(|p| {
            p.starts_with("/openai/") || p.starts_with("/anthropic/") || p.starts_with("/model/")
        })
        .collect();
    assert!(
        bedrock.is_empty(),
        "no request reaches Bedrock: {bedrock:?}"
    );
}

const ACCESS_KEY_ID: &str = "AKIAIOSFODNN7EXAMPLE";
const SECRET_ACCESS_KEY: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";

fn access_key(token: Option<&str>) -> String {
    let mut key = json!({ "access_key_id": ACCESS_KEY_ID, "secret_access_key": SECRET_ACCESS_KEY });
    if let Some(t) = token {
        key["session_token"] = json!(t);
    }
    key.to_string()
}

async fn aws_env(method: &str, secret: &str, extra: Option<&[(&str, &str)]>) -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), extra).await;
    let r = add_key(
        &env,
        json!({ "provider": "amazon-bedrock", "name": "Bedrock", "auth_method": method, "secret": secret, "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["auth_method"], method);
    assert_eq!(r.body["location"], REGION);
    for model in [DEEPSEEK, CLAUDE] {
        env.app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "amazon-bedrock", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await
            .expect_status(StatusCode::CREATED);
    }
    env
}

/// Checks a request the mock received carries a SigV4 signature of `secret` over its method,
/// path, host, `content-type`, `x-amz-*` headers and body, in the key's region.
fn assert_signed(
    sent: &kaeso_testkit::mock_provider::Received,
    id: &str,
    secret: &str,
    token: Option<&str>,
) {
    let date = sent.header("x-amz-date").unwrap();
    let at = time::PrimitiveDateTime::parse(
        date,
        time::macros::format_description!("[year][month][day]T[hour][minute][second]Z"),
    )
    .unwrap()
    .assume_utc();
    assert_eq!(sent.header("x-amz-security-token"), token);
    let signer = kaeso_upstream::sigv4::SigV4 {
        credentials: kaeso_upstream::sigv4::AwsCredentials {
            access_key_id: id.to_owned(),
            secret_access_key: secrecy::SecretString::from(secret.to_owned()),
            session_token: token.map(|t| secrecy::SecretString::from(t.to_owned())),
        },
        region: REGION.to_owned(),
        service: "bedrock".to_owned(),
        at,
    };
    let content_type = sent.header("content-type").unwrap();
    let expected = signer.sign(&kaeso_upstream::sigv4::Request {
        method: &sent.method,
        host: sent.header("host").unwrap(),
        path: &sent.path,
        query: sent.query.as_deref().unwrap_or_default(),
        headers: &[("content-type", content_type)],
        body: &sent.body,
    });
    let authorization = &expected
        .headers
        .iter()
        .find(|(n, _)| *n == "authorization")
        .unwrap()
        .1;
    assert_eq!(sent.header("authorization"), Some(authorization.as_str()));
    assert!(
        authorization.contains(&format!("/{REGION}/bedrock/aws4_request")),
        "{authorization}"
    );
}

#[tokio::test]
async fn an_access_key_signs_both_routes_with_sigv4_in_the_keys_region() {
    let token = "IQoJb3JpZ2luX2VjEXAMPLESESSIONTOKEN";
    let env = aws_env("aws_access_key", &access_key(Some(token)), None).await;
    let listed = env.app.get(&credentials(&env), &env.owner.cookie).await;
    let key = &listed.body["data"][0];
    assert_eq!(
        (&key["account"], &key["last4"]),
        (&json!(ACCESS_KEY_ID), &json!("MPLE"))
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            DEEPSEEK,
            "Hello [mock:fixture=recorded-response-format-text]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/openai/v1/chat/completions");
    assert_signed(&sent, ACCESS_KEY_ID, SECRET_ACCESS_KEY, Some(token));

    let mut body = messages(
        CLAUDE,
        "Hello [mock:fixture=recorded-thinking-enabled-budget]",
    );
    body["max_tokens"] = json!(2048);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/anthropic/v1/messages");
    assert_eq!(sent.header("x-api-key"), None);
    assert_signed(&sent, ACCESS_KEY_ID, SECRET_ACCESS_KEY, Some(token));
    assert!(!r.text().contains(SECRET_ACCESS_KEY));
}

#[tokio::test]
async fn a_malformed_access_key_and_a_role_without_the_operators_leave_are_refused() {
    let env = common::v1::env(None, None).await;
    assert!(!gateway_identity(&env).await);
    for (method, secret, message) in [
        (
            "aws_access_key",
            "not json".to_owned(),
            "Give the AWS key as a JSON object.",
        ),
        (
            "aws_access_key",
            json!({ "access_key_id": ACCESS_KEY_ID }).to_string(),
            "The AWS key has no `secret_access_key`.",
        ),
        (
            "aws_role",
            "{}".to_owned(),
            "This install doesn't let keys sign in as the gateway's AWS identity.",
        ),
    ] {
        let r = add_key(
            &env,
            json!({ "provider": "amazon-bedrock", "name": "Bedrock", "auth_method": method, "secret": secret, "location": REGION }),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{method}: {:?}", r.body);
        assert_eq!(r.body["error"]["message"], message);
    }
    let r = add_key(
        &env,
        json!({ "provider": "openai", "name": "OpenAI", "auth_method": "aws_access_key", "secret": access_key(None) }),
    )
    .await;
    assert_eq!(
        r.body["error"]["message"],
        "OpenAI takes no AWS key; add an API key."
    );
}

/// The gateway's own AWS identity: a container credentials endpoint and STS, answered by a
/// stub that counts what it was asked.
/// What the stub was asked: the body or `container`, and the `authorization` header.
type Asked = Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

struct Aws {
    url: String,
    asked: Asked,
}

async fn stub_aws(expires: time::OffsetDateTime) -> Aws {
    use axum::routing::{get, post};
    let asked: Asked = Arc::default();
    let expiration = expires
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let (log, log2) = (asked.clone(), asked.clone());
    let exp = expiration.clone();
    let app = axum::Router::new()
        .route(
            "/creds",
            get(move |headers: axum::http::HeaderMap| {
                let log = log.clone();
                let exp = exp.clone();
                async move {
                    log.lock().unwrap().push((
                        "container".into(),
                        headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                    ));
                    axum::Json(json!({
                        "AccessKeyId": "ASIACONTAINEREXAMPLE",
                        "SecretAccessKey": "containerSecret",
                        "Token": "containerToken",
                        "Expiration": exp,
                    }))
                }
            }),
        )
        .route(
            "/sts",
            post(move |headers: axum::http::HeaderMap, body: String| {
                let log = log2.clone();
                let exp = expiration.clone();
                async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    log.lock().unwrap().push((body, auth));
                    format!(
                        "<AssumeRoleResponse><AssumeRoleResult><Credentials><AccessKeyId>ASIAROLEEXAMPLE0001</AccessKeyId><SecretAccessKey>roleSecret</SecretAccessKey><SessionToken>roleToken</SessionToken><Expiration>{exp}</Expiration></Credentials></AssumeRoleResult></AssumeRoleResponse>"
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Aws { url, asked }
}

struct Vars(std::collections::HashMap<String, String>);

impl kaeso_server::services::aws_auth::Environment for Vars {
    fn var(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

#[tokio::test]
async fn the_gateways_role_signs_with_assumed_credentials_renewed_before_they_expire() {
    let now = time::OffsetDateTime::now_utc();
    let aws = stub_aws(now + time::Duration::hours(1)).await;
    let env = common::v1::env(
        Some(Arc::new(Capture::default())),
        Some(&[("KAESO_AWS_AMBIENT_CREDENTIALS", "true")]),
    )
    .await;
    kaeso_server::services::aws_auth::use_environment(Arc::new(Vars(
        [
            (
                "AWS_CONTAINER_CREDENTIALS_FULL_URI",
                format!("{}/creds", aws.url),
            ),
            ("AWS_CONTAINER_AUTHORIZATION_TOKEN", "Bearer pod".to_owned()),
            ("AWS_ENDPOINT_URL_STS", format!("{}/sts", aws.url)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect(),
    )));
    let role = json!({ "role_arn": "arn:aws:iam::123456789012:role/kaeso-bedrock", "external_id": "tenant-1" });
    let r = add_key(
        &env,
        json!({ "provider": "amazon-bedrock", "name": "Bedrock", "auth_method": "aws_role", "secret": role.to_string(), "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["account"], "kaeso-bedrock");
    assert!(gateway_identity(&env).await);
    env.app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "amazon-bedrock", "model_id": DEEPSEEK }),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED);
    env.app.clock.set(now);
    let ask = || async {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(
                DEEPSEEK,
                "Hello [mock:fixture=recorded-response-format-text]",
            )),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        env.mock.last().unwrap()
    };
    let sent = ask().await;
    assert_signed(
        &sent,
        "ASIAROLEEXAMPLE0001",
        "roleSecret",
        Some("roleToken"),
    );
    {
        let asked = aws.asked.lock().unwrap();
        assert_eq!(asked.len(), 2, "{asked:?}");
        assert_eq!(asked[0], ("container".into(), Some("Bearer pod".into())));
        assert!(asked[1].0.contains("Action=AssumeRole"), "{}", asked[1].0);
        assert!(asked[1].0.contains("ExternalId=tenant-1"), "{}", asked[1].0);
        let auth = asked[1].1.as_deref().unwrap();
        assert!(
            auth.starts_with("AWS4-HMAC-SHA256 Credential=ASIACONTAINEREXAMPLE/")
                && auth.contains(&format!("/{REGION}/sts/aws4_request")),
            "{auth}"
        );
    }
    // Cached until five minutes before they expire, then renewed.
    ask().await;
    assert_eq!(aws.asked.lock().unwrap().len(), 2);
    env.app.clock.advance(time::Duration::minutes(56));
    ask().await;
    assert_eq!(aws.asked.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn aws_refusing_a_signed_request_is_a_refused_provider_key() {
    let env = aws_env("aws_access_key", &access_key(None), None).await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            DEEPSEEK,
            &format!("Hello {}", error("built-unrecognized-client")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers."
    );
}

async fn gateway_identity(env: &Env) -> bool {
    let r = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let bedrock = r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "amazon-bedrock")
        .unwrap();
    bedrock["aws"]["gateway_identity"].as_bool().unwrap()
}

const NOVA_PATH: &str = "/model/amazon.nova-pro-v1%3A0";

fn top_song() -> Value {
    json!([{ "type": "function", "function": {
        "name": "top_song",
        "description": "Get the most popular song played on a radio station.",
        "parameters": { "type": "object", "properties": { "sign": { "type": "string" } }, "required": ["sign"] }
    } }])
}

fn sse(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|d| *d != "[DONE]")
        .map(|d| serde_json::from_str(d).unwrap())
        .collect()
}

#[tokio::test]
async fn an_openai_app_gets_a_streamed_tool_call_from_nova_through_converse() {
    let env = env().await;
    let mut body = chat(
        NOVA,
        "What is the most popular song on WZPZ? [mock:fixture=built-nova-tool-use]",
    );
    body["stream"] = json!(true);
    body["stream_options"] = json!({ "include_usage": true });
    body["tools"] = top_song();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, format!("{NOVA_PATH}/converse-stream"));
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    let converse = sent.json();
    assert_eq!(
        converse["toolConfig"]["tools"][0]["toolSpec"]["name"],
        "top_song"
    );
    assert_eq!(converse["messages"][0]["role"], "user");

    let chunks = sse(&r.text());
    let calls: Vec<&Value> = chunks
        .iter()
        .filter_map(|c| {
            c["choices"][0]["delta"]["tool_calls"][0]
                .as_object()
                .map(|_| &c["choices"][0]["delta"]["tool_calls"][0])
        })
        .collect();
    assert_eq!(
        calls[0]["id"],
        "tooluse_20Z9zl0BQWSXjFuLKdTJcA",
        "{}",
        r.text()
    );
    assert_eq!(calls[0]["function"]["name"], "top_song");
    let arguments: String = calls
        .iter()
        .filter_map(|c| c["function"]["arguments"].as_str())
        .collect();
    assert_eq!(arguments, r#"{"sign":"WZPZ"}"#);
    let text: String = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert!(text.starts_with("<thinking>"), "{text}");
    assert!(
        chunks
            .iter()
            .any(|c| c["choices"][0]["finish_reason"] == "tool_calls"),
        "{}",
        r.text()
    );
    let usage = chunks.iter().find(|c| c["usage"].is_object()).unwrap();
    assert_eq!(usage["usage"]["prompt_tokens"], 372);
    assert_eq!(usage["usage"]["completion_tokens"], 41);
}

#[tokio::test]
async fn an_anthropic_app_gets_a_streamed_tool_call_from_nova_through_converse() {
    let env = env().await;
    let mut body = messages(NOVA, "Top song on WZPZ? [mock:fixture=built-nova-tool-use]");
    body["stream"] = json!(true);
    body["tools"] = json!([{ "name": "top_song", "input_schema": { "type": "object", "properties": { "sign": { "type": "string" } } } }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.last().unwrap().path,
        format!("{NOVA_PATH}/converse-stream")
    );
    let events = sse(&r.text());
    let start = events
        .iter()
        .find(|e| e["type"] == "content_block_start" && e["content_block"]["type"] == "tool_use")
        .unwrap();
    assert_eq!(start["content_block"]["name"], "top_song");
    assert_eq!(
        start["content_block"]["id"],
        "tooluse_20Z9zl0BQWSXjFuLKdTJcA"
    );
    let input: String = events
        .iter()
        .filter_map(|e| e["delta"]["partial_json"].as_str())
        .collect();
    assert_eq!(input, r#"{"sign":"WZPZ"}"#);
    let delta = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(delta["delta"]["stop_reason"], "tool_use");
    assert_eq!(delta["usage"]["output_tokens"], 41);
    assert_eq!(events.last().unwrap()["type"], "message_stop");
}

#[tokio::test]
async fn openai_anthropic_and_gemini_apps_get_whole_answers_from_nova() {
    let env = env().await;
    let mut body = chat(NOVA, "Top song on WZPZ? [mock:fixture=built-nova-tool-use]");
    body["tools"] = top_song();
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
        env.mock.last().unwrap().path,
        format!("{NOVA_PATH}/converse")
    );
    let choice = &r.json()["choices"][0];
    assert_eq!(choice["finish_reason"], "tool_calls");
    assert_eq!(
        choice["message"]["tool_calls"][0]["function"]["name"],
        "top_song"
    );
    assert_eq!(
        choice["message"]["tool_calls"][0]["function"]["arguments"],
        r#"{"sign":"WZPZ"}"#
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(NOVA, "Hello [mock:fixture=built-nova-text]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["content"][0]["text"],
        "Hello! How can I help you today?"
    );
    assert_eq!(r.json()["stop_reason"], "end_turn");
    assert_eq!(r.json()["usage"]["input_tokens"], 4);

    let req = axum::http::Request::builder()
        .uri(format!("/v1beta/models/{NOVA}:generateContent"))
        .method("POST")
        .header("x-goog-api-key", &env.key)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "contents": [{ "role": "user", "parts": [{ "text": "Hello [mock:fixture=built-nova-text]" }] }] }).to_string(),
        ))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    let status = resp.status();
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let answer: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer["candidates"][0]["content"]["parts"][0]["text"],
        "Hello! How can I help you today?"
    );
    assert_eq!(
        env.mock.last().unwrap().path,
        format!("{NOVA_PATH}/converse")
    );
}

#[tokio::test]
async fn converse_errors_reach_the_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(NOVA, "Hello [mock:error=built-validation-too-long]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["code"],
        "context_length_exceeded",
        "{}",
        r.text()
    );

    let mut body = chat(NOVA, "Hello [mock:fixture=built-throttling-exception]");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    let text = r.text();
    assert!(
        text.contains("Too many tokens, please wait before trying again."),
        "{text}"
    );

    let mut body = chat(NOVA, "Hello");
    body["seed"] = json!(7);
    let before = env.mock.received().len();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "seed");
    assert_eq!(env.mock.received().len(), before, "refused before sending");
}
