use kaeso_core::accounts::{AccountCreation, AccountPolicy, DomainRule, OrgCreation};
use kaeso_store::pg::instance::InstanceSettings;

use crate::platform::error::ApiError;

pub fn policy(s: &InstanceSettings) -> AccountPolicy {
    AccountPolicy {
        account_creation: AccountCreation::from_stored(&s.account_creation),
        org_creation: OrgCreation::from_stored(&s.org_creation),
        allowed_domains: s
            .allowed_domains
            .iter()
            .filter_map(|d| DomainRule::from_stored(d))
            .collect(),
    }
}

pub fn refuse_sign_up(p: &AccountPolicy, email: &str) -> Option<ApiError> {
    if p.may_sign_up(email) {
        return None;
    }
    Some(ApiError::PermissionDenied(
        match p.account_creation {
            AccountCreation::AllowedDomains => {
                "This email address can't create an account here; ask an administrator to invite you."
            }
            _ => "Accounts on this install are by invitation only; ask an administrator to invite you.",
        }
        .into(),
    ))
}

pub fn refuse_org_creation(
    p: &AccountPolicy,
    is_instance_admin: bool,
    email: Option<&str>,
) -> Option<ApiError> {
    if p.may_create_orgs(is_instance_admin, email) {
        return None;
    }
    Some(ApiError::PermissionDenied(
        match p.org_creation {
            OrgCreation::AllowedDomains => {
                "Your email address can't create organizations here; ask an instance admin."
            }
            _ => "Only instance admins can create organizations on this install.",
        }
        .into(),
    ))
}
