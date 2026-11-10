//! ElevenLabs' image generations for ElevenLabs-format apps: `POST /v1/flows/image` starts a
//! generation through the model pipeline, which keeps its provider, key and upstream id in
//! PostgreSQL under an id of Kaeso's own. `GET /v1/flows/image/{generation_id}` asks the
//! provider how it stands, and the first answer that it has ended bills it, once;
//! `GET /v1/flows/image` lists the project's generations as Kaeso last saw them.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::elevenlabs::{
    Generation, GenerationStatus, decode_generation, decode_generation_query, encode_generation,
    encode_generation_list,
};
use kaeso_codecs::{EgressCtx, dispatch};
use kaeso_core::catalogue::{Effective, Fields};
use kaeso_core::cost::MediaUnits;
use kaeso_core::id::{CredentialId, ImageGenerationId, OrgId, ProjectId, RequestId};
use kaeso_core::provider::AuthStyle;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind, StreamMode};
use kaeso_store::pg::image_generations::{
    self as store, ImageGenerationFilter, ImageGenerationRow, ImageGenerationState,
};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use url::Url;

use super::errors::{Served, error, respond};
use super::hooks::{Caller, GenerationCall};
use super::pipeline::{self, Incoming, Recording};
use crate::http::client_ip::ClientIp;
use crate::services::credentials::Resolved;
use crate::state::AppState;

const D: DialectId = DialectId::ElevenLabs;

/// The generation an image provider just started, and what started it.
pub(super) struct Started<'a> {
    pub caller: &'a Caller,
    pub entry: uuid::Uuid,
    pub eff: &'a Effective,
    pub credential: CredentialId,
    pub dialect: DialectId,
    pub app: &'a kaeso_kpp::Request,
}

/// Only ElevenLabs makes an image as a generation an app asks about later; other providers
/// answer with the image itself.
pub(super) fn not_elevenlabs(eff: &Effective) -> KppError {
    KppError::unsupported_by(
        format!("image generations by {}", eff.provider.name()),
        D,
        "Use `/v1/images/generations` for this model, or choose an ElevenLabs model.",
    )
    .with_status(400)
    .with_param("model_id")
}

fn refuse(err: &KppError) -> Response {
    respond(D, err, &Served::default())
}

fn answer(body: Result<Vec<u8>, KppError>) -> Response {
    match body {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp
        }
        Err(e) => refuse(&e),
    }
}

fn not_found(id: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("No image generation `{id}` belongs to this project."),
    )
    .with_param("generation_id")
}

fn unavailable() -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        "Kaeso cannot read its image generations right now; try again shortly.",
    )
}

fn status_of(row: &ImageGenerationRow) -> GenerationStatus {
    GenerationStatus::from_wire(&row.status).unwrap_or(GenerationStatus::Pending)
}

/// The generation as the app sees it: under Kaeso's id, with the link Kaeso last got.
fn app_view(row: &ImageGenerationRow) -> Generation {
    let mut g = Generation::new(
        ImageGenerationId::from_uuid(row.id).to_string(),
        status_of(row),
    );
    g.content_url.clone_from(&row.content_url);
    g.content_mime_type.clone_from(&row.content_mime_type);
    g.failure_reason.clone_from(&row.failure_reason);
    g.error_message.clone_from(&row.error_message);
    g
}

fn state_of(g: &Generation, completed_at: Option<time::OffsetDateTime>) -> ImageGenerationState {
    ImageGenerationState {
        status: g.status.as_wire().to_owned(),
        content_url: g.content_url.clone(),
        content_mime_type: g.content_mime_type.clone(),
        failure_reason: g.failure_reason.clone(),
        error_message: g.error_message.clone(),
        completed_at: g.status.is_done().then_some(completed_at).flatten(),
    }
}

