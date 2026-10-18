#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use common::{App, Owner};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

static TOKEN_TEXT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| "t".repeat(32));

fn token() -> &'static str {
    TOKEN_TEXT.as_str()
}
const RUNNING: &str = env!("CARGO_PKG_VERSION");

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn next_patch() -> String {
    let v = semver::Version::parse(RUNNING).unwrap();
    format!("{}.{}.{}", v.major, v.minor, v.patch + 1)
}

fn next_minor_rc() -> String {
    let v = semver::Version::parse(RUNNING).unwrap();
    format!("{}.{}.0-rc.1", v.major, v.minor + 1)
}

fn next_major() -> String {
    let v = semver::Version::parse(RUNNING).unwrap();
    format!("{}.0.0", v.major + 1)
}

fn manifest(version: &str, manual: bool) -> Value {
    let digest = format!("sha256:{}", hex(&Sha256::digest(version.as_bytes())));
    let image =
        |s: &str| json!({ "name": format!("ghcr.io/logicplanes/kaeso-{s}"), "digest": digest });
    json!({
        "schema": 1,
        "version": version,
        "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
        "manual": manual,
        "manual_since": if manual { json!(version) } else { Value::Null },
        "migrations": { "postgres": "20261017140001_updates", "clickhouse": "0002_usage_rollups" },
        "updater_protocol": 1,
        "images": { "gateway": image("gateway"), "console": image("console"), "updater": image("updater") },
        "compose": { "name": "compose.yaml", "sha256": hex(&Sha256::digest(b"compose")) },
    })
}

