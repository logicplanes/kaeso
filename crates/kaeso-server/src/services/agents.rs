use std::collections::{BTreeMap, BTreeSet};

use kaeso_core::rights::{
    AccountRight, AccountRights, AgentPreset, GrantExceedsHolder, InstanceRight, InstanceRights,
    MissingDependency, OrgGrant, OrgRight, OrgRights, ProjectRight, ProjectRights, Right,
    agent_org_rights, agent_project_rights, effective_project_rights, grantable_org_rights,
    needs_confirmation,
};
use kaeso_store::pg::{agents, members};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::auth::agents::{
    AgentCaller, AgentGrant, InstancePolicy, OrgAccess, OrgPolicy, Projects,
};
use crate::platform::access::{Area, managed_by, policy_gap};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{org_from_store, project_from_store};
use crate::platform::me::agents::{GrantBody, OrgGrantBody};
use crate::state::AppState;

pub const MODELS: &[&str] = &[
    "claude", "gpt", "gemini", "gemma", "grok", "llama", "mistral", "deepseek", "qwen", "kimi",
    "glm", "minimax", "command", "nova", "phi", "granite", "jamba", "ernie", "hunyuan", "doubao",
    "yi", "nemotron", "olmo", "sonar", "reka", "unknown",
];

const CLIENT_MODELS: &[(&str, &str)] = &[
    ("claude", "claude"),
    ("codex", "gpt"),
    ("openai", "gpt"),
    ("chatgpt", "gpt"),
    ("gpt", "gpt"),
    ("gemini", "gemini"),
    ("grok", "grok"),
    ("mistral", "mistral"),
    ("deepseek", "deepseek"),
    ("qwen", "qwen"),
    ("kimi", "kimi"),
];

pub fn guess_model(names: &[&str]) -> &'static str {
    names
        .iter()
        .map(|n| n.to_lowercase())
        .find_map(|n| {
            CLIENT_MODELS
                .iter()
                .find(|(part, _)| n.contains(part))
                .map(|(_, model)| *model)
        })
        .unwrap_or("unknown")
}

pub fn known_model(model: &str) -> Option<&'static str> {
    MODELS.iter().copied().find(|m| *m == model)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    Week,
    Month,
    Quarter,
    Never,
}

impl Expiry {
    pub fn parse(s: &str) -> ApiResult<Expiry> {
        match s {
            "7d" => Ok(Expiry::Week),
            "30d" => Ok(Expiry::Month),
            "90d" => Ok(Expiry::Quarter),
            "never" => Ok(Expiry::Never),
            _ => Err(ApiError::invalid(
                "expiry",
                "`expiry` must be `7d`, `30d`, `90d` or `never`.",
            )),
        }
    }

