#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{OriginalUri, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::future::BoxFuture;
use kaeso_update::manifest::Service;
use kaeso_update::protocol::{Job, JobState};
use kaeso_updater::compose::ComposeFile;
use kaeso_updater::docker::Docker;
use kaeso_updater::engine::{Engine, Settings};
use kaeso_updater::files::{MaintenanceFlag, StateFile};
use kaeso_updater::source::{Endpoints, Source};
use kaeso_updater::verify::{Subject, Verify, VerifyError, hex};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

pub const FROM: &str = "0.2.0";
pub const TO: &str = "0.2.1";
pub const COMMIT: &str = "5388ad0b98aa3d957fea99e200259842bf746680";
pub const UPDATER_HOST: &str = "updater0host";
pub const JOB: &str = "upd_01k7a3f8m2xq4v9c5r6t8w0yz1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Unhealthy,
    Starting,
    Crash,
    Restarting,
    Slow,
}

pub const SLOW_INSPECTIONS: u32 = 4;
pub const FROM_MIGRATIONS: &str = "20261017120001_updates";

#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub reference: String,
    pub labels: BTreeMap<String, String>,
    pub env: Vec<String>,
    pub running: bool,
    pub health: Health,
    pub inspected: u32,
    pub started: bool,
}

pub type Hook = Box<dyn Fn() + Send>;

#[derive(Default)]
pub struct Daemon {
    pub containers: Vec<Container>,
    pub images: HashMap<String, Value>,
    pub calls: Vec<String>,
    pub fail_pull: bool,
    pub pulled_version: Option<String>,
    pub new_health: HashMap<String, Health>,
    pub old_health_after_restart: Option<Health>,
    pub next_id: u32,
    pub created: Vec<Value>,
    pub on_start: Option<Hook>,
    pub version_failures: u32,
    pub fail_new_start: Option<String>,
}

pub fn index_of(service: Service) -> Vec<u8> {
    format!("{{\"index\":\"{}\"}}", service.as_str()).into_bytes()
}

pub fn digest_of(service: Service) -> String {
    format!("sha256:{}", hex(&Sha256::digest(index_of(service))))
}

fn image_doc(id: &str, version: &str, reference: &str) -> Value {
    json!({
        "Id": id,
        "RepoDigests": [reference],
        "Config": {
            "Env": ["PATH=/usr/bin"],
            "Labels": { "org.opencontainers.image.version": version },
            "Healthcheck": { "Test": ["CMD", "check"] }
        }
    })
}

impl Daemon {
    pub fn install() -> Daemon {
        let mut d = Daemon::default();
        for service in [Service::Gateway, Service::Console, Service::Updater] {
            let reference = format!("{}:{FROM}", service.image());
            let id = format!("sha256:old-{}", service.as_str());
            d.images
                .insert(id.clone(), image_doc(&id, FROM, &reference));
            let container_id = if service == Service::Updater {
                format!("{UPDATER_HOST}{}", "0".repeat(52))
            } else {
                format!("{:0<64}", format!("{}old", service.as_str()))
            };
            d.containers.push(Container {
                id: container_id,
                name: format!("kaeso-{}-1", service.as_str()),
                image: id,
                reference,
                labels: BTreeMap::from([
                    ("com.docker.compose.project".into(), "kaeso".into()),
                    ("com.docker.compose.service".into(), service.as_str().into()),
                    ("com.docker.compose.oneoff".into(), "False".into()),
                ]),
                env: vec![
                    "PATH=/usr/bin".into(),
                    "KAESO_DOMAIN=kaeso.example.com".into(),
                ],
                running: true,
                health: Health::Healthy,
                inspected: 0,
                started: true,
            });
        }
        d
    }

    pub fn container(&self, name: &str) -> Option<&Container> {
        self.containers.iter().find(|c| c.name == name)
    }

    fn find(&mut self, key: &str) -> Option<&mut Container> {
        self.containers
            .iter_mut()
            .find(|c| c.id == key || c.id.starts_with(key) || c.name == key)
    }

    fn inspect(c: &mut Container) -> Value {
        c.inspected += 1;
        let health = match c.health {
            Health::Healthy => "healthy",
            Health::Unhealthy => "unhealthy",
            Health::Slow if c.inspected > SLOW_INSPECTIONS => "healthy",
            Health::Slow => "unhealthy",
            _ => "starting",
        };
        let restarts = u32::from(c.health == Health::Restarting);
        json!({
            "Id": c.id,
            "Name": format!("/{}", c.name),
            "Image": c.image,
            "RestartCount": restarts,
            "Config": {
                "Hostname": c.id.get(..12).unwrap_or(&c.id),
                "Image": c.reference,
                "Env": c.env,
                "Labels": c.labels,
                "Healthcheck": { "Test": ["CMD", "check"] }
            },
            "HostConfig": { "RestartPolicy": { "Name": "unless-stopped" }, "ReadonlyRootfs": true },
            "NetworkSettings": { "Networks": { "kaeso_edge": { "Aliases": [c.name, c.labels.get("com.docker.compose.service")] } } },
            "State": {
                "StartedAt": if c.started { "2026-10-17T10:00:00Z" } else { "0001-01-01T00:00:00Z" },
                "Running": c.running,
                "Restarting": false,
                "ExitCode": if c.running { 0 } else { 1 },
                "Health": { "Status": health }
            }
        })
    }
}