#[derive(Default)]
struct GitHub {
    releases: Vec<(String, bool, bool)>,
    status: Option<(u16, Option<&'static str>)>,
    requests: Vec<Option<String>>,
    base: String,
    download_base: Option<String>,
    downloads: u32,
    manual_since: Option<String>,
}

fn etag_of(releases: &[(String, bool, bool)]) -> String {
    let versions: Vec<String> = releases.iter().map(|(v, _, m)| format!("{v}{m}")).collect();
    format!("W/\"{}\"", versions.join(","))
}

async fn list(
    State(gh): State<Arc<Mutex<GitHub>>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let page: usize = url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .find(|(k, _)| k == "page")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(1);
    let mut gh = gh.lock().unwrap();
    let conditional = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    gh.requests.push(conditional.clone());
    if let Some((status, retry)) = gh.status {
        let mut resp = StatusCode::from_u16(status).unwrap().into_response();
        if let Some(r) = retry {
            resp.headers_mut()
                .insert(header::RETRY_AFTER, r.parse().unwrap());
        }
        return resp;
    }
    let etag = etag_of(&gh.releases);
    if conditional.as_deref() == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    let base = gh.download_base.clone().unwrap_or_else(|| gh.base.clone());
    let body: Vec<Value> = gh
        .releases
        .iter()
        .map(|(version, prerelease, _)| {
            json!({
                "tag_name": format!("v{version}"),
                "draft": false,
                "prerelease": prerelease,
                "published_at": "2026-10-20T10:00:00Z",
                "body": format!("## Added\n\n- Everything in {version}."),
                "html_url": format!("https://github.com/logicplanes/kaeso/releases/tag/v{version}"),
                "assets": [{ "name": "release.json", "browser_download_url": format!("{base}download/v{version}/release.json") }],
            })
        })
        .chain([json!({ "tag_name": "v9.9.9", "draft": true, "html_url": "x" }), json!({ "tag_name": "nightly", "html_url": "x" })])
        .collect();
    let pages: Vec<&[Value]> = body.chunks(50).collect();
    let this = pages.get(page - 1).map(|p| p.to_vec()).unwrap_or_default();
    let mut resp = ([(header::ETAG, etag)], axum::Json(this)).into_response();
    if page < pages.len() {
        let next = format!(
            "<{}repos/logicplanes/kaeso/releases?per_page=50&page={}>; rel=\"next\"",
            gh.base,
            page + 1
        );
        resp.headers_mut()
            .insert(header::LINK, next.parse().unwrap());
    }
    resp
}

async fn download(State(gh): State<Arc<Mutex<GitHub>>>, OriginalUri(uri): OriginalUri) -> Response {
    let mut gh = gh.lock().unwrap();
    gh.downloads += 1;
    let tag = uri.path().split('/').nth(2).unwrap_or_default().to_owned();
    let version = tag.trim_start_matches('v');
    match gh.releases.iter().find(|(v, _, _)| v == version) {
        Some((v, _, manual)) => {
            let mut m = manifest(v, *manual);
            if let Some(since) = gh.manual_since.clone() {
                m["manual_since"] = json!(since);
            }
            axum::Json(m).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn github(releases: Vec<(String, bool, bool)>) -> (Arc<Mutex<GitHub>>, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let gh = Arc::new(Mutex::new(GitHub {
        releases,
        base: base.clone(),
        ..GitHub::default()
    }));
    let router = Router::new()
        .route("/repos/logicplanes/kaeso/releases", get(list))
        .route("/download/{tag}/release.json", get(download))
        .with_state(gh.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });
    (gh, format!("{base}repos/logicplanes/kaeso/releases"))
}

#[derive(Default)]
struct Updater {
    started: Vec<Value>,
    finish_as: Option<&'static str>,
    refuse: bool,
    fail_start: Option<u16>,
    garbled: bool,
    down: bool,
    unauthorized: u32,
}

fn job(id: &str, to: &str, state: &str, step: &str, reason: Option<&str>) -> Value {
    json!({
        "id": id, "from": RUNNING, "to": to, "state": state, "step": step,
        "reason": reason, "detail": reason.map(|_| "the new gateway reported itself unhealthy"),
        "started_at": "2026-10-20T10:00:00Z",
        "finished_at": if state == "running" { Value::Null } else { json!("2026-10-20T10:01:00Z") },
    })
}

fn authorized(u: &mut Updater, headers: &HeaderMap) -> bool {
    let ok = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        == Some(&format!("Bearer {}", token()));
    if !ok {
        u.unauthorized += 1;
    }
    ok
}

async fn updater_status(State(u): State<Arc<Mutex<Updater>>>, headers: HeaderMap) -> Response {
    let mut u = u.lock().unwrap();
    if !authorized(&mut u, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    axum::Json(json!({ "version": "40.9.0", "protocol": 1, "docker": true, "job": null }))
        .into_response()
}

async fn updater_start(
    State(u): State<Arc<Mutex<Updater>>>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let mut u = u.lock().unwrap();
    if !authorized(&mut u, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if u.refuse {
        return StatusCode::CONFLICT.into_response();
    }
    if let Some(status) = u.fail_start {
        return StatusCode::from_u16(status).unwrap().into_response();
    }
    u.started.push(body.clone());
    if u.garbled {
        return (StatusCode::ACCEPTED, "{\"half").into_response();
    }
    let j = job(
        body["id"].as_str().unwrap(),
        body["version"].as_str().unwrap(),
        "running",
        "verifying",
        None,
    );
    (StatusCode::ACCEPTED, axum::Json(j)).into_response()
}

async fn updater_job(
    State(u): State<Arc<Mutex<Updater>>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let mut u = u.lock().unwrap();
    if !authorized(&mut u, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if u.down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let id = uri.path().rsplit('/').next().unwrap_or_default().to_owned();
    let Some(started) = u.started.iter().find(|s| s["id"] == id.as_str()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let to = started["version"].as_str().unwrap();
    let j = match u.finish_as {
        None => job(&id, to, "running", "migrating", None),
        Some("succeeded") => job(&id, to, "succeeded", "online", None),
        Some(other) => job(&id, to, other, "migrating", Some("unhealthy")),
    };
    axum::Json(j).into_response()
}

async fn updater() -> (Arc<Mutex<Updater>>, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let u = Arc::new(Mutex::new(Updater::default()));
    let router = Router::new()
        .route("/v1/status", get(updater_status))
        .route("/v1/updates", post(updater_start))
        .route("/v1/updates/{id}", get(updater_job))
        .with_state(u.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });
    (u, base)
}

fn stable() -> Vec<(String, bool, bool)> {
    vec![
        (RUNNING.to_owned(), false, false),
        (next_patch(), false, false),
        (next_minor_rc(), true, false),
    ]
}

async fn audit(app: &App, owner: &Owner, action: &str) -> Vec<Value> {
    app.get(
        &format!("/instance/audit-log?action={action}"),
        &owner.cookie,
    )
    .await
    .body["data"]
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn checking_finds_newer_releases_with_their_notes_and_asks_github_conditionally() {
    let s = TestServices::start().await;
    let (gh, url) = github(stable()).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;

    let before = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(before.status, StatusCode::OK, "{:?}", before.body);
    assert_eq!(before.body["state"], "unknown");
    assert_eq!(before.body["current"], RUNNING);
    assert_eq!(before.body["updater"]["state"], "not_installed");
    assert_eq!(
        before.body["settings"],
        json!({
            "check": false,
            "channel": "stable",
            "mode": "manual",
            "schedule": { "frequency": "daily", "weekday": "monday", "hour": 3, "timezone": "UTC" },
            "next_auto_update_at": null,
        })
    );
    assert_eq!(before.body["can_apply"], false);
    assert_eq!(
        before.body["upgrade_guide"],
        "https://docs.kaeso.app/self-hosting/upgrades"
    );

    let checked = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(checked.status, StatusCode::OK, "{:?}", checked.body);
    assert_eq!(checked.body["state"], "available");
    assert_eq!(checked.body["manual"], Value::Null);
    assert_eq!(checked.body["latest"]["version"], next_patch());
    assert_eq!(checked.body["latest"]["major"], false);
    assert_eq!(checked.body["latest"]["manual_steps"], false);
    assert!(
        checked.body["latest"]["notes"]
            .as_str()
            .unwrap()
            .contains("## Added")
    );
    assert_eq!(checked.body["releases"].as_array().unwrap().len(), 1);
    assert_eq!(checked.body["check"]["error"], Value::Null);
    assert!(checked.body["check"]["last_checked_at"].is_string());
    assert_eq!(
        checked.body["check"]["next_check_at"],
        Value::Null,
        "checking is off"
    );
    assert_eq!(checked.body["can_apply"], false);

    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let requests = gh.lock().unwrap().requests.clone();
    assert_eq!(requests, [None, Some(etag_of(&stable()))]);
    let again = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(again.body["latest"]["version"], next_patch());

    let events = audit(&app, &owner, "instance.update_check").await;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["after"]["found"], next_patch());

    let pre = app
        .call(
            Method::PATCH,
            "/instance/updates/settings",
            Some(json!({ "channel": "prerelease" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(pre.status, StatusCode::OK, "{:?}", pre.body);
    assert_eq!(pre.body["latest"]["version"], next_minor_rc());
    assert_eq!(pre.body["latest"]["prerelease"], true);
    assert_eq!(pre.body["releases"].as_array().unwrap().len(), 2);
    let changed = audit(&app, &owner, "instance.update_settings").await;
    assert_eq!(changed[0]["before"]["channel"], "stable");
    assert_eq!(
        changed[0]["after"],
        json!({
            "channel": "prerelease",
            "mode": "manual",
            "window": { "frequency": "daily", "weekday": 1, "hour": 3 },
        })
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_major_or_manual_release_is_never_installed_through_the_updater() {
    let s = TestServices::start().await;
    let (gh, url) = github(vec![(next_major(), false, false)]).await;
    let (_, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["manual"], "major");
    assert_eq!(r.body["latest"]["major"], true);
    assert_eq!(r.body["updater"]["state"], "ready");
    assert_eq!(r.body["can_apply"], false);
    let refused = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_major() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.body);
    assert_eq!(
        refused.body["error"]["message"],
        "This release needs steps by hand; follow the upgrade guide."
    );

    gh.lock().unwrap().releases = vec![(next_patch(), false, true)];
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["manual"], "manual_steps");
    assert_eq!(r.body["latest"]["manual_steps"], true);
    s.cleanup().await;
}

#[tokio::test]
async fn a_release_with_steps_by_hand_the_gateway_never_saw_still_counts() {
    let s = TestServices::start().await;
    let v = semver::Version::parse(RUNNING).unwrap();
    let skipped = format!("{}.{}.{}", v.major, v.minor, v.patch + 1);
    let target = format!("{}.{}.{}", v.major, v.minor, v.patch + 2);
    let (gh, url) = github(vec![(target.clone(), false, false)]).await;
    gh.lock().unwrap().manual_since = Some(skipped);
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["latest"]["version"], target);
    assert_eq!(r.body["manual"], "manual_steps");
    assert_eq!(r.body["can_apply"], false);
    s.cleanup().await;
}

#[tokio::test]
async fn every_page_is_read_whatever_order_github_lists_releases_in() {
    let s = TestServices::start().await;
    let v = semver::Version::parse(RUNNING).unwrap();
    let mut releases: Vec<(String, bool, bool)> = vec![(RUNNING.to_owned(), false, false)];
    releases.extend((1..=55).rev().map(|p| {
        (
            format!("{}.{}.{}", v.major, v.minor, v.patch + p),
            false,
            false,
        )
    }));
    let (gh, url) = github(releases).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["releases"].as_array().unwrap().len(), 55);
    assert_eq!(gh.lock().unwrap().requests.len(), 2, "two pages");
    s.cleanup().await;
}

#[tokio::test]
async fn a_release_github_marks_as_a_pre_release_is_not_stable() {
    let s = TestServices::start().await;
    let (_, url) = github(vec![(next_patch(), true, false)]).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["state"], "up_to_date", "{:?}", r.body);
    let pre = app
        .call(
            Method::PATCH,
            "/instance/updates/settings",
            Some(json!({ "channel": "prerelease" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(pre.body["latest"]["version"], next_patch());
    assert_eq!(pre.body["latest"]["prerelease"], true);
    s.cleanup().await;
}

#[tokio::test]
async fn an_updater_is_lost_an_hour_after_it_last_answered_not_after_the_start() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let started = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    let id = started.body["id"].as_str().unwrap().to_owned();
    let path = format!("/instance/updates/jobs/{id}");
    app.clock.advance(time::Duration::minutes(50));
    assert_eq!(app.get(&path, &owner.cookie).await.body["state"], "running");
    app.clock.advance(time::Duration::minutes(30));
    updater.lock().unwrap().down = true;
    assert_eq!(
        app.get(&path, &owner.cookie).await.body["state"],
        "running",
        "80 minutes after the start, but 30 after the last answer"
    );
    app.clock.advance(time::Duration::minutes(40));
    let lost = app.get(&path, &owner.cookie).await;
    assert_eq!(lost.body["state"], "failed");
    assert_eq!(lost.body["reason"], "updater_unreachable");
    s.cleanup().await;
}

#[tokio::test]
async fn failed_checks_are_recorded_and_retried_after_an_hour_not_at_once() {
    let s = TestServices::start().await;
    let (gh, url) = github(stable()).await;
    gh.lock().unwrap().status = Some((403, Some("7200")));
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATES_CHECK", "true"),
        ],
    )
    .await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["state"], "unknown");
    let error = r.body["check"]["error"].as_str().unwrap();
    assert!(error.contains("refused more requests"), "{error}");
    let checked = time::OffsetDateTime::parse(
        r.body["check"]["last_checked_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let next = time::OffsetDateTime::parse(
        r.body["check"]["next_check_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert!(
        next - checked >= time::Duration::hours(2),
        "honours Retry-After"
    );

    assert!(
        !kaeso_server::services::updates::due(&app.state)
            .await
            .unwrap()
    );
    app.clock.advance(time::Duration::hours(4));
    assert!(
        kaeso_server::services::updates::due(&app.state)
            .await
            .unwrap()
    );

    gh.lock().unwrap().status = Some((500, None));
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert!(
        r.body["check"]["error"]
            .as_str()
            .unwrap()
            .contains("answered 500")
    );

    gh.lock().unwrap().status = None;
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "update_check")
        .unwrap();
    app.clock.advance(time::Duration::hours(2));
    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary, "release list fetched");
    let r = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(r.body["check"]["error"], Value::Null);
    assert_eq!(r.body["state"], "available");
    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary, "check not due");
    s.cleanup().await;
}

#[tokio::test]
async fn an_install_without_internet_records_the_failure_quietly() {
    let s = TestServices::start().await;
    let app = App::start_with(
        &s,
        &[("KAESO_UPDATES_RELEASES_URL", "http://127.0.0.1:9/releases")],
    )
    .await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.body["check"]["error"],
        "127.0.0.1 cannot be reached (no connection)"
    );
    assert_eq!(r.body["state"], "unknown");
    s.cleanup().await;
}

#[tokio::test]
async fn updating_and_choosing_automatic_updates_need_a_recent_sign_in() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (_, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let checked = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(
        checked.body["can_apply"], true,
        "a ready updater is all it takes"
    );

    app.clock.advance(time::Duration::minutes(16));
    let stale = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(stale.status, StatusCode::FORBIDDEN);
    assert_eq!(stale.error_type(), "recent_sign_in_required");
    let automatic = app
        .call(
            Method::PATCH,
            "/instance/updates/settings",
            Some(json!({ "mode": "automatic" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(automatic.status, StatusCode::FORBIDDEN);
    assert_eq!(automatic.error_type(), "recent_sign_in_required");
    let unchanged = app
        .call(
            Method::PATCH,
            "/instance/updates/settings",
            Some(json!({ "mode": "manual" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        unchanged.status,
        StatusCode::OK,
        "staying manual needs no recent sign-in"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn without_the_updater_the_admin_is_told_to_update_by_hand() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let r = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "The updater isn't installed; update this install by hand."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_update_runs_once_is_audited_and_its_result_is_recorded() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let status = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(status.body["can_apply"], true, "{:?}", status.body);
    assert_eq!(
        status.body["updater"],
        json!({ "state": "ready", "version": "40.9.0" })
    );

    let wrong = app
        .post(
            "/instance/updates/apply",
            json!({ "version": "40.0.1" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);

    let started = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    assert_eq!(started.body["state"], "running");
    assert_eq!(started.body["from"], RUNNING);
    assert_eq!(started.body["to"], next_patch());
    assert_eq!(started.body["started_by"]["name"], "Owner");
    let id = started.body["id"].as_str().unwrap().to_owned();
    assert_eq!(
        updater.lock().unwrap().started[0],
        json!({ "id": id, "version": next_patch() })
    );

    let again = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(
        again.body["error"]["message"],
        "An update is already running; wait for it to finish."
    );
    let busy = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(busy.body["can_apply"], false);
    assert_eq!(busy.body["job"]["step"], "migrating");

    updater.lock().unwrap().finish_as = Some("succeeded");
    let done = app
        .get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(done.body["state"], "succeeded");
    assert_eq!(done.body["step"], "online");
    assert!(done.body["finished_at"].is_string());

    let history = app.get("/instance/updates/jobs", &owner.cookie).await;
    assert_eq!(history.body["data"].as_array().unwrap().len(), 1);
    assert_eq!(history.body["data"][0]["id"], id.as_str());

    let start = audit(&app, &owner, "instance.update_start").await;
    assert_eq!(start.len(), 1);
    assert_eq!(start[0]["before"], json!({ "version": RUNNING }));
    assert_eq!(start[0]["after"], json!({ "version": next_patch() }));
    assert_eq!(start[0]["target_id"], id.as_str());
    let finish = audit(&app, &owner, "instance.update_finish").await;
    assert_eq!(finish.len(), 1, "the result is recorded once");
    assert_eq!(finish[0]["after"]["result"], "succeeded");
    app.get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(audit(&app, &owner, "instance.update_finish").await.len(), 1);
    assert_eq!(updater.lock().unwrap().unauthorized, 0);

    let missing = app
        .get(
            "/instance/updates/jobs/upd_01k7a3f8m2xq4v9c5r6t8w0yz1",
            &owner.cookie,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let malformed = app.get("/instance/updates/jobs/nope", &owner.cookie).await;
    assert_eq!(malformed.status, StatusCode::NOT_FOUND);
    s.cleanup().await;
}

#[tokio::test]
async fn a_rolled_back_update_and_an_updater_that_refuses_are_failures() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let version = json!({ "version": next_patch() });

    updater.lock().unwrap().refuse = true;
    let busy = app
        .post(
            "/instance/updates/apply",
            version.clone(),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(busy.status, StatusCode::CONFLICT);
    assert_eq!(
        busy.body["error"]["message"],
        "The updater is busy with another update; try again later."
    );
    let first = app.get("/instance/updates/jobs", &owner.cookie).await;
    assert_eq!(first.body["data"][0]["state"], "failed");
    assert_eq!(first.body["data"][0]["reason"], "updater_refused");

    updater.lock().unwrap().refuse = false;
    updater.lock().unwrap().fail_start = Some(503);
    let unavailable = app
        .post(
            "/instance/updates/apply",
            version.clone(),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(unavailable.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable.body["error"]["message"],
        "The updater refused the update; nothing was changed."
    );
    let second = app.get("/instance/updates/jobs", &owner.cookie).await;
    assert_eq!(second.body["data"][0]["state"], "failed");
    assert_eq!(second.body["data"][0]["reason"], "updater_refused");
    updater.lock().unwrap().fail_start = None;

    updater.lock().unwrap().refuse = false;
    updater.lock().unwrap().finish_as = Some("rolled_back");
    let started = app
        .post("/instance/updates/apply", version, Some(&owner.cookie))
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "update_check")
        .unwrap();
    kaeso_server::jobs::run_once(&app.state, job).await.unwrap();
    let r = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(r.body["job"]["state"], "rolled_back");
    assert_eq!(r.body["job"]["reason"], "unhealthy");
    assert_eq!(
        r.body["job"]["detail"],
        "the new gateway reported itself unhealthy"
    );
    assert_eq!(r.body["can_apply"], true);
    s.cleanup().await;
}

#[tokio::test]
async fn a_running_update_the_updater_forgot_is_marked_interrupted() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let started = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    let id = started.body["id"].as_str().unwrap().to_owned();
    updater.lock().unwrap().started.clear();
    let r = app
        .get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(
        r.body["state"], "running",
        "an update the updater hasn't taken up yet gets a grace period"
    );
    app.clock.advance(time::Duration::minutes(3));
    let r = app
        .get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(r.body["state"], "failed");
    assert_eq!(r.body["reason"], "interrupted");
    s.cleanup().await;
}

#[tokio::test]
async fn a_start_the_updater_does_not_confirm_stays_running_until_it_reports() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    updater.lock().unwrap().garbled = true;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    let started = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.body);
    assert_eq!(started.body["state"], "running");
    let id = started.body["id"].as_str().unwrap().to_owned();
    let r = app
        .get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(r.body["state"], "running");
    assert_eq!(r.body["step"], "migrating");
    updater.lock().unwrap().finish_as = Some("needs_attention");
    let r = app
        .get(&format!("/instance/updates/jobs/{id}"), &owner.cookie)
        .await;
    assert_eq!(r.body["state"], "needs_attention");
    assert_eq!(r.body["reason"], "unhealthy");
    let finish = audit(&app, &owner, "instance.update_finish").await;
    assert_eq!(finish[0]["after"]["result"], "needs_attention");
    s.cleanup().await;
}

#[tokio::test]
async fn a_manual_check_honours_githubs_wait() {
    let s = TestServices::start().await;
    let (gh, url) = github(stable()).await;
    gh.lock().unwrap().status = Some((429, Some("600")));
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let first = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert!(
        first.body["check"]["error"]
            .as_str()
            .unwrap()
            .contains("refused more requests")
    );
    gh.lock().unwrap().status = None;
    let waiting = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(waiting.status, StatusCode::OK);
    assert_eq!(waiting.body["check"]["error"], first.body["check"]["error"]);
    assert_eq!(gh.lock().unwrap().requests.len(), 1, "GitHub asked to wait");
    app.clock.advance(time::Duration::minutes(11));
    let after = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(after.body["check"]["error"], Value::Null);
    assert_eq!(after.body["state"], "available");
    assert_eq!(gh.lock().unwrap().requests.len(), 2);
    s.cleanup().await;
}

#[tokio::test]
async fn a_release_manifest_from_elsewhere_is_ignored() {
    let s = TestServices::start().await;
    let (gh, url) = github(stable()).await;
    let elsewhere = gh.lock().unwrap().base.replace("127.0.0.1", "localhost");
    gh.lock().unwrap().download_base = Some(elsewhere);
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["latest"]["version"], next_patch());
    assert_eq!(r.body["manual"], "not_prepared");
    assert_eq!(gh.lock().unwrap().downloads, 0);
    s.cleanup().await;
}

#[tokio::test]
async fn many_new_releases_are_fetched_a_dozen_at_a_time() {
    let s = TestServices::start().await;
    let v = semver::Version::parse(RUNNING).unwrap();
    let releases: Vec<(String, bool, bool)> = (1..=15)
        .map(|p| {
            (
                format!("{}.{}.{}", v.major, v.minor, v.patch + p),
                false,
                false,
            )
        })
        .collect();
    let (gh, url) = github(releases).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(
        r.body["manual"],
        Value::Null,
        "the newest are fetched first"
    );
    assert_eq!(gh.lock().unwrap().downloads, 12);
    app.post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(gh.lock().unwrap().downloads, 15);
    assert_eq!(
        gh.lock().unwrap().requests.last().unwrap(),
        &None,
        "an incomplete list is fetched again in full"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn an_unreachable_updater_changes_nothing() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", "http://127.0.0.1:9/"),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let r = app
        .post("/instance/updates/check", json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(
        r.body["updater"],
        json!({ "state": "unreachable", "version": null })
    );
    assert_eq!(r.body["can_apply"], false);
    let refused = app
        .post(
            "/instance/updates/apply",
            json!({ "version": next_patch() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        refused.body["error"]["message"],
        "The updater couldn't be reached; nothing was changed."
    );
    let history = app.get("/instance/updates/jobs", &owner.cookie).await;
    assert_eq!(history.body["data"], json!([]));
    s.cleanup().await;
}

#[tokio::test]
async fn only_instance_admins_see_or_change_updates() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (member, _) = common::join(
        &app,
        &owner,
        json!({ "email": "grace@example.com", "role": "admin", "project_roles": [] }),
        "Grace Hopper",
    )
    .await;
    for (method, path, body) in [
        (Method::GET, "/instance/updates", None),
        (Method::POST, "/instance/updates/check", Some(json!({}))),
        (
            Method::PATCH,
            "/instance/updates/settings",
            Some(json!({ "mode": "manual" })),
        ),
        (
            Method::POST,
            "/instance/updates/apply",
            Some(json!({ "version": "9.9.9" })),
        ),
        (Method::GET, "/instance/updates/jobs", None),
    ] {
        let r = app.call(method.clone(), path, body, Some(&member)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{method} {path}");
    }
    s.cleanup().await;
}

fn next_hour(app: &App) -> time::OffsetDateTime {
    use kaeso_core::clock::Clock;
    let now = app.clock.now();
    now.replace_minute(0).unwrap().replace_second(0).unwrap() + time::Duration::hours(1)
}

async fn update_settings(app: &App, owner: &Owner, body: Value) -> common::Resp {
    app.call(
        Method::PATCH,
        "/instance/updates/settings",
        Some(body),
        Some(&owner.cookie),
    )
    .await
}

#[tokio::test]
async fn automatic_updates_follow_a_window_in_the_instance_time_zone() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;

    let fresh = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(fresh.body["settings"]["mode"], "manual");
    assert_eq!(
        fresh.body["settings"]["schedule"],
        json!({ "frequency": "daily", "weekday": "monday", "hour": 3, "timezone": "UTC" })
    );
    assert_eq!(fresh.body["settings"]["next_auto_update_at"], Value::Null);

    let late = update_settings(
        &app,
        &owner,
        json!({ "schedule": { "frequency": "weekly", "weekday": "sunday", "hour": 24 } }),
    )
    .await;
    assert_eq!(late.status, StatusCode::BAD_REQUEST);
    assert_eq!(late.body["error"]["param"], "schedule.hour");
    app.call(
        Method::PATCH,
        "/instance/settings",
        Some(json!({ "timezone": "Europe/Berlin" })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let on = update_settings(
        &app,
        &owner,
        json!({
            "mode": "automatic",
            "schedule": { "frequency": "weekly", "weekday": "sunday", "hour": 4 },
        }),
    )
    .await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.body);
    assert_eq!(on.body["settings"]["mode"], "automatic");
    assert_eq!(
        on.body["settings"]["schedule"],
        json!({ "frequency": "weekly", "weekday": "sunday", "hour": 4, "timezone": "Europe/Berlin" })
    );
    let next = time::OffsetDateTime::parse(
        on.body["settings"]["next_auto_update_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert_eq!(next.weekday(), time::Weekday::Sunday);
    assert!([2, 3].contains(&next.hour()), "04:00 in Berlin is {next}");

    let changed = audit(&app, &owner, "instance.update_settings").await;
    assert_eq!(changed[0]["before"]["mode"], "manual");
    assert_eq!(changed[0]["after"]["mode"], "automatic");
    assert_eq!(
        changed[0]["after"]["window"],
        json!({ "frequency": "weekly", "weekday": 7, "hour": 4 })
    );

    let manual = update_settings(&app, &owner, json!({ "mode": "manual" })).await;
    assert_eq!(manual.body["settings"]["next_auto_update_at"], Value::Null);
    assert_eq!(
        manual.body["settings"]["schedule"]["weekday"], "sunday",
        "the window stays for next time"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_gateway_updates_by_itself_in_the_window_and_only_once() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let window = next_hour(&app);
    let on = update_settings(
        &app,
        &owner,
        json!({
            "mode": "automatic",
            "schedule": { "frequency": "daily", "weekday": "monday", "hour": window.hour() },
        }),
    )
    .await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.body);
    assert_eq!(
        on.body["settings"]["next_auto_update_at"],
        window
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    );
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "update_check")
        .unwrap();

    let before = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert!(before.ends_with("; no update window"), "{before}");
    assert!(updater.lock().unwrap().started.is_empty());

    app.clock.set(window + time::Duration::minutes(5));
    let during = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert!(
        during.ends_with(&format!("update window: updating to {}", next_patch())),
        "{during}"
    );
    let started = updater.lock().unwrap().started.clone();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["version"], next_patch());

    let history = app.get("/instance/updates/jobs", &owner.cookie).await;
    assert_eq!(history.body["data"][0]["automatic"], true);
    assert_eq!(history.body["data"][0]["started_by"], Value::Null);
    let start = audit(&app, &owner, "instance.update_start").await;
    assert_eq!(start[0]["after"]["automatic"], true);

    updater.lock().unwrap().finish_as = Some("succeeded");
    app.clock.advance(time::Duration::minutes(10));
    let later = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert!(later.ends_with("; no update window"), "{later}");
    assert_eq!(
        updater.lock().unwrap().started.len(),
        1,
        "one update per window"
    );
    let next = app.get("/instance/updates", &owner.cookie).await;
    assert_eq!(
        next.body["settings"]["next_auto_update_at"],
        (window + time::Duration::days(1))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    );
    s.cleanup().await;
}

#[tokio::test]
async fn a_major_release_waits_for_an_admin_even_in_the_window() {
    let s = TestServices::start().await;
    let (_, url) = github(vec![(next_major(), false, false)]).await;
    let (updater, updater_url) = updater().await;
    let app = App::start_with(
        &s,
        &[
            ("KAESO_UPDATES_RELEASES_URL", url.as_str()),
            ("KAESO_UPDATER_URL", updater_url.as_str()),
            ("KAESO_UPDATER_TOKEN", token()),
        ],
    )
    .await;
    let owner = app.setup().await;
    let window = next_hour(&app);
    update_settings(
        &app,
        &owner,
        json!({
            "mode": "automatic",
            "schedule": { "frequency": "daily", "weekday": "monday", "hour": window.hour() },
        }),
    )
    .await;
    app.clock.set(window + time::Duration::minutes(1));
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "update_check")
        .unwrap();
    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert!(
        summary.ends_with("update window: the release needs steps by hand"),
        "{summary}"
    );
    assert!(updater.lock().unwrap().started.is_empty());
    s.cleanup().await;
}

#[tokio::test]
async fn without_the_updater_the_window_passes_quietly() {
    let s = TestServices::start().await;
    let (_, url) = github(stable()).await;
    let app = App::start_with(&s, &[("KAESO_UPDATES_RELEASES_URL", url.as_str())]).await;
    let owner = app.setup().await;
    let window = next_hour(&app);
    update_settings(
        &app,
        &owner,
        json!({
            "mode": "automatic",
            "schedule": { "frequency": "daily", "weekday": "monday", "hour": window.hour() },
        }),
    )
    .await;
    app.clock.set(window + time::Duration::minutes(1));
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "update_check")
        .unwrap();
    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert!(
        summary.ends_with("update window: the updater isn't ready"),
        "{summary}"
    );
    s.cleanup().await;
}

fn minimal() -> kaeso_server::config::MapSource {
    kaeso_server::config::MapSource::new()
        .set("KAESO_PUBLIC_URL", "https://kaeso.example.com")
        .set("KAESO_POSTGRES_URL", "postgres://x")
        .set("KAESO_CLICKHOUSE_URL", "http://x")
        .set("KAESO_VALKEY_URL", "redis://x")
        .set(
            "KAESO_MASTER_KEYS",
            "k1:AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
        )
        .set("KAESO_MAIL_TRANSPORT", "log")
}

#[test]
fn the_updater_token_must_be_long() {
    let s = minimal().set("KAESO_UPDATER_TOKEN", "short");
    let err = kaeso_server::config::load(&s).unwrap_err().to_string();
    assert!(err.contains("at least 32 characters"), "{err}");
    assert!(!err.contains("short"), "{err}");
}

#[test]
fn the_updater_token_comes_from_a_compose_secret_too() {
    let s = minimal()
        .set("KAESO_UPDATER_TOKEN", "")
        .set("KAESO_UPDATER_TOKEN_FILE", "/run/secrets/updater_token")
        .file("/run/secrets/updater_token", format!("{}\n", token()));
    let config = kaeso_server::config::load(&s).unwrap();
    assert_eq!(
        secrecy::ExposeSecret::expose_secret(config.updater.token.as_ref().unwrap()),
        token()
    );
}