/// Keeps the generation the provider started under an id of Kaeso's own, and answers with it
/// as the app sees it.
pub(super) async fn register(
    state: &AppState,
    rec: &mut Recording,
    s: &Started<'_>,
    started: Generation,
) -> Result<Vec<u8>, KppError> {
    let kaeso_kpp::Request::Image(app) = s.app else {
        return Err(KppError::provider(
            "An image generation started for another kind of request.",
        ));
    };
    let now = state.clock.now();
    let id = ImageGenerationId::new();
    let asked = super::media::units_of_request(app);
    let upstream = state_of(&started, Some(now));
    let row = ImageGenerationRow {
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
        upstream_id: started.id.clone(),
        request_id: rec.outcome().request_id.uuid(),
        params: serde_json::to_value(&asked).unwrap_or_default(),
        prices: serde_json::to_value(&s.eff.fields).ok(),
        status: upstream.status,
        content_url: upstream.content_url,
        content_mime_type: upstream.content_mime_type,
        failure_reason: upstream.failure_reason,
        error_message: upstream.error_message,
        units: None,
        cost: None,
        created_at: now,
        completed_at: upstream.completed_at,
        accounted_at: None,
    };
    store::insert(&state.pg, &row).await.map_err(|e| {
        tracing::error!(error = %e, "cannot keep a started image generation");
        error(
            ErrorKind::ProviderError,
            503,
            "Kaeso started the image but cannot keep track of it; try again shortly.",
        )
    })?;
    let row = if started.status.is_done() {
        Box::pin(settle(state, row, &started)).await?
    } else {
        row
    };
    // ElevenLabs documents the create call's answer as always `pending`, whatever follows.
    encode_generation(&Generation::new(
        ImageGenerationId::from_uuid(row.id).to_string(),
        GenerationStatus::Pending,
    ))
}

/// `POST /v1/flows/image`.
pub(super) async fn create(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let max_body = state.config.body_limits.images.as_usize();
    pipeline::serve(
        state,
        Incoming {
            ingress: D,
            kind: RequestKind::Image,
            headers: parts.headers,
            body,
            request_id,
            client_ip: ip.0,
            caller: None,
            source: super::hooks::Source::Api,
            stream_only: false,
            max_body,
            alias: None,
            gemini: None,
            image: None,
            transcription: None,
            elevenlabs: None,
            video: None,
            generation: Some(GenerationCall::Create),
        },
    )
    .await
}

// An id that isn't Kaeso's or belongs to another project is answered the same way, so ids
// never tell whether a generation exists elsewhere.
async fn load(state: &AppState, caller: &Caller, id: &str) -> Result<ImageGenerationRow, KppError> {
    let parsed: ImageGenerationId = id.parse().map_err(|_| not_found(id))?;
    store::get(&state.pg, caller.org_id, caller.project_id, parsed.uuid())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot load an image generation");
            unavailable()
        })?
        .ok_or_else(|| not_found(id))
}

fn generation_id(uri: &axum::http::Uri) -> String {
    uri.path()
        .strip_prefix("/v1/flows/image/")
        .unwrap_or_default()
        .to_owned()
}

/// `GET /v1/flows/image/{generation_id}`: the generation, after asking its provider how it
/// stands, with a fresh link once it is completed.
pub(super) async fn retrieve(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = generation_id(&uri);
    let found = async {
        let key = super::auth::authenticate(&state, &headers, ip.0).await?;
        let caller = pipeline::key_caller(&key);
        let row = load(&state, &caller, &id).await?;
        let row = refresh(&state, &caller, row).await?;
        encode_generation(&app_view(&row))
    }
    .await;
    answer(found)
}

