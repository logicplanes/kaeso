#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use kaeso_store::pg::custom_providers::{self as store, Fields, InUse, NewCustomProvider};
use kaeso_testkit::services::TestServices;
use uuid::Uuid;

async fn org_with_project(pg: &sqlx::PgPool, name: &str) -> (Uuid, Uuid) {
    let (org, project) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query(
        "INSERT INTO organizations (id, name, metadata_retention_days, content_retention_days) VALUES ($1, $2, 90, 30)",
    )
    .bind(org)
    .bind(name)
    .execute(pg)
    .await
    .unwrap();
    sqlx::query("INSERT INTO projects (id, org_id, name) VALUES ($1, $2, 'Checkout')")
        .bind(project)
        .bind(org)
        .execute(pg)
        .await
        .unwrap();
    (org, project)
}

async fn add_provider(pg: &sqlx::PgPool, org: Uuid, base_url: &str) -> bool {
    store::insert(
        pg,
        &NewCustomProvider {
            id: Uuid::now_v7(),
            org_id: org,
            provider: "lab",
            fields: Fields {
                name: "Lab",
                base_url,
                auth: "bearer",
                generation: "openai_chat",
                embeddings: None,
                count_tokens: None,
                server: None,
                media: vec!["speech".into()],
                decisions: None,
            },
            created_by_kind: "system",
            created_by_id: None,
        },
    )
    .await
    .unwrap()
}

async fn add_key_and_model(pg: &sqlx::PgPool, org: Uuid, project: Uuid) {
    sqlx::query(
        "INSERT INTO provider_credentials (id, org_id, provider, name, ciphertext, nonce, wrapped_key, key_id, last4, created_by_kind) \
         VALUES ($1, $2, 'lab', 'Lab key', '\\x00', '\\x00', '\\x00', 'k1', 'abcd', 'system')",
    )
    .bind(Uuid::now_v7())
    .bind(org)
    .execute(pg)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO catalogue_models (id, org_id, project_id, provider, model_id, custom, created_by_kind) \
         VALUES ($1, $2, $3, 'lab', 'llama', true, 'system')",
    )
    .bind(Uuid::now_v7())
    .bind(org)
    .bind(project)
    .execute(pg)
    .await
    .unwrap();
}

async fn count(pg: &sqlx::PgPool, table: &str, org: Uuid) -> i64 {
    let sql = match table {
        "provider_credentials" => {
            "SELECT count(*) FROM provider_credentials WHERE org_id = $1 AND provider = 'lab'"
        }
        _ => "SELECT count(*) FROM catalogue_models WHERE org_id = $1 AND provider = 'lab'",
    };
    let (n,): (i64,) = sqlx::query_as(sql).bind(org).fetch_one(pg).await.unwrap();
    n
}

#[tokio::test]
async fn each_organization_reads_and_deletes_only_its_own_custom_provider_of_an_id() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let (acme, acme_project) = org_with_project(&s.pg, "Acme").await;
    let (globex, globex_project) = org_with_project(&s.pg, "Globex").await;
    assert!(add_provider(&s.pg, acme, "https://acme.example.com/").await);
    assert!(!add_provider(&s.pg, acme, "https://twice.example.com/").await);
    assert!(add_provider(&s.pg, globex, "https://globex.example.com/").await);
    add_key_and_model(&s.pg, acme, acme_project).await;
    add_key_and_model(&s.pg, globex, globex_project).await;

    let listed = store::list(&s.pg, acme).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].base_url, "https://acme.example.com/");
    let seen = store::of_project(&s.pg, globex_project).await.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].base_url, "https://globex.example.com/");
    assert_eq!(
        seen[0].custom_provider().unwrap().base_url().as_str(),
        "https://globex.example.com/"
    );
    assert_eq!(
        seen[0].custom_provider().unwrap().media(),
        [kaeso_core::providers::MediaApi::Speech]
    );
    let mut conn = s.pg.acquire().await.unwrap();
    assert_eq!(
        store::in_use(&mut conn, acme).await.unwrap()["lab"],
        InUse {
            keys: 1,
            models: 1,
            projects: 1
        }
    );

    let removed = store::delete(&mut conn, acme, "lab")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((removed.credentials, removed.models), (1, 1));
    assert_eq!(removed.projects, [acme_project]);
    assert!(
        store::delete(&mut conn, acme, "lab")
            .await
            .unwrap()
            .is_none()
    );
    drop(conn);
    for table in ["provider_credentials", "catalogue_models"] {
        assert_eq!(count(&s.pg, table, acme).await, 0, "{table}");
        assert_eq!(count(&s.pg, table, globex).await, 1, "{table}");
    }
    assert_eq!(store::list(&s.pg, globex).await.unwrap().len(), 1);
    s.cleanup().await;
}

