#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use kaeso_store::clickhouse::logs;
use kaeso_store::clickhouse::tool_calls::{self, ToolCallContentRow, ToolCallRow};
use kaeso_store::pg::agents::TokenKind;
use kaeso_store::pg::catalogue::Subject;
use kaeso_store::pg::credentials::{self, Sealed};
use kaeso_store::pg::mcp_connections::{self as connections, NewConnection};
use kaeso_store::pg::mcp_servers::{
    self as servers, Changes, Check, NewServer, OAuthClient, Updated,
};
use kaeso_store::pg::mcp_tools::{self as tools, Approval, Definition, ServerTools};
use kaeso_store::pg::portal_grants::{self as grants, NewGrant, NewToken};
use kaeso_store::pg::tool_access;
use kaeso_testkit::services::TestServices;
use serde_json::json;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn at(minutes: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_793_000_000).unwrap() + Duration::minutes(minutes)
}

async fn exec(pg: &PgPool, sql: &'static str, ids: &[Uuid]) {
    let mut q = sqlx::query(sql);
    for id in ids {
        q = q.bind(*id);
    }
    q.execute(pg).await.unwrap();
}

struct Org {
    id: Uuid,
    project: Uuid,
    other_project: Uuid,
}

async fn org(pg: &PgPool, name: &str) -> Org {
    let o = Org {
        id: Uuid::now_v7(),
        project: Uuid::now_v7(),
        other_project: Uuid::now_v7(),
    };
    sqlx::query(
        "INSERT INTO organizations (id, name, metadata_retention_days, content_retention_days) VALUES ($1, $2, 90, 30)",
    )
    .bind(o.id)
    .bind(name)
    .execute(pg)
    .await
    .unwrap();
    for p in [o.project, o.other_project] {
        exec(
            pg,
            "INSERT INTO projects (id, org_id, name) VALUES ($1, $2, 'Marketing')",
            &[p, o.id],
        )
        .await;
    }
    o
}

async fn user(pg: &PgPool, org: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, 'Ada')")
        .bind(id)
        .bind(format!("{id}@example.com"))
        .execute(pg)
        .await
        .unwrap();
    exec(
        pg,
        "INSERT INTO org_memberships (id, org_id, user_id, preset) VALUES ($1, $2, $3, 'developer')",
        &[Uuid::now_v7(), org, id],
    )
    .await;
    id
}

async fn server(pg: &PgPool, org: Uuid, handle: &str, auth: &str) -> Uuid {
    let id = Uuid::now_v7();
    let inserted = servers::insert(
        pg,
        &NewServer {
            id,
            org_id: org,
            handle,
            name: "X",
            url: "https://mcp.example.com/mcp",
            auth_method: auth,
            credential_mode: "shared",
            oauth_registration: (auth == "oauth").then_some("cimd"),
            scopes: &[],
            change_policy: "hold",
            created_by_kind: "system",
            created_by_id: None,
        },
    )
    .await
    .unwrap();
    assert!(inserted);
    id
}

fn sealed(tag: &[u8]) -> Sealed<'_> {
    Sealed {
        ciphertext: tag,
        nonce: b"nonce",
        wrapped_key: tag,
        key_id: "k1",
    }
}

fn definition<'a>(name: &'a str, schema: &'a serde_json::Value, hash: &'a [u8]) -> Definition<'a> {
    Definition {
        name,
        title: Some("Create post"),
        description: "Posts to X.",
        input_schema: schema,
        output_schema: None,
        annotations: None,
        meta: None,
        hash,
    }
}

