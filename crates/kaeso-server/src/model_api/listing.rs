use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use kaeso_codecs::dispatch;
use kaeso_core::catalogue::capability::model_info;
use kaeso_core::catalogue::{Effective, Entry};
use kaeso_kpp::{DialectId, ErrorKind, KppError, ModelInfo, ModelLifecycle, ModelList};
use kaeso_store::pg::catalogue::ProjectModels;
use uuid::Uuid;

use super::auth;
use super::errors::{Served, error, respond};
use super::pipeline::header_list;
use crate::state::AppState;

pub(super) async fn project_models(
    state: &AppState,
    project: Uuid,
) -> Result<Arc<ProjectModels>, KppError> {
    crate::services::catalogue::project_models(state, project)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot load the project's models");
            error(
                ErrorKind::ProviderError,
                503,
                "Kaeso cannot load this project's models right now. Try again shortly.",
            )
        })
}

async fn visible(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
) -> Result<Vec<ModelInfo>, KppError> {
    let key = auth::authenticate(state, headers, client).await?;
    let models = project_models(state, key.project_id).await?;
    let access = models.access(Some(key.id), key.user_id, key.service_account_id);
    let mut out = Vec::new();
    let mut listed = HashMap::new();
    for entry in models.catalogue.entries() {
        if !access.allows(entry.id) || entry.is_unavailable() {
            continue;
        }
        if let Some(eff) = crate::services::catalogue::effective(state, entry) {
            out.push(info(state, &entry.model_id, entry, &eff));
            listed.entry(entry.id).or_insert((entry, eff));
        }
    }
    for alias in models.catalogue.aliases() {
        if let Some((entry, eff)) = listed.get(&alias.target) {
            out.push(info(state, &alias.name, entry, eff));
        }
    }
    Ok(out)
}

fn info(state: &AppState, name: &str, entry: &Entry, eff: &Effective) -> ModelInfo {
    let mut info = model_info(name, eff);
    if entry.is_deprecated(&state.models_dev.current().snapshot) {
        info.lifecycle = Some(ModelLifecycle::Deprecated);
    }
    info
}

fn json(body: Vec<u8>) -> Response {
    let mut resp = Response::new(Body::from(body));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp
}

pub async fn list(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    query: &str,
) -> Response {
    let dialect = dispatch::models_dialect(&header_list(headers));
    match list_inner(state, headers, client, query, dialect).await {
        Ok(body) => json(body),
        Err(e) => respond(dialect, &e, &Served::default()),
    }
}

async fn list_inner(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    query: &str,
    dialect: DialectId,
) -> Result<Vec<u8>, KppError> {
    let all = visible(state, headers, client).await?;
    let query = dispatch::decode_models_query(dialect, query)?;
    let default_limit = (dialect == DialectId::AnthropicMessages).then_some(20);
    let page = ModelList::page(&all, &query, default_limit)?;
    dispatch::encode_models(dialect, &page)
}

pub async fn get(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    id: &str,
) -> Response {
    let dialect = dispatch::models_dialect(&header_list(headers));
    match get_inner(state, headers, client, id).await {
        Ok(info) => match dispatch::encode_model(dialect, &info) {
            Ok(body) => json(body),
            Err(e) => respond(dialect, &e, &Served::default()),
        },
        Err(e) => respond(dialect, &e, &Served::default()),
    }
}

async fn get_inner(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    id: &str,
) -> Result<ModelInfo, KppError> {
    let key = auth::authenticate(state, headers, client).await?;
    let models = project_models(state, key.project_id).await?;
    let not_found = || {
        error(
            ErrorKind::NotFound,
            404,
            format!("The model `{id}` does not exist or this key may not use it."),
        )
    };
    let (entry, _) = models.catalogue.resolve(id).map_err(|_| not_found())?;
    if !models
        .access(Some(key.id), key.user_id, key.service_account_id)
        .allows(entry.id)
    {
        return Err(not_found());
    }
    let eff = crate::services::catalogue::effective(state, entry).ok_or_else(not_found)?;
    if entry.is_unavailable() {
        return Err(super::pipeline::unserved(&eff));
    }
    Ok(info(state, id, entry, &eff))
}
