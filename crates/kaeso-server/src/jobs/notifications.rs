use std::collections::BTreeMap;

use kaeso_core::notifications::Kind;
use kaeso_core::rights::{OrgRight, ProjectRight, effective_project_rights};
use kaeso_mail::Locale;
use kaeso_mail::notices::{self, Because, Fact, Links, ProjectUse};
use kaeso_store::pg::{instance, members, notifications, orgs, usage, users};
use rust_decimal::Decimal;
use time::macros::time;
use time::{Date, Duration, OffsetDateTime, Time, Weekday};
use uuid::Uuid;

use crate::jobs::JobError;
use crate::platform::grants::{org_from_store, project_from_store};
use crate::services::notifications::dispatch::{Send, deliver, org_holders, settings_path};
use crate::state::AppState;

// Spec: digests, the monthly cost report and the weekly summary go out at 8:31 in the morning.
const AT: Time = time!(8:31);
const BATCH: i64 = 100;
const TOP: usize = 5;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let now = state.clock.now();
    let zone = crate::time_zone::or_utc(&instance::time_zone(&state.pg).await?);
    let local = crate::time_zone::local(now, &zone);
    let offset = local.assume_utc() - now;
    let today = if local.time() >= AT {
        local.date()
    } else {
        local.date().previous_day().unwrap_or(local.date())
    };
    let cutoff = today.with_time(AT).assume_utc() - offset;
    let mut done = vec![format!("{} digests", digests(&state, cutoff, now).await?)];
    if local.time() >= AT && today.day() == 1 && first(&state, "cost", today).await {
        done.push(format!(
            "{} cost reports",
            cost_reports(&state, today).await?
        ));
    }
    if local.time() >= AT
        && today.weekday() == Weekday::Monday
        && first(&state, "weekly", today).await
    {
        done.push(format!("{} weekly summaries", weekly(&state, today).await?));
    }
    let keep = now - Duration::days(14);
    notifications::forget_digest_before(&state.pg, keep).await?;
    notifications::forget_sent_before(&state.pg, today - Duration::days(2)).await?;
    Ok(done.join(", "))
}

// Each report runs once a day across instances; if Valkey is down it may run again, and the
// per-day dedupe in `notifications_sent` keeps the second run from sending anything.
async fn first(state: &AppState, what: &str, day: Date) -> bool {
    let key = state.valkey.key(&format!("notices:{what}:{day}"));
    let mut conn = state.valkey.conn();
    let set: Result<Option<String>, _> = redis::cmd("SET")
        .arg(&key)
        .arg(1)
        .arg("NX")
        .arg("EX")
        .arg(2 * 24 * 60 * 60)
        .query_async(&mut conn)
        .await;
    !matches!(set, Ok(None))
}

fn links(state: &AppState) -> Links {
    Links {
        settings: format!("{}settings/account/notifications", state.config.public_url),
        unsubscribe: None,
    }
}

async fn digests(
    state: &AppState,
    cutoff: OffsetDateTime,
    now: OffsetDateTime,
) -> Result<usize, JobError> {
    let mut sent = 0;
    loop {
        let users_waiting = notifications::waiting_users(&state.pg, cutoff, BATCH).await?;
        if users_waiting.is_empty() {
            return Ok(sent);
        }
        let rows = notifications::claim_waiting(&state.pg, &users_waiting, cutoff, now).await?;
        let mut lines: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
        for row in rows {
            if let Some(line) = row.payload.get("line").and_then(|l| l.as_str()) {
                lines.entry(row.user_id).or_default().push(line.to_owned());
            }
        }
        let ids: Vec<Uuid> = lines.keys().copied().collect();
        for person in users::recipients(&state.pg, &ids).await? {
            let (Some(email), Some(lines)) = (&person.email, lines.get(&person.id)) else {
                continue;
            };
            if person.state != "active" {
                continue;
            }
            let lang = Locale::choose(person.locale.as_deref(), None);
            state.mailer.send(notices::digest(
                lang,
                email,
                lines,
                state.config.public_url.as_str(),
                &links(state),
            ));
            sent += 1;
        }
    }
}

fn money(amount: Decimal) -> String {
    format!("${}", amount.round_dp(2))
}

