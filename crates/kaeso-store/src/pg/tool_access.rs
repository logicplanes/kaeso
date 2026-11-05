use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use crate::StoreError;
use crate::pg::catalogue::Subject;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectToolAccess {
    pub users: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub service_accounts: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub keys: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub open: BTreeSet<Uuid>,
}

static NOTHING: BTreeSet<Uuid> = BTreeSet::new();

impl ProjectToolAccess {
    // A key or service account created after this was loaded gets no tool rather than every one,
    // as model access does.
    fn listed<'a>(
        &self,
        id: Uuid,
        restricted: &'a BTreeMap<Uuid, BTreeSet<Uuid>>,
    ) -> Option<&'a BTreeSet<Uuid>> {
        match restricted.get(&id) {
            Some(tools) => Some(tools),
            None if self.open.contains(&id) => None,
            None => Some(&NOTHING),
        }
    }

    pub fn allows(
        &self,
        user: Option<Uuid>,
        service_account: Option<Uuid>,
        key: Option<Uuid>,
        tool: Uuid,
    ) -> bool {
        let user_ok = user
            .and_then(|u| self.users.get(&u))
            .is_none_or(|tools| tools.contains(&tool));
        let account_ok = service_account
            .and_then(|s| self.listed(s, &self.service_accounts))
            .is_none_or(|tools| tools.contains(&tool));
        let key_ok = key
            .and_then(|k| self.listed(k, &self.keys))
            .is_none_or(|tools| tools.contains(&tool));
        user_ok && account_ok && key_ok
    }
}

pub async fn access(
    db: &mut PgConnection,
    project_id: Uuid,
    subject: Subject,
) -> Result<Option<BTreeSet<Uuid>>, StoreError> {
    let restricted = match subject {
        Subject::Membership(id) => sqlx::query_scalar!(
            "SELECT tools_restricted FROM project_memberships WHERE project_id = $1 AND id = $2",
            project_id,
            id
        )
        .fetch_optional(&mut *db)
        .await?,
        Subject::ServiceAccount(id) => {
            sqlx::query_scalar!(
                "SELECT tools_restricted FROM service_accounts WHERE project_id = $1 AND id = $2",
                project_id,
                id
            )
            .fetch_optional(&mut *db)
            .await?
        }
        Subject::Key(id) => {
            sqlx::query_scalar!(
                "SELECT tools_restricted FROM api_keys WHERE project_id = $1 AND id = $2",
                project_id,
                id
            )
            .fetch_optional(&mut *db)
            .await?
        }
    };
    if restricted != Some(true) {
        return Ok(None);
    }
    let (m, s, k) = ids(subject);
    let tools = sqlx::query_scalar!(
        "SELECT tool_id FROM tool_access WHERE project_id = $1 \
         AND membership_id IS NOT DISTINCT FROM $2 AND service_account_id IS NOT DISTINCT FROM $3 \
         AND api_key_id IS NOT DISTINCT FROM $4",
        project_id,
        m,
        s,
        k
    )
    .fetch_all(&mut *db)
    .await?;
    Ok(Some(tools.into_iter().collect()))
}

fn ids(subject: Subject) -> (Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match subject {
        Subject::Membership(id) => (Some(id), None, None),
        Subject::ServiceAccount(id) => (None, Some(id), None),
        Subject::Key(id) => (None, None, Some(id)),
    }
}

