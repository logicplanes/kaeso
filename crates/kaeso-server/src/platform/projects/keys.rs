use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{ApiKeyId, CatalogueModelId, OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::keys::KeyKind;
use kaeso_core::rights::{ProjectRight, ProjectRights, Right};
use kaeso_store::pg::keys::{self, KeyFilter, NewKey};
use kaeso_store::pg::service_accounts;
use secrecy::ExposeSecret;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use super::{KEYS_ARCHIVED, ensure_active};
use crate::platform::access::{Caller, Ctx, denied};
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{
    CreatedKey, ExpiryRequest, KeyAccess, KeyRotate, KeyView, creator_of, new_secret, project_max,
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

struct Rotation<'a> {
    old: &'a keys::KeyRow,
    stops_at: time::OffsetDateTime,
}

async fn create_key(
    state: &AppState,
    ctx: &Ctx,
    req: KeyCreate,
    user: Option<UserId>,
    service_account: Option<uuid::Uuid>,
    rotation: Option<Rotation<'_>>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    ensure_active(ctx, KEYS_ARCHIVED)?;
    let name = validate_name(&req.name)?;
    let (inference, platform) = req.access.flags();
    if inference
        && ctx.agent().is_some()
        && !ctx.project()?.rights.contains(ProjectRight::ApiRequest)
    {
        return Err(ApiError::PermissionDenied(AGENT_CALLS_MODELS.into()));
    }
    if platform {
        if !matches!(ctx.caller, Caller::Session(_)) {
            return Err(ApiError::PermissionDenied(
                "Keys can't create keys with platform access; use a console session.".into(),
            ));
        }
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
    let project = ctx.project()?;
    let (created_kind, created_id) = creator_of(ctx);
    let mut tx = state.pg.begin().await?;
    let key_limits = match &req.limits {
        Some(specs) => {
            super::limits::key_limits(state, &mut tx, project.project.id, id.uuid(), member, specs)
                .await?
        }
        None => Vec::new(),
    };
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
            rotated_from: rotation.as_ref().map(|r| r.old.id),
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
    match &rotation {
        None => {
            ctx.audit(Some(ctx.org()?.id()), Action::KeyCreate)
                .target("key", id)
                .after(&view)
                .write(&mut tx)
                .await?;
        }
        Some(r) => {
            if !keys::retire(&mut *tx, r.old.project_id, r.old.id, id.uuid(), r.stops_at).await? {
                return Err(ApiError::Conflict(
                    "The key changed meanwhile; reload and try again.".into(),
                ));
            }
            let retired = keys::get(&mut *tx, r.old.project_id, r.old.id)
                .await?
                .ok_or_else(|| ApiError::not_found("key"))?;
            ctx.audit(Some(ctx.org()?.id()), Action::KeyRotate)
                .target("key", ApiKeyId::from_uuid(r.old.id))
                .before(KeyView::from_row(r.old, ctx.now))
                .after(KeyView::from_row(&retired, ctx.now))
                .write(&mut tx)
                .await?;
        }
    }
    tx.commit().await?;
    if let Some(r) = &rotation {
        crate::services::keys::forget(state, &[r.old.id]).await;
    }
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
    create_key(&state, &ctx, req, Some(user), None, None).await
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
    let svc = keyed_service_account(&state, &ctx, &account).await?;
    create_key(&state, &ctx, req, None, Some(svc), None).await
}

async fn keyed_service_account(
    state: &AppState,
    ctx: &Ctx,
    account: &str,
) -> ApiResult<uuid::Uuid> {
    let account: ServiceAccountId = account
        .parse()
        .map_err(|_| ApiError::not_found("service account"))?;
    let svc = service_accounts::get(&state.pg, ctx.project()?.project.id, account.uuid())
        .await?
        .filter(|s| s.archived_at.is_none())
        .ok_or_else(|| ApiError::not_found("active service account"))?;
    let rights = super::service_accounts::grant_of(&svc)?.effective();
    kaeso_core::rights::check_project_grant(ctx.project()?.rights, ProjectRights::EMPTY, rights)?;
    Ok(svc.id)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holder {
    Owner,
    Manager,
}

fn may_enable(ctx: &Ctx, disabler: Option<&keys::Disabler>, holder: Holder) -> ApiResult<()> {
    let by_person = disabler.is_some_and(|d| d.kind.as_deref() == Some("user"));
    match holder {
        Holder::Owner => {
            let me = ctx.acting_user().map(|u| u.uuid());
            if by_person && disabler.and_then(|d| d.id) == me && me.is_some() {
                Ok(())
            } else {
                Err(ApiError::PermissionDenied(
                    "Someone else disabled this key; ask a holder of `keys.all.manage` to enable it."
                        .into(),
                ))
            }
        }
        Holder::Manager if by_person && !matches!(ctx.caller, Caller::Session(_)) => {
            Err(ApiError::PermissionDenied(PERSON_DISABLED.into()))
        }
        Holder::Manager => Ok(()),
    }
}

// A key calls models with its holder's rights, so an agent without `api.request` could otherwise
// mint itself access to models it may not call.
const AGENT_CALLS_MODELS: &str =
    "An agent needs `api.request` in this project to create keys that call models.";

pub(crate) const PERSON_DISABLED: &str =
    "A person disabled this key, so only a person can enable it; use a console session.";

pub(crate) async fn change(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    row: &keys::KeyRow,
    change: Change,
    holder: Holder,
) -> ApiResult<KeyView> {
    if row.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The key is archived and cannot change.".into(),
        ));
    }
    let (kind, by) = creator_of(ctx);
    let actor = keys::Actor { kind, id: by };
    let mut tx = state.pg.begin().await?;
    let (action, done) = match change {
        Change::Disable => (
            Action::KeyDisable,
            keys::set_disabled(&mut *tx, row.project_id, row.id, Some(ctx.now), actor).await?,
        ),
        Change::Enable => {
            let disabler = keys::disabled_by(&mut *tx, row.project_id, row.id).await?;
            may_enable(ctx, disabler.as_ref(), holder)?;
            (
                Action::KeyEnable,
                keys::set_disabled(&mut *tx, row.project_id, row.id, None, actor).await?,
            )
        }
        Change::Archive => (
            Action::KeyArchive,
            keys::archive(&mut *tx, row.project_id, row.id, ctx.now).await?,
        ),
    };
    if !done {
        return Err(ApiError::Conflict(
            "The key changed meanwhile; reload and try again.".into(),
        ));
    }
    ctx.audit(Some(org), action)
        .project(ProjectId::from_uuid(row.project_id))
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
        change(
            &state,
            &ctx,
            ctx.org()?.id(),
            &row,
            Change::Disable,
            Holder::Manager,
        )
        .await?,
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
        change(
            &state,
            &ctx,
            ctx.org()?.id(),
            &row,
            Change::Enable,
            Holder::Manager,
        )
        .await?,
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
        change(
            &state,
            &ctx,
            ctx.org()?.id(),
            &row,
            Change::Archive,
            Holder::Manager,
        )
        .await?,
    ))
}