async fn cost_reports(state: &AppState, today: Date) -> Result<usize, JobError> {
    let to = today.previous_day().unwrap_or(today);
    let from = to.replace_day(1).unwrap_or(to);
    let month = format!("{}-{:02}", from.year(), u8::from(from.month()));
    let mut sent = 0;
    for org in orgs::active_ids(&state.pg).await? {
        let by_project =
            usage::usage(&state.pg, org, None, from, to, usage::GroupBy::Project).await?;
        let total: Decimal = by_project.iter().map(|p| p.cost).sum();
        if by_project.iter().all(|p| p.requests == 0) {
            continue;
        }
        let mut top: Vec<_> = by_project.iter().collect();
        top.sort_by_key(|p| std::cmp::Reverse(p.cost));
        let mut projects = Vec::new();
        for p in top.into_iter().take(TOP) {
            let Ok(project) = p.key.parse::<Uuid>() else {
                continue;
            };
            let name =
                crate::services::notifications::triggers::project_name(state, org, project).await?;
            projects.push(ProjectUse {
                name,
                requests: None,
                cost: money(p.cost),
            });
        }
        let place = crate::services::notifications::triggers::org_name(state, org).await?;
        let total = money(total);
        let send = Send {
            kind: Kind::CostReport,
            org: Some(org),
            project: None,
            place: Some(&place),
            subject: format!("cost:{month}"),
            open: format!("{}/usage", settings_path(org)),
            actor: None,
        };
        let to = org_holders(state, org, &[OrgRight::CostsOrgView]).await?;
        let fact = Fact::CostReport {
            month: &month,
            total: &total,
            projects,
        };
        sent += deliver(state, &send, &fact, to).await?;
    }
    Ok(sent)
}

async fn weekly(state: &AppState, today: Date) -> Result<usize, JobError> {
    let to = today.previous_day().unwrap_or(today);
    let from = today - Duration::days(7);
    let mut sent = 0;
    for user in notifications::opted_in(&state.pg, Kind::WeeklySummary.as_str()).await? {
        let orgs_of = members::orgs_of_user(&state.pg, user).await?;
        let mut grants = BTreeMap::new();
        for o in &orgs_of {
            if let Ok(g) = org_from_store(&o.grant()) {
                grants.insert(o.org_id, g);
            }
        }
        let mut requests = 0_i64;
        let mut calls = 0_i64;
        let mut cost = Decimal::ZERO;
        let mut models: BTreeMap<String, i64> = BTreeMap::new();
        let mut projects = Vec::new();
        for p in members::projects_of_user(&state.pg, user).await? {
            let own = project_from_store(&p.grant()).ok();
            let held = effective_project_rights(grants.get(&p.org_id), own.as_ref());
            if !held.contains(ProjectRight::UsageView) {
                continue;
            }
            let rows = usage::usage(
                &state.pg,
                p.org_id,
                Some(p.project_id),
                from,
                to,
                usage::GroupBy::Model,
            )
            .await?;
            let tools = usage::tool_usage(
                &state.pg,
                p.org_id,
                Some(p.project_id),
                from,
                to,
                usage::GroupBy::Project,
            )
            .await?;
            let r: i64 = rows.iter().map(|m| m.requests).sum();
            let c: Decimal = rows.iter().map(|m| m.cost).sum();
            for m in &rows {
                *models.entry(m.key.clone()).or_default() += m.requests;
            }
            requests += r;
            cost += c;
            calls += tools.iter().map(|t| t.calls).sum::<i64>();
            if r > 0 {
                projects.push((
                    c,
                    ProjectUse {
                        name: p.project_name.clone(),
                        requests: Some(r.to_string()),
                        cost: money(c),
                    },
                ));
            }
        }
        if requests == 0 && calls == 0 {
            continue;
        }
        projects.sort_by_key(|p| std::cmp::Reverse(p.0));
        let mut top: Vec<(String, i64)> = models.into_iter().collect();
        top.sort_by_key(|m| std::cmp::Reverse(m.1));
        let fact = Fact::WeeklySummary {
            requests: requests.to_string(),
            tool_calls: calls.to_string(),
            cost: money(cost),
            projects: projects.into_iter().take(TOP).map(|(_, l)| l).collect(),
            top_models: top.into_iter().take(3).map(|(m, _)| m).collect(),
        };
        let send = Send {
            kind: Kind::WeeklySummary,
            org: None,
            project: None,
            place: None,
            subject: format!("weekly:{today}"),
            open: String::new(),
            actor: None,
        };
        let to = BTreeMap::from([(user, Because::Right(ProjectRight::UsageView.as_str()))]);
        sent += deliver(state, &send, &fact, to).await?;
    }
    Ok(sent)
}
