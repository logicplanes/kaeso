use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use futures::future::BoxFuture;

use kaeso_core::cost;
use kaeso_core::id::{ApiKeyId, OrgId};
use kaeso_core::limits::{self, Limit, Subject};
use kaeso_core::notifications::Kind;
use kaeso_kpp::Usage;
use kaeso_mail::{Budget, BudgetScope, LimitKind, LimitPeriod};
use kaeso_store::StoreError;
use kaeso_store::pg::limits::{self as store, UsageRow};
use kaeso_store::pg::notifications;
use kaeso_store::pg::usage as store_usage;
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::model_api::hooks::{Outcome, Sink};
use crate::platform::audit::Action;
use crate::services::limits::{self as engine, Charge};
use crate::state::AppState;

type BatchKey = (
    Date,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    String,
    String,
    String,
    String,
);

// Rows a flush took stay counted until PostgreSQL has them: a limit counter rebuilt while the
// write is under way would otherwise miss them for the rest of its period.
#[derive(Debug, Default)]
pub struct Batch {
    rows: Mutex<Rows>,
}

#[derive(Debug, Default)]
struct Rows {
    open: HashMap<BatchKey, UsageRow>,
    flushing: HashMap<u64, Vec<UsageRow>>,
    next: u64,
}

#[derive(Debug)]
pub struct Taken {
    id: u64,
    pub rows: Vec<UsageRow>,
}

fn key_of(row: &UsageRow) -> BatchKey {
    (
        row.day,
        row.project_id,
        row.api_key_id,
        row.user_id,
        row.service_account_id,
        row.provider.clone(),
        row.model_id.clone(),
        row.ingress_dialect.clone(),
        row.egress_dialect.clone(),
    )
}

fn add_to(open: &mut HashMap<BatchKey, UsageRow>, row: UsageRow) {
    match open.get_mut(&key_of(&row)) {
        Some(r) => merge(r, &row),
        None => {
            open.insert(key_of(&row), row);
        }
    }
}

impl Batch {
    fn lock(&self) -> std::sync::MutexGuard<'_, Rows> {
        self.rows.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn add(&self, row: UsageRow) {
        add_to(&mut self.lock().open, row);
    }

    pub fn take(&self) -> Taken {
        let mut state = self.lock();
        let mut rows: Vec<UsageRow> = state.open.drain().map(|(_, r)| r).collect();
        rows.sort_by(|a, b| unique_key(a).cmp(&unique_key(b)));
        let id = state.next;
        state.next = state.next.wrapping_add(1);
        state.flushing.insert(id, rows.clone());
        Taken { id, rows }
    }

    pub fn written(&self, id: u64) {
        self.lock().flushing.remove(&id);
    }

    pub fn restore(&self, id: u64, rows: Vec<UsageRow>) {
        let mut state = self.lock();
        state.flushing.remove(&id);
        for r in rows {
            add_to(&mut state.open, r);
        }
    }

    pub fn pending(
        &self,
        project: Uuid,
        subject: Subject,
        model: Option<Uuid>,
        since: Option<Date>,
    ) -> (i64, Decimal, i64) {
        let state = self.lock();
        state
            .open
            .values()
            .chain(state.flushing.values().flatten())
            .filter(|r| r.project_id == project)
            .filter(|r| match subject {
                Subject::Project(_) => true,
                Subject::User(u) => r.user_id == Some(u),
                Subject::ServiceAccount(s) => r.service_account_id == Some(s),
                Subject::Key(k) => r.api_key_id == Some(k),
            })
            .filter(|r| model.is_none_or(|m| r.catalogue_model_id == Some(m)))
            .filter(|r| since.is_none_or(|d| r.day >= d))
            .fold((0_i64, Decimal::ZERO, 0_i64), |(t, c, n), r| {
                (
                    t.saturating_add(r.input_tokens)
                        .saturating_add(r.output_tokens)
                        .saturating_add(r.cache_read_tokens)
                        .saturating_add(r.cache_write_tokens),
                    c.saturating_add(r.cost),
                    n.saturating_add(r.requests.saturating_sub(r.errors)),
                )
            })
    }
}

type UniqueKey<'a> = (
    Uuid,
    Date,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
);

fn unique_key(r: &UsageRow) -> UniqueKey<'_> {
    (
        r.project_id,
        r.day,
        r.api_key_id,
        r.user_id,
        r.service_account_id,
        &r.provider,
        &r.model_id,
        &r.ingress_dialect,
        &r.egress_dialect,
    )
}

