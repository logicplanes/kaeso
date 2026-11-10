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
async fn migrating_an_unreachable_postgres_is_an_outage_to_retry() {
    let url = secrecy::SecretString::from("postgres://kaeso:kaeso@127.0.0.1:1/kaeso");
    let pool = kaeso_store::pg::connect(&url, 1).unwrap();
    let err = kaeso_store::pg::migrate(&pool).await.unwrap_err();
    assert!(matches!(err, StoreError::Postgres(_)), "{err:?}");
    assert!(err.is_unavailable(), "{err:?}");
}

#[tokio::test]
async fn provider_columns_accept_exactly_the_provider_id_rule() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let checks: Vec<(String, String)> = sqlx::query_as(
        "SELECT conrelid::regclass::text, pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE contype = 'c' AND pg_get_constraintdef(oid) LIKE '%provider ~%' ORDER BY 1",
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
        [
            "catalogue_models",
            "custom_providers",
            "image_generations",
            "provider_credentials",
            "usage_daily",
            "videos"
        ]
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
            "tool_call_content",
            "tool_calls"
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
async fn timings_logged_in_whole_milliseconds_read_as_microseconds_without_rewriting_them() {
    let s = TestServices::start().await;
    let ch = &s.clickhouse;
    ch.migrate(&s.pg).await.unwrap();
    for sql in [
        "ALTER TABLE request_log DROP COLUMN overhead_us, DROP COLUMN stream_processing_us, DROP COLUMN request_upload_us",
        "DELETE FROM kaeso_migrations WHERE version = 13",
    ] {
        ch.client().query(sql).execute().await.unwrap();
    }
    let (org, project) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let ids = [uuid::Uuid::now_v7(), uuid::Uuid::now_v7()];
    for (id, timings) in ids.iter().zip(["3, 0, 2", "0, NULL, 0"]) {
        ch.client()
            .query(&format!(
                "INSERT INTO request_log (request_id, org_id, project_id, started_at, finished_at, \
                 expires_at, overhead_ms, request_upload_ms, stream_processing_ms) \
                 VALUES (?, ?, ?, now64(3), now64(3), now() + INTERVAL 1 DAY, {timings})"
            ))
            .bind(*id)
            .bind(org)
            .bind(project)
            .execute()
            .await
            .unwrap();
    }
    let mutations = || async {
        #[derive(clickhouse::Row, serde::Deserialize)]
        struct Count {
            n: u64,
        }
        ch.client()
            .query("SELECT count() AS n FROM system.mutations WHERE database = currentDatabase() AND table = 'request_log'")
            .fetch_one::<Count>()
            .await
            .unwrap()
            .n
    };
    let before = mutations().await;
    ch.migrate(&s.pg).await.unwrap();
    assert_eq!(
        mutations().await,
        before,
        "the old rows are read as they are"
    );

    let read = |id| kaeso_store::clickhouse::logs::get(ch, org, project, id);
    let measured = read(ids[0]).await.unwrap().unwrap();
    assert_eq!(measured.overhead_us, Some(3000));
    assert_eq!(measured.stream_processing_us, Some(2000));
    assert_eq!(
        measured.request_upload_us, None,
        "0 ms meant under a millisecond, which is unknown in microseconds"
    );
    let under_a_millisecond = read(ids[1]).await.unwrap().unwrap();
    assert_eq!(under_a_millisecond.overhead_us, None);
    assert_eq!(under_a_millisecond.request_upload_us, None);
    assert_eq!(under_a_millisecond.stream_processing_us, None);
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
    use kaeso_store::pg::oidc::{self, Upserted};
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
        verified_domains: Vec::new(),
        verification_token: "token".into(),
        display_name: Some("Acme ID".into()),
        auto_redirect: true,
        require_email_verified: true,
        confirms_sign_in: None,
    };
    let mut conn = s.pg.acquire().await.unwrap();
    assert_eq!(
        oidc::upsert(&mut conn, &connection(first)).await.unwrap(),
        Upserted::Saved
    );
    let stored = oidc::get(&mut *conn, first).await.unwrap().unwrap();
    assert_eq!(stored.display_name.as_deref(), Some("Acme ID"));
    assert!(stored.auto_redirect);
    assert_eq!(
        oidc::upsert(&mut conn, &connection(second)).await.unwrap(),
        Upserted::Saved,
        "an unverified domain blocks no other organization"
    );
    assert_eq!(
        oidc::org_for_domain(&mut *conn, "acme.example")
            .await
            .unwrap(),
        None
    );
    assert!(
        oidc::mark_verified(
            &mut *conn,
            first,
            "acme.example",
            time::OffsetDateTime::now_utc()
        )
        .await
        .unwrap()
    );
    assert_eq!(
        oidc::org_for_domain(&mut *conn, "ACME.example")
            .await
            .unwrap(),
        Some(first)
    );
    assert!(
        oidc::verified_elsewhere(&mut *conn, second, "acme.example")
            .await
            .unwrap()
    );
    assert_eq!(
        oidc::upsert(&mut conn, &connection(second)).await.unwrap(),
        Upserted::DomainTaken("acme.example".into())
    );
    assert_eq!(
        oidc::upsert(&mut conn, &connection(first)).await.unwrap(),
        Upserted::Saved
    );
    assert_eq!(
        oidc::get(&mut *conn, first)
            .await
            .unwrap()
            .unwrap()
            .verified_domains,
        ["acme.example"],
        "saving the connection again keeps its verified domains"
    );
    drop(conn);
    s.cleanup().await;
}