#[tokio::test]
async fn servers_are_stored_per_organization_with_unique_handles() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let globex = org(&s.pg, "Globex").await;
    let x = server(&s.pg, acme.id, "x", "oauth").await;
    let github = server(&s.pg, acme.id, "github", "header").await;
    server(&s.pg, globex.id, "x", "none").await;

    let twice = servers::insert(
        &s.pg,
        &NewServer {
            id: Uuid::now_v7(),
            org_id: acme.id,
            handle: "x",
            name: "Again",
            url: "https://other.example.com/mcp",
            auth_method: "none",
            credential_mode: "shared",
            oauth_registration: None,
            scopes: &[],
            change_policy: "hold",
            created_by_kind: "system",
            created_by_id: None,
        },
    )
    .await
    .unwrap();
    assert!(!twice);

    let listed = servers::list(&s.pg, acme.id).await.unwrap();
    assert_eq!(
        listed.iter().map(|r| r.handle.as_str()).collect::<Vec<_>>(),
        ["github", "x"]
    );
    assert!(servers::get(&s.pg, globex.id, x).await.unwrap().is_none());
    assert_eq!(
        servers::get_by_id(&s.pg, x).await.unwrap().unwrap().org_id,
        acme.id
    );

    let renamed = servers::update(
        &s.pg,
        acme.id,
        github,
        &Changes {
            name: Some("GitHub"),
            handle: Some("gh"),
            change_policy: Some("accept"),
        },
    )
    .await
    .unwrap();
    let Updated::Done(row) = renamed else {
        panic!("{renamed:?}")
    };
    assert_eq!(
        (
            row.name.as_str(),
            row.handle.as_str(),
            row.change_policy.as_str()
        ),
        ("GitHub", "gh", "accept")
    );
    assert_eq!(row.revision, 2);
    assert_eq!(
        servers::update(
            &s.pg,
            acme.id,
            github,
            &Changes {
                handle: Some("x"),
                ..Changes::default()
            }
        )
        .await
        .unwrap(),
        Updated::HandleTaken
    );
    assert_eq!(
        servers::update(&s.pg, globex.id, github, &Changes::default())
            .await
            .unwrap(),
        Updated::NotFound
    );

    assert!(!servers::delete(&s.pg, globex.id, x).await.unwrap());
    assert!(servers::delete(&s.pg, acme.id, x).await.unwrap());
    assert_eq!(servers::list(&s.pg, acme.id).await.unwrap().len(), 1);
    assert_eq!(servers::list(&s.pg, globex.id).await.unwrap().len(), 1);
    s.cleanup().await;
}

#[tokio::test]
async fn the_server_table_refuses_what_kaeso_would_never_write() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    for (handle, auth, mode, registration, url) in [
        ("X", "none", "shared", None, "https://x.example.com"),
        (
            "a-very-long-handle-over-24",
            "none",
            "shared",
            None,
            "https://x.example.com",
        ),
        ("x", "none", "per_user", None, "https://x.example.com"),
        ("x", "oauth", "shared", None, "https://x.example.com"),
        (
            "x",
            "header",
            "shared",
            Some("cimd"),
            "https://x.example.com",
        ),
        ("x", "none", "shared", None, "ftp://x.example.com"),
    ] {
        let r = servers::insert(
            &s.pg,
            &NewServer {
                id: Uuid::now_v7(),
                org_id: acme.id,
                handle,
                name: "X",
                url,
                auth_method: auth,
                credential_mode: mode,
                oauth_registration: registration,
                scopes: &[],
                change_policy: "hold",
                created_by_kind: "system",
                created_by_id: None,
            },
        )
        .await;
        assert!(r.is_err(), "{handle} {auth} {mode} {registration:?} {url}");
    }
    let key_without_ability = sqlx::query(
        "INSERT INTO api_keys (id, org_id, project_id, user_id, name, secret_hash, secret_prefix, secret_last4, \
         inference, platform, tools, created_by_kind) \
         VALUES ($1, $2, $3, $4, 'k', '\\x01', 'sk-kso-', 'abcd', false, false, false, 'system')",
    )
    .bind(Uuid::now_v7())
    .bind(acme.id)
    .bind(acme.project)
    .bind(user(&s.pg, acme.id).await)
    .execute(&s.pg)
    .await;
    assert!(key_without_ability.is_err());
    s.cleanup().await;
}

