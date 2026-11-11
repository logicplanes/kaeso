//! OpenAI's Videos API for apps: `POST /v1/videos` and `POST /v1/videos/{id}/remix` start a
//! job through the model pipeline, which keeps the job's provider, key and upstream id in
//! PostgreSQL under an id of Kaeso's own. Retrieving a job asks its provider how it stands; the
//! first answer that it has ended bills it, once. Its files stream from the provider to the app.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::{EgressCtx, EncodedRequest, HeaderList, dispatch, openai_videos};
use kaeso_core::catalogue::Fields;
use kaeso_core::cost::MediaUnits;
use kaeso_core::id::{CredentialId, OrgId, ProjectId, RequestId, VideoId};
use kaeso_core::provider::AuthStyle;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{
    DialectId, ErrorKind, ImageSize, KppError, Reply, RequestKind, Source, StreamMode, VideoError,
    VideoJob, VideoList, VideoRequest, VideoStatus, VideoVariant,
};
use kaeso_store::pg::videos::{self as store, VideoProgress, VideoRow};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use serde_json::{Value, json};
use time::OffsetDateTime;
use url::Url;

use super::errors::{Served, error, respond};
use super::hooks::{Caller, VideoCall};
use super::pipeline::{self, Incoming, Recording};
use crate::http::client_ip::ClientIp;
use crate::services::credentials::Resolved;
use crate::state::AppState;

const D: DialectId = DialectId::OpenAiVideos;

// A link a provider sent can redirect, as Google's file downloads do; Kaeso follows a few.
const MAX_REDIRECTS: usize = 3;

/// How a request that starts a video job enters the pipeline.
#[derive(Clone, Debug)]
pub struct VideoIncoming {
    pub call: VideoCall,
    pub remix: Option<RemixSource>,
}

/// The finished video a remix changes, which only its own provider can remix.
#[derive(Clone, Debug)]
pub struct RemixSource {
    pub id: VideoId,
    pub model: String,
    pub upstream_id: String,
    pub provider: String,
    pub link: Option<String>,
    /// The source video's length, billed as input where the provider is sent the video.
    pub seconds: Option<u32>,
}

impl RemixSource {
    pub(super) fn fill(&self, r: &mut VideoRequest) {
        r.model.clone_from(&self.model);
        r.remix_of = Some(self.upstream_id.clone());
        r.source_video = self.link.clone().map(Source::Url);
    }

    pub(super) fn check_provider(&self, provider: &str) -> Result<(), KppError> {
        if provider == self.provider {
            return Ok(());
        }
        Err(error(
            ErrorKind::InvalidRequest,
            400,
            format!(
                "`{}` can be remixed only at the provider that made it.",
                self.id
            ),
        )
        .with_param("video_id"))
    }
}

/// OpenAI shut its video models down on 2026-09-24
/// (<https://platform.openai.com/docs/deprecations>), so a `sora-*` model is refused by name.
pub(super) fn refuse_sora(r: &VideoRequest) -> Result<(), KppError> {
    if r.model.starts_with("sora-") {
        return Err(error(
            ErrorKind::InvalidRequest,
            400,
            "OpenAI shut down Sora on 24 September 2026; choose another video model.",
        )
        .with_param("model"));
    }
    Ok(())
}

/// The request as the log keeps it: every field, each uploaded image replaced by its media
/// type and size.
pub(super) fn logged_request(
    r: &VideoRequest,
    mut keep: crate::services::log_media::Keep,
) -> Bytes {
    let mut kept = r.clone();
    let mut left_out = crate::services::log_media::LeftOut::default();
    for image in &mut kept.images {
        left_out.fit(&mut image.source, &mut keep);
    }
    Bytes::from(left_out.write(&kept).unwrap_or_default())
}

fn seconds_ms(seconds: Option<u32>) -> u64 {
    seconds.map_or(0, |s| u64::from(s).saturating_mul(1000))
}

fn resolution_of(r: Option<&str>, size: Option<&ImageSize>) -> Option<String> {
    r.map(str::to_owned).or_else(|| {
        let (w, h) = size?.dimensions()?;
        Some(format!("{}p", w.min(h)))
    })
}

/// What the request asks for: its videos, their length and resolution. The job is billed by
/// what the provider reports when it ends.
pub(super) fn units_of_request(r: &VideoRequest) -> MediaUnits {
    let count = u64::from(r.count.unwrap_or(1).max(1));
    MediaUnits {
        input_images: r.images.len() as u64,
        output_videos: count,
        output_ms: seconds_ms(r.seconds).saturating_mul(count),
        size: r.size.as_ref().map(|s| s.as_wire().to_owned()),
        resolution: resolution_of(r.resolution.as_deref(), r.size.as_ref()),
        audio: r.audio,
        ..MediaUnits::default()
    }
}

/// The video job a provider just started, and what started it.
pub(super) struct Started<'a> {
    pub caller: &'a Caller,
    pub entry: uuid::Uuid,
    pub eff: &'a kaeso_core::catalogue::Effective,
    pub credential: CredentialId,
    pub dialect: DialectId,
    pub app: &'a kaeso_kpp::Request,
    pub remix: Option<&'a RemixSource>,
}

fn params_of(r: &VideoRequest, remix: Option<&RemixSource>) -> Value {
    let source_seconds = remix.filter(|s| s.link.is_some()).and_then(|s| s.seconds);
    json!({
        "prompt": r.prompt,
        "seconds": r.seconds,
        "size": r.size.as_ref().map(ImageSize::as_wire),
        "resolution": r.resolution,
        "aspect_ratio": r.aspect_ratio,
        "audio": r.audio,
        "count": r.count,
        "images": r.images.len(),
        "source_seconds": source_seconds,
    })
}

