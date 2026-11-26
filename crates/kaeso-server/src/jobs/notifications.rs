use std::collections::BTreeMap;

use kaeso_core::notifications::Kind;
use kaeso_core::rights::{OrgRight, ProjectRight, effective_project_rights};
use kaeso_mail::notices::{self, Because, DigestItem, Fact, Links, ProjectUse};
use kaeso_mail::{Amount, Locale};
use kaeso_store::pg::{instance, members, notifications, orgs, usage, users};
use rust_decimal::Decimal;
use time::macros::time;
use time::{Date, Duration, OffsetDateTime, Time, Weekday};
use uuid::Uuid;

use crate::jobs::{JobError, Ran};
use crate::platform::grants::{org_from_store, project_from_store};
use crate::services::exchange_rates::Money;
use crate::services::notifications::dispatch::{Send, deliver, org_holders, org_path};
use crate::state::AppState;

// Spec: digests, the monthly cost report and the weekly summary go out at 8:31 in the morning.
const AT: Time = time!(8:31);
const BATCH: i64 = 100;
const TOP: usize = 5;

pub async fn run(state: AppState) -> Result<Ran, JobError> {
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
    let digests = digests(&state, cutoff, now).await?;
    let mut sent = digests;
    let mut done = vec![format!("{digests} digests")];
    let mut failed = Vec::new();
    if local.time() >= AT && today.day() == 1 && first(&state, "cost", today).await {
        let outcome = cost_reports(&state, today).await;
        sent += settle(
            &state,
            "cost",
            today,
            "cost reports",
            outcome,
            &mut done,
            &mut failed,
        )
        .await;
    }
    if local.time() >= AT
        && today.weekday() == Weekday::Monday
        && first(&state, "weekly", today).await
    {
        let outcome = weekly(&state, today).await;
        sent += settle(
            &state,
            "weekly",
            today,
            "weekly summaries",
            outcome,
            &mut done,
            &mut failed,
        )
        .await;
    }
    let keep = now - Duration::days(14);
    notifications::forget_digest_before(&state.pg, keep).await?;
    notifications::forget_sent_before(&state.pg, today - Duration::days(2)).await?;
    if !failed.is_empty() {
        done.extend(failed);
        return Err(JobError::Incomplete(done.join(", ")));
    }
    Ok(Ran::of(done.join(", "), sent > 0))
}

#[derive(Default)]
struct Reports {
    sent: usize,
    failed: usize,
}

// A report that didn't reach everyone gives its day's claim back, so the next run, a minute
// later, tries again; `claim_subject` keeps anyone who already got it from getting it twice.
async fn settle(
    state: &AppState,
    what: &str,
    day: Date,
    label: &str,
    outcome: Result<Reports, JobError>,
    done: &mut Vec<String>,
    failed: &mut Vec<String>,
) -> usize {
    match outcome {
        Ok(r) if r.failed == 0 => {
            done.push(format!("{} {label}", r.sent));
            r.sent
        }
        Ok(r) => {
            release(state, what, day).await;
            failed.push(format!("{} {label}, {} failed", r.sent, r.failed));
            r.sent
        }
        Err(e) => {
            release(state, what, day).await;
            failed.push(format!("{label} failed: {e}"));
            0
        }
    }
}

