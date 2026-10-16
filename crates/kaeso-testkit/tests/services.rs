use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kaeso_store::clickhouse::ClickHouse;
use kaeso_testkit::services::{
    self, DEV_PREFIX, KEEP_VAR, SWEEP_AGE, TEST_MARK, TEST_PREFIX, TestServices,
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

struct Servers {
    pg: SecretString,
    ch: SecretString,
}

async fn servers(test: &str) -> Option<Servers> {
    TestServices::start().await.cleanup().await;
    if std::env::var(KEEP_VAR).is_ok() {
        eprintln!("SKIPPED {test}: {KEEP_VAR} is set");
        return None;
    }
    Some(Servers {
        pg: services::postgres_url().into(),
        ch: services::clickhouse_url().into(),
    })
}

impl Servers {
    async fn admin(&self) -> PgConnection {
        PgConnection::connect(self.pg.expose_secret())
            .await
            .unwrap()
    }

    fn clickhouse(&self) -> ClickHouse {
        ClickHouse::new(&self.ch, "default").unwrap()
    }

    async fn in_postgres(&self, name: &str) -> bool {
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM pg_database WHERE datname = $1")
            .bind(name)
            .fetch_one(&mut self.admin().await)
            .await
            .unwrap();
        n == 1
    }

    async fn in_clickhouse(&self, name: &str) -> bool {
        self.clickhouse()
            .client()
            .query("SELECT count() FROM system.databases WHERE name = ?")
            .bind(name)
            .fetch_one::<u64>()
            .await
            .unwrap()
            == 1
    }

    async fn create_postgres(&self, name: &str) {
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
            .execute(&mut self.admin().await)
            .await
            .unwrap();
    }

    async fn mark_postgres(&self, name: &str) {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "COMMENT ON DATABASE {name} IS '{TEST_MARK}'"
        )))
        .execute(&mut self.admin().await)
        .await
        .unwrap();
    }

    async fn create_clickhouse(&self, name: &str, marked: bool) {
        let comment = if marked {
            format!(" COMMENT '{TEST_MARK}'")
        } else {
            String::new()
        };
        self.clickhouse()
            .client()
            .query(&format!("CREATE DATABASE {name}{comment}"))
            .execute()
            .await
            .unwrap();
    }

    async fn create(&self, name: &str, marked: bool) {
        self.create_postgres(name).await;
        if marked {
            self.mark_postgres(name).await;
        }
        self.create_clickhouse(name, marked).await;
    }
}

fn name_at(prefix: &str, at: SystemTime) -> String {
    let since = at.duration_since(UNIX_EPOCH).unwrap();
    let ts = uuid::Timestamp::from_unix(uuid::NoContext, since.as_secs(), since.subsec_nanos());
    format!("{prefix}{}", Uuid::new_v7(ts).simple())
}

#[tokio::test]
async fn dropping_test_services_drops_their_databases() {
    let Some(servers) = servers("drop_on_drop").await else {
        return;
    };
    let s = TestServices::start().await;
    let name = s.name().to_owned();
    s.clickhouse.migrate(&s.pg).await.unwrap();
    assert!(servers.in_postgres(&name).await);
    assert!(servers.in_clickhouse(&name).await);
    let held = PgConnection::connect(s.pg_url.expose_secret())
        .await
        .unwrap();

    drop(s);

    assert!(
        !servers.in_postgres(&name).await,
        "{name} is still in PostgreSQL"
    );
    assert!(
        !servers.in_clickhouse(&name).await,
        "{name} is still in ClickHouse"
    );
    drop(held);
}

#[tokio::test]
async fn cleanup_drops_the_databases() {
    let Some(servers) = servers("drop_on_cleanup").await else {
        return;
    };
    let s = TestServices::start().await;
    let name = s.name().to_owned();
    s.cleanup().await;
    assert!(!servers.in_postgres(&name).await);
    assert!(!servers.in_clickhouse(&name).await);
}

#[tokio::test]
async fn a_test_that_panics_still_drops_its_databases() {
    let Some(servers) = servers("drop_on_panic").await else {
        return;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let outcome = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let s = TestServices::start().await;
                tx.send(s.name().to_owned()).unwrap();
                panic!("the test fails with {} alive", s.name());
            })
    })
    .join();
    assert!(outcome.is_err());
    let name = rx.recv().unwrap();
    assert!(
        !servers.in_postgres(&name).await,
        "{name} is still in PostgreSQL"
    );
    assert!(
        !servers.in_clickhouse(&name).await,
        "{name} is still in ClickHouse"
    );
}

#[tokio::test]
async fn the_sweep_drops_only_stale_unused_test_databases() {
    let Some(servers) = servers("sweep").await else {
        return;
    };
    let now = SystemTime::now();
    let old = now - SWEEP_AGE - Duration::from_secs(60);
    let stale = name_at(TEST_PREFIX, old);
    let stale_clickhouse_only = name_at(TEST_PREFIX, old);
    let connected = name_at(TEST_PREFIX, old);
    let recent = name_at(TEST_PREFIX, now - Duration::from_secs(60));
    let unmarked = name_at(TEST_PREFIX, old);
    let development = name_at(DEV_PREFIX, old);
    servers.create(&stale, true).await;
    servers
        .create_clickhouse(&stale_clickhouse_only, true)
        .await;
    servers.create(&recent, true).await;
    servers.create(&unmarked, false).await;
    servers.create(&development, false).await;
    servers.create_postgres(&connected).await;
    let mut url = url::Url::parse(servers.pg.expose_secret()).unwrap();
    url.set_path(&connected);
    let held = PgConnection::connect(url.as_str()).await.unwrap();
    servers.mark_postgres(&connected).await;
    servers.create_clickhouse(&connected, true).await;

    let swept = services::sweep(&servers.pg, &servers.ch, now, SWEEP_AGE).await;

    let mut seen = Vec::new();
    for name in [
        &stale,
        &stale_clickhouse_only,
        &connected,
        &recent,
        &unmarked,
        &development,
    ] {
        seen.push((
            name.clone(),
            servers.in_postgres(name).await,
            servers.in_clickhouse(name).await,
        ));
    }
    drop(held);
    for name in [&connected, &recent, &unmarked, &development] {
        services::drop_databases(&servers.pg, &servers.ch, name)
            .await
            .unwrap();
    }

    let swept = swept.unwrap();
    assert!(
        swept
            .iter()
            .all(|n| [&stale, &stale_clickhouse_only].contains(&n)),
        "{swept:?}"
    );
    let expected = [
        (stale, false, false),
        (stale_clickhouse_only, false, false),
        (connected, true, true),
        (recent, true, true),
        (unmarked, true, true),
        (development, true, true),
    ];
    assert_eq!(seen, expected);
}

#[tokio::test]
async fn drop_services_refuses_other_databases() {
    let Some(servers) = servers("drop_refuses").await else {
        return;
    };
    for name in ["kaeso", "default", "t_x; DROP DATABASE kaeso", "system"] {
        assert!(
            services::drop_databases(&servers.pg, &servers.ch, name)
                .await
                .is_err(),
            "{name}"
        );
    }
    assert!(servers.in_postgres("kaeso").await);
}
