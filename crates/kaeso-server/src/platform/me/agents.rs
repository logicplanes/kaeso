use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{AgentActivityId, AgentId, McpServerId, OrgId, ProjectId};
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
use crate::services::agents::{self as service, Granting, MODELS};
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
    #[doc = "One entry per organization of the user; one left out is closed to the agent.\n\
        Answers list an organization the agent reaches through `future_orgs` as\n\
        included, with those rights as far as the user and the org's policy allow."]
    #[serde(default)]
    pub orgs: Vec<OrgGrantBody>,
    #[doc = "The rights in organizations the user joins later; `null`: none."]
    #[serde(default)]
    pub future_orgs: Option<FutureGrantBody>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
#[schema(description = "An agent's rights in one project.")]
pub struct ProjectGrantBody {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "Project rights from the rights catalogue."]
    pub rights: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
        `models`; `unknown` until its first call."]
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
}

async fn view(state: &AppState, a: &Agent, held: &[service::HeldOrg]) -> ApiResult<AgentView> {
    let mut conn = state.pg.acquire().await?;
    let grant = service::as_seen(
        &AgentGrant::from_store(&agents::grant_of(&mut conn, a).await?),
        held,
    );
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
    let user = ctx.user_id()?.uuid();
    let rows = agents::list(&state.pg, user).await?;
    let held = service::holdings(&state.pg, user).await?;
    let mut out = Vec::with_capacity(rows.len());
    for a in &rows {
        out.push(view(&state, a, &held).await?);
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
    Ok(Json(
        view(&state, &a, &service::holdings(&state.pg, a.user_id).await?).await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new name for an agent.")]
pub struct AgentUpdate {
    #[doc = "Its name, 1 to 100 characters."]
    #[schema(example = "Release helper")]
    pub name: String,
}

#[utoipa::path(
    patch,
    path = "/me/agents/{agent}",
    summary = "Renames an agent.",
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
    let name = validate_name("name", &req.name)?;
    agents::rename(&mut *tx, a.id, &name).await?;
    ctx.audit(None, Action::AgentUpdate)
        .target("agent", id)
        .before(serde_json::json!({ "name": a.name }))
        .after(serde_json::json!({ "name": name }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let a = own(&state, &ctx, &id.to_string()).await?;
    Ok(Json(
        view(&state, &a, &service::holdings(&state.pg, a.user_id).await?).await?,
    ))
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
    let held = service::holdings(&state.pg, a.user_id).await?;
    ctx.audit(None, Action::AgentRightsUpdate)
        .target("agent", id)
        .before(GrantBody::of(&service::as_seen(&before, &held)))
        .after(GrantBody::of(&after))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let a = own(&state, &ctx, &id.to_string()).await?;
    Ok(Json(view(&state, &a, &held).await?))
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
#[serde(deny_unknown_fields)]
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
    #[doc = "For a client asking to use a project's MCP portal: the portal and its tools; `null`\n\
        for an agent of Kaeso's own MCP server."]
    pub portal: Option<PortalRequestView>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A client asking to call a project's tools as the signed-in person.")]
pub struct PortalRequestView {
    #[doc = "The organization."]
    pub org: OrgId,
    #[doc = "Its name."]
    pub org_name: String,
    #[doc = "The project whose portal the client asks for."]
    pub project: ProjectId,
    #[doc = "Its name."]
    pub project_name: String,
    #[doc = "The servers whose tools the client could call: those the project offers and you may\n\
        use, each with its tools' names on the portal."]
    pub servers: Vec<PortalServerView>,
    #[doc = "For a client known by its metadata document: the host it is published on, which\n\
        vouches for the client's name."]
    #[schema(example = "claude.ai")]
    pub client_host: Option<String>,
    #[doc = "Whether the client receives the sign-in on this computer, as a desktop app does."]
    pub local: bool,
    #[doc = "Servers of the project where each person signs in and you haven't yet: their tools\n\
        fail for you until you connect."]
    pub connect: Vec<PortalConnectView>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "One of the project's servers with the tools the client could call there.")]
pub struct PortalServerView {
    #[doc = "The server."]
    pub server: McpServerId,
    #[doc = "Its handle, the first part of each tool's name on the portal."]
    #[schema(example = "x")]
    pub handle: String,
    #[doc = "Its name."]
    #[schema(example = "X")]
    pub name: String,
    #[doc = "The tools' names on the portal."]
    #[schema(example = json!(["x__create_post"]))]
    pub tools: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "A server where you sign in with your own account before its tools work for you."
)]
pub struct PortalConnectView {
    #[doc = "The server."]
    pub server: McpServerId,
    #[doc = "Its name."]
    #[schema(example = "X")]
    pub name: String,
    #[doc = "How you sign in: `header` or `oauth`."]
    pub auth_method: crate::platform::orgs::mcp_servers::McpAuthMethod,
}

async fn to_connect(
    state: &AppState,
    user: uuid::Uuid,
    project: uuid::Uuid,
) -> ApiResult<Vec<PortalConnectView>> {
    let mine = kaeso_store::pg::mcp_connections::of_user(&state.pg, user).await?;
    Ok(kaeso_store::pg::mcp_servers::of_project(&state.pg, project)
        .await?
        .into_iter()
        .filter(|s| s.credential_mode == "per_user" && s.auth_method != "none")
        .filter(|s| {
            !mine
                .iter()
                .any(|c| c.connection.server_id == s.id && c.connection.auth_error.is_none())
        })
        .map(|s| PortalConnectView {
            server: McpServerId::from_uuid(s.id),
            auth_method: if s.auth_method == "header" {
                crate::platform::orgs::mcp_servers::McpAuthMethod::Header
            } else {
                crate::platform::orgs::mcp_servers::McpAuthMethod::Oauth
            },
            name: s.name,
        })
        .collect())
}

fn portal_of(state: &AppState, r: &agents::Request) -> Option<uuid::Uuid> {
    match r
        .resource
        .as_deref()
        .and_then(|t| oauth::target_of(state, t))
    {
        Some(oauth::Target::Portal(project)) => Some(project),
        _ => None,
    }
}

async fn portal_view(
    state: &AppState,
    session: &kaeso_store::pg::sessions::SessionRow,
    r: &agents::Request,
    project: uuid::Uuid,
) -> ApiResult<PortalRequestView> {
    let user = session.user_id;
    let mut conn = state.pg.acquire().await?;
    let row = sqlx::query_scalar!("SELECT org_id FROM projects WHERE id = $1", project)
        .fetch_optional(&mut *conn)
        .await
        .map_err(kaeso_store::StoreError::from)?
        .ok_or_else(|| ApiError::not_found("project"))?;
    let found = kaeso_store::pg::members::access_context(&mut conn, row, Some(user), Some(project))
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    let (Some(org_grant), Some(p)) = (found.org_grant.as_ref(), found.project.as_ref()) else {
        return Err(ApiError::not_found("project"));
    };
    if p.archived_at.is_some() {
        return Err(ApiError::not_found("project"));
    }
    if found.org.state != "active" {
        return Err(ApiError::OrgScheduledForDeletion);
    }
    crate::platform::access::require_policy(
        state,
        session,
        crate::platform::access::Area::of_org(&found.org),
    )
    .await?;
    let org_grant = crate::platform::grants::org_from_store(org_grant)?;
    let project_grant = found
        .project_grant
        .as_ref()
        .map(crate::platform::grants::project_from_store)
        .transpose()?;
    let rights =
        kaeso_core::rights::effective_project_rights(Some(&org_grant), project_grant.as_ref());
    if !rights.contains(ProjectRight::ToolsUse) {
        return Err(crate::platform::access::denied(
            kaeso_core::rights::Right::Project(ProjectRight::ToolsUse),
            "project",
        ));
    }
    let snapshot = crate::portal::snapshot::get(state, project).await?;
    let mut servers: Vec<PortalServerView> = snapshot
        .servers
        .iter()
        .map(|s| PortalServerView {
            server: McpServerId::from_uuid(s.id),
            handle: s.handle.clone(),
            name: s.name.clone(),
            tools: Vec::new(),
        })
        .collect();
    for tool in snapshot
        .offered()
        .filter(|t| snapshot.access.allows(Some(user), None, None, t.id))
    {
        if let Some(server) = servers.get_mut(tool.server) {
            server.tools.push(tool.exposed.clone());
        }
    }
    servers.retain(|s| !s.tools.is_empty());
    let client_host = agents::metadata_url(&mut *conn, r.client_id)
        .await?
        .and_then(|u| url::Url::parse(&u).ok())
        .and_then(|u| u.host_str().map(str::to_owned));
    Ok(PortalRequestView {
        org: OrgId::from_uuid(found.org.id),
        org_name: found.org.name.clone(),
        project: ProjectId::from_uuid(project),
        project_name: p.name.clone(),
        servers,
        client_host,
        local: oauth::redirects_locally(&r.redirect_uri),
        connect: to_connect(state, user, project).await?,
    })
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
    let session = ctx.session()?;
    let r = pending(&state, &ctx, &req).await?;
    let portal = match portal_of(&state, &r) {
        Some(project) => Some(portal_view(&state, session, &r, project).await?),
        None => None,
    };
    Ok(Json(AgentRequestView {
        redirect_host: oauth::redirect_host(&r.redirect_uri),
        client: r.client_name,
        expires_at: r.expires_at,
        portal,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The user's decision to let an agent sign in.")]
pub struct AgentApproval {
    #[doc = "The request token from `#request=`."]
    #[schema(value_type = String, write_only)]
    pub request: secrecy::SecretString,
    #[doc = "The agent's name, 1 to 100 characters."]
    #[schema(example = "Release helper")]
    pub name: String,
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
    summary = "Lets an agent sign in with the chosen name and rights.",
    description = "Only rights the user holds, the policies allow and agents may be given.\n\
    Sensitive rights need a sign-in within the last 15 minutes. The browser\n\
    goes on to `redirect_to`, where the agent exchanges the code for tokens. The\n\
    agent keeps working until it is revoked or goes 90 days unused.",
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
    let r = pending(
        &state,
        &ctx,
        &AgentRequestRef {
            request: req.request,
        },
    )
    .await?;
    if portal_of(&state, &r).is_some() {
        return Err(ApiError::Conflict(
            "This sign-in is for a project's MCP portal; reload the page.".into(),
        ));
    }
    let grant = validated(&state, &ctx, None, &req.grant).await?;
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
            model: "unknown".to_owned(),
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
            "client": r.client_name,
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
    path = "/me/agent-requests/approve-portal",
    summary = "Lets a client call a project's tools through its MCP portal as you.",
    description = "The client gets your access to the project's tools until you or a project admin\n\
    disconnect it. Its token is good only at this portal. The browser goes on to `redirect_to`,\n\
    where the client exchanges the code for tokens.",
    tag = "agents",
    request_body = AgentRequestRef,
    responses((status = 200, body = AgentRedirect))
)]
pub async fn approve_portal(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AgentRequestRef>,
) -> ApiResult<Json<AgentRedirect>> {
    let session = ctx.session()?;
    let user = session.user_id;
    let r = pending(&state, &ctx, &req).await?;
    let Some(project) = portal_of(&state, &r) else {
        return Err(ApiError::Conflict(
            "This sign-in is for an agent, not a project's MCP portal; reload the page.".into(),
        ));
    };
    let view = portal_view(&state, session, &r, project).await?;
    let grant = uuid::Uuid::now_v7();
    let mut tx = state.pg.begin().await?;
    if !agents::use_request(&mut *tx, r.id, ctx.now).await? {
        return Err(ApiError::NotFound(
            "This sign-in request has ended or was used; connect the agent again.".into(),
        ));
    }
    kaeso_store::pg::portal_grants::insert(
        &mut *tx,
        &kaeso_store::pg::portal_grants::NewGrant {
            id: grant,
            user_id: user,
            org_id: view.org.uuid(),
            project_id: project,
            client_id: r.client_id,
            redirect_uri: &r.redirect_uri,
            sign_in_method: &session.method,
            second_factor: session.second_factor,
            oidc_org_id: session.oidc_org_id,
            now: ctx.now,
        },
    )
    .await?;
    let code = oauth::issue_portal_code(
        &mut tx,
        grant,
        &r,
        &oauth::portal_resource(&state, project),
        ctx.now,
    )
    .await?;
    ctx.audit(Some(view.org), Action::PortalGrantCreate)
        .project(view.project)
        .target(
            "portal_grant",
            kaeso_core::id::PortalGrantId::from_uuid(grant),
        )
        .after(serde_json::json!({
            "client": r.client_name,
            "client_host": view.client_host,
            "tools": view.servers.iter().flat_map(|s| s.tools.iter()).collect::<Vec<_>>(),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(AgentRedirect {
        redirect_to: oauth::redirect_with_code(&state, &r, &code),
        agent: None,
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