async fn release(state: &AppState, what: &str, day: Date) {
    let key = state.valkey.key(&format!("notices:{what}:{day}"));
    let mut conn = state.valkey.conn();
    let deleted: Result<(), redis::RedisError> =
        redis::cmd("DEL").arg(&key).query_async(&mut conn).await;
    if let Err(e) = deleted {
        tracing::warn!(error = %e, what, "cannot give back a report's claim in Valkey; it is retried in two days at the latest");
    }
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
        let mut lines: BTreeMap<Uuid, Vec<DigestItem>> = BTreeMap::new();
        for row in rows {
            let text = |key: &str| {
                row.payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            };
            if let Some(line) = text("line") {
                lines.entry(row.user_id).or_default().push(DigestItem {
                    line,
                    organization: text("organization"),
                    project: text("project"),
                });
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

fn plain(amount: Decimal) -> String {
    format!("{:.2}", amount.round_dp(2))
}

// Each day's spend at that day's rate, by project.
async fn converted_costs(
    state: &AppState,
    money: &Money,
    org: Uuid,
    project: Option<Uuid>,
    from: Date,
    to: Date,
) -> Result<BTreeMap<String, Decimal>, JobError> {
    let mut sums: BTreeMap<String, Decimal> = BTreeMap::new();
    for (key, day, cost) in usage::costs_by_day(
        &state.pg,
        org,
        project,
        from,
        to,
        usage::GroupBy::Project,
        &usage::Filter::default(),
    )
    .await?
    {
        *sums.entry(key).or_default() += money.on(cost, day).unwrap_or_default();
    }
    Ok(sums)
}

async fn cost_reports(state: &AppState, today: Date) -> Result<Reports, JobError> {
    let to = today.previous_day().unwrap_or(today);
    let from = to.replace_day(1).unwrap_or(to);
    let month = format!("{}-{:02}", from.year(), u8::from(from.month()));
    let mut r = Reports::default();
    for org in orgs::active_ids(&state.pg).await? {
        match cost_report(state, org, from, to, &month).await {
            Ok(n) => r.sent += n,
            Err(e) => {
                tracing::warn!(error = %e, %org, "a monthly cost report couldn't be sent; the next run tries again");
                r.failed += 1;
            }
        }
    }
    Ok(r)
}

async fn cost_report(
    state: &AppState,
    org: Uuid,
    from: Date,
    to: Date,
    month: &str,
) -> Result<usize, JobError> {
    let by_project = usage::usage(
        &state.pg,
        org,
        None,
        from,
        to,
        usage::GroupBy::Project,
        &usage::Filter::default(),
    )
    .await?;
    if by_project.iter().all(|p| p.requests == 0) {
        return Ok(0);
    }
    let money = Money::of_org(state, org).await?;
    let converted = converted_costs(state, &money, org, None, from, to).await?;
    let in_currency = |key: &str| converted.get(key).copied().unwrap_or_default();
    let total: Decimal = converted.values().sum();
    let place = crate::services::notifications::triggers::org_name(state, org).await?;
    let mut top: Vec<_> = by_project.iter().collect();
    top.sort_by_key(|p| std::cmp::Reverse(in_currency(&p.key)));
    let mut projects = Vec::new();
    for p in top.into_iter().take(TOP) {
        let Ok(project) = p.key.parse::<Uuid>() else {
            continue;
        };
        let name =
            crate::services::notifications::triggers::project_name(state, org, project).await?;
        projects.push(ProjectUse {
            organization: place.clone(),
            name,
            requests: None,
            cost: Some(Amount::new(&plain(in_currency(&p.key)), &money.code)),
        });
    }
    let total = Amount::new(&plain(total), &money.code);
    let send = Send {
        kind: Kind::CostReport,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: format!("cost:{org}:{month}"),
        open: format!(
            "{}/analytics?from={from}&to={to}&metric=cost",
            org_path(org)
        ),
        actor: None,
    };
    let to = org_holders(state, org, &[OrgRight::CostsOrgView]).await?;
    let fact = Fact::CostReport {
        month: from,
        total: &total,
        projects,
    };
    Ok(deliver(state, &send, &fact, to).await?)
}

async fn weekly(state: &AppState, today: Date) -> Result<Reports, JobError> {
    let to = today.previous_day().unwrap_or(today);
    let from = today - Duration::days(7);
    let mut r = Reports::default();
    for user in notifications::opted_in(&state.pg, Kind::WeeklySummary.as_str()).await? {
        match weekly_summary(state, user, today, from, to).await {
            Ok(n) => r.sent += n,
            Err(e) => {
                tracing::warn!(error = %e, %user, "a weekly summary couldn't be sent; the next run tries again");
                r.failed += 1;
            }
        }
    }
    Ok(r)
}

async fn weekly_summary(
    state: &AppState,
    user: Uuid,
    today: Date,
    from: Date,
    to: Date,
) -> Result<usize, JobError> {
    let orgs_of = members::orgs_of_user(&state.pg, user).await?;
    let names: BTreeMap<Uuid, &str> = orgs_of
        .iter()
        .map(|o| (o.org_id, o.org_name.as_str()))
        .collect();
    let mut grants = BTreeMap::new();
    for o in &orgs_of {
        if let Ok(g) = org_from_store(&o.grant()) {
            grants.insert(o.org_id, g);
        }
    }
    let mut requests = 0_i64;
    let mut calls = 0_i64;
    let mut cost: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut cost_partial = false;
    let mut currencies: BTreeMap<Uuid, Money> = BTreeMap::new();
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
            &usage::Filter::default(),
        )
        .await?;
        let tools = usage::tool_usage(
            &state.pg,
            p.org_id,
            Some(p.project_id),
            from,
            to,
            usage::GroupBy::Project,
            &usage::Filter::default(),
        )
        .await?;
        let r: i64 = rows.iter().map(|m| m.requests).sum();
        let t: i64 = tools.iter().map(|t| t.calls).sum();
        let org_costs = grants
            .get(&p.org_id)
            .is_some_and(|g| g.effective().contains(OrgRight::CostsOrgView));
        let c = if held.contains(ProjectRight::CostsView) || org_costs {
            let money = match currencies.entry(p.org_id) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert(Money::of_org(state, p.org_id).await?)
                }
            };
            let money = &*money;
            let c: Decimal = converted_costs(state, money, p.org_id, Some(p.project_id), from, to)
                .await?
                .values()
                .sum();
            if r > 0 || t > 0 || !c.is_zero() {
                *cost.entry(money.code.clone()).or_default() += c;
            }
            Some((c, Amount::new(&plain(c), &money.code)))
        } else {
            cost_partial |= r > 0 || t > 0;
            None
        };
        for m in &rows {
            *models.entry(m.key.clone()).or_default() += m.requests;
        }
        requests += r;
        calls += t;
        if r > 0 {
            let (rank, shown) = c.map_or((Decimal::ZERO, None), |(c, a)| (c, Some(a)));
            projects.push((
                (rank, r),
                p.org_id,
                ProjectUse {
                    organization: names
                        .get(&p.org_id)
                        .map_or_else(|| p.org_id.to_string(), |n| (*n).to_owned()),
                    name: p.project_name.clone(),
                    requests: Some(r.to_string()),
                    cost: shown,
                },
            ));
        }
    }
    if requests == 0 && calls == 0 {
        return Ok(0);
    }
    // Ranking by visible cost would push a busy project whose cost is hidden out of the top.
    if cost_partial {
        projects.sort_by_key(|p| std::cmp::Reverse(p.0.1));
    } else {
        projects.sort_by_key(|p| std::cmp::Reverse(p.0));
    }
    let mut top: Vec<(String, i64)> = models.into_iter().collect();
    top.sort_by_key(|m| std::cmp::Reverse(m.1));
    let open = projects.first().map_or_else(String::new, |(_, org, _)| {
        format!(
            "{}/analytics/breakdown?from={from}&to={to}&by=project",
            org_path(*org)
        )
    });
    let fact = Fact::WeeklySummary {
        from,
        to,
        requests: requests.to_string(),
        tool_calls: calls.to_string(),
        cost: cost
            .iter()
            .map(|(code, c)| Amount::new(&plain(*c), code))
            .collect(),
        cost_partial,
        projects: projects.into_iter().take(TOP).map(|(_, _, l)| l).collect(),
        top_models: top.into_iter().take(3).map(|(m, _)| m).collect(),
    };
    let send = Send {
        kind: Kind::WeeklySummary,
        org: None,
        project: None,
        place: None,
        subject: format!("weekly:{today}"),
        open,
        actor: None,
    };
    let to = BTreeMap::from([(user, Because::Right(ProjectRight::UsageView.as_str()))]);
    Ok(deliver(state, &send, &fact, to).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_a_plain_amount_with_cents_for_the_email_to_format() {
        let amount = |s: &str| s.parse::<Decimal>().unwrap_or_default();
        assert_eq!(plain(amount("41.2")), "41.20");
        assert_eq!(plain(amount("1412.505")), "1412.50");
        assert_eq!(plain(amount("0")), "0.00");
        assert_eq!(plain(amount("3.14159")), "3.14");
    }
}
