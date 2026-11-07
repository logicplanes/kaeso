use std::fmt;
use std::str::FromStr;

use crate::rights::{InstanceRight, OrgRight, ProjectRight, Right};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Area {
    Security,
    Personal,
    Project,
    Tools,
    Organization,
    Instance,
}

impl Area {
    pub const ALL: &'static [Area] = &[
        Area::Security,
        Area::Personal,
        Area::Project,
        Area::Tools,
        Area::Organization,
        Area::Instance,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Area::Security => "security",
            Area::Personal => "personal",
            Area::Project => "project",
            Area::Tools => "tools",
            Area::Organization => "organization",
            Area::Instance => "instance",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Area::Security => "Security",
            Area::Personal => "Personal",
            Area::Project => "Project",
            Area::Tools => "Tools",
            Area::Organization => "Organization",
            Area::Instance => "Instance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Delivery {
    Now,
    Daily,
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a delivery; use `now`, `daily` or `off`")]
pub struct DeliveryParseError(pub String);

impl Delivery {
    pub const ALL: &'static [Delivery] = &[Delivery::Now, Delivery::Daily, Delivery::Off];

    pub const fn as_str(self) -> &'static str {
        match self {
            Delivery::Now => "now",
            Delivery::Daily => "daily",
            Delivery::Off => "off",
        }
    }
}

impl fmt::Display for Delivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Delivery {
    type Err = DeliveryParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Delivery::ALL
            .iter()
            .copied()
            .find(|d| d.as_str() == s)
            .ok_or_else(|| DeliveryParseError(s.to_owned()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a notification; see the notification catalogue")]
pub struct KindParseError(pub String);

macro_rules! kinds {
    (
        $( $variant:ident = $str:literal, $area:ident, $name:literal, $default:ident, $always:literal, $scheduled:literal, [$($holder:expr),*], $description:literal; )*
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Kind {
            $(
                #[doc = $description]
                $variant,
            )*
        }

        impl Kind {
            pub const ALL: &'static [Kind] = &[$(Kind::$variant),*];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Kind::$variant => $str,)*
                }
            }

            pub const fn area(self) -> Area {
                match self {
                    $(Kind::$variant => Area::$area,)*
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Kind::$variant => $name,)*
                }
            }

            pub const fn description(self) -> &'static str {
                match self {
                    $(Kind::$variant => $description,)*
                }
            }

            pub const fn default_delivery(self) -> Delivery {
                match self {
                    $(Kind::$variant => Delivery::$default,)*
                }
            }

            pub const fn always(self) -> bool {
                match self {
                    $(Kind::$variant => $always,)*
                }
            }

            pub const fn scheduled(self) -> bool {
                match self {
                    $(Kind::$variant => $scheduled,)*
                }
            }

            pub fn rights(self) -> &'static [Right] {
                match self {
                    $(Kind::$variant => &[$($holder),*],)*
                }
            }
        }
    };
}

impl Kind {
    // An email that goes out on its own schedule is on (`now`, when it is due) or off; the
    // daily digest would only delay it to the same morning.
    pub const fn deliveries(self) -> &'static [Delivery] {
        if self.scheduled() {
            &[Delivery::Now, Delivery::Off]
        } else {
            Delivery::ALL
        }
    }

    pub fn allows(self, delivery: Delivery) -> bool {
        !self.always() && self.deliveries().contains(&delivery)
    }

    pub fn chosen(self, delivery: Option<Delivery>) -> Delivery {
        if self.always() {
            return Delivery::Now;
        }
        match delivery {
            Some(d) if self.deliveries().contains(&d) => d,
            Some(Delivery::Daily) if self.scheduled() => Delivery::Now,
            _ => self.default_delivery(),
        }
    }
}

