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
use futures::StreamExt;
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

pub const FROM: &str = "40.2.0";
pub const TO: &str = "40.2.1";
pub const COMMIT: &str = "5388ad0b98aa3d957fea99e200259842bf746680";
pub const UPDATER_HOST: &str = "updater0host";
pub const JOB: &str = "upd_01k7a3f8m2xq4v9c5r6t8w0yz1";
pub const INSTALL_DIR: &str = "/srv/kaeso";
pub const RUNNER_LABEL: &str = "app.kaeso.updater.runner";

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
pub const FROM_MIGRATIONS: &str = "20261017140001_updates";

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
    pub restarts: u32,
    pub stop_timeout: Option<u32>,
    pub networks: Vec<String>,
    pub mounts: Value,
    pub exit_code: Option<i64>,
    pub logs: String,
    pub cmd: Vec<String>,
}

impl Container {
    pub fn task(&self) -> Option<&str> {
        self.labels.get(RUNNER_LABEL)?;
        self.labels
            .get("app.kaeso.updater.task")
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Exit(i64, &'static str),
    Hang,
}

pub type Hook = Box<dyn Fn() + Send>;

#[derive(Default)]
pub struct Daemon {
    pub containers: Vec<Container>,
    pub images: HashMap<String, Value>,
    pub calls: Vec<String>,
    pub stops: Vec<(String, String)>,
    pub fail_pull: bool,
    pub pulled_version: Option<String>,
    pub new_health: HashMap<String, Health>,
    pub old_health_after_restart: Option<Health>,
    pub next_id: u32,
    pub created: Vec<Value>,
    pub on_start: Option<Hook>,
    pub version_failures: u32,
    pub fail_new_start: Option<String>,
    pub fail_removals: u32,
    pub fail_rename_back: bool,
    pub hang: Vec<&'static str>,
    pub stall_pull: bool,
    pub compose_path: Option<PathBuf>,
    pub state_path: Option<PathBuf>,
    pub runners: HashMap<&'static str, Outcome>,
    pub ran: Vec<(String, String)>,
    pub replace_saw: Option<String>,
    pub next_runner: u32,
}

pub fn runner(job: &str, task: &str, code: Option<i64>) -> Container {
    Container {
        id: format!("{:0<64}", format!("runner{task}")),
        name: format!("runner-{task}"),
        image: "sha256:new-updater".into(),
        reference: format!(
            "{}@{}",
            Service::Updater.image(),
            digest_of(Service::Updater)
        ),
        labels: BTreeMap::from([
            (RUNNER_LABEL.into(), job.into()),
            ("app.kaeso.updater.task".into(), task.into()),
        ]),
        env: Vec::new(),
        running: code.is_none(),
        health: Health::Healthy,
        inspected: 0,
        started: true,
        restarts: 0,
        stop_timeout: None,
        networks: vec!["kaeso_docker".into()],
        mounts: json!([]),
        exit_code: code,
        logs: format!("{task} said its last words\n"),
        cmd: Vec::new(),
    }
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

fn switched(c: &Container) -> bool {
    c.task().is_none()
        && matches!(
            c.labels
                .get("com.docker.compose.service")
                .map(String::as_str),
            Some("gateway" | "console")
        )
        && c.labels
            .get("com.docker.compose.oneoff")
            .map(String::as_str)
            == Some("False")
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
                restarts: 0,
                stop_timeout: None,
                networks: if service == Service::Updater {
                    vec!["kaeso_updates".into(), "kaeso_docker".into(), "kaeso_egress".into()]
                } else {
                    vec!["kaeso_edge".into()]
                },
                mounts: if service == Service::Updater {
                    json!([{ "Type": "bind", "Source": format!("{INSTALL_DIR}/compose.yaml"), "Destination": "/kaeso/compose.yaml" }])
                } else {
                    json!([])
                },
                exit_code: None,
                logs: String::new(),
                cmd: Vec::new(),
            });
        }
        let mut proxy = d.container("kaeso-updater-1").unwrap().clone();
        proxy.id = format!("{:0<64}", "proxy");
        proxy.name = "kaeso-docker-proxy-1".into();
        proxy.image = "sha256:proxy".into();
        proxy.reference = "wollomatic/socket-proxy:1.13.2".into();
        proxy
            .labels
            .insert("com.docker.compose.service".into(), "docker-proxy".into());
        proxy.networks = vec!["kaeso_docker".into()];
        proxy.mounts = json!([]);
        d.containers.push(proxy);
        d
    }

    pub fn runners(&self) -> Vec<Container> {
        self.containers
            .iter()
            .filter(|c| c.task().is_some())
            .cloned()
            .collect()
    }

    fn run_compose(&mut self, task: &str) -> (i64, String) {
        let outcome = self.runners.get(task).cloned();
        if let Some(Outcome::Exit(code, logs)) = outcome {
            return (code, logs.to_owned());
        }
        let compose = self
            .compose_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let new = compose.contains(&format!(":{TO}@"));
        match task {
            "stop" => {
                for c in self.containers.iter_mut().filter(|c| switched(c)) {
                    c.running = false;
                }
                (0, " Container kaeso-gateway-1 Stopped\n".into())
            }
            "up" => {
                let mut failed = None;
                for i in 0..self.containers.len() {
                    if !switched(&self.containers[i]) {
                        continue;
                    }
                    self.next_id += 1;
                    let id = format!("{:0<64}", format!("up{}", self.next_id));
                    let health = if new {
                        let service =
                            self.containers[i].labels["com.docker.compose.service"].clone();
                        self.new_health
                            .get(&service)
                            .copied()
                            .unwrap_or(Health::Healthy)
                    } else {
                        Health::Healthy
                    };
                    let c = &mut self.containers[i];
                    let service = c.labels["com.docker.compose.service"].clone();
                    let version = if new { "new" } else { "old" };
                    c.id = id;
                    c.image = format!("sha256:{version}-{service}");
                    c.health = health;
                    c.running = health != Health::Crash;
                    c.started = true;
                    c.inspected = 0;
                    if health != Health::Healthy {
                        failed = Some(format!("container {} is unhealthy", c.name));
                    }
                }
                match failed {
                    Some(why) => (1, format!(" Container kaeso-gateway-1 Started\n{why}\n")),
                    None => (0, " Container kaeso-gateway-1 Healthy\n".into()),
                }
            }
            "replace" => {
                self.replace_saw = self
                    .state_path
                    .as_ref()
                    .and_then(|p| std::fs::read_to_string(p).ok());
                (0, String::new())
            }
            _ => (0, String::new()),
        }
    }

    pub fn container(&self, name: &str) -> Option<&Container> {
        self.containers.iter().find(|c| c.name == name)
    }

    pub fn add_pulled_image(&mut self, service: Service) {
        let reference = format!("{}@{}", service.image(), digest_of(service));
        let id = format!("sha256:new-{}", service.as_str());
        self.images
            .insert(reference.clone(), image_doc(&id, TO, &reference));
        self.images
            .insert(id.clone(), image_doc(&id, TO, &reference));
    }

    pub fn add_gateway_replica(&mut self, n: usize) {
        let mut replica = self.container("kaeso-gateway-1").unwrap().clone();
        replica.id = format!("{:0<64}", format!("gateway{n}old"));
        replica.name = format!("kaeso-gateway-{n}");
        self.containers.push(replica);
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
        let restarts = c.restarts + u32::from(c.health == Health::Restarting);
        let mut doc = json!({
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
            "NetworkSettings": { "Networks": c.networks.iter().map(|n| (n.clone(), json!({ "Aliases": [c.name, c.labels.get("com.docker.compose.service")] }))).collect::<serde_json::Map<String, Value>>() },
            "Mounts": c.mounts,
            "State": {
                "StartedAt": if c.started { "2026-10-17T10:00:00Z" } else { "0001-01-01T00:00:00Z" },
                "Running": c.running,
                "Restarting": false,
                "ExitCode": c.exit_code.unwrap_or(if c.running { 0 } else { 1 }),
                "Health": { "Status": health }
            }
        });
        if let Some(seconds) = c.stop_timeout {
            doc["Config"]["StopTimeout"] = json!(seconds);
        }
        doc
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
    if path.ends_with("/stop") {
        let seconds = q.get("t").cloned().unwrap_or_default();
        d.stops.push((path.clone(), seconds));
    }
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
            let wanted: Vec<(String, Option<String>)> = filters["label"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| match l.as_str().unwrap().split_once('=') {
                    Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
                    None => (l.as_str().unwrap().to_owned(), None),
                })
                .collect();
            let list: Vec<Value> = d
                .containers
                .iter()
                .filter(|c| {
                    wanted.iter().all(|(k, v)| match v {
                        Some(v) => c.labels.get(k) == Some(v),
                        None => c.labels.contains_key(k),
                    })
                })
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
        ("POST", "containers/create") if !q.contains_key("name") => {
            let spec: Value = serde_json::from_slice(&body).unwrap();
            d.created.push(spec.clone());
            let labels: BTreeMap<String, String> =
                serde_json::from_value(spec["Labels"].clone()).unwrap();
            d.next_runner += 1;
            let n = d.next_runner;
            let id = format!("{:0<64}", format!("runner{n}"));
            d.containers.push(Container {
                id: id.clone(),
                name: format!("runner-{n}"),
                image: "sha256:new-updater".into(),
                reference: spec["Image"].as_str().unwrap().to_owned(),
                labels,
                env: serde_json::from_value(spec["Env"].clone()).unwrap(),
                running: false,
                health: Health::Healthy,
                inspected: 0,
                started: false,
                restarts: 0,
                stop_timeout: None,
                networks: vec![
                    spec["HostConfig"]["NetworkMode"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                ],
                mounts: json!([]),
                exit_code: None,
                logs: String::new(),
                cmd: serde_json::from_value(spec["Cmd"].clone()).unwrap(),
            });
            (StatusCode::CREATED, axum::Json(json!({ "Id": id }))).into_response()
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
                restarts: 0,
                stop_timeout: spec["StopTimeout"]
                    .as_u64()
                    .map(|s| u32::try_from(s).unwrap()),
                networks: vec!["kaeso_edge".into()],
                mounts: json!([]),
                exit_code: None,
                logs: String::new(),
                cmd: Vec::new(),
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
            if let Some(i) = d
                .containers
                .iter()
                .position(|c| c.id == key && c.task().is_some())
            {
                match (m, action) {
                    ("POST", "start") => {
                        let task = d.containers[i].task().unwrap().to_owned();
                        let job = d.containers[i].labels[RUNNER_LABEL].clone();
                        d.ran.push((job, task.clone()));
                        d.containers[i].started = true;
                        if d.runners.get(task.as_str()) == Some(&Outcome::Hang) {
                            d.containers[i].running = true;
                        } else {
                            let (code, logs) = d.run_compose(&task);
                            let c = &mut d.containers[i];
                            c.running = false;
                            c.exit_code = Some(code);
                            c.logs = logs;
                        }
                        return StatusCode::NO_CONTENT.into_response();
                    }
                    ("GET", "logs") => {
                        let logs = d.containers[i].logs.clone().into_bytes();
                        let mut framed = vec![1, 0, 0, 0];
                        framed.extend_from_slice(&u32::try_from(logs.len()).unwrap().to_be_bytes());
                        framed.extend_from_slice(&logs);
                        return framed.into_response();
                    }
                    _ => {}
                }
            }
            let restart_health = d.old_health_after_restart;
            let fail_start = d.fail_new_start.clone();
            if (m, action) == ("POST", "start")
                && key.starts_with("new")
                && let Some(hook) = &d.on_start
            {
                hook();
            }
            if m == "DELETE" && d.fail_removals > 0 {
                d.fail_removals -= 1;
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(json!({ "message": "device or resource busy" })),
                )
                    .into_response();
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
            let refuse_rename = d.fail_rename_back
                && action == "rename"
                && new_name
                    .as_deref()
                    .is_some_and(|n| !n.ends_with("-previous"));
            if refuse_rename {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(json!({ "message": "rename refused" })),
                )
                    .into_response();
            }
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
        let compose = release_compose(version);
        self.by_version.insert(
            version.to_owned(),
            Release {
                manifest: manifest_for(version, &compose, shape),
                compose,
            },
        );
    }

    pub fn add_changed(&mut self, version: &str, shape: &Shape<'_>) {
        let compose = changed_compose(version);
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
        releases.add("41.0.0", &Shape::default());
        releases.add(
            "40.3.0",
            &Shape {
                manual: true,
                ..Shape::default()
            },
        );
        releases.add(
            "40.2.2",
            &Shape {
                manual_since: Some("40.2.1"),
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

pub fn release_compose(version: &str) -> Vec<u8> {
    let image = |s: Service| format!("{}:{version}@{}", s.image(), digest_of(s));
    format!(
        "name: kaeso\nservices:\n  gateway:\n    image: {}\n  console:\n    image: {}\n  updater:\n    image: {}\n    profiles: [updater]\n",
        image(Service::Gateway),
        image(Service::Console),
        image(Service::Updater),
    )
    .into_bytes()
}

pub fn changed_compose(version: &str) -> Vec<u8> {
    let mut compose = release_compose(version);
    compose.extend_from_slice(
        format!(
            "  mailer:\n    image: example/mailer@sha256:{}\n",
            "e".repeat(64)
        )
        .as_bytes(),
    );
    compose
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
    pub docker_url: Url,
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
        install_dir: Some(INSTALL_DIR.into()),
        poll: Duration::from_millis(5),
        gateway_timeout: Duration::from_millis(400),
        console_timeout: Duration::from_millis(400),
        rollback_timeout: Duration::from_millis(400),
        docker_wait: Duration::from_millis(400),
        compose_timeout: Duration::from_millis(400),
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
        let settings = settings(&dir);
        World::configured(dir, daemon, releases, verifier, settings).await
    }

    pub async fn configured(
        dir: PathBuf,
        daemon: Daemon,
        releases: Releases,
        verifier: Verifier,
        settings: Settings,
    ) -> World {
        let mut daemon = daemon;
        daemon.compose_path = Some(dir.join("compose.yaml"));
        daemon.state_path = Some(dir.join("state").join("job.json"));
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
            settings,
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
            docker_url,
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
            .filter(|c| {
                c.task().is_none()
                    && c.labels
                        .get("com.docker.compose.service")
                        .map(String::as_str)
                        != Some("docker-proxy")
            })
            .map(|c| c.name.clone())
            .collect();
        names.sort();
        names
    }

    pub async fn replaced(&self) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !self.ran().iter().any(|t| t == "replace") {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the updater did not replace itself"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    pub fn ran(&self) -> Vec<String> {
        self.daemon
            .lock()
            .unwrap()
            .ran
            .iter()
            .map(|(_, task)| task.clone())
            .collect()
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
    let call = format!(
        "{method} {}",
        uri.path().strip_prefix("/v1.44/").unwrap_or("")
    );
    let (hang, stall) = {
        let mut d = state.0.lock().unwrap();
        let hang = d.hang.iter().any(|h| call.starts_with(h));
        let stall = d.stall_pull && call == "POST images/create";
        if hang || stall {
            d.calls.push(call);
        }
        (hang, stall)
    };
    if hang {
        std::future::pending::<()>().await;
    }
    if stall {
        let first = futures::stream::once(async {
            Ok::<_, std::io::Error>(Bytes::from_static(b"{\"status\":\"Pulling\"}\n"))
        });
        return axum::body::Body::from_stream(first.chain(futures::stream::pending()))
            .into_response();
    }
    daemon(state, method, uri, body).await
}
