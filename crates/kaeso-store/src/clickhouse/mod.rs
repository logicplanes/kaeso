pub mod logs;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sqlx::PgPool;
use url::Url;

use crate::StoreError;
use crate::pg::{locks, with_advisory_lock};

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "request_log",
        sql: include_str!("../../migrations/clickhouse/0001_request_log.sql"),
    },
    Migration {
        version: 2,
        name: "usage_rollups",
        sql: include_str!("../../migrations/clickhouse/0002_usage_rollups.sql"),
    },
    Migration {
        version: 3,
        name: "gateway_overhead",
        sql: include_str!("../../migrations/clickhouse/0003_gateway_overhead.sql"),
    },
    Migration {
        version: 4,
        name: "cache_by_route",
        sql: include_str!("../../migrations/clickhouse/0004_cache_by_route.sql"),
    },
    Migration {
        version: 5,
        name: "request_upload",
        sql: include_str!("../../migrations/clickhouse/0005_request_upload.sql"),
    },
];

#[derive(Clone)]
pub struct ClickHouse {
    client: clickhouse::Client,
    database: String,
}

impl std::fmt::Debug for ClickHouse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClickHouse")
            .field("database", &self.database)
            .finish_non_exhaustive()
    }
}

impl ClickHouse {
    pub fn new(url: &SecretString, database: &str) -> Result<ClickHouse, StoreError> {
        let mut parsed = Url::parse(url.expose_secret())
            .map_err(|_| StoreError::Config("KAESO_CLICKHOUSE_URL is not a valid URL".into()))?;
        let user = percent_decode(parsed.username());
        let password = parsed.password().map(percent_decode);
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        let mut client = clickhouse::Client::default()
            .with_url(parsed.as_str().trim_end_matches('/'))
            .with_database(database)
            .with_setting("wait_end_of_query", "1");
        if !user.is_empty() {
            client = client.with_user(user);
        }
        if let Some(password) = password {
            client = client.with_password(password);
        }
        Ok(ClickHouse {
            client,
            database: database.to_owned(),
        })
    }

    pub fn client(&self) -> &clickhouse::Client {
        &self.client
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        self.client.query("SELECT 1").execute().await?;
        Ok(())
    }

    pub async fn migrate(&self, pg: &PgPool) -> Result<(), StoreError> {
        with_advisory_lock(pg, locks::CLICKHOUSE_MIGRATIONS, || self.migrate_locked()).await
    }

    async fn migrate_locked(&self) -> Result<(), StoreError> {
        let server = self.client.clone().with_database("default");
        server
            .query("CREATE DATABASE IF NOT EXISTS ?")
            .bind(clickhouse::sql::Identifier(&self.database))
            .execute()
            .await?;
        self.client
            .query(
                "CREATE TABLE IF NOT EXISTS kaeso_migrations \
                 (version UInt32, name String, applied_at DateTime('UTC') DEFAULT now()) \
                 ENGINE = MergeTree ORDER BY version",
            )
            .execute()
            .await?;
        #[derive(clickhouse::Row, Deserialize)]
        struct Applied {
            version: u32,
        }
        let applied: Vec<u32> = self
            .client
            .query("SELECT version FROM kaeso_migrations ORDER BY version")
            .fetch_all::<Applied>()
            .await?
            .into_iter()
            .map(|a| a.version)
            .collect();
        if let Some(unknown) = applied
            .iter()
            .find(|v| !MIGRATIONS.iter().any(|m| m.version == **v))
        {
            return Err(StoreError::ClickHouseMigrate(format!(
                "the database was migrated by a newer version of Kaeso (migration {unknown} is unknown to this version); run that version or newer"
            )));
        }
        for migration in MIGRATIONS.iter().filter(|m| !applied.contains(&m.version)) {
            for statement in statements(migration.sql) {
                self.client.query(&statement).execute().await.map_err(|e| {
                    StoreError::ClickHouseMigrate(format!(
                        "migration {} ({}) failed: {e}",
                        migration.version, migration.name
                    ))
                })?;
            }
            self.client
                .query("INSERT INTO kaeso_migrations (version, name) VALUES (?, ?)")
                .bind(migration.version)
                .bind(migration.name)
                .execute()
                .await?;
            tracing::info!(
                version = migration.version,
                name = migration.name,
                "applied ClickHouse migration"
            );
        }
        Ok(())
    }
}

fn statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in sql.lines() {
        let trimmed = line.trim_end();
        if let Some(body) = trimmed.strip_suffix(';') {
            current.push_str(body);
            if !current.trim().is_empty() {
                out.push(std::mem::take(&mut current).trim().to_owned());
            }
            current.clear();
        } else {
            current.push_str(trimmed);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_owned());
    }
    out
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_files_split_into_statements() {
        let all: Vec<String> = MIGRATIONS.iter().flat_map(|m| statements(m.sql)).collect();
        assert_eq!(all.len(), 10);
        assert!(
            all.iter()
                .all(|s| s.starts_with("CREATE ") || s.starts_with("ALTER TABLE "))
        );
        assert!(all.iter().all(|s| !s.contains("--")));
        assert!(
            all.iter()
                .all(|s| s.contains(" IF NOT EXISTS ") || s.contains(" MODIFY QUERY")),
            "every statement can run again after a migration stopped halfway"
        );
    }

    #[test]
    fn versions_are_ordered_and_unique() {
        let versions: Vec<u32> = MIGRATIONS.iter().map(|m| m.version).collect();
        let mut sorted = versions.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(versions, sorted);
    }
}