type Shared = Arc<Mutex<Daemon>>;

fn query(uri: &axum::http::Uri) -> HashMap<String, String> {
    url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .into_owned()
        .collect()
}

async fn daemon(
    State(d): State<Shared>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    let path = uri.path().strip_prefix("/v1.44/").unwrap_or("").to_owned();
    let q = query(&uri);
    let mut d = d.lock().unwrap();
    d.calls.push(format!("{method} {path}"));
    let not_found = || {
        (
            StatusCode::NOT_FOUND,
            axum::Json(json!({ "message": "no such object" })),
        )
            .into_response()
    };
    match (method.as_str(), path.as_str()) {
        ("GET", "version") if d.version_failures > 0 => {
            d.version_failures -= 1;
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        ("GET", "version") => axum::Json(json!({ "Version": "29.0.0" })).into_response(),
        ("GET", "containers/json") => {
            let filters: Value =
                serde_json::from_str(q.get("filters").map(String::as_str).unwrap_or("{}")).unwrap();
            let wanted: Vec<(String, String)> = filters["label"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    let (k, v) = l.as_str().unwrap().split_once('=').unwrap();
                    (k.to_owned(), v.to_owned())
                })
                .collect();
            let list: Vec<Value> = d
                .containers
                .iter()
                .filter(|c| wanted.iter().all(|(k, v)| c.labels.get(k) == Some(v)))
                .map(|c| json!({ "Id": c.id, "Names": [format!("/{}", c.name)] }))
                .collect();
            axum::Json(Value::Array(list)).into_response()
        }
        ("POST", "images/create") => {
            if d.fail_pull {
                return (
                    StatusCode::OK,
                    "{\"status\":\"Pulling\"}\n{\"error\":\"denied\"}\n",
                )
                    .into_response();
            }
            let name = q["fromImage"].clone();
            let digest = q["tag"].clone();
            let reference = format!("{name}@{digest}");
            let service = name.rsplit('-').next().unwrap().to_owned();
            let version = d.pulled_version.clone().unwrap_or_else(|| TO.to_owned());
            let id = format!("sha256:new-{service}");
            d.images
                .insert(reference.clone(), image_doc(&id, &version, &reference));
            d.images
                .insert(id.clone(), image_doc(&id, &version, &reference));
            (StatusCode::OK, "{\"status\":\"Downloaded\"}\n").into_response()
        }
        ("POST", "containers/create") => {
            let name = q["name"].clone();
            if d.container(&name).is_some() {
                return (
                    StatusCode::CONFLICT,
                    axum::Json(json!({ "message": "name in use" })),
                )
                    .into_response();
            }
            let spec: Value = serde_json::from_slice(&body).unwrap();
            d.created.push(spec.clone());
            let reference = spec["Image"].as_str().unwrap().to_owned();
            let Some(image) = d.images.get(&reference).cloned() else {
                return not_found();
            };
            let labels: BTreeMap<String, String> =
                serde_json::from_value(spec["Labels"].clone()).unwrap();
            let service = labels["com.docker.compose.service"].clone();
            let health = d
                .new_health
                .get(&service)
                .copied()
                .unwrap_or(Health::Healthy);
            d.next_id += 1;
            let id = format!("{:0<64}", format!("new{}", d.next_id));
            let mut env: Vec<String> = serde_json::from_value(spec["Env"].clone()).unwrap();
            env.extend(["PATH=/usr/bin".to_owned()]);
            d.containers.push(Container {
                id: id.clone(),
                name,
                image: image["Id"].as_str().unwrap().to_owned(),
                reference,
                labels,
                env,
                running: false,
                health,
                inspected: 0,
                started: false,
            });
            (StatusCode::CREATED, axum::Json(json!({ "Id": id }))).into_response()
        }
        ("GET", p) if p.starts_with("images/") && p.ends_with("/json") => {
            let reference = &p["images/".len()..p.len() - "/json".len()];
            match d.images.get(reference) {
                Some(doc) => axum::Json(doc.clone()).into_response(),
                None => not_found(),
            }
        }
        ("POST", p) if p.starts_with("images/") && p.ends_with("/tag") => {
            StatusCode::CREATED.into_response()
        }
        (m, p) if p.starts_with("containers/") => {
            let rest = &p["containers/".len()..];
            let (key, action) = rest.split_once('/').unwrap_or((rest, ""));
            let restart_health = d.old_health_after_restart;
            let fail_start = d.fail_new_start.clone();
            if (m, action) == ("POST", "start")
                && key.starts_with("new")
                && let Some(hook) = &d.on_start
            {
                hook();
            }
            if m == "DELETE" {
                let before = d.containers.len();
                d.containers.retain(|c| c.id != key);
                return if before == d.containers.len() {
                    not_found()
                } else {
                    StatusCode::NO_CONTENT.into_response()
                };
            }
            let new_name = q.get("name").cloned();
            let taken = new_name
                .as_ref()
                .is_some_and(|n| d.container(n).is_some_and(|c| c.id != key));
            let Some(c) = d.find(key) else {
                return not_found();
            };
            match (m, action) {
                ("GET", "json") => axum::Json(Daemon::inspect(c)).into_response(),
                ("POST", "start") => {
                    if c.id.contains("old")
                        && let Some(h) = restart_health
                    {
                        c.health = h;
                    }
                    c.running = c.health != Health::Crash;
                    c.started = true;
                    c.inspected = 0;
                    if c.id.starts_with("new")
                        && fail_start.as_deref()
                            == c.labels
                                .get("com.docker.compose.service")
                                .map(String::as_str)
                    {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(json!({ "message": "the connection broke" })),
                        )
                            .into_response();
                    }
                    StatusCode::NO_CONTENT.into_response()
                }
                ("POST", "stop") => {
                    c.running = false;
                    StatusCode::NO_CONTENT.into_response()
                }
                ("POST", "rename") if taken => (
                    StatusCode::CONFLICT,
                    axum::Json(json!({ "message": "name in use" })),
                )
                    .into_response(),
                ("POST", "rename") => {
                    c.name = new_name.unwrap();
                    StatusCode::NO_CONTENT.into_response()
                }
                _ => not_found(),
            }
        }
        _ => not_found(),
    }
}

