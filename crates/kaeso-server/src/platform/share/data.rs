use std::collections::HashMap;

use kaeso_core::rights::{OrgRight, OrgRights, ProjectRight, ProjectRights};
use kaeso_store::clickhouse::logs::{self as ch_logs, TimelineGroup};
use kaeso_store::pg::limits as limits_store;
use kaeso_store::pg::shared_links::{self as store, SharedLink};
use kaeso_store::pg::usage::{self as usage_store, GroupBy};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::error::ApiResult;
use crate::platform::usage::{UsageOf, UsageRow, rows};
use crate::services::exchange_rates::Money;
use crate::state::AppState;

pub const KINDS: [&str; 4] = ["analytics", "member", "budget", "project_card"];
pub const METRICS: [&str; 6] = ["requests", "tokens", "cost", "errors", "tools", "cache"];

#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
#[schema(description = "One row of a shared view: a day, or one group of a breakdown.")]
pub struct SharedRow {
    #[doc = "The day (`YYYY-MM-DD`), model (`provider/model`), route or client; `playground`\n\
        for requests from the Playground. `null` for projects, keys and members, which\n\
        go by `name` or `number`."]
    pub key: Option<String>,
    #[doc = "The project's, key's or member's name; `null` when the link hides it or the\n\
        row is a day, model, route or client."]
    pub name: Option<String>,
    #[doc = "The place of a hidden project, key or member, 1 for the one with the most\n\
        requests, shown as *Member 1* and the like; `null` when its name shows."]
    pub number: Option<u32>,
    #[doc = "Requests, refusals included."]
    pub requests: u64,
    #[doc = "Requests answered with a status of 400 or above."]
    pub errors: u64,
    #[doc = "Uncached input tokens."]
    pub input_tokens: u64,
    #[doc = "Output tokens."]
    pub output_tokens: u64,
    #[doc = "Cache reads."]
    pub cache_read_tokens: u64,
    #[doc = "Cache writes."]
    pub cache_write_tokens: u64,
    #[doc = "Reasoning tokens."]
    pub reasoning_tokens: u64,
    #[doc = "Cost in USD; `null` when the link hides costs."]
    pub cost: Option<String>,
    #[doc = "`cost` in the view's `currency`, each day at that day's ECB rate; `null`\n\
        when the link hides costs."]
    #[serde(default)]
    pub converted_cost: Option<String>,
    #[doc = "MCP tool calls."]
    pub tool_calls: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Usage split one way.")]
pub struct SharedBreakdown {
    #[doc = "`model`, `key`, `member`, `route`, `project` or `client`."]
    pub by: String,
    #[doc = "Its groups, the most requests first."]
    pub rows: Vec<SharedRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "A limit or budget of a shared project, with what it used.")]
pub struct SharedLimit {
    #[doc = "`project`, `member` or `key`."]
    pub level: String,
    #[doc = "The member's or key's name; `null` for the project or when the link hides it."]
    pub name: Option<String>,
    #[doc = "The place of a hidden member or key, as in a breakdown."]
    pub number: Option<u32>,
    #[doc = "`request_rate`, `token_rate`, `tool_call_rate`, `token_budget` or\n\
        `spend_budget`."]
    pub kind: String,
    #[doc = "The one model it counts, as `provider/model`; `null` for every model."]
    pub model: Option<String>,
    #[doc = "`minute`, `day`, `month` or `total`."]
    pub period: String,
    #[doc = "Requests, tool calls, tokens, or for a spend budget an amount in `currency`."]
    pub amount: String,
    #[doc = "The currency of a spend budget's `amount` and `used`; `null` for counts."]
    #[serde(default)]
    pub currency: Option<String>,
    #[doc = "Used in the current period, in the unit of `amount`, when known."]
    pub used: Option<String>,
    #[doc = "Whether it refuses requests once used up."]
    pub hard: bool,
    #[doc = "When the current period ends; `null` for `total` and rates."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub resets_at: Option<time::OffsetDateTime>,
}

/// What a link shows: kept as it was made for a snapshot, worked out anew for a live link.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SharedData {
    pub days: Vec<SharedRow>,
    pub previous_days: Vec<SharedRow>,
    pub breakdowns: Vec<SharedBreakdown>,
    pub limits: Vec<SharedLimit>,
    pub costs: bool,
    // Kept with a snapshot, so it reads as it was made even after the org changes currency.
    #[serde(default = "usd")]
    pub currency: String,
}

fn usd() -> String {
    kaeso_core::currency::USD.to_owned()
}