fn unix(t: OffsetDateTime) -> i64 {
    t.unix_timestamp()
}

fn at(unix: Option<i64>) -> Option<OffsetDateTime> {
    unix.and_then(|t| OffsetDateTime::from_unix_timestamp(t).ok())
}

/// Keeps the job the provider started under an id of Kaeso's own, and answers with the job as
/// the app sees it.
pub(super) async fn register(
    state: &AppState,
    rec: &mut Recording,
    s: &Started<'_>,
    job: VideoJob,
) -> Result<VideoJob, KppError> {
    let kaeso_kpp::Request::Video(app) = s.app else {
        return Err(KppError::provider(
            "A video job started for another kind of request.",
        ));
    };
    if job.id.is_empty() {
        return Err(KppError::provider(
            "The provider started a video job without naming it.",
        ));
    }
    let now = state.clock.now();
    let id = VideoId::new();
    let row = VideoRow {
        id: id.uuid(),
        org_id: s.caller.org_id,
        project_id: s.caller.project_id,
        api_key_id: s.caller.key_id,
        user_id: s.caller.user_id,
        service_account_id: s.caller.service_account_id,
        provider: s.eff.provider.id().to_owned(),
        credential_id: s.credential.uuid(),
        catalogue_model_id: Some(s.entry),
        model: app.model.clone(),
        upstream_model: s.eff.model_id.clone(),
        dialect: s.dialect.as_str().to_owned(),
        upstream_id: job.id.clone(),
        request_id: rec.outcome().request_id.uuid(),
        params: params_of(app, s.remix),
        prices: serde_json::to_value(&s.eff.fields).ok(),
        status: job.status.as_str().to_owned(),
        progress: progress_of(&job),
        error: job.error.as_ref().map(error_value),
        remixed_from: s.remix.map(|r| r.id.uuid()),
        units: None,
        cost: None,
        created_at: now,
        completed_at: None,
        expires_at: at(job.expires_at),
        accounted_at: None,
    };
    store::insert(&state.pg, &row).await.map_err(|e| {
        tracing::error!(error = %e, "cannot keep a started video job");
        error(
            ErrorKind::ProviderError,
            503,
            "Kaeso started the video but cannot keep track of it; try again shortly.",
        )
    })?;
    Ok(app_view(&row, None))
}

fn progress_of(job: &VideoJob) -> i16 {
    let p = match job.status {
        VideoStatus::Completed => 100,
        _ => job.progress.unwrap_or(0).min(100),
    };
    i16::try_from(p).unwrap_or(0)
}

fn error_value(e: &VideoError) -> Value {
    json!({ "code": e.code, "message": e.message })
}

fn param_str(row: &VideoRow, key: &str) -> Option<String> {
    row.params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn param_u32(v: &Value, key: &str) -> Option<u32> {
    v.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

/// The job as the app sees it, under Kaeso's id and the model the app named; `units` are what
/// the provider reported when it ended.
fn app_view(row: &VideoRow, remixed_from: Option<VideoId>) -> VideoJob {
    let status = VideoStatus::from_wire(&row.status).unwrap_or_default();
    let mut job = VideoJob::new(VideoId::from_uuid(row.id).to_string(), status);
    job.model.clone_from(&row.model);
    job.progress = u32::try_from(row.progress).ok();
    job.created_at = Some(unix(row.created_at));
    job.completed_at = row.completed_at.map(unix);
    job.expires_at = row.expires_at.map(unix);
    job.prompt = param_str(row, "prompt");
    let reported = row.units.as_ref();
    job.seconds = reported
        .and_then(|u| u.get("output_ms").and_then(Value::as_u64))
        .filter(|ms| *ms > 0)
        .map(|ms| u32::try_from(ms.div_ceil(1000)).unwrap_or(u32::MAX))
        .or_else(|| param_u32(&row.params, "seconds"));
    job.size = reported
        .and_then(|u| u.get("size").and_then(Value::as_str))
        .or_else(|| row.params.get("size").and_then(Value::as_str))
        .map(ImageSize::from_wire);
    job.resolution = reported
        .and_then(|u| u.get("resolution").and_then(Value::as_str))
        .map(str::to_owned)
        .or_else(|| param_str(row, "resolution"));
    job.aspect_ratio = param_str(row, "aspect_ratio");
    job.remixed_from = remixed_from
        .or(row.remixed_from.map(VideoId::from_uuid))
        .map(|id| id.to_string());
    job.error = row.error.as_ref().map(|e| {
        VideoError::new(
            e.get("code").and_then(Value::as_str).unwrap_or_default(),
            e.get("message").and_then(Value::as_str).unwrap_or_default(),
        )
    });
    job
}

fn answer(status: StatusCode, body: Result<Vec<u8>, KppError>) -> Response {
    match body {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp
        }
        Err(e) => refuse(&e),
    }
}

/// The `{video_id}` segment of a video path, read from the URI so a bad id is answered in
/// OpenAI's error format rather than by an extractor's rejection.
fn video_id(uri: &axum::http::Uri) -> String {
    uri.path()
        .strip_prefix("/v1/videos/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned()
}

fn refuse(err: &KppError) -> Response {
    respond(D, err, &Served::default())
}

fn not_found(id: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("No video `{id}` belongs to this project."),
    )
    .with_param("video_id")
}

const UNAVAILABLE: &str = "Kaeso cannot read its videos right now; try again shortly.";

/// Whether a video request failed on Kaeso's own store, not at the provider.
pub(crate) fn is_unavailable(e: &KppError) -> bool {
    e.status == Some(503) && e.message == UNAVAILABLE
}

const EXPIRED: &str = "expired; create the video again.";

/// Whether a video's files are gone at its provider.
pub(crate) fn is_expired(e: &KppError) -> bool {
    e.status == Some(404) && e.message.ends_with(EXPIRED)
}

fn unavailable() -> KppError {
    error(ErrorKind::ProviderError, 503, UNAVAILABLE)
}

/// Who asks about a video: an app with its key, which sees every video of its project, or a
/// member in the Playground, who sees only the videos they made there.
#[derive(Clone, Debug)]
pub(crate) struct Viewer {
    caller: Caller,
    playground: bool,
}

impl Viewer {
    fn app(key: &kaeso_store::pg::keys::KeyAuth) -> Viewer {
        Viewer {
            caller: pipeline::key_caller(key),
            playground: false,
        }
    }

    pub(crate) fn member(caller: Caller) -> Viewer {
        Viewer {
            caller,
            playground: true,
        }
    }

    fn sees(&self, row: &VideoRow) -> bool {
        if self.playground {
            from_playground(row) && row.user_id == self.caller.user_id
        } else {
            !from_playground(row)
        }
    }
}

/// A video a member started in the Playground: no key started it, a person did. Apps never
/// see these, and the members who didn't start them neither.
fn from_playground(row: &VideoRow) -> bool {
    row.api_key_id.is_none() && row.user_id.is_some()
}

// An id that isn't Kaeso's or belongs to another project is answered the same way, so ids
// never tell whether a video exists elsewhere.
async fn load(state: &AppState, viewer: &Viewer, id: &str) -> Result<VideoRow, KppError> {
    let parsed: VideoId = id.parse().map_err(|_| not_found(id))?;
    let c = &viewer.caller;
    let row = store::get(&state.pg, c.org_id, c.project_id, parsed.uuid())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot load a video");
            unavailable()
        })?
        .filter(|row| viewer.sees(row))
        .ok_or_else(|| not_found(id))?;
    if !viewer.playground
        && !super::listing::ModelAccess::of(c, state)
            .await?
            .allows(row.catalogue_model_id)
    {
        return Err(not_found(id));
    }
    Ok(row)
}

