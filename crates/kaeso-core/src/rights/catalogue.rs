use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a right; see the rights catalogue")]
pub struct RightParseError(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Organization,
    Members,
    Projects,
    Providers,
    Security,
    Keys,
    Logs,
    UsageBilling,
    Audit,
    Project,
    ModelUse,
    Models,
    Limits,
    UsageCosts,
    Instance,
    Account,
}

impl Category {
    pub const fn id(self) -> &'static str {
        match self {
            Category::Organization => "organization",
            Category::Members => "members",
            Category::Projects => "projects",
            Category::Providers => "providers",
            Category::Security => "security",
            Category::Keys => "keys",
            Category::Logs => "logs",
            Category::UsageBilling | Category::UsageCosts => "usage",
            Category::Audit => "audit",
            Category::Project => "project",
            Category::ModelUse => "model_use",
            Category::Models => "models",
            Category::Limits => "limits",
            Category::Instance => "instance",
            Category::Account => "account",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Category::Organization => "Organization",
            Category::Members => "Members & access",
            Category::Projects => "Projects",
            Category::Providers => "Providers",
            Category::Security => "Security",
            Category::Keys => "Keys",
            Category::Logs => "Logs & data",
            Category::UsageBilling => "Usage & billing",
            Category::Audit => "Audit",
            Category::Project => "Project",
            Category::ModelUse => "Model use",
            Category::Models => "Models",
            Category::Limits => "Limits & budgets",
            Category::UsageCosts => "Usage & costs",
            Category::Instance => "Instance",
            Category::Account => "Account",
        }
    }
}

macro_rules! rights {
    (
        $(#[$doc:meta])* $name:ident, view = $view:ident {
            $( $variant:ident = $str:literal, $cat:ident, $label:literal, $sensitive:literal, $recent:literal, $cloud:literal, [$($need:ident),*], $allows:literal; )*
        }
    ) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $(
                #[doc = $allows]
                $variant,
            )*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $str,)*
                }
            }

            pub const fn category(self) -> Category {
                match self {
                    $($name::$variant => Category::$cat,)*
                }
            }

            pub const fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label,)*
                }
            }

            pub const fn is_sensitive(self) -> bool {
                match self {
                    $($name::$variant => $sensitive,)*
                }
            }

            pub const fn needs_recent_sign_in(self) -> bool {
                match self {
                    $($name::$variant => $recent,)*
                }
            }

            pub const fn cloud_only(self) -> bool {
                match self {
                    $($name::$variant => $cloud,)*
                }
            }

            pub const fn description(self) -> &'static str {
                match self {
                    $($name::$variant => $allows,)*
                }
            }

            pub const fn needs_beyond_view(self) -> &'static [$name] {
                match self {
                    $($name::$variant => &[$($name::$need),*],)*
                }
            }

            pub fn needs(self) -> Vec<$name> {
                let mut out = Vec::new();
                if self != $name::$view {
                    out.push($name::$view);
                }
                out.extend_from_slice(self.needs_beyond_view());
                out
            }

            pub(crate) fn bit(self) -> u64 {
                1u64 << (self as u32)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = RightParseError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($str => Ok($name::$variant),)*
                    other => Err(RightParseError(other.to_owned())),
                }
            }
        }
    };
}

