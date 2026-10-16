use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, OrgPlatformKeyId};
use kaeso_core::keys::KeyKind;
use kaeso_core::rights::{OrgHolding, OrgPreset};
use kaeso_store::pg::keys;
use secrecy::ExposeSecret;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{
    CreatedKey, ExpiryRequest, OrgKeyView, creator_of, new_secret, validate_name,
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
    ctx.session()?;
    let scope = ctx.org()?;
    let admin = OrgHolding {
        org: OrgPreset::Admin.rights(),
        every_project: OrgPreset::Admin.project_rights(),
    };
    scope
        .holding()
        .check_change(&OrgHolding::default(), &admin)?;
    let name = validate_name(&req.name)?;
    let expires_at = req
        .expiry
        .resolve(ctx.now, scope.org.max_key_lifetime_days)?;
    let secret = new_secret(KeyKind::OrgPlatform);
    let id = OrgPlatformKeyId::new();
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    keys::insert_org(
        &mut *tx,
        &keys::NewOrgKey {
            id: id.uuid(),
            org_id: scope.org.id,
            name: &name,
            secret_hash: &secret.hash,
            secret_prefix: &secret.prefix,
            secret_last4: &secret.last4,
            expires_at,
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    let row = keys::get_org(&mut *tx, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new key vanished"))?;
    let view = OrgKeyView::from_row(&row, ctx.now);
    ctx.audit(Some(scope.id()), "platform_key.create")
        .target("platform_key", id)
        .after(&view)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
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

async fn act(
    state: &AppState,
    ctx: &Ctx,
    id: &str,
    action: &'static str,
) -> ApiResult<Json<OrgKeyView>> {
    let scope = ctx.org()?;
    let id: OrgPlatformKeyId = id
        .parse()
        .map_err(|_| ApiError::not_found("platform key"))?;
    let mut tx = state.pg.begin().await?;
    let done = match action {
        "platform_key.disable" => {
            keys::set_org_disabled(&mut *tx, scope.org.id, id.uuid(), Some(ctx.now)).await?
        }
        "platform_key.enable" => {
            keys::set_org_disabled(&mut *tx, scope.org.id, id.uuid(), None).await?
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
    act(&state, &ctx, &id, "platform_key.disable").await
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
    act(&state, &ctx, &id, "platform_key.enable").await
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
    act(&state, &ctx, &id, "platform_key.archive").await
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
