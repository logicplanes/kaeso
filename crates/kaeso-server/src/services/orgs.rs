use kaeso_core::id::{OrgId, ProjectId, UserId};
use kaeso_core::rights::{OrgPreset, ProjectPreset};
use kaeso_mail::Actor;
use kaeso_store::StoreError;
use kaeso_store::pg::members::{self, Grant};
use kaeso_store::pg::{instance, orgs, projects};

use crate::services::mail_lang;

pub const DEFAULT_PROJECT_NAME: &str = "Default project";

pub fn org_grant(preset: OrgPreset) -> Grant {
    Grant {
        preset: preset.as_str().to_owned(),
        ..Grant::default()
    }
}

pub fn project_grant(preset: ProjectPreset) -> Grant {
    Grant {
        preset: preset.as_str().to_owned(),
        ..Grant::default()
    }
}

pub fn default_project_preset(org: &Grant) -> ProjectPreset {
    org.preset
        .parse::<OrgPreset>()
        .map_or(ProjectPreset::Viewer, OrgPreset::default_project_preset)
}

pub async fn create(
    tx: &mut sqlx::PgConnection,
    name: &str,
    owner: UserId,
) -> Result<(OrgId, ProjectId), StoreError> {
    let settings = instance::get(&mut *tx).await?;
    let org = OrgId::new();
    orgs::insert(
        &mut *tx,
        org.uuid(),
        name,
        settings.metadata_retention_days,
        settings.content_retention_days,
    )
    .await?;
    let project = ProjectId::new();
    projects::insert(
        &mut *tx,
        project.uuid(),
        org.uuid(),
        DEFAULT_PROJECT_NAME,
        true,
    )
    .await?;
    members::insert_org(
        &mut *tx,
        org.uuid(),
        owner.uuid(),
        &org_grant(OrgPreset::Owner),
    )
    .await?;
    members::insert_project(
        &mut *tx,
        project.uuid(),
        owner.uuid(),
        &project_grant(OrgPreset::Owner.default_project_preset()),
    )
    .await?;
    Ok((org, project))
}

pub async fn add_member(
    tx: &mut sqlx::PgConnection,
    org: OrgId,
    user: UserId,
    grant: &Grant,
) -> Result<bool, StoreError> {
    let added = members::insert_org(&mut *tx, org.uuid(), user.uuid(), grant).await?;
    if added && let Some(default) = projects::default_of(&mut *tx, org.uuid()).await? {
        members::insert_project(
            &mut *tx,
            default.id,
            user.uuid(),
            &project_grant(default_project_preset(grant)),
        )
        .await?;
    }
    Ok(added)
}

pub async fn notify_deletion(
    state: &crate::state::AppState,
    ctx: &crate::platform::access::Ctx,
    org: &orgs::Org,
) -> Result<(), StoreError> {
    let name = match ctx.acting_user() {
        Some(u) => Some(
            kaeso_store::pg::users::get(&state.pg, u.uuid())
                .await?
                .and_then(|u| u.name),
        ),
        None => None,
    };
    let by = match &name {
        Some(Some(name)) => Actor::from(name),
        Some(None) => Actor::Member,
        None => Actor::Administrator,
    };
    let Some(due) = org.deletion_due_at.map(|d| d.date()) else {
        return Ok(());
    };
    let url = format!(
        "{}orgs/{}",
        state.config.public_url,
        OrgId::from_uuid(org.id)
    );
    let mut recipients: Vec<(uuid::Uuid, String)> = Vec::new();
    let mut after = None;
    loop {
        let page = members::list_org(&state.pg, org.id, after, 500, None, None).await?;
        after = page.last().map(|m| m.id);
        recipients.extend(
            page.iter()
                .filter(|m| m.user_state == "active")
                .filter_map(|m| Some((m.user_id, m.email.clone()?))),
        );
        if page.len() < 500 {
            break;
        }
    }
    let mut after = None;
    loop {
        let page =
            kaeso_store::pg::users::list(&state.pg, after, 500, None, Some("active")).await?;
        after = page.last().map(|u| u.id);
        recipients.extend(
            page.iter()
                .filter(|u| u.is_instance_admin)
                .filter_map(|u| Some((u.id, u.email.clone()?))),
        );
        if page.len() < 500 {
            break;
        }
    }
    recipients.sort_by_key(|(_, e)| e.to_lowercase());
    recipients.dedup_by(|a, b| a.1.eq_ignore_ascii_case(&b.1));
    for (user, to) in recipients {
        let lang = mail_lang::of_user(&state.pg, user, None).await;
        state
            .mailer
            .send(kaeso_mail::templates::org_scheduled_for_deletion(
                lang, &to, &org.name, by, due, &url,
            ));
    }
    Ok(())
}