#[tokio::test]
async fn an_account_credential_refreshes_in_place_until_it_is_signed_out() {
    use kaeso_store::pg::credentials::{self, NewCredential, Sealed};
    let s = TestServices::start().await;
    let org = uuid::Uuid::now_v7();
    kaeso_store::pg::orgs::insert(&s.pg, org, "Acme", 30, 30)
        .await
        .unwrap();
    let now = time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap();
    let new = |id, auth_method, expires_at, account_label| NewCredential {
        id,
        org_id: org,
        provider: "openai",
        name: "Plan",
        ciphertext: &[1],
        nonce: &[2],
        wrapped_key: &[3],
        key_id: "k1",
        last4: "",
        created_by_kind: "system",
        created_by_id: None,
        auth_method,
        expires_at,
        account_label,
        project: None,
        location: None,
    };
    let account = uuid::Uuid::now_v7();
    credentials::insert(
        &s.pg,
        &new(account, "chatgpt", Some(now), Some("ada@example.com")),
    )
    .await
    .unwrap();
    let no_expiry = new(
        uuid::Uuid::now_v7(),
        "chatgpt",
        None,
        Some("ada@example.com"),
    );
    assert!(
        credentials::insert(&s.pg, &no_expiry).await.is_err(),
        "an account always has an expiry"
    );
    let expiring_key = new(uuid::Uuid::now_v7(), "api_key", Some(now), None);
    assert!(
        credentials::insert(&s.pg, &expiring_key).await.is_err(),
        "an API key never expires"
    );
    let email = "kaeso@kaeso-example.iam.gserviceaccount.com";
    let service_account = |id, project, location, expires_at| NewCredential {
        provider: "gemini-agent-platform",
        project,
        location,
        ..new(id, "google_service_account", expires_at, Some(email))
    };
    let sa = uuid::Uuid::now_v7();
    credentials::insert(
        &s.pg,
        &service_account(sa, Some("kaeso-example"), Some("us-east5"), None),
    )
    .await
    .unwrap();
    for (project, location, expires_at, why) in [
        (
            None,
            Some("global"),
            None,
            "a service account key names its project",
        ),
        (
            Some("kaeso-example"),
            None,
            None,
            "a service account key names its location",
        ),
        (
            Some("kaeso-example"),
            Some("global"),
            Some(now),
            "a service account key stores no expiry of its own",
        ),
        (
            Some("Not A Project"),
            Some("global"),
            None,
            "projects follow Google's rule",
        ),
        (
            Some("kaeso-example"),
            Some("us-central1.example.com"),
            None,
            "locations follow the platform's rule",
        ),
    ] {
        assert!(
            credentials::insert(
                &s.pg,
                &service_account(uuid::Uuid::now_v7(), project, location, expires_at)
            )
            .await
            .is_err(),
            "{why}"
        );
    }
    let row = credentials::sealed(&s.pg, org, sa).await.unwrap().unwrap();
    assert_eq!(row.location.as_deref(), Some("us-east5"));
    assert!(
        credentials::set_location(&s.pg, org, sa, "eu")
            .await
            .unwrap()
    );
    let moved = credentials::sealed(&s.pg, org, sa).await.unwrap().unwrap();
    assert_eq!(
        (moved.location.as_deref(), moved.revision),
        (Some("eu"), row.revision + 1),
        "a new location is a new revision, so its token is fetched again"
    );
    assert!(
        !credentials::set_location(&s.pg, org, account, "eu")
            .await
            .unwrap(),
        "only a service account key has a location"
    );

    let later = now + time::Duration::hours(1);
    let sealed = Sealed {
        ciphertext: &[4],
        nonce: &[5],
        wrapped_key: &[6],
        key_id: "k2",
    };
    let mut tx = s.pg.begin().await.unwrap();
    credentials::lock_refresh(&mut tx, account).await.unwrap();
    assert!(
        credentials::refreshed(&mut *tx, account, &sealed, later)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    let row = credentials::sealed(&s.pg, org, account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            row.ciphertext,
            row.key_id.as_str(),
            row.expires_at,
            row.revision
        ),
        (vec![4], "k2", Some(later), 1),
        "a refresh keeps the revision, so what Kaeso learnt about the key stays"
    );

    credentials::sign_out(&s.pg, account, "signed_out")
        .await
        .unwrap();
    let row = credentials::sealed(&s.pg, org, account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.auth_error.as_deref(), Some("signed_out"));
    assert!(
        !credentials::refreshed(&s.pg, account, &sealed, later)
            .await
            .unwrap(),
        "a signed-out account takes no refreshed tokens"
    );
    let listed = credentials::list(&s.pg, org).await.unwrap();
    let plan = listed.iter().find(|c| c.id == account).unwrap();
    assert_eq!(plan.account_label.as_deref(), Some("ada@example.com"));
    assert_eq!(plan.auth_method, "chatgpt");
    let key = listed.iter().find(|c| c.id == sa).unwrap();
    assert_eq!(
        (key.project.as_deref(), key.location.as_deref()),
        (Some("kaeso-example"), Some("eu"))
    );

    let again = new(account, "chatgpt", Some(later), Some("ada@example.com"));
    assert!(credentials::replace(&s.pg, &again).await.unwrap());
    let row = credentials::sealed(&s.pg, org, account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (row.auth_error, row.revision),
        (None, 2),
        "signing in again clears the error"
    );
    s.cleanup().await;
}

