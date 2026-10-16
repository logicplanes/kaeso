use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use futures::future::BoxFuture;

use kaeso_core::cost;
use kaeso_core::id::{ApiKeyId, OrgId};
use kaeso_core::limits::{self, Limit, Subject};
use kaeso_kpp::Usage;
use kaeso_mail::{Budget, BudgetScope, LimitKind, LimitPeriod};
use kaeso_store::StoreError;
use kaeso_store::pg::limits::{self as store, UsageRow};
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::model_api::hooks::{Outcome, Sink};
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
);

#[derive(Debug, Default)]
pub struct Batch {
    rows: Mutex<HashMap<BatchKey, UsageRow>>,
}

impl Batch {
    pub fn add(&self, row: UsageRow) {
        let key = (
            row.day,
            row.project_id,
            row.api_key_id,
            row.user_id,
            row.service_account_id,
            row.provider.clone(),
            row.model_id.clone(),
        );
        let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        match rows.get_mut(&key) {
            Some(r) => merge(r, &row),
            None => {
                rows.insert(key, row);
            }
        }
    }

    pub fn take(&self) -> Vec<UsageRow> {
        let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        rows.drain().map(|(_, r)| r).collect()
    }

    pub fn restore(&self, rows: Vec<UsageRow>) {
        for r in rows {
            self.add(r);
        }
    }

    pub fn pending(
        &self,
        project: Uuid,
        subject: Subject,
        model: Option<Uuid>,
        since: Option<Date>,
    ) -> (i64, Decimal, i64) {
        let rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        rows.values()
            .filter(|r| r.project_id == project)
            .filter(|r| match subject {
                Subject::Project(_) => true,
                Subject::User(u) => r.user_id == Some(u),
                Subject::ServiceAccount(s) => r.service_account_id == Some(s),
                Subject::Key(k) => r.api_key_id == Some(k),
            })
            .filter(|r| model.is_none_or(|m| r.catalogue_model_id == Some(m)))
            .filter(|r| since.is_none_or(|d| r.day >= d))
            .fold((0, Decimal::ZERO, 0), |(t, c, n), r| {
                (
                    t + r.input_tokens
                        + r.output_tokens
                        + r.cache_read_tokens
                        + r.cache_write_tokens,
                    c + r.cost,
                    n + r.requests - r.errors,
                )
            })
    }
}

fn merge(into: &mut UsageRow, add: &UsageRow) {
    into.requests += add.requests;
    into.errors += add.errors;
    into.input_tokens += add.input_tokens;
    into.output_tokens += add.output_tokens;
    into.cache_read_tokens += add.cache_read_tokens;
    into.cache_write_tokens += add.cache_write_tokens;
    into.reasoning_tokens += add.reasoning_tokens;
    into.cost += add.cost;
}

fn i(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

pub async fn flush(state: &AppState) -> Result<usize, StoreError> {
    let rows = state.usage.take();
    let n = rows.len();
    if let Err(e) = store::add_usage(&state.pg, &rows).await {
        state.usage.restore(rows);
        return Err(e);
    }
    Ok(n)
}

pub fn costed(outcome: &Outcome) -> (Usage, Decimal) {
    let usage = outcome.usage.clone().unwrap_or_default();
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
        provider: outcome.provider.unwrap_or_default().to_owned(),
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
    let request_id = outcome.request_id.to_string();
    let at = outcome.started_at;
    let charge = Charge {
        tokens: i(cost::total_tokens(&usage)),
        nano_usd: cost::nano_usd(c),
    };
    match engine::charge(state, &caller, entry, &request_id, at, charge).await {
        Ok(moved) => {
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
            tracing::warn!(error = %e, "cannot add usage to the limit counters; PostgreSQL still records it");
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
            "{}/projects/{}/limits",
            state.config.public_url.as_str().trim_end_matches('/'),
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
        if let Ok(mut conn) = state.pg.acquire().await {
            let _ = crate::platform::audit::system(
                &mut conn,
                Some(OrgId::from_uuid(org)),
                "limit.threshold_reached",
                Some(("limit", kaeso_core::id::LimitId::from_uuid(limit.id).to_string())),
                Some(serde_json::json!({ "threshold": t, "used": used, "amount": limit.units(), "recipients": to.len() })),
                at,
            )
            .await;
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
    Ok((u.state == "active" && u.notify_budget_alerts)
        .then_some(u.email.map(|e| (u.id, e)))
        .flatten())
}

#[cfg(test)]
mod tests {

    use super::*;

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
