use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{AgentActivityId, AgentId, OrgId, ProjectId};
use kaeso_core::rights::{
    AccountRight, AgentPreset, InstancePreset, InstanceRight, OrgRight, ProjectRight,
    grantable_org_rights,
};
use kaeso_core::secret;
use kaeso_store::pg::agents::{self, Agent};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::auth::agents::{AgentGrant, InstancePolicy, Projects};
use crate::mcp::oauth;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::rights::{RightView, ScopeCatalogue, group};
use crate::platform::validate::validate_name;
use crate::platform::views::avatar_url;
use crate::services::agents::{self as service, Expiry, Granting, MODELS};
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(
    description = "What an agent may do: its rights on the instance, the account and each organization."
)]
pub struct GrantBody {
    #[doc = "The preset it started from: `observer`, `builder`, `operator` or `full`."]
    #[schema(example = "builder")]
    pub preset: String,
    #[doc = "Whether rights were changed after choosing the preset; the console\n\
        then shows \"Custom (based on …)\"."]
    #[serde(default)]
    pub customized: bool,
    #[doc = "Instance rights (`instance.*`); only for instance admins, within the\n\
        instance's agent policy."]
    #[serde(default)]
    #[schema(example = json!(["instance.system.view"]))]
    pub instance: Vec<String>,
    #[doc = "Account rights (`account.*`); `account.view` is always included."]
    #[serde(default)]
    #[schema(example = json!(["account.view", "account.keys.view"]))]
    pub account: Vec<String>,
    #[doc = "One entry per organization of the user; one left out is closed to the agent."]
    #[serde(default)]
    pub orgs: Vec<OrgGrantBody>,
    #[doc = "The rights in organizations the user joins later; `null`: none."]
    #[serde(default)]
    pub future_orgs: Option<FutureGrantBody>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An agent's rights in one organization.")]
