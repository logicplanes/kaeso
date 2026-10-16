use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};

use crate::StoreError;

pub mod audit;
pub mod avatars;
pub mod catalogue;
pub mod credentials;
pub mod instance;
pub mod invites;
pub mod key_lifecycle;
pub mod keys;
pub mod limits;
pub mod log_jobs;
pub mod members;
pub mod oidc;
pub mod orgs;
pub mod projects;
pub mod roles;
pub mod service_accounts;
pub mod sessions;
pub mod tokens;
pub mod totp;
pub mod users;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

pub mod locks {
    pub const CLICKHOUSE_MIGRATIONS: i64 = 0x6b61_6573_6f00_0001;
    pub const SETUP: i64 = 0x6b61_6573_6f00_0002;
    pub const MODELS_DEV: i64 = 0x6b61_6573_6f00_0003;
    pub const JOBS_BASE: i64 = 0x6b61_6573_6f00_1000;
}

pub type Tx<'a> = Transaction<'a, Postgres>;

pub fn connect(url: &SecretString, pool_size: u32) -> Result<PgPool, StoreError> {
    let options: PgConnectOptions = url.expose_secret().parse().map_err(|_| {
        StoreError::Config("KAESO_POSTGRES_URL is not a valid PostgreSQL URL".into())
    })?;
    let options = options.application_name("kaeso");
    Ok(PgPoolOptions::new()
        .max_connections(pool_size)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .connect_lazy_with(options))
}

pub async fn migrate(pool: &PgPool) -> Result<(), StoreError> {
    MIGRATOR.run(pool).await.map_err(|e| match e {
        MigrateError::VersionMissing(version) => StoreError::Migrate(format!(
            "the database was migrated by a newer version of Kaeso (migration {version} is unknown to this version); run that version or newer"
        )),
        MigrateError::VersionMismatch(version) => StoreError::Migrate(format!(
            "migration {version} was changed after it was applied; migrations are forward-only"
        )),
        other => StoreError::Migrate(other.to_string()),
    })
}

pub async fn with_advisory_lock<T, F, Fut>(pool: &PgPool, id: i64, f: F) -> Result<T, StoreError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, StoreError>>,
{
    let mut held = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(id)
        .execute(&mut *held)
        .await?;
    let value = f().await?;
    held.commit().await?;
    Ok(value)
}

pub async fn try_with_advisory_lock<T, F, Fut>(
    pool: &PgPool,
    id: i64,
    f: F,
) -> Result<Option<T>, StoreError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, StoreError>>,
{
    let mut held = pool.begin().await?;
    let (locked,): (bool,) = sqlx::query_as("SELECT pg_try_advisory_xact_lock($1)")
        .bind(id)
        .fetch_one(&mut *held)
        .await?;
    if !locked {
        return Ok(None);
    }
    let value = f().await?;
    held.commit().await?;
    Ok(Some(value))
}

pub async fn database_size(pool: &PgPool) -> Result<i64, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT pg_database_size(current_database()) AS \"b!\"")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn lock_setup(db: &mut sqlx::PgConnection) -> Result<(), StoreError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(locks::SETUP)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn ping(pool: &PgPool) -> Result<(), StoreError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}
