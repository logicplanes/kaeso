use axum::Json;
use axum::extract::State;
use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_core::provider::AuthMethod;
use kaeso_store::pg::linked_billing as store;
use kaeso_store::pg::{credentials, projects};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Who pays for a project's requests when its linked key can't.")]
pub struct LinkedFallback {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "The provider whose linked key the project uses."]
    #[schema(value_type = kaeso_core::provider::ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The key that pays instead when the linked organization's spend limit is\n\
        reached or the link is gone."]
    pub credential: CredentialId,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(
    description = "A project's fallback for its linked key, or `null` to pay only through the link."
)]
pub struct LinkedFallbackSet {
    #[doc = "The project."]
    pub project: ProjectId,
    #[doc = "The provider, one whose `linked_billing` isn't `null`."]
    #[schema(value_type = kaeso_core::provider::ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "Another key of the provider that isn't a link; `null` for none."]
    pub credential: Option<CredentialId>,
}

fn view(f: store::Fallback) -> LinkedFallback {
    LinkedFallback {
        project: ProjectId::from_uuid(f.project_id),
        provider: f.provider,
        credential: CredentialId::from_uuid(f.credential_id),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/linked-billing/fallbacks",
    summary = "The projects that fall back to another key when their linked key can't pay.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = Vec<LinkedFallback>))
)]
pub async fn fallbacks(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<Vec<LinkedFallback>>> {
    let org = ctx.org()?.org.id;
    Ok(Json(
        store::fallbacks(&state.pg, org)
            .await?
            .into_iter()
            .map(view)
            .collect(),
    ))
}

const NOT_A_FALLBACK: &str = "`credential` must be another key of the provider that isn't a link.";

#[utoipa::path(
    put,
    path = "/orgs/{org}/linked-billing/fallbacks",
    summary = "Sets who pays for a project's requests when its linked key can't.",
    description = "By default only the linked organization pays: a request its spend limit\n\
    refuses, or that finds the link gone, fails. With a fallback, the other key pays\n\
    for such a request instead, and the request log shows which key paid.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = LinkedFallbackSet,
    responses((status = 200, body = Vec<LinkedFallback>))
)]
pub async fn set_fallback(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<LinkedFallbackSet>,
) -> ApiResult<Json<Vec<LinkedFallback>>> {
    let org = ctx.org()?.org.id;
    let provider = super::credentials::validate_provider(&req.provider)?;
    if provider.linked_billing().is_none() {
        return Err(ApiError::invalid(
            "provider",
            format!(
                "{} offers no linked billing; add an API key for it.",
                provider.name
            ),
        ));
    }
    let project = req.project.uuid();
    projects::get(&state.pg, org, project)
        .await?
        .filter(|p| p.archived_at.is_none())
        .ok_or_else(|| ApiError::not_found("project"))?;
    if let Some(credential) = req.credential {
        let row = credentials::get(&state.pg, org, credential.uuid())
            .await?
            .filter(|c| c.provider == provider.id)
            .ok_or_else(|| ApiError::invalid("credential", NOT_A_FALLBACK))?;
        if row.auth_method == AuthMethod::LinkedBilling.as_str() {
            return Err(ApiError::invalid("credential", NOT_A_FALLBACK));
        }
    }
    let mut tx = state.pg.begin().await?;
    let before = store::fallback(&mut *tx, org, project, provider.id).await?;
    store::set_fallback(
        &mut *tx,
        org,
        project,
        provider.id,
        req.credential.map(|c| c.uuid()),
    )
    .await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CredentialFallback)
        .target("project", req.project)
        .before(serde_json::json!({ "provider": provider.id, "credential": before.map(CredentialId::from_uuid) }))
        .after(serde_json::json!({ "provider": provider.id, "credential": req.credential }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        store::fallbacks(&state.pg, org)
            .await?
            .into_iter()
            .map(view)
            .collect(),
    ))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What a linked key's organization paid through Kaeso this month.")]
pub struct LinkedCost {
    #[doc = "The linked key."]
    pub credential: CredentialId,
    #[doc = "The cost in USD, at the prices Kaeso knows, from the first of the month (UTC)."]
    #[schema(example = "12.5")]
    pub cost: String,
    #[doc = "The requests it paid for."]
    pub requests: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "This month's cost the org's linked keys paid, by key.")]
pub struct LinkedCosts {
    #[doc = "The first day counted (UTC, `YYYY-MM-DD`)."]
    #[schema(example = "2026-11-01")]
    pub from: String,
    #[doc = "The last day counted, today (UTC)."]
    #[schema(example = "2026-11-17")]
    pub to: String,
    #[doc = "Each linked key with requests this month."]
    pub keys: Vec<LinkedCost>,
    #[doc = "Their total."]
    #[schema(example = "12.5")]
    pub total: String,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/linked-billing/costs",
    summary = "This month's cost the org's linked keys paid, to match with the provider's\n\
    own figures for the install's app.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = LinkedCosts))
)]
pub async fn costs(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<LinkedCosts>> {
    let org = ctx.org()?.org.id;
    let today = state.clock.now().to_offset(time::UtcOffset::UTC).date();
    let from = today.replace_day(1).unwrap_or(today);
    let rows = store::linked_costs(&state.pg, org, from, today).await?;
    let total: rust_decimal::Decimal = rows.iter().map(|r| r.cost).sum();
    Ok(Json(LinkedCosts {
        from: from.to_string(),
        to: today.to_string(),
        keys: rows
            .into_iter()
            .map(|r| LinkedCost {
                credential: CredentialId::from_uuid(r.credential_id),
                cost: r.cost.normalize().to_string(),
                requests: r.requests,
            })
            .collect(),
        total: total.normalize().to_string(),
    }))
}
