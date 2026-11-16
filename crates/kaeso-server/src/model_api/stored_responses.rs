//! `GET` and `DELETE /v1/responses/{response_id}`: a response its provider keeps for the
//! calling project, read or deleted with the provider key that stored it.

use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind, StreamMode};
use kaeso_store::pg::stored_responses::{self as store, StoredResponse};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};

use super::errors::{Served, error, respond};
use super::hooks::Caller;
use super::pipeline;
use crate::http::client_ip::ClientIp;
use crate::services::stored_responses::{id_hash, replaced};
use crate::state::AppState;

const D: DialectId = DialectId::OpenAiResponses;

fn response_id(uri: &axum::http::Uri) -> String {
    uri.path()
        .strip_prefix("/v1/responses/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned()
}

// An id that isn't one the project stored, whether another project's or no one's, is answered
// alike, so ids tell nothing about other projects.
fn not_found(id: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("No response found with id '{id}'."),
    )
    .with_param("response_id")
}

fn unavailable() -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        "Kaeso cannot read its stored responses right now. Try again shortly.",
    )
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
        Err(e) => respond(D, &e, &Served::default()),
    }
}

struct Found {
    row: StoredResponse,
    provider: ProviderRef,
    credential: std::sync::Arc<crate::services::credentials::Resolved>,
}

async fn find(state: &AppState, caller: &Caller, id: &str) -> Result<Found, KppError> {
    let well_formed = (1..=256).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !well_formed {
        return Err(not_found(id));
    }
    let row = store::find(
        &state.pg,
        caller.org_id,
        caller.project_id,
        &id_hash(id),
        state.clock.now(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "cannot read a stored response");
        unavailable()
    })?
    .ok_or_else(|| not_found(id))?;
    if !super::listing::ModelAccess::of(caller, state)
        .await?
        .allows(Some(row.catalogue_model_id))
    {
        return Err(not_found(id));
    }
    pipeline::admit_follow_up(state, caller, Some(row.catalogue_model_id)).await?;
    let provider = kaeso_core::provider::provider(&row.provider)
        .map(ProviderRef::from)
        .ok_or_else(|| not_found(id))?;
    let credential = crate::services::credentials::for_provider(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        &provider,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "cannot load the provider key of a stored response");
        replaced(id)
    })?;
    if credential.id.uuid() != row.credential_id {
        return Err(replaced(id));
    }
    Ok(Found {
        row,
        provider,
        credential,
    })
}

async fn send(
    state: &AppState,
    found: &Found,
    method: &'static str,
    id: &str,
) -> Result<(u16, HeaderList, Bytes), KppError> {
    let base = pipeline::base_url(state, &found.provider).ok_or_else(|| {
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
            auth: found.provider.auth(),
            guard: pipeline::guard_for(&found.provider),
            credential: &found.credential.secret,
            request: Outgoing {
                method,
                path: format!("/v1/responses/{id}"),
                headers: HeaderList::new().with("accept", "application/json"),
                body: Bytes::new(),
            },
            stream: false,
            timeouts: Timeouts {
                request: u.request_timeout,
                first_byte: u.first_byte_timeout,
                idle: u.stream_idle_timeout,
            },
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await
        .map_err(|e| pipeline::upstream_error_from(&e, &found.provider))?;
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

/// The stored response as its provider shows it, decoded and written again in the Responses
/// format, like any reply.
async fn retrieve_as(state: &AppState, caller: &Caller, id: &str) -> Result<Vec<u8>, KppError> {
    let found = find(state, caller, id).await?;
    let (status, headers, body) = send(state, &found, "GET", id).await?;
    let ectx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off)
        .with_provider(found.provider.id())
        .with_responses_profile(found.provider.responses());
    if status == 404 {
        return Err(not_found(id));
    }
    let reply = dispatch::decode_response(D, status, &headers, &body, &ectx)?;
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    dispatch::encode_response(D, &reply, &ctx)
}

/// Deletes the stored response at its provider and forgets it; one the provider no longer keeps
/// is forgotten too.
async fn delete_as(state: &AppState, caller: &Caller, id: &str) -> Result<Vec<u8>, KppError> {
    let found = find(state, caller, id).await?;
    let (status, headers, body) = send(state, &found, "DELETE", id).await?;
    if !(200..300).contains(&status) && status != 404 {
        return Err(dispatch::decode_error(D, status, &headers, &body));
    }
    store::delete(&state.pg, caller.project_id, &found.row.id_hash)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot forget a stored response");
            unavailable()
        })?;
    serde_json::to_vec(&serde_json::json!({
        "id": id,
        "object": "response.deleted",
        "deleted": true,
    }))
    .map_err(|e| KppError::provider(e.to_string()))
}

pub(super) async fn retrieve(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = response_id(&uri);
    answer(
        async {
            let key = super::auth::authenticate(&state, &headers, ip.0).await?;
            retrieve_as(&state, &pipeline::key_caller(&key), &id).await
        }
        .await,
    )
}

pub(super) async fn delete(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = response_id(&uri);
    let deleted = async {
        let key = super::auth::authenticate(&state, &headers, ip.0).await?;
        delete_as(&state, &pipeline::key_caller(&key), &id).await
    }
    .await;
    answer(deleted)
}
