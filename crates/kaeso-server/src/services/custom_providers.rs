use kaeso_core::providers::OrgProviders;
use kaeso_store::StoreError;
use kaeso_store::pg::custom_providers::{self as store, CustomProviderRow};
use kaeso_upstream::guard;
use sqlx::PgExecutor;
use url::Url;
use uuid::Uuid;

use crate::state::AppState;

static BUILT_IN_ONLY: OrgProviders = OrgProviders::built_in_only();

pub fn enabled(state: &AppState) -> bool {
    state.config.custom_providers.enabled
}

pub fn in_effect<'a>(state: &AppState, providers: &'a OrgProviders) -> &'a OrgProviders {
    if enabled(state) {
        providers
    } else {
        &BUILT_IN_ONLY
    }
}

pub fn of_rows(rows: &[CustomProviderRow]) -> Result<OrgProviders, StoreError> {
    Ok(OrgProviders::new(
        rows.iter()
            .map(CustomProviderRow::custom_provider)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

pub async fn of_org(
    state: &AppState,
    db: impl PgExecutor<'_>,
    org: Uuid,
) -> Result<OrgProviders, StoreError> {
    if !enabled(state) {
        return Ok(OrgProviders::built_in_only());
    }
    of_rows(&store::list(db, org).await?)
}

pub async fn forget(state: &AppState, projects: impl IntoIterator<Item = Uuid>) {
    for project in projects {
        crate::services::catalogue::forget(state, project).await;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reachability {
    Allowed,
    Refused,
    Unknown,
}

pub async fn reachability(state: &AppState, url: &Url) -> Reachability {
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Reachability::Refused;
    };
    let reach = state.upstream.reach_for(url);
    match guard::resolve(host, port, &reach, state.upstream.lookup()).await {
        Ok(_) => Reachability::Allowed,
        Err(guard::ResolveError::Refused(_)) => Reachability::Refused,
        Err(guard::ResolveError::Lookup { .. }) => Reachability::Unknown,
    }
}
