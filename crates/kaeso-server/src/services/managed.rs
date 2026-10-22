use std::collections::HashMap;

use kaeso_store::StoreError;
use kaeso_store::pg::oidc::{self, Manager};
use kaeso_store::pg::users;
use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::error::ApiError;

pub fn domain_of(email: &str) -> String {
    kaeso_core::email::domain(&kaeso_core::email::canonical(email)).to_owned()
}

pub async fn of_address(db: &PgPool, email: &str) -> Result<Option<Manager>, StoreError> {
    let domain = domain_of(email);
    if domain.is_empty() {
        return Ok(None);
    }
    oidc::manager_of_domain(db, &domain).await
}

pub async fn of_user(db: &PgPool, user: Uuid) -> Result<Option<Manager>, StoreError> {
    match users::get(db, user).await?.and_then(|u| u.email) {
        Some(email) => of_address(db, &email).await,
        None => Ok(None),
    }
}

pub struct Domains(HashMap<String, (Uuid, String)>);

impl Domains {
    pub async fn load(db: &PgPool, org: Option<Uuid>) -> Result<Domains, StoreError> {
        Ok(Domains(
            oidc::managed_domains(db, org)
                .await?
                .into_iter()
                .map(|d| (d.domain, (d.org_id, d.org_name)))
                .collect(),
        ))
    }

    pub fn manager(&self, email: Option<&str>) -> Option<(Uuid, &str)> {
        let (org, name) = self.0.get(&domain_of(email?))?;
        Some((*org, name.as_str()))
    }
}

pub const SSO_ONLY: &str =
    "Your organization requires single sign-on; continue with its identity provider.";

pub async fn refuse_other_sign_in(db: &PgPool, email: &str) -> Result<(), ApiError> {
    match of_address(db, email).await? {
        Some(m) if m.require_sso => Err(ApiError::PermissionDenied(SSO_ONLY.into())),
        _ => Ok(()),
    }
}