rights!(
    OrgRight, view = OrgView {
        OrgView = "org.view", Organization, "View organization", false, false, false, [], "See the organization, its settings and its roles.";
        OrgUpdate = "org.update", Organization, "Edit organization", false, false, false, [], "Rename the organization, change its picture and general settings, and choose the default project.";
        OrgDelete = "org.delete", Organization, "Delete organization", true, true, false, [], "Schedule the organization for deletion, or restore it while it waits.";
        OrgOwnershipTransfer = "org.ownership.transfer", Organization, "Transfer ownership", true, true, false, [MembersView], "Make another member an owner in your place; only owners hold it.";
        MembersView = "members.view", Members, "View members", false, false, false, [], "See members, their roles and their rights.";
        MembersInvite = "members.invite", Members, "Invite members", false, false, false, [MembersView], "Invite people by email with a role you could grant yourself.";
        MembersInvitesRevoke = "members.invites.revoke", Members, "Cancel invites", false, false, false, [MembersView], "Cancel pending invitations.";
        MembersRemove = "members.remove", Members, "Remove members", true, false, false, [MembersView], "Remove members from the organization and archive their keys in it.";
        MembersRightsUpdate = "members.rights.update", Members, "Change member roles", false, false, false, [MembersView], "Change members' org roles and individual org rights.";
        RolesManage = "roles.manage", Members, "Manage roles", false, false, false, [], "Create, edit and delete the organization's custom roles.";
        ProjectsViewAll = "projects.view.all", Projects, "View all projects", false, false, false, [], "See every project, including ones the member isn't in.";
        ProjectsCreate = "projects.create", Projects, "Create projects", false, false, false, [], "Create projects; the creator becomes their admin.";
        ProjectsArchiveAny = "projects.archive.any", Projects, "Archive any project", true, false, false, [ProjectsViewAll], "Archive any project and all its keys.";
        ProviderCredentialsView = "provider_credentials.view", Providers, "View provider keys", false, false, false, [], "See the organization's provider keys (metadata only, never secrets).";
        ProviderCredentialsCreate = "provider_credentials.create", Providers, "Add provider keys", false, true, false, [ProviderCredentialsView], "Add provider keys to the organization.";
        ProviderCredentialsReplace = "provider_credentials.replace", Providers, "Change provider keys", false, true, false, [ProviderCredentialsView], "Replace and rename provider keys, choose each provider's default and assign keys to projects.";
        ProviderCredentialsDelete = "provider_credentials.delete", Providers, "Delete provider keys", true, false, false, [ProviderCredentialsView], "Remove a provider key from the organization.";
        ProvidersCustomView = "providers.custom.view", Providers, "View custom providers", false, false, false, [], "See the organization's custom providers, with their addresses and formats.";
        ProvidersCustomManage = "providers.custom.manage", Providers, "Manage custom providers", true, true, false, [ProvidersCustomView], "Add, change and delete the organization's custom providers, the endpoints that get its requests and keys.";
        SecuritySsoManage = "security.sso.manage", Security, "Manage SSO", false, true, false, [], "Configure the organization's identity provider and the role of people who arrive through it.";
        SecuritySsoRequire = "security.sso.require", Security, "Require SSO", true, true, false, [], "Require the accounts this organization manages to sign in only through its SSO.";
        Security2faRequire = "security.2fa.require", Security, "Require two-factor", false, true, false, [], "Require two-factor authentication for members.";
        SecurityKeyPolicyManage = "security.key_policy.manage", Security, "Set key lifetime", false, false, false, [], "Set the organization's maximum key lifetime.";
        PlatformKeysView = "platform_keys.view", Keys, "View platform keys", false, false, false, [], "See the organization's platform keys (metadata).";
        PlatformKeysCreate = "platform_keys.create", Keys, "Create platform keys", true, true, false, [PlatformKeysView], "Create and rotate organization platform keys, which manage everything at org level.";
        PlatformKeysRevoke = "platform_keys.revoke", Keys, "Disable platform keys", false, false, false, [PlatformKeysView], "Disable, enable or archive organization platform keys.";
        LogsSettingsManage = "logs.settings.manage", Logs, "Manage log settings", false, false, false, [], "Set log retention and switch content logging on or off for the organization.";
        LogsStorageView = "logs.storage.view", Logs, "View log storage", false, false, false, [], "See how many log entries exist and how much space they take.";
        LogsDelete = "logs.delete", Logs, "Delete logs", true, true, false, [LogsStorageView], "Delete the organization's logs for a time range.";
        UsageOrgView = "usage.org.view", UsageBilling, "View org usage", false, false, false, [], "See usage across all projects.";
        CostsOrgView = "costs.org.view", UsageBilling, "View org costs", false, false, false, [UsageOrgView], "See costs across all projects.";
        BillingView = "billing.view", UsageBilling, "View billing", false, false, true, [], "See plan, invoices and payment status (Kaeso Cloud).";
        BillingManage = "billing.manage", UsageBilling, "Manage billing", false, false, true, [BillingView], "Change the plan and payment method (Kaeso Cloud).";
        AuditLogView = "audit_log.view", Audit, "View audit log", false, false, false, [], "Read the organization's audit log.";
        AuditLogExport = "audit_log.export", Audit, "Export audit log", false, false, false, [AuditLogView], "Export the organization's audit log.";
    }
);

