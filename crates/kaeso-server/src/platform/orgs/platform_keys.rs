use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, OrgPlatformKeyId};
use kaeso_core::keys::KeyKind;
use kaeso_core::rights::{OrgHolding, OrgPreset, OrgRight, Right};
use kaeso_store::pg::keys;
use secrecy::ExposeSecret;
use serde::Deserialize;
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::{Caller, Ctx, denied};
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{
    CreatedKey, ExpiryRequest, KeyRotate, OrgKeyView, creator_of, new_secret, validate_name,
};
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new org platform key.")]
pub struct OrgKeyCreate {
    #[doc = "A name to recognise it by."]
    #[schema(example = "terraform")]
    pub name: String,
    #[doc = "When it expires."]
    #[serde(flatten)]
    pub expiry: ExpiryRequest,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/platform-keys",
    summary = "Creates an org platform key. Only with a console session. The key acts\n\
    with the admin role's rights, so the caller must hold all of them.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = OrgKeyCreate,
    responses((status = 201, body = CreatedKey<OrgKeyView>))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<OrgKeyCreate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<OrgKeyView>>)> {
    may_mint(&ctx)?;
    let name = validate_name(&req.name)?;
    let expires_at = req
        .expiry
        .resolve(ctx.now, ctx.org()?.org.max_key_lifetime_days)?;
    mint(&state, &ctx, &name, expires_at, None).await
}

fn may_mint(ctx: &Ctx) -> ApiResult<()> {
    ctx.session()?;
    let admin = OrgHolding {
        org: OrgPreset::Admin.rights(),
        every_project: OrgPreset::Admin.project_rights(),
    };
    ctx.org()?
        .holding()
        .check_change(&OrgHolding::default(), &admin)?;
    Ok(())
}