async fn replacement(
    state: &AppState,
    old: &keys::KeyRow,
    req: &KeyRotate,
) -> ApiResult<KeyCreate> {
    if old.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The key is archived and cannot change.".into(),
        ));
    }
    if old.replaced_by.is_some() {
        return Err(ApiError::Conflict(
            "This key was rotated already; rotate the key that replaced it.".into(),
        ));
    }
    let mut conn = state.pg.acquire().await?;
    let models = kaeso_store::pg::catalogue::access(
        &mut conn,
        old.project_id,
        kaeso_store::pg::catalogue::Subject::Key(old.id),
    )
    .await?;
    let limits: Vec<super::limits::KeyLimit> =
        kaeso_store::pg::limits::list(&mut *conn, old.project_id)
            .await?
            .into_iter()
            .filter(|l| l.api_key_id == Some(old.id))
            .map(|l| super::limits::KeyLimit {
                kind: l.kind,
                model: l.catalogue_model_id.map(CatalogueModelId::from_uuid),
                period: l.period,
                amount: Some(l.amount),
                hard: Some(l.hard),
                alert_thresholds: Some(
                    l.alert_thresholds
                        .iter()
                        .filter_map(|t| u8::try_from(*t).ok())
                        .collect(),
                ),
            })
            .collect();
    Ok(KeyCreate {
        name: old.name.clone(),
        access: KeyAccess::from_flags(old.inference, old.platform),
        expiry: req.expiry.clone(),
        models: models.map(|m| m.into_iter().map(CatalogueModelId::from_uuid).collect()),
        limits: (!limits.is_empty()).then_some(limits),
    })
}

