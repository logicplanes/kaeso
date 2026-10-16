use std::time::Duration;

use kaeso_store::StoreError;
use kaeso_store::valkey::Invalidation;
use kaeso_testkit::services::TestServices;

#[tokio::test]
async fn postgres_migrations_are_idempotent_and_refuse_newer_databases() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&s.pg)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (99991231000000, 'from the future', true, '\\x00', 0)",
    )
    .execute(&s.pg)
    .await
    .unwrap();
    let err = kaeso_store::pg::migrate(&s.pg).await.unwrap_err();
    assert!(err.to_string().contains("newer version of Kaeso"), "{err}");
    s.cleanup().await;
}

#[tokio::test]
async fn clickhouse_migrations_create_the_log_tables_once() {
    let s = TestServices::start().await;
    s.clickhouse.migrate(&s.pg).await.unwrap();
    s.clickhouse.migrate(&s.pg).await.unwrap();
    #[derive(clickhouse::Row, serde::Deserialize)]
    struct Name {
        name: String,
    }
    let tables: Vec<String> = s
        .clickhouse
        .client()
        .query("SELECT name FROM system.tables WHERE database = currentDatabase() ORDER BY name")
        .fetch_all::<Name>()
        .await
        .unwrap()
        .into_iter()
        .map(|n| n.name)
        .collect();
    assert_eq!(
        tables,
        [
            "kaeso_migrations",
            "request_content",
            "request_log",
            "usage_daily",
            "usage_daily_mv",
            "usage_hourly",
            "usage_hourly_mv"
        ]
    );
    s.clickhouse
        .client()
        .query("INSERT INTO kaeso_migrations (version, name) VALUES (999, 'future')")
        .execute()
        .await
        .unwrap();
    let err = s.clickhouse.migrate(&s.pg).await.unwrap_err();
    assert!(matches!(err, StoreError::ClickHouseMigrate(_)), "{err}");
    s.cleanup().await;
}

#[tokio::test]
async fn an_abandoned_lock_holder_releases_the_lock() {
    let s = TestServices::start().await;
    let pool = kaeso_store::pg::connect(&s.pg_url, 1).unwrap();
    let id = kaeso_store::pg::locks::JOBS_BASE + 99;
    let mut held = Box::pin(kaeso_store::pg::with_advisory_lock(&pool, id, || {
        std::future::pending::<Result<(), StoreError>>()
    }));
    let started = tokio::time::timeout(Duration::from_millis(300), &mut held).await;
    assert!(started.is_err());
    let busy = kaeso_store::pg::try_with_advisory_lock(&s.pg, id, || async { Ok(()) })
        .await
        .unwrap();
    assert_eq!(busy, None);
    drop(held);
    kaeso_store::pg::ping(&pool).await.unwrap();
    let free = kaeso_store::pg::try_with_advisory_lock(&s.pg, id, || async { Ok(()) })
        .await
        .unwrap();
    assert_eq!(free, Some(()));
    let failed = kaeso_store::pg::with_advisory_lock(&pool, id, || async {
        Err::<(), _>(StoreError::Config("work failed".into()))
    })
    .await;
    assert!(failed.is_err());
    kaeso_store::pg::ping(&pool).await.unwrap();
    let after_failure = kaeso_store::pg::try_with_advisory_lock(&s.pg, id, || async { Ok(()) })
        .await
        .unwrap();
    assert_eq!(after_failure, Some(()));
    pool.close().await;
    s.cleanup().await;
}

#[tokio::test]
async fn concurrent_migrations_serialize_on_the_lock() {
    let s = TestServices::start().await;
    let (a, b) = tokio::join!(s.clickhouse.migrate(&s.pg), s.clickhouse.migrate(&s.pg));
    a.unwrap();
    b.unwrap();
    #[derive(clickhouse::Row, serde::Deserialize)]
    struct Count {
        n: u64,
    }
    let applied = s
        .clickhouse
        .client()
        .query("SELECT count() AS n FROM kaeso_migrations")
        .fetch_one::<Count>()
        .await
        .unwrap();
    let files = std::fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/clickhouse"
    ))
    .unwrap()
    .filter(|e| {
        e.as_ref()
            .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "sql"))
    })
    .count();
    assert_eq!(
        applied.n, files as u64,
        "each migration is applied exactly once"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn invalidations_reach_every_subscriber() {
    let s = TestServices::start().await;
    s.valkey.ping().await.unwrap();
    let mut a = s.valkey.subscribe_invalidations();
    let mut b = s.valkey.subscribe_invalidations();
    assert_eq!(a.recv().await, Some(Invalidation::Connected));
    assert_eq!(b.recv().await, Some(Invalidation::Connected));
    s.valkey.publish_invalidation("key:abc").await.unwrap();
    let wait = Duration::from_secs(5);
    let message = Some(Invalidation::Message("key:abc".into()));
    assert_eq!(tokio::time::timeout(wait, a.recv()).await.unwrap(), message);
    assert_eq!(tokio::time::timeout(wait, b.recv()).await.unwrap(), message);
    s.cleanup().await;
}
