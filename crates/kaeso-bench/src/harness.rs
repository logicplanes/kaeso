use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use bytes::Bytes;
use hyper::Method;
use hyper::header::SET_COOKIE;
use kaeso_testkit::mock_provider::MockProvider;
use kaeso_testkit::services::TestServices;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

use crate::client::{Call, connect, read_all};
use crate::gateway::{READY, SETUP_TOKEN};
use crate::{Result, fail};

pub(crate) const MODEL: &str = "gpt-4o";
const MASTER_KEY: &str = "k1:AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";
const PASSWORD: &str = "correct horse battery staple";
const START_WAIT: Duration = Duration::from_secs(180);

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) idle_timeout: Duration,
    pub(crate) max_connections: usize,
}

pub(crate) struct Gateway {
    child: Child,
    stdin: Option<ChildStdin>,
    pub(crate) pid: u32,
    pub(crate) addr: SocketAddr,
    metrics: SocketAddr,
}

pub(crate) struct Install {
    pub(crate) gateway: Gateway,
    pub(crate) mock: MockProvider,
    mock_runtime: tokio::runtime::Runtime,
    pub(crate) mock_addr: SocketAddr,
    pub(crate) key: String,
    pub(crate) slow_key: String,
    services: TestServices,
    settings: Settings,
    log: PathBuf,
}

fn free_port() -> Result<SocketAddr> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?)
}

impl Install {
    pub(crate) async fn start(settings: &Settings, log: &Path) -> Result<Install> {
        let services = TestServices::start_always_dropped().await;
        // The mock answers on threads of its own, so the clients' load can't hold up its
        // accept loop: a full listen queue resets the gateway's connections to it.
        let mock_runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name("mock-provider")
            .enable_all()
            .build()?;
        let mock = mock_runtime.spawn(MockProvider::start()).await?;
        let mock_addr: SocketAddr = mock
            .base_url
            .trim_start_matches("http://")
            .parse()
            .map_err(|_| fail("the mock provider's address"))?;
        let (gateway, token) = Gateway::start(&services, &mock.base_url, settings, log).await?;
        let token = token.ok_or_else(|| fail("the gateway printed no setup link"))?;
        let (key, slow_key) = set_up(gateway.addr, &token).await?;
        Ok(Install {
            gateway,
            mock,
            mock_runtime,
            mock_addr,
            key,
            slow_key,
            services,
            settings: settings.clone(),
            log: log.to_owned(),
        })
    }

    // A fresh process for every scenario, so each one's memory starts from the same place
    // instead of what the allocator kept from the one before.
    pub(crate) async fn restart(&mut self) -> Result<()> {
        let (fresh, _) = Gateway::start(
            &self.services,
            &self.mock.base_url,
            &self.settings,
            &self.log,
        )
        .await?;
        std::mem::replace(&mut self.gateway, fresh).stop().await
    }

    pub(crate) async fn stop(self) -> Result<()> {
        let Install {
            gateway,
            services,
            mock_runtime,
            ..
        } = self;
        let stopped = gateway.stop().await;
        services.cleanup().await;
        mock_runtime.shutdown_background();
        stopped
    }
}