fn merge(into: &mut UsageRow, add: &UsageRow) {
    into.requests = into.requests.saturating_add(add.requests);
    into.errors = into.errors.saturating_add(add.errors);
    into.input_tokens = into.input_tokens.saturating_add(add.input_tokens);
    into.output_tokens = into.output_tokens.saturating_add(add.output_tokens);
    into.cache_read_tokens = into.cache_read_tokens.saturating_add(add.cache_read_tokens);
    into.cache_write_tokens = into
        .cache_write_tokens
        .saturating_add(add.cache_write_tokens);
    into.reasoning_tokens = into.reasoning_tokens.saturating_add(add.reasoning_tokens);
    into.cost = into.cost.saturating_add(add.cost);
}

fn i(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn never_fits(e: &StoreError) -> bool {
    let StoreError::Postgres(sqlx::Error::Database(db)) = e else {
        return false;
    };
    db.code()
        .is_some_and(|code| code.starts_with("22") || code.starts_with("23"))
}

// Tool calls are counted in memory per day, project and server, so a warm call writes nothing;
// the same flush as model usage writes them.
#[derive(Debug, Default)]
pub struct ToolBatch(std::sync::Mutex<HashMap<ToolKey, (i64, i64)>>);

type ToolKey = (time::Date, Uuid, Uuid, Uuid);

impl ToolBatch {
    pub fn add(
        &self,
        at: time::OffsetDateTime,
        org: Uuid,
        project: Uuid,
        server: Uuid,
        failed: bool,
    ) {
        let day = at.to_offset(time::UtcOffset::UTC).date();
        let mut open = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counts = open.entry((day, org, project, server)).or_default();
        counts.0 += 1;
        counts.1 += i64::from(failed);
    }

    fn take(&self) -> Vec<store_usage::ToolUsageRow> {
        let mut open = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        open.drain()
            .map(|((day, org_id, project_id, server_id), (calls, errors))| {
                store_usage::ToolUsageRow {
                    day,
                    org_id,
                    project_id,
                    server_id,
                    calls,
                    errors,
                }
            })
            .collect()
    }

    fn restore(&self, rows: Vec<store_usage::ToolUsageRow>) {
        let mut open = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for r in rows {
            let counts = open
                .entry((r.day, r.org_id, r.project_id, r.server_id))
                .or_default();
            counts.0 += r.calls;
            counts.1 += r.errors;
        }
    }
}

async fn flush_tools(state: &AppState) -> Result<usize, StoreError> {
    let rows = state.tool_usage.take();
    let n = rows.len();
    match store_usage::add_tool_usage(&state.pg, &rows).await {
        Ok(()) => Ok(n),
        Err(e) if never_fits(&e) => {
            metrics::counter!(crate::metrics::USAGE_ROWS_DROPPED).increment(n as u64);
            tracing::error!(error = %e, rows = n, "tool usage cannot be recorded; it is dropped");
            Ok(0)
        }
        Err(e) => {
            state.tool_usage.restore(rows);
            Err(e)
        }
    }
}

pub async fn flush(state: &AppState) -> Result<usize, StoreError> {
    let tools = flush_tools(state).await;
    let models = flush_models(state).await;
    Ok(tools? + models?)
}

async fn flush_models(state: &AppState) -> Result<usize, StoreError> {
    let taken = state.usage.take();
    let n = taken.rows.len();
    match store::add_usage(&state.pg, &taken.rows).await {
        Ok(()) => {
            state.usage.written(taken.id);
            Ok(n)
        }
        Err(e) if e.is_unavailable() || e.is_conflict() => {
            state.usage.restore(taken.id, taken.rows);
            Err(e)
        }
        Err(e) => {
            tracing::warn!(error = %e, rows = n, "cannot record usage in one batch; recording it row by row");
            flush_rows(state, taken).await
        }
    }
}

async fn flush_rows(state: &AppState, taken: Taken) -> Result<usize, StoreError> {
    let Taken { id, rows } = taken;
    let mut written = 0;
    let mut rows = rows.into_iter();
    while let Some(row) = rows.next() {
        match store::add_usage(&state.pg, std::slice::from_ref(&row)).await {
            Ok(()) => written += 1,
            Err(e) if never_fits(&e) => {
                metrics::counter!(crate::metrics::USAGE_ROWS_DROPPED).increment(1);
                tracing::error!(
                    error = %e,
                    org_id = %row.org_id,
                    project_id = %row.project_id,
                    day = %row.day,
                    requests = row.requests,
                    "a usage row cannot be recorded; it is dropped"
                );
            }
            Err(e) => {
                state
                    .usage
                    .restore(id, std::iter::once(row).chain(rows).collect());
                return Err(e);
            }
        }
    }
    state.usage.written(id);
    Ok(written)
}

pub fn costed(outcome: &Outcome) -> (Usage, Decimal) {
    let usage = outcome.usage.clone().unwrap_or_default();
    if outcome.response_cache == Some(crate::services::response_cache::Verdict::Hit) {
        return (usage, Decimal::ZERO);
    }
    let c = outcome
        .prices
        .as_ref()
        .map_or(Decimal::ZERO, |p| cost::cost(&usage, p));
    (usage, c)
}

#[derive(Debug, Default)]
pub struct Accounting;

impl Sink for Accounting {
    fn finish<'a>(&'a self, state: &'a AppState, outcome: Outcome) -> BoxFuture<'a, ()> {
        Box::pin(async move { account(state, &outcome).await })
    }
}

pub async fn account(state: &AppState, outcome: &Outcome) {
    let Some(caller) = outcome.caller.clone() else {
        return;
    };
    let (usage, c) = costed(outcome);
    state.usage.add(UsageRow {
        day: outcome.started_at.to_offset(time::UtcOffset::UTC).date(),
        org_id: caller.org_id,
        project_id: caller.project_id,
        user_id: caller.user_id,
        service_account_id: caller.service_account_id,
        api_key_id: caller.key_id,
        catalogue_model_id: outcome.catalogue_model_id,
        model_id: outcome.model_id.clone().unwrap_or_default(),
        provider: outcome
            .custom_provider
            .clone()
            .unwrap_or_else(|| outcome.provider.unwrap_or_default().to_owned()),
        ingress_dialect: outcome.ingress.as_str().to_owned(),
        egress_dialect: outcome
            .egress
            .map(|d| d.as_str().to_owned())
            .unwrap_or_default(),
        requests: 1,
        errors: i64::from(outcome.is_error()),
        input_tokens: i(usage.input_tokens),
        output_tokens: i(usage.output_tokens),
        cache_read_tokens: i(usage.cache_read_tokens),
        cache_write_tokens: i(usage.cache_write_tokens),
        reasoning_tokens: i(usage.reasoning_tokens.unwrap_or(0)),
        cost: c,
    });
    let (Some(entry), Some(provider)) = (outcome.catalogue_model_id, outcome.provider) else {
        return;
    };
    for (kind, n) in [
        ("input", usage.input_tokens),
        ("output", usage.output_tokens),
        ("cache_read", usage.cache_read_tokens),
        ("cache_write", usage.cache_write_tokens),
        ("reasoning", usage.reasoning_tokens.unwrap_or(0)),
    ] {
        if n > 0 {
            metrics::counter!(crate::metrics::TOKENS, "provider" => provider, "kind" => kind)
                .increment(n);
        }
    }
    let micro = (c * Decimal::from(1_000_000)).round();
    metrics::counter!(crate::metrics::COST, "provider" => provider)
        .increment(rust_decimal::prelude::ToPrimitive::to_u64(&micro).unwrap_or(0));
    if let Some(t) = outcome.ttfb {
        metrics::histogram!(crate::metrics::TIME_TO_FIRST_BYTE, "provider" => provider)
            .record(t.as_secs_f64());
    }
    if let Some(t) = outcome.overhead {
        metrics::histogram!(crate::metrics::GATEWAY_OVERHEAD, "provider" => provider)
            .record(t.as_secs_f64());
    }
    if let Some(t) = outcome.upload {
        metrics::histogram!(crate::metrics::REQUEST_UPLOAD, "provider" => provider)
            .record(t.as_secs_f64());
    }
    let request_id = outcome.request_id.to_string();
    let at = outcome.started_at;
    let charge = Charge {
        tokens: i(cost::total_tokens(&usage)),
        nano_usd: cost::nano_usd(c),
        succeeded: !outcome.is_error(),
    };
    match engine::charge(state, &caller, entry, &request_id, at, charge).await {
        Ok(moved) => {
            let used_up: Vec<_> = moved
                .iter()
                .filter(|(limit, before, after)| {
                    limit.hard
                        && !limit.kind.is_rate()
                        && *before < limit.units()
                        && *after >= limit.units()
                })
                .map(|(limit, _, _)| limit.clone())
                .collect();
            if !used_up.is_empty() {
                let task = state.clone();
                state.in_flight.spawn(async move {
                    for limit in used_up {
                        let period = limit.period.start(at).unwrap_or(Date::MIN);
                        let budget = budget_of(&limit);
                        crate::services::notifications::system::limit_reached(
                            &task,
                            caller.org_id,
                            caller.project_id,
                            &limit,
                            &budget,
                            period,
                        )
                        .await;
                    }
                });
            }
            let alerts: Vec<_> = moved
                .into_iter()
                .filter_map(|(limit, before, after)| {
                    let crossed = limits::crossed(&limit.thresholds, before, after, limit.units());
                    (!crossed.is_empty()).then_some((limit, crossed, after))
                })
                .collect();
            if !alerts.is_empty() {
                let task = state.clone();
                state.in_flight.spawn(async move {
                    for (limit, crossed, after) in alerts {
                        alert(
                            &task,
                            caller.org_id,
                            caller.project_id,
                            &limit,
                            &crossed,
                            at,
                            after,
                        )
                        .await;
                    }
                });
            }
        }
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "charge")
                .increment(1);
            tracing::warn!(error = %e, "cannot add usage to the limit counters; PostgreSQL still records it and the counters are rebuilt from it");
        }
    }
}

