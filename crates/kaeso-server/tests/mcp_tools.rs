#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use common::{App, Owner, uuid};
use kaeso_mcp::canonical;
use kaeso_server::services::mcp_tools;
use kaeso_store::pg::mcp_servers::{self as servers, NewServer, ServerRow};
use kaeso_store::pg::mcp_tools as tools;
use kaeso_testkit::mcp::{MockMcp, Options, default_tools};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn server(app: &App, owner: &Owner, url: &str, auth: &str, policy: &str) -> ServerRow {
    let id = uuid::Uuid::now_v7();
    let org = uuid(&owner.org_id);
    let handle = format!("s{}", &id.simple().to_string()[24..]);
    servers::insert(
        &app.state.pg,
        &NewServer {
            id,
            org_id: org,
            handle: &handle,
            name: "X",
            url,
            auth_method: auth,
            credential_mode: "shared",
            oauth_registration: (auth == "oauth").then_some("cimd"),
            scopes: &[],
            change_policy: policy,
            created_by_kind: "system",
            created_by_id: None,
        },
    )
    .await
    .unwrap();
    servers::get(&app.state.pg, org, id).await.unwrap().unwrap()
}

fn with_description(name: &str, description: &str) -> Vec<Value> {
    default_tools()
        .into_iter()
        .map(|mut t| {
            if t["name"] == name {
                t["description"] = json!(description);
            }
            t
        })
        .collect()
}

async fn tool(app: &App, row: &ServerRow, name: &str) -> tools::ToolRow {
    tools::of_server(&app.state.pg, row.id)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap()
}

#[tokio::test]
async fn a_changed_definition_is_held_until_someone_approves_it() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "none", "hold").await;

    let first = mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(first.added, ["create_post", "search_posts", "get_me"]);
    let checked = servers::get(&app.state.pg, row.org_id, row.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checked.status, "ok");
    assert_eq!(checked.protocol_version.as_deref(), Some("2026-07-28"));
    assert_eq!(checked.server_info.unwrap()["name"], "mock-x");
    let post = tool(&app, &row, "create_post").await;
    assert!(post.is_approved());
    assert_eq!(post.annotations.as_ref().unwrap()["destructiveHint"], true);
    assert_eq!(
        post.definition_hash,
        canonical::definition_hash(&default_tools()[0]).to_vec()
    );
    let me = tool(&app, &row, "get_me").await;
    assert_eq!(
        me.meta.unwrap()["io.modelcontextprotocol/ui"]["resourceUri"],
        "ui://x/me"
    );

    let again = mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(again, mcp_tools::Synced::default());

    mock.set_tools(with_description(
        "create_post",
        "Posts to X, and reads your direct messages.",
    ));
    let changed = mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(changed.changed, ["create_post"]);
    let held = tool(&app, &row, "create_post").await;
    assert_eq!(held.state, "changed");
    assert!(!held.is_approved());
    assert_eq!(
        held.description,
        "Posts to X, and reads your direct messages."
    );
    assert_eq!(
        held.approved.as_ref().unwrap()["description"],
        "Posts a message to X as the signed-in account."
    );

    mock.set_tools(default_tools());
    mcp_tools::sync(&app.state, &row).await.unwrap();
    assert!(
        tool(&app, &row, "create_post").await.is_approved(),
        "going back to the approved definition releases it"
    );

    mock.set_tools(with_description("create_post", "Posts to X."));
    mcp_tools::sync(&app.state, &row).await.unwrap();
    let held = tool(&app, &row, "create_post").await;
    assert!(
        !mcp_tools::approve(&app.state, row.org_id, held.id, &[0; 32])
            .await
            .unwrap()
    );
    assert!(
        mcp_tools::approve(&app.state, row.org_id, held.id, &held.definition_hash)
            .await
            .unwrap()
    );
    let approved = tool(&app, &row, "create_post").await;
    assert!(approved.is_approved());
    assert_eq!(approved.approved.unwrap()["description"], "Posts to X.");

    let mut fewer = default_tools();
    fewer.pop();
    mock.set_tools(fewer);
    assert_eq!(mcp_tools::sync(&app.state, &row).await.unwrap().removed, 1);
    assert_eq!(tool(&app, &row, "get_me").await.state, "removed");
    mock.set_tools(default_tools());
    mcp_tools::sync(&app.state, &row).await.unwrap();
    assert!(
        tool(&app, &row, "get_me").await.is_approved(),
        "a tool that comes back with its approved definition is active"
    );
}

#[tokio::test]
async fn a_server_that_accepts_changes_needs_no_review() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "none", "accept").await;
    mcp_tools::sync(&app.state, &row).await.unwrap();
    mock.set_tools(with_description("search_posts", "Finds posts."));
    let synced = mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(
        (synced.accepted.as_slice(), synced.changed.len()),
        (&["search_posts".to_owned()][..], 0)
    );
    let search = tool(&app, &row, "search_posts").await;
    assert!(search.is_approved());
    assert_eq!(search.approved.unwrap()["description"], "Finds posts.");
}

