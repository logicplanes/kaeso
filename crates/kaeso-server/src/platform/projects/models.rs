use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::catalogue::{self as core, Availability, Fields};
use kaeso_core::decimal_input::FieldsInput;
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_core::provider::ProviderId;
use kaeso_core::providers::{OrgProviders, ProviderRef};
use kaeso_kpp::ErrorKind;
use kaeso_store::pg::catalogue::{self, EntryRow, NewEntry};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use super::{MODELS_ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{Party, creator, creator_of};
use crate::platform::pagination::List;
use crate::platform::params::{Path, Query};
use crate::platform::precondition::{IfMatch, etag};
use crate::services::availability::{self, Unlisted};
use crate::services::catalogue as service;
use crate::services::credentials::ResolveError;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A catalogue entry.")]
pub struct CatalogueModelView {
    #[doc = "Id."]
    pub id: CatalogueModelId,
    #[doc = "The provider."]
    #[schema(example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's model id; apps call it by this name, `provider/model`\n\
        or an alias."]
    #[schema(example = "claude-sonnet-4-5")]
    pub model_id: String,
    #[doc = "Not in models.dev: every required field is an override."]
    pub custom: bool,
    #[doc = "Whether models.dev lists it. An unlisted entry keeps its last known values."]
    pub listed: bool,
    #[doc = "When models.dev stopped listing it."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub delisted_at: Option<OffsetDateTime>,
    #[doc = "Whether models.dev marks it deprecated."]
    pub deprecated: bool,
    #[doc = "Whether the provider serves it to the project's provider key, as the\n\
        last check or request found."]
    pub availability: AvailabilityState,
    #[doc = "When Kaeso last found out, by a check or a request; `null` while\n\
        `unchecked`."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub availability_checked_at: Option<OffsetDateTime>,
    #[doc = "The overridden fields."]
    pub overrides: Fields,
    #[doc = "The values in use: override, else models.dev, else Kaeso's supplement."]
    pub effective: Fields,
    #[doc = "The dialect Kaeso sends to the provider."]
    #[schema(example = "anthropic_messages")]
    pub egress_dialect: String,
    #[doc = "Aliases pointing at it."]
    pub aliases: Vec<String>,
    #[doc = "Who added it."]
    pub created_by: Party,
    #[doc = "Added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Goes up with every change, also when models.dev stops or starts listing\n\
        it. Send it as `If-Match: \"<revision>\"` to change or remove the entry only\n\
        if nothing changed it since."]
    #[schema(example = 1)]
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Whether the provider serves a model to the project's provider key.")]
pub enum AvailabilityState {
    #[doc = "The provider serves it."]
    Available,
    #[doc = "The provider answered that it does not serve it to this key: the model is\n\
        retired, or the key has no access. Model listings leave it out; requests\n\
        still go to the provider."]
    Unavailable,
    #[doc = "Not known yet, or a provider key of the org changed since."]
    Unchecked,
}

impl From<Option<Availability>> for AvailabilityState {
    fn from(a: Option<Availability>) -> AvailabilityState {
        match a {
            Some(Availability::Available) => AvailabilityState::Available,
            Some(Availability::Unavailable) => AvailabilityState::Unavailable,
            None => AvailabilityState::Unchecked,
        }
    }
}

type Tagged = [(axum::http::HeaderName, axum::http::HeaderValue); 1];

pub(super) fn view(
    state: &AppState,
    providers: &OrgProviders,
    row: &EntryRow,
    aliases: &[catalogue::AliasRow],
) -> ApiResult<CatalogueModelView> {
    let names = aliases
        .iter()
        .filter(|a| a.catalogue_model_id == row.id)
        .map(|a| a.name.clone())
        .collect();
    named_view(state, providers, row, names)
}

fn named_view(
    state: &AppState,
    providers: &OrgProviders,
    row: &EntryRow,
    aliases: Vec<String>,
) -> ApiResult<CatalogueModelView> {
    let entry = row.entry()?;
    let (effective, egress_dialect) = match service::effective(state, &entry, providers) {
        Some(eff) => (eff.fields, eff.dialect.as_str()),
        None if row.custom => (
            entry.overrides.clone(),
            entry.overrides.egress_dialect.map_or("", |d| d.as_str()),
        ),
        None => {
            return Err(ApiError::internal(format!(
                "unknown provider `{}`",
                row.provider
            )));
        }
    };
    Ok(CatalogueModelView {
        id: CatalogueModelId::from_uuid(row.id),
        provider: row.provider.clone(),
        model_id: row.model_id.clone(),
        custom: row.custom,
        listed: row.delisted_at.is_none() && !row.custom,
        delisted_at: row.delisted_at,
        deprecated: entry.is_deprecated(&state.models_dev.current().snapshot),
        availability: entry.availability.into(),
        availability_checked_at: row.availability_checked_at,
        overrides: entry.overrides,
        effective,
        egress_dialect: egress_dialect.to_owned(),
        aliases,
        created_by: creator(&row.created_by_kind, row.created_by_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    })
}

pub(super) async fn org_providers(state: &AppState, ctx: &Ctx) -> ApiResult<OrgProviders> {
    let org = ctx.project()?.project.org_id;
    Ok(crate::services::custom_providers::of_org(state, &state.pg, org).await?)
}

fn provider_of(providers: &OrgProviders, id: &str) -> ApiResult<ProviderRef> {
    providers.get(id).ok_or_else(|| {
        let mut ids: Vec<&str> = kaeso_core::provider::PROVIDERS
            .iter()
            .map(|p| p.id)
            .collect();
        ids.extend(providers.custom().iter().map(|c| c.id().as_str()));
        ApiError::invalid(
            "provider",
            format!("The provider must be one of {}.", ids.join(", ")),
        )
    })
}

fn models_dev_lists(state: &AppState, provider: &ProviderRef, model: &str) -> bool {
    provider.custom().is_none()
        && state
            .models_dev
            .current()
            .snapshot
            .model(provider.id(), model)
            .is_some()
}

fn validate_model_id(id: &str) -> ApiResult<()> {
    if id.is_empty()
        || id.chars().count() > 200
        || id.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(ApiError::invalid(
            "model_id",
            "A model id has 1 to 200 characters and no spaces.",
        ));
    }
    if id.split('/').any(|part| matches!(part, "." | "..")) {
        return Err(ApiError::invalid(
            "model_id",
            "`model_id` can't be `.` or `..`, or have either between slashes.",
        ));
    }
    Ok(())
}

fn overrides_from(
    map: Option<&serde_json::Map<String, serde_json::Value>>,
    base: Fields,
) -> ApiResult<Fields> {
    let mut fields = base;
    if let Some(map) = map {
        fields
            .patch(map)
            .map_err(|e| ApiError::invalid(&e.field, e.message))?;
    }
    Ok(fields)
}

fn check_fields(fields: &Fields, provider: &ProviderRef, custom: bool) -> ApiResult<()> {
    fields
        .validate(provider)
        .map_err(|e| ApiError::invalid(&e.field, e.message))?;
    if custom {
        fields
            .validate_custom()
            .map_err(|e| ApiError::invalid(&e.field, e.message))?;
    }
    Ok(())
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchParams {
    #[doc = "Text in the model id or name (case-insensitive); all models when absent."]
    pub q: Option<String>,
    #[doc = "Only this provider."]
    #[param(value_type = Option<ProviderId>)]
    pub provider: Option<String>,
    #[doc = "Include models models.dev marks deprecated (default false)."]
    pub include_deprecated: Option<bool>,
    #[doc = "How many results, 1 to 200 (default 50)."]
    pub limit: Option<u32>,
    #[doc = "Only current models, the ones `all_current` adds: not deprecated,\n\
        producing text or embeddings."]
    pub current: Option<bool>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A models.dev model.")]
pub struct SearchHit {
    #[doc = "The provider."]
    pub provider: String,
    #[doc = "The model id."]
    pub model_id: String,
    #[doc = "Its name."]
    pub name: Option<String>,
    #[doc = "Its family."]
    pub family: Option<String>,
    #[doc = "Release date."]
    pub release_date: Option<String>,
    #[doc = "Whether models.dev marks it deprecated."]
    pub deprecated: bool,
    #[doc = "The catalogue entry, if the project has it."]
    pub catalogue_model_id: Option<CatalogueModelId>,
    #[doc = "USD per million input tokens, as an entry would start with it."]
    #[schema(value_type = Option<String>, example = "3.00")]
    pub input_price: Option<rust_decimal::Decimal>,
    #[doc = "USD per million output tokens, as an entry would start with it."]
    #[schema(value_type = Option<String>, example = "15.00")]
    pub output_price: Option<rust_decimal::Decimal>,
    #[doc = "The context window in tokens."]
    pub context_limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The models.dev models matching a search, newest first per provider.")]
pub struct ModelSearch {
    #[doc = "The first `limit` matches."]
    pub data: Vec<SearchHit>,
    #[doc = "Whether more models match than `data` holds."]
    pub has_more: bool,
    #[doc = "How many models match."]
    pub total: u32,
    #[doc = "How many of the matches the catalogue doesn't have yet."]
    pub not_in_catalogue: u32,
}

fn is_current(
    state: &AppState,
    provider: &str,
    id: &str,
    m: &kaeso_core::models_dev::Model,
) -> bool {
    if m.is_deprecated() {
        return false;
    }
    let embeddings = state
        .models_dev
        .supplement()
        .get(provider, id)
        .and_then(|f| f.egress_dialect)
        == Some(kaeso_kpp::DialectId::OpenAiEmbeddings);
    embeddings
        || m.modalities
            .as_ref()
            .is_some_and(|x| x.output.iter().any(|o| o == "text"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/models/search",
    summary = "Searches models.dev for models to add, newest first per provider.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), SearchParams),
    responses((status = 200, body = ModelSearch))
)]
pub async fn search(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(p): Query<SearchParams>,
) -> ApiResult<Json<ModelSearch>> {
    let limit = p.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(ApiError::invalid(
            "limit",
            "`limit` must be between 1 and 200.",
        ));
    }
    let providers = org_providers(&state, &ctx).await?;
    if let Some(pr) = &p.provider {
        provider_of(&providers, pr)?;
    }
    let q = p.q.as_deref().map(str::to_lowercase);
    let rows = catalogue::list(&state.pg, ctx.project()?.project.id).await?;
    let mut in_catalogue: HashMap<(&str, &str), uuid::Uuid> = HashMap::with_capacity(rows.len());
    for r in &rows {
        in_catalogue
            .entry((r.provider.as_str(), r.model_id.as_str()))
            .or_insert(r.id);
    }
    let loaded = state.models_dev.current();
    let mut hits: Vec<SearchHit> = Vec::new();
    for (pid, prov) in &loaded.snapshot.providers {
        if p.provider.as_deref().is_some_and(|x| x != pid)
            || providers.get(pid).is_some_and(|r| r.custom().is_some())
        {
            continue;
        }
        for (mid, m) in &prov.models {
            if m.is_deprecated() && !p.include_deprecated.unwrap_or(false) {
                continue;
            }
            if p.current.unwrap_or(false) && !is_current(&state, pid, mid, m) {
                continue;
            }
            if let Some(q) = &q
                && !mid.to_lowercase().contains(q.as_str())
                && !m
                    .name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(q.as_str())
            {
                continue;
            }
            hits.push(SearchHit {
                provider: pid.clone(),
                model_id: mid.clone(),
                name: m.name.clone(),
                family: m.family.clone(),
                release_date: m.release_date.clone(),
                deprecated: m.is_deprecated(),
                catalogue_model_id: in_catalogue
                    .get(&(pid.as_str(), mid.as_str()))
                    .map(|id| CatalogueModelId::from_uuid(*id)),
                input_price: None,
                output_price: None,
                context_limit: None,
            });
        }
    }
    hits.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then(b.release_date.cmp(&a.release_date))
            .then(a.model_id.cmp(&b.model_id))
    });
    let total = u32::try_from(hits.len()).unwrap_or(u32::MAX);
    let not_in_catalogue = u32::try_from(
        hits.iter()
            .filter(|h| h.catalogue_model_id.is_none())
            .count(),
    )
    .unwrap_or(u32::MAX);
    hits.truncate(limit as usize);
    for h in &mut hits {
        if let Some(f) = core::preview(
            &h.provider,
            &h.model_id,
            &loaded.snapshot,
            state.models_dev.supplement(),
        ) {
            h.input_price = f.input_price;
            h.output_price = f.output_price;
            h.context_limit = f.context_limit;
        }
    }
    Ok(Json(ModelSearch {
        has_more: hits.len() < total as usize,
        data: hits,
        total,
        not_in_catalogue,
    }))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PreviewParams {
    #[doc = "The provider."]
    #[param(value_type = ProviderId)]
    pub provider: String,
    #[doc = "The model id."]
    pub model: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The values a new entry starts with.")]
pub struct Preview {
    #[doc = "The provider."]
    pub provider: String,
    #[doc = "The model id."]
    pub model_id: String,
    #[doc = "models.dev's values, completed by Kaeso's supplement."]
    pub fields: Fields,
    #[doc = "The dialect Kaeso would send."]
    pub egress_dialect: String,
    #[doc = "The catalogue entry, if the project has it."]
    pub catalogue_model_id: Option<CatalogueModelId>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/models/preview",
    summary = "The models.dev values of a model, as a form to start an entry from.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PreviewParams),
    responses((status = 200, body = Preview))
)]
pub async fn preview(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(p): Query<PreviewParams>,
) -> ApiResult<Json<Preview>> {
    let providers = org_providers(&state, &ctx).await?;
    let prov = provider_of(&providers, &p.provider)?;
    let loaded = state.models_dev.current();
    let fields = core::preview(
        &p.provider,
        &p.model,
        &loaded.snapshot,
        state.models_dev.supplement(),
    )
    .filter(|_| prov.custom().is_none())
    .ok_or_else(|| {
        ApiError::NotFound(format!(
            "The model `{}` is not on models.dev for `{}`; add it as a custom model.",
            p.model, p.provider
        ))
    })?;
    let rows = catalogue::list(&state.pg, ctx.project()?.project.id).await?;
    Ok(Json(Preview {
        egress_dialect: fields
            .egress_dialect
            .unwrap_or(prov.generation())
            .as_str()
            .to_owned(),
        catalogue_model_id: rows
            .iter()
            .find(|r| r.provider == p.provider && r.model_id == p.model)
            .map(|r| CatalogueModelId::from_uuid(r.id)),
        provider: p.provider,
        model_id: p.model,
        fields,
    }))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OfferedParams {
    #[doc = "One of the organization's custom providers."]
    #[param(value_type = ProviderId)]
    pub provider: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A model id a custom provider lists.")]
pub struct OfferedModel {
    #[doc = "The provider's model id."]
    #[schema(example = "meta-llama/Llama-3.3-70B-Instruct")]
    pub model_id: String,
    #[doc = "The catalogue entry, if the project has it."]
    pub catalogue_model_id: Option<CatalogueModelId>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "The model ids a custom provider's model listing names; their values, such as\n\
    prices and limits, are set by hand when one is added."
)]
pub struct OfferedModels {
    #[doc = "The ids, sorted."]
    pub data: Vec<OfferedModel>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/models/offered",
    summary = "Asks a custom provider which models it lists, with the project's key for it.",
    description = "Calls the provider's model listing (`GET /v1/models`) in its generation\n\
    format, through the same address checks as requests. Nothing is recorded.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), OfferedParams),
    responses((status = 200, body = OfferedModels))
)]
pub async fn offered(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(p): Query<OfferedParams>,
) -> ApiResult<Json<OfferedModels>> {
    let providers = org_providers(&state, &ctx).await?;
    let prov = provider_of(&providers, &p.provider)?;
    let Some(custom) = prov.custom() else {
        return Err(ApiError::invalid(
            "provider",
            "Only custom providers list their models here; search models.dev for the others.",
        ));
    };
    let id = custom.id().as_str();
    let project = &ctx.project()?.project;
    let credential = crate::services::credentials::for_provider(
        &state,
        OrgId::from_uuid(project.org_id),
        ProjectId::from_uuid(project.id),
        &prov,
    )
    .await
    .map_err(|e| match e {
        ResolveError::Missing { .. } => ApiError::Conflict(format!(
            "No provider key for {} is set up for this project; an org admin can add one.",
            prov.name()
        )),
        ResolveError::Store(e) => e.into(),
        other => ApiError::internal(other),
    })?;
    let unlisted = |status: StatusCode, message: String| ApiError::Provider {
        status,
        message,
        retry_after: None,
    };
    let ids = availability::model_ids(&state, &credential, &prov)
        .await
        .map_err(|e| match e {
            Unlisted::Unreached(e) => unlisted(StatusCode::BAD_GATEWAY, e.message),
            Unlisted::Answered(e) => match e.kind {
                ErrorKind::Authentication | ErrorKind::PermissionDenied | ErrorKind::Billing => {
                    unlisted(
                        StatusCode::BAD_GATEWAY,
                        format!("The custom provider `{id}` refused this project's key; check it under Providers."),
                    )
                }
                ErrorKind::NotFound => ApiError::NotFound(format!(
                    "The custom provider `{id}` has no model listing; type the model id instead."
                )),
                _ => {
                    tracing::info!(provider = id, kind = e.kind.as_str(), error = %e.message, "a custom provider did not list its models");
                    unlisted(
                        StatusCode::BAD_GATEWAY,
                        format!("The custom provider `{id}` did not list its models; type the model id instead."),
                    )
                }
            },
        })?;
    let rows = catalogue::list(&state.pg, project.id).await?;
    Ok(Json(OfferedModels {
        data: ids
            .into_iter()
            .map(|model_id| OfferedModel {
                catalogue_model_id: rows
                    .iter()
                    .find(|r| r.provider == id && r.model_id == model_id)
                    .map(|r| CatalogueModelId::from_uuid(r.id)),
                model_id,
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new catalogue entry.")]
pub struct ModelCreate {
    #[doc = "The provider."]
    #[schema(value_type = ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's model id."]
    #[schema(example = "claude-sonnet-4-5")]
    pub model_id: String,
    #[doc = "Fields to override (see the entry's `effective` fields for names)."]
    #[schema(value_type = Option<FieldsInput>)]
    pub overrides: Option<serde_json::Map<String, serde_json::Value>>,
    #[doc = "`true` for a model models.dev does not list; its required fields\n\
        (modalities, limits, input and output prices, egress dialect) must\n\
        then be overrides."]
    #[serde(default)]
    pub custom: bool,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/models",
    summary = "Adds a model to the project's catalogue.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ModelCreate,
    responses((status = 201, body = CatalogueModelView, headers(("ETag" = String, description = "The entry's revision as an entity tag."))))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<ModelCreate>,
) -> ApiResult<(StatusCode, Tagged, Json<CatalogueModelView>)> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let providers = org_providers(&state, &ctx).await?;
    let prov = provider_of(&providers, &req.provider)?;
    validate_model_id(&req.model_id)?;
    let listed = models_dev_lists(&state, &prov, &req.model_id);
    match (listed, req.custom) {
        (false, false) if prov.custom().is_some() => {
            return Err(ApiError::invalid(
                "custom",
                format!(
                    "`{}` is a custom provider; send `custom: true` with the model's values.",
                    req.provider
                ),
            ));
        }
        (false, false) => {
            return Err(ApiError::invalid(
                "model_id",
                format!(
                    "The model `{}` is not on models.dev for `{}`; send `custom: true` to add it anyway.",
                    req.model_id, req.provider
                ),
            ));
        }
        (true, true) => {
            return Err(ApiError::invalid(
                "custom",
                format!(
                    "The model `{}` is on models.dev; add it without `custom`.",
                    req.model_id
                ),
            ));
        }
        _ => {}
    }
    let overrides = overrides_from(req.overrides.as_ref(), Fields::default())?;
    check_fields(&overrides, &prov, req.custom)?;
    let mut tx = state.pg.begin().await?;
    let row = insert_entry(&ctx, &mut tx, &prov, &req.model_id, &overrides, req.custom)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict(format!(
                "The catalogue already has `{}/{}`.",
                req.provider, req.model_id
            ))
        })?;
    tx.commit().await?;
    let project = ctx.project()?.project.id;
    service::forget(&state, project).await;
    Ok((
        StatusCode::CREATED,
        etag(row.revision),
        Json(view(&state, &providers, &row, &[])?),
    ))
}

async fn insert_entry(
    ctx: &Ctx,
    tx: &mut sqlx::PgConnection,
    prov: &ProviderRef,
    model_id: &str,
    overrides: &Fields,
    custom: bool,
) -> ApiResult<Option<EntryRow>> {
    let project = ctx.project()?;
    let id = CatalogueModelId::new();
    let (kind, by) = creator_of(ctx);
    let json = serde_json::to_value(overrides).map_err(ApiError::internal)?;
    let added = catalogue::insert(
        &mut *tx,
        &NewEntry {
            id: id.uuid(),
            org_id: project.project.org_id,
            project_id: project.project.id,
            provider: prov.id(),
            model_id,
            overrides: &json,
            custom,
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if !added {
        return Ok(None);
    }
    let row = catalogue::get(&mut *tx, project.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new catalogue model vanished"))?;
    ctx.audit(Some(ctx.org()?.id()), "catalogue_model.create")
        .target("catalogue_model", id)
        .after(serde_json::json!({ "provider": prov.id(), "model_id": model_id, "custom": custom, "overrides": json }))
        .write(tx)
        .await?;
    Ok(Some(row))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Several models of one provider at once.")]
pub struct ModelBatch {
    #[doc = "The provider."]
    #[schema(value_type = ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The model ids to add (models.dev models only)."]
    pub model_ids: Option<Vec<String>>,
    #[doc = "Add every current model of the provider instead: listed by models.dev,\n\
        not deprecated, producing text or embeddings."]
    #[serde(default)]
    pub all_current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What a batch added.")]
pub struct BatchResult {
    #[doc = "The new entries."]
    pub added: Vec<CatalogueModelView>,
    #[doc = "Model ids the catalogue already had."]
    pub skipped: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/models/batch",
    summary = "Adds several models of one provider, for example every current Anthropic\n\
    model, in one action.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ModelBatch,
    responses((status = 201, body = BatchResult))
)]
pub async fn batch(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<ModelBatch>,
) -> ApiResult<(StatusCode, Json<BatchResult>)> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let providers = org_providers(&state, &ctx).await?;
    let prov = provider_of(&providers, &req.provider)?;
    if prov.custom().is_some() {
        return Err(ApiError::invalid(
            "provider",
            "Add a custom provider's models one at a time, with their values.",
        ));
    }
    let loaded = state.models_dev.current();
    let models = loaded.snapshot.providers.get(prov.id()).map(|p| &p.models);
    let ids: Vec<String> = match (&req.model_ids, req.all_current) {
        (Some(ids), false) => {
            if ids.is_empty() || ids.len() > 200 {
                return Err(ApiError::invalid("model_ids", "List 1 to 200 model ids."));
            }
            if let Some(missing) = ids
                .iter()
                .find(|m| models.is_none_or(|ms| !ms.contains_key(*m)))
            {
                return Err(ApiError::invalid(
                    "model_ids",
                    format!(
                        "The model `{missing}` is not on models.dev for `{}`; add it alone as a custom model.",
                        prov.id()
                    ),
                ));
            }
            ids.clone()
        }
        (None, true) => models
            .into_iter()
            .flatten()
            .filter(|(id, m)| is_current(&state, prov.id(), id, m))
            .map(|(id, _)| id.clone())
            .collect(),
        _ => {
            return Err(ApiError::bad_request(
                "Send either `model_ids` or `all_current: true`.",
            ));
        }
    };
    let mut tx = state.pg.begin().await?;
    let mut added_rows = Vec::new();
    let mut skipped = Vec::new();
    for id in &ids {
        match insert_entry(&ctx, &mut tx, &prov, id, &Fields::default(), false).await? {
            Some(row) => added_rows.push(row),
            None => skipped.push(id.clone()),
        }
    }
    tx.commit().await?;
    service::forget(&state, ctx.project()?.project.id).await;
    let added = added_rows
        .iter()
        .map(|r| view(&state, &providers, r, &[]))
        .collect::<ApiResult<Vec<_>>>()?;
    Ok((StatusCode::CREATED, Json(BatchResult { added, skipped })))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/models",
    summary = "The project's catalogue.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<CatalogueModelView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<List<CatalogueModelView>>> {
    let project = ctx.project()?.project.id;
    let providers = org_providers(&state, &ctx).await?;
    let rows = catalogue::list(&state.pg, project).await?;
    let mut aliases: HashMap<uuid::Uuid, Vec<String>> = HashMap::new();
    for a in catalogue::list_aliases(&state.pg, project).await? {
        aliases
            .entry(a.catalogue_model_id)
            .or_default()
            .push(a.name);
    }
    Ok(Json(List::complete(
        rows.iter()
            .map(|r| {
                named_view(
                    &state,
                    &providers,
                    r,
                    aliases.remove(&r.id).unwrap_or_default(),
                )
            })
            .collect::<ApiResult<Vec<_>>>()?,
    )))
}

pub(crate) async fn entry_row(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<EntryRow> {
    let id: CatalogueModelId = id
        .parse()
        .map_err(|_| ApiError::not_found("catalogue model"))?;
    catalogue::get(&state.pg, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("catalogue model"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/models/{model}",
    summary = "One catalogue entry.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = CatalogueModelView, headers(("ETag" = String, description = "The entry's revision as an entity tag."))))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<(Tagged, Json<CatalogueModelView>)> {
    let row = entry_row(&state, &ctx, &id).await?;
    let providers = org_providers(&state, &ctx).await?;
    let aliases = catalogue::list_aliases(&state.pg, row.project_id).await?;
    Ok((
        etag(row.revision),
        Json(view(&state, &providers, &row, &aliases)?),
    ))
}

async fn locked_entry(
    ctx: &Ctx,
    tx: &mut sqlx::PgConnection,
    id: &str,
    if_match: &IfMatch,
) -> ApiResult<EntryRow> {
    let id: CatalogueModelId = id
        .parse()
        .map_err(|_| ApiError::not_found("catalogue model"))?;
    let row = catalogue::get_for_update(&mut *tx, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("catalogue model"))?;
    if_match.check(row.revision, "catalogue model")?;
    Ok(row)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Override changes.")]
pub struct ModelUpdate {
    #[doc = "Fields to set; a field set to `null` returns to models.dev's value."]
    #[schema(value_type = FieldsInput)]
    pub overrides: serde_json::Map<String, serde_json::Value>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/models/{model}",
    summary = "Changes an entry's overrides.",
    description = "With `If-Match`, only while the entry is still at that revision; a\n\
    newer one answers 412.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    request_body = ModelUpdate,
    responses((status = 200, body = CatalogueModelView, headers(("ETag" = String, description = "The entry's revision as an entity tag."))))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    if_match: IfMatch,
    Body(req): Body<ModelUpdate>,
) -> ApiResult<(Tagged, Json<CatalogueModelView>)> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let providers = org_providers(&state, &ctx).await?;
    let mut tx = state.pg.begin().await?;
    let row = locked_entry(&ctx, &mut tx, &id, &if_match).await?;
    let prov = provider_of(&providers, &row.provider)?;
    let before = row.entry()?.overrides;
    let after = overrides_from(Some(&req.overrides), before.clone())?;
    check_fields(&after, &prov, row.custom)?;
    let json = serde_json::to_value(&after).map_err(ApiError::internal)?;
    if !catalogue::set_overrides(&mut *tx, row.project_id, row.id, &json).await? {
        return Err(ApiError::not_found("catalogue model"));
    }
    ctx.audit(Some(ctx.org()?.id()), "catalogue_model.update")
        .target("catalogue_model", CatalogueModelId::from_uuid(row.id))
        .before(&before)
        .after(json)
        .write(&mut tx)
        .await?;
    let row = catalogue::get(&mut *tx, row.project_id, row.id)
        .await?
        .ok_or_else(|| ApiError::internal("changed catalogue model vanished"))?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    let aliases = catalogue::list_aliases(&state.pg, row.project_id).await?;
    Ok((
        etag(row.revision),
        Json(view(&state, &providers, &row, &aliases)?),
    ))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/models/{model}",
    summary = "Removes a model from the catalogue, with its aliases and from every\n\
    member and key model list. Requests for it fail from now on.",
    description = "With `If-Match`, only while the entry is still at that revision; a\n\
    newer one answers 412.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    if_match: IfMatch,
) -> ApiResult<StatusCode> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let providers = org_providers(&state, &ctx).await?;
    let mut tx = state.pg.begin().await?;
    let row = locked_entry(&ctx, &mut tx, &id, &if_match).await?;
    let aliases = catalogue::list_aliases(&mut *tx, row.project_id).await?;
    if !catalogue::delete(&mut *tx, row.project_id, row.id).await? {
        return Err(ApiError::not_found("catalogue model"));
    }
    ctx.audit(Some(ctx.org()?.id()), "catalogue_model.delete")
        .target("catalogue_model", CatalogueModelId::from_uuid(row.id))
        .before(view(&state, &providers, &row, &aliases)?)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    Ok(StatusCode::NO_CONTENT)
}