#[tokio::test]
async fn oauth_clients_checks_and_projects_are_kept_per_server() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let globex = org(&s.pg, "Globex").await;
    let x = server(&s.pg, acme.id, "x", "oauth").await;
    let plain = server(&s.pg, acme.id, "docs", "none").await;

    assert!(servers::client_secret(&s.pg, x).await.unwrap().is_none());
    assert!(
        servers::set_oauth_client(
            &s.pg,
            x,
            &OAuthClient {
                registration: "manual",
                issuer: "https://auth.example.com",
                client_id: "kaeso",
                secret: Some(sealed(b"secret")),
            },
        )
        .await
        .unwrap()
    );
    assert!(
        !servers::set_oauth_client(
            &s.pg,
            plain,
            &OAuthClient {
                registration: "manual",
                issuer: "https://auth.example.com",
                client_id: "kaeso",
                secret: None,
            },
        )
        .await
        .unwrap()
    );
    let row = servers::get(&s.pg, acme.id, x).await.unwrap().unwrap();
    assert_eq!(row.oauth_client_id.as_deref(), Some("kaeso"));
    assert_eq!(
        row.oauth_issuer.as_deref(),
        Some("https://auth.example.com")
    );
    assert!(row.has_client_secret);
    let secret = servers::client_secret(&s.pg, x).await.unwrap().unwrap();
    assert_eq!(secret.ciphertext, b"secret");

    let info = json!({ "name": "x-mcp", "version": "2.0" });
    servers::record_check(
        &s.pg,
        x,
        &Check {
            status: "ok",
            protocol_version: Some("2026-07-28"),
            server_info: Some(&info),
            last_error: None,
            at: at(0),
        },
    )
    .await
    .unwrap();
    servers::record_check(
        &s.pg,
        x,
        &Check {
            status: "unreachable",
            protocol_version: None,
            server_info: None,
            last_error: Some("timed out"),
            at: at(20),
        },
    )
    .await
    .unwrap();
    let row = servers::get(&s.pg, acme.id, x).await.unwrap().unwrap();
    assert_eq!(row.status, "unreachable");
    assert_eq!(row.protocol_version.as_deref(), Some("2026-07-28"));
    assert_eq!(row.server_info, Some(info));
    assert_eq!(row.last_error.as_deref(), Some("timed out"));
    assert_eq!(servers::due(&s.pg, at(30), 10).await.unwrap(), [plain, x]);
    assert_eq!(servers::due(&s.pg, at(10), 10).await.unwrap(), [plain]);
    assert_eq!(servers::due(&s.pg, at(30), 1).await.unwrap(), [plain]);

    let mut conn = s.pg.acquire().await.unwrap();
    servers::set_projects(
        &mut conn,
        acme.id,
        x,
        &[acme.project, acme.other_project, globex.project],
    )
    .await
    .unwrap();
    assert_eq!(
        servers::projects(&mut *conn, x)
            .await
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>(),
        [acme.project, acme.other_project].into()
    );
    servers::set_projects(&mut conn, acme.id, x, &[acme.project])
        .await
        .unwrap();
    assert_eq!(
        servers::projects(&mut *conn, x).await.unwrap(),
        [acme.project]
    );
    servers::set_project_servers(&mut conn, acme.id, acme.project, &[x, plain])
        .await
        .unwrap();
    servers::set_project_servers(&mut conn, globex.id, globex.project, &[x])
        .await
        .unwrap();
    let used: Vec<_> = servers::of_project(&mut *conn, acme.project)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.handle)
        .collect();
    assert_eq!(used, ["docs", "x"]);
    assert!(
        servers::of_project(&mut *conn, globex.project)
            .await
            .unwrap()
            .is_empty()
    );
    servers::set_project_servers(&mut conn, acme.id, acme.project, &[plain])
        .await
        .unwrap();
    assert!(servers::projects(&mut *conn, x).await.unwrap().is_empty());
    drop(conn);

    let all = servers::all_sealed(&s.pg).await.unwrap();
    assert_eq!(all.len(), 1);
    assert!(
        !servers::set_wrapped_key(&s.pg, x, b"stale", b"new", "k2")
            .await
            .unwrap()
    );
    assert!(
        servers::set_wrapped_key(&s.pg, x, b"secret", b"new", "k2")
            .await
            .unwrap()
    );
    let usage = credentials::key_id_usage(&s.pg).await.unwrap();
    assert!(usage.contains(&("k2".to_owned(), 1)), "{usage:?}");
    s.cleanup().await;
}

