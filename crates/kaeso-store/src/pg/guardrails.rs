use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::credentials::Sealed;

#[derive(Clone, Debug, PartialEq)]
pub struct GuardrailRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub rules: Option<serde_json::Value>,
    pub timeout_ms: i32,
    pub on_error: String,
    pub request_phase: bool,
    pub reply_phase: bool,
    pub streams: String,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub revision: i64,
}

/// A guardrail with its sealed signing secret, as a request needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Armed {
    pub row: GuardrailRow,
    pub secret: Option<SealedSecret>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedSecret {
    pub guardrail_id: Uuid,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
}

#[derive(Clone, Debug)]
pub struct NewGuardrail<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub url: Option<&'a str>,
    pub rules: Option<&'a serde_json::Value>,
    pub secret: Option<Sealed<'a>>,
    pub timeout_ms: i32,
    pub on_error: &'a str,
    pub request_phase: bool,
    pub reply_phase: bool,
    pub streams: &'a str,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
}

#[derive(Clone, Debug, Default)]
pub struct Changes<'a> {
    pub name: Option<&'a str>,
    pub url: Option<&'a str>,
    pub rules: Option<&'a serde_json::Value>,
    pub timeout_ms: Option<i32>,
    pub on_error: Option<&'a str>,
    pub request_phase: Option<bool>,
    pub reply_phase: Option<bool>,
    pub streams: Option<&'a str>,
}

