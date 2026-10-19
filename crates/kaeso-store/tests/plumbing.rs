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
async fn provider_columns_accept_exactly_the_provider_id_rule() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let checks: Vec<(String, String)> = sqlx::query_as(
        "SELECT conrelid::regclass::text, pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE contype = 'c' AND pg_get_constraintdef(oid) LIKE '%provider%' ORDER BY 1",
    )
    .fetch_all(&s.pg)
    .await
    .unwrap();
    let pattern = kaeso_core::provider::ID_PATTERN;
    assert_eq!(
        checks
            .iter()
            .map(|(table, _)| table.as_str())
            .collect::<Vec<_>>(),
        ["catalogue_models", "provider_credentials", "usage_daily"]
    );
    for (table, check) in &checks {
        assert!(
            check.contains(&format!("provider ~ '{pattern}'::text")),
            "{table}: {check}"
        );
    }
    for id in [
        "openai",
        "azure-openai",
        "302ai",
        "",
        "-",
        "azure_openai",
        "azure--openai",
        "open.ai",
        "OpenAI",
        "openai-",
        "ö",
    ] {
        let (matches,): (bool,) = sqlx::query_as("SELECT $1 ~ $2")
            .bind(id)
            .bind(pattern)
            .fetch_one(&s.pg)
            .await
            .unwrap();
        assert_eq!(
            matches,
            kaeso_core::provider::ProviderId::is_valid(id),
            "{id:?}"
        );
    }
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
            "usage_daily_mv"
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
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = entered.clone();
    let mut held = Box::pin(kaeso_store::pg::with_advisory_lock(&pool, id, move || {
        signal.notify_one();
        std::future::pending::<Result<(), StoreError>>()
    }));
    tokio::select! {
        _ = &mut held => panic!("the lock holder finished"),
        () = entered.notified() => {}
    }
    let busy = kaeso_store::pg::try_with_advisory_lock(&s.pg, id, || async { Ok(()) })
        .await
        .unwrap();
    assert_eq!(busy, None);
    drop(held);
    tokio::time::timeout(
        Duration::from_secs(10),
        kaeso_store::pg::with_advisory_lock(&s.pg, id, || async { Ok(()) }),
    )
    .await
    .unwrap()
    .unwrap();
    let failed = kaeso_store::pg::with_advisory_lock(&pool, id, || async {
        Err::<(), _>(StoreError::Config("work failed".into()))
    })
    .await;
    assert!(
        matches!(&failed, Err(StoreError::Config(m)) if m == "work failed"),
        "{failed:?}"
    );
    kaeso_store::pg::ping(&pool).await.unwrap();
    let after_failure = kaeso_store::pg::try_with_advisory_lock(&s.pg, id, || async { Ok(()) })
        .await
        .unwrap();
    assert_eq!(after_failure, Some(()));
    pool.close().await;
    s.cleanup().await;
}

#[tokio::test]
async fn work_under_a_lock_can_use_the_whole_pool() {
    let s = TestServices::start().await;
    let pool = kaeso_store::pg::connect(&s.pg_url, 1).unwrap();
    let id = kaeso_store::pg::locks::JOBS_BASE + 98;
    kaeso_store::pg::with_advisory_lock(&pool, id, || kaeso_store::pg::ping(&pool))
        .await
        .unwrap();
    let ran = kaeso_store::pg::try_with_advisory_lock(&pool, id, || kaeso_store::pg::ping(&pool))
        .await
        .unwrap();
    assert_eq!(ran, Some(()));
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

async fn client_id(mut conn: redis::aio::ConnectionManager) -> redis::RedisResult<i64> {
    redis::cmd("CLIENT").arg("ID").query_async(&mut conn).await
}

#[tokio::test]
async fn bulk_traffic_takes_turns_on_its_own_connections() {
    let s = TestServices::start().await;
    let latency = client_id(s.valkey.conn()).await.unwrap();
    assert_eq!(client_id(s.valkey.conn()).await.unwrap(), latency);
    let first = client_id(s.valkey.bulk()).await.unwrap();
    let second = client_id(s.valkey.bulk()).await.unwrap();
    assert_ne!(first, second);
    assert!(![first, second].contains(&latency));
    assert_eq!(client_id(s.valkey.bulk()).await.unwrap(), first);
    assert_eq!(client_id(s.valkey.bulk()).await.unwrap(), second);
    s.cleanup().await;
}

#[tokio::test]
async fn a_command_is_answered_while_a_bulk_connection_is_busy() {
    let s = TestServices::start().await;
    let list = s.valkey.key("busy");
    let mut bulk = s.valkey.bulk();
    let busy_id = client_id(bulk.clone()).await.unwrap();
    let waiting = {
        let list = list.clone();
        tokio::spawn(async move {
            redis::cmd("BLPOP")
                .arg(&list)
                .arg(0.9)
                .query_async::<Option<(String, String)>>(&mut bulk)
                .await
        })
    };
    let mut conn = s.valkey.conn();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let clients: String = redis::cmd("CLIENT")
            .arg("LIST")
            .arg("ID")
            .arg(busy_id)
            .query_async(&mut conn)
            .await
            .unwrap();
        if clients.contains("cmd=blpop") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "BLPOP never started"
        );
        tokio::task::yield_now().await;
    }
    let _: i64 = redis::cmd("RPUSH")
        .arg(&list)
        .arg("done")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(waiting.await.unwrap().is_ok());
    let _: i64 = redis::cmd("DEL")
        .arg(&list)
        .query_async(&mut conn)
        .await
        .unwrap();
    s.cleanup().await;
}

#[tokio::test]
async fn an_sso_domain_signs_in_through_one_organization() {
    use kaeso_store::pg::oidc::{self, Connected, Upserted};
    let s = TestServices::start().await;
    let (first, second) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    for org in [first, second] {
        kaeso_store::pg::orgs::insert(&s.pg, org, "Acme", 30, 30)
            .await
            .unwrap();
    }
    let connection = |org_id| oidc::Connection {
        org_id,
        issuer: "https://idp.example.com".into(),
        client_id: "kaeso".into(),
        secret_ciphertext: vec![1],
        secret_nonce: vec![2],
        wrapped_key: vec![3],
        key_id: "k1".into(),
        domains: vec!["acme.example".into()],
        display_name: Some("Acme ID".into()),
        auto_redirect: true,
    };
    let mut conn = s.pg.acquire().await.unwrap();
    assert_eq!(
        oidc::connected(&mut *conn).await.unwrap(),
        Connected::Nowhere
    );
    assert_eq!(
        oidc::upsert(&mut conn, &connection(first)).await.unwrap(),
        Upserted::Saved
    );
    assert_eq!(
        oidc::connected(&mut *conn).await.unwrap(),
        Connected::Only(first)
    );
    let stored = oidc::get(&mut *conn, first).await.unwrap().unwrap();
    assert_eq!(stored.display_name.as_deref(), Some("Acme ID"));
    assert!(stored.auto_redirect);
    assert_eq!(
        oidc::upsert(&mut conn, &connection(second)).await.unwrap(),
        Upserted::DomainTaken("acme.example".into())
    );
    assert_eq!(
        oidc::connected(&mut *conn).await.unwrap(),
        Connected::Several
    );
    drop(conn);
    s.cleanup().await;
}
