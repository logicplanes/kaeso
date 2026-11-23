#![allow(clippy::unwrap_used)]

use crate::common;

use std::time::Duration;

use axum::http::StatusCode;
use common::v1::Env;
use kaeso_core::sources::Counted;
use kaeso_server::services::observed::{self, Batch, ObservedRequest, ObservedToolCall};
use kaeso_store::pg::sources as store;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::str::FromStr;

async fn env() -> Env {
    let e = common::v1::env(None, None).await;
    e.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&e.app.state.pg)
        .await
        .unwrap();
    e
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn add(env: &Env, body: Value) -> Value {
    let r = env
        .app
        .post(
            &format!("{}/sources", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

async fn source_row(env: &Env, id: &str) -> store::Source {
    store::get(
        &env.app.state.pg,
        common::uuid(&env.project),
        common::uuid(id),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn flush(env: &Env) {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
}

fn haiku(id: &str, email: Option<&str>) -> ObservedRequest {
    ObservedRequest {
        external_id: id.into(),
        duration: Duration::from_millis(899),
        ttfb: Some(Duration::from_millis(749)),
        provider: Some("anthropic".into()),
        model: "claude-haiku-4-5".into(),
        status: 200,
        tokens: Counted {
            input: 2_000,
            output: 1_000,
            cache_read: 10_000,
            cache_write: 0,
            reasoning: 0,
        },
        reported_cost: Some(Decimal::from_str("0.5").unwrap()),
        client: Some("claude-cli/2.1.322".into()),
        email: email.map(str::to_owned),
        dialect: Some(kaeso_kpp::DialectId::AnthropicMessages),
        ..ObservedRequest::default()
    }
}

#[tokio::test]
async fn a_source_is_added_with_its_key_shown_once_and_listed_without_it() {
    let env = env().await;
    let made = add(
        &env,
        json!({ "kind": "claude_code", "name": "Claude Code, Ada's laptop" }),
    )
    .await;
    let key = made["key"].as_str().unwrap();
    assert!(
        key.starts_with("sk-kso-src-") && key.len() == 11 + 48,
        "{key}"
    );
    let source = &made["source"];
    assert_eq!(source["kind"], "claude_code");
    assert_eq!(source["status"], "waiting");
    assert_eq!(source["user_id"], env.owner.user_id);
    assert_eq!(source["match_members"], true);
    assert_eq!(source["content"], false);
    assert!(source["ingest_url"].as_str().unwrap().ends_with("/ingest"));
    let shown = source["key"].as_str().unwrap();
    assert!(
        shown.starts_with("sk-kso-src-") && shown.contains('…'),
        "{shown}"
    );
    assert!(shown.ends_with(&key[key.len() - 4..]));

    let list = env
        .app
        .get(&format!("{}/sources", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.body["data"].as_array().unwrap().len(), 1);
    assert!(
        !list.body.to_string().contains(&key[11..]),
        "the key is never listed"
    );

    let bad = env
        .app
        .post(
            &format!("{}/sources", base(&env)),
            json!({ "kind": "cursor", "name": "x" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_source_is_renamed_disabled_given_a_new_key_and_archived() {
    let env = env().await;
    let made = add(&env, json!({ "kind": "codex", "name": "Codex" })).await;
    let id = made["source"]["id"].as_str().unwrap();
    let path = format!("{}/sources/{id}", base(&env));
    let r = env
        .app
        .call(
            axum::http::Method::PATCH,
            &path,
            Some(json!({ "name": "Codex CI", "disabled": true, "content": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["name"], "Codex CI");
    assert_eq!(r.body["status"], "disabled");
    assert_eq!(r.body["content"], true);

    let both = env
        .app
        .call(
            axum::http::Method::PATCH,
            &path,
            Some(json!({ "user_id": env.owner.user_id, "service_account_id": "svc_01k7a3f8m2xq4v9c5r6t8w0yz1" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(both.status, StatusCode::BAD_REQUEST, "{:?}", both.body);

    let rotated = env
        .app
        .post(
            &format!("{path}/rotate"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(rotated.status, StatusCode::OK, "{:?}", rotated.body);
    assert_ne!(rotated.body["key"], made["key"]);
    let old = kaeso_core::secret::hash(made["key"].as_str().unwrap());
    assert!(
        store::by_secret(&env.app.state.pg, &old)
            .await
            .unwrap()
            .is_none(),
        "the old key no longer finds the source"
    );
    let new = kaeso_core::secret::hash(rotated.body["key"].as_str().unwrap());
    assert!(
        store::by_secret(&env.app.state.pg, &new)
            .await
            .unwrap()
            .is_some()
    );

    let archived = env
        .app
        .post(
            &format!("{path}/archive"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(archived.body["status"], "archived");
    let again = env
        .app
        .post(
            &format!("{path}/rotate"),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    let list = env
        .app
        .get(&format!("{}/sources", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(list.body["data"].as_array().unwrap().len(), 0);
    let all = env
        .app
        .get(
            &format!("{}/sources?include_archived=true", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(all.body["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn observed_requests_are_logged_priced_and_counted_once_for_the_member_they_name() {
    let env = env().await;
    let made = add(
        &env,
        json!({ "kind": "claude_code", "name": "Claude Code" }),
    )
    .await;
    let id = made["source"]["id"].as_str().unwrap();
    let source = source_row(&env, id).await;
    let batch = Batch {
        requests: vec![
            haiku("req_011a", Some("OWNER@example.com")),
            haiku("req_011b", Some("nobody@example.com")),
        ],
        tool_calls: vec![ObservedToolCall {
            external_id: "toolu_01".into(),
            duration: Duration::from_millis(19),
            tool: "mcp__linear__list_issues".into(),
            server: Some("linear".into()),
            ok: true,
            client: Some("Claude Code".into()),
            ..ObservedToolCall::default()
        }],
        refused: 1,
        refusal: Some(observed::Refusal::NoModel),
    };
    let first = observed::record(&env.app.state, &source, batch.clone())
        .await
        .unwrap();
    assert_eq!(
        (
            first.requests,
            first.tool_calls,
            first.duplicates,
            first.refused
        ),
        (2, 1, 0, 1)
    );
    let again = observed::record(&env.app.state, &source, batch)
        .await
        .unwrap();
    assert_eq!(
        (again.requests, again.tool_calls, again.duplicates),
        (0, 0, 3)
    );
    flush(&env).await;

    let logs = env
        .app
        .get(&format!("{}/logs", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(logs.status, StatusCode::OK, "{:?}", logs.body);
    let rows = logs.body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    for row in rows {
        assert_eq!(row["source"], "observed");
        assert_eq!(row["model"], "claude-haiku-4-5", "{row}");
        assert_eq!(row["usage"]["input_tokens"], 2_000, "{row}");
        assert_eq!(row["usage"]["cache_read_tokens"], 10_000);
    }

    let (requests, cost, source_ids, users): (
        i64,
        Decimal,
        Vec<uuid::Uuid>,
        Vec<Option<uuid::Uuid>>,
    ) = sqlx::query_as(
        "SELECT sum(requests)::bigint, sum(cost), array_agg(DISTINCT source_id), \
               array_agg(DISTINCT user_id) FROM usage_daily WHERE project_id = $1",
    )
    .bind(common::uuid(&env.project))
    .fetch_one(&env.app.state.pg)
    .await
    .unwrap();
    assert_eq!(requests, 2);
    assert_eq!(source_ids, vec![source.id]);
    assert_eq!(users, vec![Some(common::uuid(&env.owner.user_id))]);
    let priced = observed::price(
        &env.app.state,
        common::uuid(&env.project),
        Some("anthropic"),
        "claude-haiku-4-5",
    )
    .await;
    let per_request = kaeso_core::cost::cost_with_media(
        &{
            let mut u = kaeso_kpp::Usage::new(2_000, 1_000);
            u.cache_read_tokens = 10_000;
            u
        },
        priced.prices.as_ref().unwrap(),
        None,
    );
    assert!(per_request > Decimal::ZERO);
    assert_eq!(
        cost,
        per_request * Decimal::from(2),
        "KPMR's price wins over the reported one"
    );

    let view = env
        .app
        .get(&format!("{}/sources/{id}", base(&env)), &env.owner.cookie)
        .await
        .body;
    assert_eq!(view["status"], "connected");
    assert_eq!(view["activity"][0]["requests"], 2);
    assert_eq!(view["activity"][0]["tool_calls"], 1);
    assert_eq!(view["activity"][0]["duplicates"], 3);
    assert_eq!(
        view["activity"][0]["refused"], 2,
        "both sends refused the record"
    );
    assert_eq!(view["last_error"], "no_model");

    let calls = env
        .app
        .get(
            &format!("{}/logs/tool-calls", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(calls.status, StatusCode::OK, "{:?}", calls.body);
    assert_eq!(calls.body["data"][0]["source"], "observed");
    assert_eq!(calls.body["data"][0]["tool"], "mcp__linear__list_issues");
}

#[tokio::test]
async fn a_cost_the_source_reports_counts_when_kaeso_knows_no_price_for_the_model() {
    let env = env().await;
    let made = add(&env, json!({ "kind": "http", "name": "Scripts" })).await;
    let source = source_row(&env, made["source"]["id"].as_str().unwrap()).await;
    let mut r = haiku("run-1", None);
    r.provider = None;
    r.model = "in-house-7b".into();
    r.reported_cost = Some(Decimal::from_str("0.25").unwrap());
    observed::record(
        &env.app.state,
        &source,
        Batch {
            requests: vec![r],
            ..Batch::default()
        },
    )
    .await
    .unwrap();
    flush(&env).await;
    let cost: Decimal =
        sqlx::query_scalar("SELECT sum(cost) FROM usage_daily WHERE project_id = $1")
            .bind(common::uuid(&env.project))
            .fetch_one(&env.app.state.pg)
            .await
            .unwrap();
    assert_eq!(cost, Decimal::from_str("0.25").unwrap());
    let logs = env
        .app
        .get(&format!("{}/logs", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(
        logs.body["data"][0]["cost_basis"], "source",
        "{:?}",
        logs.body
    );
}

#[tokio::test]
async fn a_viewer_sees_sources_but_cannot_add_one() {
    let env = env().await;
    add(&env, json!({ "kind": "gemini_cli", "name": "Gemini CLI" })).await;
    let (cookie, _) = common::join_first_project(
        &env.app,
        &env.owner,
        json!({ "email": "viewer@example.com", "role": "member" }),
        "Viewer",
    )
    .await;
    let list = env
        .app
        .get(&format!("{}/sources", base(&env)), &cookie)
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.body);
    let add = env
        .app
        .post(
            &format!("{}/sources", base(&env)),
            json!({ "kind": "codex", "name": "Codex" }),
            Some(&cookie),
        )
        .await;
    assert_eq!(add.status, StatusCode::FORBIDDEN, "{:?}", add.body);
}

#[tokio::test]
async fn a_source_counts_only_for_people_who_can_see_its_project() {
    let env = env().await;
    let other =
        common::create_project(&env.app, &env.owner.org_id, &env.owner.cookie, "Other").await;
    let (_, outsider) = common::join(
        &env.app,
        &env.owner,
        json!({ "email": "outsider@example.com", "role": "member", "project_roles": [{ "project_id": other, "role": "developer" }] }),
        "Outsider",
    )
    .await;
    let r = env
        .app
        .post(
            &format!("{}/sources", base(&env)),
            json!({ "kind": "codex", "name": "Codex", "user_id": outsider }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "user_id");

    let made = add(&env, json!({ "kind": "codex", "name": "Codex" })).await;
    let id = made["source"]["id"].as_str().unwrap();
    let source = source_row(&env, id).await;
    observed::record(
        &env.app.state,
        &source,
        Batch {
            requests: vec![haiku("from-outsider", Some("outsider@example.com"))],
            ..Batch::default()
        },
    )
    .await
    .unwrap();
    flush(&env).await;
    let users: Vec<Option<uuid::Uuid>> =
        sqlx::query_scalar("SELECT DISTINCT user_id FROM usage_daily WHERE project_id = $1")
            .bind(common::uuid(&env.project))
            .fetch_all(&env.app.state.pg)
            .await
            .unwrap();
    assert_eq!(
        users,
        vec![Some(common::uuid(&env.owner.user_id))],
        "someone outside the project is never charged; the source's member is"
    );

    let elsewhere = env
        .app
        .get(
            &format!("/orgs/{}/projects/{other}/sources/{id}", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        elsewhere.status,
        StatusCode::NOT_FOUND,
        "a source is only found in its project"
    );
}

#[tokio::test]
async fn observed_spend_counts_toward_a_budget_and_a_disabled_source_counts_nothing() {
    let env = env().await;
    let budget = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "spend_budget", "period": "month", "amount": "100" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(budget.status, StatusCode::CREATED, "{:?}", budget.body);
    let limit = budget.body["id"].as_str().unwrap().to_owned();
    let made = add(&env, json!({ "kind": "http", "name": "Scripts" })).await;
    let id = made["source"]["id"].as_str().unwrap();
    let source = source_row(&env, id).await;
    let mut r = haiku("priced-by-the-source", None);
    r.provider = None;
    r.model = "in-house-7b".into();
    r.reported_cost = Some(Decimal::from(3));
    let mut absurd = haiku("absurd", None);
    absurd.reported_cost = Some(Decimal::from(1_000_000_000));
    let mut old = haiku("old", None);
    old.started_at =
        Some(kaeso_core::clock::Clock::now(&*env.app.state.clock) - time::Duration::days(8));
    let done = observed::record(
        &env.app.state,
        &source,
        Batch {
            requests: vec![r, absurd, old],
            ..Batch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!((done.requests, done.refused), (1, 2));
    env.app.state.in_flight.settled().await;
    let mut used = Value::Null;
    for _ in 0..200 {
        used = env
            .app
            .get(&format!("{}/limits/{limit}", base(&env)), &env.owner.cookie)
            .await
            .body["used"]
            .clone();
        if used == "3" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        used, "3",
        "an observed request's spend counts toward the project's budget"
    );

    env.app
        .call(
            axum::http::Method::PATCH,
            &format!("{}/sources/{id}", base(&env)),
            Some(json!({ "disabled": true })),
            Some(&env.owner.cookie),
        )
        .await
        .expect_status(StatusCode::OK);
    let source = source_row(&env, id).await;
    let none = observed::record(
        &env.app.state,
        &source,
        Batch {
            requests: vec![haiku("while-disabled", None)],
            ..Batch::default()
        },
    )
    .await
    .unwrap();
    assert_eq!((none.requests, none.refused), (0, 1));
}

#[tokio::test]
async fn the_log_lists_only_observed_requests_when_asked() {
    let env = env().await;
    common::v1::call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(common::v1::messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    let made = add(
        &env,
        json!({ "kind": "claude_code", "name": "Claude Code" }),
    )
    .await;
    let source = source_row(&env, made["source"]["id"].as_str().unwrap()).await;
    observed::record(
        &env.app.state,
        &source,
        Batch {
            requests: vec![haiku("req_011c", None)],
            ..Batch::default()
        },
    )
    .await
    .unwrap();
    flush(&env).await;
    let all = env
        .app
        .get(&format!("{}/logs", base(&env)), &env.owner.cookie)
        .await;
    assert_eq!(
        all.body["data"].as_array().unwrap().len(),
        2,
        "{:?}",
        all.body
    );
    let only = env
        .app
        .get(
            &format!("{}/logs?source=observed", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(only.status, StatusCode::OK, "{:?}", only.body);
    let rows = only.body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["source"], "observed");
}

#[test]
fn the_api_document_describes_a_source_with_its_own_schema() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let list = &doc["paths"]["/platform/v1/orgs/{org}/projects/{project}/sources/{source}"]["get"]
        ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"];
    let name = list.as_str().unwrap().rsplit('/').next().unwrap();
    let schema = &doc["components"]["schemas"][name];
    assert_eq!(
        schema["properties"]["kind"]["$ref"], "#/components/schemas/SourceKindView",
        "another type took the name {name}: {schema}"
    );
    assert!(schema["properties"]["status"].is_object());
}