#[tokio::test]
async fn the_agent_host_id_is_chosen_once_per_install() {
    let s = TestServices::start().await;
    sqlx::query(
        "INSERT INTO instance_settings (org_creation, metadata_retention_days, content_retention_days, \
         max_metadata_retention_days, max_content_retention_days) VALUES ('everyone', 30, 30, 30, 30)",
    )
    .execute(&s.pg)
    .await
    .unwrap();
    let first = "urn:uuid:5f0c6f42-8f0e-4d2b-9a57-1e7c2d9b3a10";
    let second = "urn:uuid:0e2b4c6d-1a3f-4b5c-8d7e-9f0a1b2c3d4e";
    let chosen = kaeso_store::pg::instance::agent_host_id(&s.pg, first)
        .await
        .unwrap();
    assert_eq!(chosen, first);
    let again = kaeso_store::pg::instance::agent_host_id(&s.pg, second)
        .await
        .unwrap();
    assert_eq!(again, first);
    s.cleanup().await;
}

#[tokio::test]
async fn a_device_attempt_is_locked_kept_and_used_up_by_its_id() {
    use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
    let s = TestServices::start().await;
    let now = time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap();
    let token = |kind, hash: &'static [u8]| NewToken {
        kind,
        hash,
        user_id: None,
        email: None,
        data: serde_json::json!({ "interval": 5 }),
        expires_at: now + time::Duration::minutes(10),
    };
    let device = tokens::insert(&s.pg, token(TokenKind::ProviderDevice, b"device"))
        .await
        .unwrap();
    let link = tokens::insert(&s.pg, token(TokenKind::MagicLink, b"link"))
        .await
        .unwrap();

    let mut tx = s.pg.begin().await.unwrap();
    let found = tokens::lock_attempt(&mut tx, device, now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.kind, "provider_device");
    assert_eq!(found.token.data["interval"], 5);
    tokens::keep_data(&mut *tx, device, &serde_json::json!({ "interval": 10 }))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut conn = s.pg.acquire().await.unwrap();
    assert!(
        tokens::lock_attempt(&mut conn, link, now)
            .await
            .unwrap()
            .is_none(),
        "only sign-in attempts of provider accounts are found by id"
    );
    let kept = tokens::lock_attempt(&mut conn, device, now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(kept.token.data["interval"], 10);
    assert!(
        tokens::lock_attempt(&mut conn, device, now + time::Duration::minutes(10))
            .await
            .unwrap()
            .is_none(),
        "an expired attempt is gone"
    );
    assert!(tokens::use_up(&mut *conn, device, now).await.unwrap());
    assert!(!tokens::use_up(&mut *conn, device, now).await.unwrap());
    assert!(
        tokens::lock_attempt(&mut conn, device, now)
            .await
            .unwrap()
            .is_none()
    );
    drop(conn);
    s.cleanup().await;
}

#[tokio::test]
async fn a_grok_account_can_lose_its_api_access() {
    use kaeso_store::pg::credentials::{self, NewCredential};
    let s = TestServices::start().await;
    let org = uuid::Uuid::now_v7();
    kaeso_store::pg::orgs::insert(&s.pg, org, "Acme", 30, 30)
        .await
        .unwrap();
    let now = time::OffsetDateTime::now_utc();
    let new = |id, auth_method| NewCredential {
        id,
        org_id: org,
        provider: "xai",
        name: "Grok",
        ciphertext: &[1],
        nonce: &[2],
        wrapped_key: &[3],
        key_id: "k1",
        last4: "",
        created_by_kind: "system",
        created_by_id: None,
        auth_method,
        expires_at: Some(now),
        account_label: Some("ada@example.com"),
        project: None,
        location: None,
    };
    let grok = uuid::Uuid::now_v7();
    credentials::insert(&s.pg, &new(grok, "grok"))
        .await
        .unwrap();
    assert!(
        credentials::insert(&s.pg, &new(uuid::Uuid::now_v7(), "password"))
            .await
            .is_err()
    );
    credentials::sign_out(&s.pg, grok, "no_api_access")
        .await
        .unwrap();
    let row = credentials::sealed(&s.pg, org, grok)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (row.auth_method.as_str(), row.auth_error.as_deref()),
        ("grok", Some("no_api_access"))
    );
    assert!(
        credentials::sign_out(&s.pg, grok, "forgotten")
            .await
            .is_err(),
        "an account stops only for a known reason"
    );
    s.cleanup().await;
}
