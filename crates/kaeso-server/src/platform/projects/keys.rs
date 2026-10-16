use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{ApiKeyId, OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::keys::KeyKind;
use kaeso_core::rights::ProjectRight;
use kaeso_store::pg::keys::{self, KeyFilter, NewKey};
use kaeso_store::pg::service_accounts;
use secrecy::ExposeSecret;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{
    CreatedKey, ExpiryRequest, KeyAccess, KeyView, creator_of, new_secret, project_max,
    validate_name,
};
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new key.")]
pub struct KeyCreate {
    #[doc = "A name to recognise it by."]
    #[schema(example = "support-bot")]
    pub name: String,
    #[doc = "What it may call; default `inference`."]
    #[serde(default)]
    pub access: KeyAccess,
    #[doc = "When it expires."]
    #[serde(flatten)]
    pub expiry: ExpiryRequest,
    #[doc = "The catalogue models it may call, within its member's; every model its\n\
        member may use when absent."]
    pub models: Option<Vec<kaeso_core::id::CatalogueModelId>>,
    #[doc = "Limits on the key, only tighter than its member's and the project's."]
    pub limits: Option<Vec<super::limits::KeyLimit>>,
}

fn writable(ctx: &Ctx) -> ApiResult<()> {
    if ctx.project()?.project.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The project is archived; its keys cannot change.".into(),
        ));
    }
    Ok(())
}

