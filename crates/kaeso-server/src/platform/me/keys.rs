use axum::Json;
use axum::extract::State;
use kaeso_core::id::{ApiKeyId, OrgId, ProjectId};
use kaeso_core::rights::{ProjectRight, Right, effective_project_rights};
use kaeso_store::pg::{keys, members};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::platform::access::{
    Area, Ctx, agent_scope, managed_by, policy_gap, refused, require_policy,
};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{org_from_store, project_from_store};
use crate::platform::keys::KeyView;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::projects::keys::{Change, Holder, change};
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
    summary = "The signed-in user's keys, in every project of the orgs this session may\n\
    open.",
    description = "Leaves out the keys in an org whose `access` in `auth/session` isn't `ok`, \
    such as another org's when the session came from the SSO of an org that doesn't manage the \
    account. An agent sees only the keys in projects it can open, in orgs that allow agents.",
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
    let session = ctx.sign_in()?;
    let managed = managed_by(&state, session).await?.map(|m| m.org_id);
    let open: Vec<uuid::Uuid> = members::orgs_of_user(&state.pg, session.user_id)
        .await?
        .into_iter()
        .filter(|m| {
            let area = Area::Org {
                id: m.org_id,
                require_sso: m.require_sso,
                require_2fa: m.require_2fa,
            };
            policy_gap(session, managed, area).is_none()
        })
        .map(|m| m.org_id)
        .collect();
    let projects = match ctx.agent() {
        Some(agent) => Some(
            crate::services::agents::holdings(&state.pg, session.user_id)
                .await?
                .iter()
                .filter(|held| open.contains(&held.id))
                .filter_map(|held| crate::services::agents::reach(agent, held))
                .flat_map(|reach| reach.projects.into_iter().map(|(id, _)| id))
                .collect::<Vec<_>>(),
        ),
        None => None,
    };
    let rows = keys::list_of_user(
        &state.pg,
        session.user_id,
        &open,
        projects.as_deref(),
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
    if let Some(agent) = ctx.agent() {
        let (_, project) = agent_scope(
            state,
            agent,
            OrgId::from_uuid(row.org_id),
            Some(ProjectId::from_uuid(row.project_id)),
            true,
        )
        .await
        .map_err(|e| match e {
            ApiError::NotFound(_) => ApiError::not_found("key"),
            e => e,
        })?;
        if !project.is_some_and(|p| p.rights.contains(ProjectRight::KeysOwnManage)) {
            return Err(refused(
                &ctx.caller,
                Right::Project(ProjectRight::KeysOwnManage),
                "project",
            ));
        }
        return Ok(row);
    }
    let mut conn = state.pg.acquire().await?;
    let found =
        members::access_context(&mut conn, row.org_id, Some(me.uuid()), Some(row.project_id))
            .await?;
    if let Some(f) = &found {
        if f.org.state == "scheduled_for_deletion" {
            return Err(ApiError::OrgScheduledForDeletion);
        }
        require_policy(state, ctx.sign_in()?, Area::of_org(&f.org)).await?;
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
            Holder::Owner,
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
            Holder::Owner,
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
            Holder::Owner,
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
