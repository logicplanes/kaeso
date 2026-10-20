use kaeso_core::id::OrgId;
use kaeso_store::pg::oidc;

use crate::platform::error::ApiError;
use crate::state::AppState;

#[derive(Debug, PartialEq, Eq)]
pub enum OperatorVerify {
    NotADomain,
    NotAnOrg,
    NotListed,
    Several(Vec<OrgId>),
    VerifiedElsewhere,
    AlreadyVerified(OrgId),
    Verified(OrgId),
}

pub async fn operator_verify_domain(
    state: &AppState,
    domain: &str,
    org: Option<&str>,
) -> Result<OperatorVerify, ApiError> {
    let org = match org.map(str::parse::<OrgId>) {
        None => None,
        Some(Ok(org)) => Some(org),
        Some(Err(_)) => return Ok(OperatorVerify::NotAnOrg),
    };
    let Some(domain) = kaeso_core::email::normalize_domain(domain.trim()) else {
        return Ok(OperatorVerify::NotADomain);
    };
    let owners: Vec<OrgId> = oidc::domain_owners(&state.pg, &domain)
        .await?
        .into_iter()
        .map(OrgId::from_uuid)
        .collect();
    let org = match (org, owners.as_slice()) {
        (Some(org), _) if owners.contains(&org) => org,
        (Some(_), _) | (None, []) => return Ok(OperatorVerify::NotListed),
        (None, [only]) => *only,
        (None, _) => return Ok(OperatorVerify::Several(owners)),
    };
    let mut tx = state.pg.begin().await?;
    if oidc::verified_elsewhere(&mut *tx, org.uuid(), &domain).await? {
        return Ok(OperatorVerify::VerifiedElsewhere);
    }
    if !oidc::mark_verified(&mut *tx, org.uuid(), &domain, state.clock.now()).await? {
        return Ok(OperatorVerify::AlreadyVerified(org));
    }
    crate::platform::audit::system(
        &mut tx,
        Some(org),
        "security.sso_domain_verify",
        Some(("org", org.to_string())),
        Some(serde_json::json!({ "domain": domain, "by": "operator" })),
        state.clock.now(),
    )
    .await?;
    tx.commit().await?;
    Ok(OperatorVerify::Verified(org))
}
