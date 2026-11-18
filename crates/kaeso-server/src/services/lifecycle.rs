//! Model retirement in a project: which of its models a provider retires, who still calls
//! them, moving aliases and route steps to the replacement, and the notices on the way.

use std::collections::BTreeMap;

use kaeso_core::catalogue::{Entry, Fields};
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_core::kpmr::{self, Lifecycle};
use kaeso_mail::notices::{Fact, ModelChange};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, AliasRow, EntryRow, NewEntry};
use kaeso_store::pg::model_lifecycle::{self as store, Stage};
use kaeso_store::pg::usage::{self, Filter, GroupBy, Model as UsageModel};
use rust_decimal::Decimal;
use serde_json::json;
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::services::notifications::dispatch::{Send, deliver_with, project_holders, project_path};
use crate::state::AppState;

/// Warnings before a shutdown, in days, latest first.
pub const WARN_DAYS: [(i64, Stage); 3] = [
    (1, Stage::Shutdown1),
    (7, Stage::Shutdown7),
    (30, Stage::Shutdown30),
];

/// The banner on a project's Models page names shutdowns this many days ahead.
pub const SOON_DAYS: i64 = 90;

/// A catalogue model whose provider retires it, or has announced that it will.
#[derive(Clone, Debug)]
pub struct Affected {
    pub row: EntryRow,
    pub entry: Entry,
    pub lifecycle: Lifecycle,
    /// Whether KPMR marks it deprecated (or retired) today.
    pub deprecated: bool,
    pub retired: bool,
    pub replacement: Option<Replacement>,
    pub uses: Vec<AliasUse>,
}

/// The model the provider recommends instead, and where the project stands with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub model_id: String,
    /// The project's catalogue model for it, when the project has it.
    pub entry: Option<Uuid>,
    /// Whether KPMR lists it for the provider; one it doesn't can't be added.
    pub listed: bool,
    /// Input and output prices per million tokens: the retiring model's and the replacement's.
    pub input_price: (Option<Decimal>, Option<Decimal>),
    pub output_price: (Option<Decimal>, Option<Decimal>),
}

/// An alias whose route calls the model, with the paths of those steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasUse {
    pub alias_id: Uuid,
    pub name: String,
    pub paths: Vec<String>,
}

pub fn today(state: &AppState) -> Date {
    state.clock.now().date()
}

fn kpmr_model<'a>(snapshot: &'a kpmr::Catalogue, entry: &'a Entry) -> Option<&'a kpmr::Model> {
    entry.kpmr_model(snapshot)
}

/// Whether KPMR has anything to say about the model's retirement.
pub fn is_affected(model: &kpmr::Model) -> bool {
    model.is_deprecated() || model.lifecycle.is_some()
}

fn uses_of(aliases: &[AliasRow], id: Uuid) -> Vec<AliasUse> {
    aliases
        .iter()
        .filter_map(|a| {
            let paths = a.route.paths_of(CatalogueModelId::from_uuid(id));
            (!paths.is_empty()).then(|| AliasUse {
                alias_id: a.id,
                name: a.name.clone(),
                paths,
            })
        })
        .collect()
}

fn replacement_of(
    snapshot: &kpmr::Catalogue,
    entries: &[EntryRow],
    entry: &Entry,
    model: &kpmr::Model,
    lifecycle: &Lifecycle,
) -> Option<Replacement> {
    let to = lifecycle.replacement.as_deref()?;
    let next = snapshot.model(&entry.provider, to);
    let was = Fields::from_kpmr(model);
    let is = next.map(Fields::from_kpmr).unwrap_or_default();
    Some(Replacement {
        model_id: to.to_owned(),
        entry: entries
            .iter()
            .find(|e| e.provider == entry.provider && e.model_id == to)
            .map(|e| e.id),
        listed: next.is_some(),
        input_price: (was.input_price, is.input_price),
        output_price: (was.output_price, is.output_price),
    })
}

/// Every catalogue model of the project that its provider retires or will retire, soonest
/// shutdown first.
pub async fn affected(state: &AppState, project: Uuid) -> Result<Vec<Affected>, StoreError> {
    let entries = catalogue::list(&state.pg, project).await?;
    let aliases = catalogue::list_aliases(&state.pg, project).await?;
    let loaded = state.kpmr.current();
    let snapshot = &loaded.snapshot;
    let day = today(state);
    let mut out = Vec::new();
    for row in &entries {
        let entry = row.entry()?;
        let Some(model) = kpmr_model(snapshot, &entry) else {
            continue;
        };
        if !is_affected(model) {
            continue;
        }
        let lifecycle = model.lifecycle.clone().unwrap_or_default();
        out.push(Affected {
            row: row.clone(),
            deprecated: model.is_deprecated(),
            retired: model.is_retired(day),
            replacement: replacement_of(snapshot, &entries, &entry, model, &lifecycle),
            uses: uses_of(&aliases, row.id),
            lifecycle,
            entry,
        });
    }
    out.sort_by(|a, b| {
        let at = |x: &Affected| x.lifecycle.shutdown_on.unwrap_or(Date::MAX);
        at(a)
            .cmp(&at(b))
            .then_with(|| a.row.provider.cmp(&b.row.provider))
            .then_with(|| a.row.model_id.cmp(&b.row.model_id))
    });
    Ok(out)
}

