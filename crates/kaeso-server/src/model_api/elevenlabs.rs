//! ElevenLabs-format apps: the official `elevenlabs` SDKs with Kaeso as their base URL. Speech,
//! dialogue, sound effects, music, the voice changer and speech to text go through the model
//! pipeline; voice listings are the account's own at ElevenLabs and are asked of ElevenLabs
//! with the project's key; account, cloning and dubbing management stay at ElevenLabs.

use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use kaeso_codecs::elevenlabs::{ElevenLabsCall, voice_listing_request, voices_reply};
use kaeso_core::id::{OrgId, ProjectId, RequestId};
use kaeso_core::provider::provider;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError};
use kaeso_upstream::{Call, Outgoing, ResponseBody};

use super::errors::{Served, error, respond};
use super::pipeline::{self, Incoming};
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

const D: DialectId = DialectId::ElevenLabs;

fn refuse(err: &KppError) -> Response {
    respond(D, err, &Served::default())
}

/// A generation call: its path names the operation and the voice.
pub(super) async fn generate(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    OriginalUri(uri): OriginalUri,
    req: Request,
) -> Response {
    let Some(call) = ElevenLabsCall::from_path(uri.path(), uri.query().unwrap_or("")) else {
        return refuse(&unserved(uri.path()));
    };
    let (parts, body) = req.into_parts();
    let limits = &state.config.body_limits;
    let uploads = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| {
            t.trim_start()
                .to_ascii_lowercase()
                .starts_with("multipart/")
        });
    let max_body = if uploads {
        limits.audio.as_usize()
    } else {
        limits.model_api.as_usize()
    };
    let kind = call.kind();
    pipeline::serve(
        state,
        Incoming {
            ingress: D,
            kind,
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
            elevenlabs: Some(call),
            video: None,
        },
    )
    .await
}

fn unserved(path: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!(
            "Kaeso does not serve `{path}`; manage voices, dubbing and your account at ElevenLabs."
        ),
    )
}

/// What an ElevenLabs app calls that Kaeso doesn't route: account, cloning, dubbing, history
/// and the rest of ElevenLabs' management API.
pub(super) async fn management(OriginalUri(uri): OriginalUri) -> Response {
    refuse(&unserved(uri.path()))
}

/// `GET /v1/voices`, `GET /v2/voices` and `GET /v1/voices/{voice_id}`, asked of ElevenLabs with
/// the project's ElevenLabs key.
pub(super) async fn voices(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    match relay_voices(&state, &headers, ip, uri.path(), uri.query().unwrap_or("")).await {
        Ok(resp) => resp,
        Err(e) => refuse(&e),
    }
}

async fn relay_voices(
    state: &AppState,
    headers: &HeaderMap,
    ip: ClientIp,
    path: &str,
    query: &str,
) -> Result<Response, KppError> {
    let key = super::auth::authenticate(state, headers, ip.0).await?;
    let encoded = voice_listing_request(path, query).map_err(|e| {
        if e.status == Some(404) {
            unserved(path)
        } else {
            e
        }
    })?;
    let Some(builtin) = provider("elevenlabs") else {
        return Err(unserved(path));
    };
    let elevenlabs = ProviderRef::from(builtin);
    let credential = crate::services::credentials::for_provider(
        state,
        OrgId::from_uuid(key.org_id),
        ProjectId::from_uuid(key.project_id),
        &elevenlabs,
    )
    .await
    .map_err(|e| match e {
        crate::services::credentials::ResolveError::Missing { .. } => {
            error(ErrorKind::InvalidRequest, 400, e.to_string())
        }
        other => {
            tracing::error!(error = %other, "cannot load the ElevenLabs credential");
            error(
                ErrorKind::ProviderError,
                503,
                "Kaeso cannot load the provider credential right now. Try again shortly.",
            )
        }
    })?;
    let base = pipeline::base_url(state, &elevenlabs).ok_or_else(|| {
        error(
            ErrorKind::ProviderError,
            500,
            "No base URL is configured for this provider.",
        )
    })?;
    let u = &state.config.upstream;
    let resp = state
        .upstream
        .send(Call {
            base_url: base,
            auth: elevenlabs.auth(),
            guarded: false,
            credential: &credential.secret,
            request: Outgoing::from(encoded),
            stream: false,
            timeouts: kaeso_upstream::Timeouts {
                request: u.request_timeout,
                first_byte: u.first_byte_timeout,
                idle: u.stream_idle_timeout,
            },
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await
        .map_err(|e| pipeline::upstream_error(&e))?;
    let bytes = match resp.body {
        ResponseBody::Full(b) => b.to_vec(),
        ResponseBody::Stream(mut chunks) => {
            let mut all = Vec::new();
            while let Some(chunk) = futures::StreamExt::next(&mut chunks).await {
                all.extend_from_slice(&chunk.map_err(|e| pipeline::upstream_error(&e))?);
            }
            all
        }
    };
    let body = voices_reply(resp.status, &resp.headers, &bytes)?;
    let mut out = Response::new(axum::body::Body::from(body));
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    *out.status_mut() = StatusCode::OK;
    Ok(out)
}
