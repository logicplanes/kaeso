use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clickhouse::sql::Identifier;
use kaeso_store::clickhouse::ClickHouse;
use kaeso_store::valkey::Valkey;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

pub const POSTGRES_VAR: &str = "KAESO_TEST_POSTGRES_URL";
pub const CLICKHOUSE_VAR: &str = "KAESO_TEST_CLICKHOUSE_URL";
pub const VALKEY_VAR: &str = "KAESO_TEST_VALKEY_URL";
pub const KEEP_VAR: &str = "KAESO_TEST_KEEP_DATABASES";

pub const DEFAULT_POSTGRES_URL: &str = "postgres://kaeso:kaeso@127.0.0.1:55432/kaeso";
pub const DEFAULT_CLICKHOUSE_URL: &str = "http://kaeso:kaeso@127.0.0.1:58123";
pub const DEFAULT_VALKEY_URL: &str = "redis://127.0.0.1:56379";

pub const TEST_PREFIX: &str = "t_";
pub const DEV_PREFIX: &str = "dev_";
pub const TEST_MARK: &str = "kaeso-testkit: test database";
pub const SWEEP_AGE: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Test,
    Development,
}

impl Kind {
    fn prefix(self) -> &'static str {
        match self {
            Kind::Test => TEST_PREFIX,
            Kind::Development => DEV_PREFIX,
        }
    }
}

#[derive(Debug)]
pub struct ServicesError(String);

impl std::fmt::Display for ServicesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ServicesError {}

fn fail<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> ServicesError + '_ {
    move |e| ServicesError(format!("{what}: {e}"))
}

#[derive(Debug)]
pub struct TestServices {
    pub pg: PgPool,
    pub pg_url: SecretString,
    pub clickhouse: ClickHouse,
    pub clickhouse_url: SecretString,
    pub clickhouse_database: String,
    pub valkey: Valkey,
    pub valkey_url: SecretString,
    pub valkey_prefix: String,
    databases: Databases,
}

impl TestServices {
    pub async fn start() -> TestServices {
        create(Kind::Test, urls(), true).await
    }

    pub async fn start_detached(kind: Kind) -> TestServices {
        create(kind, urls(), false).await
    }

    pub fn name(&self) -> &str {
        &self.databases.name
    }

    pub async fn cleanup(self) {
        let TestServices { pg, databases, .. } = self;
        pg.close().await;
        databases.drop_now().await;
    }
}

pub fn postgres_url() -> String {
    std::env::var(POSTGRES_VAR).unwrap_or_else(|_| DEFAULT_POSTGRES_URL.to_owned())
}

pub fn clickhouse_url() -> String {
    std::env::var(CLICKHOUSE_VAR).unwrap_or_else(|_| DEFAULT_CLICKHOUSE_URL.to_owned())
}

pub fn valkey_url() -> String {
    std::env::var(VALKEY_VAR).unwrap_or_else(|_| DEFAULT_VALKEY_URL.to_owned())
}

fn urls() -> (String, String, String) {
    (postgres_url(), clickhouse_url(), valkey_url())
}

static SWEPT: AtomicBool = AtomicBool::new(false);
async fn create(kind: Kind, (pg, ch, vk): (String, String, String), owned: bool) -> TestServices {
    let admin_url = SecretString::from(pg);
    let clickhouse_url = SecretString::from(ch);
    if !SWEPT.swap(true, Ordering::SeqCst) {
        match sweep(&admin_url, &clickhouse_url, SystemTime::now(), SWEEP_AGE).await {
            Ok(dropped) if !dropped.is_empty() => eprintln!(
                "kaeso-testkit: dropped {} unused test databases older than {} hours",
                dropped.len(),
                SWEEP_AGE.as_secs() / 3600
            ),
            Ok(_) => {}
            Err(e) => eprintln!("kaeso-testkit: could not sweep stale test databases: {e}"),
        }
    }

    let databases = Databases {
        name: format!("{}{}", kind.prefix(), Uuid::now_v7().simple()),
        admin_url,
        clickhouse_url,
        owned,
        dropped: false,
    };
    let name = databases.name.clone();

    let mut admin = PgConnection::connect(databases.admin_url.expose_secret())
        .await
        .expect("connect to KAESO_TEST_POSTGRES_URL");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&mut admin)
        .await
        .expect("create a test database");
    if kind == Kind::Test {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "COMMENT ON DATABASE {name} IS '{TEST_MARK}'"
        )))
        .execute(&mut admin)
        .await
        .expect("mark the test database");
    }
    let _ = admin.close().await;
    let mut url = url::Url::parse(databases.admin_url.expose_secret())
        .expect("KAESO_TEST_POSTGRES_URL is a URL");
    url.set_path(&name);
    let pg_url = SecretString::from(url.to_string());
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(url.as_str())
        .await
        .expect("connect to the test database");
    kaeso_store::pg::migrate(&pool)
        .await
        .expect("migrate the test database");

    let server = ClickHouse::new(&databases.clickhouse_url, "default").expect("ClickHouse client");
    let comment = match kind {
        Kind::Test => format!(" COMMENT '{TEST_MARK}'"),
        Kind::Development => String::new(),
    };
    server
        .client()
        .query(&format!("CREATE DATABASE ?{comment}"))
        .bind(Identifier(&name))
        .execute()
        .await
        .expect("create a test ClickHouse database");
    let clickhouse = ClickHouse::new(&databases.clickhouse_url, &name).expect("ClickHouse client");
    let valkey_url = SecretString::from(vk);
    let valkey_prefix = format!("kaeso:{name}:");
    let valkey = Valkey::new(&valkey_url, &valkey_prefix).expect("Valkey client");
    TestServices {
        pg: pool,
        pg_url,
        clickhouse,
        clickhouse_url: databases.clickhouse_url.clone(),
        clickhouse_database: name,
        valkey,
        valkey_url,
        valkey_prefix,
        databases,
    }
}