#[derive(Clone)]
pub struct Release {
    pub manifest: Vec<u8>,
    pub compose: Vec<u8>,
}

#[derive(Default)]
pub struct Releases {
    pub by_version: HashMap<String, Release>,
}

impl Releases {
    pub fn add(&mut self, version: &str, shape: &Shape<'_>) {
        let compose = compose_text(version);
        self.by_version.insert(
            version.to_owned(),
            Release {
                manifest: manifest_for(version, &compose, shape),
                compose,
            },
        );
    }

    pub fn standard() -> Releases {
        let mut releases = Releases::default();
        releases.add(TO, &Shape::default());
        releases.add(FROM, &Shape::default());
        releases.add("1.0.0", &Shape::default());
        releases.add(
            "0.3.0",
            &Shape {
                manual: true,
                ..Shape::default()
            },
        );
        releases.add(
            "0.2.2",
            &Shape {
                manual_since: Some("0.2.1"),
                ..Shape::default()
            },
        );
        releases
    }
}

async fn release_server(
    State(r): State<Arc<Mutex<Releases>>>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let path = uri.path().to_owned();
    if let Some(rest) = path.strip_prefix("/releases/v") {
        let (version, file) = rest.split_once('/').unwrap();
        let r = r.lock().unwrap();
        return match (r.by_version.get(version), file) {
            (Some(rel), "release.json") => rel.manifest.clone().into_response(),
            (Some(rel), "compose.yaml") => rel.compose.clone().into_response(),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if path == "/registry/token" {
        return axum::Json(json!({ "token": "anonymous" })).into_response();
    }
    if let Some(rest) = path.strip_prefix("/registry/v2/logicplanes/kaeso-") {
        let service = rest.split('/').next().unwrap();
        let service = Service::ALL
            .into_iter()
            .find(|s| s.as_str() == service)
            .unwrap();
        return index_of(service).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

#[derive(Default)]
pub struct Verifier {
    pub reject: Option<String>,
    pub seen: Mutex<Vec<String>>,
}

impl Verify for Verifier {
    fn verify<'a>(&'a self, subject: Subject<'a>) -> BoxFuture<'a, Result<(), VerifyError>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(subject.name.to_owned());
            assert_eq!(subject.commit, COMMIT);
            if self.reject.as_deref() == Some(subject.name) {
                return Err(VerifyError::Untrusted {
                    name: subject.name.to_owned(),
                    why: "not signed by the release workflow".into(),
                });
            }
            Ok(())
        })
    }
}

pub fn compose_text(version: &str) -> Vec<u8> {
    format!(
        "name: kaeso\nservices:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:{version}\n  console:\n    image: ghcr.io/logicplanes/kaeso-console:{version}\n  updater:\n    image: ghcr.io/logicplanes/kaeso-updater:{version}\n    profiles: [updater]\n"
    )
    .into_bytes()
}

pub struct Shape<'a> {
    pub manual: bool,
    pub manual_since: Option<&'a str>,
    pub postgres: &'a str,
}

