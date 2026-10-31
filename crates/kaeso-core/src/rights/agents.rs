use std::fmt;
use std::str::FromStr;

use super::catalogue::{AccountRight, InstanceRight, OrgRight, ProjectRight, Right};
use super::presets::{OrgPreset, PresetParseError, ProjectPreset};
use super::{AccountRights, InstanceRights, OrgRights, ProjectRights, org_reach};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentPreset {
    Observer,
    Builder,
    Operator,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstancePreset {
    None,
    Read,
    Configure,
    Admin,
}

pub fn grantable_org_rights() -> OrgRights {
    OrgRights::all().minus(
        [
            OrgRight::OrgOwnershipTransfer,
            OrgRight::SecuritySsoManage,
            OrgRight::SecuritySsoRequire,
        ]
        .into_iter()
        .collect(),
    )
}

pub fn needs_confirmation(right: Right) -> bool {
    right.is_sensitive() || right.needs_recent_sign_in()
}

impl AgentPreset {
    pub const ALL: [AgentPreset; 4] = [
        AgentPreset::Observer,
        AgentPreset::Builder,
        AgentPreset::Operator,
        AgentPreset::Full,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AgentPreset::Observer => "observer",
            AgentPreset::Builder => "builder",
            AgentPreset::Operator => "operator",
            AgentPreset::Full => "full",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            AgentPreset::Observer => "Observer",
            AgentPreset::Builder => "Builder",
            AgentPreset::Operator => "Operator",
            AgentPreset::Full => "Full access",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            AgentPreset::Observer => {
                "Reads usage, costs, models, limits, log metadata and audit logs; never prompts or outputs."
            }
            AgentPreset::Builder => {
                "Works in projects like a developer and manages their models; sees personal keys."
            }
            AgentPreset::Operator => {
                "Administers organizations and projects, except deleting, ownership and billing."
            }
            AgentPreset::Full => "Everything you can do that an agent may be given.",
        }
    }

    pub fn org_rights(self) -> OrgRights {
        use OrgRight as R;
        match self {
            AgentPreset::Observer => [
                R::OrgView,
                R::MembersView,
                R::ProjectsViewAll,
                R::LogsStorageView,
                R::UsageOrgView,
                R::CostsOrgView,
                R::AuditLogView,
            ]
            .into_iter()
            .collect(),
            AgentPreset::Builder => OrgPreset::Developer.rights(),
            AgentPreset::Operator => OrgPreset::Admin.rights().intersect(grantable_org_rights()),
            AgentPreset::Full => grantable_org_rights(),
        }
    }

    pub fn project_rights(self) -> ProjectRights {
        use ProjectRight as R;
        match self {
            AgentPreset::Observer => [
                R::ProjectView,
                R::ModelsView,
                R::LimitsView,
                R::LogsMetadataView,
                R::UsageView,
                R::CostsView,
                R::ProjectAuditLogView,
            ]
            .into_iter()
            .collect(),
            // Prompts and outputs stay out of the default preset: granting them needs a fresh sign-in.
            AgentPreset::Builder => ProjectPreset::Developer
                .rights()
                .minus(ProjectRights::EMPTY.with(R::LogsContentView))
                .with(R::ModelsManage),
            AgentPreset::Operator | AgentPreset::Full => ProjectPreset::Admin.rights(),
        }
    }

    pub fn account_rights(self) -> AccountRights {
        use AccountRight as R;
        match self {
            AgentPreset::Observer => [R::AccountView].into_iter().collect(),
            AgentPreset::Builder => [R::AccountView, R::AccountKeysView].into_iter().collect(),
            AgentPreset::Operator => [
                R::AccountView,
                R::AccountUpdate,
                R::AccountKeysView,
                R::AccountNotificationsManage,
            ]
            .into_iter()
            .collect(),
            AgentPreset::Full => AccountRights::all(),
        }
    }

    pub const fn instance(self) -> InstancePreset {
        match self {
            AgentPreset::Observer => InstancePreset::Read,
            AgentPreset::Builder => InstancePreset::None,
            AgentPreset::Operator => InstancePreset::Configure,
            AgentPreset::Full => InstancePreset::Admin,
        }
    }

    fn up_to(self) -> impl Iterator<Item = AgentPreset> {
        AgentPreset::ALL.into_iter().filter(move |p| *p <= self)
    }

    pub fn org_ceiling(self) -> OrgRights {
        self.up_to()
            .fold(OrgRights::EMPTY, |acc, p| acc.union(p.org_rights()))
    }

    pub fn project_ceiling(self) -> ProjectRights {
        self.up_to()
            .fold(ProjectRights::EMPTY, |acc, p| acc.union(p.project_rights()))
    }
}

impl InstancePreset {
    pub const ALL: [InstancePreset; 4] = [
        InstancePreset::None,
        InstancePreset::Read,
        InstancePreset::Configure,
        InstancePreset::Admin,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            InstancePreset::None => "none",
            InstancePreset::Read => "read",
            InstancePreset::Configure => "configure",
            InstancePreset::Admin => "admin",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            InstancePreset::None => "None",
            InstancePreset::Read => "Read",
            InstancePreset::Configure => "Configure",
            InstancePreset::Admin => "Admin",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            InstancePreset::None => "No access to the instance.",
            InstancePreset::Read => {
                "Reads the system, users, organizations and the instance audit log."
            }
            InstancePreset::Configure => {
                "Reads the instance and changes its settings and update settings."
            }
            InstancePreset::Admin => "Everything instance admins do that an agent may be given.",
        }
    }

    pub fn rights(self) -> InstanceRights {
        use InstanceRight as R;
        let read: InstanceRights = [
            R::InstanceSystemView,
            R::InstanceUsersView,
            R::InstanceOrgsView,
            R::InstanceAuditLogView,
        ]
        .into_iter()
        .collect();
        match self {
            InstancePreset::None => InstanceRights::EMPTY,
            InstancePreset::Read => read,
            InstancePreset::Configure => read
                .with(R::InstanceSettingsManage)
                .with(R::InstanceUpdatesManage),
            InstancePreset::Admin => InstanceRights::all(),
        }
    }
}

pub fn agent_org_rights(user: OrgRights, granted: OrgRights, max: AgentPreset) -> OrgRights {
    user.intersect(granted)
        .intersect(max.org_ceiling())
        .intersect(grantable_org_rights())
}

// Reaching into projects through `projects.view.all` or `projects.archive.any` counts only for the
// org rights the agent ends up with, never for the user's own.
pub fn agent_project_rights(
    user: ProjectRights,
    granted: ProjectRights,
    max: AgentPreset,
    org: OrgRights,
) -> ProjectRights {
    user.intersect(
        granted
            .intersect(max.project_ceiling())
            .union(org_reach(org)),
    )
}

pub fn agent_instance_rights(
    is_admin: bool,
    granted: InstanceRights,
    max: InstancePreset,
) -> InstanceRights {
    if is_admin {
        granted.intersect(max.rights())
    } else {
        InstanceRights::EMPTY
    }
}

pub fn agent_account_rights(granted: AccountRights) -> AccountRights {
    granted.with(AccountRight::AccountView)
}

impl FromStr for AgentPreset {
    type Err = PresetParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        AgentPreset::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| PresetParseError(s.to_owned()))
    }
}

impl FromStr for InstancePreset {
    type Err = PresetParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InstancePreset::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| PresetParseError(s.to_owned()))
    }
}

impl fmt::Display for AgentPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for InstancePreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