async fn video_request(
    state: AppState,
    request_id: RequestId,
    ip: ClientIp,
    req: Request,
    video: VideoIncoming,
) -> Response {
    let (parts, body) = req.into_parts();
    pipeline::serve(
        state.clone(),
        Incoming {
            ingress: D,
            kind: RequestKind::Video,
            headers: parts.headers,
            body,
            request_id,
            client_ip: ip.0,
            caller: None,
            source: super::hooks::Source::Api,
            stream_only: false,
            max_body: state.config.body_limits.model_api.as_usize(),
            alias: None,
            gemini: None,
            image: None,
            transcription: None,
            elevenlabs: None,
            video: Some(video),
            generation: None,
        },
    )
    .await
}

/// `POST /v1/videos`.
pub(super) async fn create(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    req: Request,
) -> Response {
    let video = VideoIncoming {
        call: VideoCall::Create,
        remix: None,
    };
    video_request(state, request_id, ip, req, video).await
}

/// `POST /v1/videos/{video_id}/remix`: a new job at the provider that made the video, from
/// the video and a new prompt.
pub(super) async fn remix(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    OriginalUri(uri): OriginalUri,
    req: Request,
) -> Response {
    let id = video_id(&uri);
    let source = match remix_source(&state, req.headers(), ip, &id).await {
        Ok(source) => source,
        Err(e) => return refuse(&e),
    };
    let video = VideoIncoming {
        call: VideoCall::Remix,
        remix: Some(source),
    };
    video_request(state, request_id, ip, req, video).await
}

async fn remix_source(
    state: &AppState,
    headers: &HeaderMap,
    ip: ClientIp,
    id: &str,
) -> Result<RemixSource, KppError> {
    let viewer = Viewer::app(&*super::auth::authenticate(state, headers, ip.0).await?);
    let asking = Asking::new(state, &viewer);
    let row = refresh(&asking, load(state, &viewer, id).await?).await?;
    if row.status != VideoStatus::Completed.as_str() {
        return Err(error(
            ErrorKind::InvalidRequest,
            400,
            format!(
                "`{id}` is {}; only a completed video can be remixed.",
                row.status
            ),
        )
        .with_param("video_id"));
    }
    let dialect: DialectId = row.dialect.parse().map_err(|_| unavailable())?;
    let link = if dialect == DialectId::XaiVideos {
        let up = reach(&asking, &row).await?;
        let job = poll(state, &up, &row).await?;
        match job.output(VideoVariant::Video).map(|o| &o.source) {
            Some(Source::Url(u)) => Some(u.clone()),
            _ => None,
        }
    } else {
        None
    };
    let seconds = row
        .units
        .as_ref()
        .and_then(|u| serde_json::from_value::<MediaUnits>(u.clone()).ok())
        .filter(|u| u.output_videos > 0 && u.output_ms > 0)
        .map(|u| u.output_ms / u.output_videos / 1000)
        .and_then(|s| u32::try_from(s).ok())
        .or_else(|| param_u32(&row.params, "seconds"));
    Ok(RemixSource {
        seconds,
        id: VideoId::from_uuid(row.id),
        model: row.model.clone(),
        upstream_id: row.upstream_id.clone(),
        provider: row.provider.clone(),
        link,
    })
}

