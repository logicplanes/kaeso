use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use kaeso_core::catalogue::resolve::check_subset;
use kaeso_core::id::{ApiKeyId, CatalogueModelId, OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_store::pg::catalogue::{self, Subject};
use kaeso_store::pg::{keys, members, service_accounts};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::services::catalogue as service;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A model list.")]
pub struct ModelAccess {
    #[doc = "Every model the level above allows."]
    pub all: bool,
    #[doc = "The chosen catalogue models (empty with `all`)."]
    pub models: Vec<CatalogueModelId>,
    #[doc = "The catalogue models usable after every level above is applied."]
    pub effective: Vec<CatalogueModelId>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new model list: either `all: true`, or `models`.")]
pub struct ModelAccessUpdate {
    #[doc = "Every model the level above allows."]
    pub all: Option<bool>,
    #[doc = "The chosen catalogue models."]
    pub models: Option<Vec<CatalogueModelId>>,
}

impl ModelAccessUpdate {
    pub fn list(&self) -> ApiResult<Option<BTreeSet<Uuid>>> {
        match (self.all, &self.models) {
            (Some(true), None) => Ok(None),
            (None | Some(false), Some(m)) => Ok(Some(m.iter().map(|id| id.uuid()).collect())),
            _ => Err(ApiError::bad_request(
                "Send either `all: true` or a `models` list.",
            )),
        }
    }
}

fn ids(set: &BTreeSet<Uuid>) -> Vec<CatalogueModelId> {
    set.iter()
        .copied()
        .map(CatalogueModelId::from_uuid)
        .collect()
}

async fn catalogue_ids(state: &AppState, project: Uuid) -> ApiResult<BTreeSet<Uuid>> {
    Ok(catalogue::list(&state.pg, project)
        .await?
        .into_iter()
        .map(|e| e.id)
        .collect())
}

fn view(list: Option<&BTreeSet<Uuid>>, above: &BTreeSet<Uuid>) -> ModelAccess {
    let effective: BTreeSet<Uuid> = match list {
        None => above.clone(),
        Some(l) => l.intersection(above).copied().collect(),
    };
    ModelAccess {
        all: list.is_none(),
        models: list.map(ids).unwrap_or_default(),
        effective: ids(&effective),
    }
}

fn not_in(id: Uuid, level: &str) -> ApiError {
    ApiError::invalid(
        "models",
        format!(
            "`{}` is not in the {level}.",
            CatalogueModelId::from_uuid(id)
        ),
    )
}

async fn member_of(
    state: &AppState,
    project: Uuid,
    user: Option<Uuid>,
    account: Option<Uuid>,
) -> ApiResult<(Option<Subject>, Option<BTreeSet<Uuid>>)> {
    let subject = match (user, account) {
        (Some(u), _) => members::get_project(&state.pg, project, u)
            .await?
            .map(|m| Subject::Membership(m.id)),
        (None, Some(s)) => Some(Subject::ServiceAccount(s)),
        (None, None) => None,
    };
    let list = match subject {
        Some(s) => catalogue::access(&mut *state.pg.acquire().await?, project, s).await?,
        None => None,
    };
    Ok((subject, list))
}

async fn write(
    state: &AppState,
    ctx: &Ctx,
    project: Uuid,
    subject: Subject,
    list: Option<&BTreeSet<Uuid>>,
    target: (&'static str, String),
    prune: Option<(Option<Uuid>, Option<Uuid>)>,
) -> ApiResult<()> {
    let mut tx = state.pg.begin().await?;
    let before = catalogue::access(&mut tx, project, subject).await?;
    catalogue::set_access(&mut tx, project, subject, list).await?;
    if let (Some((user, account)), Some(allowed)) = (prune, list) {
        catalogue::prune_key_lists(&mut *tx, project, user, account, allowed).await?;
    }
    let as_json = |l: Option<&BTreeSet<Uuid>>| match l {
        None => serde_json::json!({ "all": true }),
        Some(l) => serde_json::json!({ "models": ids(l) }),
    };
    ctx.audit(Some(ctx.org()?.id()), "model_access.update")
        .target(target.0, target.1)
        .before(as_json(before.as_ref()))
        .after(as_json(list))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(state, project).await;
    Ok(())
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/members/{user}/models",
    summary = "A project member's models.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    responses((status = 200, body = ModelAccess))
)]
pub async fn member_get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelAccess>> {
    let project = ctx.project()?.project.id;
    let user: UserId = user
        .parse()
        .map_err(|_| ApiError::not_found("project member"))?;
    let (subject, list) = member_of(&state, project, Some(user.uuid()), None).await?;
    subject.ok_or_else(|| ApiError::not_found("project member"))?;
    Ok(Json(view(
        list.as_ref(),
        &catalogue_ids(&state, project).await?,
    )))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/members/{user}/models",
    summary = "Sets a project member's models. Their keys' lists are narrowed to match.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    request_body = ModelAccessUpdate,
    responses((status = 200, body = ModelAccess))
)]
pub async fn member_set(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
    Body(req): Body<ModelAccessUpdate>,
) -> ApiResult<Json<ModelAccess>> {
    super::models::writable(&ctx)?;
    let project = ctx.project()?.project.id;
    let user: UserId = user
        .parse()
        .map_err(|_| ApiError::not_found("project member"))?;
    let (subject, _) = member_of(&state, project, Some(user.uuid()), None).await?;
    let subject = subject.ok_or_else(|| ApiError::not_found("project member"))?;
    let list = req.list()?;
    let catalogue = catalogue_ids(&state, project).await?;
    if let Some(l) = &list {
        check_subset(l, Some(&catalogue)).map_err(|id| not_in(id, "project's catalogue"))?;
    }
    write(
        &state,
        &ctx,
        project,
        subject,
        list.as_ref(),
        ("user", user.to_string()),
        Some((Some(user.uuid()), None)),
    )
    .await?;
    Ok(Json(view(list.as_ref(), &catalogue)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}/models",
    summary = "A service account's models.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    responses((status = 200, body = ModelAccess))
)]
pub async fn account_get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, account)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelAccess>> {
    let project = ctx.project()?.project.id;
    let account = account_id(&state, project, &account).await?;
    let (_, list) = member_of(&state, project, None, Some(account)).await?;
    Ok(Json(view(
        list.as_ref(),
        &catalogue_ids(&state, project).await?,
    )))
}