async fn alert(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    limit: &Limit,
    thresholds: &[u8],
    at: OffsetDateTime,
    used: i64,
) {
    let period_start = limit.period.start(at).unwrap_or(Date::MIN);
    for t in thresholds {
        match store::mark_alert(&state.pg, limit.id, period_start, *t).await {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                tracing::warn!(error = %e, "cannot record a budget alert");
                continue;
            }
        }
        let mut to = match store::alert_recipients(&state.pg, project).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "cannot find budget alert recipients");
                Vec::new()
            }
        };
        if let Subject::Key(key) = limit.subject
            && let Ok(Some(owner)) = key_owner(state, project, key).await
            && !to.iter().any(|(id, _)| *id == owner.0)
        {
            to.push(owner);
        }
        let key_id = match limit.subject {
            Subject::Key(k) => ApiKeyId::from_uuid(k).to_string(),
            _ => String::new(),
        };
        let scope = match limit.subject {
            Subject::Project(_) => BudgetScope::Project,
            Subject::User(_) | Subject::ServiceAccount(_) => BudgetScope::Member,
            Subject::Key(_) => BudgetScope::Key(&key_id),
        };
        let budget = budget_of(limit);
        let url = format!(
            "{}orgs/{}/projects/{}/limits",
            state.config.public_url,
            OrgId::from_uuid(org),
            kaeso_core::id::ProjectId::from_uuid(project)
        );
        for (user, email) in &to {
            let lang = crate::services::mail_lang::of_user(&state.pg, *user, None).await;
            state.mailer.send(kaeso_mail::templates::budget_alert(
                lang,
                email,
                &budget,
                scope,
                u32::from(*t),
                &url,
            ));
        }
        let audited = match state.pg.acquire().await {
            Ok(mut conn) => {
                crate::platform::audit::system_event(
                    &mut conn,
                    Some(OrgId::from_uuid(org)),
                    Action::LimitThresholdReached,
                    Some(("limit", kaeso_core::id::LimitId::from_uuid(limit.id).to_string())),
                    None,
                    Some(serde_json::json!({ "threshold": t, "used": used, "amount": limit.units(), "recipients": to.len() })),
                    at,
                )
                .await
            }
            Err(e) => Err(StoreError::Postgres(e)),
        };
        if let Err(e) = audited {
            tracing::warn!(error = %e, threshold = t, "cannot audit a budget alert");
        }
    }
}