/// What moving a catalogue model to its replacement changes.
#[derive(Clone, Debug)]
pub struct MovePlan {
    pub from: Affected,
    pub to: Replacement,
    pub steps: usize,
}

pub fn plan(found: Affected) -> ApiResult<MovePlan> {
    let Some(to) = found.replacement.clone() else {
        return Err(ApiError::Conflict(format!(
            "KPMR names no replacement for `{}/{}`, so it can't be moved.",
            found.row.provider, found.row.model_id
        )));
    };
    if to.entry.is_none() && !to.listed {
        return Err(ApiError::Conflict(format!(
            "The replacement `{}/{}` isn't in KPMR, so it can't be added.",
            found.row.provider, to.model_id
        )));
    }
    let steps = found.uses.iter().map(|u| u.paths.len()).sum();
    Ok(MovePlan {
        from: found,
        to,
        steps,
    })
}

/// Finds one catalogue model's retirement in the project, or says it has none.
pub async fn find(state: &AppState, project: Uuid, id: Uuid) -> ApiResult<Affected> {
    affected(state, project)
        .await?
        .into_iter()
        .find(|a| a.row.id == id)
        .ok_or_else(|| {
            ApiError::NotFound(
                "No model of this project is retired or has a shutdown date with that id."
                    .to_owned(),
            )
        })
}

/// What a move changed: the replacement's catalogue model, whether it was added, and the
/// aliases before and after.
#[derive(Clone, Debug)]
pub struct Moved {
    pub replacement: Uuid,
    pub added: bool,
    pub aliases: Vec<(AliasRow, kaeso_core::catalogue::route::Step)>,
}

/// Moves every alias and route step that names the model to its replacement, adding the
/// replacement to the catalogue first when the project doesn't have it.
pub async fn apply(
    tx: &mut sqlx::PgConnection,
    org: Uuid,
    project: Uuid,
    plan: &MovePlan,
    by: (&str, Option<Uuid>),
) -> ApiResult<Moved> {
    let (replacement, added) = match plan.to.entry {
        Some(id) => (id, false),
        None => {
            let id = CatalogueModelId::new().uuid();
            let overrides = serde_json::to_value(Fields::default()).map_err(ApiError::internal)?;
            let inserted = catalogue::insert(
                &mut *tx,
                &NewEntry {
                    id,
                    org_id: org,
                    project_id: project,
                    provider: &plan.from.row.provider,
                    model_id: &plan.to.model_id,
                    overrides: &overrides,
                    custom: false,
                    created_by_kind: by.0,
                    created_by_id: by.1,
                },
            )
            .await?;
            if !inserted {
                return Err(ApiError::Conflict(format!(
                    "The catalogue got `{}/{}` meanwhile; try again.",
                    plan.from.row.provider, plan.to.model_id
                )));
            }
            (id, true)
        }
    };
    let from = CatalogueModelId::from_uuid(plan.from.row.id);
    let to = CatalogueModelId::from_uuid(replacement);
    let mut aliases = Vec::new();
    for alias in catalogue::list_aliases(&mut *tx, project).await? {
        if !alias.route.uses(from) {
            continue;
        }
        let mut route = alias.route.clone();
        route.replace(from, to);
        if catalogue::update_alias(&mut *tx, project, alias.id, &alias.name, &route)
            .await?
            .is_none()
        {
            return Err(ApiError::internal("an alias moved under its own name"));
        }
        aliases.push((alias, route));
    }
    Ok(Moved {
        replacement,
        added,
        aliases,
    })
}

/// The audit record of a move: the model, its replacement and each alias before and after.
pub fn audited(plan: &MovePlan, moved: &Moved) -> (serde_json::Value, serde_json::Value) {
    let before = json!({
        "provider": plan.from.row.provider,
        "model_id": plan.from.row.model_id,
        "aliases": moved.aliases.iter().map(|(a, _)| json!({ "name": a.name, "route": a.route })).collect::<Vec<_>>(),
    });
    let after = json!({
        "replacement": plan.to.model_id,
        "replacement_id": CatalogueModelId::from_uuid(moved.replacement).to_string(),
        "replacement_added": moved.added,
        "aliases": moved.aliases.iter().map(|(a, r)| json!({ "name": a.name, "route": r })).collect::<Vec<_>>(),
    });
    (before, after)
}