/// `GET /v1/videos`: the project's videos as Kaeso last saw them, without asking providers.
pub(super) async fn list(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let listed = async {
        let key = super::auth::authenticate(&state, &headers, ip.0).await?;
        let viewer = Viewer::app(&key);
        let q = openai_videos::decode_list_query(uri.query().unwrap_or(""))?;
        let after = match &q.after {
            Some(a) => Some(load(&state, &viewer, a).await.map_err(|_| {
                error(
                    ErrorKind::NotFound,
                    404,
                    format!("No video `{a}` belongs to this project."),
                )
                .with_param("after")
            })?),
            None => None,
        };
        let limit = q.limit();
        let rows = visible(
            &state,
            &viewer,
            after.map(|r| r.id),
            q.order == kaeso_kpp::ListOrder::Asc,
            limit as usize + 1,
        )
        .await?;
        let has_more = rows.len() > limit as usize;
        let videos = rows
            .iter()
            .take(limit as usize)
            .map(|r| app_view(r, None))
            .collect();
        openai_videos::encode_list(&VideoList::new(videos, has_more))
    }
    .await;
    answer(StatusCode::OK, listed)
}

// The project's videos the viewer may see, `wanted` at most, in pages until there are enough
// or the project has no more; members' Playground videos are left out without a gap in paging.
async fn visible(
    state: &AppState,
    viewer: &Viewer,
    mut after: Option<uuid::Uuid>,
    ascending: bool,
    wanted: usize,
) -> Result<Vec<VideoRow>, KppError> {
    let c = &viewer.caller;
    let models = if viewer.playground {
        None
    } else {
        Some(super::listing::ModelAccess::of(c, state).await?)
    };
    let page = i64::try_from(wanted).unwrap_or(i64::MAX);
    let mut out = Vec::with_capacity(wanted);
    loop {
        let rows = store::list(&state.pg, c.org_id, c.project_id, after, ascending, page)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "cannot list videos");
                unavailable()
            })?;
        let full = rows.len() == wanted;
        after = rows.last().map(|r| r.id);
        out.extend(rows.into_iter().filter(|r| {
            viewer.sees(r)
                && models
                    .as_ref()
                    .is_none_or(|m| m.allows(r.catalogue_model_id))
        }));
        if !full || out.len() >= wanted {
            out.truncate(wanted);
            return Ok(out);
        }
    }
}

/// `GET /v1/videos/{video_id}`: the job, after asking its provider how it stands.
pub(super) async fn retrieve(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = video_id(&uri);
    let found = async {
        let viewer = Viewer::app(&*super::auth::authenticate(&state, &headers, ip.0).await?);
        retrieve_as(&state, &viewer, &id)
            .await
            .map(|(body, _)| body)
    }
    .await;
    answer(StatusCode::OK, found)
}

/// The job as OpenAI's Videos API shows it, after asking its provider how it stands.
pub(crate) async fn retrieve_as(
    state: &AppState,
    viewer: &Viewer,
    id: &str,
) -> Result<(Vec<u8>, Option<String>), KppError> {
    let row = refresh(&Asking::new(state, viewer), load(state, viewer, id).await?).await?;
    let body = dispatch::encode_response(
        D,
        &Reply::Video(app_view(&row, None)),
        &kaeso_codecs::EncodeCtx::new(RequestKind::Video, StreamMode::Off),
    )?;
    Ok((body, charged(&row)))
}

/// What the job was charged once it ended: its cost, `0` for a job that failed, or `unknown`
/// when the model has no price for what it made; `None` while it runs.
fn charged(row: &VideoRow) -> Option<String> {
    row.accounted_at?;
    Some(if row.status != VideoStatus::Completed.as_str() {
        "0".to_owned()
    } else {
        row.cost
            .map_or_else(|| "unknown".to_owned(), |c| c.normalize().to_string())
    })
}

/// `DELETE /v1/videos/{video_id}`: deletes the job at its provider where the provider can, and
/// forgets it.
pub(super) async fn delete(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = video_id(&uri);
    let deleted = async {
        let viewer = Viewer::app(&*super::auth::authenticate(&state, &headers, ip.0).await?);
        delete_as(&state, &viewer, &id).await
    }
    .await;
    answer(StatusCode::OK, deleted)
}

/// Deletes the job at its provider where the provider can, and forgets it.
pub(crate) async fn delete_as(
    state: &AppState,
    viewer: &Viewer,
    id: &str,
) -> Result<Vec<u8>, KppError> {
    {
        let row = load(state, viewer, id).await?;
        let reached = match reach(&Asking::new(state, viewer), &row).await {
            Err(e) if e.status == Some(429) => return Err(e),
            r => r,
        };
        if let Ok(up) = reached
            && let Some(req) = dispatch::encode_video_delete(
                up.dialect,
                &row.upstream_id,
                &row.upstream_model,
                &up.ectx,
            )?
        {
            let resp = send(state, &up, req).await?;
            if !(200..300).contains(&resp.0) && resp.0 != 404 {
                return Err(dispatch::decode_error(up.dialect, resp.0, &resp.1, &resp.2));
            }
        }
        let c = &viewer.caller;
        store::delete(&state.pg, c.org_id, c.project_id, row.id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "cannot delete a video");
                unavailable()
            })?;
        state.caches.finished_videos.invalidate(&row.id).await;
        openai_videos::encode_deleted(&VideoId::from_uuid(row.id).to_string())
    }
}

/// The parts of OpenAI's Videos API Kaeso doesn't route: edits, extensions and characters.
pub(super) async fn not_served(OriginalUri(uri): OriginalUri) -> Response {
    refuse(&error(
        ErrorKind::NotFound,
        404,
        format!(
            "Kaeso does not serve `{}`; remix a video or create a new one.",
            uri.path()
        ),
    ))
}