pub async fn restore(
    state: &crate::state::AppState,
    ctx: &crate::platform::access::Ctx,
    org: OrgId,
) -> Result<orgs::Org, crate::platform::error::ApiError> {
    use crate::platform::error::ApiError;
    let mut tx = state.pg.begin().await?;
    let mut row = orgs::get_for_update(&mut *tx, org.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    if row.state != "scheduled_for_deletion" {
        return Err(ApiError::Conflict(
            "The organization is not scheduled for deletion.".into(),
        ));
    }
    row.state = "active".into();
    row.deletion_due_at = None;
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(org), "org.restore")
        .target("org", org)
        .write(&mut tx)
        .await?;
    ctx.audit(None, "org.restore")
        .target("org", org)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_org(state, org.uuid()).await;
    Ok(row)
}

pub async fn deletion_policy(
    db: impl sqlx::PgExecutor<'_>,
) -> Result<(bool, time::Duration), StoreError> {
    let s = instance::get(db).await?;
    Ok((
        s.org_deletion,
        time::Duration::days(i64::from(s.org_deletion_grace_days)),
    ))
}

pub async fn ensure_changeable(
    db: impl sqlx::PgExecutor<'_>,
    org: uuid::Uuid,
) -> Result<(), crate::platform::error::ApiError> {
    match orgs::get(db, org).await? {
        Some(o) if o.state == "scheduled_for_deletion" => {
            Err(crate::platform::error::ApiError::OrgScheduledForDeletion)
        }
        _ => Ok(()),
    }
}

pub fn ensure_deletion_allowed(allowed: bool) -> Result<(), crate::platform::error::ApiError> {
    if allowed {
        Ok(())
    } else {
        Err(crate::platform::error::ApiError::PermissionDenied(
            "This instance doesn't allow deleting organizations.".into(),
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PurgeError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Audit(#[from] crate::platform::error::ApiError),
}

pub async fn purge(
    state: &crate::state::AppState,
    org: uuid::Uuid,
    now: time::OffsetDateTime,
) -> Result<bool, PurgeError> {
    let due = |o: &orgs::Org| {
        o.state == "scheduled_for_deletion" && o.deletion_due_at.is_some_and(|d| d <= now)
    };
    match orgs::get(&state.pg, org).await? {
        Some(o) if due(&o) => {}
        _ => return Ok(false),
    }
    delete_logs(state, org).await?;
    let counters: Vec<String> = kaeso_store::pg::limits::list_org(&state.pg, org)
        .await?
        .iter()
        .filter_map(|l| l.limit().ok())
        .map(|l| state.valkey.key(&l.counter(now)))
        .flat_map(|k| [format!("{k}:f"), k])
        .collect();
    let projects = kaeso_store::pg::projects::ids_of_org(&state.pg, org).await?;
    crate::services::keys::forget_org(state, org).await;
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    let Some(row) = orgs::get_for_update(&mut *tx, org).await?.filter(due) else {
        return Ok(false);
    };
    orgs::delete_final(&mut tx, org).await?;
    crate::platform::audit::system_change(
        &mut tx,
        None,
        "org.delete_final",
        Some(("org", OrgId::from_uuid(org).to_string())),
        Some(serde_json::json!({ "name": row.name, "deletion_due_at": row.deletion_due_at })),
        None,
        now,
    )
    .await?;
    tx.commit().await.map_err(StoreError::from)?;
    if !counters.is_empty() {
        let mut conn = state.valkey.conn();
        if let Err(e) = redis::cmd("DEL")
            .arg(&counters)
            .query_async::<i64>(&mut conn)
            .await
        {
            tracing::warn!(error = %e, "cannot delete the deleted org's limit counters");
        }
    }
    crate::services::credentials::forget_org(state, org).await;
    for project in projects {
        crate::services::limits::forget(state, project).await;
    }
    Ok(true)
}

async fn delete_logs(state: &crate::state::AppState, org: uuid::Uuid) -> Result<(), StoreError> {
    if !state.clickhouse_migrated() {
        state.clickhouse.migrate(&state.pg).await?;
        state.set_clickhouse_migrated();
    }
    kaeso_store::clickhouse::logs::delete_org(&state.clickhouse, org).await
}