/// `GET /v1/flows/image`: the project's generations, newest first, as Kaeso last saw them,
/// without asking providers.
pub(super) async fn list(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let listed = async {
        let key = super::auth::authenticate(&state, &headers, ip.0).await?;
        let caller = pipeline::key_caller(&key);
        let q = decode_generation_query(uri.query().unwrap_or(""))?;
        let before = match &q.cursor {
            Some(c) => Some(
                load(&state, &caller, c)
                    .await
                    .map_err(|e| {
                        if e.status == Some(404) {
                            error(
                                ErrorKind::InvalidRequest,
                                400,
                                format!(
                                    "`cursor` names no image generation of this project: `{c}`."
                                ),
                            )
                            .with_param("cursor")
                        } else {
                            e
                        }
                    })?
                    .id,
            ),
            None => None,
        };
        let filter = ImageGenerationFilter {
            before,
            status: q.status.map(|s| s.as_wire().to_owned()),
            model: q.model_id.clone(),
        };
        let size = usize::try_from(q.page_size).unwrap_or(usize::MAX);
        let rows = store::list(
            &state.pg,
            caller.org_id,
            caller.project_id,
            &filter,
            i64::from(q.page_size) + 1,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot list image generations");
            unavailable()
        })?;
        let has_more = rows.len() > size;
        let page: Vec<Generation> = rows.iter().take(size).map(app_view).collect();
        let next = has_more
            .then(|| page.last().map(|g| g.id.clone()))
            .flatten();
        encode_generation_list(&page, next.as_deref(), has_more)
    }
    .await;
    answer(listed)
}

/// Where and how a generation's provider is reached: with the key that started it.
struct Reached {
    provider: ProviderRef,
    credential: Arc<Resolved>,
    base: Url,
    auth: Option<AuthStyle>,
    dialect: DialectId,
}

fn replaced_key() -> KppError {
    error(
        ErrorKind::InvalidRequest,
        409,
        "The provider key that started this image was replaced, so Kaeso can't reach it.",
    )
    .with_param("generation_id")
}

