//! Model lines in a project: names such as `claude-opus-latest` that KPMR keeps pointed at a
//! provider's newest stable model, and how each project follows them.

use kaeso_core::catalogue::Fields;
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_core::kpmr;
use kaeso_mail::notices::{Fact, LineChange, LinePrice};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::ProjectModels;
use kaeso_store::pg::lines::{self as store, LineRow};
use rust_decimal::Decimal;
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::platform::audit::Action;
use crate::platform::error::ApiResult;
use crate::services::notifications::dispatch::{Send, deliver_with, project_holders, project_path};
use crate::state::AppState;

/// How a project follows a line when KPMR moves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Move when KPMR moves.
    Follow,
    /// Wait for an owner to move or keep.
    Ask,
    /// Move this many days after KPMR moved.
    Delay(i32),
    /// Stay until someone unpins.
    Pin,
}

impl Policy {
    pub fn of(policy: &str, delay_days: Option<i32>) -> Policy {
        match (policy, delay_days) {
            ("ask", _) => Policy::Ask,
            ("delay", Some(d)) => Policy::Delay(d),
            ("pin", _) => Policy::Pin,
            _ => Policy::Follow,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Policy::Follow => "follow",
            Policy::Ask => "ask",
            Policy::Delay(_) => "delay",
            Policy::Pin => "pin",
        }
    }

    pub fn delay_days(self) -> Option<i32> {
        match self {
            Policy::Delay(d) => Some(d),
            _ => None,
        }
    }
}

/// Where KPMR points a line now, if it lists the line and its model.
pub fn kpmr_target<'a>(
    snapshot: &'a kpmr::Catalogue,
    provider: &str,
    line: &str,
) -> Option<&'a kpmr::Line> {
    snapshot.providers.get(provider)?.line(line)
}