fn budget_of(limit: &Limit) -> Budget {
    Budget {
        kind: match limit.kind {
            limits::Kind::RequestRate => LimitKind::RequestRate,
            limits::Kind::TokenRate => LimitKind::TokenRate,
            limits::Kind::TokenBudget => LimitKind::TokenBudget,
            limits::Kind::SpendBudget => LimitKind::SpendBudget,
            limits::Kind::ToolCallRate => LimitKind::ToolCallRate,
        },
        period: match limit.period {
            limits::Period::Minute => LimitPeriod::Minute,
            limits::Period::Day => LimitPeriod::Day,
            limits::Period::Month => LimitPeriod::Month,
            limits::Period::Total => LimitPeriod::Total,
        },
        amount: limit.amount.normalize().to_string(),
    }
}

async fn key_owner(
    state: &AppState,
    project: Uuid,
    key: Uuid,
) -> Result<Option<(Uuid, String)>, StoreError> {
    let Some(k) = kaeso_store::pg::keys::get(&state.pg, project, key).await? else {
        return Ok(None);
    };
    let Some(user) = k.user_id else {
        return Ok(None);
    };
    let Some(u) = kaeso_store::pg::users::get(&state.pg, user).await? else {
        return Ok(None);
    };
    let off = !notifications::turned_off(&state.pg, &[u.id], Kind::BudgetAlerts.as_str())
        .await?
        .is_empty();
    let muted = notifications::mutes_of(&state.pg, &[u.id])
        .await?
        .contains(&(u.id, k.org_id));
    Ok((u.state == "active" && !off && !muted)
        .then_some(u.email.map(|e| (u.id, e)))
        .flatten())
}

