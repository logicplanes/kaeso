use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{AgentId, OrgId, UserId};
use kaeso_core::rights::AgentPreset;
use kaeso_store::pg::agents;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::auth::agents::OrgPolicy;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::orgs::members::MemberUser;
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::platform::validate::display_name;
use crate::platform::views::avatar_url;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An agent of a member that may act in the organization.")]
pub struct OrgAgentView {
    #[doc = "The agent."]
    pub id: AgentId,
    #[doc = "The name its user gave it."]
    #[schema(example = "Release helper")]
    pub name: String,
    #[doc = "The model family it reports, or `unknown`."]
    #[schema(example = "claude")]
    pub model: String,
    #[doc = "The member it acts for."]
    pub user: MemberUser,
    #[doc = "When it was authorized."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When it last used Kaeso; `null` if never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "When its sign-in ends; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/agents",
    summary = "The members' agents that may act in the organization, newest first.",
    description = "Each acts only with rights its member holds here and the org's agent\n\
    policy allows.",
    tag = "agents",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<OrgAgentView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<OrgAgentView>>> {
    let org = ctx.org()?;
    let rows = agents::in_org(&state.pg, org.org.id, ctx.now).await?;
    Ok(Json(List::complete(
        rows.into_iter()
            .map(|a| OrgAgentView {
                id: AgentId::from_uuid(a.id),
                name: a.name,
                model: a.model,
                user: MemberUser {
                    id: UserId::from_uuid(a.user_id),
                    name: a
                        .user_name
                        .as_deref()
                        .and_then(display_name)
                        .unwrap_or_else(|| "Deleted user".to_owned()),
                    email: a.user_email,
                    state: "active".to_owned(),
                    avatar_url: avatar_url(a.user_avatar.as_deref()),
                },
                created_at: a.created_at,
                last_used_at: a.last_used_at,
                expires_at: a.expires_at,
            })
            .collect(),
    )))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/agents/{agent}",
    summary = "Closes the organization to a member's agent.",
    description = "The agent keeps working elsewhere; its member can open the org to it\n\
    again.",
    tag = "agents",
    params(("org" = OrgId, Path), ("agent" = AgentId, Path)),
    responses((status = 204, description = "Closed."))
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?;
    let id: AgentId = id.parse().map_err(|_| ApiError::not_found("agent"))?;
    let found = agents::in_org(&state.pg, org.org.id, ctx.now)
        .await?
        .into_iter()
        .find(|a| a.id == id.uuid())
        .ok_or_else(|| ApiError::not_found("agent"))?;
    let mut tx = state.pg.begin().await?;
    agents::exclude_org(&mut tx, found.id, org.org.id).await?;
    ctx.audit(Some(org.id()), "agent.org_access_revoke")
        .target("agent", id)
        .before(serde_json::json!({
            "name": found.name,
            "model": found.model,
            "user": UserId::from_uuid(found.user_id).to_string(),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Whether the organization lets its members' agents act in it, and how far.")]
pub struct OrgAgentPolicy {
    #[doc = "Whether members' agents may act in the organization."]
    pub allowed: bool,
    #[doc = "The most agents may have here: `observer`, `builder`, `operator` or\n\
        `full`; each includes the ones before it."]
    #[schema(example = "full")]
    pub max: String,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/security/agents",
    summary = "The organization's agent policy.",
    tag = "agents",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgAgentPolicy))
)]
pub async fn policy(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<OrgAgentPolicy>> {
    let p = agents::org_policy(&state.pg, ctx.org()?.org.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    let p = OrgPolicy::from_store(&p);
    Ok(Json(OrgAgentPolicy {
        allowed: p.allowed,
        max: p.max.as_str().to_owned(),
    }))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/agents",
    summary = "Sets the organization's agent policy.",
    description = "Narrowing it applies to every agent at its next request; no agent can\n\
    change it.",
    tag = "agents",
    params(("org" = OrgId, Path)),
    request_body = OrgAgentPolicy,
    responses((status = 200, body = OrgAgentPolicy))
)]
pub async fn set_policy(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<OrgAgentPolicy>,
) -> ApiResult<Json<OrgAgentPolicy>> {
    let org = ctx.org()?;
    let max: AgentPreset = req.max.parse().map_err(|_| {
        ApiError::invalid(
            "max",
            "`max` must be `observer`, `builder`, `operator` or `full`.",
        )
    })?;
    let mut tx = state.pg.begin().await?;
    let before = agents::org_policy(&mut *tx, org.org.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    agents::set_org_policy(
        &mut *tx,
        org.org.id,
        agents::Policy {
            allowed: req.allowed,
            max: max.as_str(),
        },
    )
    .await?;
    ctx.audit(Some(org.id()), "security.agents_policy")
        .target("org", org.id())
        .before(serde_json::json!({ "allowed": before.allowed, "max": before.max }))
        .after(serde_json::json!({ "allowed": req.allowed, "max": max.as_str() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(OrgAgentPolicy {
        allowed: req.allowed,
        max: max.as_str().to_owned(),
    }))
}
