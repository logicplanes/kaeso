use kaeso_core::id::{OrgId, UserId};
use kaeso_core::rights::OrgPreset;
use kaeso_mail::Actor;
use kaeso_store::StoreError;
use kaeso_store::pg::members::{self, Grant};
use kaeso_store::pg::{instance, orgs};

use crate::platform::audit::Action;
use crate::services::mail_lang;

pub fn org_grant(preset: OrgPreset) -> Grant {
    Grant {
        preset: preset.as_str().to_owned(),
        ..Grant::default()
    }
}

pub async fn create(
    tx: &mut sqlx::PgConnection,
    name: &str,
    owner: UserId,
) -> Result<OrgId, StoreError> {
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
    members::insert_org(
        &mut *tx,
        org.uuid(),
        owner.uuid(),
        &org_grant(OrgPreset::Owner),
    )
    .await?;
    Ok(org)
}

pub async fn notify_deletion(
    state: &crate::state::AppState,
    ctx: &crate::platform::access::Ctx,
    org: &orgs::Org,
) {
    // The org is scheduled already, so a failure here mustn't turn the answer into an error.
    if let Err(e) = send_deletion_notices(state, ctx, org).await {
        tracing::error!(org = %OrgId::from_uuid(org.id), error = %e, "cannot send the deletion notices");
    }
}

async fn send_deletion_notices(
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
    let Some(due) = org.deletion_due_at else {
        return Ok(());
    };
    let zone = crate::time_zone::or_utc(&kaeso_store::pg::instance::time_zone(&state.pg).await?);
    let (due, zone) = (
        crate::time_zone::local(due, &zone),
        zone.iana_name().unwrap_or("UTC"),
    );
    let url = format!(
        "{}orgs/{}",
        state.config.public_url,
        OrgId::from_uuid(org.id)
    );
    let mut recipients = members::members_and_instance_admins(&state.pg, org.id).await?;
    recipients.sort_by_key(|r| r.email.to_lowercase());
    recipients.dedup_by(|a, b| a.email.eq_ignore_ascii_case(&b.email));
    for r in recipients {
        let lang = mail_lang::of_user(&state.pg, r.user_id, None).await;
        state
            .mailer
            .send(kaeso_mail::templates::org_scheduled_for_deletion(
                lang, &r.email, &org.name, by, due, zone, &url,
            ));
    }
    Ok(())
}

// Once due, the deletion job may already have deleted its logs, which a restore can't bring back.
const PAST_RESTORE: &str =
    "The organization is being deleted for good and can no longer be restored.";

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
    if row.deletion_due_at.is_some_and(|d| d <= ctx.now) {
        return Err(ApiError::Conflict(PAST_RESTORE.into()));
    }
    row.state = "active".into();
    row.deletion_due_at = None;
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(org), Action::OrgRestore)
        .target("org", org)
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgRestore)
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
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    // The row stays locked until the org is gone, so a restore that started before the due date
    // either commits first and stops this, or waits and finds nothing left to restore.
    let Some(row) = orgs::get_for_update(&mut *tx, org).await?.filter(due) else {
        return Ok(false);
    };
    delete_logs(state, org).await?;
    let counters: Vec<String> = kaeso_store::pg::limits::list_org(&mut *tx, org)
        .await?
        .iter()
        .filter_map(|l| l.limit().ok())
        .map(|l| state.valkey.key(&l.counter(now)))
        .flat_map(|k| [format!("{k}:f"), k])
        .collect();
    let projects = kaeso_store::pg::projects::ids_of_org(&mut *tx, org).await?;
    crate::services::keys::forget_org(state, org).await;
    orgs::delete_final(&mut tx, org).await?;
    crate::platform::audit::system_change(
        &mut tx,
        None,
        Action::OrgDeleteFinal,
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
    let Some(clickhouse) = state.clickhouse.as_ref() else {
        return Ok(());
    };
    if !state.clickhouse_migrated() {
        clickhouse.migrate(&state.pg).await?;
        state.set_clickhouse_migrated();
    }
    kaeso_store::clickhouse::logs::delete_org(clickhouse, org).await
}