#[tokio::test]
async fn rejecting_a_change_switches_the_tool_off_where_it_was_on() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "none", "hold").await;
    mcp_tools::sync(&app.state, &row).await.unwrap();
    let projects = kaeso_store::pg::projects::ids_of_org(&app.state.pg, row.org_id).await;
    let project = projects.unwrap()[0];
    let mut conn = app.state.pg.acquire().await.unwrap();
    servers::set_project_servers(&mut conn, row.org_id, project, &[row.id])
        .await
        .unwrap();
    let post = tool(&app, &row, "create_post").await;
    tools::set_enabled(&mut *conn, project, &[post.id], true)
        .await
        .unwrap();
    drop(conn);

    assert_eq!(
        mcp_tools::reject(&app.state, row.org_id, post.id)
            .await
            .unwrap(),
        Some(0),
        "an approved tool has nothing to reject"
    );
    mock.set_tools(with_description("create_post", "Changed."));
    mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(
        mcp_tools::reject(&app.state, row.org_id, post.id)
            .await
            .unwrap(),
        Some(1)
    );
    assert!(
        tools::projects_using(&app.state.pg, post.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(tool(&app, &row, "create_post").await.state, "changed");
    assert_eq!(
        mcp_tools::reject(&app.state, row.org_id, uuid::Uuid::now_v7())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn tools_a_client_would_refuse_are_skipped() {
    let s = TestServices::start().await;
    let mut listed = default_tools();
    listed.extend([
        json!({ "name": "", "inputSchema": { "type": "object" } }),
        json!({ "name": "no_schema" }),
        json!({ "name": "get_me", "inputSchema": { "type": "object" } }),
        json!({
            "name": "bad_header",
            "inputSchema": { "type": "object", "properties": {
                "ids": { "type": "array", "items": { "type": "string", "x-mcp-header": "Id" } } } }
        }),
        json!({
            "name": "good_header",
            "inputSchema": { "type": "object", "properties": {
                "region": { "type": "string", "x-mcp-header": "Region" } } }
        }),
    ]);
    let mock = MockMcp::start(Options {
        tools: listed,
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let row = server(&app, &owner, &mock.url(), "none", "hold").await;
    let synced = mcp_tools::sync(&app.state, &row).await.unwrap();
    assert_eq!(
        synced.added,
        ["create_post", "search_posts", "get_me", "good_header"]
    );
    let skipped: Vec<&str> = synced.skipped.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(skipped, ["", "no_schema", "get_me", "bad_header"]);
    assert_eq!(
        synced.skipped[3].1,
        "`Id` sits where only `properties` may lead"
    );
}

#[tokio::test]
async fn servers_kaeso_cannot_reach_or_sign_in_to_are_marked() {
    let s = TestServices::start().await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let closed = server(&app, &owner, "http://127.0.0.1:9/mcp", "none", "hold").await;
    assert!(mcp_tools::sync(&app.state, &closed).await.is_err());
    let after = servers::get(&app.state.pg, closed.org_id, closed.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "unreachable");
    assert!(after.last_error.is_some());
    assert!(after.last_checked_at.is_some());

    let mock = MockMcp::start(Options::default()).await;
    let unsigned = server(&app, &owner, &mock.url(), "oauth", "hold").await;
    assert!(mcp_tools::sync(&app.state, &unsigned).await.is_err());
    let after = servers::get(&app.state.pg, unsigned.org_id, unsigned.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "signed_out");
    assert!(
        mock.seen().is_empty(),
        "nothing is sent without a connection"
    );
}

#[tokio::test]
async fn the_sync_job_asks_each_server_once_per_interval() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let a = server(&app, &owner, &mock.url(), "none", "hold").await;
    let b = server(&app, &owner, &mock.url(), "none", "hold").await;
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "mcp_sync")
        .unwrap();
    let discovers = || {
        mock.seen()
            .iter()
            .filter(|s| s.method == "server/discover")
            .count()
    };

    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary, "2 MCP servers synced, 0 not reached");
    assert_eq!(discovers(), 2);
    for row in [&a, &b] {
        assert_eq!(
            tools::of_server(&app.state.pg, row.id).await.unwrap().len(),
            3
        );
    }
    kaeso_server::jobs::run_once(&app.state, job).await.unwrap();
    app.clock.advance(time::Duration::minutes(10));
    kaeso_server::jobs::run_once(&app.state, job).await.unwrap();
    assert_eq!(
        discovers(),
        2,
        "nothing is due before the interval, less its spread"
    );
    app.clock.advance(time::Duration::minutes(6));
    let summary = kaeso_server::jobs::run_once(&app.state, job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary, "2 MCP servers synced, 0 not reached");
    assert_eq!(discovers(), 4);
}
