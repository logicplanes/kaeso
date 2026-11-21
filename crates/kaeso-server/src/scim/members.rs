//! What provisioning does to a membership: added with the single sign-on default role, or
//! removed as an admin removes a member, with their keys archived. Removal also ends the sessions
//! the organization's single sign-on started, with the agents and portal grants approved in them.

use axum::http::StatusCode;
use kaeso_core::id::{OrgId, UserId};
use kaeso_store::pg::{key_lifecycle, members, orgs, scim as store, users};
use serde_json::json;
use uuid::Uuid;

use super::{ScimError, ScimResult};
use crate::auth::sessions;
use crate::platform::audit::{Action, system_event};
use crate::platform::grants::RoleRef;
use crate::state::AppState;

/// Makes `user` a member with the role single sign-on gives new members, and records that SCIM
/// manages them. A member already is one, with their role kept.
pub(super) async fn provision(
    state: &AppState,
    org: Uuid,
    user: Uuid,
    external_id: Option<&str>,
    created_account: bool,
) -> ScimResult<()> {
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let account = users::get(&mut *tx, user)
        .await?
        .ok_or_else(|| ScimError::not_found("No such user."))?;
    if account.state != "active" {
        return Err(ScimError::conflict(
            None,
            "This account is deactivated on the install; an instance admin can reactivate it.",
        ));
    }
    if members::get_org(&mut *tx, org, user).await?.is_none() {
        let row = orgs::get(&mut *tx, org)
            .await?
            .ok_or_else(|| ScimError::not_found("No such organization."))?;
        if row.state != "active" {
            return Err(ScimError::new(
                StatusCode::FORBIDDEN,
                None,
                "The organization is scheduled for deletion and takes no new members.",
            ));
        }
        let preset = row.sso_default_preset.clone().ok_or_else(|| {
            ScimError::conflict(
                None,
                "The organization gives people who arrive through single sign-on no role; choose one under Security first.",
            )
        })?;
        let grant = members::Grant {
            preset,
            role_id: row.sso_default_role_id,
            ..Default::default()
        };
        members::insert_org(&mut *tx, org, user, &grant).await?;
        system_event(
            &mut tx,
            Some(OrgId::from_uuid(org)),
            Action::MemberProvision,
            Some(("user", UserId::from_uuid(user).to_string())),
            None,
            Some(json!({
                "via": "scim",
                "external_id": external_id,
                "preset": grant.preset,
                "role_id": grant.role_id.map(|r| kaeso_core::id::RoleId::from_uuid(r).to_string()),
                "created_account": created_account,
            })),
            now,
        )
        .await?;
    }
    store::put_user(&mut *tx, org, user, external_id, true, now).await?;
    tx.commit().await?;
    Ok(())
}

/// Removes `user` from the organization and its projects and archives their keys in it, as
/// removing a member does, and ends the sessions its single sign-on started, with the agents and
/// portal grants approved in them. The last owner stays.
pub(super) async fn deprovision(
    state: &AppState,
    org: Uuid,
    user: Uuid,
    external_id: Option<&str>,
) -> ScimResult<()> {
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    members::lock_owners(&mut tx, org).await?;
    let Some(target) = members::lock_org(&mut *tx, org, user).await? else {
        store::put_user(&mut *tx, org, user, external_id, false, now).await?;
        tx.commit().await?;
        return Ok(());
    };
    if target.preset == "owner" && members::count_owners(&mut *tx, org).await? <= 1 {
        return Err(ScimError::conflict(
            None,
            "This is the organization's last owner; they must transfer ownership before they can be removed.",
        ));
    }
    members::delete_org(&mut tx, org, user).await?;
    let limited = kaeso_store::pg::limits::delete_of_user(&mut *tx, org, None, user).await?;
    let archived = key_lifecycle::archive_user_keys_in_org(&mut *tx, org, user, now).await?;
    let ended = sessions::revoke_sso_in(&mut tx, org, Some(user), now).await?;
    let remaining = members::org_count_of_user(&mut *tx, user).await?;
    let mut after = serde_json::Map::new();
    after.insert("via".into(), "scim".into());
    after.insert("external_id".into(), external_id.into());
    after.insert("archived_keys".into(), archived.len().into());
    after.extend(ended.counts());
    system_event(
        &mut tx,
        Some(OrgId::from_uuid(org)),
        Action::MemberDeprovision,
        Some(("user", UserId::from_uuid(user).to_string())),
        Some(json!({ "role": RoleRef::of_org(&target.grant()) })),
        Some(serde_json::Value::Object(after)),
        now,
    )
    .await?;
    store::put_user(&mut *tx, org, user, external_id, false, now).await?;
    tx.commit().await?;
    crate::services::keys::forget(state, &archived).await;
    for project in limited {
        crate::services::limits::forget(state, project).await;
    }
    ended.forget(state).await;
    if remaining == 0 {
        sessions::revoke_all(state, UserId::from_uuid(user), None).await?;
    }
    Ok(())
}