rights!(
    ProjectRight, view = ProjectView {
        ProjectView = "project.view", Project, "View project", false, false, false, [], "See the project and its settings.";
        ProjectUpdate = "project.update", Project, "Edit project", false, false, false, [], "Rename the project and change its settings and picture.";
        ProjectArchive = "project.archive", Project, "Archive project", true, false, false, [], "Archive this project and all its keys.";
        ProjectMembersView = "project.members.view", Members, "View members", false, false, false, [], "See the project's members, their roles and their rights.";
        ProjectMembersAdd = "project.members.add", Members, "Add members", false, false, false, [ProjectMembersView], "Add organization members to the project with a role you could grant yourself.";
        ProjectMembersRemove = "project.members.remove", Members, "Remove members", false, false, false, [ProjectMembersView], "Remove members from the project and archive their keys in it.";
        ProjectMembersRightsUpdate = "project.members.rights.update", Members, "Change member roles", false, false, false, [ProjectMembersView], "Change members' project roles and individual project rights.";
        ServiceAccountsView = "service_accounts.view", Members, "View service accounts", false, false, false, [], "See the project's service accounts and their rights.";
        ServiceAccountsCreate = "service_accounts.create", Members, "Create service accounts", false, false, false, [ServiceAccountsView], "Create service accounts with rights you hold yourself.";
        ServiceAccountsRightsUpdate = "service_accounts.rights.update", Members, "Change service accounts", false, false, false, [ServiceAccountsView], "Rename service accounts and change their roles and rights.";
        ServiceAccountsArchive = "service_accounts.archive", Members, "Archive service accounts", false, false, false, [ServiceAccountsView], "Archive service accounts and their keys.";
        ApiRequest = "api.request", ModelUse, "Call models with keys", false, false, false, [], "Call the project's models with one's own API keys.";
        PlaygroundUse = "playground.use", ModelUse, "Use the Playground", false, false, false, [], "Call the project's models from the console Playground.";
        KeysOwnCreate = "keys.own.create", Keys, "Create own keys", false, false, false, [], "Create and rotate API keys for oneself.";
        KeysOwnManage = "keys.own.manage", Keys, "Manage own keys", false, false, false, [], "Disable, enable or archive one's own API keys.";
        KeysAllView = "keys.all.view", Keys, "View all keys", false, false, false, [], "See every key in the project (metadata).";
        KeysAllManage = "keys.all.manage", Keys, "Manage all keys", false, false, false, [KeysAllView], "Disable, enable or archive anyone's keys in the project.";
        ServiceAccountsKeysCreate = "service_accounts.keys.create", Keys, "Create service account keys", false, false, false, [ServiceAccountsView], "Create and rotate keys for the project's service accounts.";
        KeysPlatformAccessGrant = "keys.platform_access.grant", Keys, "Give keys platform access", true, true, false, [], "Give a new key access to the Platform API.";
        SecurityKeyPolicyProjectManage = "security.key_policy.project.manage", Keys, "Set key lifetime", false, false, false, [], "Set the project's maximum key lifetime.";
        ModelsView = "models.view", Models, "View models", false, false, false, [], "See the project's models, aliases and routing.";
        ModelsManage = "models.manage", Models, "Manage models", false, false, false, [ModelsView], "Add, change and remove models and aliases.";
        LimitsView = "limits.view", Limits, "View limits", false, false, false, [], "See project, member and key limits, budgets and model access.";
        LimitsManage = "limits.manage", Limits, "Manage limits", false, false, false, [LimitsView], "Change project, member and key limits, budgets and model access.";
        LogsMetadataView = "logs.metadata.view", Logs, "View request logs", false, false, false, [], "See request logs: model, tokens, cost, latency, status and key.";
        LogsContentView = "logs.content.view", Logs, "View prompts and outputs", true, false, false, [LogsMetadataView], "See request and response bodies in the logs.";
        LogsExport = "logs.export", Logs, "Export logs", true, false, false, [LogsMetadataView], "Export the project's request logs.";
        LogsContentManage = "logs.content.manage", Logs, "Switch content logging", true, false, false, [], "Switch content logging on or off for this project.";
        UsageView = "usage.view", UsageCosts, "View usage", false, false, false, [], "See the project's usage.";
        CostsView = "costs.view", UsageCosts, "View costs", false, false, false, [UsageView], "See the project's costs.";
        ProjectAuditLogView = "project.audit_log.view", Audit, "View project audit log", false, false, false, [], "Read the audit log of changes in this project.";
    }
);

