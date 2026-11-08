#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::v1::{Env, call, chat, messages};
use kaeso_testkit::google::ServiceAccountKey;
use kaeso_testkit::mock_provider::{GOOGLE_TOKEN_PATH, Received};
use serde_json::{Value, json};

const PROVIDER: &str = "gemini-agent-platform";
const EXPRESS_KEY: &str = "AQ.TestExpressModeKey0000000000000000";
const FLASH: &str = "gemini-3.5-flash";
const EMAIL: &str = "kaeso@kaeso-example.iam.gserviceaccount.com";
const PROJECT: &str = "kaeso-example";

fn fixture(name: &str) -> String {
    format!("[mock:fixture={name}]")
}

async fn env() -> Env {
    let env = common::v1::env_with_google_token(None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": PROVIDER, "model_id": FLASH }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

async fn add_express(env: &Env) -> Value {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": PROVIDER, "name": "express", "secret": EXPRESS_KEY }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn add_service_account(env: &Env, key: &ServiceAccountKey, location: Option<&str>) -> Value {
    let mut body = json!({
        "provider": PROVIDER,
        "name": "service account",
        "secret": key.json,
        "auth_method": "google_service_account",
    });
    if let Some(l) = location {
        body["location"] = json!(l);
    }
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

fn token_requests(env: &Env) -> Vec<Received> {
    env.mock
        .received()
        .into_iter()
        .filter(|r| r.path == GOOGLE_TOKEN_PATH)
        .collect()
}

fn model_calls(env: &Env) -> Vec<Received> {
    env.mock
        .received()
        .into_iter()
        .filter(|r| r.path != GOOGLE_TOKEN_PATH)
        .collect()
}

fn form(r: &Received) -> Vec<(String, String)> {
    url::form_urlencoded::parse(&r.body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

async fn ask(env: &Env, text: &str) -> common::v1::Answer {
    call(
        env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(FLASH, text)),
    )
    .await
}

#[tokio::test]
async fn an_express_mode_key_calls_gemini_models_without_a_project_in_the_path() {
    let env = env().await;
    let created = add_express(&env).await;
    assert_eq!(created["auth_method"], "api_key");
    assert_eq!(created["project"], Value::Null);
    assert_eq!(created["location"], Value::Null);
    let r = ask(&env, &format!("Hello {}", fixture("recorded-text-simple"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some(PROVIDER));
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/v1/publishers/google/models/gemini-3.5-flash:generateContent"
    );
    assert_eq!(sent.header("x-goog-api-key"), Some(EXPRESS_KEY));
    assert_eq!(sent.header("authorization"), None);
    assert!(sent.query.is_none(), "{:?}", sent.query);
    let body = sent.json();
    assert!(body.get("model").is_none(), "{body}");
    for content in body["contents"].as_array().unwrap() {
        assert!(content["role"].is_string(), "{body}");
    }
    assert_eq!(
        r.json()["choices"][0]["message"]["content"],
        "Hello, how are you today?"
    );
    assert!(token_requests(&env).is_empty());

    let mut stream = messages(FLASH, &format!("Count {}", fixture("recorded-stream-text")));
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.path,
        "/v1/publishers/google/models/gemini-3.5-flash:streamGenerateContent"
    );
    assert_eq!(sent.query.as_deref(), Some("alt=sse"));
    assert!(r.text().contains("event: message_stop"));
}

#[tokio::test]
async fn a_service_account_key_signs_a_jwt_trades_it_for_a_token_and_calls_its_project() {
    let env = env().await;
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    let created = add_service_account(&env, &key, Some("us-east5")).await;
    assert_eq!(created["auth_method"], "google_service_account");
    assert_eq!(created["account"], EMAIL);
    assert_eq!(created["project"], PROJECT);
    assert_eq!(created["location"], "us-east5");
    assert_eq!(created["last4"], "cafe");
    assert!(!created.to_string().contains("PRIVATE KEY"), "{created}");

    let before = kaeso_core::clock::Clock::now(&env.app.clock).unix_timestamp();
    let r = ask(&env, &format!("Hello {}", fixture("recorded-text-simple"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    let exchanges = token_requests(&env);
    assert_eq!(exchanges.len(), 1);
    let exchange = &exchanges[0];
    assert_eq!(exchange.method, "POST");
    assert_eq!(
        exchange.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    let form = form(exchange);
    assert_eq!(
        form.iter().find(|(k, _)| k == "grant_type").unwrap().1,
        "urn:ietf:params:oauth:grant-type:jwt-bearer"
    );
    let assertion = &form.iter().find(|(k, _)| k == "assertion").unwrap().1;
    let (header, claims) = key.verify(assertion).expect("the assertion verifies");
    assert_eq!(header["alg"], "RS256");
    assert_eq!(header["typ"], "JWT");
    assert_eq!(header["kid"], key.private_key_id.as_str());
    assert_eq!(claims["iss"], EMAIL);
    assert_eq!(claims["aud"], "https://oauth2.googleapis.com/token");
    assert_eq!(
        claims["scope"],
        "https://www.googleapis.com/auth/cloud-platform"
    );
    assert!(claims.get("sub").is_none(), "{claims}");
    let iat = claims["iat"].as_i64().unwrap();
    assert_eq!(iat, before);
    assert_eq!(claims["exp"].as_i64().unwrap(), iat + 3600);

    let sent = model_calls(&env).pop().unwrap();
    assert_eq!(
        sent.path,
        "/v1/projects/kaeso-example/locations/us-east5/publishers/google/models/gemini-3.5-flash:generateContent"
    );
    assert_eq!(sent.header("authorization"), Some("Bearer ya29.mock-1"));
    assert_eq!(sent.header("x-goog-api-key"), None);

    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": FLASH, "messages": [{ "role": "user", "content": "Hi" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = model_calls(&env).pop().unwrap();
    assert_eq!(
        sent.path,
        "/v1/projects/kaeso-example/locations/us-east5/publishers/google/models/gemini-3.5-flash:countTokens"
    );
    assert_eq!(sent.header("authorization"), Some("Bearer ya29.mock-1"));
    assert_eq!(token_requests(&env).len(), 1, "the token is reused");
}

#[tokio::test]
async fn the_token_is_renewed_five_minutes_before_it_expires_and_once_for_concurrent_requests() {
    let env = env().await;
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    add_service_account(&env, &key, None).await;
    let asks = (0..8).map(|_| ask(&env, "Hello"));
    for r in futures::future::join_all(asks).await {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    assert_eq!(
        token_requests(&env).len(),
        1,
        "one exchange for eight requests"
    );
    let sent = model_calls(&env).pop().unwrap();
    assert_eq!(
        sent.path,
        "/v1/projects/kaeso-example/locations/global/publishers/google/models/gemini-3.5-flash:generateContent"
    );

    env.app.clock.advance(time::Duration::minutes(54));
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    assert_eq!(token_requests(&env).len(), 1);
    env.app.clock.advance(time::Duration::minutes(1));
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    assert_eq!(
        token_requests(&env).len(),
        2,
        "renewed 5 minutes before expiry"
    );
    let sent = model_calls(&env).pop().unwrap();
    assert_eq!(sent.header("authorization"), Some("Bearer ya29.mock-2"));
}

#[tokio::test]
async fn changing_or_deleting_the_key_drops_its_token() {
    let env = env().await;
    let org = env.owner.org_id.clone();
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    let created = add_service_account(&env, &key, None).await;
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    assert_eq!(token_requests(&env).len(), 1);

    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("/orgs/{org}/credentials/{id}"),
            Some(json!({ "name": "moved", "location": "eu" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["location"], "eu");
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    assert_eq!(token_requests(&env).len(), 2);
    let sent = model_calls(&env).pop().unwrap();
    assert_eq!(
        sent.path,
        "/v1/projects/kaeso-example/locations/eu/publishers/google/models/gemini-3.5-flash:generateContent"
    );
    assert_eq!(sent.header("authorization"), Some("Bearer ya29.mock-2"));

    let other =
        ServiceAccountKey::new("other@kaeso-example.iam.gserviceaccount.com", "kaeso-other");
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials/{id}/replace"),
            json!({ "secret": other.json }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["project"], "kaeso-other");
    assert_eq!(r.body["location"], "eu");
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    let exchanges = token_requests(&env);
    assert_eq!(exchanges.len(), 3);
    let assertion = form(exchanges.last().unwrap())
        .into_iter()
        .find(|(k, _)| k == "assertion")
        .unwrap()
        .1;
    assert!(other.verify(&assertion).is_some());
    assert!(key.verify(&assertion).is_none());

    let uuid = common::uuid(&id);
    assert!(env.app.state.google_tokens.holds(uuid));
    env.app
        .call(
            Method::DELETE,
            &format!("/orgs/{org}/credentials/{id}"),
            None,
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::NO_CONTENT);
    assert!(!env.app.state.google_tokens.holds(uuid));
}

#[tokio::test]
async fn keys_that_are_not_service_account_keys_are_refused_when_added() {
    let env = env().await;
    let org = env.owner.org_id.clone();
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    let edit = |f: &dyn Fn(&mut serde_json::Map<String, Value>)| {
        let mut v: Value = serde_json::from_str(&key.json).unwrap();
        f(v.as_object_mut().unwrap());
        v.to_string()
    };
    let cases = [
        (
            "not json".to_owned(),
            "secret",
            "Paste the service account's whole JSON key file.",
        ),
        (
            edit(&|m| {
                m.insert("type".into(), json!("authorized_user"));
            }),
            "secret",
            "The JSON key isn't a service account key; its `type` must be `service_account`.",
        ),
        (
            edit(&|m| {
                m.insert(
                    "token_uri".into(),
                    json!("https://attacker.example.com/token"),
                );
            }),
            "secret",
            "The JSON key's `token_uri` must be `https://oauth2.googleapis.com/token`.",
        ),
        (
            edit(&|m| {
                m.insert(
                    "private_key".into(),
                    json!("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n"),
                );
            }),
            "secret",
            "The JSON key's `private_key` isn't an RSA key; download a new key from Google Cloud.",
        ),
        (
            edit(&|m| {
                m.remove("client_email");
            }),
            "secret",
            "The JSON key has no `client_email`; download a new key from Google Cloud.",
        ),
    ];
    for (secret, param, message) in cases {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/credentials"),
                json!({ "provider": PROVIDER, "name": "bad", "secret": secret, "auth_method": "google_service_account" }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{:?}", r.body);
        assert_eq!(r.body["error"]["message"], message);
        assert!(!r.body.to_string().contains("PRIVATE KEY"));
    }
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": PROVIDER, "name": "bad", "secret": key.json, "auth_method": "google_service_account", "location": "mars-north1" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "location");
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "openai", "name": "bad", "secret": key.json, "auth_method": "google_service_account" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "auth_method");
    assert_eq!(
        r.body["error"]["message"],
        "OpenAI takes no service account key; add an API key."
    );
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": PROVIDER, "name": "bad", "secret": EXPRESS_KEY, "location": "eu" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "location");
    assert!(token_requests(&env).is_empty());
}

#[tokio::test]
async fn a_refused_or_unavailable_token_endpoint_reaches_apps_as_a_provider_error_in_their_format()
{
    let env = env().await;
    let refused = ServiceAccountKey::new("refused@kaeso-example.iam.gserviceaccount.com", PROJECT);
    add_service_account(&env, &refused, None).await;
    let r = ask(&env, "Hello").await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let body = r.json();
    assert_eq!(
        body["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    assert!(!r.text().contains("Invalid JWT"), "{}", r.text());
    assert!(model_calls(&env).is_empty());

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(FLASH, "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::BAD_GATEWAY);
    env.app.state.in_flight.settled().await;
    env.app.state.mailer.flush().await;
    let mails: Vec<_> = env
        .app
        .mail
        .to("owner@example.com")
        .into_iter()
        .filter(|m| m.kind.as_str() == "provider_keys")
        .collect();
    assert_eq!(mails.len(), 1, "three refusals in a row need attention");
    for line in [
        "Key: service account (…cafe)\n",
        "Provider's answer: HTTP 400: Google refused the service account key: invalid_grant\n",
    ] {
        assert!(mails[0].text.contains(line), "{line}\n{}", mails[0].text);
    }

    let env = self::env().await;
    let down = ServiceAccountKey::new("unavailable@kaeso-example.iam.gserviceaccount.com", PROJECT);
    add_service_account(&env, &down, None).await;
    let r = ask(&env, "Hello").await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Google can't sign in the provider key right now. Try again shortly."
    );
}

#[tokio::test]
async fn check_availability_counts_tokens_and_tells_a_refused_key() {
    let env = env().await;
    let org = env.owner.org_id.clone();
    let base = format!("/orgs/{org}/projects/{}", env.project);
    let r = env
        .app
        .post(
            &format!("{base}/models"),
            json!({
                "provider": PROVIDER,
                "model_id": "unserved-gemini",
                "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 1000, "output_limit": 100,
                    "input_price": "1", "output_price": "1", "egress_dialect": "gemini"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    add_service_account(&env, &key, None).await;
    let r = env
        .app
        .post(
            &format!("{base}/models/check"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let availability = |model: &str| {
        r.body["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["provider"] == PROVIDER && m["model_id"] == model)
            .map(|m| m["availability"].clone())
            .unwrap()
    };
    assert_eq!(availability(FLASH), "available", "{:?}", r.body);
    assert_eq!(
        availability("unserved-gemini"),
        "unavailable",
        "{:?}",
        r.body
    );
    let counted: Vec<String> = model_calls(&env)
        .iter()
        .map(|r| r.path.clone())
        .filter(|p| p.starts_with("/v1/projects/"))
        .collect();
    assert_eq!(counted.len(), 2, "{counted:?}");
    assert!(
        counted.iter().all(|p| p.ends_with(":countTokens")),
        "{counted:?}"
    );

    let env = self::env().await;
    let refused = ServiceAccountKey::new("refused@kaeso-example.iam.gserviceaccount.com", PROJECT);
    add_service_account(&env, &refused, None).await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let r = env
        .app
        .post(
            &format!("{base}/models/check"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let failed = r.body["failed"].as_array().unwrap();
    let ours = failed.iter().find(|f| f["provider"] == PROVIDER).unwrap();
    assert_eq!(ours["reason"], "key_refused", "{:?}", r.body);
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Captured {
        self.clone()
    }
}

#[tokio::test]
async fn the_key_its_assertion_and_its_tokens_never_reach_the_log_the_api_or_the_audit_trail() {
    let logs = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let env = env().await;
    let org = env.owner.org_id.clone();
    let key = ServiceAccountKey::new(EMAIL, PROJECT);
    let created = add_service_account(&env, &key, None).await;
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::OK);
    let refused = ServiceAccountKey::new("refused@kaeso-example.iam.gserviceaccount.com", PROJECT);
    let id = created["id"].as_str().unwrap();
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials/{id}/replace"),
            json!({ "secret": refused.json }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(ask(&env, "Hello").await.status, StatusCode::BAD_GATEWAY);
    env.app.state.in_flight.settled().await;

    let pem_line = |k: &ServiceAccountKey| -> String {
        let v: Value = serde_json::from_str(&k.json).unwrap();
        v["private_key"]
            .as_str()
            .unwrap()
            .lines()
            .nth(1)
            .unwrap()
            .to_owned()
    };
    let mut secrets = vec![pem_line(&key), pem_line(&refused), "ya29.mock-1".to_owned()];
    for exchange in token_requests(&env) {
        let assertion = form(&exchange)
            .into_iter()
            .find(|(k, _)| k == "assertion")
            .unwrap()
            .1;
        secrets.push(assertion.split('.').nth(2).unwrap().to_owned());
    }
    let listed = env
        .app
        .get(&format!("/orgs/{org}/credentials"), &env.owner.cookie)
        .await;
    let audit = env
        .app
        .get(&format!("/orgs/{org}/audit-log"), &env.owner.cookie)
        .await;
    let logged = logs.text();
    assert!(logged.contains("Google refused a service account key's assertion"));
    assert!(logged.contains("invalid_grant"));
    assert!(!logged.contains("Invalid JWT Signature"));
    for secret in &secrets {
        assert!(!logged.contains(secret.as_str()), "{secret} is in the log");
        for body in [&created, &r.body, &listed.body, &audit.body] {
            assert!(
                !body.to_string().contains(secret.as_str()),
                "{secret} in {body}"
            );
        }
    }
}
