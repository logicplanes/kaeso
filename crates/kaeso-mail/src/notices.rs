use crate::layout::{Content, Footnote, Link, Reason};
use crate::locale::{Locale, zh};
use crate::templates::{Actor, Budget, BudgetScope, Email, EmailKind, scope_en};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Because<'a> {
    AboutYou,
    Right(&'a str),
    Daily,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Links {
    pub settings: String,
    pub unsubscribe: Option<Unsubscribe>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsubscribe {
    pub page: String,
    pub one_click: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    Logs,
    AuditLog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access<'a> {
    Added(&'a str),
    Removed,
    Changed,
    InstanceAdmin(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyChange {
    Disabled,
    Rotated,
    Archived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security<'a> {
    SignInRules,
    SingleSignOn,
    AgentPolicy,
    KeyPolicy,
    PlatformKeyCreated(&'a str),
    PlatformKeyRotated(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership<'a> {
    Joined,
    JoinedBySso,
    Left,
    Removed(Actor<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKey {
    Refused,
    SignedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelChange {
    Delisted,
    Price,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    Available,
    Finished,
    RolledBack,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instance<'a> {
    Settings,
    AdminSso,
    AgentPolicy,
    OrgCreated(&'a str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectUse {
    pub name: String,
    pub requests: Option<String>,
    pub cost: String,
}

impl ProjectUse {
    fn line(&self, lang: Locale) -> String {
        match (lang, &self.requests) {
            (Locale::En, Some(r)) => format!("{}: {r} requests, {}", self.name, self.cost),
            (Locale::De, Some(r)) => format!("{}: {r} Anfragen, {}", self.name, self.cost),
            (Locale::ZhCn, Some(r)) => zh(&[&self.name, "：", r, " 次请求，", &self.cost]),
            (Locale::En | Locale::De, None) => format!("{}: {}", self.name, self.cost),
            (Locale::ZhCn, None) => zh(&[&self.name, "：", &self.cost]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fact<'a> {
    NewApp {
        app: &'a str,
    },
    ExportReady(Export),
    WeeklySummary {
        requests: String,
        tool_calls: String,
        cost: String,
        projects: Vec<ProjectUse>,
        top_models: Vec<String>,
    },
    AccessChanged {
        change: Access<'a>,
        by: Actor<'a>,
    },
    LimitReached {
        budget: &'a Budget,
        scope: BudgetScope<'a>,
    },
    KeyChanged {
        key: &'a str,
        change: KeyChange,
        by: Actor<'a>,
    },
    ContentLogging {
        on: bool,
        by: Actor<'a>,
    },
    ServerSignedOut {
        server: &'a str,
    },
    ToolHeld {
        server: &'a str,
        tool: &'a str,
    },
    ConnectionExpired {
        server: &'a str,
    },
    PortalClient {
        client: &'a str,
        by: Actor<'a>,
    },
    OrgSecurity {
        change: Security<'a>,
        by: Actor<'a>,
    },
    Member {
        person: &'a str,
        change: Membership<'a>,
    },
    InviteAccepted {
        person: &'a str,
    },
    ProviderKey {
        key: &'a str,
        change: ProviderKey,
    },
    Model {
        model: &'a str,
        change: ModelChange,
    },
    CostReport {
        month: &'a str,
        total: &'a str,
        projects: Vec<ProjectUse>,
    },
    InstanceUpdate {
        version: &'a str,
        change: Update,
    },
    SignedUp {
        person: &'a str,
    },
    InstanceChanged {
        change: Instance<'a>,
        by: Actor<'a>,
    },
}

struct Words {
    heading: String,
    sentence: String,
    button: &'static str,
}

fn top_models(lang: Locale, models: &[String]) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    Some(match lang {
        Locale::En => format!("Top models: {}.", models.join(", ")),
        Locale::De => format!("Meistgenutzte Modelle: {}.", models.join(", ")),
        Locale::ZhCn => zh(&["常用模型：", &models.join("、"), "。"]),
    })
}

impl Fact<'_> {
    pub fn kind(&self) -> EmailKind {
        match self {
            Fact::NewApp { .. } => EmailKind::NewApp,
            Fact::ExportReady(_) => EmailKind::ExportReady,
            Fact::WeeklySummary { .. } => EmailKind::WeeklySummary,
            Fact::AccessChanged { .. } => EmailKind::AccessChanged,
            Fact::LimitReached { .. } => EmailKind::LimitReached,
            Fact::KeyChanged { .. } => EmailKind::KeyChanged,
            Fact::ContentLogging { .. } => EmailKind::ContentLogging,
            Fact::ServerSignedOut { .. } | Fact::ToolHeld { .. } => EmailKind::McpServers,
            Fact::ConnectionExpired { .. } => EmailKind::ConnectionExpired,
            Fact::PortalClient { .. } => EmailKind::PortalClient,
            Fact::OrgSecurity { .. } => EmailKind::OrgSecurity,
            Fact::Member { .. } => EmailKind::MemberChanges,
            Fact::InviteAccepted { .. } => EmailKind::InviteAccepted,
            Fact::ProviderKey { .. } => EmailKind::ProviderKeys,
            Fact::Model { .. } => EmailKind::ModelChanges,
            Fact::CostReport { .. } => EmailKind::CostReport,
            Fact::InstanceUpdate { .. } => EmailKind::InstanceUpdates,
            Fact::SignedUp { .. } => EmailKind::InstancePeople,
            Fact::InstanceChanged {
                change: Instance::OrgCreated(_),
                ..
            } => EmailKind::InstancePeople,
            Fact::InstanceChanged { .. } => EmailKind::InstanceSettings,
        }
    }

    fn items(&self, lang: Locale) -> Vec<String> {
        match self {
            Fact::WeeklySummary { projects, .. } | Fact::CostReport { projects, .. } => {
                projects.iter().map(|p| p.line(lang)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn words(&self, lang: Locale) -> Words {
        match lang {
            Locale::En => self.en(),
            Locale::De => self.de(),
            Locale::ZhCn => self.zh(),
        }
    }

    fn en(&self) -> Words {
        let w = |heading: String, sentence: String, button| Words {
            heading,
            sentence,
            button,
        };
        let who = |a: &Actor<'_>| a.name(Locale::En);
        match self {
            Fact::NewApp { app } => w(
                format!("{app} can act as you"),
                format!("You let {app} act as you; if you didn't, disconnect it now."),
                "Review access",
            ),
            Fact::ExportReady(e) => {
                let what = match e {
                    Export::Logs => "request logs",
                    Export::AuditLog => "audit log",
                };
                w(
                    "Your export finished".to_owned(),
                    format!("The export of the {what} you started finished."),
                    "Open Kaeso",
                )
            }
            Fact::WeeklySummary {
                requests,
                tool_calls,
                cost,
                ..
            } => w(
                "Your week in Kaeso".to_owned(),
                format!(
                    "Last week your projects sent {requests} requests and {tool_calls} tool calls for {cost}."
                ),
                "Open usage",
            ),
            Fact::AccessChanged { change, by } => {
                let (heading, sentence) = match change {
                    Access::Added(role) => (
                        "You were added".to_owned(),
                        format!("{} added you as {role}.", who(by)),
                    ),
                    Access::Removed => (
                        "You were removed".to_owned(),
                        format!("{} removed you; your keys there are archived.", who(by)),
                    ),
                    Access::Changed => (
                        "Your rights changed".to_owned(),
                        format!("{} changed your rights.", who(by)),
                    ),
                    Access::InstanceAdmin(true) => (
                        "You're an instance admin".to_owned(),
                        format!("{} made you an instance admin.", who(by)),
                    ),
                    Access::InstanceAdmin(false) => (
                        "You're no longer an instance admin".to_owned(),
                        format!("{} took away your instance admin rights.", who(by)),
                    ),
                };
                w(heading, sentence, "Open Kaeso")
            }
            Fact::LimitReached { budget, scope } => w(
                "A budget is used up".to_owned(),
                format!(
                    "{} used all of its {}; requests over it are refused.",
                    scope_en(*scope),
                    budget.english()
                ),
                "Open limits",
            ),
            Fact::KeyChanged { key, change, by } => {
                let (heading, verb) = match change {
                    KeyChange::Disabled => ("Your key was disabled", "disabled"),
                    KeyChange::Rotated => ("Your key was rotated", "rotated"),
                    KeyChange::Archived => ("Your key was archived", "archived"),
                };
                w(
                    heading.to_owned(),
                    format!("{} {verb} your key {key}.", who(by)),
                    "Open keys",
                )
            }
            Fact::ContentLogging { on, by } => {
                let (heading, sentence) = if *on {
                    (
                        "Content logging is on",
                        format!(
                            "{} turned content logging on; new prompts and replies are kept.",
                            who(by)
                        ),
                    )
                } else {
                    (
                        "Content logging is off",
                        format!(
                            "{} turned content logging off; new requests keep only metadata.",
                            who(by)
                        ),
                    )
                };
                w(heading.to_owned(), sentence, "Open request logs")
            }
            Fact::ServerSignedOut { server } => w(
                format!("{server} signed Kaeso out"),
                "Its tools stop working until someone signs in again.".to_owned(),
                "Open MCP servers",
            ),
            Fact::ToolHeld { server, tool } => w(
                format!("{tool} changed"),
                format!("{tool} on {server} changed; no project can call it until it's approved."),
                "Review the change",
            ),
            Fact::ConnectionExpired { server } => w(
                format!("{server} signed you out"),
                "Its tools don't work for you until you connect your account again.".to_owned(),
                "Open connections",
            ),
            Fact::PortalClient { client, by } => w(
                format!("{client} was connected"),
                format!(
                    "{} connected {client} to the portal; it can call tools as them.",
                    who(by)
                ),
                "Open connected clients",
            ),
            Fact::OrgSecurity { change, by } => {
                let (heading, what) = match change {
                    Security::SignInRules => (
                        "Sign-in rules changed".to_owned(),
                        "changed the sign-in rules".to_owned(),
                    ),
                    Security::SingleSignOn => (
                        "Single sign-on changed".to_owned(),
                        "changed single sign-on".to_owned(),
                    ),
                    Security::AgentPolicy => (
                        "The agent policy changed".to_owned(),
                        "changed the agent policy".to_owned(),
                    ),
                    Security::KeyPolicy => (
                        "The key policy changed".to_owned(),
                        "changed the key policy".to_owned(),
                    ),
                    Security::PlatformKeyCreated(name) => (
                        "A platform key was created".to_owned(),
                        format!("created the platform key {name}"),
                    ),
                    Security::PlatformKeyRotated(name) => (
                        "A platform key was rotated".to_owned(),
                        format!("rotated the platform key {name}"),
                    ),
                };
                w(heading, format!("{} {what}.", who(by)), "Open security")
            }
            Fact::Member { person, change } => {
                let (heading, sentence) = match change {
                    Membership::Joined => (
                        format!("{person} joined"),
                        format!("{person} joined the organization."),
                    ),
                    Membership::JoinedBySso => (
                        format!("{person} joined"),
                        format!("{person} joined the organization through single sign-on."),
                    ),
                    Membership::Left => (
                        format!("{person} left"),
                        format!("{person} left the organization."),
                    ),
                    Membership::Removed(by) => (
                        format!("{person} was removed"),
                        format!("{} removed {person} from the organization.", who(by)),
                    ),
                };
                w(heading, sentence, "Open members")
            }
            Fact::InviteAccepted { person } => w(
                format!("{person} accepted"),
                format!("{person} accepted your invitation and joined."),
                "Open members",
            ),
            Fact::ProviderKey { key, change } => match change {
                ProviderKey::Refused => w(
                    format!("{key} keeps being refused"),
                    format!("The provider refused {key} three times in a row; replace it."),
                    "Open provider keys",
                ),
                ProviderKey::SignedOut => w(
                    format!("{key} signed out"),
                    format!("The account {key} signed out; sign in to it again."),
                    "Open provider keys",
                ),
            },
            Fact::Model { model, change } => match change {
                ModelChange::Delisted => w(
                    format!("{model} is delisted"),
                    format!("The provider delisted {model}; requests to it may fail."),
                    "Open models",
                ),
                ModelChange::Price => w(
                    format!("{model} changed its price"),
                    format!("{model} has a new price; costs from now on use it."),
                    "Open models",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                format!("{month} cost {total}"),
                "These projects cost the most last month.".to_owned(),
                "Open usage",
            ),
            Fact::InstanceUpdate { version, change } => match change {
                Update::Available => w(
                    format!("Kaeso {version} is out"),
                    format!("Kaeso {version} is ready to install."),
                    "Open updates",
                ),
                Update::Finished => w(
                    format!("Kaeso runs {version}"),
                    format!("The update to Kaeso {version} finished."),
                    "Open updates",
                ),
                Update::RolledBack => w(
                    format!("The update to {version} failed"),
                    format!("The update to Kaeso {version} failed and was rolled back."),
                    "Open updates",
                ),
                Update::Failed => w(
                    format!("The update to {version} failed"),
                    format!(
                        "The update to Kaeso {version} failed; the updates page says what to do."
                    ),
                    "Open updates",
                ),
            },
            Fact::SignedUp { person } => w(
                format!("{person} signed up"),
                format!("{person} created an account."),
                "Open users",
            ),
            Fact::InstanceChanged { change, by } => {
                let (heading, sentence) = match change {
                    Instance::Settings => (
                        "Instance settings changed".to_owned(),
                        format!("{} changed the instance settings.", who(by)),
                    ),
                    Instance::AdminSso => (
                        "Admin sign-in changed".to_owned(),
                        format!("{} changed single sign-on for instance admins.", who(by)),
                    ),
                    Instance::AgentPolicy => (
                        "The agent policy changed".to_owned(),
                        format!("{} changed the instance's agent policy.", who(by)),
                    ),
                    Instance::OrgCreated(name) => (
                        format!("{name} was created"),
                        format!("{} created the organization {name}.", who(by)),
                    ),
                };
                w(heading, sentence, "Open instance")
            }
        }
    }

    fn de(&self) -> Words {
        let w = |heading: String, sentence: String, button| Words {
            heading,
            sentence,
            button,
        };
        let who = |a: &Actor<'_>| a.name(Locale::De);
        match self {
            Fact::NewApp { app } => w(
                format!("{app} kann als Sie handeln"),
                format!(
                    "Sie haben {app} erlaubt, als Sie zu handeln; trennen Sie es, falls nicht."
                ),
                "Zugriff prüfen",
            ),
            Fact::ExportReady(e) => {
                let what = match e {
                    Export::Logs => "der Anfrageprotokolle",
                    Export::AuditLog => "des Audit-Logs",
                };
                w(
                    "Ihr Export ist abgeschlossen".to_owned(),
                    format!("Der von Ihnen gestartete Export {what} ist abgeschlossen."),
                    "Kaeso öffnen",
                )
            }
            Fact::WeeklySummary {
                requests,
                tool_calls,
                cost,
                ..
            } => w(
                "Ihre Woche in Kaeso".to_owned(),
                format!(
                    "Ihre Projekte sendeten letzte Woche {requests} Anfragen und {tool_calls} Tool-Aufrufe für {cost}."
                ),
                "Nutzung öffnen",
            ),
            Fact::AccessChanged { change, by } => {
                let (heading, sentence) = match change {
                    Access::Added(role) => (
                        "Sie wurden hinzugefügt".to_owned(),
                        format!("{} hat Sie als {role} hinzugefügt.", who(by)),
                    ),
                    Access::Removed => (
                        "Sie wurden entfernt".to_owned(),
                        format!(
                            "{} hat Sie entfernt; Ihre Schlüssel dort sind archiviert.",
                            who(by)
                        ),
                    ),
                    Access::Changed => (
                        "Ihre Rechte haben sich geändert".to_owned(),
                        format!("{} hat Ihre Rechte geändert.", who(by)),
                    ),
                    Access::InstanceAdmin(true) => (
                        "Sie sind Instanz-Admin".to_owned(),
                        format!("{} hat Sie zum Instanz-Admin gemacht.", who(by)),
                    ),
                    Access::InstanceAdmin(false) => (
                        "Sie sind kein Instanz-Admin mehr".to_owned(),
                        format!(
                            "{} hat Ihnen die Rechte als Instanz-Admin entzogen.",
                            who(by)
                        ),
                    ),
                };
                w(heading, sentence, "Kaeso öffnen")
            }
            Fact::LimitReached { budget, scope } => {
                let scope = match scope {
                    BudgetScope::Project => "Das Projekt".to_owned(),
                    BudgetScope::Member => "Ein Mitglied des Projekts".to_owned(),
                    BudgetScope::Key(id) => format!("Der API-Schlüssel {id}"),
                };
                w(
                    "Ein Budget ist aufgebraucht".to_owned(),
                    format!(
                        "{scope} hat das gesamte {} verbraucht; weitere Anfragen werden abgelehnt.",
                        budget.german_genitive()
                    ),
                    "Limits öffnen",
                )
            }
            Fact::KeyChanged { key, change, by } => {
                let (heading, verb) = match change {
                    KeyChange::Disabled => ("Ihr Schlüssel wurde deaktiviert", "deaktiviert"),
                    KeyChange::Rotated => ("Ihr Schlüssel wurde rotiert", "rotiert"),
                    KeyChange::Archived => ("Ihr Schlüssel wurde archiviert", "archiviert"),
                };
                w(
                    heading.to_owned(),
                    format!("{} hat Ihren Schlüssel {key} {verb}.", who(by)),
                    "Schlüssel öffnen",
                )
            }
            Fact::ContentLogging { on, by } => {
                let (heading, sentence) = if *on {
                    (
                        "Inhalte werden protokolliert",
                        format!(
                            "{} hat die Inhaltsprotokollierung eingeschaltet; neue Prompts werden gespeichert.",
                            who(by)
                        ),
                    )
                } else {
                    (
                        "Inhalte werden nicht mehr protokolliert",
                        format!(
                            "{} hat die Inhaltsprotokollierung ausgeschaltet; nur Metadaten bleiben.",
                            who(by)
                        ),
                    )
                };
                w(heading.to_owned(), sentence, "Anfrageprotokolle öffnen")
            }
            Fact::ServerSignedOut { server } => w(
                format!("{server} hat Kaeso abgemeldet"),
                "Seine Tools funktionieren erst wieder, wenn sich jemand erneut anmeldet."
                    .to_owned(),
                "MCP-Server öffnen",
            ),
            Fact::ToolHeld { server, tool } => w(
                format!("{tool} hat sich geändert"),
                format!(
                    "{tool} auf {server} hat sich geändert; kein Projekt ruft es vor der Freigabe auf."
                ),
                "Änderung prüfen",
            ),
            Fact::ConnectionExpired { server } => w(
                format!("{server} hat Sie abgemeldet"),
                "Seine Tools funktionieren für Sie erst wieder, wenn Sie Ihr Konto neu verbinden."
                    .to_owned(),
                "Verbindungen öffnen",
            ),
            Fact::PortalClient { client, by } => w(
                format!("{client} wurde verbunden"),
                format!(
                    "{} hat {client} mit dem Portal verbunden; es ruft Tools als diese Person auf.",
                    who(by)
                ),
                "Verbundene Clients öffnen",
            ),
            Fact::OrgSecurity { change, by } => {
                let (heading, what) = match change {
                    Security::SignInRules => (
                        "Anmelderegeln geändert".to_owned(),
                        "hat die Anmelderegeln geändert".to_owned(),
                    ),
                    Security::SingleSignOn => (
                        "Single Sign-on geändert".to_owned(),
                        "hat Single Sign-on geändert".to_owned(),
                    ),
                    Security::AgentPolicy => (
                        "Agenten-Richtlinie geändert".to_owned(),
                        "hat die Agenten-Richtlinie geändert".to_owned(),
                    ),
                    Security::KeyPolicy => (
                        "Schlüsselrichtlinie geändert".to_owned(),
                        "hat die Schlüsselrichtlinie geändert".to_owned(),
                    ),
                    Security::PlatformKeyCreated(name) => (
                        "Plattformschlüssel erstellt".to_owned(),
                        format!("hat den Plattformschlüssel {name} erstellt"),
                    ),
                    Security::PlatformKeyRotated(name) => (
                        "Plattformschlüssel rotiert".to_owned(),
                        format!("hat den Plattformschlüssel {name} rotiert"),
                    ),
                };
                w(heading, format!("{} {what}.", who(by)), "Sicherheit öffnen")
            }
            Fact::Member { person, change } => {
                let (heading, sentence) = match change {
                    Membership::Joined => (
                        format!("{person} ist beigetreten"),
                        format!("{person} ist der Organisation beigetreten."),
                    ),
                    Membership::JoinedBySso => (
                        format!("{person} ist beigetreten"),
                        format!("{person} ist der Organisation über Single Sign-on beigetreten."),
                    ),
                    Membership::Left => (
                        format!("{person} ist ausgetreten"),
                        format!("{person} hat die Organisation verlassen."),
                    ),
                    Membership::Removed(by) => (
                        format!("{person} wurde entfernt"),
                        format!("{} hat {person} aus der Organisation entfernt.", who(by)),
                    ),
                };
                w(heading, sentence, "Mitglieder öffnen")
            }
            Fact::InviteAccepted { person } => w(
                format!("{person} hat angenommen"),
                format!("{person} hat Ihre Einladung angenommen und ist beigetreten."),
                "Mitglieder öffnen",
            ),
            Fact::ProviderKey { key, change } => match change {
                ProviderKey::Refused => w(
                    format!("{key} wird abgelehnt"),
                    format!(
                        "Der Anbieter hat {key} dreimal nacheinander abgelehnt; ersetzen Sie ihn."
                    ),
                    "Anbieter-Schlüssel öffnen",
                ),
                ProviderKey::SignedOut => w(
                    format!("{key} ist abgemeldet"),
                    format!("Das Konto {key} wurde abgemeldet; melden Sie sich erneut an."),
                    "Anbieter-Schlüssel öffnen",
                ),
            },
            Fact::Model { model, change } => match change {
                ModelChange::Delisted => w(
                    format!("{model} wird eingestellt"),
                    format!(
                        "Der Anbieter hat {model} eingestellt; Anfragen daran können fehlschlagen."
                    ),
                    "Modelle öffnen",
                ),
                ModelChange::Price => w(
                    format!("{model} hat einen neuen Preis"),
                    format!("{model} hat einen neuen Preis; Kosten ab jetzt nutzen ihn."),
                    "Modelle öffnen",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                format!("{month} kostete {total}"),
                "Diese Projekte haben im letzten Monat am meisten gekostet.".to_owned(),
                "Nutzung öffnen",
            ),
            Fact::InstanceUpdate { version, change } => match change {
                Update::Available => w(
                    format!("Kaeso {version} ist erschienen"),
                    format!("Kaeso {version} kann installiert werden."),
                    "Updates öffnen",
                ),
                Update::Finished => w(
                    format!("Kaeso läuft mit {version}"),
                    format!("Das Update auf Kaeso {version} ist abgeschlossen."),
                    "Updates öffnen",
                ),
                Update::RolledBack => w(
                    format!("Das Update auf {version} schlug fehl"),
                    format!(
                        "Das Update auf Kaeso {version} ist fehlgeschlagen und wurde zurückgerollt."
                    ),
                    "Updates öffnen",
                ),
                Update::Failed => w(
                    format!("Das Update auf {version} schlug fehl"),
                    format!(
                        "Das Update auf Kaeso {version} ist fehlgeschlagen; die Update-Seite nennt die Schritte."
                    ),
                    "Updates öffnen",
                ),
            },
            Fact::SignedUp { person } => w(
                format!("{person} hat sich registriert"),
                format!("{person} hat ein Konto erstellt."),
                "Benutzer öffnen",
            ),
            Fact::InstanceChanged { change, by } => {
                let (heading, sentence) = match change {
                    Instance::Settings => (
                        "Instanzeinstellungen geändert".to_owned(),
                        format!("{} hat die Instanzeinstellungen geändert.", who(by)),
                    ),
                    Instance::AdminSso => (
                        "Admin-Anmeldung geändert".to_owned(),
                        format!(
                            "{} hat Single Sign-on für Instanz-Admins geändert.",
                            who(by)
                        ),
                    ),
                    Instance::AgentPolicy => (
                        "Agenten-Richtlinie geändert".to_owned(),
                        format!(
                            "{} hat die Agenten-Richtlinie der Instanz geändert.",
                            who(by)
                        ),
                    ),
                    Instance::OrgCreated(name) => (
                        format!("{name} wurde erstellt"),
                        format!("{} hat die Organisation {name} erstellt.", who(by)),
                    ),
                };
                w(heading, sentence, "Instanz öffnen")
            }
        }
    }

    fn zh(&self) -> Words {
        let w = |heading: String, sentence: String, button| Words {
            heading,
            sentence,
            button,
        };
        let who = |a: &Actor<'_>| a.name(Locale::ZhCn);
        match self {
            Fact::NewApp { app } => w(
                zh(&[app, "可以代表您操作"]),
                zh(&[
                    "您已允许 ",
                    app,
                    " 代表您操作；如果不是您允许的，请立即断开它。",
                ]),
                "检查访问权限",
            ),
            Fact::ExportReady(e) => {
                let what = match e {
                    Export::Logs => "请求日志",
                    Export::AuditLog => "审计日志",
                };
                w(
                    "您的导出已完成".to_owned(),
                    zh(&["您发起的", what, "导出已完成。"]),
                    "打开 Kaeso",
                )
            }
            Fact::WeeklySummary {
                requests,
                tool_calls,
                cost,
                ..
            } => w(
                "您在 Kaeso 的一周".to_owned(),
                zh(&[
                    "上周您的项目发送了 ",
                    requests,
                    " 次请求和 ",
                    tool_calls,
                    " 次工具调用，成本为 ",
                    cost,
                    "。",
                ]),
                "打开用量",
            ),
            Fact::AccessChanged { change, by } => {
                let by = who(by);
                let (heading, sentence) = match change {
                    Access::Added(role) => (
                        "您已被添加".to_owned(),
                        zh(&[&by, "已将您添加为", role, "。"]),
                    ),
                    Access::Removed => (
                        "您已被移除".to_owned(),
                        zh(&[&by, "已将您移除；您在那里的密钥已归档。"]),
                    ),
                    Access::Changed => {
                        ("您的权限已变更".to_owned(), zh(&[&by, "更改了您的权限。"]))
                    }
                    Access::InstanceAdmin(true) => (
                        "您已成为实例管理员".to_owned(),
                        zh(&[&by, "已将您设为实例管理员。"]),
                    ),
                    Access::InstanceAdmin(false) => (
                        "您不再是实例管理员".to_owned(),
                        zh(&[&by, "取消了您的实例管理员身份。"]),
                    ),
                };
                w(heading, sentence, "打开 Kaeso")
            }
            Fact::LimitReached { budget, scope } => {
                let whose = match scope {
                    BudgetScope::Project => "该项目的".to_owned(),
                    BudgetScope::Member => "项目中一名成员的".to_owned(),
                    BudgetScope::Key(id) => zh(&["API 密钥", id, "的"]),
                };
                w(
                    "预算已用完".to_owned(),
                    zh(&[
                        &whose,
                        &budget.chinese(),
                        "已全部用完；超出的请求会被拒绝。",
                    ]),
                    "打开限额",
                )
            }
            Fact::KeyChanged { key, change, by } => {
                let (heading, verb) = match change {
                    KeyChange::Disabled => ("您的密钥已停用", "停用了"),
                    KeyChange::Rotated => ("您的密钥已轮换", "轮换了"),
                    KeyChange::Archived => ("您的密钥已归档", "归档了"),
                };
                w(
                    heading.to_owned(),
                    zh(&[&who(by), verb, "您的密钥 ", key, "。"]),
                    "打开密钥",
                )
            }
            Fact::ContentLogging { on, by } => {
                let (heading, sentence) = if *on {
                    (
                        "内容记录已开启",
                        zh(&[&who(by), "开启了内容记录；新的提示词和回复会被保存。"]),
                    )
                } else {
                    (
                        "内容记录已关闭",
                        zh(&[&who(by), "关闭了内容记录；新请求只保存元数据。"]),
                    )
                };
                w(heading.to_owned(), sentence, "打开请求日志")
            }
            Fact::ServerSignedOut { server } => w(
                zh(&[server, "已让 Kaeso 退出登录"]),
                "在有人重新登录之前，它的工具无法使用。".to_owned(),
                "打开 MCP 服务器",
            ),
            Fact::ToolHeld { server, tool } => w(
                zh(&[tool, "已变更"]),
                zh(&[
                    server,
                    " 上的 ",
                    tool,
                    " 已变更；在批准之前，任何项目都无法调用它。",
                ]),
                "审核变更",
            ),
            Fact::ConnectionExpired { server } => w(
                zh(&[server, "已让您退出登录"]),
                "在您重新连接账户之前，您无法使用它的工具。".to_owned(),
                "打开连接",
            ),
            Fact::PortalClient { client, by } => w(
                zh(&[client, "已连接"]),
                zh(&[
                    &who(by),
                    "已将 ",
                    client,
                    " 连接到门户；它会以该用户的身份调用工具。",
                ]),
                "打开已连接的客户端",
            ),
            Fact::OrgSecurity { change, by } => {
                let (heading, what) = match change {
                    Security::SignInRules => {
                        ("登录规则已变更".to_owned(), "更改了登录规则".to_owned())
                    }
                    Security::SingleSignOn => {
                        ("单点登录已变更".to_owned(), "更改了单点登录".to_owned())
                    }
                    Security::AgentPolicy => {
                        ("智能体策略已变更".to_owned(), "更改了智能体策略".to_owned())
                    }
                    Security::KeyPolicy => {
                        ("密钥策略已变更".to_owned(), "更改了密钥策略".to_owned())
                    }
                    Security::PlatformKeyCreated(name) => {
                        ("已创建平台密钥".to_owned(), zh(&["创建了平台密钥 ", name]))
                    }
                    Security::PlatformKeyRotated(name) => {
                        ("已轮换平台密钥".to_owned(), zh(&["轮换了平台密钥 ", name]))
                    }
                };
                w(heading, zh(&[&who(by), &what, "。"]), "打开安全设置")
            }
            Fact::Member { person, change } => {
                let (heading, sentence) = match change {
                    Membership::Joined => (zh(&[person, "已加入"]), zh(&[person, "已加入组织。"])),
                    Membership::JoinedBySso => (
                        zh(&[person, "已加入"]),
                        zh(&[person, "已通过单点登录加入组织。"]),
                    ),
                    Membership::Left => (zh(&[person, "已离开"]), zh(&[person, "已离开组织。"])),
                    Membership::Removed(by) => (
                        zh(&[person, "已被移除"]),
                        zh(&[&who(by), "已将 ", person, " 移出组织。"]),
                    ),
                };
                w(heading, sentence, "打开成员")
            }
            Fact::InviteAccepted { person } => w(
                zh(&[person, "已接受邀请"]),
                zh(&[person, "已接受您的邀请并加入。"]),
                "打开成员",
            ),
            Fact::ProviderKey { key, change } => match change {
                ProviderKey::Refused => w(
                    zh(&[key, "一再被拒绝"]),
                    zh(&["服务商连续三次拒绝了 ", key, "；请替换它。"]),
                    "打开服务商密钥",
                ),
                ProviderKey::SignedOut => w(
                    zh(&[key, "已退出登录"]),
                    zh(&["账户 ", key, " 已退出登录；请重新登录。"]),
                    "打开服务商密钥",
                ),
            },
            Fact::Model { model, change } => match change {
                ModelChange::Delisted => w(
                    zh(&[model, "已下架"]),
                    zh(&["服务商已下架 ", model, "；对它的请求可能失败。"]),
                    "打开模型",
                ),
                ModelChange::Price => w(
                    zh(&[model, "的价格已变更"]),
                    zh(&[model, " 有了新价格；此后的成本按新价格计算。"]),
                    "打开模型",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                zh(&[month, "的成本为 ", total]),
                "以下是上个月成本最高的项目。".to_owned(),
                "打开用量",
            ),
            Fact::InstanceUpdate { version, change } => match change {
                Update::Available => w(
                    zh(&["Kaeso ", version, " 已发布"]),
                    zh(&["Kaeso ", version, " 可以安装了。"]),
                    "打开更新",
                ),
                Update::Finished => w(
                    zh(&["Kaeso 已运行 ", version]),
                    zh(&["到 Kaeso ", version, " 的更新已完成。"]),
                    "打开更新",
                ),
                Update::RolledBack => w(
                    zh(&["到 ", version, " 的更新失败"]),
                    zh(&["到 Kaeso ", version, " 的更新失败，已回滚。"]),
                    "打开更新",
                ),
                Update::Failed => w(
                    zh(&["到 ", version, " 的更新失败"]),
                    zh(&[
                        "到 Kaeso ",
                        version,
                        " 的更新失败；更新页面说明了该怎么做。",
                    ]),
                    "打开更新",
                ),
            },
            Fact::SignedUp { person } => w(
                zh(&[person, "已注册"]),
                zh(&[person, "创建了账户。"]),
                "打开用户",
            ),
            Fact::InstanceChanged { change, by } => {
                let by = who(by);
                let (heading, sentence) = match change {
                    Instance::Settings => {
                        ("实例设置已变更".to_owned(), zh(&[&by, "更改了实例设置。"]))
                    }
                    Instance::AdminSso => (
                        "管理员登录方式已变更".to_owned(),
                        zh(&[&by, "更改了实例管理员的单点登录。"]),
                    ),
                    Instance::AgentPolicy => (
                        "智能体策略已变更".to_owned(),
                        zh(&[&by, "更改了实例的智能体策略。"]),
                    ),
                    Instance::OrgCreated(name) => {
                        (zh(&["已创建", name]), zh(&[&by, "创建了组织 ", name, "。"]))
                    }
                };
                w(heading, sentence, "打开实例")
            }
        }
    }
}

fn because(lang: Locale, why: Because<'_>) -> String {
    match (lang, why) {
        (Locale::En, Because::AboutYou) => {
            "You get this email because it's about your account.".to_owned()
        }
        (Locale::En, Because::Right(r)) => format!("You get this email because you hold {r}."),
        (Locale::En, Because::Daily) => {
            "You get this email because you chose daily emails.".to_owned()
        }
        (Locale::De, Because::AboutYou) => {
            "Sie erhalten diese E-Mail, weil sie Ihr Konto betrifft.".to_owned()
        }
        (Locale::De, Because::Right(r)) => {
            format!("Sie erhalten diese E-Mail, weil Sie {r} haben.")
        }
        (Locale::De, Because::Daily) => {
            "Sie erhalten diese E-Mail, weil Sie tägliche E-Mails gewählt haben.".to_owned()
        }
        (Locale::ZhCn, Because::AboutYou) => "您收到此邮件，是因为它与您的账户有关。".to_owned(),
        (Locale::ZhCn, Because::Right(r)) => zh(&["您收到此邮件，是因为您拥有 ", r, "。"]),
        (Locale::ZhCn, Because::Daily) => "您收到此邮件，是因为您选择了每日邮件。".to_owned(),
    }
}

fn footnote(lang: Locale, why: Because<'_>, links: &Links) -> Footnote {
    let (settings, unsubscribe) = match lang {
        Locale::En => ("Choose your emails", "Unsubscribe"),
        Locale::De => ("E-Mails wählen", "Abmelden"),
        Locale::ZhCn => ("选择邮件", "退订"),
    };
    let mut out = vec![Link {
        label: settings,
        url: links.settings.clone(),
    }];
    if let Some(u) = &links.unsubscribe {
        out.push(Link {
            label: unsubscribe,
            url: u.page.clone(),
        });
    }
    Footnote {
        because: because(lang, why),
        links: out,
    }
}

fn subject(place: Option<&str>, heading: &str) -> String {
    match place {
        Some(p) => format!("[{p}] {heading}"),
        None => heading.to_owned(),
    }
}

pub fn notice(
    lang: Locale,
    to: &str,
    place: Option<&str>,
    fact: &Fact<'_>,
    why: Because<'_>,
    open: &str,
    links: &Links,
) -> Email {
    let words = fact.words(lang);
    let mut content = Content::new(lang, words.heading.clone(), Reason::Notice)
        .sentence(words.sentence)
        .items(fact.items(lang))
        .action(words.button, open)
        .footnote(footnote(lang, why, links));
    if let Fact::WeeklySummary {
        top_models: models, ..
    } = fact
        && let Some(note) = top_models(lang, models)
    {
        content = content.note(note);
    }
    Email::new(to, fact.kind(), subject(place, &words.heading), content)
        .list_unsubscribe(links.unsubscribe.as_ref().map(|u| u.one_click.clone()))
}

pub fn line(lang: Locale, place: Option<&str>, fact: &Fact<'_>) -> String {
    subject(place, &fact.words(lang).heading)
}

pub fn digest(lang: Locale, to: &str, lines: &[String], open: &str, links: &Links) -> Email {
    let n = lines.len();
    let (heading, sentence, button) = match lang {
        Locale::En => (
            if n == 1 {
                "1 update from Kaeso".to_owned()
            } else {
                format!("{n} updates from Kaeso")
            },
            "Here is what happened since the last summary.",
            "Open Kaeso",
        ),
        Locale::De => (
            if n == 1 {
                "1 Neuigkeit von Kaeso".to_owned()
            } else {
                format!("{n} Neuigkeiten von Kaeso")
            },
            "Das ist seit der letzten Zusammenfassung passiert.",
            "Kaeso öffnen",
        ),
        Locale::ZhCn => (
            zh(&["Kaeso 的 ", &n.to_string(), " 条更新"]),
            "以下是自上次摘要以来发生的事。",
            "打开 Kaeso",
        ),
    };
    Email::new(
        to,
        EmailKind::Digest,
        heading.clone(),
        Content::new(lang, heading, Reason::Notice)
            .sentence(sentence)
            .items(lines.to_vec())
            .action(button, open)
            .footnote(footnote(lang, Because::Daily, links)),
    )
}