impl Gateway {
    async fn start(
        services: &TestServices,
        mock: &str,
        settings: &Settings,
        log: &Path,
    ) -> Result<(Gateway, Option<String>)> {
        let addr = free_port()?;
        let metrics = free_port()?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("gateway")
            .env_clear()
            .envs(
                ["PATH", "HOME", "LLVM_PROFILE_FILE", "TMPDIR"]
                    .into_iter()
                    .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_owned(), v))),
            )
            .env(
                "KAESO_PUBLIC_URL",
                format!("http://localhost:{}", addr.port()),
            )
            .env("KAESO_LISTEN", addr.to_string())
            .env("KAESO_METRICS_LISTEN", metrics.to_string())
            .env("KAESO_POSTGRES_URL", services.pg_url.expose_secret())
            .env(
                "KAESO_CLICKHOUSE_URL",
                services.clickhouse_url.expose_secret(),
            )
            .env("KAESO_CLICKHOUSE_DATABASE", &services.clickhouse_database)
            .env("KAESO_VALKEY_URL", services.valkey_url.expose_secret())
            .env("KAESO_VALKEY_KEY_PREFIX", &services.valkey_prefix)
            .env("KAESO_MASTER_KEYS", MASTER_KEY)
            .env("KAESO_MAIL_TRANSPORT", "log")
            .env("KAESO_KPMR_SYNC", "false")
            .env("KAESO_UPDATES_CHECK", "false")
            .env("KAESO_PROVIDERS_OPENAI_BASE_URL", mock)
            .env(
                "KAESO_SERVER_MAX_CONNECTIONS",
                settings.max_connections.to_string(),
            )
            .env(
                "KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT",
                format!("{}ms", settings.idle_timeout.as_millis()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| fail("the gateway exited at once"))?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| fail("no gateway output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| fail("no gateway output"))?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .await?;
        let mut lines = BufReader::new(stdout).lines();
        let token = tokio::time::timeout(START_WAIT, async {
            while let Some(line) = lines.next_line().await? {
                if let Some(rest) = line.strip_prefix(READY) {
                    return Ok(rest.strip_prefix(SETUP_TOKEN).map(str::to_owned));
                }
                file.write_all(line.as_bytes()).await?;
                file.write_all(b"\n").await?;
            }
            Err(fail(&format!(
                "the gateway stopped before it was ready; see {}",
                log.display()
            )))
        })
        .await
        .map_err(|_| fail("the gateway did not start in time"))??;
        tokio::spawn(async move {
            let mut err = BufReader::new(stderr).lines();
            loop {
                let line = tokio::select! {
                    l = lines.next_line() => l,
                    l = err.next_line() => l,
                };
                let Ok(Some(line)) = line else { break };
                if file.write_all(line.as_bytes()).await.is_err()
                    || file.write_all(b"\n").await.is_err()
                {
                    break;
                }
            }
        });
        let gateway = Gateway {
            child,
            stdin,
            pid,
            addr,
            metrics,
        };
        gateway.wait_ready().await?;
        Ok((gateway, token))
    }

    async fn wait_ready(&self) -> Result<()> {
        let deadline = Instant::now() + START_WAIT;
        loop {
            let ready = async {
                let mut conn = connect(self.addr, None).await?;
                let resp = conn
                    .send(Call {
                        method: Method::GET,
                        path: "/healthz",
                        bearer: None,
                        origin: None,
                        cookie: None,
                        body: None,
                    })
                    .await?;
                Ok::<_, crate::Error>(resp.status().is_success())
            };
            if matches!(ready.await, Ok(true)) {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(fail("the gateway did not answer /healthz in time"));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub(crate) async fn metrics(&self) -> Result<String> {
        let mut conn = connect(self.metrics, None).await?;
        let resp = conn
            .send(Call {
                method: Method::GET,
                path: "/metrics",
                bearer: None,
                origin: None,
                cookie: None,
                body: None,
            })
            .await?;
        let (_, _, body) = read_all(resp).await?;
        Ok(String::from_utf8_lossy(&body).into_owned())
    }

    async fn stop(mut self) -> Result<()> {
        drop(self.stdin.take());
        match tokio::time::timeout(Duration::from_secs(60), self.child.wait()).await {
            Ok(status) if status.as_ref().is_ok_and(|s| s.success()) => Ok(()),
            Ok(_) => Err(fail("the gateway stopped with an error")),
            Err(_) => {
                self.child.kill().await?;
                Err(fail("the gateway did not stop within a minute"))
            }
        }
    }
}

pub(crate) fn by_label(metrics: &str, name: &str, label: &str) -> BTreeMap<String, u64> {
    let prefix = format!("{name}{{");
    let key = format!("{label}=\"");
    let mut out = BTreeMap::new();
    for line in metrics.lines() {
        let Some((labels, value)) = line
            .strip_prefix(&prefix)
            .and_then(|rest| rest.split_once("} "))
        else {
            continue;
        };
        let Some(at) = labels.find(&key) else {
            continue;
        };
        let rest = labels.get(at + key.len()..).unwrap_or_default();
        let Some((name, _)) = rest.split_once('"') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<f64>() else {
            continue;
        };
        *out.entry(name.to_owned()).or_default() += value as u64;
    }
    out
}

struct Platform {
    addr: SocketAddr,
    origin: String,
    cookie: Option<String>,
}

impl Platform {
    async fn call(&mut self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut conn = connect(self.addr, None).await?;
        let path = format!("/platform/v1/{path}");
        let resp = conn
            .send(Call {
                method,
                path: &path,
                bearer: None,
                origin: Some(&self.origin),
                cookie: self.cookie.as_deref(),
                body: body.map(|b| Bytes::from(b.to_string())),
            })
            .await?;
        let (status, headers, body) = read_all(resp).await?;
        for value in headers.get_all(SET_COOKIE) {
            let Ok(text) = value.to_str() else { continue };
            let pair = text.split(';').next().unwrap_or_default();
            if pair.starts_with("__Host-kaeso_session=") || pair.starts_with("kaeso_session=") {
                self.cookie = Some(pair.to_owned());
            }
        }
        if !(200..300).contains(&status) {
            return Err(fail(&format!(
                "{path} answered {status}: {}",
                String::from_utf8_lossy(&body)
            )));
        }
        if body.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&body)?)
    }
}

fn text(v: &Value, pointer: &str) -> Result<String> {
    v.pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| fail(&format!("no {pointer} in {v}")))
}

