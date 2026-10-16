use std::fmt;
use std::str::FromStr;

use super::catalogue::{OrgRight, ProjectRight};
use super::{OrgRights, ProjectRights};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrgPreset {
    Owner,
    Admin,
    Billing,
    Developer,
    Auditor,
    Member,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPreset {
    Admin,
    Developer,
    Viewer,
}

impl OrgPreset {
    pub const ALL: [OrgPreset; 6] = [
        OrgPreset::Owner,
        OrgPreset::Admin,
        OrgPreset::Billing,
        OrgPreset::Developer,
        OrgPreset::Auditor,
        OrgPreset::Member,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            OrgPreset::Owner => "owner",
            OrgPreset::Admin => "admin",
            OrgPreset::Billing => "billing",
            OrgPreset::Developer => "developer",
            OrgPreset::Auditor => "auditor",
            OrgPreset::Member => "member",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            OrgPreset::Owner => "Owner",
            OrgPreset::Admin => "Admin",
            OrgPreset::Billing => "Billing",
            OrgPreset::Developer => "Developer",
            OrgPreset::Auditor => "Auditor",
            OrgPreset::Member => "Member",
        }
    }

    pub fn project_rights(self) -> ProjectRights {
        match self {
            OrgPreset::Owner | OrgPreset::Admin => ProjectRights::all(),
            OrgPreset::Auditor => ProjectPreset::Viewer
                .rights()
                .with(ProjectRight::ProjectAuditLogView),
            OrgPreset::Billing | OrgPreset::Developer | OrgPreset::Member => ProjectRights::EMPTY,
        }
    }

    pub fn rights(self) -> OrgRights {
        use OrgRight as R;
        match self {
            OrgPreset::Owner => OrgRights::all(),
            OrgPreset::Admin => OrgRights::all().minus(
                [R::OrgDelete, R::OrgOwnershipTransfer, R::BillingManage]
                    .into_iter()
                    .collect(),
            ),
            OrgPreset::Billing => [
                R::OrgView,
                R::MembersView,
                R::ProjectsViewAll,
                R::LogsStorageView,
                R::UsageOrgView,
                R::CostsOrgView,
                R::BillingView,
                R::BillingManage,
            ]
            .into_iter()
            .collect(),
            OrgPreset::Developer => [
                R::OrgView,
                R::MembersView,
                R::ProjectsCreate,
                R::ProviderCredentialsView,
                R::LogsStorageView,
                R::UsageOrgView,
                R::CostsOrgView,
            ]
            .into_iter()
            .collect(),
            OrgPreset::Auditor => [
                R::OrgView,
                R::MembersView,
                R::ProjectsViewAll,
                R::ProviderCredentialsView,
                R::PlatformKeysView,
                R::LogsStorageView,
                R::UsageOrgView,
                R::CostsOrgView,
                R::BillingView,
                R::AuditLogView,
                R::AuditLogExport,
            ]
            .into_iter()
            .collect(),
            OrgPreset::Member => [R::OrgView, R::MembersView].into_iter().collect(),
        }
    }

    pub const fn default_project_preset(self) -> ProjectPreset {
        match self {
            OrgPreset::Owner | OrgPreset::Admin => ProjectPreset::Admin,
            OrgPreset::Developer => ProjectPreset::Developer,
            OrgPreset::Billing | OrgPreset::Auditor | OrgPreset::Member => ProjectPreset::Viewer,
        }
    }
}

impl ProjectPreset {
    pub const ALL: [ProjectPreset; 3] = [
        ProjectPreset::Admin,
        ProjectPreset::Developer,
        ProjectPreset::Viewer,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ProjectPreset::Admin => "admin",
            ProjectPreset::Developer => "developer",
            ProjectPreset::Viewer => "viewer",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            ProjectPreset::Admin => "Admin",
            ProjectPreset::Developer => "Developer",
            ProjectPreset::Viewer => "Viewer",
        }
    }

    pub fn rights(self) -> ProjectRights {
        use ProjectRight as R;
        let viewer: ProjectRights = [
            R::ProjectView,
            R::ProjectMembersView,
            R::KeysAllView,
            R::ServiceAccountsView,
            R::ModelsView,
            R::LimitsView,
            R::LogsMetadataView,
            R::UsageView,
            R::CostsView,
        ]
        .into_iter()
        .collect();
        match self {
            ProjectPreset::Admin => ProjectRights::all(),
            ProjectPreset::Developer => viewer.union(
                [
                    R::ApiRequest,
                    R::PlaygroundUse,
                    R::KeysOwnCreate,
                    R::KeysOwnManage,
                    R::LogsContentView,
                ]
                .into_iter()
                .collect(),
            ),
            ProjectPreset::Viewer => viewer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a preset")]
pub struct PresetParseError(pub String);

impl FromStr for OrgPreset {
    type Err = PresetParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OrgPreset::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| PresetParseError(s.to_owned()))
    }
}

impl FromStr for ProjectPreset {
    type Err = PresetParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ProjectPreset::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| PresetParseError(s.to_owned()))
    }
}

impl fmt::Display for OrgPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for ProjectPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