kinds! {
    AccountSecurity = "account_security", Security, "Account security", Now, true, false, [], "Your password, two-factor authentication, email address or sessions changed.";
    NewApp = "new_app", Security, "A new app can act as you", Now, true, false, [], "You let an app or an MCP client act as you.";
    ExportReady = "export_ready", Personal, "Export finished", Now, false, false, [Right::Project(ProjectRight::LogsExport), Right::Org(OrgRight::AuditLogExport)], "A log or audit log export you started finished.";
    WeeklySummary = "weekly_summary", Personal, "Weekly summary", Off, false, true, [Right::Project(ProjectRight::UsageView)], "On Mondays: requests, cost, tool calls and top models of your projects.";
    AccessChanged = "access_changed", Project, "Your access changed", Now, false, false, [], "You were added or removed, or your rights changed.";
    BudgetAlerts = "budget_alerts", Project, "Budget alerts", Now, false, false, [Right::Project(ProjectRight::LimitsManage), Right::Project(ProjectRight::KeysOwnCreate)], "A budget you're responsible for crosses a threshold.";
    LimitReached = "limit_reached", Project, "Limit reached", Daily, false, false, [Right::Project(ProjectRight::LimitsManage), Right::Project(ProjectRight::KeysOwnCreate)], "A limit blocks requests of a project or of your keys.";
    KeyExpiry = "key_expiry", Project, "Your key expires", Now, false, false, [Right::Project(ProjectRight::KeysOwnCreate), Right::Project(ProjectRight::KeysAllManage)], "A key you own expires soon.";
    KeyChanged = "key_changed", Project, "Your key changed", Now, false, false, [Right::Project(ProjectRight::KeysOwnCreate)], "Someone else disabled, rotated or archived a key you own.";
    ContentLogging = "content_logging", Project, "Content logging changed", Daily, false, false, [Right::Project(ProjectRight::ProjectView)], "A project you're in starts or stops keeping prompts and replies.";
    McpServers = "mcp_servers", Tools, "MCP servers need attention", Now, false, false, [Right::Org(OrgRight::McpServersManage)], "A shared connection signed out, or a changed tool waits for review.";
    ConnectionExpired = "connection_expired", Tools, "Your connection expired", Now, false, false, [Right::Project(ProjectRight::ToolsUse)], "A server signed your own connection out.";
    PortalClients = "portal_clients", Tools, "New client on a portal", Daily, false, false, [Right::Project(ProjectRight::ToolsManage)], "Someone connected a new client to a project's MCP portal.";
    OrgSecurity = "org_security", Organization, "Security settings changed", Now, false, false, [Right::Org(OrgRight::SecuritySsoManage), Right::Org(OrgRight::PlatformKeysView)], "Someone else changed sign-in rules, single sign-on or platform keys.";
    OrgOwnership = "org_ownership", Organization, "Ownership and deletion", Now, true, false, [Right::Org(OrgRight::OrgOwnershipTransfer)], "An organization changes owners, is scheduled for deletion or is restored.";
    OrgMembers = "org_members", Organization, "Member changes", Daily, false, false, [Right::Org(OrgRight::MembersInvite)], "People join or leave an organization.";
    InviteAccepted = "invite_accepted", Organization, "Your invitation was accepted", Now, false, false, [Right::Org(OrgRight::MembersInvite)], "Someone you invited joined.";
    ProviderKeys = "provider_keys", Organization, "Provider keys need attention", Now, false, false, [Right::Org(OrgRight::ProviderCredentialsReplace)], "A provider rejects a key again and again, or an account signed out.";
    ModelChanges = "model_changes", Organization, "Model changes", Daily, false, false, [Right::Project(ProjectRight::ModelsManage)], "A model a project uses is delisted or changes its price.";
    CostReport = "cost_report", Organization, "Monthly cost report", Now, false, true, [Right::Org(OrgRight::CostsOrgView)], "On the 1st: last month's cost by project and model.";
    InstanceUpdates = "instance_updates", Instance, "Updates", Now, false, false, [Right::Instance(InstanceRight::InstanceUpdatesManage)], "A new version is out, or an update finished or was rolled back.";
    InstancePeople = "instance_people", Instance, "New organizations and people", Daily, false, false, [Right::Instance(InstanceRight::InstanceUsersView)], "Someone signed up or created an organization.";
    InstanceSettings = "instance_settings", Instance, "Instance settings changed", Now, false, false, [Right::Instance(InstanceRight::InstanceSettingsManage)], "Another admin changed the instance's settings, sign-in or agent policy.";
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Kind {
    type Err = KindParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Kind::ALL
            .iter()
            .copied()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| KindParseError(s.to_owned()))
    }
}