    pub fn from(self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        match self {
            Expiry::Week => Some(now + Duration::days(7)),
            Expiry::Month => Some(now + Duration::days(30)),
            Expiry::Quarter => Some(now + Duration::days(90)),
            Expiry::Never => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HeldProject {
    pub id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub avatar: Option<String>,
    pub rights: ProjectRights,
}

#[derive(Clone, Debug)]
pub struct HeldOrg {
    pub id: Uuid,
    pub name: String,
    pub avatar: Option<String>,
    pub state: String,
    pub area: Area,
    pub grant: OrgGrant,
    pub policy: OrgPolicy,
    pub projects: Vec<HeldProject>,
}

impl HeldOrg {
    pub fn org_rights(&self) -> OrgRights {
        self.grant.effective()
    }

    pub fn project_rights(&self) -> ProjectRights {
        self.projects
            .iter()
            .fold(self.grant.every_project(), |acc, p| acc.union(p.rights))
    }
}

pub async fn holdings(pg: &PgPool, user: Uuid) -> ApiResult<Vec<HeldOrg>> {
    let orgs = members::orgs_of_user(pg, user).await?;
    let ids: Vec<Uuid> = orgs.iter().map(|o| o.org_id).collect();
    let policies: BTreeMap<Uuid, OrgPolicy> = agents::org_policies(pg, &ids)
        .await?
        .into_iter()
        .map(|(id, p)| (id, OrgPolicy::from_store(&p)))
        .collect();
    let mut own: BTreeMap<Uuid, members::Grant> = members::projects_of_user(pg, user)
        .await?
        .into_iter()
        .map(|p| (p.project_id, p.grant()))
        .collect();
    let mut projects: BTreeMap<Uuid, Vec<agents::OrgProject>> = BTreeMap::new();
    for p in agents::active_projects(pg, &ids).await? {
        projects.entry(p.org_id).or_default().push(p);
    }
    let mut out = Vec::with_capacity(orgs.len());
    for o in orgs {
        let grant = org_from_store(&o.grant())?;
        let mut held = Vec::new();
        for p in projects.remove(&o.org_id).unwrap_or_default() {
            let project_grant = own
                .remove(&p.id)
                .as_ref()
                .map(project_from_store)
                .transpose()?;
            let rights = effective_project_rights(Some(&grant), project_grant.as_ref());
            if !rights.is_empty() {
                held.push(HeldProject {
                    id: p.id,
                    name: p.name,
                    is_default: p.is_default,
                    avatar: p.avatar,
                    rights,
                });
            }
        }
        out.push(HeldOrg {
            id: o.org_id,
            name: o.org_name,
            avatar: o.org_avatar,
            state: o.org_state,
            area: Area::Org {
                id: o.org_id,
                require_sso: o.require_sso,
                require_2fa: o.require_2fa,
            },
            grant,
            policy: policies.get(&o.org_id).copied().unwrap_or(OrgPolicy {
                allowed: false,
                max: AgentPreset::Observer,
            }),
            projects: held,
        });
    }
    Ok(out)
}

pub struct Granting<'a> {
    pub holdings: &'a [HeldOrg],
    pub is_instance_admin: bool,
    pub instance: InstancePolicy,
}

fn unknown(param: &str, name: &str) -> ApiError {
    ApiError::invalid(
        param,
        format!("`{name}` is not a right an agent can be given."),
    )
}

fn parsed<S: FromIterator<R>, R: std::str::FromStr>(param: &str, names: &[String]) -> ApiResult<S> {
    names
        .iter()
        .map(|n| n.parse::<R>().map_err(|_| unknown(param, n)))
        .collect()
}

fn need_of<R: Copy>(
    rights: impl Iterator<Item = R>,
    contains: impl Fn(R) -> bool,
    needs: impl Fn(R) -> Vec<R>,
) -> Option<(R, R)> {
    rights
        .flat_map(|r| needs(r).into_iter().map(move |n| (r, n)))
        .find(|(_, n)| !contains(*n))
}

fn needs_met<R: Copy + Into<Right>>(
    param: &str,
    rights: impl Iterator<Item = R>,
    contains: impl Fn(R) -> bool,
    needs: impl Fn(R) -> Vec<R>,
) -> ApiResult<()> {
    match need_of(rights, contains, needs) {
        None => Ok(()),
        Some((right, need)) => Err(ApiError::invalid(
            param,
            MissingDependency::Gained {
                right: right.into().as_str(),
                needs: need.into().as_str(),
            }
            .to_string(),
        )),
    }
}

fn exceeds(param: &str, missing: Vec<&'static str>) -> ApiResult<()> {
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ApiError::invalid(
            param,
            GrantExceedsHolder { missing }.to_string(),
        ))
    }
}

fn beyond_policy(param: &str, first: Option<&'static str>) -> ApiResult<()> {
    match first {
        None => Ok(()),
        Some(right) => Err(ApiError::invalid(
            param,
            format!("The agent policy here doesn't allow `{right}`; leave it out."),
        )),
    }
}

