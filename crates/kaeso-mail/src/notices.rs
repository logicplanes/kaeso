use time::{Date, PrimitiveDateTime};

use crate::layout::{
    Content, Cover, Detail, Footnote, Group, Label, Link, List, Reason, Row, Stat, Table, never,
};
use crate::locale::{Locale, zh};
use crate::templates::{Actor, Budget, BudgetScope, Email, EmailKind, budget_details};

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
pub struct Answer<'a> {
    pub status: u16,
    pub message: &'a str,
    pub billing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKey<'a> {
    Refused {
        answer: Answer<'a>,
        first: PrimitiveDateTime,
        last: PrimitiveDateTime,
    },
    SignedOut {
        at: PrimitiveDateTime,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credential<'a> {
    pub organization: &'a str,
    pub provider: &'a str,
    pub name: &'a str,
    pub last4: Option<&'a str>,
    pub account: Option<&'a str>,
    pub default: bool,
    pub projects: &'a [String],
    pub last_used: Option<PrimitiveDateTime>,
    pub zone: &'a str,
}

impl Credential<'_> {
    fn is_account(&self) -> bool {
        self.account.is_some()
    }

    fn shown(&self, lang: Locale) -> String {
        let (open, close) = match lang {
            Locale::En | Locale::De => (" (", ")"),
            Locale::ZhCn => ("（", "）"),
        };
        match (self.account, self.last4) {
            (Some(account), _) => format!("{}{open}{account}{close}", self.name),
            (None, Some(last4)) => format!("{}{open}…{last4}{close}", self.name),
            (None, None) => self.name.to_owned(),
        }
    }

    fn used_by(&self, lang: Locale) -> String {
        let list = self.projects.join(match lang {
            Locale::En | Locale::De => ", ",
            Locale::ZhCn => "、",
        });
        match (lang, self.default, self.projects.is_empty()) {
            (Locale::En, true, true) => "Every project without a key of its own".to_owned(),
            (Locale::En, true, false) => {
                format!("{list}, and every project without a key of its own")
            }
            (Locale::En, false, true) => "No project".to_owned(),
            (Locale::De, true, true) => "Jedes Projekt ohne eigenen Schlüssel".to_owned(),
            (Locale::De, true, false) => {
                format!("{list} und jedes Projekt ohne eigenen Schlüssel")
            }
            (Locale::De, false, true) => "Kein Projekt".to_owned(),
            (Locale::ZhCn, true, true) => "所有没有自己密钥的项目".to_owned(),
            (Locale::ZhCn, true, false) => zh(&[&list, "，以及所有没有自己密钥的项目"]),
            (Locale::ZhCn, false, true) => "没有项目".to_owned(),
            (_, false, false) => list,
        }
    }

    fn details(&self, lang: Locale, change: &ProviderKey<'_>) -> Vec<Detail> {
        let which = if self.is_account() {
            Label::Account
        } else {
            Label::Key
        };
        let mut out = vec![
            Label::Organization.of(lang, self.organization),
            Label::Provider.of(lang, self.provider),
            which.of(lang, self.shown(lang)),
            Label::UsedBy.of(lang, self.used_by(lang)),
        ];
        match change {
            ProviderKey::Refused {
                answer,
                first,
                last,
            } => {
                let said = match lang {
                    Locale::En | Locale::De => {
                        format!("HTTP {}: {}", answer.status, answer.message)
                    }
                    Locale::ZhCn => format!("HTTP {}：{}", answer.status, answer.message),
                };
                out.push(Label::Answer.of(lang, said));
                out.push(Label::FirstRefused.of(lang, lang.moment(*first, self.zone)));
                out.push(Label::LastRefused.of(lang, lang.moment(*last, self.zone)));
            }
            ProviderKey::SignedOut { at } => {
                out.push(Label::SignedOut.of(lang, lang.moment(*at, self.zone)));
            }
        }
        out.push(
            Label::LastUsed.of(
                lang,
                self.last_used
                    .map_or_else(|| never(lang).to_owned(), |at| lang.moment(at, self.zone)),
            ),
        );
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Price {
    Input,
    Output,
    CacheRead,
    CacheWrite,
    CacheWriteHour,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceChange {
    pub price: Price,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl PriceChange {
    fn detail(&self, lang: Locale) -> Detail {
        let label = match self.price {
            Price::Input => Label::InputPrice,
            Price::Output => Label::OutputPrice,
            Price::CacheRead => Label::CacheReadPrice,
            Price::CacheWrite => Label::CacheWritePrice,
            Price::CacheWriteHour => Label::CacheWriteHourPrice,
        };
        let n = |v: &Option<String>| v.as_deref().map_or("—".to_owned(), |v| lang.number(v));
        let (from, to) = (n(&self.from), n(&self.to));
        let value = match lang {
            Locale::En => format!("{from} → {to} USD per million tokens"),
            Locale::De => format!("{from} → {to} USD pro Million Token"),
            Locale::ZhCn => format!("每百万 Token {from} → {to} USD"),
        };
        label.of(lang, value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelChange {
    Delisted,
    Price(Vec<PriceChange>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install<'a> {
    Window {
        at: PrimitiveDateTime,
        zone: &'a str,
    },
    FromConsole,
    ByHand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update<'a> {
    Available {
        running: &'a str,
        install: Install<'a>,
    },
    Finished,
    RolledBack,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notes<'a> {
    pub markdown: &'a str,
    pub url: &'a str,
}

const HIGHLIGHTS: usize = 4;
const HIGHLIGHT_CHARS: usize = 160;

pub fn highlights(markdown: &str) -> (Vec<String>, usize) {
    let all: Vec<String> = markdown
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(plain)
        .filter(|line| !line.is_empty())
        .collect();
    let more = all.len().saturating_sub(HIGHLIGHTS);
    (
        all.into_iter().take(HIGHLIGHTS).map(shorten).collect(),
        more,
    )
}

fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line.trim();
    if let Some(after) = rest.strip_prefix("**").and_then(|r| r.split_once(":**")) {
        rest = after.1.trim_start();
    }
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let tail = &rest[open + 1..];
        match tail
            .split_once("](")
            .and_then(|(text, after)| after.split_once(')').map(|(_, after)| (text, after)))
        {
            Some((text, after)) => {
                if !text.starts_with('#') {
                    out.push_str(text);
                }
                rest = after;
            }
            None => {
                out.push('[');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    let out = out.replace(['`', '*'], "").replace("()", "");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn shorten(line: String) -> String {
    if line.chars().count() <= HIGHLIGHT_CHARS {
        return end_with_stop(line);
    }
    let head: String = line.chars().take(HIGHLIGHT_CHARS).collect();
    if let Some(at) = head.find("; ").or_else(|| head.find(". ")) {
        return end_with_stop(head[..at].to_owned());
    }
    let cut = head.rfind(' ').map_or(head.as_str(), |at| &head[..at]);
    format!("{}…", cut.trim_end_matches([',', ';', ':']))
}

fn end_with_stop(mut line: String) -> String {
    if !line.ends_with(['.', '!', '?', '…']) {
        line.push('.');
    }
    line
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
    pub organization: String,
    pub name: String,
    pub requests: Option<String>,
    pub cost: String,
}

impl ProjectUse {
    fn line(&self, lang: Locale) -> String {
        let cost = lang.usd(&self.cost);
        match (lang, self.requests.as_deref().map(|r| lang.number(r))) {
            (Locale::En, Some(r)) => format!("{}: {r} requests, {cost}", self.name),
            (Locale::De, Some(r)) => format!("{}: {r} Anfragen, {cost}", self.name),
            (Locale::ZhCn, Some(r)) => zh(&[&self.name, "：", &r, " 次请求，", &cost]),
            (Locale::En | Locale::De, None) => format!("{}: {cost}", self.name),
            (Locale::ZhCn, None) => zh(&[&self.name, "：", &cost]),
        }
    }

    fn row(&self, lang: Locale) -> Row {
        let mut cells = vec![self.name.clone()];
        cells.extend(self.requests.as_deref().map(|r| lang.number(r)));
        cells.push(lang.usd(&self.cost));
        Row {
            cells,
            text: self.line(lang),
            heading: false,
        }
    }
}

fn organizations(projects: &[ProjectUse]) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for p in projects {
        if !out.contains(&p.organization.as_str()) {
            out.push(&p.organization);
        }
    }
    out
}

fn only_organization(projects: &[ProjectUse]) -> Option<&str> {
    match organizations(projects).as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

fn projects_table(lang: Locale, projects: &[ProjectUse], requests: bool) -> Option<Table> {
    if projects.is_empty() {
        return None;
    }
    let (project, count, cost) = match lang {
        Locale::En => ("Project", "Requests", "Cost"),
        Locale::De => ("Projekt", "Anfragen", "Kosten"),
        Locale::ZhCn => ("项目", "请求", "成本"),
    };
    let mut head = vec![project];
    if requests {
        head.push(count);
    }
    head.push(cost);
    let orgs = organizations(projects);
    let rows = if orgs.len() > 1 {
        orgs.iter()
            .flat_map(|org| {
                std::iter::once(Row {
                    cells: vec![(*org).to_owned()],
                    text: (*org).to_owned(),
                    heading: true,
                })
                .chain(
                    projects
                        .iter()
                        .filter(|p| p.organization == *org)
                        .map(|p| p.row(lang)),
                )
            })
            .collect()
    } else {
        projects.iter().map(|p| p.row(lang)).collect()
    };
    Some(Table { head, rows })
}

fn usage_stats(lang: Locale, requests: &str, tool_calls: &str, cost: &str) -> Vec<Stat> {
    let (r, t, c) = match lang {
        Locale::En => ("Requests", "Tool calls", "Cost"),
        Locale::De => ("Anfragen", "Tool-Aufrufe", "Kosten"),
        Locale::ZhCn => ("请求", "工具调用", "成本"),
    };
    vec![
        Stat {
            label: r,
            value: lang.number(requests),
        },
        Stat {
            label: t,
            value: lang.number(tool_calls),
        },
        Stat {
            label: c,
            value: lang.usd(cost),
        },
    ]
}

fn installs(lang: Locale, install: Install<'_>) -> String {
    match (lang, install) {
        (Locale::En, Install::Window { at, zone }) => {
            format!("By itself, {}", lang.moment(at, zone))
        }
        (Locale::De, Install::Window { at, zone }) => {
            format!("Automatisch, {}", lang.moment(at, zone))
        }
        (Locale::ZhCn, Install::Window { at, zone }) => zh(&["自动安装，", &lang.moment(at, zone)]),
        (Locale::En, Install::FromConsole) => "From the updates page".to_owned(),
        (Locale::De, Install::FromConsole) => "Über die Update-Seite".to_owned(),
        (Locale::ZhCn, Install::FromConsole) => "在更新页面安装".to_owned(),
        (Locale::En, Install::ByHand) => "By hand, with the upgrade guide".to_owned(),
        (Locale::De, Install::ByHand) => "Von Hand, mit der Upgrade-Anleitung".to_owned(),
        (Locale::ZhCn, Install::ByHand) => "按照升级指南手动更新".to_owned(),
    }
}

fn release_list(lang: Locale, notes: &Notes<'_>) -> List {
    let (items, more) = highlights(notes.markdown);
    let label = match lang {
        Locale::En => "What changed",
        Locale::De => "Was sich ändert",
        Locale::ZhCn => "更新内容",
    };
    let more = (more > 0).then(|| match (lang, more) {
        (Locale::En, 1) => "And 1 more change in the release notes.".to_owned(),
        (Locale::En, n) => format!("And {n} more changes in the release notes."),
        (Locale::De, 1) => "Und 1 weitere Änderung in den Release-Notizen.".to_owned(),
        (Locale::De, n) => format!("Und {n} weitere Änderungen in den Release-Notizen."),
        (Locale::ZhCn, n) => zh(&["发布说明中还有 ", &n.to_string(), " 项更改。"]),
    });
    List {
        label: (!items.is_empty()).then_some(label),
        items,
        groups: Vec::new(),
        more,
        lang: (lang != Locale::En).then_some(Locale::En),
    }
}

fn notes_link(lang: Locale, notes: &Notes<'_>) -> Option<Link> {
    let label = match lang {
        Locale::En => "Read the release notes",
        Locale::De => "Release-Notizen lesen",
        Locale::ZhCn => "阅读发布说明",
    };
    (!notes.url.is_empty()).then(|| Link {
        label,
        url: notes.url.to_owned(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fact<'a> {
    NewApp {
        app: &'a str,
    },
    ExportReady(Export),
    WeeklySummary {
        from: Date,
        to: Date,
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
        organization: &'a str,
        project: &'a str,
        budget: &'a Budget,
        scope: BudgetScope<'a>,
        resets: Option<PrimitiveDateTime>,
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
        key: &'a Credential<'a>,
        change: ProviderKey<'a>,
    },
    Model {
        provider: &'a str,
        project: &'a str,
        model: &'a str,
        change: ModelChange,
    },
    CostReport {
        month: Date,
        total: &'a str,
        projects: Vec<ProjectUse>,
    },
    InstanceUpdate {
        version: &'a str,
        change: Update<'a>,
        notes: Option<Notes<'a>>,
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

    fn table(&self, lang: Locale) -> Option<Table> {
        match self {
            Fact::WeeklySummary { projects, .. } => projects_table(lang, projects, true),
            Fact::CostReport { projects, .. } => projects_table(lang, projects, false),
            _ => None,
        }
    }

    fn stats(&self, lang: Locale) -> Vec<Stat> {
        match self {
            Fact::WeeklySummary {
                requests,
                tool_calls,
                cost,
                ..
            } => usage_stats(lang, requests, tool_calls, cost),
            _ => Vec::new(),
        }
    }

    fn cover(&self) -> Option<Cover> {
        match self {
            Fact::NewApp { .. } => Some(Cover::Apps),
            Fact::WeeklySummary { .. } => Some(Cover::Summary),
            Fact::CostReport { .. } => Some(Cover::Costs),
            Fact::InviteAccepted { .. } => Some(Cover::Welcome),
            Fact::InstanceUpdate {
                change: Update::Available { .. } | Update::Finished,
                ..
            } => Some(Cover::Release),
            _ => None,
        }
    }

    fn list(&self, lang: Locale) -> List {
        match self {
            Fact::InstanceUpdate {
                change: Update::Available { .. } | Update::Finished,
                notes: Some(notes),
                ..
            } => release_list(lang, notes),
            _ => List::default(),
        }
    }

    fn link(&self, lang: Locale) -> Option<Link> {
        match self {
            Fact::InstanceUpdate {
                change: Update::Available { .. } | Update::Finished,
                notes: Some(notes),
                ..
            } => notes_link(lang, notes),
            _ => None,
        }
    }

    fn preheader(&self, lang: Locale) -> Option<String> {
        let Fact::WeeklySummary {
            requests,
            tool_calls,
            cost,
            ..
        } = self
        else {
            return None;
        };
        let (r, t, c) = (
            lang.number(requests),
            lang.number(tool_calls),
            lang.usd(cost),
        );
        Some(match lang {
            Locale::En => {
                format!("Last week your projects sent {r} requests and {t} tool calls for {c}.")
            }
            Locale::De => format!(
                "Ihre Projekte sendeten letzte Woche {r} Anfragen und {t} Tool-Aufrufe für {c}."
            ),
            Locale::ZhCn => zh(&[
                "上周您的项目发送了 ",
                &r,
                " 次请求和 ",
                &t,
                " 次工具调用，成本为 ",
                &c,
                "。",
            ]),
        })
    }

    fn details(&self, lang: Locale) -> Vec<Detail> {
        match self {
            Fact::LimitReached {
                organization,
                project,
                budget,
                scope,
                resets,
            } => budget_details(lang, organization, project, *scope, budget, None, *resets),
            Fact::ProviderKey { key, change } => key.details(lang, change),
            Fact::WeeklySummary { top_models, .. } if !top_models.is_empty() => {
                let list = top_models.join(match lang {
                    Locale::En | Locale::De => ", ",
                    Locale::ZhCn => "、",
                });
                vec![Label::TopModels.of(lang, list)]
            }
            Fact::InstanceUpdate {
                version,
                change: Update::Available { running, install },
                ..
            } => vec![
                Label::Version.of(lang, *version),
                Label::Running.of(lang, *running),
                Label::Installs.of(lang, installs(lang, *install)),
            ],
            Fact::Model {
                provider,
                project,
                model,
                change,
            } => {
                let mut out = vec![
                    Label::Project.of(lang, *project),
                    Label::Provider.of(lang, *provider),
                    Label::Model.of(lang, *model),
                ];
                if let ModelChange::Price(changes) = change {
                    out.extend(changes.iter().map(|c| c.detail(lang)));
                }
                out
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
                from, to, projects, ..
            } => {
                let range = Locale::En.range(*from, *to);
                w(
                    "Your week in Kaeso".to_owned(),
                    match only_organization(projects) {
                        Some(org) => {
                            format!("Here is how your projects in {org} used Kaeso, {range}.")
                        }
                        None => format!("Here is how your projects used Kaeso, {range}."),
                    },
                    "Open usage",
                )
            }
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
            Fact::LimitReached {
                project,
                budget,
                scope,
                ..
            } => {
                let who = scope.name(Locale::En, project);
                let its = match scope {
                    BudgetScope::Project | BudgetScope::Key(_) => "its",
                    BudgetScope::Member(_) => "the",
                };
                w(
                    format!(
                        "{} reached {its} {}",
                        scope.short(Locale::En, project),
                        budget.label_en()
                    ),
                    format!(
                        "{who} used all of {its} {}; requests over it are refused.",
                        budget.english_grouped()
                    ),
                    "Open limits",
                )
            }
            Fact::KeyChanged { key, change, by } => {
                let verb = match change {
                    KeyChange::Disabled => "disabled",
                    KeyChange::Rotated => "rotated",
                    KeyChange::Archived => "archived",
                };
                w(
                    format!("Your key {key} was {verb}"),
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
            Fact::ProviderKey { key, change } => {
                let (provider, name, shown) = (key.provider, key.name, key.shown(Locale::En));
                match change {
                    ProviderKey::Refused { answer, .. } => {
                        let (heading, sentence) = match (key.is_account(), answer.billing) {
                            (false, false) => (
                                format!("{provider} key {name} keeps being refused"),
                                format!(
                                    "{provider} refused the key {shown} three times in a row; replace it."
                                ),
                            ),
                            (false, true) => (
                                format!("{provider} key {name} keeps being refused"),
                                format!(
                                    "{provider} refused {shown} three times over billing; add credit or replace the key."
                                ),
                            ),
                            (true, false) => (
                                format!("{provider} account {name} keeps being refused"),
                                format!(
                                    "{provider} refused the account {shown} three times in a row; sign in to it again."
                                ),
                            ),
                            (true, true) => (
                                format!("{provider} account {name} keeps being refused"),
                                format!(
                                    "{provider} refused the account {shown} three times over billing; check its plan."
                                ),
                            ),
                        };
                        w(heading, sentence, "Open provider keys")
                    }
                    ProviderKey::SignedOut { .. } => w(
                        format!("{provider} account {name} signed out"),
                        format!(
                            "{provider} ended the session of the account {shown}; sign in to it again."
                        ),
                        "Open provider keys",
                    ),
                }
            }
            Fact::Model {
                provider,
                project,
                model,
                change,
            } => match change {
                ModelChange::Delisted => w(
                    format!("{model} is delisted"),
                    format!("{provider} delisted {model}; requests to it from {project} may fail."),
                    "Open models",
                ),
                ModelChange::Price(_) => w(
                    format!("{model} changed its price"),
                    format!("{provider} changed the price of {model}; costs from now on use it."),
                    "Open models",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                format!(
                    "{} cost {}",
                    Locale::En.month(*month),
                    Locale::En.usd(total)
                ),
                "These projects cost the most last month.".to_owned(),
                "Open usage",
            ),
            Fact::InstanceUpdate {
                version, change, ..
            } => match change {
                Update::Available { install, .. } => w(
                    format!("Kaeso {version} is out"),
                    match install {
                        Install::Window { .. } => {
                            "It installs by itself in the next update window."
                        }
                        Install::FromConsole => {
                            "Install it from the updates page when it suits you."
                        }
                        Install::ByHand => "It needs steps by hand; the upgrade guide lists them.",
                    }
                    .to_owned(),
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
                from, to, projects, ..
            } => {
                let range = Locale::De.range(*from, *to);
                w(
                    "Ihre Woche in Kaeso".to_owned(),
                    match only_organization(projects) {
                        Some(org) => format!(
                            "So haben Ihre Projekte in {org} Kaeso im Zeitraum {range} genutzt."
                        ),
                        None => {
                            format!("So haben Ihre Projekte Kaeso im Zeitraum {range} genutzt.")
                        }
                    },
                    "Nutzung öffnen",
                )
            }
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
            Fact::LimitReached {
                project,
                budget,
                scope,
                ..
            } => w(
                format!(
                    "{} hat das {} erreicht",
                    scope.short(Locale::De, project),
                    budget.label_de()
                ),
                format!(
                    "{} hat das gesamte {} verbraucht; weitere Anfragen werden abgelehnt.",
                    scope.name(Locale::De, project),
                    budget.german_accusative()
                ),
                "Limits öffnen",
            ),
            Fact::KeyChanged { key, change, by } => {
                let verb = match change {
                    KeyChange::Disabled => "deaktiviert",
                    KeyChange::Rotated => "rotiert",
                    KeyChange::Archived => "archiviert",
                };
                w(
                    format!("Ihr Schlüssel {key} wurde {verb}"),
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
            Fact::ProviderKey { key, change } => {
                let (provider, name, shown) = (key.provider, key.name, key.shown(Locale::De));
                match change {
                    ProviderKey::Refused { answer, .. } => {
                        let (heading, sentence) = match (key.is_account(), answer.billing) {
                            (false, false) => (
                                format!("{provider}-Schlüssel {name} wird abgelehnt"),
                                format!(
                                    "{provider} hat den Schlüssel {shown} dreimal nacheinander abgelehnt; ersetzen Sie ihn."
                                ),
                            ),
                            (false, true) => (
                                format!("{provider}-Schlüssel {name} wird abgelehnt"),
                                format!(
                                    "{provider} hat {shown} dreimal wegen Abrechnung abgelehnt; Konto aufladen oder Schlüssel ersetzen."
                                ),
                            ),
                            (true, false) => (
                                format!("{provider}-Konto {name} wird abgelehnt"),
                                format!(
                                    "{provider} hat das Konto {shown} dreimal nacheinander abgelehnt; melden Sie sich erneut an."
                                ),
                            ),
                            (true, true) => (
                                format!("{provider}-Konto {name} wird abgelehnt"),
                                format!(
                                    "{provider} hat das Konto {shown} dreimal wegen Abrechnung abgelehnt; prüfen Sie seinen Tarif."
                                ),
                            ),
                        };
                        w(heading, sentence, "Anbieter-Schlüssel öffnen")
                    }
                    ProviderKey::SignedOut { .. } => w(
                        format!("{provider}-Konto {name} ist abgemeldet"),
                        format!(
                            "{provider} hat die Sitzung des Kontos {shown} beendet; melden Sie sich erneut an."
                        ),
                        "Anbieter-Schlüssel öffnen",
                    ),
                }
            }
            Fact::Model {
                provider,
                project,
                model,
                change,
            } => match change {
                ModelChange::Delisted => w(
                    format!("{model} wird eingestellt"),
                    format!(
                        "{provider} hat {model} eingestellt; Anfragen aus {project} daran können fehlschlagen."
                    ),
                    "Modelle öffnen",
                ),
                ModelChange::Price(_) => w(
                    format!("{model} hat einen neuen Preis"),
                    format!(
                        "{provider} hat den Preis von {model} geändert; Kosten ab jetzt nutzen ihn."
                    ),
                    "Modelle öffnen",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                format!(
                    "{} kostete {}",
                    Locale::De.month(*month),
                    Locale::De.usd(total)
                ),
                "Diese Projekte haben im letzten Monat am meisten gekostet.".to_owned(),
                "Nutzung öffnen",
            ),
            Fact::InstanceUpdate {
                version, change, ..
            } => match change {
                Update::Available { install, .. } => w(
                    format!("Kaeso {version} ist erschienen"),
                    match install {
                        Install::Window { .. } => {
                            "Es wird im nächsten Update-Fenster automatisch installiert."
                        }
                        Install::FromConsole => {
                            "Installieren Sie es über die Update-Seite, wann es Ihnen passt."
                        }
                        Install::ByHand => {
                            "Es verlangt Schritte von Hand; die Upgrade-Anleitung nennt sie."
                        }
                    }
                    .to_owned(),
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
                from, to, projects, ..
            } => {
                let range = Locale::ZhCn.range(*from, *to);
                w(
                    "您在 Kaeso 的一周".to_owned(),
                    match only_organization(projects) {
                        Some(org) => zh(&[
                            "以下是您在 ",
                            org,
                            " 中的项目在",
                            &range,
                            "期间使用 Kaeso 的情况。",
                        ]),
                        None => zh(&["以下是您的项目在", &range, "期间使用 Kaeso 的情况。"]),
                    },
                    "打开用量",
                )
            }
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
            Fact::LimitReached {
                project,
                budget,
                scope,
                ..
            } => w(
                zh(&[
                    &scope.short(Locale::ZhCn, project),
                    "已达到",
                    budget.label_zh(),
                ]),
                zh(&[
                    &scope.name(Locale::ZhCn, project),
                    "的",
                    &budget.chinese(),
                    "已全部用完；超出的请求会被拒绝。",
                ]),
                "打开限额",
            ),
            Fact::KeyChanged { key, change, by } => {
                let (done, verb) = match change {
                    KeyChange::Disabled => ("已停用", "停用了"),
                    KeyChange::Rotated => ("已轮换", "轮换了"),
                    KeyChange::Archived => ("已归档", "归档了"),
                };
                w(
                    zh(&["您的密钥", key, done]),
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
            Fact::ProviderKey { key, change } => {
                let (provider, name, shown) = (key.provider, key.name, key.shown(Locale::ZhCn));
                match change {
                    ProviderKey::Refused { answer, .. } => {
                        let (heading, sentence) = match (key.is_account(), answer.billing) {
                            (false, false) => (
                                zh(&[provider, "密钥", name, "一再被拒绝"]),
                                zh(&[provider, "连续三次拒绝了密钥", &shown, "；请替换它。"]),
                            ),
                            (false, true) => (
                                zh(&[provider, "密钥", name, "一再被拒绝"]),
                                zh(&[
                                    provider,
                                    "因账单问题连续三次拒绝了",
                                    &shown,
                                    "；请为账户充值或替换密钥。",
                                ]),
                            ),
                            (true, false) => (
                                zh(&[provider, "账户", name, "一再被拒绝"]),
                                zh(&[provider, "连续三次拒绝了账户", &shown, "；请重新登录。"]),
                            ),
                            (true, true) => (
                                zh(&[provider, "账户", name, "一再被拒绝"]),
                                zh(&[
                                    provider,
                                    "因账单或配额问题连续三次拒绝了账户",
                                    &shown,
                                    "；请检查其套餐。",
                                ]),
                            ),
                        };
                        w(heading, sentence, "打开服务商密钥")
                    }
                    ProviderKey::SignedOut { .. } => w(
                        zh(&[provider, "账户", name, "已退出登录"]),
                        zh(&[provider, "结束了账户", &shown, "的会话；请重新登录。"]),
                        "打开服务商密钥",
                    ),
                }
            }
            Fact::Model {
                provider,
                project,
                model,
                change,
            } => match change {
                ModelChange::Delisted => w(
                    zh(&[model, "已下架"]),
                    zh(&[
                        provider,
                        "已下架",
                        model,
                        "；",
                        project,
                        "对它的请求可能失败。",
                    ]),
                    "打开模型",
                ),
                ModelChange::Price(_) => w(
                    zh(&[model, "的价格已变更"]),
                    zh(&[
                        provider,
                        "更改了",
                        model,
                        "的价格；此后的成本按新价格计算。",
                    ]),
                    "打开模型",
                ),
            },
            Fact::CostReport { month, total, .. } => w(
                zh(&[
                    &Locale::ZhCn.month(*month),
                    "的成本为 ",
                    &Locale::ZhCn.usd(total),
                ]),
                "以下是上个月成本最高的项目。".to_owned(),
                "打开用量",
            ),
            Fact::InstanceUpdate {
                version, change, ..
            } => match change {
                Update::Available { install, .. } => w(
                    zh(&["Kaeso ", version, " 已发布"]),
                    match install {
                        Install::Window { .. } => "它会在下一个更新窗口内自动安装。",
                        Install::FromConsole => "您可以随时在更新页面安装。",
                        Install::ByHand => "此版本需要手动操作；升级指南列出了具体步骤。",
                    }
                    .to_owned(),
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
    let why = match why {
        Because::Right(id) => {
            Because::Right(crate::right_names::right_name(lang, id).unwrap_or(id))
        }
        other => other,
    };
    match (lang, why) {
        (Locale::En, Because::AboutYou) => {
            "You get this email because it's about your account.".to_owned()
        }
        (Locale::En, Because::Right(r)) => {
            format!("You get this email because you hold the right “{r}”.")
        }
        (Locale::En, Because::Daily) => {
            "You get this email because you chose daily emails.".to_owned()
        }
        (Locale::De, Because::AboutYou) => {
            "Sie erhalten diese E-Mail, weil sie Ihr Konto betrifft.".to_owned()
        }
        (Locale::De, Because::Right(r)) => {
            format!("Sie erhalten diese E-Mail, weil Sie das Recht „{r}“ haben.")
        }
        (Locale::De, Because::Daily) => {
            "Sie erhalten diese E-Mail, weil Sie tägliche E-Mails gewählt haben.".to_owned()
        }
        (Locale::ZhCn, Because::AboutYou) => "您收到此邮件，是因为它与您的账户有关。".to_owned(),
        (Locale::ZhCn, Because::Right(r)) => zh(&["您收到此邮件，是因为您拥有“", r, "”权限。"]),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place<'a> {
    pub organization: &'a str,
    pub project: Option<&'a str>,
}

impl<'a> Place<'a> {
    fn name(self) -> &'a str {
        self.project.unwrap_or(self.organization)
    }

    fn scope(self) -> String {
        scope(self.organization, self.project)
    }
}

fn scope(organization: &str, project: Option<&str>) -> String {
    match project {
        Some(p) => format!("{organization} / {p}"),
        None => organization.to_owned(),
    }
}

fn subject(place: Option<Place<'_>>, heading: &str) -> String {
    match place {
        Some(p) => format!("[{}] {heading}", p.name()),
        None => heading.to_owned(),
    }
}

pub fn notice(
    lang: Locale,
    to: &str,
    place: Option<Place<'_>>,
    fact: &Fact<'_>,
    why: Because<'_>,
    open: &str,
    links: &Links,
) -> Email {
    let words = fact.words(lang);
    let mut content = Content::new(lang, words.heading.clone(), Reason::Notice)
        .eyebrow(place.map(Place::scope))
        .sentence(words.sentence)
        .stats(fact.stats(lang))
        .details(fact.details(lang))
        .table(fact.table(lang))
        .list(fact.list(lang))
        .action(words.button, open)
        .link(fact.link(lang))
        .footnote(footnote(lang, why, links));
    if let Some(cover) = fact.cover() {
        content = content.cover(cover);
    }
    if let Some(preheader) = fact.preheader(lang) {
        content = content.preheader(preheader);
    }
    Email::new(to, fact.kind(), subject(place, &words.heading), content)
        .list_unsubscribe(links.unsubscribe.as_ref().map(|u| u.one_click.clone()))
}

pub fn line(lang: Locale, fact: &Fact<'_>) -> String {
    fact.words(lang).heading
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigestItem {
    pub line: String,
    pub organization: Option<String>,
    pub project: Option<String>,
}

fn digest_list(items: &[DigestItem]) -> List {
    let mut list = List::default();
    for item in items {
        let Some(org) = &item.organization else {
            list.items.push(item.line.clone());
            continue;
        };
        let title = scope(org, item.project.as_deref());
        match list.groups.iter_mut().find(|g| g.title == title) {
            Some(group) => group.items.push(item.line.clone()),
            None => list.groups.push(Group {
                title,
                items: vec![item.line.clone()],
            }),
        }
    }
    list
}

pub fn digest(lang: Locale, to: &str, items: &[DigestItem], open: &str, links: &Links) -> Email {
    let n = items.len();
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
            .cover(Cover::Summary)
            .sentence(sentence)
            .list(digest_list(items))
            .action(button, open)
            .footnote(footnote(lang, Because::Daily, links)),
    )
}