#[tokio::test]
async fn connections_are_shared_or_personal_and_leave_with_the_person() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let x = server(&s.pg, acme.id, "x", "oauth").await;
    let ada = user(&s.pg, acme.id).await;
    let mut conn = s.pg.acquire().await.unwrap();

    let new = |user_id: Option<Uuid>, tag: &'static [u8], label: &'static str| NewConnection {
        id: Uuid::now_v7(),
        server_id: x,
        org_id: acme.id,
        user_id,
        kind: "oauth",
        issuer: Some("https://auth.example.com"),
        header_name: None,
        sealed: sealed(tag),
        last4: "abcd",
        expires_at: Some(at(60)),
        account_label: Some(label),
    };
    let shared = connections::put(&mut conn, &new(None, b"one", "@kaesoapp"))
        .await
        .unwrap();
    let again = connections::put(&mut conn, &new(None, b"two", "@kaeso"))
        .await
        .unwrap();
    assert_eq!(shared, again);
    let mine = connections::put(&mut conn, &new(Some(ada), b"ada", "@ada"))
        .await
        .unwrap();
    assert_ne!(mine, shared);

    let row = connections::sealed(&mut *conn, x, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (row.ciphertext.as_slice(), row.revision),
        (b"two".as_slice(), 2)
    );
    assert_eq!(
        connections::sealed(&mut *conn, x, Some(ada))
            .await
            .unwrap()
            .unwrap()
            .id,
        mine
    );
    assert_eq!(
        connections::of_server(&mut *conn, x).await.unwrap().len(),
        2
    );
    assert_eq!(
        connections::shared_of_org(&mut *conn, acme.id)
            .await
            .unwrap()
            .len(),
        1
    );
    let ada_rows = connections::of_user(&mut *conn, ada).await.unwrap();
    assert_eq!(ada_rows.len(), 1);
    assert_eq!(ada_rows[0].server_handle, "x");
    assert_eq!(
        ada_rows[0].connection.account_label.as_deref(),
        Some("@ada")
    );

    assert!(
        !connections::refreshed(&mut *conn, shared, 1, &sealed(b"late"), Some(at(90)))
            .await
            .unwrap()
    );
    assert!(
        connections::refreshed(&mut *conn, shared, 2, &sealed(b"three"), Some(at(90)))
            .await
            .unwrap()
    );
    assert!(connections::sign_out(&mut *conn, shared).await.unwrap());
    assert!(!connections::sign_out(&mut *conn, shared).await.unwrap());
    assert!(
        !connections::refreshed(&mut *conn, shared, 3, &sealed(b"four"), None)
            .await
            .unwrap()
    );
    connections::put(&mut conn, &new(None, b"five", "@kaesoapp"))
        .await
        .unwrap();
    let back = connections::sealed(&mut *conn, x, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(back.auth_error, None);

    connections::touch(&mut *conn, shared, at(0)).await.unwrap();
    connections::touch(&mut *conn, shared, at(0) + Duration::seconds(10))
        .await
        .unwrap();
    let used = connections::shared_of_org(&mut *conn, acme.id)
        .await
        .unwrap()[0]
        .last_used_at;
    assert_eq!(used, Some(at(0)));

    assert_eq!(connections::all_sealed(&mut *conn).await.unwrap().len(), 2);
    assert!(
        connections::set_wrapped_key(&mut *conn, mine, b"ada", b"rewrapped", "k9")
            .await
            .unwrap()
    );
    drop(conn);
    exec(
        &s.pg,
        "DELETE FROM org_memberships WHERE user_id = $1",
        &[ada],
    )
    .await;
    assert!(
        connections::sealed(&s.pg, x, Some(ada))
            .await
            .unwrap()
            .is_none()
    );
    assert!(connections::delete(&s.pg, x, None).await.unwrap());
    assert!(!connections::delete(&s.pg, x, None).await.unwrap());

    let stranger = Uuid::now_v7();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, 'stranger@example.com', 'Sam')")
        .bind(stranger)
        .execute(&s.pg)
        .await
        .unwrap();
    let mut conn = s.pg.acquire().await.unwrap();
    assert!(
        connections::put(&mut conn, &new(Some(stranger), b"x", "@sam"))
            .await
            .is_err()
    );
    drop(conn);
    s.cleanup().await;
}

#[tokio::test]
async fn tools_keep_their_definition_and_hold_changes() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let globex = org(&s.pg, "Globex").await;
    let x = server(&s.pg, acme.id, "x", "none").await;
    let schema = json!({ "type": "object" });
    let first = json!({ "name": "create_post" });
    let (post, search) = (Uuid::now_v7(), Uuid::now_v7());

    assert!(
        tools::insert(
            &s.pg,
            post,
            x,
            acme.id,
            &definition("create_post", &schema, &[1; 32]),
            Some(&Approval {
                hash: &[1; 32],
                definition: &first
            }),
            at(0)
        )
        .await
        .unwrap()
    );
    assert!(
        !tools::insert(
            &s.pg,
            Uuid::now_v7(),
            x,
            acme.id,
            &definition("create_post", &schema, &[1; 32]),
            None,
            at(0)
        )
        .await
        .unwrap()
    );
    assert!(
        tools::insert(
            &s.pg,
            search,
            x,
            acme.id,
            &definition("search", &schema, &[2; 32]),
            None,
            at(0)
        )
        .await
        .unwrap()
    );
    let row = tools::get(&s.pg, acme.id, post).await.unwrap().unwrap();
    assert!(row.is_approved());
    assert_eq!(row.changed_at, None);
    let held = tools::get(&s.pg, acme.id, search).await.unwrap().unwrap();
    assert_eq!(
        (held.state.as_str(), held.changed_at),
        ("changed", Some(at(0)))
    );
    assert!(tools::get(&s.pg, globex.id, post).await.unwrap().is_none());

    assert!(
        tools::update_definition(
            &s.pg,
            post,
            &definition("create_post", &schema, &[3; 32]),
            None,
            at(5)
        )
        .await
        .unwrap()
    );
    let changed = tools::get(&s.pg, acme.id, post).await.unwrap().unwrap();
    assert_eq!(
        (changed.state.as_str(), changed.changed_at),
        ("changed", Some(at(5)))
    );
    assert!(!changed.is_approved());
    assert!(
        tools::update_definition(
            &s.pg,
            post,
            &definition("create_post", &schema, &[1; 32]),
            None,
            at(6)
        )
        .await
        .unwrap()
    );
    assert!(
        tools::get(&s.pg, acme.id, post)
            .await
            .unwrap()
            .unwrap()
            .is_approved()
    );
    let accepted = json!({ "name": "create_post", "v": 4 });
    tools::update_definition(
        &s.pg,
        post,
        &definition("create_post", &schema, &[4; 32]),
        Some(&Approval {
            hash: &[4; 32],
            definition: &accepted,
        }),
        at(7),
    )
    .await
    .unwrap();
    let row = tools::get(&s.pg, acme.id, post).await.unwrap().unwrap();
    assert!(row.is_approved());
    assert_eq!(row.approved, Some(accepted));
    assert!(
        !tools::update_definition(
            &s.pg,
            post,
            &definition("renamed", &schema, &[4; 32]),
            None,
            at(8)
        )
        .await
        .unwrap()
    );

    let approval = json!({ "name": "search" });
    assert!(
        !tools::approve(
            &s.pg,
            acme.id,
            search,
            &[9; 32],
            &Approval {
                hash: &[9; 32],
                definition: &approval
            },
            at(9)
        )
        .await
        .unwrap()
    );
    assert!(
        !tools::approve(
            &s.pg,
            globex.id,
            search,
            &[2; 32],
            &Approval {
                hash: &[2; 32],
                definition: &approval
            },
            at(9)
        )
        .await
        .unwrap()
    );
    assert!(
        tools::approve(
            &s.pg,
            acme.id,
            search,
            &[2; 32],
            &Approval {
                hash: &[2; 32],
                definition: &approval
            },
            at(9)
        )
        .await
        .unwrap()
    );
    assert!(
        tools::get(&s.pg, acme.id, search)
            .await
            .unwrap()
            .unwrap()
            .is_approved()
    );
    assert_eq!(
        tools::counts(&s.pg, acme.id).await.unwrap(),
        [(
            x,
            ServerTools {
                total: 2,
                changed: 0
            }
        )]
    );

    let gone = tools::mark_removed(&s.pg, x, &["create_post".to_owned()], at(10))
        .await
        .unwrap();
    assert_eq!(gone, [search]);
    assert!(
        tools::mark_removed(&s.pg, x, &["create_post".to_owned()], at(11))
            .await
            .unwrap()
            .is_empty()
    );
    let names: Vec<_> = tools::of_server(&s.pg, x)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.state)
        .collect();
    assert_eq!(names, ["active", "removed"]);
    assert_eq!(
        tools::counts(&s.pg, acme.id).await.unwrap(),
        [(
            x,
            ServerTools {
                total: 1,
                changed: 0
            }
        )]
    );
    s.cleanup().await;
}