async fn mint(
    state: &AppState,
    ctx: &Ctx,
    name: &str,
    expires_at: Option<OffsetDateTime>,
    rotation: Option<(&keys::OrgKeyRow, OffsetDateTime)>,
) -> ApiResult<(StatusCode, Json<CreatedKey<OrgKeyView>>)> {
    let scope = ctx.org()?;
    let secret = new_secret(KeyKind::OrgPlatform);
    let id = OrgPlatformKeyId::new();
    let (kind, by) = creator_of(ctx);
    let mut tx = state.pg.begin().await?;
    keys::insert_org(
        &mut *tx,
        &keys::NewOrgKey {
            id: id.uuid(),
            org_id: scope.org.id,
            name,
            secret_hash: &secret.hash,
            secret_prefix: &secret.prefix,
            secret_last4: &secret.last4,
            expires_at,
            created_by_kind: kind,
            created_by_id: by,
            rotated_from: rotation.map(|(old, _)| old.id),
        },
    )
    .await?;
    let row = keys::get_org(&mut *tx, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new key vanished"))?;
    let view = OrgKeyView::from_row(&row, ctx.now);
    match rotation {
        None => {
            ctx.audit(Some(scope.id()), Action::PlatformKeyCreate)
                .target("platform_key", id)
                .after(&view)
                .write(&mut tx)
                .await?;
        }
        Some((old, stops_at)) => {
            if !keys::retire_org(&mut *tx, scope.org.id, old.id, id.uuid(), stops_at).await? {
                return Err(ApiError::Conflict(
                    "The key changed meanwhile; reload and try again.".into(),
                ));
            }
            let retired = keys::get_org(&mut *tx, scope.org.id, old.id)
                .await?
                .ok_or_else(|| ApiError::not_found("platform key"))?;
            ctx.audit(Some(scope.id()), Action::PlatformKeyRotate)
                .target("platform_key", OrgPlatformKeyId::from_uuid(old.id))
                .before(OrgKeyView::from_row(old, ctx.now))
                .after(OrgKeyView::from_row(&retired, ctx.now))
                .write(&mut tx)
                .await?;
        }
    }
    tx.commit().await?;
    if let Some((old, _)) = rotation {
        crate::services::keys::forget_org_key(state, old.id).await;
    }
    Ok((
        StatusCode::CREATED,
        Json(CreatedKey {
            key: view,
            secret: secret.secret.expose_secret().to_owned(),
        }),
    ))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OrgKeyFilter {
    #[doc = "Include archived keys (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/platform-keys",
    summary = "The org's platform keys.",
    tag = "orgs",
    params(("org" = OrgId, Path), PageParams, OrgKeyFilter),
    responses((status = 200, body = List<OrgKeyView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(page): Query<PageParams>,
    Query(f): Query<OrgKeyFilter>,
) -> ApiResult<Json<List<OrgKeyView>>> {
    let page = page.page()?;
    let rows = keys::list_org(
        &state.pg,
        ctx.org()?.org.id,
        f.include_archived.unwrap_or(false),
        page.after,
        page.fetch(),
    )
    .await?;
    let now = ctx.now;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |k| k.id,
        |k| OrgKeyView::from_row(&k, now),
    )))
}

async fn act(state: &AppState, ctx: &Ctx, id: &str, action: Action) -> ApiResult<Json<OrgKeyView>> {
    let scope = ctx.org()?;
    let id: OrgPlatformKeyId = id
        .parse()
        .map_err(|_| ApiError::not_found("platform key"))?;
    let (kind, by) = creator_of(ctx);
    let actor = keys::Actor { kind, id: by };
    let mut tx = state.pg.begin().await?;
    let done = match action {
        Action::PlatformKeyDisable => {
            keys::set_org_disabled(&mut *tx, scope.org.id, id.uuid(), Some(ctx.now), actor).await?
        }
        Action::PlatformKeyEnable => {
            let by_person = keys::org_disabled_by(&mut *tx, scope.org.id, id.uuid())
                .await?
                .is_some_and(|d| d.kind.as_deref() == Some("user"));
            if by_person && !matches!(ctx.caller, Caller::Session(_)) {
                return Err(ApiError::PermissionDenied(
                    crate::platform::projects::keys::PERSON_DISABLED.into(),
                ));
            }
            keys::set_org_disabled(&mut *tx, scope.org.id, id.uuid(), None, actor).await?
        }
        _ => keys::archive_org(&mut *tx, scope.org.id, id.uuid(), ctx.now).await?,
    };
    if !done {
        return Err(ApiError::NotFound(
            "No such platform key that can change.".into(),
        ));
    }
    ctx.audit(Some(scope.id()), action)
        .target("platform_key", id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_org_key(state, id.uuid()).await;
    let row = keys::get_org(&state.pg, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("platform key"))?;
    Ok(Json(OrgKeyView::from_row(&row, ctx.now)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/platform-keys/{key}/disable",
    summary = "Disables an org platform key.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("key" = OrgPlatformKeyId, Path)),
    responses((status = 200, body = OrgKeyView))
)]
pub async fn disable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, id)): Path<(String, String)>,
) -> ApiResult<Json<OrgKeyView>> {
    act(&state, &ctx, &id, Action::PlatformKeyDisable).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/platform-keys/{key}/enable",
    summary = "Enables a disabled org platform key.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("key" = OrgPlatformKeyId, Path)),
    responses((status = 200, body = OrgKeyView))
)]
pub async fn enable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, id)): Path<(String, String)>,
) -> ApiResult<Json<OrgKeyView>> {
    act(&state, &ctx, &id, Action::PlatformKeyEnable).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/platform-keys/{key}/archive",
    summary = "Archives an org platform key. Permanent.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("key" = OrgPlatformKeyId, Path)),
    responses((status = 200, body = OrgKeyView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, id)): Path<(String, String)>,
) -> ApiResult<Json<OrgKeyView>> {
    act(&state, &ctx, &id, Action::PlatformKeyArchive).await
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/platform-keys/{key}",
    summary = "One org platform key.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("key" = OrgPlatformKeyId, Path)),
    responses((status = 200, body = OrgKeyView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<OrgKeyView>> {
    let id: OrgPlatformKeyId = id
        .parse()
        .map_err(|_| ApiError::not_found("platform key"))?;
    let row = keys::get_org(&state.pg, ctx.org()?.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("platform key"))?;
    Ok(Json(OrgKeyView::from_row(&row, ctx.now)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/platform-keys/{key}/rotate",
    summary = "Rotates an org platform key: a new key with the same name, while the old\n\
    key keeps working for the grace period. Only with a console session.",
    description = "Answers with the new key and its secret, shown once. The old key stops at \
    the end of the grace period, or at its own expiry if that is sooner, and links to the new \
    key in `replaced_by`. Rotating also needs `platform_keys.revoke`, and the caller must hold \
    every right of the admin role, as for creating one.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("key" = OrgPlatformKeyId, Path)),
    request_body = KeyRotate,
    responses((status = 201, body = CreatedKey<OrgKeyView>))
)]
pub async fn rotate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, id)): Path<(String, String)>,
    Body(req): Body<KeyRotate>,
) -> ApiResult<(StatusCode, Json<CreatedKey<OrgKeyView>>)> {
    let scope = ctx.org()?;
    let id: OrgPlatformKeyId = id
        .parse()
        .map_err(|_| ApiError::not_found("platform key"))?;
    let old = keys::get_org(&state.pg, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("platform key"))?;
    if !scope.rights.contains(OrgRight::PlatformKeysRevoke) {
        return Err(denied(
            Right::Org(OrgRight::PlatformKeysRevoke),
            "organization",
        ));
    }
    may_mint(&ctx)?;
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
    let stops_at = req.grace_ends_at(ctx.now)?;
    let expires_at = req
        .expiry
        .resolve(ctx.now, scope.org.max_key_lifetime_days)?;
    mint(&state, &ctx, &old.name, expires_at, Some((&old, stops_at))).await
}