/// Requests to the model in the project over the last `days` days, today included.
pub async fn requests(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    provider: &str,
    model: &str,
    days: i64,
) -> Result<u64, StoreError> {
    let to = today(state);
    let groups = usage::usage(
        &state.pg,
        org,
        Some(project),
        to - Duration::days(days - 1),
        to,
        GroupBy::Provider,
        &Filter {
            model: Some(UsageModel {
                provider: Some(provider.to_owned()),
                id: model.to_owned(),
            }),
            ..Filter::default()
        },
    )
    .await?;
    Ok(groups
        .iter()
        .map(|g| u64::try_from(g.requests).unwrap_or(0))
        .sum())
}

/// The stage a project should hear about now, if any: the warning for the nearest threshold
/// the shutdown has crossed, while the model still gets requests.
pub fn due_warning(lifecycle: &Lifecycle, day: Date) -> Option<(i64, Vec<Stage>)> {
    let left = lifecycle.days_left(day)?;
    if left < 0 {
        return None;
    }
    let crossed: Vec<Stage> = WARN_DAYS
        .iter()
        .filter(|(d, _)| left <= *d)
        .map(|(_, s)| *s)
        .collect();
    (!crossed.is_empty()).then_some((left, crossed))
}

struct Notice<'a> {
    row: &'a EntryRow,
    change: ModelChange,
    kind: kaeso_core::notifications::Kind,
    subject: String,
}

async fn notify(state: &AppState, n: Notice<'_>) -> ApiResult<usize> {
    let place = crate::services::notifications::triggers::project_name(
        state,
        n.row.org_id,
        n.row.project_id,
    )
    .await?;
    let provider = crate::services::notifications::triggers::provider_name(
        state,
        n.row.org_id,
        &n.row.provider,
    )
    .await?;
    let send = Send {
        kind: n.kind,
        org: Some(n.row.org_id),
        project: Some(n.row.project_id),
        place: Some(&place),
        subject: n.subject,
        open: format!(
            "{}/models?tab=retiring",
            project_path(n.row.org_id, n.row.project_id)
        ),
        actor: None,
    };
    let to = project_holders(
        state,
        n.row.org_id,
        n.row.project_id,
        &[kaeso_core::rights::ProjectRight::ModelsView],
    )
    .await?;
    let ids = json!({
        "catalogue_model_id": CatalogueModelId::from_uuid(n.row.id).to_string(),
        "provider_id": n.row.provider,
    });
    deliver_with(
        state,
        &send,
        &Fact::Model {
            provider: &provider,
            project: &place,
            model: &n.row.model_id,
            change: n.change,
        },
        to,
        Some(ids),
    )
    .await
}

/// What one run of the lifecycle job did.
#[derive(Debug, Default)]
pub struct Run {
    pub announced: usize,
    pub warned: usize,
    pub moved: usize,
    pub added: usize,
}

/// Sends the announcements and warnings that are due, and on a shutdown day moves the aliases
/// of projects that asked for it and adds the replacement for projects that answer with it.
pub async fn run(state: &AppState) -> ApiResult<Run> {
    let loaded = state.kpmr.current();
    let snapshot = &loaded.snapshot;
    let day = today(state);
    let now = state.clock.now();
    let mut run = Run::default();
    let mut settings: BTreeMap<Uuid, store::Settings> = BTreeMap::new();
    for (provider_id, provider) in &snapshot.providers {
        for (model_id, model) in &provider.models {
            let Some(lifecycle) = model.lifecycle.as_ref() else {
                continue;
            };
            let rows = catalogue::of_model(&state.pg, provider_id, model_id).await?;
            for row in &rows {
                Box::pin(one(
                    state,
                    row,
                    lifecycle,
                    day,
                    now,
                    &mut settings,
                    &mut run,
                ))
                .await?;
            }
        }
    }
    Ok(run)
}

