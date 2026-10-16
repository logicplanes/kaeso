use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use kaeso_codecs::dispatch;
use kaeso_core::catalogue::capability::model_info;
use kaeso_core::catalogue::resolve::resolve;
use kaeso_kpp::{DialectId, ErrorKind, KppError, ModelInfo, ModelList};
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
    let mut effective = Vec::new();
    for entry in &models.entries {
        if !access.allows(entry.id) {
            continue;
        }
        if let Some(eff) = crate::services::catalogue::effective(state, entry) {
            out.push(model_info(&entry.model_id, &eff));
            effective.push((entry.id, eff));
        }
    }
    for alias in &models.aliases {
        if let Some((_, eff)) = effective.iter().find(|(id, _)| *id == alias.target) {
            out.push(model_info(&alias.name, eff));
        }
    }
    Ok(out)
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
    let (entry, _) = resolve(id, &models.entries, &models.aliases).map_err(|_| not_found())?;
    if !models
        .access(Some(key.id), key.user_id, key.service_account_id)
        .allows(entry.id)
    {
        return Err(not_found());
    }
    let eff = crate::services::catalogue::effective(state, entry).ok_or_else(not_found)?;
    Ok(model_info(id, &eff))
}
