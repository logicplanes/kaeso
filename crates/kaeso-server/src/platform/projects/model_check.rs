use axum::Json;
use axum::extract::State;
use futures::StreamExt;
use kaeso_core::catalogue::Availability;
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_kpp::ErrorKind;
use kaeso_store::pg::catalogue::{self, EntryRow};
use serde::Serialize;
use utoipa::ToSchema;

use super::models::{CatalogueModelView, view};
use super::{MODELS_ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::params::Path;
use crate::services::availability::{self, Probe};
use crate::services::catalogue as service;
use crate::services::credentials::{self, ResolveError};
use crate::state::AppState;

const AT_ONCE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Why a model could not be checked.")]
pub enum CheckFailureReason {
    #[doc = "The project has no provider key for the model's provider."]
    NoKey,
    #[doc = "The provider refused the project's provider key."]
    KeyRefused,
    #[doc = "The provider did not answer the check, or answered with an error."]
    ProviderError,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A model the check could not decide; it keeps its earlier availability.")]
pub struct CheckFailure {
    #[doc = "The catalogue entry."]
    pub catalogue_model_id: CatalogueModelId,
    #[doc = "The provider."]
    #[schema(example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's model id."]
    #[schema(example = "claude-sonnet-4-5")]
    pub model_id: String,
    #[doc = "Why."]
    pub reason: CheckFailureReason,
    #[doc = "What is wrong, as one sentence."]
    pub message: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What a check of the catalogue found.")]
pub struct CheckResult {
    #[doc = "Every entry of the catalogue, with its availability after the check."]
    pub models: Vec<CatalogueModelView>,
    #[doc = "The entries the check could not decide."]
    pub failed: Vec<CheckFailure>,
}

fn failure(row: &EntryRow, provider: &str, reason: CheckFailureReason) -> CheckFailure {
    let message = match reason {
        CheckFailureReason::NoKey => {
            format!("No provider key for {provider} is set up for this project.")
        }
        CheckFailureReason::KeyRefused => {
            format!("{provider} refused the project's provider key; an org admin can replace it.")
        }
        CheckFailureReason::ProviderError => {
            format!("{provider} did not answer the check; try again shortly.")
        }
    };
    CheckFailure {
        catalogue_model_id: CatalogueModelId::from_uuid(row.id),
        provider: row.provider.clone(),
        model_id: row.model_id.clone(),
        reason,
        message,
    }
}

async fn check_one(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    row: &EntryRow,
) -> ApiResult<Result<Availability, CheckFailure>> {
    let entry = row.entry()?;
    let eff = service::effective(state, &entry)
        .ok_or_else(|| ApiError::internal(format!("unknown provider `{}`", row.provider)))?;
    let name = eff.provider.name;
    let credential = match credentials::resolve(state, org, project, eff.provider.id).await {
        Ok(c) => c,
        Err(ResolveError::Missing { .. }) => {
            return Ok(Err(failure(row, name, CheckFailureReason::NoKey)));
        }
        Err(ResolveError::Crypto(e)) => {
            return Err(ApiError::internal(format!(
                "cannot decrypt a provider credential: {e}"
            )));
        }
        Err(ResolveError::Store(e)) => return Err(e.into()),
    };
    match availability::probe(state, &credential.secret, &eff).await {
        Probe::Served => Ok(Ok(Availability::Available)),
        Probe::Unserved => Ok(Ok(Availability::Unavailable)),
        Probe::Failed(e) => {
            tracing::info!(provider = eff.provider.id, model = %eff.model_id, kind = e.kind.as_str(), error = %e.message, "a model check failed");
            let reason = match e.kind {
                ErrorKind::Authentication | ErrorKind::PermissionDenied | ErrorKind::Billing => {
                    CheckFailureReason::KeyRefused
                }
                _ => CheckFailureReason::ProviderError,
            };
            Ok(Err(failure(row, name, reason)))
        }
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/models/check",
    summary = "Asks each model's provider, with the project's provider key, whether it\n\
    serves the model, and records the answer.",
    description = "A model the provider does not serve to the key, because it is retired\n\
    or the key has no access, becomes `unavailable`: model listings leave it\n\
    out, and requests for it still go to the provider and fail there. Kaeso\n\
    also learns this from requests the provider answers with model not found.\n\
    Changing a provider key of the org resets its provider's models to\n\
    `unchecked`.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = CheckResult))
)]
pub async fn check(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<CheckResult>> {
    let project_id = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let org = ctx.org()?.id();
    let project = ctx.project()?.id();
    let rows = catalogue::list(&state.pg, project_id).await?;
    let outcomes: Vec<_> = futures::stream::iter(rows.clone().into_iter().map(|row| {
        let state = state.clone();
        async move { (row.id, check_one(&state, org, project, &row).await) }
    }))
    .buffered(AT_ONCE)
    .collect()
    .await;
    let mut tx = state.pg.begin().await?;
    let mut available = Vec::new();
    let mut unavailable = Vec::new();
    let mut failed = Vec::new();
    for ((id, outcome), row) in outcomes.into_iter().zip(&rows) {
        match outcome? {
            Ok(a) => {
                catalogue::set_availability(&mut *tx, project_id, id, a, ctx.now).await?;
                let name = format!("{}/{}", row.provider, row.model_id);
                match a {
                    Availability::Available => available.push(name),
                    Availability::Unavailable => unavailable.push(name),
                }
            }
            Err(f) => failed.push(f),
        }
    }
    ctx.audit(Some(org), "catalogue_model.check")
        .target("project", project)
        .after(serde_json::json!({
            "available": available,
            "unavailable": unavailable,
            "failed": failed.iter().map(|f| format!("{}/{}", f.provider, f.model_id)).collect::<Vec<_>>(),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, project_id).await;
    let rows = catalogue::list(&state.pg, project_id).await?;
    let aliases = catalogue::list_aliases(&state.pg, project_id).await?;
    let models = rows
        .iter()
        .map(|r| view(&state, r, &aliases))
        .collect::<ApiResult<Vec<_>>>()?;
    Ok(Json(CheckResult { models, failed }))
}
