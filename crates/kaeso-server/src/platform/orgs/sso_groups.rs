use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use kaeso_core::id::{OrgId, ProjectId, RoleId};
use kaeso_store::pg::oidc;
use kaeso_store::pg::sso_groups::{self as store, Rule, RuleProject, Settings};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{RoleBook, check_org, check_project, org_to_store, project_to_store};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::services::sso_groups::{ADD, SYNC};
use crate::state::AppState;

pub const MAX_RULES: usize = 200;
const MAX_SCOPES: usize = 10;

const NO_SSO: &str = "Connect an identity provider first; group rules apply to its sign-ins.";
const NO_OWNERS: &str = "A rule can't make owners; choose another role.";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "A project a rule makes a person a member of.")]
pub struct GroupRuleProject {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "The project role: `admin`, `developer`, `viewer` or a custom project role's id."]
    #[schema(example = "developer")]
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(
    description = "A rule: people in a group get an organization role and project memberships."
)]
pub struct GroupRule {
    #[doc = "The group's value in the claim, compared exactly."]
    #[schema(example = "kaeso-admins")]
    pub group: String,
    #[doc = "The organization role the first matching rule gives: a built-in role other than\n\
        `owner`, or a custom role's id. `null` leaves the role to other rules or the default."]
    #[schema(example = "admin")]
    pub role: Option<String>,
    #[doc = "Projects the person joins, each with a role. Every matching rule adds its own."]
    #[serde(default)]
    pub projects: Vec<GroupRuleProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Roles and projects from the identity provider's groups.")]
pub struct SsoGroups {
    #[doc = "The ID token claim that lists the person's groups."]
    #[schema(example = "groups")]
    pub claim: String,
    #[doc = "Extra scopes Kaeso asks the provider for, such as `groups`."]
    pub scopes: Vec<String>,
    #[doc = "`add` never takes anything away; `sync` also removes what a rule gave once\n\
        no rule gives it."]
    #[schema(example = "add")]
    pub mode: String,
    #[doc = "The rules in order; the first match sets the organization role."]
    pub rules: Vec<GroupRule>,
    #[doc = "When a sign-in last came without the claim, which changed nothing."]
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub claim_missing_at: Option<OffsetDateTime>,
    #[doc = "The groups your own last sign-in through the provider carried, when it had any."]
    pub your_last_groups: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The organization's group claim, mode and rules.")]
pub struct SsoGroupsUpdate {
    #[doc = "The ID token claim that lists the person's groups."]
    #[serde(default = "default_claim")]
    pub claim: String,
    #[doc = "Extra scopes to ask the provider for."]
    #[serde(default)]
    pub scopes: Vec<String>,
    #[doc = "`add` or `sync`."]
    #[serde(default = "default_mode")]
    pub mode: String,
    #[doc = "The rules in order, at most 200."]
    #[serde(default)]
    pub rules: Vec<GroupRule>,
}

fn default_claim() -> String {
    "groups".to_owned()
}

fn default_mode() -> String {
    ADD.to_owned()
}

fn role_string(preset: &str, role_id: Option<uuid::Uuid>) -> String {
    match role_id {
        Some(id) => RoleId::from_uuid(id).to_string(),
        None => preset.to_owned(),
    }
}

async fn view(state: &AppState, ctx: &Ctx, org: uuid::Uuid) -> ApiResult<SsoGroups> {
    let settings = store::settings(&state.pg, org).await?;
    let rules = store::rules(&state.pg, org).await?;
    let mine = match ctx.session() {
        Ok(s) => store::last_groups(&state.pg, org, s.user_id).await?,
        Err(_) => None,
    };
    Ok(SsoGroups {
        claim: settings
            .as_ref()
            .map_or_else(default_claim, |s| s.claim.clone()),
        scopes: settings
            .as_ref()
            .map(|s| s.scopes.clone())
            .unwrap_or_default(),
        mode: settings
            .as_ref()
            .map_or_else(default_mode, |s| s.mode.clone()),
        claim_missing_at: settings.as_ref().and_then(|s| s.claim_missing_at),
        rules: rules
            .iter()
            .map(|r| GroupRule {
                group: r.group_value.clone(),
                role: r
                    .org_preset
                    .as_deref()
                    .map(|p| role_string(p, r.org_role_id)),
                projects: r
                    .projects
                    .iter()
                    .map(|p| GroupRuleProject {
                        project: ProjectId::from_uuid(p.project_id),
                        role: role_string(&p.preset, p.role_id),
                    })
                    .collect(),
            })
            .collect(),
        your_last_groups: mine,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/security/sso/groups",
    summary = "The organization's roles and projects from its identity provider's groups.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = SsoGroups))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<SsoGroups>> {
    let org = ctx.org()?.org.id;
    Ok(Json(view(&state, &ctx, org).await?))
}

fn token_like(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':' | b'/'))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/sso/groups",
    summary = "Sets the group claim, the mode and the rules. A rule gives only roles whose\n\
    rights you hold, and never Owner.",
    description = "The rules apply at each sign-in through the organization's provider, to the \
    accounts it manages. A token without the claim changes nothing.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = SsoGroupsUpdate,
    responses((status = 200, body = SsoGroups))
)]
pub async fn put(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    Body(req): Body<SsoGroupsUpdate>,
) -> ApiResult<Json<SsoGroups>> {
    let scope = ctx.org()?;
    let org = scope.org.id;
    if oidc::get(&state.pg, org).await?.is_none() {
        return Err(ApiError::Conflict(NO_SSO.to_owned()));
    }
    let claim = req.claim.trim().to_owned();
    if !token_like(&claim) {
        return Err(ApiError::invalid(
            "claim",
            "Name the claim with letters, digits and `_-.:/`, at most 100.",
        ));
    }
    if req.scopes.len() > MAX_SCOPES || !req.scopes.iter().all(|s| token_like(s)) {
        return Err(ApiError::invalid(
            "scopes",
            "Give at most 10 scopes of letters, digits and `_-.:/`.",
        ));
    }
    if req.mode != ADD && req.mode != SYNC {
        return Err(ApiError::invalid("mode", "`mode` is `add` or `sync`."));
    }
    if req.rules.len() > MAX_RULES {
        return Err(ApiError::invalid("rules", "Keep to at most 200 rules."));
    }
    let book = RoleBook::load(&state.pg, org).await?;
    let open = store::active_projects(&state.pg, org).await?;
    let mut rules = Vec::with_capacity(req.rules.len());
    let mut seen = BTreeSet::new();
    for r in &req.rules {
        let group = r.group.trim().to_owned();
        if group.is_empty() || group.chars().count() > 256 {
            return Err(ApiError::invalid(
                "rules",
                "Name each rule's group, at most 256 characters.",
            ));
        }
        if !seen.insert(group.clone()) {
            return Err(ApiError::invalid(
                "rules",
                format!("The group `{group}` has two rules."),
            ));
        }
        let org_role = match r.role.as_deref() {
            None => None,
            Some(role) => {
                let (grant, _) = book.org_role(role, "rules")?;
                if grant.is_owner() {
                    return Err(ApiError::invalid("rules", NO_OWNERS));
                }
                check_org(scope, None, &grant)?;
                Some(org_to_store(&grant))
            }
        };
        let mut projects = Vec::with_capacity(r.projects.len());
        let mut in_rule = BTreeSet::new();
        for p in &r.projects {
            let id = p.project.uuid();
            if !open.contains(&id) || !in_rule.insert(id) {
                return Err(ApiError::invalid(
                    "rules",
                    format!(
                        "`{project}` isn't an open project of the organization, or is named twice.",
                        project = p.project
                    ),
                ));
            }
            let (grant, _) = book.project_role(&p.role, "rules")?;
            check_project(scope.every_project, None, &grant)?;
            let stored = project_to_store(&grant);
            projects.push(RuleProject {
                project_id: id,
                preset: stored.preset,
                role_id: stored.role_id,
            });
        }
        rules.push(Rule {
            group_value: group,
            org_preset: org_role.as_ref().map(|g| g.preset.clone()),
            org_role_id: org_role.and_then(|g| g.role_id),
            projects,
        });
    }
    let mut tx = state.pg.begin().await?;
    store::put(
        &mut tx,
        org,
        &Settings {
            claim: claim.clone(),
            scopes: req.scopes.clone(),
            mode: req.mode.clone(),
            claim_missing_at: None,
        },
        &rules,
    )
    .await?;
    ctx.audit(Some(OrgId::from_uuid(org)), Action::SecuritySsoGroupsUpdate)
        .target("org", OrgId::from_uuid(org))
        .after(serde_json::json!({ "claim": claim, "mode": req.mode, "rules": rules.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(view(&state, &ctx, org).await?))
}
