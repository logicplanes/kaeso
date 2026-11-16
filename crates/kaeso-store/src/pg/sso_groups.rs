use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::members::Grant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub claim: String,
    pub scopes: Vec<String>,
    pub mode: String,
    pub claim_missing_at: Option<OffsetDateTime>,
}

/// A project a rule makes the person a member of, with the role they get there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleProject {
    pub project_id: Uuid,
    pub preset: String,
    pub role_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub group_value: String,
    pub org_preset: Option<String>,
    pub org_role_id: Option<Uuid>,
    pub projects: Vec<RuleProject>,
}

pub async fn settings(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Option<Settings>, StoreError> {
    Ok(sqlx::query_as!(
        Settings,
        "SELECT claim, scopes, mode, claim_missing_at FROM sso_group_settings WHERE org_id = $1",
        org_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn rules(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Rule>, StoreError> {
    let rows = sqlx::query!(
        "SELECT group_value, org_preset, org_role_id, projects FROM sso_group_rules \
         WHERE org_id = $1 ORDER BY position",
        org_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Rule {
            group_value: r.group_value,
            org_preset: r.org_preset,
            org_role_id: r.org_role_id,
            projects: serde_json::from_value(r.projects).unwrap_or_default(),
        })
        .collect())
}

/// Replaces the organization's settings and rules.
pub async fn put(
    db: &mut sqlx::PgConnection,
    org_id: Uuid,
    s: &Settings,
    rules: &[Rule],
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO sso_group_settings (org_id, claim, scopes, mode) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (org_id) DO UPDATE SET claim = EXCLUDED.claim, scopes = EXCLUDED.scopes, \
         mode = EXCLUDED.mode, claim_missing_at = CASE WHEN sso_group_settings.claim = EXCLUDED.claim \
         THEN sso_group_settings.claim_missing_at END, updated_at = now()",
        org_id,
        s.claim,
        &s.scopes,
        s.mode
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM sso_group_rules WHERE org_id = $1", org_id)
        .execute(&mut *db)
        .await?;
    for (i, r) in rules.iter().enumerate() {
        sqlx::query!(
            "INSERT INTO sso_group_rules (org_id, position, group_value, org_preset, org_role_id, projects) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            org_id,
            i32::try_from(i).unwrap_or(i32::MAX),
            r.group_value,
            r.org_preset,
            r.org_role_id,
            serde_json::to_value(&r.projects).unwrap_or_default()
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn claim_missing(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sso_group_settings SET claim_missing_at = $2 WHERE org_id = $1",
        org_id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_last_groups(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    groups: &[String],
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sso_identities SET last_groups = $3 WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id,
        groups
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn last_groups(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Vec<String>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT last_groups FROM sso_identities WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .fetch_optional(db)
    .await?
    .flatten())
}

/// Whether a rule gave the member's organization role.
pub async fn org_from_rule(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT from_sso_group FROM org_memberships WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

/// Sets the member's organization role; `from_rule` records whether a rule gave it.
pub async fn set_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    g: &Grant,
    from_rule: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE org_memberships SET preset = $3, role_id = $4, rights_added = '{}', rights_removed = '{}', \
         from_sso_group = $5, updated_at = now() WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id,
        g.preset,
        g.role_id,
        from_rule
    )
    .execute(db)
    .await?;
    Ok(())
}

/// One of the person's project memberships in the organization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    pub project_id: Uuid,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub from_sso_group: bool,
}

pub async fn project_memberships(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Membership>, StoreError> {
    Ok(sqlx::query_as!(
        Membership,
        "SELECT m.project_id, m.preset, m.role_id, m.from_sso_group FROM project_memberships m \
         JOIN projects p ON p.id = m.project_id WHERE p.org_id = $1 AND m.user_id = $2",
        org_id,
        user_id
    )
    .fetch_all(db)
    .await?)
}

/// The organization's projects that take members: not archived.
pub async fn active_projects(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM projects WHERE org_id = $1 AND archived_at IS NULL",
        org_id
    )
    .fetch_all(db)
    .await?)
}

/// Adds or changes a project membership a rule gives.
pub async fn put_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
    preset: &str,
    role_id: Option<Uuid>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO project_memberships (id, project_id, user_id, preset, role_id, from_sso_group) \
         VALUES ($1, $2, $3, $4, $5, true) ON CONFLICT (project_id, user_id) DO UPDATE SET \
         preset = EXCLUDED.preset, role_id = EXCLUDED.role_id, rights_added = '{}', rights_removed = '{}', \
         from_sso_group = true, updated_at = now()",
        Uuid::now_v7(),
        project_id,
        user_id,
        preset,
        role_id
    )
    .execute(db)
    .await?;
    Ok(())
}