#[tokio::test]
async fn projects_switch_on_tools_of_the_servers_they_use() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let x = server(&s.pg, acme.id, "x", "none").await;
    let docs = server(&s.pg, acme.id, "docs", "none").await;
    let schema = json!({ "type": "object" });
    let (post, read) = (Uuid::now_v7(), Uuid::now_v7());
    tools::insert(
        &s.pg,
        post,
        x,
        acme.id,
        &definition("create_post", &schema, &[1; 32]),
        None,
        at(0),
    )
    .await
    .unwrap();
    tools::insert(
        &s.pg,
        read,
        docs,
        acme.id,
        &definition("read", &schema, &[2; 32]),
        None,
        at(0),
    )
    .await
    .unwrap();
    let mut conn = s.pg.acquire().await.unwrap();
    servers::set_project_servers(&mut conn, acme.id, acme.project, &[x])
        .await
        .unwrap();

    assert_eq!(
        tools::set_enabled(&mut *conn, acme.project, &[post, read], true)
            .await
            .unwrap(),
        1
    );
    let listed = tools::of_project(&mut *conn, acme.project).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (
            listed[0].server_handle.as_str(),
            listed[0].enabled,
            listed[0].confirm
        ),
        ("x", true, false)
    );
    assert_eq!(
        tools::projects_using(&mut *conn, post).await.unwrap(),
        [acme.project]
    );
    assert_eq!(
        tools::set_enabled(&mut *conn, acme.project, &[post], false)
            .await
            .unwrap(),
        1
    );
    assert!(!tools::of_project(&mut *conn, acme.project).await.unwrap()[0].enabled);
    tools::set_enabled(&mut *conn, acme.project, &[post], true)
        .await
        .unwrap();
    assert_eq!(
        tools::disable_everywhere(&mut *conn, post).await.unwrap(),
        1
    );
    assert!(
        tools::projects_using(&mut *conn, post)
            .await
            .unwrap()
            .is_empty()
    );

    tools::set_enabled(&mut *conn, acme.project, &[post], true)
        .await
        .unwrap();
    servers::set_project_servers(&mut conn, acme.id, acme.project, &[])
        .await
        .unwrap();
    assert!(
        tools::projects_using(&mut *conn, post)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        tools::of_project(&mut *conn, acme.project)
            .await
            .unwrap()
            .is_empty()
    );
    drop(conn);
    s.cleanup().await;
}