/// The prices per million tokens that change when a line moves.
pub fn prices(snapshot: &kpmr::Catalogue, provider: &str, from: &str, to: &str) -> Vec<LinePrice> {
    let fields = |m: &str| {
        snapshot
            .model(provider, m)
            .map(Fields::from_kpmr)
            .unwrap_or_default()
    };
    let (a, b) = (fields(from), fields(to));
    let pair = |name: &'static str, x: Option<Decimal>, y: Option<Decimal>| {
        (x != y).then(|| LinePrice {
            price: name,
            from: x.map(|d| d.normalize().to_string()),
            to: y.map(|d| d.normalize().to_string()),
        })
    };
    [
        pair("input", a.input_price, b.input_price),
        pair("output", a.output_price, b.output_price),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Why a line can't move to `to`, if it can't: the project's provider key is missing, or a key,
/// member or service account that may call the line may not call the model the line would reach.
pub async fn held(
    state: &AppState,
    row: &LineRow,
    to: &str,
    models: &ProjectModels,
) -> Option<String> {
    let has_key = crate::services::credentials::resolve(
        state,
        OrgId::from_uuid(row.org_id),
        ProjectId::from_uuid(row.project_id),
        &row.provider,
    )
    .await
    .is_ok();
    if !has_key {
        return Some(format!(
            "No provider key for `{}` is set up for this project, so the line stays on `{}`.",
            row.provider, row.target
        ));
    }
    let target = models
        .catalogue
        .entries()
        .iter()
        .find(|e| e.provider == row.provider && e.model_id == to && !e.is_line())?;
    let restricted = models
        .users
        .values()
        .chain(models.service_accounts.values())
        .chain(models.keys.values())
        .any(|allowed| allowed.contains(&row.catalogue_model_id) && !allowed.contains(&target.id));
    restricted.then(|| {
        format!(
            "Some keys or members may call `{}` but not `{to}`, so the line stays on `{}`.",
            row.line, row.target
        )
    })
}

/// Moves a line to `to` and records it. Gives the model it pointed at before.
pub async fn move_line(
    tx: &mut sqlx::PgConnection,
    row: &LineRow,
    to: &str,
    at: OffsetDateTime,
) -> Result<String, StoreError> {
    store::move_to(&mut *tx, row.catalogue_model_id, to, Some(&row.target), at).await?;
    Ok(row.target.clone())
}

pub fn audited(
    row: &LineRow,
    from: &str,
    to: &str,
    how: &str,
) -> (serde_json::Value, serde_json::Value) {
    (
        json!({ "line": row.line, "provider": row.provider, "target": from }),
        json!({ "line": row.line, "provider": row.provider, "target": to, "how": how }),
    )
}

pub async fn notify(state: &AppState, row: &LineRow, change: LineChange) -> ApiResult<usize> {
    let place =
        crate::services::notifications::triggers::project_name(state, row.org_id, row.project_id)
            .await?;
    let provider =
        crate::services::notifications::triggers::provider_name(state, row.org_id, &row.provider)
            .await?;
    let subject = match &change {
        LineChange::Moved { to, .. } => format!("line.moved:{}:{to}", row.catalogue_model_id),
        LineChange::Available { to, .. } => {
            format!("line.move_available:{}:{to}", row.catalogue_model_id)
        }
    };
    let send = Send {
        kind: kaeso_core::notifications::Kind::ModelLines,
        org: Some(row.org_id),
        project: Some(row.project_id),
        place: Some(&place),
        subject,
        open: format!(
            "{}/models?model={}",
            project_path(row.org_id, row.project_id),
            CatalogueModelId::from_uuid(row.catalogue_model_id)
        ),
        actor: None,
    };
    let to = project_holders(
        state,
        row.org_id,
        row.project_id,
        &[kaeso_core::rights::ProjectRight::ModelsManage],
    )
    .await?;
    let ids = json!({
        "catalogue_model_id": CatalogueModelId::from_uuid(row.catalogue_model_id).to_string(),
        "provider_id": row.provider,
    });
    deliver_with(
        state,
        &send,
        &Fact::Line {
            provider: &provider,
            project: &place,
            line: &row.line,
            change,
        },
        to,
        Some(ids),
    )
    .await
}

/// What one run of the lines job did.
#[derive(Debug, Default)]
pub struct Run {
    pub moved: usize,
    pub waiting: usize,
    pub held: usize,
}

/// Follows KPMR for every line of every project, each by its policy.
pub async fn run(state: &AppState) -> ApiResult<Run> {
    let loaded = state.kpmr.current();
    let snapshot = &loaded.snapshot;
    let now = state.clock.now();
    let mut run = Run::default();
    for row in store::all(&state.pg).await? {
        let Some(line) = kpmr_target(snapshot, &row.provider, &row.line) else {
            continue;
        };
        Box::pin(one(state, snapshot, &row, &line.model, now, &mut run)).await?;
    }
    Ok(run)
}

async fn one(
    state: &AppState,
    snapshot: &kpmr::Catalogue,
    row: &LineRow,
    to: &str,
    now: OffsetDateTime,
    run: &mut Run,
) -> ApiResult<()> {
    let policy = Policy::of(&row.policy, row.delay_days);
    if to == row.target {
        if row.pending.is_some() || row.held.is_some() {
            store::set_waiting(
                &state.pg,
                row.catalogue_model_id,
                None,
                row.kept.as_deref(),
                None,
                now,
            )
            .await?;
        }
        return Ok(());
    }
    let waiting_since = match row.pending_since {
        Some(since) if row.pending.as_deref() == Some(to) => since,
        _ => now,
    };
    if policy == Policy::Ask && row.kept.as_deref() == Some(to) {
        return Ok(());
    }
    let due = match policy {
        Policy::Pin => return Ok(()),
        Policy::Follow => true,
        Policy::Ask => false,
        Policy::Delay(days) => now >= waiting_since + Duration::days(i64::from(days)),
    };
    let models = crate::services::catalogue::project_models(state, row.project_id).await?;
    if let Some(reason) = held(state, row, to, &models).await {
        if row.held.as_deref() != Some(reason.as_str()) {
            store::set_waiting(
                &state.pg,
                row.catalogue_model_id,
                Some((to, waiting_since)),
                row.kept.as_deref(),
                Some(&reason),
                now,
            )
            .await?;
            run.held += 1;
        }
        return Ok(());
    }
    if !due {
        let fresh = row.pending.as_deref() != Some(to);
        if fresh {
            store::set_waiting(
                &state.pg,
                row.catalogue_model_id,
                Some((to, waiting_since)),
                row.kept.as_deref(),
                None,
                now,
            )
            .await?;
            run.waiting += 1;
            if policy == Policy::Ask {
                notify(
                    state,
                    row,
                    LineChange::Available {
                        from: row.target.clone(),
                        to: to.to_owned(),
                        prices: prices(snapshot, &row.provider, &row.target, to),
                    },
                )
                .await?;
            }
        }
        return Ok(());
    }
    let mut tx = state.pg.begin().await?;
    let from = move_line(&mut tx, row, to, now).await?;
    let (before, after) = audited(row, &from, to, policy.as_str());
    crate::platform::audit::system_project_event(
        &mut tx,
        (
            OrgId::from_uuid(row.org_id),
            ProjectId::from_uuid(row.project_id),
        ),
        Action::CatalogueModelLineMove,
        Some((
            "catalogue_model",
            CatalogueModelId::from_uuid(row.catalogue_model_id).to_string(),
        )),
        Some(before),
        Some(after),
        now,
    )
    .await?;
    tx.commit().await?;
    crate::services::catalogue::forget(state, row.project_id).await;
    run.moved += 1;
    notify(
        state,
        row,
        LineChange::Moved {
            from,
            to: to.to_owned(),
            prices: prices(snapshot, &row.provider, &row.target, to),
            how: policy.as_str(),
        },
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_reads_back_from_its_name_and_delay() {
        for p in [Policy::Follow, Policy::Ask, Policy::Delay(3), Policy::Pin] {
            assert_eq!(Policy::of(p.as_str(), p.delay_days()), p);
        }
        assert_eq!(Policy::of("delay", None), Policy::Follow);
        assert_eq!(Policy::of("later", None), Policy::Follow);
    }
}