fn project_rights(
    param: &str,
    names: &[String],
    ceiling: ProjectRights,
    held: ProjectRights,
) -> ApiResult<ProjectRights> {
    let rights: ProjectRights = parsed(param, names)?;
    beyond_policy(param, rights.minus(ceiling).names().first().copied())?;
    exceeds(param, rights.minus(held).names())?;
    needs_met(
        param,
        rights.iter(),
        |r| rights.contains(r),
        ProjectRight::needs,
    )?;
    Ok(rights)
}

fn org_access(held: &HeldOrg, body: &OrgGrantBody) -> ApiResult<OrgAccess> {
    if !held.policy.allowed {
        return Err(ApiError::invalid(
            "orgs",
            "An organization you chose doesn't allow agents; leave it out.",
        ));
    }
    let rights: OrgRights = parsed("orgs", &body.rights)?;
    if let Some(never) = rights.minus(grantable_org_rights()).names().first() {
        return Err(ApiError::invalid(
            "orgs",
            format!("`{never}` can't be given to agents."),
        ));
    }
    beyond_policy(
        "orgs",
        rights
            .minus(held.policy.max.org_ceiling())
            .names()
            .first()
            .copied(),
    )?;
    exceeds("orgs", rights.minus(held.org_rights()).names())?;
    needs_met(
        "orgs",
        rights.iter(),
        |r| rights.contains(r),
        OrgRight::needs,
    )?;
    let ceiling = held.policy.max.project_ceiling();
    let projects = if body.projects.all {
        Projects::All(project_rights(
            "orgs",
            &body.projects.rights,
            ceiling,
            held.project_rights(),
        )?)
    } else {
        let mut each = BTreeMap::new();
        for p in &body.projects.selected {
            let project = held
                .projects
                .iter()
                .find(|h| h.id == p.project.uuid())
                .ok_or_else(|| ApiError::not_found("project"))?;
            let rights = project_rights("orgs", &p.rights, ceiling, project.rights)?;
            if each.insert(project.id, rights).is_some() {
                return Err(ApiError::invalid(
                    "orgs",
                    "Each project can be listed only once.",
                ));
            }
        }
        Projects::Selected(each)
    };
    Ok(OrgAccess { rights, projects })
}

pub fn validate(g: &Granting<'_>, body: &GrantBody) -> ApiResult<AgentGrant> {
    let preset: AgentPreset = body.preset.parse().map_err(|_| {
        ApiError::invalid(
            "preset",
            "`preset` must be `observer`, `builder`, `operator` or `full`.",
        )
    })?;
    let instance: InstanceRights = parsed("instance", &body.instance)?;
    if !instance.is_empty() && !g.is_instance_admin {
        return Err(ApiError::invalid(
            "instance",
            "Only instance admins can give an agent instance rights.",
        ));
    }
    beyond_policy(
        "instance",
        instance
            .minus(g.instance.max.rights())
            .names()
            .first()
            .copied(),
    )?;
    needs_met(
        "instance",
        instance.iter(),
        |r| instance.contains(r),
        InstanceRight::needs,
    )?;
    let account: AccountRights = parsed::<AccountRights, AccountRight>("account", &body.account)?
        .with(AccountRight::AccountView);
    needs_met(
        "account",
        account.iter(),
        |r| account.contains(r),
        AccountRight::needs,
    )?;
    let mut orgs = BTreeMap::new();
    for o in &body.orgs {
        let held = g
            .holdings
            .iter()
            .find(|h| h.id == o.org.uuid())
            .ok_or_else(|| ApiError::not_found("organization"))?;
        let access = o.included.then(|| org_access(held, o)).transpose()?;
        if orgs.insert(held.id, access).is_some() {
            return Err(ApiError::invalid(
                "orgs",
                "Each organization can be listed only once.",
            ));
        }
    }
    for held in g.holdings {
        orgs.entry(held.id).or_insert(None);
    }
    let future = match &body.future_orgs {
        None => None,
        Some(f) => {
            let rights: OrgRights = parsed("future_orgs", &f.rights)?;
            if let Some(never) = rights.minus(grantable_org_rights()).names().first() {
                return Err(ApiError::invalid(
                    "future_orgs",
                    format!("`{never}` can't be given to agents."),
                ));
            }
            needs_met(
                "future_orgs",
                rights.iter(),
                |r| rights.contains(r),
                OrgRight::needs,
            )?;
            let every: ProjectRights = parsed("future_orgs", &f.project_rights)?;
            needs_met(
                "future_orgs",
                every.iter(),
                |r| every.contains(r),
                ProjectRight::needs,
            )?;
            Some(OrgAccess {
                rights,
                projects: Projects::All(every),
            })
        }
    };
    Ok(AgentGrant {
        preset,
        customized: body.customized,
        instance,
        account,
        future,
        orgs,
    })
}