#[derive(Debug)]
struct Databases {
    name: String,
    admin_url: SecretString,
    clickhouse_url: SecretString,
    owned: bool,
    dropped: bool,
}

impl Databases {
    fn keep(&self) -> bool {
        !self.owned || self.dropped || keep_requested(&self.name)
    }
    async fn drop_now(mut self) {
        if !self.keep()
            && let Err(e) = drop_databases(&self.admin_url, &self.clickhouse_url, &self.name).await
        {
            eprintln!("kaeso-testkit: {e}");
        }
        self.dropped = true;
    }
}

impl Drop for Databases {
    fn drop(&mut self) {
        if self.keep() {
            return;
        }
        self.dropped = true;
        let name = self.name.clone();
        let admin_url = self.admin_url.clone();
        let clickhouse_url = self.clickhouse_url.clone();
        let worker = std::thread::Builder::new()
            .name("kaeso-testkit-drop".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(fail("start a runtime to drop the test databases"))?;
                runtime.block_on(drop_databases(&admin_url, &clickhouse_url, &name))
            });
        let outcome = match worker {
            Ok(worker) => worker.join().unwrap_or_else(|_| {
                Err(ServicesError("dropping the test databases panicked".into()))
            }),
            Err(e) => Err(fail("start a thread to drop the test databases")(e)),
        };
        if let Err(e) = outcome {
            eprintln!("kaeso-testkit: {e}");
        }
    }
}
fn keep_requested(name: &str) -> bool {
    let keep = std::env::var(KEEP_VAR).is_ok_and(|v| !v.is_empty() && v != "0");
    if keep {
        eprintln!("kaeso-testkit: kept the test databases {name} ({KEEP_VAR} is set)");
    }
    keep
}

pub fn is_services_name(name: &str) -> bool {
    let hex = name
        .strip_prefix(TEST_PREFIX)
        .or_else(|| name.strip_prefix(DEV_PREFIX));
    hex.is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

pub fn created_at(name: &str) -> Option<SystemTime> {
    if !is_services_name(name) {
        return None;
    }
    let id = Uuid::try_parse(name.strip_prefix(TEST_PREFIX)?).ok()?;
    if id.get_version_num() != 7 {
        return None;
    }
    let (secs, nanos) = id.get_timestamp()?.to_unix();
    UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}

pub fn is_stale(name: &str, now: SystemTime, max_age: Duration) -> bool {
    created_at(name)
        .and_then(|at| now.duration_since(at).ok())
        .is_some_and(|age| age >= max_age)
}

pub async fn drop_databases(
    admin_url: &SecretString,
    clickhouse_url: &SecretString,
    name: &str,
) -> Result<(), ServicesError> {
    if !is_services_name(name) {
        return Err(ServicesError(format!(
            "refusing to drop {name:?}: not a test kit database name ({TEST_PREFIX}… or {DEV_PREFIX}…)"
        )));
    }
    let pg = async {
        let mut admin = PgConnection::connect(admin_url.expose_secret())
            .await
            .map_err(fail("connect to KAESO_TEST_POSTGRES_URL"))?;
        let dropped = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS {name} WITH (FORCE)"
        )))
        .execute(&mut admin)
        .await
        .map_err(|e| ServicesError(format!("drop the PostgreSQL database {name}: {e}")));
        let _ = admin.close().await;
        dropped.map(|_| ())
    };
    let pg = pg.await;
    let ch = drop_clickhouse(clickhouse_url, name).await;
    pg.and(ch)
}

