use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grant {
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
}

macro_rules! impl_grant {
    ($t:ty) => {
        impl $t {
            pub fn grant(&self) -> Grant {
                Grant {
                    preset: self.preset.clone(),
                    role_id: self.role_id,
                    rights_added: self.rights_added.clone(),
                    rights_removed: self.rights_removed.clone(),
                    role_name: self.role_name.clone(),
                    role_rights: self.role_rights.clone(),
                    role_project_rights: self.role_project_rights.clone(),
                }
            }
        }
    };
}
pub(crate) use impl_grant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgMember {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub name: Option<String>,
    pub user_state: String,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
    pub created_at: OffsetDateTime,
    pub avatar: Option<String>,
}

impl_grant!(OrgMember);

pub async fn insert_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    g: &Grant,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "INSERT INTO org_memberships (id, org_id, user_id, preset, role_id, rights_added, rights_removed) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (org_id, user_id) DO NOTHING",
        Uuid::now_v7(),
        org_id,
        user_id,
        g.preset,
        g.role_id,
        &g.rights_added,
        &g.rights_removed
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn get_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Option<OrgMember>, StoreError> {
    Ok(sqlx::query_as!(
        OrgMember,
        "SELECT m.id, m.org_id, m.user_id, u.email, u.name, u.state AS user_state, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", \
         m.created_at, u.avatar FROM org_memberships m JOIN users u ON u.id = m.user_id \
         LEFT JOIN org_roles r ON r.id = m.role_id WHERE m.org_id = $1 AND m.user_id = $2",
        org_id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    after: Option<Uuid>,
    limit: i64,
    preset: Option<&str>,
    role: Option<Uuid>,
) -> Result<Vec<OrgMember>, StoreError> {
    Ok(sqlx::query_as!(
        OrgMember,
        "SELECT m.id, m.org_id, m.user_id, u.email, u.name, u.state AS user_state, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", \
         m.created_at, u.avatar FROM org_memberships m JOIN users u ON u.id = m.user_id \
         LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.org_id = $1 AND ($2::uuid IS NULL OR m.id > $2) AND ($4::text IS NULL OR m.preset = $4) \
         AND ($5::uuid IS NULL OR m.role_id = $5) ORDER BY m.id LIMIT $3",
        org_id,
        after,
        limit,
        preset,
        role
    )
    .fetch_all(db)
    .await?)
}

pub async fn all_org(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<OrgMember>, StoreError> {
    Ok(sqlx::query_as!(
        OrgMember,
        "SELECT m.id, m.org_id, m.user_id, u.email, u.name, u.state AS user_state, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", \
         m.created_at, u.avatar FROM org_memberships m JOIN users u ON u.id = m.user_id \
         LEFT JOIN org_roles r ON r.id = m.role_id WHERE m.org_id = $1 ORDER BY m.id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn update_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    g: &Grant,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE org_memberships SET preset = $3, role_id = $4, rights_added = $5, rights_removed = $6, \
         updated_at = now() WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id,
        g.preset,
        g.role_id,
        &g.rights_added,
        &g.rights_removed
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_org(
    db: &mut sqlx::PgConnection,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_memberships WHERE user_id = $2 AND project_id IN (SELECT id FROM projects WHERE org_id = $1)",
        org_id,
        user_id
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "DELETE FROM org_memberships WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn count_owners(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM org_memberships m JOIN users u ON u.id = m.user_id \
         WHERE m.org_id = $1 AND m.preset = 'owner' AND u.state <> 'deleted'",
        org_id
    )
    .fetch_one(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct UserOrg {
    pub org_id: Uuid,
    pub org_name: String,
    pub org_state: String,
    pub org_deletion_due_at: Option<OffsetDateTime>,
    pub require_2fa: bool,
    pub require_sso: bool,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
    pub org_avatar: Option<String>,
}

impl_grant!(UserOrg);

pub async fn orgs_of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<UserOrg>, StoreError> {
    Ok(sqlx::query_as!(
        UserOrg,
        "SELECT o.id AS org_id, o.name AS org_name, o.state AS org_state, o.deletion_due_at AS org_deletion_due_at, \
         o.require_2fa, o.require_sso, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", o.avatar AS org_avatar \
         FROM org_memberships m JOIN organizations o ON o.id = m.org_id LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.user_id = $1 ORDER BY o.id",
        user_id
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectMember {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
    pub created_at: OffsetDateTime,
    pub avatar: Option<String>,
}

impl_grant!(ProjectMember);

pub async fn insert_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
    g: &Grant,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "INSERT INTO project_memberships (id, project_id, user_id, preset, role_id, rights_added, rights_removed) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (project_id, user_id) DO NOTHING",
        Uuid::now_v7(),
        project_id,
        user_id,
        g.preset,
        g.role_id,
        &g.rights_added,
        &g.rights_removed
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn org_presets(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT user_id, preset FROM org_memberships WHERE org_id = $1 ORDER BY id",
        org_id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.user_id, r.preset))
    .collect())
}

pub async fn join_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    users: &[(Uuid, String)],
) -> Result<u64, StoreError> {
    let ids: Vec<Uuid> = users.iter().map(|_| Uuid::now_v7()).collect();
    let user_ids: Vec<Uuid> = users.iter().map(|(u, _)| *u).collect();
    let presets: Vec<String> = users.iter().map(|(_, p)| p.clone()).collect();
    Ok(sqlx::query!(
        "INSERT INTO project_memberships (id, project_id, user_id, preset) \
         SELECT id, $2, user_id, preset FROM UNNEST($1::uuid[], $3::uuid[], $4::text[]) AS j (id, user_id, preset) \
         ON CONFLICT (project_id, user_id) DO NOTHING",
        &ids,
        project_id,
        &user_ids,
        &presets
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn get_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ProjectMember>, StoreError> {
    Ok(sqlx::query_as!(
        ProjectMember,
        "SELECT m.id, m.project_id, m.user_id, u.email, u.name, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", m.created_at, u.avatar \
         FROM project_memberships m JOIN users u ON u.id = m.user_id LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.project_id = $1 AND m.user_id = $2",
        project_id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ProjectMember>, StoreError> {
    Ok(sqlx::query_as!(
        ProjectMember,
        "SELECT m.id, m.project_id, m.user_id, u.email, u.name, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", m.created_at, u.avatar \
         FROM project_memberships m JOIN users u ON u.id = m.user_id LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.project_id = $1 AND ($2::uuid IS NULL OR m.id > $2) ORDER BY m.id LIMIT $3",
        project_id,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn all_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<ProjectMember>, StoreError> {
    Ok(sqlx::query_as!(
        ProjectMember,
        "SELECT m.id, m.project_id, m.user_id, u.email, u.name, m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", m.created_at, u.avatar \
         FROM project_memberships m JOIN users u ON u.id = m.user_id LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.project_id = $1 ORDER BY m.id",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn update_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
    g: &Grant,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE project_memberships SET preset = $3, role_id = $4, rights_added = $5, rights_removed = $6, \
         updated_at = now() WHERE project_id = $1 AND user_id = $2",
        project_id,
        user_id,
        g.preset,
        g.role_id,
        &g.rights_added,
        &g.rights_removed
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM project_memberships WHERE project_id = $1 AND user_id = $2",
        project_id,
        user_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

#[derive(Clone, Debug)]
pub struct AccessContext {
    pub org: super::orgs::Org,
    pub org_grant: Option<Grant>,
    pub project: Option<super::projects::Project>,
    pub project_grant: Option<Grant>,
}

pub async fn access_context(
    db: &mut sqlx::PgConnection,
    org_id: Uuid,
    user_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<Option<AccessContext>, StoreError> {
    let Some(org) = super::orgs::get(&mut *db, org_id).await? else {
        return Ok(None);
    };
    let org_grant = match user_id {
        Some(user) => sqlx::query_as!(
            Grant,
            "SELECT m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\" FROM org_memberships m LEFT JOIN org_roles r ON r.id = m.role_id \
             WHERE m.org_id = $1 AND m.user_id = $2",
            org_id,
            user
        )
        .fetch_optional(&mut *db)
        .await?,
        None => None,
    };
    let project = match project_id {
        Some(p) => super::projects::get(&mut *db, org_id, p).await?,
        None => None,
    };
    let project_grant = match (&project, user_id) {
        (Some(p), Some(user)) => sqlx::query_as!(
            Grant,
            "SELECT m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\" FROM project_memberships m LEFT JOIN org_roles r ON r.id = m.role_id \
             WHERE m.project_id = $1 AND m.user_id = $2",
            p.id,
            user
        )
        .fetch_optional(&mut *db)
        .await?,
        _ => None,
    };
    Ok(Some(AccessContext {
        org,
        org_grant,
        project,
        project_grant,
    }))
}

pub async fn org_count_of_user(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM org_memberships WHERE user_id = $1",
        user_id
    )
    .fetch_one(db)
    .await?)
}

pub async fn orgs_where_last_owner(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT o.id, o.name FROM org_memberships m JOIN organizations o ON o.id = m.org_id \
         WHERE m.user_id = $1 AND m.preset = 'owner' AND NOT EXISTS ( \
           SELECT 1 FROM org_memberships m2 JOIN users u ON u.id = m2.user_id \
           WHERE m2.org_id = m.org_id AND m2.preset = 'owner' AND m2.user_id <> $1 AND u.state <> 'deleted')",
        user_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
}

pub async fn delete_all_of_user(
    db: &mut sqlx::PgConnection,
    user_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_memberships WHERE user_id = $1",
        user_id
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM org_memberships WHERE user_id = $1", user_id)
        .execute(&mut *db)
        .await?;
    Ok(())
}

pub async fn org_counts(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<(i64, i64), StoreError> {
    let row = sqlx::query!(
        "SELECT (SELECT count(*) FROM org_memberships WHERE org_id = $1) AS \"members!\", \
         (SELECT count(*) FROM projects WHERE org_id = $1 AND archived_at IS NULL) AS \"projects!\"",
        org_id
    )
    .fetch_one(db)
    .await?;
    Ok((row.members, row.projects))
}

#[derive(Clone, Debug)]
pub struct UserProject {
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub is_default: bool,
    pub project_avatar: Option<String>,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
}

impl_grant!(UserProject);

pub async fn projects_of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<UserProject>, StoreError> {
    Ok(sqlx::query_as!(
        UserProject,
        "SELECT p.org_id, p.id AS project_id, p.name AS project_name, p.is_default, p.avatar AS project_avatar, \
         m.preset, m.role_id, m.rights_added, m.rights_removed, r.name AS \"role_name?\", r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\" FROM project_memberships m JOIN projects p ON p.id = m.project_id \
         LEFT JOIN org_roles r ON r.id = m.role_id \
         WHERE m.user_id = $1 AND p.archived_at IS NULL ORDER BY p.org_id, p.id",
        user_id
    )
    .fetch_all(db)
    .await?)
}