rights!(
    InstanceRight, view = InstanceSystemView {
        InstanceSystemView = "instance.system.view", Instance, "View the system", false, false, false, [], "See the system and its model data, the instance settings and the update status.";
        InstanceSettingsManage = "instance.settings.manage", Instance, "Change instance settings", true, false, false, [], "Change general, mail, logging, time zone and account settings, and send a test email.";
        InstanceUpdatesManage = "instance.updates.manage", Instance, "Manage updates", true, false, false, [], "Check for updates and change the update settings.";
        InstanceUpdatesApply = "instance.updates.apply", Instance, "Apply updates", true, false, false, [InstanceUpdatesManage], "Install a new version of Kaeso.";
        InstanceUsersView = "instance.users.view", Instance, "View users", false, false, false, [], "See every account of the instance and its organizations.";
        InstanceUsersManage = "instance.users.manage", Instance, "Manage users", true, false, false, [InstanceUsersView], "Invite, deactivate and reactivate accounts, end their sessions and reset their two-factor.";
        InstanceOrgsView = "instance.orgs.view", Instance, "View organizations", false, false, false, [], "See every organization with its members, projects and usage.";
        InstanceOrgsManage = "instance.orgs.manage", Instance, "Manage organizations", true, false, false, [InstanceOrgsView], "Create and rename organizations, appoint owners and schedule or cancel their deletion.";
        InstanceAuditLogView = "instance.audit_log.view", Instance, "View instance audit log", false, false, false, [], "Read the instance audit log.";
    }
);

rights!(
    AccountRight, view = AccountView {
        AccountView = "account.view", Account, "View the account", false, false, false, [], "See the profile, organizations, notification settings and what's new.";
        AccountUpdate = "account.update", Account, "Edit the profile", false, false, false, [], "Change the name, picture, language and appearance.";
        AccountKeysView = "account.keys.view", Account, "View personal keys", false, false, false, [], "See the account's API keys in every project (metadata).";
        AccountKeysManage = "account.keys.manage", Account, "Manage personal keys", true, false, false, [AccountKeysView], "Disable, enable or archive the account's API keys.";
        AccountNotificationsManage = "account.notifications.manage", Account, "Change notifications", false, false, false, [], "Choose which emails the account gets.";
        AccountMembershipsLeave = "account.memberships.leave", Account, "Leave organizations", true, false, false, [], "Leave organizations the account belongs to.";
    }
);

impl OrgRight {
    pub const fn owner_only(self) -> bool {
        matches!(self, OrgRight::OrgOwnershipTransfer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Right {
    Org(OrgRight),
    Project(ProjectRight),
    Instance(InstanceRight),
    Account(AccountRight),
}

impl Right {
    pub const fn as_str(self) -> &'static str {
        match self {
            Right::Org(r) => r.as_str(),
            Right::Project(r) => r.as_str(),
            Right::Instance(r) => r.as_str(),
            Right::Account(r) => r.as_str(),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Right::Org(r) => r.label(),
            Right::Project(r) => r.label(),
            Right::Instance(r) => r.label(),
            Right::Account(r) => r.label(),
        }
    }

    pub const fn is_sensitive(self) -> bool {
        match self {
            Right::Org(r) => r.is_sensitive(),
            Right::Project(r) => r.is_sensitive(),
            Right::Instance(r) => r.is_sensitive(),
            Right::Account(r) => r.is_sensitive(),
        }
    }

    pub const fn needs_recent_sign_in(self) -> bool {
        match self {
            Right::Org(r) => r.needs_recent_sign_in(),
            Right::Project(r) => r.needs_recent_sign_in(),
            Right::Instance(_) | Right::Account(_) => false,
        }
    }

    pub const fn needs_session(self) -> bool {
        match self {
            Right::Org(r) => r.needs_session(),
            Right::Project(_) | Right::Instance(_) | Right::Account(_) => false,
        }
    }
}

impl OrgRight {
    pub const fn needs_session(self) -> bool {
        matches!(
            self,
            OrgRight::SecuritySsoManage | OrgRight::SecuritySsoRequire
        )
    }
}

impl fmt::Display for Right {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<OrgRight> for Right {
    fn from(r: OrgRight) -> Right {
        Right::Org(r)
    }
}

impl From<ProjectRight> for Right {
    fn from(r: ProjectRight) -> Right {
        Right::Project(r)
    }
}

impl From<InstanceRight> for Right {
    fn from(r: InstanceRight) -> Right {
        Right::Instance(r)
    }
}

impl From<AccountRight> for Right {
    fn from(r: AccountRight) -> Right {
        Right::Account(r)
    }
}

impl FromStr for Right {
    type Err = RightParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse()
            .map(Right::Org)
            .or_else(|_| s.parse().map(Right::Project))
            .or_else(|_| s.parse().map(Right::Instance))
            .or_else(|_| s.parse().map(Right::Account))
    }
}