async fn set_up(addr: SocketAddr, token: &str) -> Result<(String, String)> {
    let mut p = Platform {
        addr,
        origin: format!("http://localhost:{}", addr.port()),
        cookie: None,
    };
    p.call(
        Method::POST,
        "auth/setup",
        Some(json!({
            "token": token,
            "email": "owner@example.com",
            "name": "Owner",
            "org_name": "Benchmark",
            "password": PASSWORD,
        })),
    )
    .await?;
    let session = p.call(Method::GET, "auth/session", None).await?;
    let org = text(&session, "/memberships/0/org_id")?;
    let projects = p
        .call(Method::GET, &format!("orgs/{org}/projects"), None)
        .await?;
    let project = text(&projects, "/data/0/id")?;
    p.call(
        Method::POST,
        &format!("orgs/{org}/credentials"),
        Some(json!({ "provider": "openai", "name": "mock", "secret": "sk-bench-openai-credential-0001" })),
    )
    .await?;
    let key = serving_key(&mut p, &org, &project).await?;
    let slow = p
        .call(
            Method::POST,
            &format!("orgs/{org}/projects"),
            Some(json!({ "name": "Stalled apps" })),
        )
        .await?;
    let slow = text(&slow, "/id")?;
    // Slow streams send large deltas, and a project that logs content keeps every one Kaeso
    // delivered for the log; the memory check bounds what Kaeso buffers for a stalled app.
    p.call(
        Method::PATCH,
        &format!("orgs/{org}/projects/{slow}/logs/settings"),
        Some(json!({ "content_logging": false })),
    )
    .await?;
    let slow_key = serving_key(&mut p, &org, &slow).await?;
    Ok((key, slow_key))
}

async fn serving_key(p: &mut Platform, org: &str, project: &str) -> Result<String> {
    p.call(
        Method::POST,
        &format!("orgs/{org}/projects/{project}/models"),
        Some(json!({ "provider": "openai", "model_id": MODEL })),
    )
    .await?;
    let key = p
        .call(
            Method::POST,
            &format!("orgs/{org}/projects/{project}/keys"),
            Some(json!({ "name": "bench" })),
        )
        .await?;
    text(&key, "/secret")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_are_summed_by_one_label() {
        let text = "# TYPE kaeso_stream_failures_total counter\n\
            kaeso_stream_failures_total{kind=\"client_stalled\",provider=\"openai\"} 4\n\
            kaeso_stream_failures_total{kind=\"timeout\",provider=\"openai\"} 1\n\
            kaeso_stream_failures_total{kind=\"timeout\",provider=\"anthropic\"} 2\n\
            kaeso_requests_total{kind=\"timeout\"} 9\n";
        let kinds = by_label(text, "kaeso_stream_failures_total", "kind");
        assert_eq!(
            kinds.into_iter().collect::<Vec<_>>(),
            [("client_stalled".to_owned(), 4), ("timeout".to_owned(), 3)]
        );
    }
}