pub struct OrgGrantBody {
    #[doc = "The organization."]
    pub org: OrgId,
    #[doc = "Whether the agent may act in it."]
    pub included: bool,
    #[doc = "Org rights from the rights catalogue."]
    #[serde(default)]
    #[schema(example = json!(["org.view", "usage.org.view"]))]
    pub rights: Vec<String>,
    #[doc = "Its projects."]
    #[serde(default)]
    pub projects: ProjectsGrantBody,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An agent's rights in the projects of one organization.")]
pub struct ProjectsGrantBody {
    #[doc = "Every project, including ones created later, with `rights`; `false`:\n\
        only the projects in `selected`."]
    pub all: bool,
    #[doc = "Project rights in every project, for `all`."]
    #[serde(default)]
    #[schema(example = json!(["project.view", "models.view"]))]
    pub rights: Vec<String>,
    #[doc = "Projects and their rights, when not `all`."]
    #[serde(default)]
    pub selected: Vec<ProjectGrantBody>,
}

impl Default for ProjectsGrantBody {
    fn default() -> ProjectsGrantBody {
        ProjectsGrantBody {
            all: true,
            rights: Vec::new(),
            selected: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An agent's rights in one project.")]
pub struct ProjectGrantBody {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "Project rights from the rights catalogue."]
    pub rights: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An agent's rights in organizations its user joins later.")]
pub struct FutureGrantBody {
    #[doc = "Org rights."]
    #[serde(default)]
    pub rights: Vec<String>,
    #[doc = "Project rights in every project of those organizations."]
    #[serde(default)]
    pub project_rights: Vec<String>,
}

fn strings(names: Vec<&'static str>) -> Vec<String> {
    names.into_iter().map(str::to_owned).collect()
}

impl GrantBody {
    pub fn of(g: &AgentGrant) -> GrantBody {
        GrantBody {
            preset: g.preset.as_str().to_owned(),
            customized: g.customized,
            instance: strings(g.instance.names()),
            account: strings(g.account.names()),
            orgs: g
                .orgs
                .iter()
                .map(|(org, access)| OrgGrantBody {
                    org: OrgId::from_uuid(*org),
                    included: access.is_some(),
                    rights: access
                        .as_ref()
                        .map_or_else(Vec::new, |a| strings(a.rights.names())),
                    projects: match access.as_ref().map(|a| &a.projects) {
                        None => ProjectsGrantBody::default(),
                        Some(Projects::All(r)) => ProjectsGrantBody {
                            all: true,
                            rights: strings(r.names()),
                            selected: Vec::new(),
                        },
                        Some(Projects::Selected(each)) => ProjectsGrantBody {
                            all: false,
                            rights: Vec::new(),
                            selected: each
                                .iter()
                                .map(|(p, r)| ProjectGrantBody {
                                    project: ProjectId::from_uuid(*p),
                                    rights: strings(r.names()),
                                })
                                .collect(),
                        },
                    },
                })
                .collect(),
            future_orgs: g.future.as_ref().map(|f| FutureGrantBody {
                rights: strings(f.rights.names()),
                project_rights: strings(f.projects.every_project().names()),
            }),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An agent connected through the MCP server.")]
pub struct AgentView {
    #[doc = "The agent."]
    pub id: AgentId,
    #[doc = "The name its user gave it."]
    #[schema(example = "Release helper")]
    pub name: String,
    #[doc = "The model family it reports with every tool call, one of the catalogue's\n\
        `models`; until its first call, guessed from its client's name, else `unknown`."]
    #[schema(example = "claude")]
    pub model: String,
    #[doc = "The name its client registered with."]
    #[schema(example = "codex-mcp-client")]
    pub client: Option<String>,
    #[doc = "Its rights."]
    pub grant: GrantBody,
    #[doc = "When it was authorized."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When it last used Kaeso; `null` if never. After 90 days unused it\n\
        stops working."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "When its sign-in ends; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
}

async fn view(state: &AppState, a: &Agent) -> ApiResult<AgentView> {
    let mut conn = state.pg.acquire().await?;
    let grant = AgentGrant::from_store(&agents::grant_of(&mut conn, a).await?);
    let client = match a.client_id {
        Some(id) => agents::get_client(&mut *conn, id).await?.map(|c| c.name),
        None => None,
    };
    Ok(AgentView {
        id: AgentId::from_uuid(a.id),
        name: a.name.clone(),
        model: a.model.clone(),
        client,
        grant: GrantBody::of(&grant),
        created_at: a.created_at,
        last_used_at: a.last_used_at,
        expires_at: a.expires_at,
    })
}

async fn own(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<Agent> {
    let id: AgentId = id.parse().map_err(|_| ApiError::not_found("agent"))?;
    agents::get(&state.pg, ctx.user_id()?.uuid(), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("agent"))
}

#[utoipa::path(
    get,
    path = "/me/agents",
    summary = "The user's agents, newest first.",
    tag = "agents",
    responses((status = 200, body = List<AgentView>))
)]
pub async fn list(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<List<AgentView>>> {
    let rows = agents::list(&state.pg, ctx.user_id()?.uuid()).await?;
    let mut out = Vec::with_capacity(rows.len());
    for a in &rows {
        out.push(view(&state, a).await?);
    }
    Ok(Json(List::complete(out)))
}

#[utoipa::path(
    get,
    path = "/me/agents/{agent}",
    summary = "One of the user's agents.",
    tag = "agents",
    params(("agent" = AgentId, Path)),
    responses((status = 200, body = AgentView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<AgentView>> {
    let a = own(&state, &ctx, &id).await?;
    Ok(Json(view(&state, &a).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to an agent; fields left out stay.")]
pub struct AgentUpdate {
    #[doc = "Its name, 1 to 100 characters."]
    pub name: Option<String>,
    #[doc = "A new lifetime from now: `7d`, `30d`, `90d` or `never`."]
    #[schema(example = "90d")]
    pub expiry: Option<String>,
}

#[utoipa::path(
    patch,
    path = "/me/agents/{agent}",
    summary = "Renames an agent or gives it a new lifetime.",
    tag = "agents",
    params(("agent" = AgentId, Path)),
    request_body = AgentUpdate,
    responses((status = 200, body = AgentView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Body(req): Body<AgentUpdate>,
) -> ApiResult<Json<AgentView>> {
    let user = ctx.user_id()?;
    let id: AgentId = id.parse().map_err(|_| ApiError::not_found("agent"))?;
    let mut tx = state.pg.begin().await?;
    let a = agents::lock(&mut tx, user.uuid(), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("agent"))?;
    let name = req
        .name
        .as_deref()
        .map(|n| validate_name("name", n))
        .transpose()?
        .unwrap_or_else(|| a.name.clone());
    let expires_at = match req.expiry.as_deref() {
        Some(e) => Expiry::parse(e)?.from(ctx.now),
        None => a.expires_at,
    };
    agents::update(
        &mut *tx,
        a.id,
        &agents::Details {
            name: &name,
            expires_at,
        },
    )
    .await?;
    ctx.audit(None, Action::AgentUpdate)
        .target("agent", id)
        .before(serde_json::json!({ "name": a.name, "expires_at": a.expires_at.map(|t| t.unix_timestamp()) }))
        .after(serde_json::json!({ "name": name, "expires_at": expires_at.map(|t| t.unix_timestamp()) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let a = own(&state, &ctx, &id.to_string()).await?;
    Ok(Json(view(&state, &a).await?))
}

async fn granting_for(
    state: &AppState,
    ctx: &Ctx,
) -> ApiResult<(Vec<service::HeldOrg>, bool, InstancePolicy)> {
    let session = ctx.session()?;
    let holdings = service::holdings(&state.pg, session.user_id).await?;
    let instance = InstancePolicy::from_store(&agents::instance_policy(&state.pg).await?);
    Ok((holdings, session.is_instance_admin, instance))
}

pub(crate) async fn validated(
    state: &AppState,
    ctx: &Ctx,
    before: Option<&AgentGrant>,
    body: &GrantBody,
) -> ApiResult<AgentGrant> {
    let (holdings, is_instance_admin, instance) = granting_for(state, ctx).await?;
    if !instance.allowed {
        return Err(ApiError::PermissionDenied(
            crate::auth::agents::Refusal::NotAllowed.message().into(),
        ));
    }
    let grant = service::validate(
        &Granting {
            holdings: &holdings,
            is_instance_admin,
            instance,
        },
        body,
    )?;
    if service::gains_sensitive(before, &grant) {
        ctx.require_recent_sign_in()?;
    }
    Ok(grant)
}

#[utoipa::path(
    put,
    path = "/me/agents/{agent}/rights",
    summary = "Replaces an agent's rights.",
    description = "Only rights the user holds, the instance's and each org's agent policy\n\
    allow, and agents may be given. Giving sensitive rights it didn't have\n\
    needs a sign-in within the last 15 minutes.",
    tag = "agents",
    params(("agent" = AgentId, Path)),
    request_body = GrantBody,
    responses((status = 200, body = AgentView))
)]
pub async fn set_rights(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Body(req): Body<GrantBody>,
) -> ApiResult<Json<AgentView>> {
    let user = ctx.user_id()?;
    let id: AgentId = id.parse().map_err(|_| ApiError::not_found("agent"))?;
    let mut tx = state.pg.begin().await?;
    let a = agents::lock(&mut tx, user.uuid(), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("agent"))?;
    let before = AgentGrant::from_store(&agents::grant_of(&mut tx, &a).await?);
    let after = validated(&state, &ctx, Some(&before), &req).await?;
    agents::replace_grant(&mut tx, a.id, &after.to_store()).await?;
    ctx.audit(None, Action::AgentRightsUpdate)
        .target("agent", id)
        .before(GrantBody::of(&before))
        .after(GrantBody::of(&after))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let a = own(&state, &ctx, &id.to_string()).await?;
    Ok(Json(view(&state, &a).await?))
}

#[utoipa::path(
    delete,
    path = "/me/agents/{agent}",
    summary = "Revokes an agent; its tokens stop working at once.",
    tag = "agents",
    params(("agent" = AgentId, Path)),
    responses((status = 204, description = "Revoked."))
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let user = ctx.user_id()?;
    let id: AgentId = id.parse().map_err(|_| ApiError::not_found("agent"))?;
    let mut tx = state.pg.begin().await?;
    let a = agents::lock(&mut tx, user.uuid(), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("agent"))?;
    agents::revoke(&mut tx, a.id, ctx.now).await?;
    ctx.audit(None, Action::AgentRevoke)
        .target("agent", id)
        .before(serde_json::json!({ "name": a.name, "model": a.model }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "One tool call of an agent.")]
pub struct AgentActivityView {
    #[doc = "The call."]
    pub id: AgentActivityId,
    #[doc = "When."]
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    #[doc = "The MCP tool it called."]
    #[schema(example = "get_orgs_org_usage")]
    pub tool: String,
    #[doc = "The organization it acted in, if any."]
    pub org_id: Option<OrgId>,
    #[doc = "The project it acted in, if any."]
    pub project_id: Option<ProjectId>,
    #[doc = "What it acted on: the ids the call named."]
    #[schema(example = "lim_01k7a3f8m2xq4v9c5r6t8w0yz1")]
    pub target: Option<String>,
    #[doc = "The HTTP status the call ended with."]
    #[schema(example = 200)]
    pub status: u16,
    #[doc = "`ok`, or the error type, such as `insufficient_rights`."]
    #[schema(example = "ok")]
    pub result: String,
    #[doc = "The agent's address."]
    pub ip: Option<String>,
}

#[utoipa::path(
    get,
    path = "/me/agents/{agent}/activity",
    summary = "An agent's tool calls, newest first; kept for 90 days.",
    tag = "agents",
    params(("agent" = AgentId, Path), PageParams),
    responses((status = 200, body = List<AgentActivityView>))
)]
pub async fn activity(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<List<AgentActivityView>>> {
    let a = own(&state, &ctx, &id).await?;
    let page = page.page()?;
    let rows = agents::activity(&state.pg, a.id, page.after, page.fetch()).await?;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |r| r.id,
        |r| AgentActivityView {
            id: AgentActivityId::from_uuid(r.id),
            at: r.at,
            tool: r.tool,
            org_id: r.org_id.map(OrgId::from_uuid),
            project_id: r.project_id.map(ProjectId::from_uuid),
            target: r.target,
            status: u16::try_from(r.status).unwrap_or_default(),
            result: r.result,
            ip: r.ip.map(|n| n.addr().to_string()),
        },
    )))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A preset for an agent's rights.")]
pub struct AgentPresetView {
    #[doc = "`observer`, `builder`, `operator` or `full`."]
    #[schema(example = "builder")]
    pub id: &'static str,
    #[doc = "The name shown in the console."]
    #[schema(example = "Builder")]
    pub name: &'static str,
    #[doc = "What it allows, as one sentence."]
    pub description: &'static str,
    #[doc = "Its org rights, before the user's own and the org's policy narrow them."]
    pub org: Vec<&'static str>,
    #[doc = "Its project rights."]
    pub project: Vec<&'static str>,
    #[doc = "Its account rights."]
    pub account: Vec<&'static str>,
    #[doc = "Its instance preset, for instance admins."]
    #[schema(example = "none")]
    pub instance: &'static str,
    #[doc = "The org rights an org whose policy allows at most this preset lets\n\
        agents have."]
    pub org_ceiling: Vec<&'static str>,
    #[doc = "The project rights such an org lets agents have."]
    pub project_ceiling: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A preset for an agent's instance rights.")]
pub struct InstancePresetView {
    #[doc = "`none`, `read`, `configure` or `admin`."]
    #[schema(example = "read")]
    pub id: &'static str,
    #[doc = "The name shown in the console."]
    #[schema(example = "Read")]
    pub name: &'static str,
    #[doc = "What it allows, as one sentence."]
    pub description: &'static str,
    #[doc = "Its instance rights."]
    pub rights: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Everything an agent can be given.")]
pub struct AgentCatalogue {
    #[doc = "Instance rights."]
    pub instance: ScopeCatalogue,
    #[doc = "Account rights."]
    pub account: ScopeCatalogue,
    #[doc = "Org rights."]
    pub org: ScopeCatalogue,
    #[doc = "Project rights."]
    pub project: ScopeCatalogue,
    #[doc = "The presets, in order."]
    pub presets: Vec<AgentPresetView>,
    #[doc = "The instance presets, in order."]
    pub instance_presets: Vec<InstancePresetView>,
    #[doc = "Org rights no agent can be given."]
    #[schema(example = json!(["org.ownership.transfer", "security.sso.manage", "security.sso.require"]))]
    pub never_grantable: Vec<&'static str>,
    #[doc = "The model families an agent can report, `unknown` last."]
    pub models: Vec<&'static str>,
}

fn right_view<R: Copy + Into<kaeso_core::rights::Right>>(
    scope: &'static str,
    owner_only: bool,
    needs: Vec<&'static str>,
    r: R,
) -> RightView {
    let right: kaeso_core::rights::Right = r.into();
    RightView {
        id: right.as_str(),
        scope,
        name: right.label(),
        description: match right {
            kaeso_core::rights::Right::Org(r) => r.description(),
            kaeso_core::rights::Right::Project(r) => r.description(),
            kaeso_core::rights::Right::Instance(r) => r.description(),
            kaeso_core::rights::Right::Account(r) => r.description(),
        },
        sensitive: kaeso_core::rights::needs_confirmation(right),
        recent_sign_in: right.needs_recent_sign_in(),
        cloud_only: match right {
            kaeso_core::rights::Right::Org(r) => r.cloud_only(),
            _ => false,
        },
        owner_only,
        needs,
    }
}

#[utoipa::path(
    get,
    path = "/me/agents/catalogue",
    summary = "The rights, presets and model families for agents.",
    description = "`sensitive` marks rights whose grant needs a sign-in within the last 15\n\
    minutes and a confirmation in the console.",
    tag = "agents",
    responses((status = 200, body = AgentCatalogue))
)]
pub async fn catalogue(_ctx: Ctx) -> Json<AgentCatalogue> {
    Json(AgentCatalogue {
        instance: group(InstanceRight::ALL, InstanceRight::category, |r| {
            right_view(
                "instance",
                false,
                r.needs().into_iter().map(InstanceRight::as_str).collect(),
                r,
            )
        }),
        account: group(AccountRight::ALL, AccountRight::category, |r| {
            right_view(
                "account",
                false,
                r.needs().into_iter().map(AccountRight::as_str).collect(),
                r,
            )
        }),
        org: group(OrgRight::ALL, OrgRight::category, |r| {
            right_view(
                "org",
                r.owner_only(),
                r.needs().into_iter().map(OrgRight::as_str).collect(),
                r,
            )
        }),
        project: group(ProjectRight::ALL, ProjectRight::category, |r| {
            right_view(
                "project",
                false,
                r.needs().into_iter().map(ProjectRight::as_str).collect(),
                r,
            )
        }),
        presets: AgentPreset::ALL
            .into_iter()
            .map(|p| AgentPresetView {
                id: p.as_str(),
                name: p.name(),
                description: p.description(),
                org: p.org_rights().names(),
                project: p.project_rights().names(),
                account: p.account_rights().names(),
                instance: p.instance().as_str(),
                org_ceiling: p.org_ceiling().names(),
                project_ceiling: p.project_ceiling().names(),
            })
            .collect(),
        instance_presets: InstancePreset::ALL
            .into_iter()
            .map(|p| InstancePresetView {
                id: p.as_str(),
                name: p.name(),
                description: p.description(),
                rights: p.rights().names(),
            })
            .collect(),
        never_grantable: kaeso_core::rights::OrgRights::all()
            .minus(grantable_org_rights())
            .names(),
        models: MODELS.to_vec(),
    })
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project the user could open to an agent.")]
pub struct GrantableProject {
    #[doc = "The project."]
    pub id: ProjectId,
    #[doc = "Its name."]
    pub name: String,
    #[doc = "Whether it is the org's default project."]
    pub is_default: bool,
    #[doc = "Its picture; `null` without one."]
    pub avatar_url: Option<String>,
    #[doc = "The user's rights in it, within the org's agent policy."]
    pub rights: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An organization the user could open to an agent.")]
pub struct GrantableOrg {
    #[doc = "The organization."]
    pub id: OrgId,
    #[doc = "Its name."]
    pub name: String,
    #[doc = "Its picture; `null` without one."]
    pub avatar_url: Option<String>,
    #[doc = "Whether it allows its members' agents."]
    pub allowed: bool,
    #[doc = "The most its policy lets agents have: `observer`, `builder`,\n\
        `operator` or `full`."]
    #[schema(example = "full")]
    pub max: &'static str,
    #[doc = "The org rights the user could give an agent here."]
    pub rights: Vec<&'static str>,
    #[doc = "The project rights the user holds in every project here."]
    pub every_project: Vec<&'static str>,
    #[doc = "The projects the user can open to an agent."]
    pub projects: Vec<GrantableProject>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What the user could give an agent now.")]
pub struct Grantable {
    #[doc = "The instance rights the user could give; empty unless an instance admin."]
    pub instance: Vec<&'static str>,
    #[doc = "Whether the instance allows agents."]
    pub allowed: bool,
    #[doc = "The most the instance lets agents have there: `none`, `read`,\n\
        `configure` or `admin`."]
    #[schema(example = "configure")]
    pub instance_max: &'static str,
    #[doc = "The account rights the user could give."]
    pub account: Vec<&'static str>,
    #[doc = "The user's organizations."]
    pub orgs: Vec<GrantableOrg>,
}

#[utoipa::path(
    get,
    path = "/me/agents/grantable",
    summary = "What the user could give an agent now, per organization and project.",
    tag = "agents",
    responses((status = 200, body = Grantable))
)]
pub async fn grantable(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<Grantable>> {
    let (holdings, is_admin, instance) = granting_for(&state, &ctx).await?;
    Ok(Json(Grantable {
        instance: if is_admin {
            instance.max.rights().names()
        } else {
            Vec::new()
        },
        allowed: instance.allowed,
        instance_max: instance.max.as_str(),
        account: kaeso_core::rights::AccountRights::all().names(),
        orgs: holdings
            .iter()
            .map(|o| {
                let ceiling = o.policy.max.project_ceiling();
                GrantableOrg {
                    id: OrgId::from_uuid(o.id),
                    name: o.name.clone(),
                    avatar_url: avatar_url(o.avatar.as_deref()),
                    allowed: o.policy.allowed,
                    max: o.policy.max.as_str(),
                    rights: o
                        .org_rights()
                        .intersect(grantable_org_rights())
                        .intersect(o.policy.max.org_ceiling())
                        .names(),
                    every_project: o.grant.every_project().intersect(ceiling).names(),
                    projects: o
                        .projects
                        .iter()
                        .map(|p| GrantableProject {
                            id: ProjectId::from_uuid(p.id),
                            name: p.name.clone(),
                            is_default: p.is_default,
                            avatar_url: avatar_url(p.avatar.as_deref()),
                            rights: p.rights.intersect(ceiling).names(),
                        })
                        .collect(),
                }
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(
    description = "A sign-in request of an agent, from the fragment of the console's consent page."
)]
pub struct AgentRequestRef {
    #[doc = "The request token from `#request=`."]
    #[schema(value_type = String, write_only)]
    pub request: secrecy::SecretString,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An agent asking to sign in.")]
pub struct AgentRequestView {
    #[doc = "The name its client registered with."]
    #[schema(example = "Claude Code")]
    pub client: String,
    #[doc = "Where the browser goes back to after the decision."]
    #[schema(example = "localhost")]
    pub redirect_host: String,
    #[doc = "When the request ends."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

async fn pending(state: &AppState, ctx: &Ctx, req: &AgentRequestRef) -> ApiResult<agents::Request> {
    use secrecy::ExposeSecret;
    agents::pending_request(
        &state.pg,
        &secret::hash(req.request.expose_secret()),
        ctx.now,
    )
    .await?
    .ok_or_else(|| {
        ApiError::NotFound(
            "This sign-in request has ended or was used; connect the agent again.".into(),
        )
    })
}

#[utoipa::path(
    post,
    path = "/me/agent-requests/view",
    summary = "An agent's sign-in request, for the consent page.",
    tag = "agents",
    request_body = AgentRequestRef,
    responses((status = 200, body = AgentRequestView))
)]
pub async fn request(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AgentRequestRef>,
) -> ApiResult<Json<AgentRequestView>> {
    ctx.session()?;
    let r = pending(&state, &ctx, &req).await?;
    Ok(Json(AgentRequestView {
        client: r.client_name,
        redirect_host: oauth::redirect_host(&r.redirect_uri),
        expires_at: r.expires_at,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The user's decision to let an agent sign in.")]
pub struct AgentApproval {
    #[doc = "The request token from `#request=`."]
    #[schema(value_type = String, write_only)]
    pub request: secrecy::SecretString,
    #[doc = "The agent's name, 1 to 100 characters."]
    #[schema(example = "Release helper")]
    pub name: String,
    #[doc = "How long its sign-in lasts: `7d`, `30d`, `90d` or `never`."]
    #[schema(example = "90d")]
    pub expiry: String,
    #[doc = "Its rights."]
    pub grant: GrantBody,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Where to send the browser after a decision.")]
pub struct AgentRedirect {
    #[doc = "The agent's address, with the authorization code or the refusal."]
    pub redirect_to: String,
    #[doc = "The new agent, when authorized."]
    pub agent: Option<AgentId>,
}

#[utoipa::path(
    post,
    path = "/me/agent-requests/approve",
    summary = "Lets an agent sign in with the chosen name, rights and lifetime.",
    description = "Only rights the user holds, the policies allow and agents may be given.\n\
    Sensitive rights need a sign-in within the last 15 minutes. The browser\n\
    goes on to `redirect_to`, where the agent exchanges the code for tokens.",
    tag = "agents",
    request_body = AgentApproval,
    responses((status = 200, body = AgentRedirect))
)]
pub async fn approve(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AgentApproval>,
) -> ApiResult<Json<AgentRedirect>> {
    let session = ctx.session()?.clone();
    let name = validate_name("name", &req.name)?;
    let expires_at = Expiry::parse(&req.expiry)?.from(ctx.now);
    let r = pending(
        &state,
        &ctx,
        &AgentRequestRef {
            request: req.request,
        },
    )
    .await?;
    let grant = validated(&state, &ctx, None, &req.grant).await?;
    let client = agents::get_client(&state.pg, r.client_id).await?;
    let names: Vec<&str> = client
        .iter()
        .flat_map(|c| [Some(c.name.as_str()), c.software_id.as_deref()])
        .flatten()
        .collect();
    let model = service::guess_model(&names);
    let id = AgentId::new();
    let mut tx = state.pg.begin().await?;
    if !agents::use_request(&mut *tx, r.id, ctx.now).await? {
        return Err(ApiError::NotFound(
            "This sign-in request has ended or was used; connect the agent again.".into(),
        ));
    }
    agents::insert_agent(
        &mut tx,
        &Agent {
            id: id.uuid(),
            user_id: session.user_id,
            client_id: Some(r.client_id),
            name: name.clone(),
            model: model.to_owned(),
            preset: String::new(),
            customized: false,
            instance_rights: Vec::new(),
            account_rights: Vec::new(),
            future_orgs: false,
            future_org_rights: Vec::new(),
            future_project_rights: Vec::new(),
            sign_in_method: session.method.clone(),
            second_factor: session.second_factor,
            oidc_org_id: session.oidc_org_id,
            created_at: ctx.now,
            last_used_at: None,
            expires_at,
            revoked_at: None,
        },
        &grant.to_store(),
    )
    .await?;
    let code = oauth::issue_code(&mut tx, id.uuid(), &r, ctx.now).await?;
    ctx.audit(None, Action::AgentAuthorize)
        .target("agent", id)
        .after(serde_json::json!({
            "name": name,
            "model": model,
            "client": r.client_name,
            "expires_at": expires_at.map(|t| t.unix_timestamp()),
            "grant": GrantBody::of(&grant),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(AgentRedirect {
        redirect_to: oauth::redirect_with_code(&state, &r, &code),
        agent: Some(id),
    }))
}

#[utoipa::path(
    post,
    path = "/me/agent-requests/deny",
    summary = "Refuses an agent's sign-in request.",
    tag = "agents",
    request_body = AgentRequestRef,
    responses((status = 200, body = AgentRedirect))
)]
pub async fn deny(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AgentRequestRef>,
) -> ApiResult<Json<AgentRedirect>> {
    ctx.session()?;
    let r = pending(&state, &ctx, &req).await?;
    agents::use_request(&state.pg, r.id, ctx.now).await?;
    Ok(Json(AgentRedirect {
        redirect_to: oauth::redirect_with_error(
            &state,
            &r.redirect_uri,
            r.state.as_deref(),
            "access_denied",
        ),
        agent: None,
    }))
}
