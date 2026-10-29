use axum::Json;
use axum::extract::State;
use kaeso_core::rights::InstancePreset;
use kaeso_store::pg::agents;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::agents::InstancePolicy;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(
    description = "Whether agents may sign in to the instance, and how far instance admins' agents may administer it."
)]
pub struct InstanceAgentPolicy {
    #[doc = "Whether agents may sign in through the MCP server at all."]
    pub allowed: bool,
    #[doc = "The most instance admins' agents may have on the instance: `none`,\n\
        `read`, `configure` or `admin`."]
    #[schema(example = "configure")]
    pub max: String,
}

#[utoipa::path(
    get,
    path = "/instance/settings/agents",
    summary = "The instance's agent policy.",
    tag = "instance",
    responses((status = 200, body = InstanceAgentPolicy))
)]
pub async fn get(State(state): State<AppState>, _ctx: Ctx) -> ApiResult<Json<InstanceAgentPolicy>> {
    let p = InstancePolicy::from_store(&agents::instance_policy(&state.pg).await?);
    Ok(Json(InstanceAgentPolicy {
        allowed: p.allowed,
        max: p.max.as_str().to_owned(),
    }))
}

#[utoipa::path(
    put,
    path = "/instance/settings/agents",
    summary = "Sets the instance's agent policy.",
    description = "Turning agents off stops every agent at its next request; no agent can\n\
    change this setting.",
    tag = "instance",
    request_body = InstanceAgentPolicy,
    responses((status = 200, body = InstanceAgentPolicy))
)]
pub async fn set(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<InstanceAgentPolicy>,
) -> ApiResult<Json<InstanceAgentPolicy>> {
    let max: InstancePreset = req.max.parse().map_err(|_| {
        ApiError::invalid(
            "max",
            "`max` must be `none`, `read`, `configure` or `admin`.",
        )
    })?;
    let mut tx = state.pg.begin().await?;
    let before = agents::instance_policy(&mut *tx).await?;
    agents::set_instance_policy(
        &mut *tx,
        agents::Policy {
            allowed: req.allowed,
            max: max.as_str(),
        },
    )
    .await?;
    ctx.audit(None, Action::InstanceAgentsPolicy)
        .before(serde_json::json!({ "allowed": before.allowed, "max": before.max }))
        .after(serde_json::json!({ "allowed": req.allowed, "max": max.as_str() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(InstanceAgentPolicy {
        allowed: req.allowed,
        max: max.as_str().to_owned(),
    }))
}