fn require_right(ctx: &Ctx, right: ProjectRight) -> ApiResult<()> {
    if ctx.project()?.rights.contains(right) {
        Ok(())
    } else {
        Err(denied(Right::Project(right), "project"))
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/keys/{key}/rotate",
    summary = "Rotates one of the caller's keys: a new key with the same name, access,\n\
    models and limits, while the old key keeps working for the grace period.",
    description = "Answers with the new key and its secret, shown once. The old key stops at \
    the end of the grace period, or at its own expiry if that is sooner, and links to the new \
    key in `replaced_by`. Rotating also needs `keys.own.manage` (or `keys.all.manage`), and a \
    key with platform access needs what creating one needs.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("key" = ApiKeyId, Path)),
    request_body = KeyRotate,
    responses((status = 201, body = CreatedKey<KeyView>))
)]
pub async fn rotate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    Body(req): Body<KeyRotate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    let row = key(&state, &ctx, &id).await?;
    if row.service_account_id.is_some() {
        return Err(ApiError::bad_request(
            "Rotate a service account's key through service-accounts/{account}/keys/{key}/rotate.",
        ));
    }
    let user = ctx
        .acting_user()
        .filter(|u| Some(u.uuid()) == row.user_id)
        .ok_or_else(|| {
            ApiError::PermissionDenied(
                "Only a key's owner can rotate it; the new key acts with the owner's rights."
                    .into(),
            )
        })?;
    if !ctx.project()?.rights.contains(ProjectRight::KeysAllManage) {
        require_right(&ctx, ProjectRight::KeysOwnManage)?;
    }
    let stops_at = req.grace_ends_at(ctx.now)?;
    let create = replacement(&state, &row, &req).await?;
    create_key(
        &state,
        &ctx,
        create,
        Some(user),
        None,
        Some(Rotation {
            old: &row,
            stops_at,
        }),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}/keys/{key}/rotate",
    summary = "Rotates a service account's key: a new key with the same name, access,\n\
    models and limits, while the old key keeps working for the grace period.",
    description = "Answers with the new key and its secret, shown once. The old key stops at \
    the end of the grace period, or at its own expiry if that is sooner, and links to the new \
    key in `replaced_by`. Rotating also needs `keys.all.manage`, and a key with platform access \
    needs what creating one needs.",
    tag = "projects",
    params(
        ("org" = OrgId, Path),
        ("project" = ProjectId, Path),
        ("account" = ServiceAccountId, Path),
        ("key" = ApiKeyId, Path)
    ),
    request_body = KeyRotate,
    responses((status = 201, body = CreatedKey<KeyView>))
)]
pub async fn rotate_for_service_account(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, account, id)): Path<(String, String, String, String)>,
    Body(req): Body<KeyRotate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<KeyView>>)> {
    let svc = keyed_service_account(&state, &ctx, &account).await?;
    let row = key(&state, &ctx, &id).await?;
    if row.service_account_id != Some(svc) {
        return Err(ApiError::not_found("key"));
    }
    require_right(&ctx, ProjectRight::KeysAllManage)?;
    let stops_at = req.grace_ends_at(ctx.now)?;
    let create = replacement(&state, &row, &req).await?;
    create_key(
        &state,
        &ctx,
        create,
        None,
        Some(svc),
        Some(Rotation {
            old: &row,
            stops_at,
        }),
    )
    .await
}
