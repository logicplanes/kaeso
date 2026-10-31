use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_store::pg::response_cache::{self as store, ProjectCache};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::access::Ctx;
use super::error::{ApiError, ApiResult};
use super::json::Body as JsonBody;
use crate::platform::audit::Action;
use crate::platform::params::Path;
use crate::services::response_cache::{forget, sweep};
use crate::state::AppState;

pub const MIN_TTL_SECONDS: i32 = 60;
pub const MAX_TTL_SECONDS: i32 = 7 * 24 * 3600;

const OFF_ON_INSTALL: &str = "The response cache is off on this install.";
const OFF_IN_ORG: &str =
    "The organization has the response cache off; a project cannot turn it on.";
const ARCHIVED: &str = "The project is archived; its response cache cannot change.";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An organization's response cache policy.")]
pub struct OrgResponseCache {
    #[doc = "Whether the organization's projects may turn the response cache on."]
    pub allowed: bool,
    #[doc = "Whether the install offers the response cache at all."]
    pub available: bool,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/response-cache",
    summary = "Whether the organization's projects may use the response cache.",
    tag = "response-cache",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgResponseCache))
)]
pub async fn get_org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<OrgResponseCache>> {
    let allowed = store::org_allows(&state.pg, ctx.org()?.org.id).await?;
    Ok(Json(OrgResponseCache {
        allowed,
        available: state.config.response_cache.enabled,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A change to the organization's response cache policy.")]
pub struct OrgResponseCacheUpdate {
    #[doc = "Let projects turn the response cache on. Off turns it off in every\n\
        project and clears their entries."]
    pub allowed: bool,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/response-cache",
    summary = "Allows or forbids the response cache in the organization's projects.",
    description = "Forbidding it stops every project's cache at once and clears their entries;\n\
    allowing it again leaves each project's own switch as it was.",
    tag = "response-cache",
    params(("org" = OrgId, Path)),
    request_body = OrgResponseCacheUpdate,
    responses((status = 200, body = OrgResponseCache))
)]
pub async fn update_org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    JsonBody(req): JsonBody<OrgResponseCacheUpdate>,
) -> ApiResult<Json<OrgResponseCache>> {
    let mut tx = state.pg.begin().await?;
    let org = crate::platform::orgs::org::lock_changeable(&mut tx, &ctx).await?;
    let before = store::org_allows(&mut *tx, org.id).await?;
    let mut cleared = Vec::new();
    if before != req.allowed {
        store::set_org(&mut *tx, org.id, req.allowed).await?;
        if !req.allowed {
            cleared = store::clear_org(&mut *tx, org.id).await?;
        }
        ctx.audit(
            Some(OrgId::from_uuid(org.id)),
            Action::ResponseCacheOrgUpdate,
        )
        .target("org", OrgId::from_uuid(org.id))
        .before(serde_json::json!({ "allowed": before }))
        .after(serde_json::json!({ "allowed": req.allowed }))
        .write(&mut tx)
        .await?;
    }
    tx.commit().await?;
    if before != req.allowed {
        for p in projects_of(&state, org.id).await? {
            forget(&state, p).await;
        }
        sweep(&state, cleared);
    }
    Ok(Json(OrgResponseCache {
        allowed: req.allowed,
        available: state.config.response_cache.enabled,
    }))
}

async fn projects_of(state: &AppState, org: uuid::Uuid) -> ApiResult<Vec<uuid::Uuid>> {
    let mut all = Vec::new();
    let mut after = None;
    loop {
        let page =
            kaeso_store::pg::projects::list(&state.pg, org, None, None, true, after, 500).await?;
        all.extend(page.iter().map(|p| p.id));
        match page.last() {
            Some(p) if page.len() == 500 => after = Some(p.id),
            _ => return Ok(all),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "A project's response cache.")]
pub struct ProjectResponseCache {
    #[doc = "The project's own switch."]
    pub enabled: bool,
    #[doc = "How long a reply stays in the cache, in seconds."]
    pub ttl_seconds: i32,
    #[doc = "Whether the organization allows the response cache."]
    pub org_allowed: bool,
    #[doc = "Whether the install offers the response cache at all."]
    pub available: bool,
    #[doc = "Whether requests are answered from the cache: the install offers it,\n\
        the organization allows it and the project's switch is on."]
    pub effective: bool,
}

impl ProjectResponseCache {
    fn of(state: &AppState, project: &ProjectCache, org_allowed: bool) -> ProjectResponseCache {
        let available = state.config.response_cache.enabled;
        ProjectResponseCache {
            enabled: project.enabled,
            ttl_seconds: project.ttl_seconds,
            org_allowed,
            available,
            effective: available && org_allowed && project.enabled,
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/response-cache",
    summary = "The project's response cache: its switch and how long replies are kept.",
    tag = "response-cache",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectResponseCache))
)]
pub async fn get_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectResponseCache>> {
    let p = &ctx.project()?.project;
    let s = store::settings(&state.pg, p.id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    let project = ProjectCache {
        enabled: s.enabled,
        ttl_seconds: s.ttl_seconds,
        generation: s.generation,
    };
    Ok(Json(ProjectResponseCache::of(
        &state,
        &project,
        s.org_allows,
    )))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A change to a project's response cache.")]
pub struct ProjectResponseCacheUpdate {
    #[doc = "Answer repeated identical requests from the cache. Off clears the\n\
        project's entries."]
    pub enabled: Option<bool>,
    #[doc = "How long a reply stays in the cache, in seconds: 60 to 604800\n\
        (7 days). Applies to replies stored from now on."]
    pub ttl_seconds: Option<i32>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/response-cache",
    summary = "Turns the project's response cache on or off, or changes how long replies\n\
    are kept.",
    description = "Turning it off clears the project's entries. It cannot be turned on while the\n\
    organization forbids it or the install does not offer it.",
    tag = "response-cache",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectResponseCacheUpdate,
    responses((status = 200, body = ProjectResponseCache))
)]
pub async fn update_project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    JsonBody(req): JsonBody<ProjectResponseCacheUpdate>,
) -> ApiResult<Json<ProjectResponseCache>> {
    let id = crate::platform::projects::ensure_active(&ctx, ARCHIVED)?;
    if let Some(ttl) = req.ttl_seconds
        && !(MIN_TTL_SECONDS..=MAX_TTL_SECONDS).contains(&ttl)
    {
        return Err(ApiError::invalid(
            "ttl_seconds",
            "Must be 60 to 604800 seconds (1 minute to 7 days).",
        ));
    }
    let org_id = ctx.org()?.org.id;
    let mut tx = state.pg.begin().await?;
    let turning_on = req.enabled == Some(true);
    let org_allowed = if turning_on {
        crate::platform::orgs::org::lock_changeable(&mut tx, &ctx).await?;
        store::org_allows(&mut *tx, org_id).await?
    } else {
        store::org_allows(&mut *tx, org_id).await?
    };
    let before = store::project_for_update(&mut *tx, org_id, id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    if turning_on && !before.enabled {
        if !state.config.response_cache.enabled {
            return Err(ApiError::Conflict(OFF_ON_INSTALL.to_owned()));
        }
        if !org_allowed {
            return Err(ApiError::Conflict(OFF_IN_ORG.to_owned()));
        }
    }
    let mut after = before;
    if let Some(on) = req.enabled {
        after.enabled = on;
    }
    if let Some(ttl) = req.ttl_seconds {
        after.ttl_seconds = ttl;
    }
    let cleared = before.enabled && !after.enabled;
    if cleared {
        after.generation = before.generation.saturating_add(1);
    }
    if after != before {
        store::set_project(&mut *tx, org_id, id, &after).await?;
        ctx.audit(
            Some(OrgId::from_uuid(org_id)),
            Action::ResponseCacheProjectUpdate,
        )
        .target("project", ProjectId::from_uuid(id))
        .before(serde_json::json!({ "enabled": before.enabled, "ttl_seconds": before.ttl_seconds }))
        .after(serde_json::json!({ "enabled": after.enabled, "ttl_seconds": after.ttl_seconds }))
        .write(&mut tx)
        .await?;
    }
    tx.commit().await?;
    if after != before {
        forget(&state, id).await;
    }
    if cleared {
        sweep(&state, vec![id]);
    }
    Ok(Json(ProjectResponseCache::of(&state, &after, org_allowed)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/response-cache/clear",
    summary = "Clears the project's response cache.",
    description = "Every stored reply stops being served at once, on every gateway instance; the\n\
    memory they take is freed in the background.",
    tag = "response-cache",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 204))
)]
pub async fn clear(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let id = crate::platform::projects::ensure_active(&ctx, ARCHIVED)?;
    let org_id = ctx.org()?.org.id;
    let mut tx = state.pg.begin().await?;
    let mut p = store::project_for_update(&mut *tx, org_id, id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    p.generation = p.generation.saturating_add(1);
    store::set_project(&mut *tx, org_id, id, &p).await?;
    ctx.audit(Some(OrgId::from_uuid(org_id)), Action::ResponseCacheClear)
        .target("project", ProjectId::from_uuid(id))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    forget(&state, id).await;
    sweep(&state, vec![id]);
    Ok(StatusCode::NO_CONTENT)
}