#[cfg(test)]
mod tests {

    use super::*;

    fn row(project: u128, day: u8, model: &str) -> UsageRow {
        UsageRow {
            day: Date::from_calendar_date(2026, time::Month::October, day).unwrap(),
            org_id: Uuid::nil(),
            project_id: Uuid::from_u128(project),
            user_id: None,
            service_account_id: None,
            api_key_id: None,
            catalogue_model_id: None,
            model_id: model.into(),
            provider: "anthropic".into(),
            ingress_dialect: "anthropic_messages".into(),
            egress_dialect: "anthropic_messages".into(),
            requests: 1,
            errors: 0,
            input_tokens: 1,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost: Decimal::ZERO,
        }
    }

    #[test]
    fn rows_being_written_still_count_until_they_are_written() {
        let batch = Batch::default();
        let project = Uuid::from_u128(1);
        let pending = || {
            batch
                .pending(project, Subject::Project(project), None, None)
                .0
        };
        batch.add(row(1, 1, "a"));
        let first = batch.take();
        assert_eq!(pending(), 2, "taken for a write that has not finished");
        batch.add(row(1, 1, "a"));
        assert_eq!(pending(), 4);
        batch.restore(first.id, first.rows);
        assert_eq!(pending(), 4, "a failed write is counted once");
        let second = batch.take();
        assert_eq!(second.rows.len(), 1);
        assert_eq!(pending(), 4);
        batch.written(second.id);
        assert_eq!(pending(), 0);
    }

    #[test]
    fn batches_are_written_in_the_order_of_their_unique_key() {
        let batch = Batch::default();
        for (p, d, m) in [
            (2, 1, "b"),
            (1, 2, "a"),
            (2, 1, "a"),
            (1, 1, "z"),
            (1, 2, "a"),
        ] {
            batch.add(row(p, d, m));
        }
        let taken: Vec<(u128, u8, String, i64)> = batch
            .take()
            .rows
            .into_iter()
            .map(|r| (r.project_id.as_u128(), r.day.day(), r.model_id, r.requests))
            .collect();
        assert_eq!(
            taken,
            [
                (1, 1, "z".into(), 1),
                (1, 2, "a".into(), 2),
                (2, 1, "a".into(), 1),
                (2, 1, "b".into(), 1)
            ]
        );
    }

    #[test]
    fn requests_on_different_routes_stay_apart() {
        let batch = Batch::default();
        batch.add(row(1, 1, "a"));
        batch.add(row(1, 1, "a"));
        batch.add(UsageRow {
            ingress_dialect: "openai_chat".into(),
            ..row(1, 1, "a")
        });
        let taken: Vec<(String, i64)> = batch
            .take()
            .rows
            .into_iter()
            .map(|r| (r.ingress_dialect, r.requests))
            .collect();
        assert_eq!(
            taken,
            [("anthropic_messages".into(), 2), ("openai_chat".into(), 1)]
        );
    }

    #[test]
    fn budget_alerts_describe_limits_as_they_always_did() {
        for kind in [
            limits::Kind::RequestRate,
            limits::Kind::TokenRate,
            limits::Kind::TokenBudget,
            limits::Kind::SpendBudget,
        ] {
            for period in [
                limits::Period::Minute,
                limits::Period::Day,
                limits::Period::Month,
                limits::Period::Total,
            ] {
                for amount in ["50", "12.50", "1500000"] {
                    let limit = Limit {
                        id: Uuid::nil(),
                        subject: Subject::Project(Uuid::nil()),
                        kind,
                        model: None,
                        period,
                        amount: amount.parse().unwrap(),
                        hard: false,
                        thresholds: vec![80],
                    };
                    assert_eq!(budget_of(&limit).english(), limit.describe());
                }
            }
        }
    }
}