#[tokio::test]
async fn tool_access_restricts_members_service_accounts_and_keys() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let x = server(&s.pg, acme.id, "x", "none").await;
    let other = server(&s.pg, acme.id, "other", "none").await;
    let schema = json!({ "type": "object" });
    let (post, search, elsewhere) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    for (id, srv, name, hash) in [
        (post, x, "create_post", 1u8),
        (search, x, "search", 2),
        (elsewhere, other, "e", 3),
    ] {
        tools::insert(
            &s.pg,
            id,
            srv,
            acme.id,
            &definition(name, &schema, &[hash; 32]),
            None,
            at(0),
        )
        .await
        .unwrap();
    }
    let mut conn = s.pg.acquire().await.unwrap();
    servers::set_project_servers(&mut conn, acme.id, acme.project, &[x])
        .await
        .unwrap();
    let ada = user(&s.pg, acme.id).await;
    let (membership, account, key) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    exec(
        &s.pg,
        "INSERT INTO project_memberships (id, project_id, user_id, preset) VALUES ($1, $2, $3, 'developer')",
        &[membership, acme.project, ada],
    )
    .await;
    exec(
        &s.pg,
        "INSERT INTO service_accounts (id, project_id, name, preset) VALUES ($1, $2, 'Bot', 'developer')",
        &[account, acme.project],
    )
    .await;
    sqlx::query(
        "INSERT INTO api_keys (id, org_id, project_id, user_id, name, secret_hash, secret_prefix, secret_last4, \
         inference, platform, tools, created_by_kind) \
         VALUES ($1, $2, $3, $4, 'k', '\\x02', 'sk-kso-', 'abcd', false, false, true, 'system')",
    )
    .bind(key)
    .bind(acme.id)
    .bind(acme.project)
    .bind(ada)
    .execute(&s.pg)
    .await
    .unwrap();

    for subject in [
        Subject::Membership(membership),
        Subject::ServiceAccount(account),
        Subject::Key(key),
    ] {
        assert_eq!(
            tool_access::access(&mut conn, acme.project, subject)
                .await
                .unwrap(),
            None
        );
    }
    let only_post: BTreeSet<Uuid> = [post, elsewhere].into();
    assert!(
        tool_access::set_access(
            &mut conn,
            acme.project,
            Subject::Membership(membership),
            Some(&only_post)
        )
        .await
        .unwrap()
    );
    assert!(
        tool_access::set_access(
            &mut conn,
            acme.project,
            Subject::Key(key),
            Some(&BTreeSet::new())
        )
        .await
        .unwrap()
    );
    assert!(
        !tool_access::set_access(&mut conn, acme.project, Subject::Key(Uuid::now_v7()), None)
            .await
            .unwrap()
    );
    assert_eq!(
        tool_access::access(&mut conn, acme.project, Subject::Membership(membership))
            .await
            .unwrap(),
        Some([post].into())
    );
    assert_eq!(
        tool_access::access(&mut conn, acme.project, Subject::Key(key))
            .await
            .unwrap(),
        Some(BTreeSet::new())
    );

    let map = tool_access::project_access(&mut conn, acme.project)
        .await
        .unwrap();
    assert_eq!(map.users.get(&ada), Some(&[post].into()));
    assert_eq!(map.keys.get(&key), Some(&BTreeSet::new()));
    assert!(map.open.contains(&account));
    assert!(map.allows(Some(ada), None, None, post));
    assert!(!map.allows(Some(ada), None, None, search));
    assert!(!map.allows(Some(ada), None, Some(key), post));
    assert!(map.allows(None, Some(account), None, search));

    tool_access::set_access(
        &mut conn,
        acme.project,
        Subject::ServiceAccount(account),
        Some(&[search].into()),
    )
    .await
    .unwrap();
    tool_access::clear_service_account(&mut *conn, account)
        .await
        .unwrap();
    assert_eq!(
        tool_access::access(&mut conn, acme.project, Subject::ServiceAccount(account))
            .await
            .unwrap(),
        Some(BTreeSet::new())
    );
    tool_access::set_access(
        &mut conn,
        acme.project,
        Subject::Membership(membership),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        tool_access::access(&mut conn, acme.project, Subject::Membership(membership))
            .await
            .unwrap(),
        None
    );
    drop(conn);
    s.cleanup().await;
}

