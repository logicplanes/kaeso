#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("PostgreSQL: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("PostgreSQL migrations: {0}")]
    Migrate(String),
    #[error("ClickHouse: {0}")]
    ClickHouse(#[from] ::clickhouse::error::Error),
    #[error("ClickHouse migrations: {0}")]
    ClickHouseMigrate(String),
    #[error("Valkey: {0}")]
    Valkey(#[from] redis::RedisError),
    #[error("{0}")]
    Config(String),
    #[error("stored data is inconsistent: {0}")]
    Corrupt(String),
}
