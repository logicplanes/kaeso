#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};
use uuid::Uuid;

use common::uuid;
use kaeso_core::clock::Clock;

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn create(env: &Env, path: &str, body: Value) -> Value {
    let r = env
        .app
        .post(
            &format!("{}{path}", base(env)),
            body,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

type Counts = BTreeMap<String, (u64, u64, u64)>;

async fn postgres(env: &Env) -> Counts {
    let rows: Vec<(String, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT provider, model_id, sum(requests)::bigint, sum(errors)::bigint, sum(input_tokens)::bigint \
         FROM usage_daily WHERE project_id = $1 GROUP BY provider, model_id",
    )
    .bind(uuid(&env.project))
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap();
    rows.into_iter()
        .map(|(p, m, r, e, i)| {
            let n = |v: i64| u64::try_from(v).unwrap();
            (format!("{p}/{m}"), (n(r), n(e), n(i)))
        })
        .collect()
}

async fn usage_api(env: &Env, group: &str) -> Counts {
    let today = env.app.clock.now().date();
    let r = env
        .app
        .get(
            &format!(
                "{}/usage?group_by={group}&from={}&to={}",
                base(env),
                today - time::Duration::days(1),
                today + time::Duration::days(1)
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
        .get("data")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|g| {
            let n = |f: &str| g.get(f).and_then(Value::as_u64).unwrap();
            (
                g.get("key").and_then(Value::as_str).unwrap().to_owned(),
                (n("requests"), n("errors"), n("input_tokens")),
            )
        })
        .collect()
}

async fn request_log(env: &Env, key: &str) -> Counts {
    #[derive(clickhouse::Row, serde::Deserialize)]
    struct Group {
        key: String,
        requests: u64,
        errors: u64,
        input_tokens: u64,
    }
    env.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .client()
        .query(&format!(
            "SELECT {key} AS key, count() AS requests, countIf(status >= 400) AS errors, \
             sum(input_tokens) AS input_tokens FROM request_log WHERE project_id = ? GROUP BY key"
        ))
        .bind(uuid(&env.project))
        .fetch_all::<Group>()
        .await
        .unwrap()
        .into_iter()
        .map(|g| (g.key, (g.requests, g.errors, g.input_tokens)))
        .collect()
}

async fn ask(env: &Env, key: &str, body: Value) -> common::v1::Answer {
    call(env, "/v1/messages", Some(key), true, Some(body)).await
}

#[tokio::test]
async fn usage_counts_the_requests_and_errors_the_request_log_holds() {
    let env = common::v1::env(None, None).await;
    let state = &env.app.state;
    state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&state.pg)
        .await
        .unwrap();
    assert_eq!(
        ask(&env, &env.key, messages("claude-haiku-4-5", "Hi"))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        ask(&env, &env.key, messages("claude-haiku-4-5", "Hi again"))
            .await
            .status,
        StatusCode::OK
    );
    let r = ask(
        &env,
        &env.key,
        messages("claude-haiku-4-5", "[mock:error=overloaded]"),
    )
    .await;
    assert!(r.status.is_server_error(), "{}", r.text());
    let r = ask(&env, &env.key, messages("claude-unknown-9", "Hi")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let r = ask(&env, &env.key, json!({ "max_tokens": 5 })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let limited = create(&env, "/keys", json!({ "name": "limited" })).await;
    let secret = limited["secret"].as_str().unwrap().to_owned();
    create(
        &env,
        "/limits",
        json!({ "level": "key", "key": limited["id"], "kind": "request_rate", "period": "minute", "amount": 1 }),
    )
    .await;
    assert_eq!(
        ask(&env, &secret, messages("claude-haiku-4-5", "Hi"))
            .await
            .status,
        StatusCode::OK
    );
    let r = ask(&env, &secret, messages("claude-haiku-4-5", "Hi")).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let mut cut = chat("claude-haiku-4-5", "[mock:cut]");
    cut["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(cut),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);

    env.app.state.in_flight.settled().await;

    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    env.app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    assert_eq!(env.app.state.logs.len(), 0, "the log buffer drained");
    let pg = postgres(&env).await;
    assert_eq!(
        pg,
        usage_api(&env, "model").await,
        "the API and PostgreSQL disagree"
    );
    assert_eq!(
        pg,
        request_log(&env, "concat(provider, '/', resolved_model)").await,
        "usage and the request log disagree"
    );
    let haiku = pg["anthropic/claude-haiku-4-5"];
    assert_eq!((haiku.0, haiku.1), (6, 3), "{pg:?}");
    assert!(haiku.2 > 0);
    assert_eq!(pg["/"], (2, 2, 0), "{pg:?}");

    let by_key: Counts = request_log(&env, "toString(key_id)").await;
    let api_keys = usage_api(&env, "key").await;
    assert_eq!(api_keys.len(), by_key.len(), "{api_keys:?}");
    let pg_keys: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        "SELECT api_key_id, sum(requests)::bigint, sum(errors)::bigint FROM usage_daily \
         WHERE project_id = $1 GROUP BY api_key_id",
    )
    .bind(uuid(&env.project))
    .fetch_all(&env.app.state.pg)
    .await
    .unwrap();
    assert_eq!(pg_keys.len(), by_key.len());
    for (key, requests, errors) in pg_keys {
        let c = by_key[&key.to_string()];
        assert_eq!(
            (c.0, c.1),
            (
                u64::try_from(requests).unwrap(),
                u64::try_from(errors).unwrap()
            ),
            "key {key}"
        );
    }
    let limited_counts = by_key[&uuid(limited["id"].as_str().unwrap()).to_string()];
    assert_eq!((limited_counts.0, limited_counts.1), (2, 1));
    let limited_api = api_keys[limited["id"].as_str().unwrap()];
    assert_eq!((limited_api.0, limited_api.1), (2, 1));
}

#[tokio::test]
async fn usage_by_route_shows_how_much_of_the_input_each_route_read_from_the_cache() {
    let env = common::v1::env(None, None).await;
    let cached = "Hi [mock:gen=5] [mock:in=100] [mock:cached=80]";
    let uncached = "Hi [mock:gen=5] [mock:in=100] [mock:cached=0]";
    for text in [cached, cached] {
        let r = ask(&env, &env.key, messages("claude-haiku-4-5", text)).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("claude-haiku-4-5", uncached)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();

    let rate = |body: &Value| -> Vec<(String, u64, Option<f64>)> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["key"].as_str().unwrap().to_owned(),
                    r["requests"].as_u64().unwrap(),
                    r["cache_hit_rate"].as_f64(),
                )
            })
            .collect()
    };
    for scope in [format!("/orgs/{}", env.owner.org_id), base(&env)] {
        let r = env
            .app
            .get(&format!("{scope}/usage?group_by=route"), &env.owner.cookie)
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(
            rate(&r.body),
            vec![
                (
                    "anthropic_messages:anthropic_messages:anthropic".to_owned(),
                    2,
                    Some(0.8)
                ),
                (
                    "openai_chat:anthropic_messages:anthropic".to_owned(),
                    1,
                    Some(0.0)
                ),
            ]
        );
    }
    let r = env
        .app
        .get(
            &format!("{}/usage?group_by=model", base(&env)),
            &env.owner.cookie,
        )
        .await;
    let both = 160.0 / 300.0;
    assert_eq!(
        rate(&r.body),
        vec![("anthropic/claude-haiku-4-5".to_owned(), 3, Some(both))]
    );
}

#[tokio::test]
async fn embedding_routes_have_no_cache_hit_rate() {
    let env = common::v1::env(None, None).await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "text-embedding-3-small", "input": "hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();

    let r = env
        .app
        .get(
            &format!("{}/usage?group_by=route", base(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let row = &r.body["data"][0];
    assert_eq!(row["key"], "openai_embeddings:openai_embeddings:openai");
    assert!(row["input_tokens"].as_u64().unwrap() > 0, "{row}");
    assert_eq!(row["cache_hit_rate"], Value::Null, "{row}");
}

async fn rollup_day(env: &Env, day: &str, requests: i64) {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[kaeso_store::pg::limits::UsageRow {
            requests,
            ..usage_row(
                time::Date::parse(day, &format).unwrap(),
                uuid(&env.owner.org_id),
                uuid(&env.project),
                1,
            )
        }],
    )
    .await
    .unwrap();
}

fn keyed(body: &Value) -> Vec<(String, u64)> {
    body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["key"].as_str().unwrap().to_owned(),
                r["requests"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn long_periods_go_by_week_or_month_from_the_first_day_with_usage() {
    let env = common::v1::env(None, None).await;
    let org = format!("/orgs/{}", env.owner.org_id);
    let get = async |path: String| env.app.get(&path, &env.owner.cookie).await;

    for scope in [org.clone(), base(&env)] {
        let r = get(format!("{scope}/usage/span")).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(r.body, json!({ "first_day": null, "last_day": null }));
    }

    for (day, requests) in [
        ("2024-01-01", 3),
        ("2024-01-03", 5),
        ("2024-01-08", 7),
        ("2024-02-15", 11),
        ("2025-03-10", 13),
    ] {
        rollup_day(&env, day, requests).await;
    }
    for scope in [org.clone(), base(&env)] {
        let r = get(format!("{scope}/usage/span")).await;
        assert_eq!(
            r.body,
            json!({ "first_day": "2024-01-01", "last_day": "2025-03-10" })
        );
    }

    let period = "from=2024-01-01&to=2025-12-31";
    let weeks = get(format!("{}/usage?{period}&group_by=week", base(&env))).await;
    assert_eq!(weeks.status, StatusCode::OK, "{:?}", weeks.body);
    let expect = |rows: &[(&str, u64)]| {
        rows.iter()
            .map(|(k, n)| ((*k).to_owned(), *n))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keyed(&weeks.body),
        expect(&[
            ("2024-01-01", 8),
            ("2024-01-08", 7),
            ("2024-02-12", 11),
            ("2025-03-10", 13)
        ])
    );
    let months = get(format!("{org}/usage?{period}&group_by=month")).await;
    assert_eq!(
        keyed(&months.body),
        expect(&[("2024-01-01", 15), ("2024-02-01", 11), ("2025-03-01", 13)])
    );
    let part = get(format!(
        "{}/usage?from=2024-01-02&to=2024-01-09&group_by=week",
        base(&env)
    ))
    .await;
    assert_eq!(
        keyed(&part.body),
        expect(&[("2024-01-01", 5), ("2024-01-08", 7)])
    );
    let models = get(format!(
        "{}/usage?from=1900-01-01&group_by=model",
        base(&env)
    ))
    .await;
    assert_eq!(models.status, StatusCode::OK, "{:?}", models.body);
    assert_eq!(
        keyed(&models.body),
        expect(&[("anthropic/claude-haiku-4-5", 39)])
    );

    let r = get(format!("{}/usage?{period}&group_by=day", base(&env))).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "from", "{:?}", r.body);
    let r = get(format!(
        "{}/usage?from=2025-01-02&to=2025-01-01",
        base(&env)
    ))
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
}

fn usage_row(
    day: time::Date,
    org: Uuid,
    project: Uuid,
    input_tokens: i64,
) -> kaeso_store::pg::limits::UsageRow {
    kaeso_store::pg::limits::UsageRow {
        day,
        org_id: org,
        project_id: project,
        user_id: None,
        service_account_id: None,
        api_key_id: None,
        catalogue_model_id: None,
        model_id: "claude-haiku-4-5".into(),
        provider: "anthropic".into(),
        ingress_dialect: "anthropic_messages".into(),
        egress_dialect: "anthropic_messages".into(),
        requests: 1,
        errors: 0,
        input_tokens,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        cost: rust_decimal::Decimal::ZERO,
    }
}

#[tokio::test]
async fn a_usage_row_postgres_refuses_never_blocks_the_others() {
    let env = common::v1::env(None, None).await;
    let state = &env.app.state;
    let (org, project) = (uuid(&env.owner.org_id), uuid(&env.project));
    let today = env.app.clock.now().date();
    assert_eq!(
        ask(&env, &env.key, messages("claude-haiku-4-5", "Hi"))
            .await
            .status,
        StatusCode::OK
    );
    state.in_flight.settled().await;
    state
        .usage
        .add(usage_row(today, Uuid::new_v4(), Uuid::new_v4(), 5));
    assert!(postgres(&env).await.is_empty());
    assert_eq!(
        kaeso_server::services::usage::flush(state).await.unwrap(),
        1
    );
    assert!(state.usage.take().is_empty());
    let recorded = postgres(&env).await;
    let recorded = *recorded.get("anthropic/claude-haiku-4-5").unwrap();
    assert_eq!(recorded.0, 1);
    state.usage.add(usage_row(today, org, project, 7));
    assert_eq!(
        kaeso_server::services::usage::flush(state).await.unwrap(),
        1
    );
    assert_eq!(
        postgres(&env).await.get("anthropic/claude-haiku-4-5"),
        Some(&(2, 0, recorded.2 + 7))
    );
}

#[test]
fn pending_usage_saturates_instead_of_overflowing() {
    let batch = kaeso_server::services::usage::Batch::default();
    let (org, project) = (Uuid::new_v4(), Uuid::new_v4());
    let day = time::OffsetDateTime::UNIX_EPOCH.date();
    batch.add(usage_row(day, org, project, i64::MAX - 1));
    batch.add(usage_row(day, org, project, 5));
    let (tokens, _, requests) = batch.pending(
        project,
        kaeso_core::limits::Subject::Project(project),
        None,
        None,
    );
    assert_eq!((tokens, requests), (i64::MAX, 2));
    let rows = batch.take();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().unwrap().input_tokens, i64::MAX);
}

#[tokio::test]
async fn usage_by_member_and_key_names_people_service_accounts_and_the_playground() {
    let env = common::v1::env(None, None).await;
    let (org, project) = (uuid(&env.owner.org_id), uuid(&env.project));
    let today = env.app.clock.now().date();
    let (user, account, key) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[
            kaeso_store::pg::limits::UsageRow {
                user_id: Some(user),
                ..usage_row(today, org, project, 1)
            },
            kaeso_store::pg::limits::UsageRow {
                service_account_id: Some(account),
                api_key_id: Some(key),
                requests: 2,
                ..usage_row(today, org, project, 1)
            },
        ],
    )
    .await
    .unwrap();
    let get = async |group: &str| {
        let r = env
            .app
            .get(
                &format!("{}/usage?group_by={group}", base(&env)),
                &env.owner.cookie,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        keyed(&r.body)
    };
    let mut members = vec![
        (
            format!(
                "service_account:{}",
                kaeso_core::id::ServiceAccountId::from_uuid(account)
            ),
            2,
        ),
        (
            format!("user:{}", kaeso_core::id::UserId::from_uuid(user)),
            1,
        ),
    ];
    members.sort();
    assert_eq!(get("member").await, members);
    let mut keys = vec![
        ("playground".to_owned(), 1),
        (kaeso_core::id::ApiKeyId::from_uuid(key).to_string(), 2),
    ];
    keys.sort();
    let mut got = get("key").await;
    got.sort();
    assert_eq!(got, keys);
}