pub async fn set_access(
    db: &mut PgConnection,
    project_id: Uuid,
    subject: Subject,
    tools: Option<&BTreeSet<Uuid>>,
) -> Result<bool, StoreError> {
    let restricted = tools.is_some();
    let found = match subject {
        Subject::Membership(id) => sqlx::query!(
            "UPDATE project_memberships SET tools_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
            project_id,
            id,
            restricted
        )
        .execute(&mut *db)
        .await?,
        Subject::ServiceAccount(id) => sqlx::query!(
            "UPDATE service_accounts SET tools_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
            project_id,
            id,
            restricted
        )
        .execute(&mut *db)
        .await?,
        Subject::Key(id) => sqlx::query!(
            "UPDATE api_keys SET tools_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
            project_id,
            id,
            restricted
        )
        .execute(&mut *db)
        .await?,
    }
    .rows_affected()
        == 1;
    if !found {
        return Ok(false);
    }
    let (m, s, k) = ids(subject);
    sqlx::query!(
        "DELETE FROM tool_access WHERE project_id = $1 AND membership_id IS NOT DISTINCT FROM $2 \
         AND service_account_id IS NOT DISTINCT FROM $3 AND api_key_id IS NOT DISTINCT FROM $4",
        project_id,
        m,
        s,
        k
    )
    .execute(&mut *db)
    .await?;
    if let Some(tools) = tools {
        let list: Vec<Uuid> = tools.iter().copied().collect();
        sqlx::query!(
            "INSERT INTO tool_access (project_id, membership_id, service_account_id, api_key_id, tool_id) \
             SELECT $1, $2, $3, $4, t.id FROM mcp_tools t \
             JOIN project_mcp_servers ps ON ps.server_id = t.server_id AND ps.project_id = $1 \
             WHERE t.id = ANY($5::uuid[])",
            project_id,
            m,
            s,
            k,
            &list
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(true)
}

pub async fn project_access(
    db: &mut PgConnection,
    project_id: Uuid,
) -> Result<ProjectToolAccess, StoreError> {
    let mut out = ProjectToolAccess::default();
    let users = sqlx::query!(
        "SELECT pm.user_id, ta.tool_id AS \"tool?\" FROM project_memberships pm \
         LEFT JOIN tool_access ta ON ta.membership_id = pm.id WHERE pm.project_id = $1 AND pm.tools_restricted",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in users {
        out.users.entry(r.user_id).or_default().extend(r.tool);
    }
    let accounts = sqlx::query!(
        "SELECT s.id, s.tools_restricted, ta.tool_id AS \"tool?\" FROM service_accounts s \
         LEFT JOIN tool_access ta ON ta.service_account_id = s.id AND s.tools_restricted \
         WHERE s.project_id = $1",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in accounts {
        if r.tools_restricted {
            out.service_accounts.entry(r.id).or_default().extend(r.tool);
        } else {
            out.open.insert(r.id);
        }
    }
    let keys = sqlx::query!(
        "SELECT k.id, k.tools_restricted, ta.tool_id AS \"tool?\" FROM api_keys k \
         LEFT JOIN tool_access ta ON ta.api_key_id = k.id AND k.tools_restricted \
         WHERE k.project_id = $1 AND k.archived_at IS NULL",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in keys {
        if r.tools_restricted {
            out.keys.entry(r.id).or_default().extend(r.tool);
        } else {
            out.open.insert(r.id);
        }
    }
    Ok(out)
}

pub async fn clear_service_account(
    db: impl PgExecutor<'_>,
    service_account_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM tool_access WHERE service_account_id = $1",
        service_account_id
    )
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn unrestricted_callers_reach_every_tool() {
        let access = ProjectToolAccess {
            open: [id(10), id(20)].into(),
            ..ProjectToolAccess::default()
        };
        assert!(access.allows(Some(id(1)), None, Some(id(10)), id(99)));
        assert!(access.allows(None, Some(id(20)), None, id(99)));
    }

    #[test]
    fn every_list_on_the_way_must_name_the_tool() {
        let access = ProjectToolAccess {
            users: [(id(1), [id(100), id(101)].into())].into(),
            keys: [(id(10), [id(100)].into())].into(),
            ..ProjectToolAccess::default()
        };
        assert!(access.allows(Some(id(1)), None, Some(id(10)), id(100)));
        assert!(!access.allows(Some(id(1)), None, Some(id(10)), id(101)));
        assert!(!access.allows(Some(id(1)), None, None, id(102)));
        assert!(access.allows(Some(id(2)), None, None, id(102)));
    }

    #[test]
    fn a_key_it_has_not_seen_gets_no_tool() {
        let access = ProjectToolAccess::default();
        assert!(!access.allows(None, None, Some(id(10)), id(100)));
        assert!(!access.allows(None, Some(id(20)), None, id(100)));
    }
}