impl SharedData {
    // A snapshot kept before organizations had a currency holds dollars only.
    pub fn kept(mut self) -> SharedData {
        if self.currency == kaeso_core::currency::USD {
            let fill = |rows: &mut Vec<SharedRow>| {
                for r in rows.iter_mut().filter(|r| r.converted_cost.is_none()) {
                    r.converted_cost.clone_from(&r.cost);
                }
            };
            fill(&mut self.days);
            fill(&mut self.previous_days);
            for b in &mut self.breakdowns {
                fill(&mut b.rows);
            }
        }
        self
    }

    pub fn without_costs(mut self) -> SharedData {
        let strip = |rows: &mut Vec<SharedRow>| {
            rows.iter_mut().for_each(|r| {
                r.cost = None;
                r.converted_cost = None;
            });
        };
        strip(&mut self.days);
        strip(&mut self.previous_days);
        for b in &mut self.breakdowns {
            strip(&mut b.rows);
        }
        self.limits.retain(|l| l.kind != "spend_budget");
        self.costs = false;
        self
    }
}

/// What a person may share of an org, or of one of its projects.
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub org: OrgRights,
    pub project: Option<ProjectRights>,
}

impl Seen {
    fn project(&self, right: ProjectRight) -> bool {
        self.project.is_some_and(|p| p.contains(right))
    }

    /// Whether the link's numbers are within what the person sees, and whether its costs are.
    pub fn allows(&self, link: &SharedLink) -> Option<bool> {
        if !self.org.contains(OrgRight::UsageShare) {
            return None;
        }
        match link.kind.as_str() {
            "analytics" if link.project_id.is_none() => self
                .org
                .contains(OrgRight::UsageOrgView)
                .then(|| self.org.contains(OrgRight::CostsOrgView)),
            "analytics" | "project_card" => self
                .project(ProjectRight::UsageView)
                .then(|| self.project(ProjectRight::CostsView)),
            "budget" => self
                .project(ProjectRight::LimitsView)
                .then(|| self.project(ProjectRight::CostsView)),
            "member" => {
                let own = link.member_id == Some(link.created_by);
                (own || self.org.contains(OrgRight::UsageOrgView))
                    .then(|| self.org.contains(OrgRight::CostsOrgView))
            }
            _ => None,
        }
    }

