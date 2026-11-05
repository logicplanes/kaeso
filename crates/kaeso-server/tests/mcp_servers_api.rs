#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{App, ORIGIN, Owner, uuid};
use kaeso_testkit::mcp::{MockMcp, OAuth, Options, default_tools};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};
use url::Url;

const LOOPBACK: (&str, &str) = ("KAESO_MCP_SERVERS_ALLOWED_NETWORKS", "127.0.0.0/8");

fn servers(owner: &Owner) -> String {
    format!("/orgs/{}/mcp-servers", owner.org_id)
}

async fn add(app: &App, owner: &Owner, body: Value) -> Value {
    let r = app.post(&servers(owner), body, Some(&owner.cookie)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn settled(app: &App) {
    app.state.in_flight.settled().await;
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

fn named<'a>(list: &'a Value, key: &str, name: &str) -> &'a Value {
    list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t[key] == name)
        .unwrap()
}

async fn audited(app: &App, owner: &Owner) -> Vec<String> {
    let r = app
        .get(
            &format!("/orgs/{}/audit-log?limit=100", owner.org_id),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn an_admin_adds_a_server_turns_its_tools_on_and_reviews_a_change() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;

    let probe = app
        .post(
            &format!("{}/probe", servers(&owner)),
            json!({ "url": mock.url() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(probe.status, StatusCode::OK, "{:?}", probe.body);
    assert_eq!(probe.body["problem"], Value::Null);
    assert_eq!(probe.body["protocol_version"], "2026-07-28");
    assert_eq!(probe.body["sign_in"]["method"], "none");
    assert_eq!(probe.body["handle"], "mock-x");
    assert_eq!(probe.body["tools"].as_array().unwrap().len(), 3);
    assert_eq!(
        probe.body["tools"][0]["annotations"]["destructiveHint"],
        true
    );

    let created = add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project] }),
    )
    .await;
    assert_eq!(created["change_policy"], "hold");
    assert_eq!(created["projects"], json!([project]));
    assert_eq!(created["created_by"]["id"], owner.user_id);
    let server = created["id"].as_str().unwrap().to_owned();
    settled(&app).await;
    let r = app
        .post(
            &servers(&owner),
            json!({ "handle": "x", "name": "Again", "url": mock.url(), "auth_method": "none" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "This organization already has an MCP server `x`."
    );

    let listed = app.get(&servers(&owner), &owner.cookie).await;
    let x = named(&listed.body, "handle", "x");
    assert_eq!(x["status"], "ok");
    assert_eq!(x["tools"], json!({ "total": 3, "changed": 0 }));
    assert_eq!(x["server_info"]["name"], "mock-x");
    let base = format!("{}/{server}", servers(&owner));
    let tools = app.get(&format!("{base}/tools"), &owner.cookie).await;
    let post = named(&tools.body, "name", "create_post").clone();
    assert_eq!(post["exposed_name"], "x__create_post");
    assert_eq!(post["state"], "active");
    assert_eq!(post["hash"].as_str().unwrap().len(), 64);
    assert_eq!(post["hash"], post["approved_hash"]);

    let at_project = format!("/orgs/{}/projects/{project}", owner.org_id);
    let used = app
        .get(&format!("{at_project}/mcp-servers"), &owner.cookie)
        .await;
    assert_eq!(used.body["data"][0]["handle"], "x");
    assert_eq!(used.body["data"][0]["connected"], true);
    assert_eq!(used.body["data"][0]["tools_on"], 0);
    let on = app
        .call(
            Method::PATCH,
            &format!("{at_project}/tools"),
            Some(json!({ "tools": [{ "id": post["id"], "enabled": true }] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.body);
    let p = named(&on.body, "name", "create_post");
    assert_eq!(
        (p["enabled"].clone(), p["offered"].clone()),
        (json!(true), json!(true))
    );

    mock.set_tools(with_description(
        "create_post",
        "Posts to X and reads your direct messages.",
    ));
    let refreshed = app
        .post(&format!("{base}/refresh"), json!({}), Some(&owner.cookie))
        .await;
    assert_eq!(refreshed.status, StatusCode::OK, "{:?}", refreshed.body);
    assert_eq!(refreshed.body["held"], json!(["create_post"]));
    assert_eq!(refreshed.body["server"]["tools"]["changed"], 1);
    let tools = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    let p = named(&tools.body, "name", "create_post");
    assert_eq!(
        (p["state"].clone(), p["offered"].clone()),
        (json!("changed"), json!(false))
    );

    let tool = post["id"].as_str().unwrap();
    let change = app
        .get(&format!("{base}/tools/{tool}/change"), &owner.cookie)
        .await;
    assert_eq!(
        change.body["approved"]["description"],
        "Posts a message to X as the signed-in account."
    );
    assert_eq!(
        change.body["current"]["description"],
        "Posts to X and reads your direct messages."
    );
    let r = app
        .post(
            &format!("{base}/tools/{tool}/approve"),
            json!({ "hash": post["hash"] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "The tool changed again since you reviewed it; review it again."
    );
    let r = app
        .post(
            &format!("{base}/tools/{tool}/approve"),
            json!({ "hash": change.body["tool"]["hash"] }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["state"], "active");
    let tools = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    assert_eq!(named(&tools.body, "name", "create_post")["offered"], true);

    mock.set_tools(with_description("create_post", "Changed once more."));
    app.post(&format!("{base}/refresh"), json!({}), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::OK);
    let r = app
        .post(
            &format!("{base}/tools/{tool}/reject"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let tools = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    assert_eq!(named(&tools.body, "name", "create_post")["enabled"], false);
    let search = named(&tools.body, "name", "search_posts")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = app
        .post(
            &format!("{base}/tools/{search}/reject"),
            json!({}),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        r.body["error"]["message"],
        "This tool has no change waiting for review."
    );

    let r = app
        .raw(
            Request::builder()
                .method(Method::PATCH)
                .uri(format!("/platform/v1{base}"))
                .header(
                    header::COOKIE,
                    format!("__Host-kaeso_session={}", owner.cookie),
                )
                .header(header::ORIGIN, ORIGIN)
                .header(header::IF_MATCH, "\"99\"")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"handle":"x-two"}"#))
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED, "{:?}", r.body);
    let r = app
        .call(
            Method::PATCH,
            &base,
            Some(json!({ "handle": "x-two", "change_policy": "accept" })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["change_policy"], "accept");
    let tools = app.get(&format!("{base}/tools"), &owner.cookie).await;
    assert_eq!(
        named(&tools.body, "name", "create_post")["exposed_name"],
        "x_two__create_post"
    );

    let r = app
        .call(Method::DELETE, &base, None, Some(&owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = app.get(&base, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(
        r.body["error"]["message"],
        "No such MCP server, or you cannot see it."
    );
    let used = app
        .get(&format!("{at_project}/mcp-servers"), &owner.cookie)
        .await;
    assert_eq!(used.body["data"], json!([]));

    let actions = audited(&app, &owner).await;
    for action in [
        "mcp_server.create",
        "project.tools_update",
        "tool.approve",
        "tool.reject",
        "mcp_server.update",
        "mcp_server.delete",
    ] {
        assert!(
            actions.iter().any(|a| a == action),
            "{action} in {actions:?}"
        );
    }
}

#[tokio::test]
async fn addresses_and_fields_are_checked_before_anything_is_stored() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let base = json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none" });
    for (change, param, message) in [
        (
            json!({}),
            "url",
            "`url` points to an address this install does not allow.",
        ),
        (
            json!({ "url": "ftp://mcp.example.com" }),
            "url",
            "`url` must start with `https://`, or `http://` for an allowed network.",
        ),
        (
            json!({ "url": "https://me:pw@mcp.example.com/mcp" }),
            "url",
            "`url` can't hold a user name or password; use a secret header instead.",
        ),
        (
            json!({ "handle": "X Y" }),
            "handle",
            "`handle` must be 1 to 24 lowercase letters, digits and hyphens.",
        ),
        (
            json!({ "name": " " }),
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        ),
        (
            json!({ "credential_mode": "per_user" }),
            "credential_mode",
            "A server without sign-in has no accounts; use `shared`.",
        ),
        (
            json!({ "scopes": ["posts.read"] }),
            "auth_method",
            "`oauth_client` and `scopes` are only for `oauth` servers.",
        ),
        (
            json!({ "projects": ["proj_00000000000000000000000000"] }),
            "projects",
            "`proj_00000000000000000000000000` is not a project of this organization.",
        ),
    ] {
        let mut body = base.clone();
        for (k, v) in change.as_object().unwrap() {
            body[k] = v.clone();
        }
        if param != "url" {
            body["url"] = json!("https://mcp.example.com/mcp");
        }
        let r = app.post(&servers(&owner), body, Some(&owner.cookie)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{change}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{change}");
        assert_eq!(r.body["error"]["message"], message, "{change}");
    }
    let r = app
        .post(
            &format!("{}/probe", servers(&owner)),
            json!({ "url": mock.url() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        mock.seen().is_empty(),
        "nothing reached the refused address"
    );
    assert!(
        app.get(&servers(&owner), &owner.cookie).await.body["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_header_server_is_connected_with_a_secret_kaeso_never_shows() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        token: Some("s3cret-token-value".into()),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let probe = format!("{}/probe", servers(&owner));

    let r = app
        .post(&probe, json!({ "url": mock.url() }), Some(&owner.cookie))
        .await;
    assert_eq!(r.body["sign_in"]["method"], "header");
    assert_eq!(r.body["tools"], Value::Null);
    let r = app
        .post(
            &probe,
            json!({ "url": mock.url(), "header": { "name": "Authorization", "value": "Bearer wrong" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["problem"], "header_refused");
    let r = app
        .post(
            &probe,
            json!({ "url": mock.url(), "header": { "name": "Authorization", "value": "Bearer s3cret-token-value" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["problem"], Value::Null);
    assert_eq!(r.body["tools"].as_array().unwrap().len(), 3);

    let created = add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "header" }),
    )
    .await;
    assert_eq!(created["connection"], Value::Null);
    let connection = format!(
        "{}/{}/connection",
        servers(&owner),
        created["id"].as_str().unwrap()
    );
    for (body, param, message) in [
        (
            json!({ "header": { "name": "Content-Type", "value": "x" } }),
            "header.name",
            "Kaeso sets this header itself; choose another one.",
        ),
        (
            json!({ "header": { "name": "Authorization", "value": "a\nb" } }),
            "header.value",
            "`header.value` must be printable ASCII without line breaks.",
        ),
        (
            json!({ "oauth": {} }),
            "oauth",
            "This server takes a secret header; send `header`.",
        ),
    ] {
        let r = app.post(&connection, body, Some(&owner.cookie)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        assert_eq!(
            (
                r.body["error"]["param"].as_str(),
                r.body["error"]["message"].as_str()
            ),
            (Some(param), Some(message))
        );
    }
    let r = app
        .post(
            &connection,
            json!({ "header": { "name": "Authorization", "value": "Bearer s3cret-token-value" } }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["redirect_url"], Value::Null);
    let c = &r.body["server"]["connection"];
    assert_eq!(
        (
            c["kind"].as_str(),
            c["header_name"].as_str(),
            c["last4"].as_str()
        ),
        (Some("header"), Some("Authorization"), Some("alue"))
    );
    assert_eq!(r.body["server"]["status"], "ok");
    assert_eq!(r.body["server"]["tools"]["total"], 3);
    let listed = app.get(&servers(&owner), &owner.cookie).await;
    let audit = app
        .get(
            &format!("/orgs/{}/audit-log?limit=100", owner.org_id),
            &owner.cookie,
        )
        .await;
    for shown in [&r.body, &listed.body, &audit.body] {
        assert!(!shown.to_string().contains("s3cret-token-value"));
    }
    assert!(
        audited(&app, &owner)
            .await
            .contains(&"mcp_server.connect".to_owned())
    );
}

#[tokio::test]
async fn an_oauth_sign_in_starts_in_the_api_and_finishes_in_the_browser() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options {
        oauth: Some(OAuth {
            cimd: true,
            label: Some("@kaesoapp".into()),
            ..OAuth::default()
        }),
        ..Options::default()
    })
    .await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let probe = app
        .post(
            &format!("{}/probe", servers(&owner)),
            json!({ "url": mock.url() }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(probe.body["sign_in"]["method"], "oauth");
    assert_eq!(probe.body["sign_in"]["client_metadata_documents"], true);
    assert_eq!(
        probe.body["sign_in"]["scopes"],
        json!(["posts.read", "posts.write"])
    );

    let created = add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "oauth" }),
    )
    .await;
    assert_eq!(created["oauth"]["registration"], "cimd");
    let server = created["id"].as_str().unwrap().to_owned();
    let r = app
        .post(
            &format!("{}/{server}/connection", servers(&owner)),
            json!({ "oauth": {} }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let start = r.body["redirect_url"].as_str().unwrap();
    assert!(start.starts_with(&mock.base()), "{start}");

    let back = Url::parse(&mock.authorize(start)).unwrap();
    assert_eq!(back.origin().ascii_serialization(), ORIGIN);
    let r = app
        .raw(
            Request::builder()
                .uri(format!("{}?{}", back.path(), back.query().unwrap()))
                .header(
                    header::COOKIE,
                    format!("__Host-kaeso_session={}", owner.cookie),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(
        r.headers[header::LOCATION],
        format!(
            "{ORIGIN}/orgs/{}/mcp-servers?server={server}&result=connected",
            owner.org_id
        )
        .as_str()
    );
    settled(&app).await;
    let after = app
        .get(&format!("{}/{server}", servers(&owner)), &owner.cookie)
        .await;
    assert_eq!(after.body["connection"]["account"], "@kaesoapp");
    assert_eq!(after.body["connection"]["kind"], "oauth");
    assert_eq!(after.body["status"], "ok");
    assert_eq!(after.body["tools"]["total"], 3);
    assert!(
        audited(&app, &owner)
            .await
            .contains(&"mcp_server.connect".to_owned())
    );
}

#[tokio::test]
async fn rights_decide_who_sees_and_changes_servers_and_tools() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project] }),
    )
    .await;
    settled(&app).await;
    let (dev, _) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "dev@example.com", "role": "developer" }),
        "Dev",
    )
    .await;
    let (viewer, _) = common::join_first_project(
        &app,
        &owner,
        json!({ "email": "member@example.com", "role": "member" }),
        "Viewer",
    )
    .await;
    let tools = format!("/orgs/{}/projects/{project}/tools", owner.org_id);

    assert_eq!(app.get(&servers(&owner), &dev).await.status, StatusCode::OK);
    let r = app
        .post(
            &servers(&owner),
            json!({ "handle": "y", "name": "Y", "url": mock.url(), "auth_method": "none" }),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["right"], "mcp_servers.manage");
    let r = app.get(&tools, &dev).await;
    assert_eq!(r.status, StatusCode::OK);
    let id = r.body["data"][0]["id"].clone();
    let r = app
        .call(
            Method::PATCH,
            &tools,
            Some(json!({ "tools": [{ "id": id, "enabled": true }] })),
            Some(&dev),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    let r = app.get(&servers(&owner), &viewer).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get(&tools, &viewer).await.status, StatusCode::OK);
    let r = app
        .call(
            Method::PATCH,
            &tools,
            Some(json!({ "tools": [{ "id": id, "enabled": false }] })),
            Some(&viewer),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["error"]["right"], "tools.manage");
}

#[tokio::test]
async fn tool_access_narrows_who_may_call_which_tool() {
    let s = TestServices::start().await;
    let mock = MockMcp::start(Options::default()).await;
    let app = App::start_with(&s, &[LOOPBACK]).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    add(
        &app,
        &owner,
        json!({ "handle": "x", "name": "X", "url": mock.url(), "auth_method": "none", "projects": [project] }),
    )
    .await;
    settled(&app).await;
    let at_project = format!("/orgs/{}/projects/{project}", owner.org_id);
    let listed = app.get(&format!("{at_project}/tools"), &owner.cookie).await;
    let post = named(&listed.body, "name", "create_post")["id"].clone();
    let search = named(&listed.body, "name", "search_posts")["id"].clone();
    let me = named(&listed.body, "name", "get_me")["id"].clone();
    app.call(
        Method::PATCH,
        &format!("{at_project}/tools"),
        Some(json!({ "tools": [{ "id": post, "enabled": true }, { "id": search, "enabled": true }] })),
        Some(&owner.cookie),
    )
    .await
    .expect_status(StatusCode::OK);

    let mine = format!("{at_project}/tool-access/{}", owner.user_id);
    let r = app.get(&mine, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], true);
    let mut effective: Vec<Value> = r.body["effective"].as_array().unwrap().clone();
    effective.sort_by_key(|v| v.as_str().unwrap().to_owned());
    let mut both = vec![post.clone(), search.clone()];
    both.sort_by_key(|v| v.as_str().unwrap().to_owned());
    assert_eq!(effective, both, "only tools that are on count");

    let r = app
        .call(
            Method::PUT,
            &mine,
            Some(json!({ "tools": [post, me] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["all"], false);
    assert_eq!(r.body["effective"], json!([post]));

    let key = app
        .post(
            &format!("{at_project}/keys"),
            json!({ "name": "ci" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(key.status, StatusCode::CREATED, "{:?}", key.body);
    let key_access = format!(
        "{at_project}/tool-access/{}",
        key.body["id"].as_str().unwrap()
    );
    let r = app
        .call(
            Method::PUT,
            &key_access,
            Some(json!({ "tools": [search] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        r.body["error"]["message"],
        format!(
            "`{}` is not among the tools the key's member may call.",
            search.as_str().unwrap()
        )
    );
    let r = app
        .call(
            Method::PUT,
            &key_access,
            Some(json!({ "all": true, "tools": [] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(
        r.body["error"]["message"],
        "Send either `all: true` or a `tools` list."
    );
    let r = app.get(&key_access, &owner.cookie).await;
    assert_eq!(
        r.body["effective"],
        json!([post]),
        "a key stays within its member's tools"
    );

    let account = app
        .post(
            &format!("{at_project}/service-accounts"),
            json!({ "name": "deployer", "role": "developer" }),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(account.status, StatusCode::CREATED, "{:?}", account.body);
    let r = app
        .call(
            Method::PUT,
            &format!(
                "{at_project}/tool-access/{}",
                account.body["id"].as_str().unwrap()
            ),
            Some(json!({ "tools": [search] })),
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.body["effective"], json!([search]));

    let r = app
        .get(
            &format!("{at_project}/tool-access/usr_00000000000000000000000000"),
            &owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(
        r.body["error"]["message"],
        "No such member, service account or key, or you cannot see it."
    );
    assert!(
        audited(&app, &owner)
            .await
            .contains(&"tool_access.update".to_owned())
    );
}

#[tokio::test]
async fn connected_portal_clients_are_listed_and_disconnected() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let project = common::first_project(&app, &owner).await;
    let now = app.state.clock.now();
    let client = uuid::Uuid::now_v7();
    kaeso_store::pg::agents::insert_client(
        &app.state.pg,
        &kaeso_store::pg::agents::Client {
            id: client,
            name: "Claude".into(),
            redirect_uris: vec!["https://claude.example/callback".into()],
            software_id: None,
            software_version: None,
            created_at: now,
        },
    )
    .await
    .unwrap();
    let grant = uuid::Uuid::now_v7();
    kaeso_store::pg::portal_grants::insert(
        &app.state.pg,
        &kaeso_store::pg::portal_grants::NewGrant {
            id: grant,
            user_id: uuid(&owner.user_id),
            org_id: uuid(&owner.org_id),
            project_id: uuid(&project),
            client_id: client,
            redirect_uri: "https://claude.example/callback",
            now,
        },
    )
    .await
    .unwrap();
    let base = format!("/orgs/{}/projects/{project}/portal-grants", owner.org_id);
    let r = app.get(&base, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["data"][0]["client_name"], "Claude");
    assert_eq!(r.body["data"][0]["user"], owner.user_id);
    let id = r.body["data"][0]["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("grant_"));
    let r = app
        .call(
            Method::DELETE,
            &format!("{base}/{id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&base, &owner.cookie).await.body["data"], json!([]));
    let r = app
        .call(
            Method::DELETE,
            &format!("{base}/{id}"),
            None,
            Some(&owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(
        r.body["error"]["message"],
        "No such portal client, or you cannot see it."
    );
    assert!(
        audited(&app, &owner)
            .await
            .contains(&"portal_grant.revoke".to_owned())
    );
}
