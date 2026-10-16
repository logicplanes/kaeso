use sqlx::PgExecutor;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct Avatar {
    pub content_type: String,
    pub data: Vec<u8>,
}

pub async fn put(
    db: impl PgExecutor<'_>,
    hash: &str,
    content_type: &str,
    data: &[u8],
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO avatars (hash, content_type, data) VALUES ($1, $2, $3) ON CONFLICT (hash) DO NOTHING",
        hash,
        content_type,
        data
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(db: impl PgExecutor<'_>, hash: &str) -> Result<Option<Avatar>, StoreError> {
    Ok(sqlx::query_as!(
        Avatar,
        "SELECT content_type, data FROM avatars WHERE hash = $1",
        hash
    )
    .fetch_optional(db)
    .await?)
}

pub async fn delete_if_unused(db: impl PgExecutor<'_>, hash: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM avatars a WHERE a.hash = $1 \
         AND NOT EXISTS (SELECT 1 FROM users WHERE avatar = $1) \
         AND NOT EXISTS (SELECT 1 FROM organizations WHERE avatar = $1) \
         AND NOT EXISTS (SELECT 1 FROM projects WHERE avatar = $1)",
        hash
    )
    .execute(db)
    .await?;
    Ok(())
}