/// Where and how a job's provider is reached: with the key that started the job.
struct Reached {
    provider: ProviderRef,
    credential: Arc<Resolved>,
    platform: Option<crate::services::agent_platform::Egress>,
    base: Url,
    auth: Option<AuthStyle>,
    dialect: DialectId,
    ectx: EgressCtx,
}

impl Reached {
    fn secret(&self) -> &secrecy::SecretString {
        self.platform
            .as_ref()
            .map_or(&self.credential.secret, |p| &p.secret)
    }
}

fn setup() -> KppError {
    error(
        ErrorKind::ProviderError,
        500,
        "No base URL is configured for this provider.",
    )
}

/// An app's request about one of its videos. The first time it reaches the provider, it counts
/// against the key's request rates, since it spends the provider's quota with the org's key.
struct Asking<'a> {
    state: &'a AppState,
    viewer: &'a Viewer,
    counted: std::sync::atomic::AtomicBool,
}

impl<'a> Asking<'a> {
    fn new(state: &'a AppState, viewer: &'a Viewer) -> Self {
        Asking {
            state,
            viewer,
            counted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

async fn reach(asking: &Asking<'_>, row: &VideoRow) -> Result<Reached, KppError> {
    let state = asking.state;
    if !asking
        .counted
        .swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        pipeline::admit_follow_up(state, &asking.viewer.caller, row.catalogue_model_id).await?;
    }
    let dialect: DialectId = row.dialect.parse().map_err(|_| unavailable())?;
    let provider = match kaeso_core::provider::provider(&row.provider) {
        Some(p) => ProviderRef::from(p),
        None => super::listing::project_models(
            state,
            row.project_id,
            row.api_key_id,
            row.service_account_id,
        )
        .await?
        .providers
        .get(&row.provider)
        .ok_or_else(unavailable)?,
    };
    let egress = provider.videos(&row.upstream_model).ok_or_else(|| {
        error(
            ErrorKind::ProviderError,
            502,
            format!("{} no longer serves this video's model.", provider.name()),
        )
    })?;
    let credential = crate::services::credentials::for_provider(
        state,
        OrgId::from_uuid(row.org_id),
        ProjectId::from_uuid(row.project_id),
        &provider,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "cannot load the provider key of a video");
        replaced_key()
    })?;
    if credential.id.uuid() != row.credential_id {
        return Err(replaced_key());
    }
    let platform = crate::services::agent_platform::egress(
        state,
        &provider,
        &credential,
        dialect,
        &row.upstream_model,
    )
    .await
    .map_err(crate::services::agent_platform::EgressError::into_kpp)?;
    let mut ectx = EgressCtx::for_kind(RequestKind::Video, StreamMode::Off)
        .with_provider(provider.id())
        .with_gemini_profile(provider.gemini())
        .with_videos_profile(egress.profile);
    let base = match &platform {
        Some(p) => {
            ectx = p.apply(ectx);
            p.base.clone()
        }
        None => pipeline::base_url(state, &provider)
            .cloned()
            .ok_or_else(setup)?,
    };
    let base = if egress.from_root {
        super::media::host_root(&base)
    } else {
        base
    };
    let auth = match &platform {
        Some(p) => Some(p.auth),
        None => egress.auth.or_else(|| provider.auth()),
    };
    Ok(Reached {
        provider,
        credential,
        platform,
        base,
        auth,
        dialect,
        ectx,
    })
}

fn replaced_key() -> KppError {
    error(
        ErrorKind::InvalidRequest,
        409,
        "The provider key that started this video was replaced, so Kaeso can't reach it.",
    )
    .with_param("video_id")
}

fn timeouts(state: &AppState) -> Timeouts {
    let u = &state.config.upstream;
    Timeouts {
        request: u.request_timeout,
        first_byte: u.first_byte_timeout,
        idle: u.stream_idle_timeout,
    }
}

