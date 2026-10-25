use std::collections::BTreeMap;
use std::sync::Arc;

use kaeso_core::rights::{
    AccountRights, AgentPreset, InstancePreset, InstanceRights, OrgRights, ProjectRights,
    agent_account_rights, agent_instance_rights,
};
use kaeso_core::secret;
use kaeso_store::StoreError;
use kaeso_store::pg::agents::{self, Agent, AgentAuth, StoredPolicy};
use kaeso_store::pg::sessions::SessionRow;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::state::AppState;

pub const ACCESS_PREFIX: &str = "kso_at_";
pub const REFRESH_PREFIX: &str = "kso_rt_";
pub const CODE_PREFIX: &str = "kso_ac_";
pub const ACCESS_LIFETIME: Duration = Duration::hours(1);
pub const CODE_LIFETIME: Duration = Duration::minutes(5);
pub const REQUEST_LIFETIME: Duration = Duration::minutes(15);
pub const UNUSED_LIFETIME: Duration = Duration::days(90);
const TOUCH_EVERY: Duration = Duration::seconds(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projects {
    All(ProjectRights),
    Selected(BTreeMap<Uuid, ProjectRights>),
}

impl Projects {
    pub fn granted(&self, project: Uuid) -> ProjectRights {
        match self {
            Projects::All(rights) => *rights,
            Projects::Selected(each) => each.get(&project).copied().unwrap_or_default(),
        }
    }

    pub fn every_project(&self) -> ProjectRights {
        match self {
            Projects::All(rights) => *rights,
            Projects::Selected(_) => ProjectRights::EMPTY,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgAccess {
    pub rights: OrgRights,
    pub projects: Projects,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentGrant {
    pub preset: AgentPreset,
    pub customized: bool,
    pub instance: InstanceRights,
    pub account: AccountRights,
    pub future: Option<OrgAccess>,
    pub orgs: BTreeMap<Uuid, Option<OrgAccess>>,
}

fn lenient<S: FromIterator<R>, R: std::str::FromStr>(names: &[String]) -> S {
    names.iter().filter_map(|n| n.parse().ok()).collect()
}

impl AgentGrant {
    pub fn from_store(g: &agents::Grant) -> AgentGrant {
        let mut selected: BTreeMap<Uuid, BTreeMap<Uuid, ProjectRights>> = BTreeMap::new();
        for p in &g.projects {
            selected
                .entry(p.org_id)
                .or_default()
                .insert(p.project_id, lenient(&p.rights));
        }
        let orgs = g
            .orgs
            .iter()
            .map(|o| {
                let access = o.included.then(|| OrgAccess {
                    rights: lenient(&o.rights),
                    projects: if o.all_projects {
                        Projects::All(lenient(&o.project_rights))
                    } else {
                        Projects::Selected(selected.remove(&o.org_id).unwrap_or_default())
                    },
                });
                (o.org_id, access)
            })
            .collect();
        AgentGrant {
            preset: g.preset.parse().unwrap_or(AgentPreset::Observer),
            customized: g.customized,
            instance: lenient(&g.instance_rights),
            account: agent_account_rights(lenient(&g.account_rights)),
            future: g.future_orgs.then(|| OrgAccess {
                rights: lenient(&g.future_org_rights),
                projects: Projects::All(lenient(&g.future_project_rights)),
            }),
            orgs,
        }
    }

    pub fn to_store(&self) -> agents::Grant {
        let mut orgs = Vec::new();
        let mut projects = Vec::new();
        for (org, access) in &self.orgs {
            let Some(access) = access else {
                orgs.push(agents::AgentOrg {
                    org_id: *org,
                    included: false,
                    rights: Vec::new(),
                    all_projects: true,
                    project_rights: Vec::new(),
                });
                continue;
            };
            let (all, every) = match &access.projects {
                Projects::All(r) => (true, names(r.names())),
                Projects::Selected(each) => {
                    for (project, rights) in each {
                        projects.push(agents::AgentProject {
                            org_id: *org,
                            project_id: *project,
                            rights: names(rights.names()),
                        });
                    }
                    (false, Vec::new())
                }
            };
            orgs.push(agents::AgentOrg {
                org_id: *org,
                included: true,
                rights: names(access.rights.names()),
                all_projects: all,
                project_rights: every,
            });
        }
        let future = self.future.as_ref();
        agents::Grant {
            preset: self.preset.as_str().to_owned(),
            customized: self.customized,
            instance_rights: names(self.instance.names()),
            account_rights: names(self.account.names()),
            future_orgs: future.is_some(),
            future_org_rights: future.map_or_else(Vec::new, |f| names(f.rights.names())),
            future_project_rights: future
                .map_or_else(Vec::new, |f| names(f.projects.every_project().names())),
            orgs,
            projects,
        }
    }

    pub fn org(&self, org: Uuid) -> Option<&OrgAccess> {
        match self.orgs.get(&org) {
            Some(listed) => listed.as_ref(),
            None => self.future.as_ref(),
        }
    }
}

fn names(n: Vec<&'static str>) -> Vec<String> {
    n.into_iter().map(str::to_owned).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstancePolicy {
    pub allowed: bool,
    pub max: InstancePreset,
}

impl InstancePolicy {
    pub fn from_store(p: &StoredPolicy) -> InstancePolicy {
        InstancePolicy {
            allowed: p.allowed,
            max: p.max.parse().unwrap_or(InstancePreset::None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrgPolicy {
    pub allowed: bool,
    pub max: AgentPreset,
}

impl OrgPolicy {
    pub fn from_store(p: &StoredPolicy) -> OrgPolicy {
        OrgPolicy {
            allowed: p.allowed,
            max: p.max.parse().unwrap_or(AgentPreset::Observer),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentCaller {
    pub agent: Agent,
    pub grant: AgentGrant,
    pub is_instance_admin: bool,
    pub instance_policy: InstancePolicy,
    pub sign_in: SessionRow,
}

impl AgentCaller {
    pub fn user_id(&self) -> Uuid {
        self.agent.user_id
    }

    pub fn instance_rights(&self) -> InstanceRights {
        agent_instance_rights(
            self.is_instance_admin,
            self.grant.instance,
            self.instance_policy.max,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Invalid,
    NotAllowed,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Refusal::Invalid => {
                "This agent's sign-in has ended or was revoked; connect the agent again."
            }
            Refusal::NotAllowed => {
                "This Kaeso install doesn't allow agents; ask an instance admin."
            }
        }
    }
}

pub fn is_access_token(presented: &str) -> bool {
    presented.starts_with(ACCESS_PREFIX)
}

pub fn unused_too_long(agent: &Agent, now: OffsetDateTime) -> bool {
    now - agent.last_used_at.unwrap_or(agent.created_at) >= UNUSED_LIFETIME
}

pub async fn authenticate(
    state: &AppState,
    presented: &str,
) -> Result<Result<Arc<AgentCaller>, Refusal>, StoreError> {
    let now = state.clock.now();
    let hash = secret::hash(presented);
    let Some(AgentAuth {
        agent,
        user_state,
        is_instance_admin,
        ..
    }) = agents::authenticate(&state.pg, &hash, now).await?
    else {
        return Ok(Err(Refusal::Invalid));
    };
    if user_state != "active" || unused_too_long(&agent, now) {
        return Ok(Err(Refusal::Invalid));
    }
    let instance_policy = InstancePolicy::from_store(&agents::instance_policy(&state.pg).await?);
    if !instance_policy.allowed {
        return Ok(Err(Refusal::NotAllowed));
    }
    let mut conn = state.pg.acquire().await?;
    let grant = AgentGrant::from_store(&agents::grant_of(&mut conn, &agent).await?);
    drop(conn);
    agents::touch(&state.pg, agent.id, now, TOUCH_EVERY).await?;
    let sign_in = SessionRow {
        id: agent.id,
        user_id: agent.user_id,
        method: agent.sign_in_method.clone(),
        second_factor: agent.second_factor,
        oidc_org_id: agent.oidc_org_id,
        signed_in_at: agent.created_at,
        last_used_at: agent.last_used_at.unwrap_or(agent.created_at),
        expires_at: now + UNUSED_LIFETIME,
        user_state,
        is_instance_admin,
    };
    Ok(Ok(Arc::new(AgentCaller {
        agent,
        grant,
        is_instance_admin,
        instance_policy,
        sign_in,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_core::rights::{AccountRight, OrgRight, ProjectRight};

    fn grant() -> AgentGrant {
        let org_a = Uuid::now_v7();
        let org_b = Uuid::now_v7();
        let project = Uuid::now_v7();
        AgentGrant {
            preset: AgentPreset::Builder,
            customized: true,
            instance: InstanceRights::EMPTY,
            account: AccountRights::EMPTY.with(AccountRight::AccountView),
            future: Some(OrgAccess {
                rights: OrgRights::EMPTY.with(OrgRight::OrgView),
                projects: Projects::All(ProjectRights::EMPTY.with(ProjectRight::ProjectView)),
            }),
            orgs: BTreeMap::from([
                (
                    org_a,
                    Some(OrgAccess {
                        rights: OrgRights::EMPTY.with(OrgRight::OrgView),
                        projects: Projects::Selected(BTreeMap::from([(
                            project,
                            ProjectRights::EMPTY
                                .with(ProjectRight::ProjectView)
                                .with(ProjectRight::ModelsView),
                        )])),
                    }),
                ),
                (org_b, None),
            ]),
        }
    }

    #[test]
    fn a_grant_survives_the_store() {
        let g = grant();
        assert_eq!(AgentGrant::from_store(&g.to_store()), g);
    }

    #[test]
    fn an_unchecked_org_stays_closed_while_unknown_orgs_follow_the_future_setting() {
        let g = grant();
        let (closed, _) = g.orgs.iter().find(|(_, a)| a.is_none()).unwrap();
        assert!(g.org(*closed).is_none());
        assert_eq!(g.org(Uuid::now_v7()), g.future.as_ref());
        let mut without_future = g;
        without_future.future = None;
        assert!(without_future.org(Uuid::now_v7()).is_none());
    }

    #[test]
    fn selected_projects_open_only_themselves() {
        let g = grant();
        let access = g.orgs.values().flatten().next().unwrap();
        let Projects::Selected(each) = &access.projects else {
            panic!("selected projects");
        };
        let (project, rights) = each.iter().next().unwrap();
        assert_eq!(access.projects.granted(*project), *rights);
        assert!(access.projects.granted(Uuid::now_v7()).is_empty());
        assert!(access.projects.every_project().is_empty());
    }

    #[test]
    fn rights_a_release_no_longer_knows_are_left_out() {
        let mut stored = grant().to_store();
        stored.account_rights.push("account.gone".to_owned());
        stored.preset = "retired".to_owned();
        let g = AgentGrant::from_store(&stored);
        assert_eq!(
            g.account,
            AccountRights::EMPTY.with(AccountRight::AccountView)
        );
        assert_eq!(g.preset, AgentPreset::Observer);
    }

    #[test]
    fn an_agent_left_unused_for_ninety_days_stops_working() {
        let now = OffsetDateTime::now_utc();
        let mut a = Agent {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            client_id: None,
            name: "Agent".into(),
            model: "unknown".into(),
            preset: "builder".into(),
            customized: false,
            instance_rights: vec![],
            account_rights: vec![],
            future_orgs: false,
            future_org_rights: vec![],
            future_project_rights: vec![],
            sign_in_method: "password".into(),
            second_factor: false,
            oidc_org_id: None,
            created_at: now - Duration::days(200),
            last_used_at: Some(now - Duration::days(89)),
            revoked_at: None,
        };
        assert!(!unused_too_long(&a, now));
        a.last_used_at = Some(now - Duration::days(90));
        assert!(unused_too_long(&a, now));
        a.last_used_at = None;
        assert!(unused_too_long(&a, now));
    }
}
