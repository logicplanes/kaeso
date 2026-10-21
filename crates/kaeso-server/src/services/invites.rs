use kaeso_core::id::{OrgId, ProjectId, RoleId, UserId};
use kaeso_core::rights::OrgPreset;
use kaeso_store::StoreError;
use kaeso_store::pg::invites::{self, Invite};
use kaeso_store::pg::members::Grant;
use kaeso_store::pg::{members, projects, roles};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::orgs::{add_member, org_grant};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRole {
    pub project_id: ProjectId,
    pub preset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<RoleId>,
}

impl ProjectRole {
    pub fn grant(&self) -> Grant {
        Grant {
            preset: self.preset.clone(),
            role_id: self.role_id.map(|r| r.uuid()),
            ..Grant::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accepted {
    Joined { org: OrgId, revoked_others: u64 },
    Instance,
    Gone,
    AlreadyMember,
}

pub async fn accept(
    tx: &mut sqlx::PgConnection,
    invite: &Invite,
    user: UserId,
    now: OffsetDateTime,
) -> Result<Accepted, StoreError> {
    if !invites::accept(&mut *tx, invite.id, user.uuid(), now).await? {
        return Ok(Accepted::Gone);
    }
    let Some(org) = invite.org_id.map(OrgId::from_uuid) else {
        return Ok(Accepted::Instance);
    };
    let grant = match (invite.org_preset.as_deref(), invite.org_role_id) {
        (Some("custom"), Some(id)) if roles::get(&mut *tx, org.uuid(), id).await?.is_some() => {
            Grant {
                preset: "custom".into(),
                role_id: Some(id),
                ..Grant::default()
            }
        }
        (Some(p), _) => org_grant(p.parse::<OrgPreset>().unwrap_or(OrgPreset::Member)),
        (None, _) => org_grant(OrgPreset::Member),
    };
    // Never touch an existing member's roles: a stale invitation would restore what was taken away.
    if !add_member(&mut *tx, org, user, &grant).await? {
        return Ok(Accepted::AlreadyMember);
    }
    let revoked_others =
        invites::revoke_others_for(&mut *tx, org.uuid(), &invite.email, invite.id, now).await?;
    let project_roles: Vec<ProjectRole> =
        serde_json::from_value(invite.project_roles.clone()).unwrap_or_default();
    for role in project_roles {
        if let Some(id) = role.role_id
            && roles::get(&mut *tx, org.uuid(), id.uuid()).await?.is_none()
        {
            continue;
        }
        let Some(project) = projects::get(&mut *tx, org.uuid(), role.project_id.uuid()).await?
        else {
            continue;
        };
        if project.archived_at.is_some() {
            continue;
        }
        let grant = role.grant();
        if !members::insert_project(&mut *tx, project.id, user.uuid(), &grant).await? {
            members::update_project(&mut *tx, project.id, user.uuid(), &grant).await?;
        }
    }
    Ok(Accepted::Joined {
        org,
        revoked_others,
    })
}
