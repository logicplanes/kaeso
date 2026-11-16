//! Webhooks that receive an organization's operational notices, and their outbox.

use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

/// An organization has at most this many webhooks.
pub const MAX_PER_ORG: i64 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub url: String,
    pub format: String,
    pub kinds: Vec<String>,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub consecutive_failures: i32,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A sealed signing secret, as `kaeso_crypto::seal` makes it.
#[derive(Clone, Copy, Debug)]
pub struct Secret<'a> {
    pub ciphertext: &'a [u8],
    pub nonce: &'a [u8],
    pub wrapped_key: &'a [u8],
    pub key_id: &'a str,
}

#[derive(Clone, Debug)]
pub struct NewWebhook<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: &'a str,
    pub url: &'a str,
    pub format: &'a str,
    pub kinds: &'a [String],
    pub secret: Secret<'a>,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
    pub at: OffsetDateTime,
}

/// What an update sets; `None` keeps the field.
#[derive(Clone, Debug, Default)]
pub struct Change<'a> {
    pub name: Option<&'a str>,
    pub url: Option<&'a str>,
    pub format: Option<&'a str>,
    pub kinds: Option<&'a [String]>,
    pub project_id: Option<Option<Uuid>>,
    pub enabled: Option<bool>,
}

/// Holds the organization's webhooks for the rest of the transaction, so two can't be added past
/// the limit at once.
pub async fn lock(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('notification_webhooks:' || $1::uuid::text, 0))",
        org_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn count(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM notification_webhooks WHERE org_id = $1",
        org_id
    )
    .fetch_one(db)
    .await?)
}