fn added<I: Iterator<Item = Right>>(rights: I) -> bool {
    rights.into_iter().any(needs_confirmation)
}

fn access_rights(a: Option<&OrgAccess>) -> (OrgRights, BTreeMap<Option<Uuid>, ProjectRights>) {
    let Some(a) = a else {
        return (OrgRights::EMPTY, BTreeMap::new());
    };
    let projects = match &a.projects {
        Projects::All(r) => BTreeMap::from([(None, *r)]),
        Projects::Selected(each) => each.iter().map(|(id, r)| (Some(*id), *r)).collect(),
    };
    (a.rights, projects)
}

fn access_gains(before: Option<&OrgAccess>, after: Option<&OrgAccess>) -> bool {
    let (org_before, projects_before) = access_rights(before);
    let (org_after, projects_after) = access_rights(after);
    let every_before = projects_before.get(&None).copied().unwrap_or_default();
    added(org_after.minus(org_before).iter().map(Right::Org))
        || projects_after.iter().any(|(id, r)| {
            let had = projects_before.get(id).copied().unwrap_or(every_before);
            added(r.minus(had).iter().map(Right::Project))
        })
}

// Sensitive rights need a fresh sign-in to grant, like the actions they allow; rights the agent
// already had, or that are only taken away, need none.
pub fn gains_sensitive(before: Option<&AgentGrant>, after: &AgentGrant) -> bool {
    let instance = before.map_or(InstanceRights::EMPTY, |b| b.instance);
    let account = before.map_or(AccountRights::EMPTY, |b| b.account);
    if added(after.instance.minus(instance).iter().map(Right::Instance))
        || added(after.account.minus(account).iter().map(Right::Account))
        || access_gains(
            before.and_then(|b| b.future.as_ref()),
            after.future.as_ref(),
        )
    {
        return true;
    }
    let orgs: BTreeSet<Uuid> = after.orgs.keys().copied().collect();
    orgs.into_iter().any(|org| {
        let had = before.and_then(|b| b.org(org));
        access_gains(had, after.org(org))
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usable {
    pub instance: InstanceRights,
    pub account: AccountRights,
    pub org: OrgRights,
    pub project: ProjectRights,
}

impl Usable {
    pub fn contains(&self, right: Right) -> bool {
        match right {
            Right::Instance(r) => self.instance.contains(r),
            Right::Account(r) => self.account.contains(r),
            Right::Org(r) => self.org.contains(r),
            Right::Project(r) => self.project.contains(r),
        }
    }
}

pub async fn usable(state: &AppState, agent: &AgentCaller) -> ApiResult<Usable> {
    let held = holdings(&state.pg, agent.user_id()).await?;
    let managed = managed_by(state, &agent.sign_in).await?.map(|m| m.org_id);
    let mut out = Usable {
        account: agent.grant.account,
        ..Usable::default()
    };
    if agent.is_instance_admin {
        let admin_sso = if crate::platform::access::from_sso(&agent.sign_in) {
            kaeso_store::pg::instance::admin_sso_org(&state.pg).await?
        } else {
            None
        };
        if policy_gap(&agent.sign_in, managed, Area::Instance { admin_sso }).is_none() {
            out.instance = agent.instance_rights();
        }
    }
    for org in &held {
        let Some(access) = agent.grant.org(org.id) else {
            continue;
        };
        if !org.policy.allowed || policy_gap(&agent.sign_in, managed, org.area).is_some() {
            continue;
        }
        let rights = agent_org_rights(org.org_rights(), access.rights, org.policy.max);
        out.org = out.org.union(rights);
        for p in &org.projects {
            out.project = out.project.union(agent_project_rights(
                p.rights,
                access.projects.granted(p.id),
                org.policy.max,
                rights,
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_core::rights::{InstancePreset, OrgPreset, ProjectPreset};

    #[test]
    fn models_are_guessed_from_the_names_clients_register() {
        assert_eq!(guess_model(&["Claude Code"]), "claude");
        assert_eq!(guess_model(&["codex-mcp-client"]), "gpt");
        assert_eq!(guess_model(&["gemini-cli-mcp-client"]), "gemini");
        assert_eq!(guess_model(&["Cursor"]), "unknown");
        assert_eq!(guess_model(&["My tool", "com.anthropic.claude"]), "claude");
        assert_eq!(guess_model(&[]), "unknown");
        for m in MODELS {
            assert_eq!(known_model(m), Some(*m));
        }
        assert_eq!(known_model("claude_code"), None);
    }

    #[test]
    fn expiry_offers_a_week_a_month_a_quarter_or_never() {
        let now = OffsetDateTime::now_utc();
        assert_eq!(
            Expiry::parse("7d").ok().and_then(|e| e.from(now)),
            Some(now + Duration::days(7))
        );
        assert_eq!(
            Expiry::parse("30d").ok().and_then(|e| e.from(now)),
            Some(now + Duration::days(30))
        );
        assert_eq!(
            Expiry::parse("90d").ok().and_then(|e| e.from(now)),
            Some(now + Duration::days(90))
        );
        assert_eq!(Expiry::parse("never").ok().map(|e| e.from(now)), Some(None));
        assert!(Expiry::parse("1y").is_err());
    }

    fn held(preset: OrgPreset, policy: OrgPolicy) -> HeldOrg {
        let id = Uuid::now_v7();
        HeldOrg {
            id,
            name: "Acme".into(),
            avatar: None,
            state: "active".into(),
            area: Area::Org {
                id,
                require_sso: false,
                require_2fa: false,
            },
            grant: OrgGrant::preset(preset),
            policy,
            projects: vec![HeldProject {
                id: Uuid::now_v7(),
                name: "Default".into(),
                is_default: true,
                avatar: None,
                rights: ProjectPreset::Developer.rights(),
            }],
        }
    }

    fn open() -> OrgPolicy {
        OrgPolicy {
            allowed: true,
            max: AgentPreset::Full,
        }
    }

    fn body(org: &HeldOrg, rights: &[&str], projects: &[&str]) -> GrantBody {
        serde_json::from_value(serde_json::json!({
            "preset": "builder",
            "customized": false,
            "instance": [],
            "account": [],
            "orgs": [{
                "org": kaeso_core::id::OrgId::from_uuid(org.id),
                "included": true,
                "rights": rights,
                "projects": { "all": true, "rights": projects, "selected": [] }
            }],
            "future_orgs": null
        }))
        .unwrap()
    }

    fn granting(h: &[HeldOrg], admin: bool) -> Granting<'_> {
        Granting {
            holdings: h,
            is_instance_admin: admin,
            instance: InstancePolicy {
                allowed: true,
                max: InstancePreset::Configure,
            },
        }
    }

    #[test]
    fn an_agent_gets_only_rights_its_user_holds() {
        let h = [held(OrgPreset::Developer, open())];
        let ok = validate(
            &granting(&h, false),
            &body(&h[0], &["org.view"], &["project.view", "models.view"]),
        )
        .unwrap();
        assert!(ok.org(h[0].id).unwrap().rights.contains(OrgRight::OrgView));
        let err = validate(
            &granting(&h, false),
            &body(&h[0], &["org.view", "roles.manage"], &[]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("roles.manage"), "{err}");
        let err =
            validate(&granting(&h, false), &body(&h[0], &[], &["limits.manage"])).unwrap_err();
        assert!(err.to_string().contains("limits.manage"), "{err}");
    }

    #[test]
    fn never_grantable_rights_and_policies_are_enforced() {
        let owner = [held(OrgPreset::Owner, open())];
        let err = validate(
            &granting(&owner, false),
            &body(&owner[0], &["org.view", "security.sso.manage"], &[]),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("can't be given to agents"),
            "{err}"
        );
        let capped = [held(
            OrgPreset::Owner,
            OrgPolicy {
                allowed: true,
                max: AgentPreset::Observer,
            },
        )];
        let err = validate(
            &granting(&capped, false),
            &body(&capped[0], &["org.view", "roles.manage"], &[]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("agent policy"), "{err}");
        let closed = [held(
            OrgPreset::Owner,
            OrgPolicy {
                allowed: false,
                max: AgentPreset::Full,
            },
        )];
        let err = validate(
            &granting(&closed, false),
            &body(&closed[0], &["org.view"], &[]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("doesn't allow agents"), "{err}");
    }

    #[test]
    fn needs_must_come_along_and_unknown_rights_are_refused() {
        let h = [held(OrgPreset::Owner, open())];
        let err = validate(
            &granting(&h, false),
            &body(&h[0], &["org.view"], &["models.manage"]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("needs"), "{err}");
        let err =
            validate(&granting(&h, false), &body(&h[0], &["org.everything"], &[])).unwrap_err();
        assert!(err.to_string().contains("org.everything"), "{err}");
    }

    #[test]
    fn instance_rights_are_for_instance_admins_within_the_instance_policy() {
        let h: [HeldOrg; 0] = [];
        let mut b: GrantBody = serde_json::from_value(serde_json::json!({
            "preset": "operator", "customized": false, "instance": ["instance.system.view"],
            "account": [], "orgs": [], "future_orgs": null
        }))
        .unwrap();
        assert!(validate(&granting(&h, false), &b).is_err());
        assert!(validate(&granting(&h, true), &b).is_ok());
        b.instance = vec![
            "instance.updates.manage".into(),
            "instance.updates.apply".into(),
            "instance.system.view".into(),
        ];
        let err = validate(&granting(&h, true), &b).unwrap_err();
        assert!(err.to_string().contains("instance.updates.apply"), "{err}");
    }

    #[test]
    fn orgs_left_out_are_closed_even_with_future_orgs_on() {
        let h = [
            held(OrgPreset::Developer, open()),
            held(OrgPreset::Developer, open()),
        ];
        let mut b = body(&h[0], &["org.view"], &[]);
        b.future_orgs = Some(
            serde_json::from_value(
                serde_json::json!({ "rights": ["org.view"], "project_rights": [] }),
            )
            .unwrap(),
        );
        let g = validate(&granting(&h, false), &b).unwrap();
        assert!(g.org(h[0].id).is_some());
        assert!(g.org(h[1].id).is_none());
        assert!(g.org(Uuid::now_v7()).is_some());
    }

    #[test]
    fn granting_a_sensitive_right_needs_confirmation_once() {
        let h = [held(OrgPreset::Owner, open())];
        let plain = validate(
            &granting(&h, false),
            &body(&h[0], &["org.view"], &["project.view"]),
        )
        .unwrap();
        assert!(!gains_sensitive(None, &plain));
        let content = validate(
            &granting(&h, false),
            &body(
                &h[0],
                &["org.view"],
                &["project.view", "logs.metadata.view", "logs.content.view"],
            ),
        )
        .unwrap();
        assert!(gains_sensitive(Some(&plain), &content));
        assert!(!gains_sensitive(Some(&content), &plain));
        assert!(!gains_sensitive(Some(&content), &content));
        let mut keys = plain.clone();
        keys.account = keys
            .account
            .with(AccountRight::AccountKeysView)
            .with(AccountRight::AccountKeysManage);
        assert!(gains_sensitive(Some(&plain), &keys));
    }
}