// Asking about a generation spends the provider's quota with the org's key, so it counts
// against the key's request rates.
async fn reach(
    state: &AppState,
    caller: &Caller,
    row: &ImageGenerationRow,
) -> Result<Reached, KppError> {
    pipeline::admit_follow_up(state, caller, row.catalogue_model_id).await?;
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
    let egress = super::media::egress_of(&provider, RequestKind::Image, &row.upstream_model)
        .ok_or_else(|| {
            error(
                ErrorKind::ProviderError,
                502,
                format!("{} no longer serves this image's model.", provider.name()),
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
        tracing::warn!(error = %e, "cannot load the provider key of an image generation");
        replaced_key()
    })?;
    if credential.id.uuid() != row.credential_id {
        return Err(replaced_key());
    }
    let base = pipeline::base_url(state, &provider)
        .cloned()
        .ok_or_else(|| {
            error(
                ErrorKind::ProviderError,
                500,
                "No base URL is configured for this provider.",
            )
        })?;
    let base = if egress.from_root {
        super::media::host_root(&base)
    } else {
        base
    };
    let auth = egress.auth.or_else(|| provider.auth());
    Ok(Reached {
        provider,
        credential,
        base,
        auth,
        dialect,
    })
}

async fn poll(
    state: &AppState,
    up: &Reached,
    row: &ImageGenerationRow,
) -> Result<Generation, KppError> {
    let ctx =
        EgressCtx::for_kind(RequestKind::Image, StreamMode::Off).with_provider(up.provider.id());
    let req = dispatch::encode_task_poll(up.dialect, &row.upstream_id, &ctx)?;
    let u = &state.config.upstream;
    let resp = state
        .upstream
        .send(Call {
            base_url: &up.base,
            auth: up.auth,
            guard: pipeline::guard_for(&up.provider),
            credential: &up.credential.secret,
            request: Outgoing::from(req),
            stream: false,
            timeouts: Timeouts {
                request: u.request_timeout,
                first_byte: u.first_byte_timeout,
                idle: u.stream_idle_timeout,
            },
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
    decode_generation(resp.status, &resp.headers, &body)
}

/// The generation after asking its provider, for one that hasn't failed: a running one may
/// have ended, and a completed one gets a fresh link, since ElevenLabs' links last about an
/// hour. Asking is best effort: when the provider can't answer, the app gets the generation
/// as Kaeso last saw it.
async fn refresh(
    state: &AppState,
    caller: &Caller,
    row: ImageGenerationRow,
) -> Result<ImageGenerationRow, KppError> {
    if status_of(&row) == GenerationStatus::Failed {
        return Ok(row);
    }
    let seen = match reach(state, caller, &row).await {
        Ok(up) => match poll(state, &up, &row).await {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(provider = %row.provider, kind = e.kind.as_str(), error = %e.message, "a provider did not say how an image generation stands");
                return Ok(row);
            }
        },
        Err(e) if matches!(e.status, Some(409 | 429)) => return Err(e),
        Err(e) => {
            tracing::warn!(provider = %row.provider, error = %e.message, "cannot reach an image generation's provider");
            return Ok(row);
        }
    };
    if status_of(&row) == GenerationStatus::Completed && seen.status != GenerationStatus::Completed
    {
        return Ok(row);
    }
    Box::pin(settle(state, row, &seen)).await
}

/// Writes what the provider said; the first request to see the generation end bills it.
async fn settle(
    state: &AppState,
    mut row: ImageGenerationRow,
    seen: &Generation,
) -> Result<ImageGenerationRow, KppError> {
    let now = state.clock.now();
    let failed_store = |e: kaeso_store::StoreError| {
        tracing::error!(error = %e, "cannot keep an image generation's progress");
        unavailable()
    };
    let next = state_of(seen, Some(row.completed_at.unwrap_or(now)));
    store::update(&state.pg, row.org_id, row.id, &next)
        .await
        .map_err(failed_store)?;
    row.status.clone_from(&next.status);
    row.content_url = next.content_url;
    row.content_mime_type = next.content_mime_type;
    row.failure_reason = next.failure_reason;
    row.error_message = next.error_message;
    row.completed_at = row.completed_at.or(next.completed_at);
    if !seen.status.is_done() || row.accounted_at.is_some() {
        return Ok(row);
    }
    let completed = seen.status == GenerationStatus::Completed;
    let mut units = serde_json::from_value::<MediaUnits>(row.params.clone()).unwrap_or_default();
    units.output_images = u64::from(completed);
    let prices = row
        .prices
        .as_ref()
        .and_then(|p| serde_json::from_value::<Fields>(p.clone()).ok());
    let cost = completed
        .then(|| {
            prices.as_ref().map(|p| {
                kaeso_core::cost::cost_with_media(&kaeso_kpp::Usage::default(), p, Some(&units))
            })
        })
        .flatten();
    let won = store::account(
        &state.pg,
        row.org_id,
        row.id,
        serde_json::to_value(&units).ok().as_ref(),
        cost,
        now,
    )
    .await
    .map_err(failed_store)?;
    if won {
        if completed {
            record_completion(state, &row, units.clone(), prices).await;
        }
        row.units = serde_json::to_value(&units).ok();
        row.cost = cost;
        row.accounted_at = Some(now);
    }
    Ok(row)
}

async fn record_completion(
    state: &AppState,
    row: &ImageGenerationRow,
    units: MediaUnits,
    prices: Option<Fields>,
) {
    let mut o = pipeline::blank_outcome(RequestId::new());
    o.caller = Some(Caller {
        org_id: row.org_id,
        project_id: row.project_id,
        key_id: row.api_key_id,
        user_id: row.user_id,
        service_account_id: row.service_account_id,
    });
    o.ingress = D;
    o.kind = RequestKind::Image;
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
    o.response_model = Some(row.upstream_model.clone());
    o.prices = prices;
    o.status = 200;
    o.started_at = state.clock.now();
    o.upstream_request_id = Some(row.upstream_id.clone());
    o.media = Some(units);
    o.generation = Some(GenerationCall::Completed);
    state.sink.finish(state, o).await;
}
