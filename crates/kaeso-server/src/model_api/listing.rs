use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use kaeso_codecs::dispatch;
use kaeso_core::catalogue::capability::model_info;
use kaeso_core::catalogue::resolve::{Alias, Resolved};
use kaeso_core::catalogue::{Effective, Entry};
use kaeso_core::id::CatalogueModelId;
use kaeso_kpp::{DialectId, ErrorKind, KppError, ModelInfo, ModelLifecycle, ModelList};
use kaeso_store::pg::catalogue::ProjectModels;
use uuid::Uuid;

use super::auth;
use super::errors::{Served, error, respond};
use super::pipeline::header_list;
use crate::state::AppState;

// A job made with a model the asker may not use stays hidden from it; one whose model left the
// catalogue is seen only by askers without a model list.
pub(super) struct ModelAccess {
    models: Arc<ProjectModels>,
    caller: super::hooks::Caller,
}

impl ModelAccess {
    pub(super) async fn of(
        caller: &super::hooks::Caller,
        state: &AppState,
    ) -> Result<ModelAccess, KppError> {
        Ok(ModelAccess {
            models: project_models(
                state,
                caller.project_id,
                caller.key_id,
                caller.service_account_id,
            )
            .await?,
            caller: caller.clone(),
        })
    }

    pub(super) fn allows(&self, model: Option<Uuid>) -> bool {
        let c = &self.caller;
        let access = self
            .models
            .access(c.key_id, c.user_id, c.service_account_id);
        match model {
            Some(m) => access.allows(m),
            None => access.member.is_none() && access.key.is_none(),
        }
    }
}

pub(super) async fn project_models(
    state: &AppState,
    project: Uuid,
    key: Option<Uuid>,
    service_account: Option<Uuid>,
) -> Result<Arc<ProjectModels>, KppError> {
    crate::services::catalogue::project_models_for(state, project, key, service_account)
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
    let models =
        project_models(state, key.project_id, Some(key.id), key.service_account_id).await?;
    let access = models.access(Some(key.id), key.user_id, key.service_account_id);
    let mut out = Vec::new();
    let mut listed = HashMap::new();
    for entry in models.catalogue.entries() {
        if !access.allows(entry.id) || entry.is_unavailable() {
            continue;
        }
        if let Some(eff) = crate::services::catalogue::effective(state, entry, &models.providers) {
            let name = crate::services::catalogue::request_name(entry, &models.catalogue);
            out.push(info(state, &name, entry, &eff));
            listed.entry(entry.id).or_insert((entry, eff));
        }
    }
    for alias in models.catalogue.aliases() {
        if let Some((entry, eff)) = alias_models(alias).find_map(|m| listed.get(&m.uuid())) {
            out.push(info(state, &alias.name, entry, eff));
        }
    }
    Ok(out)
}

// An alias reads as its main model, or else the first of its others the key may use.
fn alias_models(alias: &Alias) -> impl Iterator<Item = CatalogueModelId> + '_ {
    let main = alias.route.primary();
    main.into_iter().chain(
        alias
            .route
            .models_in_order()
            .into_iter()
            .filter(move |m| Some(*m) != main),
    )
}

fn info(state: &AppState, name: &str, entry: &Entry, eff: &Effective) -> ModelInfo {
    let mut info = model_info(name, eff);
    if entry.is_deprecated(&state.kpmr.current().snapshot) {
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
    let default_limit = (dialect == DialectId::AnthropicMessages).then_some(20);
    list_as(state, headers, client, query, dialect, default_limit).await
}

pub async fn list_as(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    query: &str,
    dialect: DialectId,
    default_limit: Option<u32>,
) -> Response {
    match list_inner(state, headers, client, query, dialect, default_limit).await {
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
    default_limit: Option<u32>,
) -> Result<Vec<u8>, KppError> {
    let all = visible(state, headers, client).await?;
    let query = dispatch::decode_models_query(dialect, query)?;
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
    get_as(state, headers, client, id, dialect).await
}

pub async fn get_as(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
    id: &str,
    dialect: DialectId,
) -> Response {
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
    let models =
        project_models(state, key.project_id, Some(key.id), key.service_account_id).await?;
    let not_found = || {
        error(
            ErrorKind::NotFound,
            404,
            format!("The model `{id}` does not exist or this key may not use it."),
        )
    };
    let access = models.access(Some(key.id), key.user_id, key.service_account_id);
    let entry = match models.catalogue.resolve(id).map_err(|_| not_found())? {
        Resolved::Model(entry, _) => entry,
        Resolved::Alias(alias) => alias_models(alias)
            .filter_map(|m| models.catalogue.model(m))
            .find(|e| access.allows(e.id) && !e.is_unavailable())
            .ok_or_else(not_found)?,
    };
    if !access.allows(entry.id) {
        return Err(not_found());
    }
    let eff = crate::services::catalogue::effective(state, entry, &models.providers)
        .ok_or_else(not_found)?;
    if entry.is_unavailable() {
        return Err(super::pipeline::unserved(&eff));
    }
    Ok(info(state, id, entry, &eff))
}