pub async fn insert(db: impl PgExecutor<'_>, w: &NewWebhook<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO notification_webhooks (id, org_id, project_id, name, url, format, kinds, \
         ciphertext, nonce, wrapped_key, key_id, created_by_kind, created_by_id, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)",
        w.id,
        w.org_id,
        w.project_id,
        w.name,
        w.url,
        w.format,
        w.kinds,
        w.secret.ciphertext,
        w.secret.nonce,
        w.secret.wrapped_key,
        w.secret.key_id,
        w.created_by_kind,
        w.created_by_id,
        w.at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<WebhookRow>, StoreError> {
    Ok(sqlx::query_as!(
        WebhookRow,
        "SELECT id, org_id, project_id, name, url, format, kinds, ciphertext, nonce, wrapped_key, key_id, \
         enabled, disabled_reason, consecutive_failures, last_status, last_error, last_attempt_at, \
         created_by_kind, created_by_id, created_at, updated_at \
         FROM notification_webhooks WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<WebhookRow>, StoreError> {
    Ok(sqlx::query_as!(
        WebhookRow,
        "SELECT id, org_id, project_id, name, url, format, kinds, ciphertext, nonce, wrapped_key, key_id, \
         enabled, disabled_reason, consecutive_failures, last_status, last_error, last_attempt_at, \
         created_by_kind, created_by_id, created_at, updated_at \
         FROM notification_webhooks WHERE org_id = $1 ORDER BY created_at, id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

/// Applies `change`; turning a webhook on clears its failures. False when it doesn't exist.
pub async fn update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    change: &Change<'_>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "UPDATE notification_webhooks SET \
         name = COALESCE($3, name), \
         url = COALESCE($4, url), \
         format = COALESCE($5, format), \
         kinds = COALESCE($6, kinds), \
         project_id = CASE WHEN $7 THEN $8 ELSE project_id END, \
         enabled = COALESCE($9, enabled), \
         disabled_reason = CASE WHEN $9 IS NULL THEN disabled_reason ELSE NULL END, \
         consecutive_failures = CASE WHEN $9 THEN 0 ELSE consecutive_failures END, \
         updated_at = $10 \
         WHERE org_id = $1 AND id = $2",
        org_id,
        id,
        change.name,
        change.url,
        change.format,
        change.kinds,
        change.project_id.is_some(),
        change.project_id.flatten(),
        change.enabled,
        at
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn set_secret(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    secret: Secret<'_>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "UPDATE notification_webhooks SET ciphertext = $3, nonce = $4, wrapped_key = $5, key_id = $6, \
         updated_at = $7 WHERE org_id = $1 AND id = $2",
        org_id,
        id,
        secret.ciphertext,
        secret.nonce,
        secret.wrapped_key,
        secret.key_id,
        at
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "DELETE FROM notification_webhooks WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// The organization's webhooks that are on and take `kind`, with their format: for an event of
/// the organization, those without a project; for an event of a project, those without one or
/// with this one.
pub async fn subscribed(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Option<Uuid>,
    kind: &str,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT id, format FROM notification_webhooks \
         WHERE org_id = $1 AND enabled AND $3 = ANY(kinds) \
         AND (project_id IS NULL OR project_id = $2) \
         ORDER BY created_at, id",
        org_id,
        project_id,
        kind
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.format))
    .collect())
}

#[derive(Clone, Debug)]
pub struct NewDelivery<'a> {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub message_id: Uuid,
    pub event_type: &'a str,
    pub body: &'a str,
    pub at: OffsetDateTime,
}

pub async fn enqueue(db: impl PgExecutor<'_>, d: &NewDelivery<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO webhook_deliveries (id, webhook_id, message_id, event_type, body, next_attempt_at, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
        d.id,
        d.webhook_id,
        d.message_id,
        d.event_type,
        d.body,
        d.at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// A delivery that is due, with what sending it needs.
#[derive(Clone, Debug)]
pub struct Due {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub message_id: Uuid,
    pub event_type: String,
    pub body: String,
    pub attempts: i32,
    pub created_at: OffsetDateTime,
}

pub async fn due(
    db: impl PgExecutor<'_>,
    now: OffsetDateTime,
    limit: i64,
) -> Result<Vec<Due>, StoreError> {
    Ok(sqlx::query_as!(
        Due,
        "SELECT d.id, d.webhook_id, w.org_id, w.name, w.url, w.enabled, w.ciphertext, w.nonce, \
         w.wrapped_key, w.key_id, d.message_id, d.event_type, d.body, d.attempts, d.created_at \
         FROM webhook_deliveries d JOIN notification_webhooks w ON w.id = d.webhook_id \
         WHERE d.status = 'pending' AND d.next_attempt_at <= $1 \
         ORDER BY d.next_attempt_at, d.id LIMIT $2",
        now,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// How a try ended: delivered, to be tried again at `next`, or failed for good.
#[derive(Clone, Debug)]
pub struct Attempt<'a> {
    pub status: Option<i32>,
    pub error: Option<&'a str>,
    pub delivered: bool,
    pub next: Option<OffsetDateTime>,
    pub at: OffsetDateTime,
}

/// Records a try of a delivery and of its webhook; the webhook's failures in a row after it.
pub async fn record_attempt(
    db: &mut sqlx::PgConnection,
    id: Uuid,
    webhook_id: Uuid,
    a: &Attempt<'_>,
) -> Result<i32, StoreError> {
    let state = match (a.delivered, a.next) {
        (true, _) => "delivered",
        (false, Some(_)) => "pending",
        (false, None) => "failed",
    };
    sqlx::query!(
        "UPDATE webhook_deliveries SET attempts = attempts + 1, status = $2, last_status = $3, \
         last_error = $4, next_attempt_at = COALESCE($5, next_attempt_at), \
         finished_at = CASE WHEN $2 = 'pending' THEN NULL ELSE $6::timestamptz END \
         WHERE id = $1",
        id,
        state,
        a.status,
        a.error,
        a.next,
        a.at
    )
    .execute(&mut *db)
    .await?;
    Ok(sqlx::query_scalar!(
        "UPDATE notification_webhooks SET last_status = $2, last_error = $3, last_attempt_at = $4, \
         consecutive_failures = CASE WHEN $5 THEN 0 ELSE consecutive_failures + 1 END \
         WHERE id = $1 RETURNING consecutive_failures",
        webhook_id,
        a.status,
        a.error,
        a.at,
        a.delivered
    )
    .fetch_optional(&mut *db)
    .await?
    .unwrap_or(0))
}

/// Turns a webhook off after too many failures in a row; false when it was off already.
pub async fn disable_for_failures(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "UPDATE notification_webhooks SET enabled = false, disabled_reason = 'failures', updated_at = $2 \
         WHERE id = $1 AND enabled",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// A delivery that is no longer due: its webhook was turned off or the delivery is for a
/// webhook that no longer takes it.
pub async fn drop_pending(
    db: impl PgExecutor<'_>,
    id: Uuid,
    reason: &str,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE webhook_deliveries SET status = 'failed', last_error = $2, finished_at = $3 \
         WHERE id = $1 AND status = 'pending'",
        id,
        reason,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryRow {
    pub id: Uuid,
    pub message_id: Uuid,
    pub event_type: String,
    pub status: String,
    pub attempts: i32,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub next_attempt_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

/// A webhook's latest deliveries, newest first.
pub async fn deliveries(
    db: impl PgExecutor<'_>,
    webhook_id: Uuid,
    limit: i64,
) -> Result<Vec<DeliveryRow>, StoreError> {
    Ok(sqlx::query_as!(
        DeliveryRow,
        "SELECT id, message_id, event_type, status, attempts, last_status, last_error, next_attempt_at, \
         created_at, finished_at FROM webhook_deliveries WHERE webhook_id = $1 \
         ORDER BY created_at DESC, id DESC LIMIT $2",
        webhook_id,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// Forgets deliveries made before `before`; how many.
pub async fn prune(db: impl PgExecutor<'_>, before: OffsetDateTime) -> Result<u64, StoreError> {
    let r = sqlx::query!(
        "DELETE FROM webhook_deliveries WHERE created_at < $1 AND status <> 'pending'",
        before
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}