async fn account_id(state: &AppState, project: Uuid, raw: &str) -> ApiResult<Uuid> {
    let id: ServiceAccountId = raw
        .parse()
        .map_err(|_| ApiError::not_found("service account"))?;
    service_accounts::get(&state.pg, project, id.uuid())
        .await?
        .map(|s| s.id)
        .ok_or_else(|| ApiError::not_found("service account"))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}/models",
    summary = "Sets a service account's models. Its keys' lists are narrowed to match.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    request_body = ModelAccessUpdate,
    responses((status = 200, body = ModelAccess))
)]
pub async fn account_set(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, account)): Path<(String, String, String)>,
    Body(req): Body<ModelAccessUpdate>,
) -> ApiResult<Json<ModelAccess>> {
    super::models::writable(&ctx)?;
    let project = ctx.project()?.project.id;
    let account = account_id(&state, project, &account).await?;
    let list = req.list()?;
    let catalogue = catalogue_ids(&state, project).await?;
    if let Some(l) = &list {
        check_subset(l, Some(&catalogue)).map_err(|id| not_in(id, "project's catalogue"))?;
    }
    write(
        &state,
        &ctx,
        project,
        Subject::ServiceAccount(account),
        list.as_ref(),
        (
            "service_account",
            ServiceAccountId::from_uuid(account).to_string(),
        ),
        Some((None, Some(account))),
    )
    .await?;
    Ok(Json(view(list.as_ref(), &catalogue)))
}

pub(crate) async fn member_allowed(
    state: &AppState,
    project: Uuid,
    key: &keys::KeyRow,
) -> ApiResult<BTreeSet<Uuid>> {
    member_allowed_for(state, project, key.user_id, key.service_account_id).await
}

pub(crate) async fn member_allowed_for(
    state: &AppState,
    project: Uuid,
    user: Option<Uuid>,
    account: Option<Uuid>,
) -> ApiResult<BTreeSet<Uuid>> {
    let catalogue = catalogue_ids(state, project).await?;
    let (_, list) = member_of(state, project, user, account).await?;
    Ok(match list {
        None => catalogue,
        Some(l) => l.intersection(&catalogue).copied().collect(),
    })
}

pub(crate) fn check_key_list(list: &BTreeSet<Uuid>, allowed: &BTreeSet<Uuid>) -> ApiResult<()> {
    check_subset(list, Some(allowed)).map_err(|id| not_in(id, "models the key's member may use"))
}

async fn key_row(state: &AppState, project: Uuid, raw: &str) -> ApiResult<keys::KeyRow> {
    let id: ApiKeyId = raw.parse().map_err(|_| ApiError::not_found("key"))?;
    keys::get(&state.pg, project, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("key"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/keys/{key}/models",
    summary = "A key's models.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    responses((status = 200, body = ModelAccess))
)]
pub async fn key_get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, key)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelAccess>> {
    let project = ctx.project()?.project.id;
    let key = key_row(&state, project, &key).await?;
    let list = catalogue::access(
        &mut *state.pg.acquire().await?,
        project,
        Subject::Key(key.id),
    )
    .await?;
    Ok(Json(view(
        list.as_ref(),
        &member_allowed(&state, project, &key).await?,
    )))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/keys/{key}/models",
    summary = "Sets a key's models, within its member's.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    request_body = ModelAccessUpdate,
    responses((status = 200, body = ModelAccess))
)]
pub async fn key_set(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, key)): Path<(String, String, String)>,
    Body(req): Body<ModelAccessUpdate>,
) -> ApiResult<Json<ModelAccess>> {
    super::models::writable(&ctx)?;
    let project = ctx.project()?.project.id;
    let key = key_row(&state, project, &key).await?;
    if key.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The key is archived and cannot change.".into(),
        ));
    }
    let list = req.list()?;
    let allowed = member_allowed(&state, project, &key).await?;
    if let Some(l) = &list {
        check_key_list(l, &allowed)?;
    }
    write(
        &state,
        &ctx,
        project,
        Subject::Key(key.id),
        list.as_ref(),
        ("key", ApiKeyId::from_uuid(key.id).to_string()),
        None,
    )
    .await?;
    Ok(Json(view(list.as_ref(), &allowed)))
}