    /// Which names the person sees, so a link shows no name its maker couldn't.
    pub fn names(&self, link: &SharedLink) -> Names {
        let in_project = link.project_id.is_some();
        Names {
            members: !link.hide_members
                && if in_project {
                    self.project(ProjectRight::ProjectMembersView)
                } else {
                    self.org.contains(OrgRight::MembersView)
                },
            keys: !link.hide_keys && self.project(ProjectRight::KeysAllView),
            projects: !link.hide_projects,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Names {
    pub members: bool,
    pub keys: bool,
    pub projects: bool,
}

pub(crate) fn period(link: &SharedLink, today: Date) -> (Date, Date) {
    match (link.live, link.period_days, link.from_day, link.to_day) {
        (true, Some(days), _, _) => (today - time::Duration::days(i64::from(days - 1)), today),
        (_, _, Some(from), Some(to)) => (from, to),
        _ => (today, today),
    }
}

fn shared(r: &UsageRow, costs: bool) -> SharedRow {
    SharedRow {
        key: Some(r.key.clone()),
        name: None,
        number: None,
        requests: r.requests,
        errors: r.errors,
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        cache_read_tokens: r.cache_read_tokens,
        cache_write_tokens: r.cache_write_tokens,
        reasoning_tokens: r.reasoning_tokens,
        cost: if costs { r.cost.clone() } else { None },
        converted_cost: if costs {
            r.converted_cost.clone()
        } else {
            None
        },
        tool_calls: r.tool_calls,
    }
}

fn by_requests(rows: &mut [SharedRow]) {
    rows.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.key.cmp(&b.key)));
}

// Projects, keys and members go by their name, or by their place when the link hides names.
fn name_rows(rows: &mut [SharedRow], names: &HashMap<String, String>, show: bool) {
    by_requests(rows);
    for (i, row) in rows.iter_mut().enumerate() {
        let key = row.key.take().unwrap_or_default();
        if key == "playground" {
            row.key = Some(key);
            continue;
        }
        if show && let Some(name) = names.get(&key) {
            row.name = Some(name.clone());
        } else {
            row.number = u32::try_from(i + 1).ok();
        }
    }
}

struct Book {
    projects: HashMap<String, String>,
    keys: HashMap<String, String>,
    members: HashMap<String, String>,
}

async fn book(state: &AppState, org: Uuid) -> ApiResult<Book> {
    let projects = store::project_names(&state.pg, org).await?;
    Ok(Book {
        projects: projects
            .into_iter()
            .map(|(id, n)| (kaeso_core::id::ProjectId::from_uuid(id).to_string(), n))
            .collect(),
        keys: HashMap::new(),
        members: HashMap::new(),
    })
}

async fn fill_names(
    state: &AppState,
    book: &mut Book,
    rows: &[UsageRow],
    by: &str,
) -> ApiResult<()> {
    use kaeso_core::id::{ApiKeyId, ServiceAccountId, UserId};
    match by {
        "key" => {
            let ids: Vec<Uuid> = rows
                .iter()
                .filter_map(|r| r.key.parse::<ApiKeyId>().ok().map(|k| k.uuid()))
                .collect();
            for (id, name) in store::key_names(&state.pg, &ids).await? {
                book.keys.insert(ApiKeyId::from_uuid(id).to_string(), name);
            }
        }
        "member" => {
            let mut users = Vec::new();
            let mut accounts = Vec::new();
            for r in rows {
                match r.key.split_once(':') {
                    Some(("user", id)) => users.extend(id.parse::<UserId>().ok().map(|u| u.uuid())),
                    Some(("service_account", id)) => {
                        accounts.extend(id.parse::<ServiceAccountId>().ok().map(|s| s.uuid()));
                    }
                    _ => {}
                }
            }
            for (id, name) in store::user_names(&state.pg, &users).await? {
                book.members
                    .insert(format!("user:{}", UserId::from_uuid(id)), name);
            }
            for (id, name) in store::service_account_names(&state.pg, &accounts).await? {
                book.members.insert(
                    format!("service_account:{}", ServiceAccountId::from_uuid(id)),
                    name,
                );
            }
        }
        _ => {}
    }
    Ok(())
}

const LONGEST_CLIENT_SPAN: i64 = 62;

// Clients come from the request log, which keeps up to its retention; the breakdown is left out
// when the log can't be read or the period is longer than Analytics reads it for.
async fn clients(
    state: &AppState,
    org: Uuid,
    project: Option<Uuid>,
    filter: &usage_store::Filter,
    (from, to): (Date, Date),
    costs: bool,
    money: &Money,
) -> Option<Vec<SharedRow>> {
    if (to - from).whole_days() >= LONGEST_CLIENT_SPAN {
        return None;
    }
    let ch = crate::platform::logs::log_store(state).ok()?;
    let start = from.midnight().assume_utc();
    let end = to.next_day()?.midnight().assume_utc();
    // A bucket per day, so each day's cost is converted at its own rate.
    let step = 86_400;
    let query = ch_logs::Timeline {
        project,
        filter: filter.clone(),
        failed: None,
        group: Some(TimelineGroup::Client),
        top: Some(20),
    };
    let found = ch_logs::timeline(ch, org, start, end, step, &query)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "a shared link's clients couldn't be read"))
        .ok()?;
    let mut by: HashMap<String, SharedRow> = HashMap::new();
    for r in found {
        let row = by.entry(r.key.clone()).or_insert_with(|| SharedRow {
            key: Some(r.key.clone()),
            ..SharedRow::default()
        });
        row.requests += r.requests;
        row.errors += r.errors;
        row.input_tokens += r.input_tokens;
        row.output_tokens += r.output_tokens;
        row.cache_read_tokens += r.cache_read_tokens;
        row.cache_write_tokens += r.cache_write_tokens;
        row.reasoning_tokens += r.reasoning_tokens;
        if costs {
            let add = |sum: &Option<String>, more: rust_decimal::Decimal| {
                sum.as_deref()
                    .and_then(|c| c.parse::<rust_decimal::Decimal>().ok())
                    .unwrap_or_default()
                    + more
            };
            let cost = kaeso_core::cost::cost_from_raw(r.cost);
            let day = time::OffsetDateTime::from_unix_timestamp(r.start_unix)
                .map_or(from, |at| at.date());
            let converted = money.on(cost, day).unwrap_or_default();
            row.cost = Some(add(&row.cost, cost).normalize().to_string());
            row.converted_cost = Some(money.text(add(&row.converted_cost, converted)));
        }
    }
    let mut rows: Vec<SharedRow> = by.into_values().collect();
    by_requests(&mut rows);
    Some(rows)
}