async fn drop_clickhouse(clickhouse_url: &SecretString, name: &str) -> Result<(), ServicesError> {
    let server =
        ClickHouse::new(clickhouse_url, "default").map_err(fail("KAESO_TEST_CLICKHOUSE_URL"))?;
    server
        .client()
        .query("DROP DATABASE IF EXISTS ? SYNC")
        .bind(Identifier(name))
        .execute()
        .await
        .map_err(|e| ServicesError(format!("drop the ClickHouse database {name}: {e}")))
}

pub async fn sweep(
    admin_url: &SecretString,
    clickhouse_url: &SecretString,
    now: SystemTime,
    max_age: Duration,
) -> Result<Vec<String>, ServicesError> {
    let mut dropped = Vec::new();
    let mut admin = PgConnection::connect(admin_url.expose_secret())
        .await
        .map_err(fail("connect to KAESO_TEST_POSTGRES_URL"))?;
    let idle: Vec<(String,)> = sqlx::query_as(
        "SELECT d.datname::text FROM pg_database d \
         WHERE d.datname ~ '^t_[0-9a-f]{32}$' \
           AND shobj_description(d.oid, 'pg_database') = $1 \
           AND NOT EXISTS (SELECT 1 FROM pg_stat_activity a WHERE a.datid = d.oid)",
    )
    .bind(TEST_MARK)
    .fetch_all(&mut admin)
    .await
    .map_err(fail("list the test databases in PostgreSQL"))?;
    for (name,) in idle {
        if !is_stale(&name, now, max_age) {
            continue;
        }
        let drop = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS {name}"
        )))
        .execute(&mut admin)
        .await;
        if drop.is_ok() {
            dropped.push(name);
        }
    }
    let remaining: HashSet<String> = sqlx::query_as::<_, (String,)>(
        "SELECT datname::text FROM pg_database WHERE datname ~ '^t_'",
    )
    .fetch_all(&mut admin)
    .await
    .map_err(fail("list the test databases in PostgreSQL"))?
    .into_iter()
    .map(|(name,)| name)
    .collect();
    let _ = admin.close().await;

    #[derive(clickhouse::Row, Deserialize)]
    struct Name {
        name: String,
    }
    let server =
        ClickHouse::new(clickhouse_url, "default").map_err(fail("KAESO_TEST_CLICKHOUSE_URL"))?;
    let marked = server
        .client()
        .query("SELECT name FROM system.databases WHERE match(name, '^t_[0-9a-f]{32}$') AND comment = ?")
        .bind(TEST_MARK)
        .fetch_all::<Name>()
        .await
        .map_err(fail("list the test databases in ClickHouse"))?;
    for Name { name } in marked {
        if remaining.contains(&name) || !is_stale(&name, now, max_age) {
            continue;
        }
        if drop_clickhouse(clickhouse_url, &name).await.is_ok() && !dropped.contains(&name) {
            dropped.push(name);
        }
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {

    use super::*;

    fn test_name(at: SystemTime) -> String {
        let since = at.duration_since(UNIX_EPOCH).unwrap();
        let ts = uuid::Timestamp::from_unix(uuid::NoContext, since.as_secs(), since.subsec_nanos());
        format!("{TEST_PREFIX}{}", Uuid::new_v7(ts).simple())
    }

    #[test]
    fn the_creation_time_comes_from_the_v7_uuid() {
        let now = SystemTime::now();
        let name = test_name(now - Duration::from_secs(7 * 3600));
        assert!(is_stale(&name, now, SWEEP_AGE));
        assert!(!is_stale(
            &name,
            now - Duration::from_secs(2 * 3600),
            SWEEP_AGE
        ));
        let fresh = format!("{TEST_PREFIX}{}", Uuid::now_v7().simple());
        assert!(!is_stale(&fresh, SystemTime::now(), SWEEP_AGE));
    }

    #[test]
    fn only_test_kit_names_can_be_stale() {
        let long_after = SystemTime::now() + Duration::from_secs(365 * 24 * 3600);
        let old = test_name(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
        assert!(is_stale(&old, long_after, SWEEP_AGE));
        let hex = old.strip_prefix(TEST_PREFIX).unwrap();
        for name in [
            format!("{DEV_PREFIX}{hex}"),
            format!("x_{hex}"),
            hex.to_uppercase(),
            format!("{TEST_PREFIX}550e8400e29b41d4a716446655440000"),
            format!("{TEST_PREFIX}{hex}; DROP DATABASE kaeso"),
            "kaeso".into(),
            "t_".into(),
        ] {
            assert!(!is_stale(&name, long_after, SWEEP_AGE), "{name}");
        }
        assert!(is_services_name(&format!("{DEV_PREFIX}{hex}")));
        assert!(!is_services_name("kaeso"));
        assert!(!is_services_name(&format!(
            "{TEST_PREFIX}{hex} WITH (FORCE)"
        )));
    }
}