async fn one(
    state: &AppState,
    row: &EntryRow,
    lifecycle: &Lifecycle,
    day: Date,
    now: OffsetDateTime,
    settings: &mut BTreeMap<Uuid, store::Settings>,
    run: &mut Run,
) -> ApiResult<()> {
    let (project, provider, model) = (row.project_id, row.provider.as_str(), row.model_id.as_str());
    let shutdown = lifecycle.shutdown_on;
    let announced = lifecycle.announced().is_none_or(|d| d <= day);
    if announced
        && !lifecycle.is_shut_down(day)
        && store::claim(
            &state.pg,
            project,
            provider,
            model,
            Stage::Deprecated,
            shutdown,
            now,
        )
        .await?
    {
        run.announced += 1;
        notify(
            state,
            Notice {
                row,
                change: ModelChange::Deprecated {
                    shutdown_on: shutdown,
                    replacement: lifecycle.replacement.clone(),
                },
                kind: kaeso_core::notifications::Kind::ModelDeprecated,
                subject: format!(
                    "model.deprecated:{}:{}",
                    row.id,
                    shutdown.map(kpmr::day::format).unwrap_or_default()
                ),
            },
        )
        .await?;
    }
    if let (Some((left, stages)), Some(shutdown_on)) = (due_warning(lifecycle, day), shutdown) {
        let calls = requests(state, row.org_id, project, provider, model, 7).await?;
        if calls > 0 {
            let mut fresh = false;
            for stage in &stages {
                fresh |= store::claim(&state.pg, project, provider, model, *stage, shutdown, now)
                    .await?;
            }
            if fresh {
                {
                    run.warned += 1;
                    notify(
                        state,
                        Notice {
                            row,
                            change: ModelChange::ShutdownSoon {
                                shutdown_on,
                                days: left,
                                replacement: lifecycle.replacement.clone(),
                                requests: calls,
                            },
                            kind: kaeso_core::notifications::Kind::ModelShutdownSoon,
                            subject: format!("model.shutdown_soon:{}:{left}", row.id),
                        },
                    )
                    .await?;
                }
            }
        }
    }
    if !lifecycle.is_shut_down(day) || lifecycle.replacement.is_none() {
        return Ok(());
    }
    let s = match settings.get(&project) {
        Some(s) => *s,
        None => {
            let s = store::settings(&state.pg, project).await?;
            settings.insert(project, s);
            s
        }
    };
    if !s.move_on_shutdown && !s.serve_replacement {
        return Ok(());
    }
    if !store::claim(
        &state.pg,
        project,
        provider,
        model,
        Stage::Moved,
        shutdown,
        now,
    )
    .await?
    {
        return Ok(());
    }
    let Ok(plan) = find(state, project, row.id).await.and_then(plan) else {
        return Ok(());
    };
    let mut tx = state.pg.begin().await?;
    let moved = if s.move_on_shutdown {
        apply(&mut tx, row.org_id, project, &plan, ("system", None)).await?
    } else if plan.to.entry.is_none() {
        let mut only_add = plan.clone();
        only_add.from.uses.clear();
        let added = apply(&mut tx, row.org_id, project, &only_add, ("system", None)).await?;
        Moved {
            aliases: Vec::new(),
            ..added
        }
    } else {
        tx.commit().await?;
        return Ok(());
    };
    let (before, after) = audited(&plan, &moved);
    if s.move_on_shutdown || moved.added {
        crate::platform::audit::system_project_event(
            &mut tx,
            (OrgId::from_uuid(row.org_id), ProjectId::from_uuid(project)),
            Action::CatalogueModelMove,
            Some((
                "catalogue_model",
                CatalogueModelId::from_uuid(row.id).to_string(),
            )),
            Some(before),
            Some(after),
            now,
        )
        .await?;
    }
    tx.commit().await?;
    crate::services::catalogue::forget(state, project).await;
    run.added += usize::from(moved.added);
    if s.move_on_shutdown {
        run.moved += 1;
        notify(
            state,
            Notice {
                row,
                change: ModelChange::Moved {
                    replacement: plan.to.model_id.clone(),
                    aliases: moved.aliases.len(),
                },
                kind: kaeso_core::notifications::Kind::ModelShutdownSoon,
                subject: format!("model.moved:{}", row.id),
            },
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifecycle(shutdown: &str) -> Lifecycle {
        Lifecycle {
            shutdown_on: kpmr::day::parse(shutdown),
            ..Lifecycle::default()
        }
    }

    #[test]
    fn a_warning_is_due_at_30_7_and_1_days_and_names_every_threshold_crossed() {
        let d = |s: &str| kpmr::day::parse(s).unwrap();
        let l = lifecycle("2027-02-01");
        assert_eq!(due_warning(&l, d("2026-12-01")), None);
        assert_eq!(
            due_warning(&l, d("2027-01-02")),
            Some((30, vec![Stage::Shutdown30]))
        );
        assert_eq!(
            due_warning(&l, d("2027-01-30")),
            Some((2, vec![Stage::Shutdown7, Stage::Shutdown30]))
        );
        assert_eq!(
            due_warning(&l, d("2027-02-01")),
            Some((
                0,
                vec![Stage::Shutdown1, Stage::Shutdown7, Stage::Shutdown30]
            ))
        );
        assert_eq!(due_warning(&l, d("2027-02-02")), None);
        assert_eq!(due_warning(&Lifecycle::default(), d("2027-02-02")), None);
    }
}
