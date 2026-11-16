use axum::Json;
use axum::extract::State;
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_store::pg::stored_responses as store;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::access::Ctx;
use super::error::{ApiError, ApiResult};
use super::json::Body as JsonBody;
use crate::platform::audit::Action;
use crate::platform::params::Path;
use crate::services::response_cache::forget;
use crate::state::AppState;

const OFF_IN_ORG: &str =
    "The organization doesn't allow stored responses; a project can't turn them on.";
const ARCHIVED: &str = "The project is archived; its stored responses setting cannot change.";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Whether the organization's projects may keep responses at providers.")]
pub struct OrgStoredResponses {
    #[doc = "Whether projects may let providers keep responses for `previous_response_id`."]
    pub allowed: bool,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/stored-responses",
    summary = "Whether the organization's projects may keep responses at providers.",
    tag = "stored-responses",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgStoredResponses))
)]
pub async fn get_org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<OrgStoredResponses>> {
    let allowed = store::org_allows(&state.pg, ctx.org()?.org.id).await?;
    Ok(Json(OrgStoredResponses { allowed }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(
    description = "A change to whether the organization's projects may keep responses at providers."
)]
pub struct OrgStoredResponsesUpdate {
    #[doc = "Let projects keep responses at providers. Off stops it in every project;\n\
        responses already kept stay at their providers until they expire."]
    pub allowed: bool,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/stored-responses",
    summary = "Allows or forbids the organization's projects to keep responses at providers.",
    tag = "stored-responses",
    params(("org" = OrgId, Path)),
    request_body = OrgStoredResponsesUpdate,
    responses((status = 200, body = OrgStoredResponses))
)]
pub async fn update_org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    JsonBody(req): JsonBody<OrgStoredResponsesUpdate>,
) -> ApiResult<Json<OrgStoredResponses>> {
    let mut tx = state.pg.begin().await?;
    let org = crate::platform::orgs::org::lock_changeable(&mut tx, &ctx).await?;
    let before = store::org_allows(&mut *tx, org.id).await?;
    if before != req.allowed {
        store::set_org(&mut *tx, org.id, req.allowed).await?;
        ctx.audit(
            Some(OrgId::from_uuid(org.id)),
            Action::StoredResponsesOrgUpdate,
        )
        .target("org", OrgId::from_uuid(org.id))
        .before(serde_json::json!({ "allowed": before }))
        .after(serde_json::json!({ "allowed": req.allowed }))
        .write(&mut tx)
        .await?;
    }
    tx.commit().await?;
    if before != req.allowed {
        for p in crate::platform::response_cache::projects_of(&state, org.id).await? {
            forget(&state, p).await;
        }
    }
    Ok(Json(OrgStoredResponses {
        allowed: req.allowed,
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Whether a project keeps responses at providers.")]
pub struct ProjectStoredResponses {
    #[doc = "The project's own switch."]
    pub enabled: bool,
    #[doc = "Whether the organization allows it."]
    pub org_allowed: bool,
    #[doc = "Whether requests may keep and continue responses at providers: the\n\
        organization allows it and the project's switch is on."]
    pub effective: bool,
}

impl ProjectStoredResponses {
    fn of(enabled: bool, org_allowed: bool) -> ProjectStoredResponses {
        ProjectStoredResponses {
            enabled,
            org_allowed,
            effective: enabled && org_allowed,
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/stored-responses",
    summary = "Whether the project keeps responses at providers.",
    tag = "stored-responses",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectStoredResponses))
)]
pub async fn get_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectStoredResponses>> {
    let p = &ctx.project()?.project;
    let s = store::settings(&state.pg, p.id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    Ok(Json(ProjectStoredResponses::of(s.enabled, s.org_allows)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A change to whether a project keeps responses at providers.")]
pub struct ProjectStoredResponsesUpdate {
    #[doc = "Let providers keep this project's responses so apps can continue them with\n\
        `previous_response_id`. Off refuses `store: true` and continuations; responses\n\
        already kept stay at their providers until they expire."]
    pub enabled: bool,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/stored-responses",
    summary = "Turns responses kept at providers on or off for the project.",
    description = "It cannot be turned on while the organization forbids it.",
    tag = "stored-responses",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectStoredResponsesUpdate,
    responses((status = 200, body = ProjectStoredResponses))
)]
pub async fn update_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    JsonBody(req): JsonBody<ProjectStoredResponsesUpdate>,
) -> ApiResult<Json<ProjectStoredResponses>> {
    let id = crate::platform::projects::ensure_active(&ctx, ARCHIVED)?;
    let org_id = ctx.org()?.org.id;
    let mut tx = state.pg.begin().await?;
    if req.enabled {
        crate::platform::orgs::org::lock_changeable(&mut tx, &ctx).await?;
    }
    let org_allowed = store::org_allows(&mut *tx, org_id).await?;
    let before = store::project_for_update(&mut *tx, org_id, id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    if req.enabled && !before && !org_allowed {
        return Err(ApiError::Conflict(OFF_IN_ORG.to_owned()));
    }
    if before != req.enabled {
        store::set_project(&mut *tx, org_id, id, req.enabled).await?;
        ctx.audit(
            Some(OrgId::from_uuid(org_id)),
            Action::StoredResponsesProjectUpdate,
        )
        .target("project", ProjectId::from_uuid(id))
        .before(serde_json::json!({ "enabled": before }))
        .after(serde_json::json!({ "enabled": req.enabled }))
        .write(&mut tx)
        .await?;
    }
    tx.commit().await?;
    if before != req.enabled {
        forget(&state, id).await;
    }
    Ok(Json(ProjectStoredResponses::of(req.enabled, org_allowed)))
}
