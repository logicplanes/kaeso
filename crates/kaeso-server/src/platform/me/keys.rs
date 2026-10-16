use axum::Json;
use axum::extract::State;
use kaeso_core::id::{ApiKeyId, OrgId};
use kaeso_core::rights::{ProjectRight, effective_project_rights};
use kaeso_store::pg::{keys, members};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{org_from_store, project_from_store};
use crate::platform::keys::KeyView;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::projects::keys::{Change, change};
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MyKeyFilter {
    #[doc = "Include archived keys (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/me/keys",
    summary = "Every key of the signed-in user, in every project.",
    tag = "me",
    params(PageParams, MyKeyFilter),
    responses((status = 200, body = List<KeyView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Query(page): Query<PageParams>,
    Query(f): Query<MyKeyFilter>,
) -> ApiResult<Json<List<KeyView>>> {
    let page = page.page()?;
    let rows = keys::list_of_user(
        &state.pg,
        ctx.user_id()?.uuid(),
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
        |k| KeyView::from_row(&k, now),
    )))
}

async fn own(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<keys::KeyRow> {
    let me = ctx.user_id()?;
    let id: ApiKeyId = id.parse().map_err(|_| ApiError::not_found("key"))?;
    let row = keys::get_of_user(&state.pg, me.uuid(), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("key"))?;
    let mut conn = state.pg.acquire().await?;
    let found =
        members::access_context(&mut conn, row.org_id, Some(me.uuid()), Some(row.project_id))
            .await?;
    if found
        .as_ref()
        .is_some_and(|f| f.org.state == "scheduled_for_deletion")
    {
        return Err(ApiError::OrgScheduledForDeletion);
    }
    let rights = match found {
        Some(f) => {
            let org = f.org_grant.as_ref().map(org_from_store).transpose()?;
            let project = f
                .project_grant
                .as_ref()
                .map(project_from_store)
                .transpose()?;
            effective_project_rights(org.as_ref(), project.as_ref())
        }
        None => Default::default(),
    };
    if !rights.contains(ProjectRight::KeysOwnManage) {
        return Err(ApiError::PermissionDenied(
            "You need the right `keys.own.manage` in this key's project to change it.".into(),
        ));
    }
    Ok(row)
}

#[utoipa::path(
    post,
    path = "/me/keys/{key}/disable",
    summary = "Disables one of the user's keys.",
    tag = "me",
    params(("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn disable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<KeyView>> {
    let row = own(&state, &ctx, &id).await?;
    Ok(Json(
        change(
            &state,
            &ctx,
            OrgId::from_uuid(row.org_id),
            &row,
            Change::Disable,
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/me/keys/{key}/enable",
    summary = "Enables one of the user's disabled keys.",
    tag = "me",
    params(("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn enable(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<KeyView>> {
    let row = own(&state, &ctx, &id).await?;
    Ok(Json(
        change(
            &state,
            &ctx,
            OrgId::from_uuid(row.org_id),
            &row,
            Change::Enable,
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/me/keys/{key}/archive",
    summary = "Archives one of the user's keys. Permanent.",
    tag = "me",
    params(("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<KeyView>> {
    let row = own(&state, &ctx, &id).await?;
    Ok(Json(
        change(
            &state,
            &ctx,
            OrgId::from_uuid(row.org_id),
            &row,
            Change::Archive,
        )
        .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/me/keys/{key}",
    summary = "One of the user's own keys.",
    tag = "me",
    params(("key" = ApiKeyId, Path)),
    responses((status = 200, body = KeyView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<KeyView>> {
    Ok(Json(KeyView::from_row(
        &own(&state, &ctx, &id).await?,
        ctx.now,
    )))
}
