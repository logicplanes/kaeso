#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod copy;
pub mod v1;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use kaeso_core::clock::{ManualClock, SharedClock};
use kaeso_mail::MemorySink;
use kaeso_server::config::{self, Config, MapSource};
use kaeso_server::serve::Overrides;
use kaeso_server::state::AppState;
use kaeso_testkit::services::TestServices;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tower::ServiceExt;

pub const ORIGIN: &str = "https://kaeso.test";

pub fn config(s: &TestServices, extra: &[(&str, &str)]) -> Config {
    let mut source = MapSource::new()
        .set("KAESO_PUBLIC_URL", ORIGIN)
        .set("KAESO_POSTGRES_URL", s.pg_url.expose_secret())
        .set("KAESO_CLICKHOUSE_URL", s.clickhouse_url.expose_secret())
        .set("KAESO_CLICKHOUSE_DATABASE", s.clickhouse_database.clone())
        .set("KAESO_VALKEY_URL", s.valkey_url.expose_secret())
        .set("KAESO_VALKEY_KEY_PREFIX", s.valkey_prefix.clone())
        .set(
            "KAESO_MASTER_KEYS",
            "k1:AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
        )
        .set("KAESO_MAIL_TRANSPORT", "log")
        .set("KAESO_MODELS_DEV_SYNC", "false");
    for (k, v) in extra {
        source = source.set(k, *v);
    }
    config::load(&source).unwrap()
}

pub async fn state(s: &TestServices) -> AppState {
    state_with(s, &[]).await
}

pub async fn state_with(s: &TestServices, extra: &[(&str, &str)]) -> AppState {
    kaeso_server::serve::prepare(config(s, extra))
        .await
        .unwrap()
}

pub fn manual_clock() -> (ManualClock, SharedClock) {
    let clock = ManualClock::new(
        time::OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .unwrap(),
    );
    (clock.clone(), Arc::new(clock))
}

pub struct App {
    pub state: AppState,
    pub router: Router,
    pub mail: MemorySink,
    pub clock: ManualClock,
}

#[derive(Debug)]
pub struct Resp {
    pub status: StatusCode,
    pub body: Value,
    pub headers: HeaderMap,
}

impl Resp {
    pub fn cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| c.strip_prefix("__Host-kaeso_session="))
            .and_then(|rest| rest.split(';').next())
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    }

    pub fn set_cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| {
                c.strip_prefix(&format!("{name}="))
                    .map(|rest| rest.split(';').next().unwrap_or("").to_owned())
            })
    }

    pub fn error_type(&self) -> &str {
        self.body["error"]["type"].as_str().unwrap_or("")
    }
}

impl App {
    pub async fn start(s: &TestServices) -> App {
        App::start_with(s, &[]).await
    }

    pub async fn start_with(s: &TestServices, extra: &[(&str, &str)]) -> App {
        App::start_hooked(s, extra, None, None).await
    }

    pub async fn start_hooked(
        s: &TestServices,
        extra: &[(&str, &str)],
        limits: Option<Arc<dyn kaeso_server::model_api::hooks::Limits>>,
        sink: Option<Arc<dyn kaeso_server::model_api::hooks::Sink>>,
    ) -> App {
        let (clock, shared) = manual_clock();
        let mail = MemorySink::new();
        let state = kaeso_server::serve::prepare_with(
            config(s, extra),
            Overrides {
                clock: Some(shared),
                mail: Some(kaeso_mail::Transport::Memory(mail.clone())),
                limits,
                sink,
            },
        )
        .await
        .unwrap();
        App::from_state(state, mail, clock).await
    }

    pub async fn second_instance(&self, s: &TestServices) -> App {
        let state = kaeso_server::serve::prepare_with(
            config(s, &[]),
            Overrides {
                clock: Some(Arc::new(self.clock.clone())),
                mail: Some(kaeso_mail::Transport::Memory(self.mail.clone())),
                ..Overrides::default()
            },
        )
        .await
        .unwrap();
        App::from_state(state, self.mail.clone(), self.clock.clone()).await
    }

    async fn from_state(state: AppState, mail: MemorySink, clock: ManualClock) -> App {
        let router = kaeso_server::app::router(state.clone());
        for _ in 0..200 {
            if state.caches.is_subscribed() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        App {
            state,
            router,
            mail,
            clock,
        }
    }

    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> Resp {
        self.call_with(method, path, body, cookie, Some(ORIGIN))
            .await
    }

    pub async fn call_with(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        cookie: Option<&str>,
        origin: Option<&str>,
    ) -> Resp {
        let mut req = Request::builder()
            .method(method)
            .uri(format!("/platform/v1{path}"));
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, format!("__Host-kaeso_session={c}"));
        }
        if let Some(o) = origin {
            req = req.header(header::ORIGIN, o);
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        self.raw(req).await
    }

    pub async fn raw(&self, req: Request<Body>) -> Resp {
        let path = req.uri().path().to_owned();
        let resp = self.send(req).await;
        if path.starts_with("/platform/") {
            for message in [&resp.body["error"]["message"], &resp.body["message"]] {
                if let Some(m) = message.as_str()
                    && let Some(problem) = copy::sent_problem(m)
                {
                    panic!("{path}: message breaks the copy rule ({problem}): {m}");
                }
            }
        }
        resp
    }

    async fn send(&self, req: Request<Body>) -> Resp {
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = axum::body::to_bytes(res.into_body(), 16 << 20)
            .await
            .unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        Resp {
            status,
            body,
            headers,
        }
    }

    pub async fn with_key(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: &str,
    ) -> Resp {
        let req = Request::builder()
            .method(method)
            .uri(format!("/platform/v1{path}"))
            .header(header::AUTHORIZATION, format!("Bearer {key}"));
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        self.raw(req).await
    }

    pub async fn get(&self, path: &str, cookie: &str) -> Resp {
        self.call(Method::GET, path, None, Some(cookie)).await
    }

    pub async fn post(&self, path: &str, body: Value, cookie: Option<&str>) -> Resp {
        self.call(Method::POST, path, Some(body), cookie).await
    }

    pub async fn setup(&self) -> Owner {
        let url = kaeso_server::setup::issue_link(&self.state)
            .await
            .unwrap()
            .unwrap();
        let token = url.split("#token=").nth(1).unwrap();
        let r = self
            .post(
                "/auth/setup",
                json!({
                    "token": token,
                    "email": "owner@example.com",
                    "name": "Owner",
                    "password": "correct horse battery",
                    "org_name": "Acme",
                }),
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        let cookie = r.cookie().unwrap();
        let who = self.get("/auth/session", &cookie).await;
        Owner {
            cookie,
            user_id: r.body["user"]["id"].as_str().unwrap().to_owned(),
            org_id: who.body["memberships"][0]["org_id"]
                .as_str()
                .unwrap()
                .to_owned(),
        }
    }

    pub fn last_token(&self, to: &str) -> String {
        let mails = self.mail.to(to);
        let text = &mails.last().expect("an email").text;
        let start = text.find("#token=").expect("a link with a token") + 7;
        text[start..].split_whitespace().next().unwrap().to_owned()
    }

    pub async fn wait_for_mail(&self, to: &str, n: usize) {
        for _ in 0..200 {
            if self.mail.to(to).len() >= n {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!(
            "expected {n} emails to {to}, got {}",
            self.mail.to(to).len()
        );
    }
}

pub struct Owner {
    pub cookie: String,
    pub user_id: String,
    pub org_id: String,
}