impl Default for Shape<'_> {
    fn default() -> Self {
        Shape {
            manual: false,
            manual_since: None,
            postgres: FROM_MIGRATIONS,
        }
    }
}

pub fn manifest_for(version: &str, compose: &[u8], shape: &Shape<'_>) -> Vec<u8> {
    let image = |s: Service| json!({ "name": s.image(), "digest": digest_of(s) });
    let since = if shape.manual {
        Some(version)
    } else {
        shape.manual_since
    };
    serde_json::to_vec(&json!({
        "schema": 1,
        "version": version,
        "commit": COMMIT,
        "manual": shape.manual,
        "manual_since": since,
        "migrations": { "postgres": shape.postgres, "clickhouse": "0002_usage_rollups" },
        "updater_protocol": 1,
        "images": {
            "gateway": image(Service::Gateway),
            "console": image(Service::Console),
            "updater": image(Service::Updater),
        },
        "compose": { "name": "compose.yaml", "sha256": hex(&Sha256::digest(compose)) },
    }))
    .unwrap()
}

pub struct World {
    pub daemon: Shared,
    pub releases: Arc<Mutex<Releases>>,
    pub verifier: Arc<Verifier>,
    pub dir: PathBuf,
    pub engine: Arc<Engine>,
}

async fn serve(router: Router) -> Url {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await });
    Url::parse(&format!("http://{addr}/")).unwrap()
}

pub fn settings(dir: &std::path::Path) -> Settings {
    Settings {
        compose: ComposeFile::new(dir.join("compose.yaml")),
        maintenance: MaintenanceFlag::new(dir.join("maintenance")),
        state: StateFile::new(dir.join("state")),
        hostname: UPDATER_HOST.into(),
        poll: Duration::from_millis(5),
        gateway_timeout: Duration::from_millis(400),
        console_timeout: Duration::from_millis(400),
        rollback_timeout: Duration::from_millis(400),
        docker_wait: Duration::from_millis(400),
        stop_seconds: 0,
    }
}

impl World {
    pub async fn new(name: &str, daemon: Daemon, verifier: Verifier) -> World {
        let dir = std::env::temp_dir().join(format!("kaeso-updater-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("maintenance")).unwrap();
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(dir.join("compose.yaml"), compose_text(FROM)).unwrap();
        World::with(dir, daemon, Releases::standard(), verifier).await
    }

    pub async fn with(
        dir: PathBuf,
        daemon: Daemon,
        releases: Releases,
        verifier: Verifier,
    ) -> World {
        let daemon = Arc::new(Mutex::new(daemon));
        let releases = Arc::new(Mutex::new(releases));
        let docker_url = serve(
            Router::new()
                .fallback(daemon_handler)
                .with_state(daemon.clone()),
        )
        .await;
        let source_url = serve(
            Router::new()
                .fallback(release_server)
                .with_state(releases.clone()),
        )
        .await;
        let source = Source::new(Endpoints {
            releases: source_url.join("releases/").unwrap(),
            attestations: source_url.join("attestations/").unwrap(),
            registry: source_url.join("registry/").unwrap(),
        })
        .unwrap();
        let verifier = Arc::new(verifier);
        let engine = Engine::new(
            settings(&dir),
            Docker::new(&docker_url).unwrap(),
            source,
            verifier.clone(),
        );
        World {
            daemon,
            releases,
            verifier,
            dir,
            engine,
        }
    }

    pub async fn finished(&self) -> Job {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(job) = self.engine.last().filter(|j| j.state != JobState::Running) {
                return job;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the update did not finish"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    pub fn compose(&self) -> Vec<u8> {
        std::fs::read(self.dir.join("compose.yaml")).unwrap()
    }

    pub fn maintenance(&self) -> bool {
        self.dir.join("maintenance/active").exists()
    }

    pub fn container(&self, name: &str) -> Option<Container> {
        self.daemon.lock().unwrap().container(name).cloned()
    }

    pub fn running(&self, name: &str) -> Container {
        let d = self.daemon.lock().unwrap();
        let c = d
            .container(name)
            .unwrap_or_else(|| panic!("no container {name}"))
            .clone();
        assert!(c.running, "{name} is not running");
        c
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .daemon
            .lock()
            .unwrap()
            .containers
            .iter()
            .map(|c| c.name.clone())
            .collect();
        names.sort();
        names
    }

    pub fn calls(&self, prefix: &str) -> Vec<String> {
        self.daemon
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|c| c.starts_with(prefix))
            .cloned()
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn daemon_handler(
    state: State<Shared>,
    method: Method,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    daemon(state, method, uri, body).await
}