pub async fn insert(db: impl PgExecutor<'_>, g: &NewGuardrail<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO guardrails (id, org_id, name, url, secret_ciphertext, secret_nonce, \
         secret_wrapped_key, secret_key_id, timeout_ms, on_error, request_phase, reply_phase, \
         streams, created_by_kind, created_by_id, kind, rules) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        g.id,
        g.org_id,
        g.name,
        g.url,
        g.secret.as_ref().map(|x| x.ciphertext),
        g.secret.as_ref().map(|x| x.nonce),
        g.secret.as_ref().map(|x| x.wrapped_key),
        g.secret.as_ref().map(|x| x.key_id),
        g.timeout_ms,
        g.on_error,
        g.request_phase,
        g.reply_phase,
        g.streams,
        g.created_by_kind,
        g.created_by_id,
        g.kind,
        g.rules
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<GuardrailRow>, StoreError> {
    Ok(sqlx::query_as!(
        GuardrailRow,
        "SELECT id, org_id, kind, name, url, rules, timeout_ms, on_error, request_phase, reply_phase, \
         streams, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM guardrails WHERE org_id = $1 ORDER BY name, id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<GuardrailRow>, StoreError> {
    Ok(sqlx::query_as!(
        GuardrailRow,
        "SELECT id, org_id, kind, name, url, rules, timeout_ms, on_error, request_phase, reply_phase, \
         streams, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM guardrails WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    c: &Changes<'_>,
) -> Result<Option<GuardrailRow>, StoreError> {
    Ok(sqlx::query_as!(
        GuardrailRow,
        "UPDATE guardrails SET name = COALESCE($3, name), url = COALESCE($4, url), \
         timeout_ms = COALESCE($5, timeout_ms), on_error = COALESCE($6, on_error), \
         request_phase = COALESCE($7, request_phase), reply_phase = COALESCE($8, reply_phase), \
         streams = COALESCE($9, streams), rules = COALESCE($10, rules), revision = revision + 1, \
         updated_at = now() \
         WHERE org_id = $1 AND id = $2 \
         RETURNING id, org_id, kind, name, url, rules, timeout_ms, on_error, request_phase, \
         reply_phase, streams, created_by_kind, created_by_id, created_at, updated_at, revision",
        org_id,
        id,
        c.name,
        c.url,
        c.timeout_ms,
        c.on_error,
        c.request_phase,
        c.reply_phase,
        c.streams,
        c.rules
    )
    .fetch_optional(db)
    .await?)
}

pub async fn set_secret(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    secret: &Sealed<'_>,
) -> Result<Option<GuardrailRow>, StoreError> {
    Ok(sqlx::query_as!(
        GuardrailRow,
        "UPDATE guardrails SET secret_ciphertext = $3, secret_nonce = $4, secret_wrapped_key = $5, \
         secret_key_id = $6, revision = revision + 1, updated_at = now() \
         WHERE org_id = $1 AND id = $2 \
         RETURNING id, org_id, kind, name, url, rules, timeout_ms, on_error, request_phase, \
         reply_phase, streams, created_by_kind, created_by_id, created_at, updated_at, revision",
        org_id,
        id,
        secret.ciphertext,
        secret.nonce,
        secret.wrapped_key,
        secret.key_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM guardrails WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

fn sealed_of(
    guardrail_id: Uuid,
    ciphertext: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
    wrapped_key: Option<Vec<u8>>,
    key_id: Option<String>,
) -> Option<SealedSecret> {
    Some(SealedSecret {
        guardrail_id,
        ciphertext: ciphertext?,
        nonce: nonce?,
        wrapped_key: wrapped_key?,
        key_id: key_id?,
    })
}

pub async fn sealed(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<SealedSecret>, StoreError> {
    let row = sqlx::query!(
        "SELECT id, secret_ciphertext, secret_nonce, secret_wrapped_key, secret_key_id \
         FROM guardrails WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.and_then(|r| {
        sealed_of(
            r.id,
            r.secret_ciphertext,
            r.secret_nonce,
            r.secret_wrapped_key,
            r.secret_key_id,
        )
    }))
}

/// Every guardrail of the organization with the projects that use it.
pub async fn project_links(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT guardrail_id, project_id FROM project_guardrails WHERE org_id = $1 \
         ORDER BY guardrail_id, project_id",
        org_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.guardrail_id, r.project_id))
        .collect())
}

/// The guardrails a project uses, in the order they run.
pub async fn of_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<GuardrailRow>, StoreError> {
    Ok(sqlx::query_as!(
        GuardrailRow,
        "SELECT g.id, g.org_id, g.kind, g.name, g.url, g.rules, g.timeout_ms, g.on_error, \
         g.request_phase, g.reply_phase, g.streams, g.created_by_kind, g.created_by_id, g.created_at, g.updated_at, \
         g.revision \
         FROM project_guardrails p JOIN guardrails g ON g.id = p.guardrail_id AND g.org_id = p.org_id \
         WHERE p.project_id = $1 ORDER BY p.position",
        project_id
    )
    .fetch_all(db)
    .await?)
}

/// The guardrails a project uses, in the order they run, with their sealed secrets.
pub async fn armed_for_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<Armed>, StoreError> {
    let rows = sqlx::query!(
        "SELECT g.id, g.org_id, g.kind, g.name, g.url, g.rules, g.timeout_ms, g.on_error, \
         g.request_phase, g.reply_phase, g.streams, g.created_by_kind, g.created_by_id, g.created_at, g.updated_at, \
         g.revision, g.secret_ciphertext, g.secret_nonce, g.secret_wrapped_key, g.secret_key_id \
         FROM project_guardrails p JOIN guardrails g ON g.id = p.guardrail_id AND g.org_id = p.org_id \
         WHERE p.project_id = $1 ORDER BY p.position",
        project_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Armed {
            secret: sealed_of(
                r.id,
                r.secret_ciphertext,
                r.secret_nonce,
                r.secret_wrapped_key,
                r.secret_key_id,
            ),
            row: GuardrailRow {
                id: r.id,
                org_id: r.org_id,
                kind: r.kind,
                name: r.name,
                url: r.url,
                rules: r.rules,
                timeout_ms: r.timeout_ms,
                on_error: r.on_error,
                request_phase: r.request_phase,
                reply_phase: r.reply_phase,
                streams: r.streams,
                created_by_kind: r.created_by_kind,
                created_by_id: r.created_by_id,
                created_at: r.created_at,
                updated_at: r.updated_at,
                revision: r.revision,
            },
        })
        .collect())
}

/// Sets which of the organization's guardrails a project uses, in this order. Ids that aren't
/// the organization's are left out; the caller checks them first.
pub async fn set_project(
    db: &mut PgConnection,
    org_id: Uuid,
    project_id: Uuid,
    guardrails: &[Uuid],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_guardrails WHERE org_id = $1 AND project_id = $2",
        org_id,
        project_id
    )
    .execute(&mut *db)
    .await?;
    let positions: Vec<i32> = (0..guardrails.len())
        .map(|i| i32::try_from(i).unwrap_or(i32::MAX))
        .collect();
    sqlx::query!(
        "INSERT INTO project_guardrails (project_id, org_id, guardrail_id, position) \
         SELECT $2, g.org_id, g.id, w.position \
         FROM unnest($3::uuid[], $4::int[]) AS w(id, position) \
         JOIN guardrails g ON g.id = w.id AND g.org_id = $1",
        org_id,
        project_id,
        guardrails,
        &positions
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<SealedSecret>, StoreError> {
    let rows = sqlx::query!(
        "SELECT id, secret_ciphertext, secret_nonce, secret_wrapped_key, secret_key_id FROM guardrails \
         WHERE secret_ciphertext IS NOT NULL"
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            sealed_of(
                r.id,
                r.secret_ciphertext,
                r.secret_nonce,
                r.secret_wrapped_key,
                r.secret_key_id,
            )
        })
        .collect())
}

pub async fn set_wrapped_key(
    db: impl PgExecutor<'_>,
    id: Uuid,
    old_wrapped_key: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE guardrails SET secret_wrapped_key = $3, secret_key_id = $4 \
         WHERE id = $1 AND secret_wrapped_key = $2",
        id,
        old_wrapped_key,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}