#[tokio::test]
async fn portal_grants_and_tokens_are_bound_to_one_portal() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    let acme = org(&s.pg, "Acme").await;
    let ada = user(&s.pg, acme.id).await;
    let client = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_clients (id, name, redirect_uris, created_at, metadata_url) \
         VALUES ($1, 'Claude', ARRAY['https://claude.ai/api/mcp/auth_callback'], now(), 'https://claude.ai/oauth/client.json')",
    )
    .bind(client)
    .execute(&s.pg)
    .await
    .unwrap();
    let grant = Uuid::now_v7();
    grants::insert(
        &s.pg,
        &NewGrant {
            id: grant,
            user_id: ada,
            org_id: acme.id,
            project_id: acme.project,
            client_id: client,
            redirect_uri: "https://claude.ai/api/mcp/auth_callback",
            sign_in_method: "password",
            second_factor: false,
            oidc_org_id: None,
            now: at(0),
        },
    )
    .await
    .unwrap();
    let listed = grants::of_project(&s.pg, acme.project).await.unwrap();
    assert_eq!(
        (listed.len(), listed[0].client_name.as_str()),
        (1, "Claude")
    );
    assert_eq!(grants::of_user(&s.pg, ada).await.unwrap().len(), 1);
    assert!(
        grants::get(&s.pg, acme.other_project, grant)
            .await
            .unwrap()
            .is_none()
    );
    grants::touch(&s.pg, grant, at(1)).await.unwrap();
    assert_eq!(
        grants::get(&s.pg, acme.project, grant)
            .await
            .unwrap()
            .unwrap()
            .last_used_at,
        Some(at(1))
    );

    let resource = format!("https://kaeso.example.com/mcp/{}", acme.project);
    let family = Uuid::now_v7();
    let token = |kind, hash: &'static [u8], code: bool| NewToken {
        id: Uuid::now_v7(),
        grant_id: grant,
        kind,
        token_hash: hash,
        family,
        resource: &resource,
        redirect_uri: code.then_some("https://claude.ai/api/mcp/auth_callback"),
        code_challenge: code.then_some("challenge"),
        now: at(0),
        expires_at: Some(at(10)),
    };
    let mut conn = s.pg.acquire().await.unwrap();
    grants::insert_token(&mut *conn, &token(TokenKind::Code, b"code", true))
        .await
        .unwrap();
    grants::insert_token(&mut *conn, &token(TokenKind::Access, b"access", false))
        .await
        .unwrap();
    grants::insert_token(&mut *conn, &token(TokenKind::Refresh, b"refresh", false))
        .await
        .unwrap();

    let spent = grants::spend(&mut conn, TokenKind::Code, b"code", at(1))
        .await
        .unwrap()
        .unwrap();
    assert!(spent.fresh);
    assert_eq!(
        (spent.resource.as_str(), spent.client_id),
        (resource.as_str(), client)
    );
    assert!(
        !grants::spend(&mut conn, TokenKind::Code, b"code", at(2))
            .await
            .unwrap()
            .unwrap()
            .fresh
    );
    assert!(
        grants::spend(&mut conn, TokenKind::Access, b"code", at(2))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        grants::spend(&mut conn, TokenKind::Code, b"code", at(11))
            .await
            .unwrap()
            .is_none()
    );

    let auth = grants::authenticate(&mut *conn, b"access", at(1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            auth.project_id,
            auth.resource.as_str(),
            auth.user_state.as_str()
        ),
        (acme.project, resource.as_str(), "active")
    );
    assert!(
        grants::authenticate(&mut *conn, b"access", at(10))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        grants::authenticate(&mut *conn, b"refresh", at(1))
            .await
            .unwrap()
            .is_none()
    );

    assert_eq!(
        grants::revoke_token(&mut *conn, b"refresh", at(2))
            .await
            .unwrap(),
        Some(family)
    );
    assert_eq!(
        grants::revoke_token(&mut *conn, b"code", at(2))
            .await
            .unwrap(),
        None
    );
    grants::revoke_family(&mut *conn, family, at(3))
        .await
        .unwrap();
    assert!(
        grants::authenticate(&mut *conn, b"access", at(4))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(grants::sweep(&mut *conn, at(4)).await.unwrap(), 3);

    grants::insert_token(&mut *conn, &token(TokenKind::Access, b"again", false))
        .await
        .unwrap();
    assert!(
        grants::revoke(&mut conn, acme.project, grant, at(5))
            .await
            .unwrap()
    );
    assert!(
        !grants::revoke(&mut conn, acme.project, grant, at(6))
            .await
            .unwrap()
    );
    assert!(
        grants::authenticate(&mut *conn, b"again", at(5))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        grants::of_project(&mut *conn, acme.project)
            .await
            .unwrap()
            .is_empty()
    );

    let second = Uuid::now_v7();
    grants::insert(
        &mut *conn,
        &NewGrant {
            id: second,
            user_id: ada,
            org_id: acme.id,
            project_id: acme.project,
            client_id: client,
            redirect_uri: "http://localhost:6274/callback",
            sign_in_method: "password",
            second_factor: false,
            oidc_org_id: None,
            now: at(7),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        grants::revoke_of_user(&mut *conn, ada, at(8))
            .await
            .unwrap(),
        [second]
    );
    drop(conn);
    exec(
        &s.pg,
        "DELETE FROM org_memberships WHERE user_id = $1",
        &[ada],
    )
    .await;
    assert!(
        grants::get(&s.pg, acme.project, second)
            .await
            .unwrap()
            .is_none()
    );
    s.cleanup().await;
}

#[tokio::test]
async fn tool_calls_are_logged_and_deleted_with_the_request_log() {
    let s = TestServices::start().await;
    kaeso_store::pg::migrate(&s.pg).await.unwrap();
    s.clickhouse.migrate(&s.pg).await.unwrap();
    let (org, project, other) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let row = |id: Uuid, project: Uuid, minute: i64| ToolCallRow {
        id,
        org_id: org,
        project_id: project,
        server_id: Uuid::now_v7(),
        started_at: at(minute),
        tool: "x__create_post".to_owned(),
        upstream_tool: "create_post".to_owned(),
        caller_kind: "grant".to_owned(),
        caller_id: Uuid::now_v7(),
        key_id: None,
        grant_id: Some(Uuid::now_v7()),
        client_name: "Claude".to_owned(),
        protocol_version: "2026-07-28".to_owned(),
        source: "portal".to_owned(),
        status: "ok".to_owned(),
        error_type: String::new(),
        error_message: String::new(),
        duration_ms: 120,
        upstream_ms: Some(110),
        request_bytes: 64,
        response_bytes: 512,
        has_content: true,
        expires_at: at(0) + Duration::days(30),
    };
    let (first, second, third) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let rows = [
        row(first, project, 0),
        row(second, project, 5),
        row(third, other, 0),
    ];
    tool_calls::insert_calls(&s.clickhouse, &rows)
        .await
        .unwrap();
    tool_calls::insert_calls(&s.clickhouse, &[]).await.unwrap();
    let content = ToolCallContentRow {
        id: first,
        org_id: org,
        project_id: project,
        started_at: at(0),
        arguments: r#"{"text":"hi"}"#.to_owned(),
        result: r#"{"content":[]}"#.to_owned(),
        expires_at: at(0) + Duration::days(30),
    };
    tool_calls::insert_contents(&s.clickhouse, [&content])
        .await
        .unwrap();
    tool_calls::insert_contents(&s.clickhouse, [])
        .await
        .unwrap();

    assert_eq!(
        tool_calls::get(&s.clickhouse, org, project, first)
            .await
            .unwrap(),
        Some(rows[0].clone())
    );
    assert_eq!(
        tool_calls::get(&s.clickhouse, org, other, first)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        tool_calls::content(&s.clickhouse, org, project, first)
            .await
            .unwrap(),
        Some(content)
    );
    let mut counted = tool_calls::counts(&s.clickhouse, org, project, at(-1))
        .await
        .unwrap();
    counted.sort_by_key(|c| c.last);
    assert_eq!(
        counted
            .iter()
            .map(|c| (c.server_id, c.upstream_tool.as_str(), c.calls, c.last))
            .collect::<Vec<_>>(),
        [
            (rows[0].server_id, "create_post", 1, at(0)),
            (rows[1].server_id, "create_post", 1, at(5)),
        ]
    );
    assert_eq!(
        tool_calls::counts(&s.clickhouse, org, project, at(1))
            .await
            .unwrap()
            .len(),
        1,
        "calls before `since` aren't counted"
    );

    // ClickHouse applies lightweight deletes in the background; wait for each to land.
    async fn gone<F, Fut>(check: F)
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        for _ in 0..100 {
            if check().await {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("the rows were not deleted");
    }
    let ch = &s.clickhouse;
    logs::delete_range(ch, org, Some(project), at(-1), at(1), true)
        .await
        .unwrap();
    gone(|| async {
        tool_calls::content(ch, org, project, first)
            .await
            .unwrap()
            .is_none()
    })
    .await;
    assert!(
        tool_calls::get(ch, org, project, first)
            .await
            .unwrap()
            .is_some()
    );
    logs::delete_range(ch, org, Some(project), at(-1), at(1), false)
        .await
        .unwrap();
    gone(|| async {
        tool_calls::get(ch, org, project, first)
            .await
            .unwrap()
            .is_none()
    })
    .await;
    assert!(
        tool_calls::get(ch, org, project, second)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        tool_calls::get(ch, org, other, third)
            .await
            .unwrap()
            .is_some()
    );
    logs::delete_org(ch, org).await.unwrap();
    gone(|| async {
        tool_calls::get(ch, org, project, second)
            .await
            .unwrap()
            .is_none()
            && tool_calls::get(ch, org, other, third)
                .await
                .unwrap()
                .is_none()
    })
    .await;
    s.cleanup().await;
}
