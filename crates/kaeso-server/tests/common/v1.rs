use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use kaeso_server::model_api::hooks::{Limits, Outcome, Sink};
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{App, Owner};

pub const OPENAI_SECRET: &str = "sk-test-openai-credential-0001";
pub const ANTHROPIC_SECRET: &str = "sk-ant-test-credential-0002";

#[derive(Debug, Default)]
pub struct Capture(pub Mutex<Vec<Outcome>>);

impl Sink for Capture {
    fn finish<'a>(
        &'a self,
        _state: &'a kaeso_server::state::AppState,
        outcome: Outcome,
    ) -> futures::future::BoxFuture<'a, ()> {
        self.0.lock().unwrap().push(outcome);
        Box::pin(async {})
    }
}

impl Capture {
    pub fn all(&self) -> Vec<Outcome> {
        self.0.lock().unwrap().clone()
    }
}

pub struct Env {
    pub app: App,
    pub mock: MockProvider,
    pub owner: Owner,
    pub project: String,
    pub key: String,
    pub capture: Arc<Capture>,
    pub services: TestServices,
}

pub async fn env(capture: Option<Arc<Capture>>, extra: Option<&[(&str, &str)]>) -> Env {
    env_hooked(capture, None, extra).await
}

pub async fn env_hooked(
    capture: Option<Arc<Capture>>,
    limits: Option<Arc<dyn Limits>>,
    extra: Option<&[(&str, &str)]>,
) -> Env {
    let s = TestServices::start().await;
    let mock = MockProvider::start().await;
    let mut settings = vec![
        ("KAESO_PROVIDERS_OPENAI_BASE_URL", mock.base_url.as_str()),
        ("KAESO_PROVIDERS_ANTHROPIC_BASE_URL", mock.base_url.as_str()),
    ];
    settings.extend_from_slice(extra.unwrap_or(&[]));
    let sink = capture.clone().map(|c| c as Arc<dyn Sink>);
    let app = App::start_hooked(&s, &settings, limits, sink).await;
    let capture = capture.unwrap_or_default();
    let owner = app.setup().await;
    let project = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (provider, secret) in [("openai", OPENAI_SECRET), ("anthropic", ANTHROPIC_SECRET)] {
        let r = app
            .post(
                &format!("/orgs/{}/credentials", owner.org_id),
                json!({ "provider": provider, "name": provider, "secret": secret }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    let base = format!("/orgs/{}/projects/{project}", owner.org_id);
    for (provider, model) in [
        ("anthropic", "claude-haiku-4-5"),
        ("openai", "gpt-4o"),
        ("openai", "text-embedding-3-small"),
    ] {
        let r = app
            .post(
                &format!("{base}/models"),
                json!({ "provider": provider, "model_id": model }),
                Some(&owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    let r = app
        .post(&format!("{base}/models"), json!({ "provider": "openai", "model_id": "gpt-4o-mini", "overrides": { "timeout_seconds": 1 } }), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let key = app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "app" }),
            Some(&owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    Env {
        app,
        mock,
        owner,
        project,
        key,
        capture,
        services: s,
    }
}

pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&self.body)))
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

pub async fn call(
    env: &Env,
    path: &str,
    auth: Option<&str>,
    anthropic: bool,
    body: Option<Value>,
) -> Answer {
    call_app(&env.app, path, auth, anthropic, body).await
}

pub async fn call_app(
    app: &App,
    path: &str,
    auth: Option<&str>,
    anthropic: bool,
    body: Option<Value>,
) -> Answer {
    let mut req = Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" });
    if let Some(k) = auth {
        req = if anthropic {
            req.header("x-api-key", k)
        } else {
            req.header("authorization", format!("Bearer {k}"))
        };
    }
    if anthropic {
        req = req.header("anthropic-version", "2023-06-01");
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
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

pub fn messages(model: &str, text: &str) -> Value {
    json!({ "model": model, "max_tokens": 256, "messages": [{ "role": "user", "content": text }] })
}

pub fn chat(model: &str, text: &str) -> Value {
    json!({ "model": model, "messages": [{ "role": "user", "content": text }] })
}