async fn send(
    state: &AppState,
    up: &Reached,
    req: EncodedRequest,
) -> Result<(u16, HeaderList, Bytes), KppError> {
    let resp = state
        .upstream
        .send(Call {
            base_url: &up.base,
            auth: up.auth,
            guard: pipeline::guard_for(&up.provider),
            credential: up.secret(),
            request: Outgoing::from(req),
            stream: false,
            timeouts: timeouts(state),
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await
        .map_err(|e| pipeline::upstream_error_from(&e, &up.provider))?;
    let body = match resp.body {
        ResponseBody::Full(b) => b,
        ResponseBody::Stream(mut s) => {
            let mut all = Vec::new();
            while let Some(chunk) = s.next().await {
                all.extend_from_slice(&chunk.map_err(|e| pipeline::upstream_error(&e))?);
            }
            Bytes::from(all)
        }
    };
    Ok((resp.status, resp.headers, body))
}

async fn poll(state: &AppState, up: &Reached, row: &VideoRow) -> Result<VideoJob, KppError> {
    let req =
        dispatch::encode_video_poll(up.dialect, &row.upstream_id, &row.upstream_model, &up.ectx)?;
    let (status, headers, body) = send(state, up, req).await?;
    dispatch::decode_video_poll(up.dialect, status, &headers, &body, &up.ectx)
}

/// The job after asking its provider, for a job that hasn't ended. Asking is best effort: when
/// the provider can't answer, the app gets the job as Kaeso last saw it and asks again later.
async fn refresh(asking: &Asking<'_>, row: VideoRow) -> Result<VideoRow, KppError> {
    let state = asking.state;
    if VideoStatus::from_wire(&row.status).is_some_and(VideoStatus::is_done) {
        return Ok(row);
    }
    let job = match reach(asking, &row).await {
        Ok(up) => match poll(state, &up, &row).await {
            Ok(job) => job,
            Err(e) => {
                tracing::warn!(provider = %row.provider, kind = e.kind.as_str(), error = %e.message, "a provider did not say how a video job stands");
                return Ok(row);
            }
        },
        Err(e) if matches!(e.status, Some(409 | 429)) => return Err(e),
        Err(e) => {
            tracing::warn!(provider = %row.provider, error = %e.message, "cannot reach a video job's provider");
            return Ok(row);
        }
    };
    Box::pin(settle(state, row, job)).await
}

/// Writes what the provider said; the first request to see the job end bills it.
async fn settle(state: &AppState, mut row: VideoRow, job: VideoJob) -> Result<VideoRow, KppError> {
    let now = state.clock.now();
    let done = job.status.is_done();
    let progress = VideoProgress {
        status: job.status.as_str().to_owned(),
        progress: progress_of(&job),
        error: job.error.as_ref().map(error_value),
        completed_at: done.then(|| at(job.completed_at).unwrap_or(now)),
        expires_at: at(job.expires_at),
    };
    let failed_store = |e: kaeso_store::StoreError| {
        tracing::error!(error = %e, "cannot keep a video job's progress");
        unavailable()
    };
    if !done {
        store::update_progress(&state.pg, row.org_id, row.id, &progress)
            .await
            .map_err(failed_store)?;
    } else {
        let units = units_of_job(&row, &job);
        let prices = row
            .prices
            .as_ref()
            .and_then(|p| serde_json::from_value::<Fields>(p.clone()).ok());
        let usage = job.usage.clone().unwrap_or_default();
        let cost = (job.status == VideoStatus::Completed)
            .then(|| {
                prices
                    .as_ref()
                    .map(|p| kaeso_core::cost::cost_with_media(&usage, p, Some(&units)))
            })
            .flatten();
        let won = store::account(
            &state.pg,
            row.org_id,
            row.id,
            &progress,
            serde_json::to_value(&units).ok().as_ref(),
            cost,
            now,
        )
        .await
        .map_err(failed_store)?;
        if won && job.status == VideoStatus::Completed {
            record_completion(state, &row, &job, units.clone(), prices).await;
        }
        if won {
            row.units = serde_json::to_value(&units).ok();
            row.cost = cost;
            row.accounted_at = Some(now);
        } else if let Ok(Some(fresh)) =
            store::get(&state.pg, row.org_id, row.project_id, row.id).await
        {
            return Ok(fresh);
        }
    }
    row.status = progress.status;
    row.progress = progress.progress;
    row.error = progress.error;
    row.completed_at = progress.completed_at;
    if progress.expires_at.is_some() {
        row.expires_at = progress.expires_at;
    }
    Ok(row)
}

/// What a finished job made, as its provider reports it, or as it was asked for where the
/// provider doesn't say.
fn units_of_job(row: &VideoRow, job: &VideoJob) -> MediaUnits {
    let videos = job
        .outputs
        .iter()
        .filter(|o| o.variant == VideoVariant::Video)
        .count() as u64;
    let asked = param_u32(&row.params, "count").map_or(1, u64::from);
    let count = if videos > 0 { videos } else { asked };
    let seconds = job.seconds.or_else(|| param_u32(&row.params, "seconds"));
    let size = job
        .size
        .as_ref()
        .map(|s| s.as_wire().to_owned())
        .or_else(|| param_str(row, "size"));
    let resolution = job
        .resolution
        .clone()
        .or_else(|| param_str(row, "resolution"))
        .or_else(|| resolution_of(None, size.as_deref().map(ImageSize::from_wire).as_ref()))
        .map(|r| r.to_ascii_lowercase());
    MediaUnits {
        input_images: param_u32(&row.params, "images").map_or(0, u64::from),
        input_ms: seconds_ms(param_u32(&row.params, "source_seconds")),
        output_videos: count,
        output_ms: seconds_ms(seconds).saturating_mul(count),
        size,
        resolution,
        audio: job
            .audio
            .or_else(|| row.params.get("audio").and_then(Value::as_bool)),
        ..MediaUnits::default()
    }
}

async fn record_completion(
    state: &AppState,
    row: &VideoRow,
    job: &VideoJob,
    units: MediaUnits,
    prices: Option<Fields>,
) {
    let now = state.clock.now();
    let mut o = pipeline::blank_outcome(RequestId::new());
    o.caller = Some(Caller {
        org_id: row.org_id,
        project_id: row.project_id,
        key_id: row.api_key_id,
        user_id: row.user_id,
        service_account_id: row.service_account_id,
    });
    o.ingress = D;
    o.kind = RequestKind::Video;
    if from_playground(row) {
        o.source = super::hooks::Source::Playground;
    }
    o.egress = row.dialect.parse().ok();
    match kaeso_core::provider::provider(&row.provider) {
        Some(p) => o.provider = Some(p.id),
        None => {
            o.provider = Some(kaeso_core::providers::METRIC_LABEL);
            o.custom_provider = Some(row.provider.clone());
        }
    }
    o.requested_model = Some(row.model.clone());
    o.catalogue_model_id = row.catalogue_model_id;
    o.model_id = Some(row.upstream_model.clone());
    o.response_model = Some(if job.model.is_empty() {
        row.upstream_model.clone()
    } else {
        job.model.clone()
    });
    o.prices = prices;
    o.status = 200;
    o.usage = job.usage.clone();
    o.started_at = now;
    o.upstream_request_id = Some(row.upstream_id.clone());
    o.media = Some(units);
    o.video = Some(VideoCall::Completed);
    state.sink.finish(state, o).await;
}

/// `GET /v1/videos/{video_id}/content`: a file of a completed video, streamed from where its
/// provider keeps it, with `Range` passed through.
pub(super) async fn content(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = video_id(&uri);
    let fetched = async {
        let viewer = Viewer::app(&*super::auth::authenticate(&state, &headers, ip.0).await?);
        content_as(&state, &viewer, &headers, &id, uri.query().unwrap_or("")).await
    }
    .await;
    match fetched {
        Ok(resp) => resp,
        Err(e) => refuse(&e),
    }
}

/// A file of a completed video, streamed from where its provider keeps it, with `Range`
/// passed through.
pub(crate) async fn content_as(
    state: &AppState,
    viewer: &Viewer,
    headers: &HeaderMap,
    id: &str,
    query: &str,
) -> Result<Response, KppError> {
    let variant = openai_videos::decode_content_query(query)?;
    let asking = Asking::new(state, viewer);
    let row = refresh(&asking, load(state, viewer, id).await?).await?;
    if row.status != VideoStatus::Completed.as_str() {
        return Err(error(
            ErrorKind::InvalidRequest,
            400,
            format!(
                "`{id}` is {}; its files are ready once it is completed.",
                row.status
            ),
        )
        .with_param("video_id"));
    }
    if row.expires_at.is_some_and(|t| t <= state.clock.now()) {
        return Err(error(
            ErrorKind::NotFound,
            404,
            format!("The files of `{id}` {EXPIRED}"),
        )
        .with_param("video_id"));
    }
    let finished = state.caches.finished_videos.get(&row.id).await;
    if finished.is_some() {
        asking
            .counted
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let up = reach(&asking, &row).await?;
    let job = match finished {
        Some(job) => job,
        None => {
            let job = Arc::new(poll(state, &up, &row).await?);
            state
                .caches
                .finished_videos
                .insert(row.id, job.clone())
                .await;
            job
        }
    };
    let Some(output) = job.output(variant) else {
        return Err(error(
            ErrorKind::NotFound,
            404,
            format!(
                "{} offers no {} for this video.",
                up.provider.name(),
                variant.as_str()
            ),
        )
        .with_param("variant"));
    };
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let media_type = output.media_type.clone();
    match &output.source {
        Source::Base64(b) => inline(
            &b.data,
            media_type.unwrap_or_else(|| b.media_type.clone()),
            range.as_deref(),
        ),
        Source::Url(link) => stream_link(state, &up, link, range.as_deref(), media_type).await,
        Source::FileId(f) => {
            let req = dispatch::encode_video_file(up.dialect, &f.id, &up.ectx)?;
            let (status, h, body) = send(state, &up, req).await?;
            match dispatch::decode_video_file(up.dialect, status, &h, &body)? {
                Source::Url(link) => {
                    stream_link(state, &up, &link, range.as_deref(), media_type).await
                }
                _ => Err(KppError::provider(
                    "The provider named no link for the video.",
                )),
            }
        }
        _ => Err(KppError::provider(
            "The provider named no file for the video.",
        )),
    }
}

/// A file the provider sent inside its answer, as Veo and Gemini Omni do.
fn inline(bytes: &Bytes, media_type: String, range: Option<&str>) -> Result<Response, KppError> {
    let total = bytes.len();
    let (status, body, content_range) = match range.and_then(|r| byte_range(r, total)) {
        Some((start, end)) if end < total => (
            StatusCode::PARTIAL_CONTENT,
            bytes.slice(start..=end),
            Some(format!("bytes {start}-{end}/{total}")),
        ),
        _ => (StatusCode::OK, bytes.clone(), None),
    };
    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = status;
    let h = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&media_type) {
        h.insert(header::CONTENT_TYPE, v);
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Some(cr) = content_range.and_then(|c| HeaderValue::from_str(&c).ok()) {
        h.insert(header::CONTENT_RANGE, cr);
    }
    Ok(resp)
}

/// One `bytes=start-end` range of RFC 9110 section 14.1.2 within `total` bytes.
fn byte_range(range: &str, total: usize) -> Option<(usize, usize)> {
    let spec = range.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let last = total - 1;
    match (start.trim(), end.trim()) {
        ("", n) => {
            let n: usize = n.parse().ok()?;
            (n > 0).then(|| (total.saturating_sub(n), last))
        }
        (s, "") => {
            let s: usize = s.parse().ok()?;
            (s <= last).then_some((s, last))
        }
        (s, e) => {
            let (s, e): (usize, usize) = (s.parse().ok()?, e.parse().ok()?);
            (s <= e && s <= last).then_some((s, e.min(last)))
        }
    }
}

const PASSED_HEADERS: &[header::HeaderName] = &[
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::ETAG,
    header::LAST_MODIFIED,
];

fn same_origin(link: &Url, base: &Url) -> bool {
    link.host_str() == base.host_str()
        && link.port_or_known_default() == base.port_or_known_default()
}

// A link on another host is the provider's choice of address, so it never gets the reach an
// admin opened for custom providers' own base URLs.
fn link_guard(own_host: bool, provider: &ProviderRef) -> kaeso_upstream::Guard {
    if own_host {
        pipeline::guard_for(provider)
    } else {
        kaeso_upstream::Guard::PublicOnly
    }
}

/// Streams a file from a link the provider named: a path at the provider when it starts with
/// `/`, otherwise a full URL. The provider's key goes only to the provider's own host.
async fn stream_link(
    state: &AppState,
    up: &Reached,
    link: &str,
    range: Option<&str>,
    media_type: Option<String>,
) -> Result<Response, KppError> {
    let mut url = if link.starts_with('/') {
        Url::parse(&format!("{}{link}", up.base.as_str().trim_end_matches('/')))
    } else {
        Url::parse(link)
    }
    .map_err(|_| KppError::provider("The provider named a link Kaeso cannot read."))?;
    let unreachable = || KppError::provider("The provider named a link Kaeso cannot fetch.");
    for _ in 0..=MAX_REDIRECTS {
        if !matches!(url.scheme(), "https" | "http") {
            return Err(unreachable());
        }
        let own_host = same_origin(&url, &up.base);
        let mut origin = url.clone();
        origin.set_path("/");
        origin.set_query(None);
        let path = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_owned(),
        };
        let mut headers = HeaderList::new();
        if let Some(r) = range {
            headers.push("range", r);
        }
        let empty = secrecy::SecretString::from(String::new());
        let resp = state
            .upstream
            .send(Call {
                base_url: &origin,
                auth: if own_host { up.auth } else { None },
                guard: link_guard(own_host, &up.provider),
                credential: if own_host { up.secret() } else { &empty },
                request: Outgoing {
                    method: "GET",
                    path,
                    headers,
                    body: Bytes::new(),
                },
                stream: true,
                timeouts: timeouts(state),
                max_body: state.config.body_limits.provider_response.as_usize(),
            })
            .await
            .map_err(|e| pipeline::upstream_error_from(&e, &up.provider))?;
        if (300..400).contains(&resp.status)
            && let Some(location) = resp.headers.get("location")
        {
            url = url.join(location).map_err(|_| unreachable())?;
            continue;
        }
        if !(200..300).contains(&resp.status) {
            tracing::warn!(provider = %up.provider.id(), status = resp.status, "a video's file did not download");
            return Err(error(
                ErrorKind::ProviderError,
                502,
                format!(
                    "{} did not hand over the video's file; try again shortly.",
                    up.provider.name()
                ),
            ));
        }
        let mut out = Response::new(match resp.body {
            ResponseBody::Full(b) => Body::from(b),
            ResponseBody::Stream(s) => Body::from_stream(s),
        });
        *out.status_mut() = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);
        let h = out.headers_mut();
        for name in PASSED_HEADERS {
            if let Some(v) = resp.headers.get(name.as_str())
                && let Ok(v) = HeaderValue::from_str(v)
            {
                h.insert(name.clone(), v);
            }
        }
        if let Some(t) = media_type.filter(|_| !h.contains_key(header::CONTENT_TYPE))
            && let Ok(v) = HeaderValue::from_str(&t)
        {
            h.insert(header::CONTENT_TYPE, v);
        }
        return Ok(out);
    }
    Err(unreachable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_on_another_host_are_fetched_from_public_addresses_only() {
        use kaeso_upstream::Guard;
        let base = Url::parse("https://api.x.ai/v1").unwrap();
        let xai = ProviderRef::from(kaeso_core::provider::provider("xai").unwrap());
        for (link, guard) in [
            ("https://api.x.ai/v1/files/a.mp4", Guard::Off),
            ("https://api.x.ai:443/a.mp4", Guard::Off),
            ("https://vidgen.x.ai/a.mp4", Guard::PublicOnly),
            ("https://api.x.ai:8443/a.mp4", Guard::PublicOnly),
            ("http://10.0.0.7/a.mp4", Guard::PublicOnly),
        ] {
            let link = Url::parse(link).unwrap();
            assert_eq!(link_guard(same_origin(&link, &base), &xai), guard, "{link}");
        }
    }

    #[test]
    fn ranges_follow_rfc_9110() {
        assert_eq!(byte_range("bytes=0-9", 100), Some((0, 9)));
        assert_eq!(byte_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(byte_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(byte_range("bytes=50-500", 100), Some((50, 99)));
        assert_eq!(byte_range("bytes=100-", 100), None);
        assert_eq!(byte_range("bytes=0-1,4-5", 100), None);
        assert_eq!(byte_range("items=0-1", 100), None);
        assert_eq!(byte_range("bytes=5-1", 100), None);
        assert_eq!(byte_range("bytes=-0", 100), None);
    }

    #[test]
    fn requests_are_logged_without_their_images_and_counted_by_what_they_ask() {
        let mut r = VideoRequest::new("m", kaeso_kpp::VideoOperation::Generate)
            .with_prompt("fox")
            .with_image(kaeso_kpp::VideoImage::new(
                Source::base64("image/png", "AAAAAAAA").unwrap(),
                kaeso_kpp::VideoImageRole::FirstFrame,
            ));
        r.seconds = Some(8);
        r.size = Some(ImageSize::pixels(1280, 720));
        let kept = String::from_utf8(
            logged_request(&r, crate::services::log_media::Keep::media()).to_vec(),
        )
        .unwrap();
        assert!(kept.contains("AAAAAAAA"), "{kept}");
        let logged = String::from_utf8(
            logged_request(&r, crate::services::log_media::Keep::nothing()).to_vec(),
        )
        .unwrap();
        assert!(!logged.contains("AAAAAAAA") && logged.contains(r#""bytes":6"#));
        let units = units_of_request(&r);
        assert_eq!((units.output_videos, units.output_ms), (1, 8000));
        assert_eq!(units.resolution.as_deref(), Some("720p"));
        assert!(
            refuse_sora(&VideoRequest::new(
                "sora-2",
                kaeso_kpp::VideoOperation::Generate
            ))
            .is_err()
        );
        assert!(refuse_sora(&r).is_ok());
    }
}