async fn create_key(
    state: &AppState,
    ctx: &Ctx,
    req: KeyCreate,
    user: Option<UserId>,
    service_account: Option<uuid::Uuid>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    writable(ctx)?;
    let name = validate_name(&req.name)?;
    let (inference, platform) = req.access.flags();
    if platform {
        if !ctx
            .project()?
            .rights
            .contains(ProjectRight::KeysPlatformAccessGrant)
        {
            return Err(ApiError::PermissionDenied(
                "Giving a key platform access needs the right `keys.platform_access.grant` in this project.".into(),
            ));
        }
        ctx.require_recent_sign_in()?;
    }
    let expires_at = req.expiry.resolve(ctx.now, project_max(ctx)?)?;
    let models: Option<std::collections::BTreeSet<uuid::Uuid>> = req
        .models
        .as_ref()
        .map(|m| m.iter().map(|id| id.uuid()).collect());
    if let Some(list) = &models {
        let allowed = super::access::member_allowed_for(
            state,
            ctx.project()?.project.id,
            user.map(|u| u.uuid()),
            service_account,
        )
        .await?;
        super::access::check_key_list(list, &allowed)?;
    }
    let secret = new_secret(KeyKind::Project);
    let id = ApiKeyId::new();
    let member = user
        .map(|u| kaeso_core::limits::Subject::User(u.uuid()))
        .or(service_account.map(kaeso_core::limits::Subject::ServiceAccount));
    let key_limits = match &req.limits {
        Some(specs) => {
            super::limits::key_limits(state, ctx.project()?.project.id, id.uuid(), member, specs)
                .await?
        }
        None => Vec::new(),
    };
    let project = ctx.project()?;
    let (created_kind, created_id) = creator_of(ctx);
    let mut tx = state.pg.begin().await?;
    keys::insert(
        &mut *tx,
        &NewKey {
            id: id.uuid(),
            org_id: project.project.org_id,
            project_id: project.project.id,
            user_id: user.map(|u| u.uuid()),
            service_account_id: service_account,
            name: &name,
            secret_hash: &secret.hash,
            secret_prefix: &secret.prefix,
            secret_last4: &secret.last4,
            inference,
            platform,
            expires_at,
            created_by_kind: created_kind,
            created_by_id: created_id,
        },
    )
    .await?;
    if let Some(list) = &models {
        kaeso_store::pg::catalogue::set_access(
            &mut tx,
            project.project.id,
            kaeso_store::pg::catalogue::Subject::Key(id.uuid()),
            Some(list),
        )
        .await?;
    }
    for l in &key_limits {
        kaeso_store::pg::limits::insert(&mut *tx, project.project.org_id, project.project.id, l)
            .await?;
    }
    let row = keys::get(&mut *tx, project.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new key vanished"))?;
    let view = KeyView::from_row(&row, ctx.now);
    ctx.audit(Some(ctx.org()?.id()), "key.create")
        .target("key", id)
        .after(serde_json::to_value(&view).unwrap_or_default())
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    if models.is_some() {
        crate::services::catalogue::forget(state, project.project.id).await;
    }
    if !key_limits.is_empty() {
        crate::services::limits::forget(state, project.project.id).await;
    }
    Ok((
        StatusCode::CREATED,
        Json(CreatedKey {
            key: view,
            secret: secret.secret.expose_secret().to_owned(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/keys",
    summary = "Creates a key for the caller (a user key). Its rights are the user's in\n\
    this project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = KeyCreate,
    responses((status = 201, body = CreatedKey<KeyView>))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<KeyCreate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    let user = ctx.acting_user().ok_or_else(|| {
        ApiError::PermissionDenied(
            "Service accounts get keys through service-accounts/{account}/keys.".into(),
        )
    })?;
    create_key(&state, &ctx, req, Some(user), None).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}/keys",
    summary = "Creates a key for a service account. It survives its creator.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    request_body = KeyCreate,
    responses((status = 201, body = CreatedKey<KeyView>))
)]
pub async fn create_for_service_account(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, account)): Path<(String, String, String)>,
    Body(req): Body<KeyCreate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    let account: ServiceAccountId = account
        .parse()
        .map_err(|_| ApiError::not_found("service account"))?;
    let svc = service_accounts::get(&state.pg, ctx.project()?.project.id, account.uuid())
        .await?
        .filter(|s| s.archived_at.is_none())
        .ok_or_else(|| ApiError::not_found("active service account"))?;
    create_key(&state, &ctx, req, None, Some(svc.id)).await
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct KeyFilterParams {
    #[doc = "Only this user's keys."]
    pub user: Option<UserId>,
    #[doc = "Only this service account's keys."]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "Include archived keys (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/keys",
    summary = "The project's keys.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams, KeyFilterParams),
    responses((status = 200, body = List<KeyView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
    Query(f): Query<KeyFilterParams>,
) -> ApiResult<Json<List<KeyView>>> {
    let page = page.page()?;
    let filter = KeyFilter {
        user_id: f.user.map(|u| u.uuid()),
        service_account_id: f.service_account.map(|s| s.uuid()),
        include_archived: f.include_archived.unwrap_or(false),
    };
    let rows = keys::list(
        &state.pg,
        ctx.project()?.project.id,
        filter,
        page.after,
        page.fetch(),
    )
    .await?;
    let now = ctx.now;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |k| k.id,
        |k| KeyView::from_row(&k, now),
    )))
}

async fn key(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<keys::KeyRow> {
    let id: ApiKeyId = id.parse().map_err(|_| ApiError::not_found("key"))?;
    keys::get(&state.pg, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("key"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/keys/{key}",
    summary = "One key.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<KeyView>> {
    Ok(Json(KeyView::from_row(
        &key(&state, &ctx, &id).await?,
        ctx.now,
    )))
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Change {
    Disable,
    Enable,
    Archive,
}

pub(crate) async fn change(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    row: &keys::KeyRow,
    change: Change,
) -> ApiResult<KeyView> {
    if row.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The key is archived and cannot change.".into(),
        ));
    }
    let mut tx = state.pg.begin().await?;
    let (action, done) = match change {
        Change::Disable => (
            "key.disable",
            keys::set_disabled(&mut *tx, row.id, Some(ctx.now)).await?,
        ),
        Change::Enable => (
            "key.enable",
            keys::set_disabled(&mut *tx, row.id, None).await?,
        ),
        Change::Archive => (
            "key.archive",
            keys::archive(&mut *tx, row.id, ctx.now).await?,
        ),
    };
    if !done {
        return Err(ApiError::Conflict(
            "The key changed meanwhile; reload and try again.".into(),
        ));
    }
    ctx.audit(Some(org), action)
        .target("key", ApiKeyId::from_uuid(row.id))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(state, &[row.id]).await;
    let after = keys::get(&state.pg, row.project_id, row.id)
        .await?
        .ok_or_else(|| ApiError::not_found("key"))?;
    Ok(KeyView::from_row(&after, ctx.now))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/keys/{key}/disable",
    summary = "Disables a key; it stops working on every instance within seconds and\n\
    can be enabled again.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn disable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<KeyView>> {
    let row = key(&state, &ctx, &id).await?;
    Ok(Json(
        change(&state, &ctx, ctx.org()?.id(), &row, Change::Disable).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/keys/{key}/enable",
    summary = "Enables a disabled key again.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn enable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<KeyView>> {
    let row = key(&state, &ctx, &id).await?;
    Ok(Json(
        change(&state, &ctx, ctx.org()?.id(), &row, Change::Enable).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/keys/{key}/archive",
    summary = "Archives a key. Permanent; its history is kept.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<KeyView>> {
    let row = key(&state, &ctx, &id).await?;
    Ok(Json(
        change(&state, &ctx, ctx.org()?.id(), &row, Change::Archive).await?,
    ))
}
