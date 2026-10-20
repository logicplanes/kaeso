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

const UNAVAILABLE_CODES: &[&str] = &["57P01", "57P02", "57P03", "53300"];

const CONFLICT_CODES: &[&str] = &["40001", "40P01"];

impl StoreError {
    pub fn is_conflict(&self) -> bool {
        let StoreError::Postgres(sqlx::Error::Database(db)) = self else {
            return false;
        };
        db.code()
            .is_some_and(|code| CONFLICT_CODES.contains(&code.as_ref()))
    }

    pub fn is_unavailable(&self) -> bool {
        match self {
            StoreError::Postgres(e) => postgres_unavailable(e),
            StoreError::ClickHouse(e) => matches!(
                e,
                ::clickhouse::error::Error::Network(_) | ::clickhouse::error::Error::TimedOut
            ),
            StoreError::Valkey(e) => valkey_unavailable(e),
            _ => false,
        }
    }
}

fn postgres_unavailable(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::WorkerCrashed => true,
        sqlx::Error::Database(db) => db.code().is_some_and(|code| {
            code.starts_with("08") || UNAVAILABLE_CODES.contains(&code.as_ref())
        }),
        _ => false,
    }
}

fn valkey_unavailable(e: &redis::RedisError) -> bool {
    use redis::{ErrorKind, ServerErrorKind};
    e.is_io_error()
        || e.is_connection_dropped()
        || e.is_connection_refusal()
        || e.is_timeout()
        || matches!(
            e.kind(),
            ErrorKind::ClusterConnectionNotFound
                | ErrorKind::Server(
                    ServerErrorKind::BusyLoading
                        | ServerErrorKind::MasterDown
                        | ServerErrorKind::ClusterDown
                        | ServerErrorKind::TryAgain
                )
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lost_connections_are_unavailable_and_bad_queries_are_not() {
        assert!(StoreError::Postgres(sqlx::Error::PoolTimedOut).is_unavailable());
        assert!(StoreError::Postgres(sqlx::Error::PoolClosed).is_unavailable());
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert!(StoreError::Postgres(sqlx::Error::Io(refused)).is_unavailable());
        assert!(!StoreError::Postgres(sqlx::Error::RowNotFound).is_unavailable());
        assert!(!StoreError::Corrupt("x".into()).is_unavailable());
        assert!(!StoreError::Config("x".into()).is_unavailable());
    }

    #[test]
    fn clickhouse_outages_are_unavailable_and_bad_queries_are_not() {
        use ::clickhouse::error::Error;
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert!(StoreError::ClickHouse(Error::Network(Box::new(refused))).is_unavailable());
        assert!(StoreError::ClickHouse(Error::TimedOut).is_unavailable());
        assert!(!StoreError::ClickHouse(Error::BadResponse("Code: 62".into())).is_unavailable());
        assert!(!StoreError::ClickHouse(Error::NotEnoughData).is_unavailable());
        assert!(!StoreError::ClickHouseMigrate("x".into()).is_unavailable());
    }

    #[test]
    fn valkey_outages_are_unavailable_and_bad_commands_are_not() {
        use redis::{ErrorKind, RedisError, ServerErrorKind};
        for kind in [
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::TimedOut,
        ] {
            let e = RedisError::from(std::io::Error::from(kind));
            assert!(StoreError::Valkey(e).is_unavailable(), "{kind:?}");
        }
        let loading =
            RedisError::from((ErrorKind::Server(ServerErrorKind::BusyLoading), "loading"));
        assert!(StoreError::Valkey(loading).is_unavailable());
        let wrong_type = RedisError::from((ErrorKind::UnexpectedReturnType, "not a string"));
        assert!(!StoreError::Valkey(wrong_type).is_unavailable());
        let script = RedisError::from((ErrorKind::Server(ServerErrorKind::NoScript), "no script"));
        assert!(!StoreError::Valkey(script).is_unavailable());
        let refused = RedisError::from((ErrorKind::AuthenticationFailed, "wrong password"));
        assert!(!StoreError::Valkey(refused).is_unavailable());
    }
}
