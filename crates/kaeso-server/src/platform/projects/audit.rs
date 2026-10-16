use axum::Json;
use axum::extract::State;
use kaeso_core::id::{OrgId, ProjectId};

use crate::platform::access::Ctx;
use crate::platform::audit_log::{self, AuditEventView, AuditFilter};
use crate::platform::error::ApiResult;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/audit-log",
    summary = "The project's audit log, newest first.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams, AuditFilter),
    responses((status = 200, body = List<AuditEventView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
    Query(mut filter): Query<AuditFilter>,
) -> ApiResult<Json<List<AuditEventView>>> {
    audit_log::check_range(&filter)?;
    filter.project = Some(ctx.project()?.id());
    Ok(Json(
        audit_log::page(&state.pg, Some(ctx.org()?.id()), page.page()?, &filter).await?,
    ))
}
