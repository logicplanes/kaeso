use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Setup,
    MagicLink,
    PasswordReset,
    EmailChange,
    EmailChangeCancel,
    TotpChallenge,
    Signup,
    OidcState,
}

impl TokenKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenKind::Setup => "setup",
            TokenKind::MagicLink => "magic_link",
            TokenKind::PasswordReset => "password_reset",
            TokenKind::EmailChange => "email_change",
            TokenKind::EmailChangeCancel => "email_change_cancel",
            TokenKind::TotpChallenge => "totp_challenge",
            TokenKind::Signup => "signup",
            TokenKind::OidcState => "oidc_state",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
    pub data: Value,
    pub expires_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct NewToken<'a> {
    pub kind: TokenKind,
    pub hash: &'a [u8],
    pub user_id: Option<Uuid>,
    pub email: Option<&'a str>,
    pub data: Value,
    pub expires_at: OffsetDateTime,
}

pub async fn insert(db: impl PgExecutor<'_>, t: NewToken<'_>) -> Result<Uuid, StoreError> {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO sign_in_tokens (id, kind, token_hash, user_id, email, data, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        id,
        t.kind.as_str(),
        t.hash,
        t.user_id,
        t.email,
        t.data,
        t.expires_at,
    )
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn retire_all(
    db: impl PgExecutor<'_>,
    kind: TokenKind,
    user_id: Option<Uuid>,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sign_in_tokens SET used_at = $1 WHERE kind = $2 AND used_at IS NULL \
         AND ($3::uuid IS NULL OR user_id = $3)",
        now,
        kind.as_str(),
        user_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn consume(
    db: impl PgExecutor<'_>,
    kind: TokenKind,
    hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Token>, StoreError> {
    let row = sqlx::query_as!(
        Token,
        "UPDATE sign_in_tokens SET used_at = $1 \
         WHERE kind = $2 AND token_hash = $3 AND used_at IS NULL AND expires_at > $1 \
         RETURNING id, user_id, email, data, expires_at",
        now,
        kind.as_str(),
        hash,
    )
    .fetch_optional(db)
    .await?;
    Ok(row)
}

pub async fn peek(
    db: impl PgExecutor<'_>,
    kind: TokenKind,
    hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Token>, StoreError> {
    let row = sqlx::query_as!(
        Token,
        "SELECT id, user_id, email, data, expires_at FROM sign_in_tokens \
         WHERE kind = $1 AND token_hash = $2 AND used_at IS NULL AND expires_at > $3",
        kind.as_str(),
        hash,
        now,
    )
    .fetch_optional(db)
    .await?;
    Ok(row)
}

pub async fn delete_of_user(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<u64, StoreError> {
    Ok(
        sqlx::query!("DELETE FROM sign_in_tokens WHERE user_id = $1", user_id)
            .execute(db)
            .await?
            .rows_affected(),
    )
}

pub async fn delete_stale(db: impl PgExecutor<'_>, now: OffsetDateTime) -> Result<u64, StoreError> {
    let day_ago = now - time::Duration::days(1);
    let done = sqlx::query!(
        "DELETE FROM sign_in_tokens WHERE expires_at < $1 OR used_at < $2",
        now,
        day_ago
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}