/// The numbers of a link for its period, with costs only when `costs`.
pub(crate) async fn compute(
    state: &AppState,
    link: &SharedLink,
    names: Names,
    costs: bool,
    (from, to): (Date, Date),
) -> ApiResult<SharedData> {
    let org = link.org_id;
    let project = link.project_id;
    let filter = usage_store::Filter {
        member: link.member_id.map(usage_store::Member::User),
        ..usage_store::Filter::default()
    };
    let money = Money::of_org(state, org).await?;
    let mut data = SharedData {
        costs,
        currency: money.code.clone(),
        ..SharedData::default()
    };
    if link.kind == "budget" {
        data.limits = limits(state, link, names).await?;
        return Ok(data);
    }
    let days = (to - from).whole_days() + 1;
    let before = (
        from - time::Duration::days(days),
        from - time::Duration::days(1),
    );
    let of = || UsageOf {
        org,
        project,
        costs,
    };
    let day_rows = |window| rows(state, of(), window, GroupBy::Day, &filter, &money);
    data.days = day_rows((from, to))
        .await?
        .iter()
        .map(|r| shared(r, costs))
        .collect();
    data.previous_days = day_rows(before)
        .await?
        .iter()
        .map(|r| shared(r, costs))
        .collect();
    let dims: &[(&str, GroupBy)] = match (link.kind.as_str(), project) {
        ("project_card", _) => &[("model", GroupBy::Model)],
        ("member", _) => &[("model", GroupBy::Model), ("project", GroupBy::Project)],
        ("analytics", None) => &[
            ("project", GroupBy::Project),
            ("model", GroupBy::Model),
            ("member", GroupBy::Member),
            ("route", GroupBy::Route),
        ],
        _ => &[
            ("model", GroupBy::Model),
            ("key", GroupBy::Key),
            ("member", GroupBy::Member),
            ("route", GroupBy::Route),
        ],
    };
    let mut book = book(state, org).await?;
    for (by, group) in dims {
        let found = rows(state, of(), (from, to), *group, &filter, &money).await?;
        fill_names(state, &mut book, &found, by).await?;
        let mut out: Vec<SharedRow> = found.iter().map(|r| shared(r, costs)).collect();
        match *by {
            "project" => name_rows(&mut out, &book.projects, names.projects),
            "key" => name_rows(&mut out, &book.keys, names.keys),
            "member" => name_rows(&mut out, &book.members, names.members),
            _ => by_requests(&mut out),
        }
        data.breakdowns.push(SharedBreakdown {
            by: (*by).to_owned(),
            rows: out,
        });
    }
    if link.kind != "project_card"
        && let Some(rows) = clients(state, org, project, &filter, (from, to), costs, &money).await
    {
        data.breakdowns.push(SharedBreakdown {
            by: "client".into(),
            rows,
        });
    }
    Ok(data)
}

async fn limits(state: &AppState, link: &SharedLink, names: Names) -> ApiResult<Vec<SharedLimit>> {
    use kaeso_core::limits::Subject;
    let Some(project) = link.project_id else {
        return Ok(Vec::new());
    };
    let now = state.clock.now();
    let rows = limits_store::list(&state.pg, project).await?;
    let mut users = Vec::new();
    let mut accounts = Vec::new();
    let mut keys = Vec::new();
    let mut models = Vec::new();
    let mut found = Vec::new();
    for row in &rows {
        let view = crate::platform::projects::limits::view(state, project, row, now).await?;
        let l = row.limit()?;
        match l.subject {
            Subject::User(u) => users.push(u),
            Subject::ServiceAccount(s) => accounts.push(s),
            Subject::Key(k) => keys.push(k),
            Subject::Project(_) => {}
        }
        models.extend(l.model);
        found.push((l, view));
    }
    let mut member_names: HashMap<Uuid, String> = store::user_names(&state.pg, &users)
        .await?
        .into_iter()
        .collect();
    member_names.extend(store::service_account_names(&state.pg, &accounts).await?);
    let key_names: HashMap<Uuid, String> = store::key_names(&state.pg, &keys)
        .await?
        .into_iter()
        .collect();
    let model_names: HashMap<Uuid, String> = store::model_names(&state.pg, &models)
        .await?
        .into_iter()
        .collect();
    let mut places: HashMap<(&str, Uuid), u32> = HashMap::new();
    let mut out = Vec::new();
    for (l, view) in found {
        let (name, number) = match l.subject {
            Subject::Project(_) => (None, None),
            Subject::User(id) | Subject::ServiceAccount(id) => {
                let next =
                    u32::try_from(places.iter().filter(|((k, _), _)| *k == "member").count() + 1)
                        .unwrap_or(0);
                let place = *places.entry(("member", id)).or_insert(next);
                if names.members {
                    (member_names.get(&id).cloned(), None)
                } else {
                    (None, Some(place))
                }
            }
            Subject::Key(id) => {
                let next =
                    u32::try_from(places.iter().filter(|((k, _), _)| *k == "key").count() + 1)
                        .unwrap_or(0);
                let place = *places.entry(("key", id)).or_insert(next);
                if names.keys {
                    (key_names.get(&id).cloned(), None)
                } else {
                    (None, Some(place))
                }
            }
        };
        out.push(SharedLimit {
            level: view.level.to_owned(),
            name,
            number,
            kind: view.kind.to_owned(),
            model: l.model.and_then(|m| model_names.get(&m).cloned()),
            period: view.period.to_owned(),
            amount: view.amount,
            currency: view.currency,
            used: view.used,
            hard: view.hard,
            resets_at: view.resets_at,
        });
    }
    Ok(out)
}
