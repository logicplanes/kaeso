use kaeso_core::id::OrgId;
use kaeso_store::StoreError;
use kaeso_store::pg::instance::{self, AdminSso, InstanceSettings};
use kaeso_store::pg::{oidc, orgs};
use serde_json::{Map, Value};

use crate::platform::audit::Action;
use crate::platform::error::ApiError;
use crate::state::AppState;

pub struct Change {
    pub settings: InstanceSettings,
    pub before: Option<AdminSso>,
    pub after: Option<AdminSso>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChangeError {
    #[error("the organization doesn't exist")]
    UnknownOrg,
    #[error("the organization has no SSO connection")]
    NotConnected,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<ChangeError> for ApiError {
    fn from(e: ChangeError) -> ApiError {
        match e {
            ChangeError::UnknownOrg => ApiError::not_found("organization"),
            ChangeError::NotConnected => ApiError::invalid(
                "org",
                "This organization has no SSO connection; connect one first.",
            ),
            ChangeError::Store(e) => e.into(),
        }
    }
}

pub async fn change(
    db: &mut sqlx::PgConnection,
    org: Option<OrgId>,
) -> Result<Change, ChangeError> {
    let settings = instance::get_for_update(&mut *db).await?;
    let before = instance::admin_sso(&mut *db).await?;
    if let Some(org) = org {
        if orgs::get(&mut *db, org.uuid()).await?.is_none() {
            return Err(ChangeError::UnknownOrg);
        }
        if oidc::get(&mut *db, org.uuid()).await?.is_none() {
            return Err(ChangeError::NotConnected);
        }
    }
    instance::set_admin_sso_org(&mut *db, org.map(|o| o.uuid())).await?;
    let after = instance::admin_sso(&mut *db).await?;
    Ok(Change {
        settings,
        before,
        after,
    })
}

pub fn audited(a: Option<&AdminSso>) -> Map<String, Value> {
    Map::from_iter([
        (
            "org".to_owned(),
            a.map(|a| OrgId::from_uuid(a.org_id).to_string()).into(),
        ),
        ("name".to_owned(), a.map(|a| a.org_name.clone()).into()),
    ])
}

#[derive(Debug, PartialEq, Eq)]
pub enum OperatorAdminSso {
    NotAnOrg,
    UnknownOrg,
    NotConnected,
    Chosen { org: OrgId, name: String },
    Cleared,
}

pub async fn operator_set(state: &AppState, org: &str) -> Result<OperatorAdminSso, ApiError> {
    let org = match org.trim() {
        "none" => None,
        id => match id.parse::<OrgId>() {
            Ok(org) => Some(org),
            Err(_) => return Ok(OperatorAdminSso::NotAnOrg),
        },
    };
    let mut tx = state.pg.begin().await?;
    let changed = match change(&mut tx, org).await {
        Ok(changed) => changed,
        Err(ChangeError::UnknownOrg) => return Ok(OperatorAdminSso::UnknownOrg),
        Err(ChangeError::NotConnected) => return Ok(OperatorAdminSso::NotConnected),
        Err(ChangeError::Store(e)) => return Err(e.into()),
    };
    let mut after = audited(changed.after.as_ref());
    after.insert("by".to_owned(), "operator".into());
    crate::platform::audit::system_change(
        &mut tx,
        None,
        Action::InstanceAdminSsoUpdate,
        None,
        Some(Value::Object(audited(changed.before.as_ref()))),
        Some(Value::Object(after)),
        state.clock.now(),
    )
    .await?;
    tx.commit().await?;
    Ok(match changed.after {
        Some(chosen) => OperatorAdminSso::Chosen {
            org: OrgId::from_uuid(chosen.org_id),
            name: chosen.org_name,
        },
        None => OperatorAdminSso::Cleared,
    })
}