#[tokio::test]
async fn the_table_refuses_what_the_gateway_would_never_write() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let (org, _) = org_with_project(&s.pg, "Acme").await;
    for (provider, base_url, generation, count_tokens) in [
        ("Lab", "https://x.example.com", "openai_chat", None),
        ("lab", "file:///etc/passwd", "openai_chat", None),
        ("lab", "https://x.example.com", "openai_embeddings", None),
        (
            "lab",
            "https://x.example.com",
            "openai_chat",
            Some("openai_chat"),
        ),
        (
            "lab",
            "https://x.example.com",
            "anthropic_messages",
            Some("openai_responses"),
        ),
    ] {
        let r = sqlx::query(
            "INSERT INTO custom_providers (id, org_id, provider, name, base_url, auth, generation, count_tokens, created_by_kind) \
             VALUES ($1, $2, $3, 'Lab', $4, 'bearer', $5, $6, 'system')",
        )
        .bind(Uuid::now_v7())
        .bind(org)
        .bind(provider)
        .bind(base_url)
        .bind(generation)
        .bind(count_tokens)
        .execute(&s.pg)
        .await;
        assert!(
            r.is_err(),
            "{provider} {base_url} {generation} {count_tokens:?}"
        );
    }
    s.cleanup().await;
}

#[tokio::test]
async fn a_custom_providers_decisions_api_is_stored_and_read_back_only_in_a_shape_kaeso_writes() {
    use kaeso_core::providers::{DecisionsApi, DecisionsFormat, ImagePlacement, MediaApi};
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let (org, project) = org_with_project(&s.pg, "Acme").await;
    let decisions = serde_json::json!({"format": "system_one", "path": "/decisions/v1/systemone", "media": "root"});
    assert!(
        store::insert(
            &s.pg,
            &NewCustomProvider {
                id: Uuid::now_v7(),
                org_id: org,
                provider: "lab",
                fields: Fields {
                    name: "Lab",
                    base_url: "https://lab.example.com/",
                    auth: "bearer",
                    generation: "openai_chat",
                    embeddings: None,
                    count_tokens: None,
                    server: None,
                    media: vec!["decisions".into()],
                    decisions: Some(decisions),
                },
                created_by_kind: "system",
                created_by_id: None,
            },
        )
        .await
        .unwrap()
    );
    let row = &store::of_project(&s.pg, project).await.unwrap()[0];
    let p = row.custom_provider().unwrap();
    assert_eq!(p.media(), [MediaApi::Decisions]);
    assert_eq!(
        p.decisions(),
        Some(&DecisionsApi {
            format: DecisionsFormat::SystemOne,
            path: "/decisions/v1/systemone".into(),
            media: Some(ImagePlacement::Root),
        })
    );

    let mut draft = p.draft();
    draft.decisions = Some(DecisionsApi::of(DecisionsFormat::OpenaiDecisions));
    let changed = kaeso_core::providers::CustomProvider::new(draft).unwrap();
    assert!(
        store::update(&s.pg, org, "lab", &Fields::of(&changed))
            .await
            .unwrap()
    );
    let row = store::get(&s.pg, org, "lab").await.unwrap().unwrap();
    assert_eq!(
        row.decisions,
        Some(serde_json::json!({"format": "openai_decisions", "path": "/v1/decisions"}))
    );
    assert_eq!(row.custom_provider().unwrap(), changed);

    let mut off = changed.draft();
    off.media = Vec::new();
    off.decisions = None;
    let off = kaeso_core::providers::CustomProvider::new(off).unwrap();
    assert!(
        store::update(&s.pg, org, "lab", &Fields::of(&off))
            .await
            .unwrap()
    );
    let row = store::get(&s.pg, org, "lab").await.unwrap().unwrap();
    assert_eq!((row.media.len(), row.decisions), (0, None));

    let refused = [
        ("{decisions}", None),
        (
            "{}",
            Some(r#"{"format": "system_one", "path": "/v1/systemone", "media": "assets"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "system_one", "path": "/v1/systemone"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "system_one", "path": "/v1/systemone", "media": "pixels"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "/v1/decisions", "media": "assets"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "gemini_decisions", "path": "/v1beta/decisions"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "v1/decisions"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "/v1/../admin"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "/v1/decisions?x=1"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "/v1//decisions"}"#),
        ),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": 7}"#),
        ),
        ("{decisions}", Some(r#"{"format": "openai_decisions"}"#)),
        ("{decisions}", Some(r#"{"path": "/v1/decisions"}"#)),
        (
            "{decisions}",
            Some(r#"{"format": "openai_decisions", "path": "/v1/decisions", "key": "x"}"#),
        ),
        ("{decisions}", Some(r#"["system_one"]"#)),
        ("{robots}", None),
    ];
    for (media, decisions) in refused {
        let r = sqlx::query(
            "INSERT INTO custom_providers (id, org_id, provider, name, base_url, auth, generation, media, decisions, created_by_kind) \
             VALUES ($1, $2, 'other', 'Other', 'https://x.example.com/', 'bearer', 'openai_chat', $3::text[], $4::jsonb, 'system')",
        )
        .bind(Uuid::now_v7())
        .bind(org)
        .bind(media)
        .bind(decisions)
        .execute(&s.pg)
        .await;
        assert!(r.is_err(), "{media} {decisions:?}");
    }
    s.cleanup().await;
}
