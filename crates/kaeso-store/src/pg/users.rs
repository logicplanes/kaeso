use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    pub name: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<OffsetDateTime>,
    pub state: String,
    pub is_instance_admin: bool,
    pub notify_budget_alerts: bool,
    pub notify_key_expiry: bool,
    pub created_at: OffsetDateTime,
    pub avatar: Option<String>,
}

pub async fn any_exist(db: impl PgExecutor<'_>) -> Result<bool, StoreError> {
    let row = sqlx::query_scalar!("SELECT EXISTS (SELECT 1 FROM users) AS \"exists!\"")
        .fetch_one(db)
        .await?;
    Ok(row)
}

#[derive(Clone, Debug)]
pub struct NewUser<'a> {
    pub id: Uuid,
    pub email: &'a str,
    pub name: &'a str,
    pub password_hash: Option<&'a str>,
    pub email_verified_at: Option<OffsetDateTime>,
    pub is_instance_admin: bool,
}

pub async fn insert(
    db: impl sqlx::Acquire<'_, Database = sqlx::Postgres>,
    u: &NewUser<'_>,
) -> Result<bool, StoreError> {
    let mut tx = db.begin().await?;
    lock_address(&mut tx, u.email).await?;
    let done = sqlx::query!(
        "INSERT INTO users (id, email, name, password_hash, email_verified_at, is_instance_admin) \
         SELECT $1, $2, $3, $4, $5, $6 \
         WHERE NOT EXISTS (SELECT 1 FROM users WHERE email_key(email) = email_key($2)) \
         ON CONFLICT DO NOTHING",
        u.id,
        u.email,
        u.name,
        u.password_hash,
        u.email_verified_at,
        u.is_instance_admin,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(done.rows_affected() == 1)
}

// The canonical form has no unique index, because accounts stored before it existed may share
// one; this lock makes the check and the write of two accounts with one form take turns.
async fn lock_address(db: &mut sqlx::PgConnection, email: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('users_email:' || email_key($1), 0))",
        email
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<User>, StoreError> {
    Ok(sqlx::query_as!(
        User,
        "SELECT id, email, name, password_hash, email_verified_at, state, is_instance_admin, \
         notify_budget_alerts, notify_key_expiry, created_at, avatar FROM users WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

// Two accounts with one email_key() predate the rule, and neither is picked.
pub async fn get_by_email(
    db: impl PgExecutor<'_>,
    email: &str,
) -> Result<Option<User>, StoreError> {
    let mut same = sqlx::query_as!(
        User,
        "SELECT id, email, name, password_hash, email_verified_at, state, is_instance_admin, \
         notify_budget_alerts, notify_key_expiry, created_at, avatar FROM users \
         WHERE email_key(email) = email_key($1) ORDER BY id",
        email
    )
    .fetch_all(db)
    .await?;
    if same.len() > 1 {
        let users: Vec<Uuid> = same.iter().map(|u| u.id).collect();
        tracing::warn!(
            ?users,
            "accounts share one canonical email address; refusing to look either up by it"
        );
        return Ok(None);
    }
    Ok(same.pop())
}

pub async fn set_password(db: impl PgExecutor<'_>, id: Uuid, hash: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1",
        id,
        hash
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_name(db: impl PgExecutor<'_>, id: Uuid, name: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET name = $2, updated_at = now() WHERE id = $1",
        id,
        name
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_locale(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<String>, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT locale FROM users WHERE id = $1", id)
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipient {
    pub id: Uuid,
    pub email: Option<String>,
    pub state: String,
    pub notify_key_expiry: bool,
    pub locale: Option<String>,
}

pub async fn recipients(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<Recipient>, StoreError> {
    Ok(sqlx::query_as!(
        Recipient,
        "SELECT id, email, state, notify_key_expiry, locale FROM users WHERE id = ANY($1) ORDER BY id",
        ids
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_locale(
    db: impl PgExecutor<'_>,
    id: Uuid,
    locale: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET locale = $2, updated_at = now() WHERE id = $1",
        id,
        locale
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn email_taken(
    db: impl PgExecutor<'_>,
    email: &str,
    except: Option<Uuid>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM users WHERE email_key(email) = email_key($1) \
         AND ($2::uuid IS NULL OR id <> $2)) AS \"t!\"",
        email,
        except
    )
    .fetch_one(db)
    .await?)
}

pub async fn set_email(
    db: impl sqlx::Acquire<'_, Database = sqlx::Postgres>,
    id: Uuid,
    email: &str,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let mut tx = db.begin().await?;
    lock_address(&mut tx, email).await?;
    let result = sqlx::query!(
        "UPDATE users SET email = $2, email_verified_at = $3, updated_at = now() WHERE id = $1 \
         AND NOT EXISTS (SELECT 1 FROM users WHERE email_key(email) = email_key($2) AND id <> $1)",
        id,
        email,
        at
    )
    .execute(&mut *tx)
    .await;
    match result {
        Ok(done) => {
            tx.commit().await?;
            Ok(done.rows_affected() == 1)
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub async fn mark_verified(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $2), updated_at = now() WHERE id = $1",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_notifications(
    db: impl PgExecutor<'_>,
    id: Uuid,
    budget_alerts: bool,
    key_expiry: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET notify_budget_alerts = $2, notify_key_expiry = $3, updated_at = now() WHERE id = $1",
        id,
        budget_alerts,
        key_expiry
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_state(db: impl PgExecutor<'_>, id: Uuid, state: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET state = $2, updated_at = now() WHERE id = $1 AND state <> 'deleted'",
        id,
        state
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_instance_admin(
    db: impl PgExecutor<'_>,
    id: Uuid,
    admin: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET is_instance_admin = $2, updated_at = now() WHERE id = $1",
        id,
        admin
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn lock_instance_admins(db: &mut sqlx::PgConnection) -> Result<(), StoreError> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended('instance_admins', 0))")
        .execute(db)
        .await?;
    Ok(())
}

pub async fn count_instance_admins(db: impl PgExecutor<'_>) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM users WHERE is_instance_admin AND state = 'active'"
    )
    .fetch_one(db)
    .await?)
}

pub async fn delete_personal_data(
    db: &mut sqlx::PgConnection,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<Option<String>, StoreError> {
    let avatar = sqlx::query_scalar!("SELECT avatar FROM users WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *db)
        .await?
        .flatten();
    sqlx::query!(
        "UPDATE users SET state = 'deleted', email = NULL, name = NULL, password_hash = NULL, \
         email_verified_at = NULL, is_instance_admin = false, avatar = NULL, locale = NULL, \
         deleted_at = $2, updated_at = now() WHERE id = $1",
        id,
        at
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM sso_identities WHERE user_id = $1", id)
        .execute(&mut *db)
        .await?;
    Ok(avatar)
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
    search: Option<&str>,
    state: Option<&str>,
) -> Result<Vec<User>, StoreError> {
    let like = |s: &str| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    };
    let pattern = search.map(like);
    let stored = search
        .and_then(kaeso_core::email::stored_form)
        .map(|s| like(&s));
    Ok(sqlx::query_as!(
        User,
        "SELECT id, email, name, password_hash, email_verified_at, state, is_instance_admin, \
         notify_budget_alerts, notify_key_expiry, created_at, avatar FROM users \
         WHERE ($1::uuid IS NULL OR id > $1) \
         AND ($3::text IS NULL OR email ILIKE $3 OR name ILIKE $3 OR email ILIKE $5) \
         AND ($4::text IS NULL OR state = $4) \
         ORDER BY id LIMIT $2",
        after,
        limit,
        pattern,
        state,
        stored,
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_avatar(
    db: impl PgExecutor<'_>,
    id: Uuid,
    avatar: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET avatar = $2, updated_at = now() WHERE id = $1",
        id,
        avatar
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn seen_version(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<String>, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT seen_version FROM users WHERE id = $1", id)
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

pub async fn set_seen_version(
    db: impl PgExecutor<'_>,
    id: Uuid,
    version: &str,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE users SET seen_version = $2 WHERE id = $1",
        id,
        version
    )
    .execute(db)
    .await?;
    Ok(())
}
