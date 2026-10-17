#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod copy;
pub mod flaky_valkey;
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
pub const PASSWORD: &str = "correct horse battery";

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

pub async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while !check().await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{what} did not happen within 10 seconds"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
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

    #[track_caller]
    pub fn expect_status(self, status: StatusCode) -> Resp {
        assert_eq!(self.status, status, "{:?}", self.body);
        self
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
        let valkey_reachable = !extra.iter().any(|(k, _)| *k == "KAESO_VALKEY_URL");
        App::from_state(state, mail, clock, valkey_reachable).await
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
        App::from_state(state, self.mail.clone(), self.clock.clone(), true).await
    }

    async fn from_state(
        state: AppState,
        mail: MemorySink,
        clock: ManualClock,
        valkey_reachable: bool,
    ) -> App {
        kaeso_server::services::models_dev::spawn_reloader(&state);
        let router = kaeso_server::app::router(state.clone());
        if valkey_reachable {
            eventually("the cache invalidation subscription", || {
                let subscribed = state.caches.is_subscribed();
                async move { subscribed }
            })
            .await;
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
        let (status, headers, bytes) = self.raw_bytes(req).await;
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Resp {
            status,
            body,
            headers,
        }
    }

    pub async fn raw_bytes(&self, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let path = req.uri().path().to_owned();
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = axum::body::to_bytes(res.into_body(), 16 << 20)
            .await
            .unwrap()
            .to_vec();
        if path.starts_with("/platform/")
            && let Ok(body) = serde_json::from_slice::<Value>(&bytes)
        {
            for message in [&body["error"]["message"], &body["message"]] {
                if let Some(m) = message.as_str()
                    && let Some(problem) = copy::sent_problem(m)
                {
                    panic!("{path}: message breaks the copy rule ({problem}): {m}");
                }
            }
        }
        (status, headers, bytes)
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
        let mut body = json!({
            "token": token,
            "email": "owner@example.com",
            "name": "Owner",
            "org_name": "Acme",
        });
        if self.state.config.auth.password {
            body["password"] = json!(PASSWORD);
        }
        let r = self.post("/auth/setup", body, None).await;
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

pub fn uuid(public: &str) -> uuid::Uuid {
    let raw = public.split_once('_').unwrap().1;
    uuid::Uuid::from_u128(kaeso_core::id::decode_base32(raw).unwrap())
}

fn base32_decode(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = ALPHABET.iter().position(|a| *a == c).unwrap() as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

pub fn totp_code(secret: &str, app: &App) -> String {
    use kaeso_core::clock::Clock;
    let now = app.clock.now().unix_timestamp() as u64;
    let secret = kaeso_crypto::SecretBox::new(Box::new(base32_decode(secret)));
    kaeso_crypto::totp::code_at(&secret, now).unwrap()
}

pub async fn join(app: &App, owner: &Owner, invite: Value, name: &str) -> (String, String) {
    let email = kaeso_core::email::normalize(invite["email"].as_str().unwrap()).unwrap();
    let before = app.mail.to(&email).len();
    app.post(
        &format!("/orgs/{}/invites", owner.org_id),
        invite,
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::CREATED);
    app.wait_for_mail(&email, before + 1).await;
    let accepted = app
        .post(
            "/auth/invites/accept",
            json!({ "token": app.last_token(&email), "name": name, "password": PASSWORD }),
            None,
        )
        .await
        .expect_status(StatusCode::OK);
    (
        accepted.cookie().unwrap(),
        accepted.body["user"]["id"].as_str().unwrap().to_owned(),
    )
}
