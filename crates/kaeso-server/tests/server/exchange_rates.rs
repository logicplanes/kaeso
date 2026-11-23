#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::uuid;
use common::v1::{Env, call, chat};
use kaeso_core::clock::Clock as _;
use serde_json::{Value, json};

// The ECB's two files as it writes them: single quotes in the daily one, double in the history.
const DAILY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gesmes:Envelope><Cube>\n\
    <Cube time='2026-11-16'>\n<Cube currency='USD' rate='1.1014'/>\n<Cube currency='JPY' rate='175.78'/>\n\
    <Cube currency='GBP' rate='0.83450'/>\n</Cube></Cube></gesmes:Envelope>";
const HISTORY: &str = "<gesmes:Envelope><Cube><Cube time=\"2026-11-13\"><Cube currency=\"USD\" rate=\"1.0900\"/>\
    <Cube currency=\"JPY\" rate=\"170.00\"/><Cube currency=\"GBP\" rate=\"0.8400\"/></Cube>\
    <Cube time=\"2026-11-12\"><Cube currency=\"USD\" rate=\"1.0800\"/><Cube currency=\"JPY\" rate=\"169.00\"/>\
    <Cube currency=\"GBP\" rate=\"0.8300\"/></Cube></Cube></gesmes:Envelope>";

#[derive(Clone, Default)]
struct Ecb {
    // `None` answers 503.
    daily: Arc<Mutex<Option<&'static str>>>,
    history: Arc<Mutex<Option<&'static str>>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Ecb {
    async fn start() -> (Ecb, String) {
        let ecb = Ecb::default();
        *ecb.daily.lock().unwrap() = Some(DAILY);
        *ecb.history.lock().unwrap() = Some(HISTORY);
        let served = ecb.clone();
        let app = axum::Router::new().fallback(axum::routing::get(move |uri: axum::http::Uri| {
            let ecb = served.clone();
            async move {
                let file = uri.path().trim_start_matches("/ecb/").to_owned();
                ecb.asked.lock().unwrap().push(file.clone());
                let body = match file.as_str() {
                    "eurofxref-daily.xml" => *ecb.daily.lock().unwrap(),
                    "eurofxref-hist-90d.xml" => *ecb.history.lock().unwrap(),
                    _ => None,
                };
                match body {
                    Some(b) => (StatusCode::OK, b.to_owned()),
                    None => (StatusCode::SERVICE_UNAVAILABLE, String::new()),
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });
        (ecb, format!("http://{addr}/ecb/"))
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

async fn run_job(env: &Env) -> String {
    let job = kaeso_server::jobs::all()
        .into_iter()
        .find(|j| j.name == "exchange_rates")
        .unwrap();
    kaeso_server::jobs::run_once(&env.app.state, job)
        .await
        .unwrap()
        .unwrap()
}

async fn rates(env: &Env) -> Value {
    let r = env.app.get("/me/exchange-rates", &env.owner.cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

fn per_usd(rates: &Value, code: &str) -> Option<String> {
    rates["rates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["currency"] == code)
        .map(|r| r["per_usd"].as_str().unwrap().to_owned())
}

async fn env_with(ecb: &str) -> Env {
    common::v1::env(None, Some(&[("KAESO_EXCHANGE_RATES_URL", ecb)])).await
}

#[tokio::test]
async fn without_rates_only_dollars_and_the_first_fetch_takes_90_days() {
    let (ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    let none = rates(&env).await;
    assert_eq!(none["day"], Value::Null);
    assert_eq!(none["fetching"], true);
    assert_eq!(
        none["rates"],
        json!([{ "currency": "USD", "per_usd": "1" }])
    );
    let r = set_currency(&env, "EUR").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "currency");
    assert_eq!(
        r.body["error"]["message"],
        "Kaeso has no exchange rate for EUR; use a currency from `exchange-rates`."
    );

    assert_eq!(run_job(&env).await, "2 new days, latest 2026-11-13");
    assert_eq!(ecb.asked(), ["eurofxref-hist-90d.xml"]);
    let got = rates(&env).await;
    assert_eq!(
        (&got["day"], &got["first_day"]),
        (&json!("2026-11-13"), &json!("2026-11-12"))
    );
    assert_eq!(per_usd(&got, "EUR").as_deref(), Some("0.917431192661"));
    assert_eq!(per_usd(&got, "USD").as_deref(), Some("1"));
    assert_eq!(run_job(&env).await, "not due", "six hours after a fetch");

    env.app.clock.advance(time::Duration::hours(7));
    assert_eq!(run_job(&env).await, "1 new days, latest 2026-11-16");
    assert_eq!(ecb.asked().last().unwrap(), "eurofxref-daily.xml");
    assert_eq!(
        per_usd(&rates(&env).await, "GBP").as_deref(),
        Some("0.75767205375")
    );
}

#[tokio::test]
async fn a_failed_fetch_keeps_the_rates_and_is_tried_again_in_15_minutes() {
    let (ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    *ecb.history.lock().unwrap() = None;
    assert!(run_job(&env).await.starts_with("failed: "));
    let settings = env.app.get("/instance/settings", &env.owner.cookie).await;
    assert!(
        settings.body["exchange_rates"]["error"]
            .as_str()
            .unwrap()
            .contains("503"),
        "{:?}",
        settings.body
    );
    assert_eq!(run_job(&env).await, "not due");
    env.app.clock.advance(time::Duration::minutes(16));
    *ecb.history.lock().unwrap() = Some("<html>maintenance</html>");
    assert_eq!(
        run_job(&env).await,
        "failed: eurofxref-hist-90d.xml: the file has no day of rates"
    );
    env.app.clock.advance(time::Duration::minutes(16));
    *ecb.history.lock().unwrap() = Some(HISTORY);
    assert_eq!(run_job(&env).await, "2 new days, latest 2026-11-13");
    let settings = env.app.get("/instance/settings", &env.owner.cookie).await;
    assert_eq!(settings.body["exchange_rates"]["error"], Value::Null);
    assert_eq!(settings.body["exchange_rates"]["day"], "2026-11-13");

    // Turned off, nothing is fetched and the rates stay.
    let r = env
        .app
        .call(
            Method::PATCH,
            "/instance/settings",
            Some(json!({ "exchange_rates": false })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["exchange_rates"]["fetch"], false);
    env.app.clock.advance(time::Duration::days(2));
    let before = ecb.asked().len();
    assert_eq!(run_job(&env).await, "not due");
    assert_eq!(ecb.asked().len(), before);
    let got = rates(&env).await;
    assert_eq!(got["fetching"], false);
    assert_eq!(got["day"], "2026-11-13");
}

async fn model_id(env: &Env, model: &str) -> String {
    let models = env
        .app
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == model)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn set_prices(env: &Env, id: &str, overrides: Value) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/models/{id}",
                env.owner.org_id, env.project
            ),
            Some(json!({ "overrides": overrides })),
            Some(&env.owner.cookie),
        )
        .await
}

#[tokio::test]
async fn prices_in_euros_are_kept_as_entered_and_costed_in_dollars() {
    let (_ecb, url) = Ecb::start().await;
    let env = common::v1::env(
        Some(Arc::new(common::v1::Capture::default())),
        Some(&[("KAESO_EXCHANGE_RATES_URL", &url)]),
    )
    .await;
    let gpt = model_id(&env, "gpt-4o").await;
    let r = set_prices(
        &env,
        &gpt,
        json!({ "currency": "EUR", "input_price": "2.50", "output_price": "10" }),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "no rates yet: {:?}",
        r.body
    );
    assert_eq!(r.body["error"]["param"], "currency");
    assert_eq!(
        r.body["error"]["message"],
        "Kaeso has no exchange rate for EUR; enter the prices in USD."
    );
    let r = set_prices(&env, &gpt, json!({ "currency": "euro" })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body["error"]["param"], "currency");

    run_job(&env).await;
    env.app.clock.advance(time::Duration::hours(7));
    run_job(&env).await;
    let r = set_prices(
        &env,
        &gpt,
        json!({ "currency": "EUR", "input_price": "2.50", "output_price": "10" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["overrides"]["currency"], "EUR");
    assert_eq!(r.body["overrides"]["input_price"], "2.5");
    // At 1.1014 dollars to the euro.
    assert_eq!(r.body["effective"]["input_price"], "2.7535");
    assert_eq!(r.body["effective"]["output_price"], "11.014");
    assert_eq!(r.body["effective"].get("currency"), None);

    let answer = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", "Hi")),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let charged = env
        .outcomes()
        .await
        .into_iter()
        .find(|o| o.model_id.as_deref() == Some("gpt-4o"))
        .unwrap();
    let prices = charged.prices.unwrap();
    assert_eq!(
        (
            prices.input_price.map(|p| p.to_string()),
            prices.output_price.map(|p| p.to_string())
        ),
        (Some("2.7535".into()), Some("11.014".into()))
    );
}

async fn set_currency(env: &Env, code: &str) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &format!("/orgs/{}", env.owner.org_id),
            Some(json!({ "currency": code })),
            Some(&env.owner.cookie),
        )
        .await
}

// A request whose worst case fits the budget: the mock answers with 18 output tokens.
fn capped() -> Value {
    json!({ "model": "claude-haiku-4-5", "max_tokens": 18, "messages": [{ "role": "user", "content": "Hi" }] })
}

async fn with_rates(env: &Env) {
    run_job(env).await;
    env.app.clock.advance(time::Duration::hours(7));
    run_job(env).await;
}

async fn spend_on(env: &Env, d: &str, cost: rust_decimal::Decimal) {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    kaeso_store::pg::limits::add_usage(
        &env.app.state.pg,
        &[kaeso_store::pg::limits::UsageRow {
            day: time::Date::parse(d, &format).unwrap(),
            org_id: uuid(&env.owner.org_id),
            project_id: uuid(&env.project),
            user_id: None,
            service_account_id: None,
            api_key_id: None,
            catalogue_model_id: None,
            model_id: "gpt-4o".into(),
            provider: "openai".into(),
            ingress_dialect: "openai_chat".into(),
            egress_dialect: "openai_chat".into(),
            credential_id: None,
            source_id: None,
            requests: 1,
            errors: 0,
            input_tokens: 1,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cost,
            media: Default::default(),
        }],
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn the_org_sets_its_currency_and_the_change_is_audited() {
    let (_ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    let org = format!("/orgs/{}", env.owner.org_id);
    assert_eq!(
        env.app.get(&org, &env.owner.cookie).await.body["currency"],
        "USD"
    );
    with_rates(&env).await;
    for refused in ["euro", "CHF"] {
        let r = set_currency(&env, refused).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{refused}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "currency");
    }
    let r = set_currency(&env, "EUR").await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["currency"], "EUR");
    assert_eq!(
        env.app.get(&org, &env.owner.cookie).await.body["currency"],
        "EUR"
    );
    let memberships = env.app.get("/me/memberships", &env.owner.cookie).await;
    assert_eq!(
        memberships.body["data"][0]["org_currency"], "EUR",
        "{:?}",
        memberships.body
    );
    let audit = env
        .app
        .get(
            &format!("{org}/audit-log?action=org.update"),
            &env.owner.cookie,
        )
        .await;
    let event = &audit.body["data"][0];
    assert_eq!(
        (&event["before"]["currency"], &event["after"]["currency"]),
        (&json!("USD"), &json!("EUR")),
        "{event}"
    );

    // The currency is the org's, never a person's.
    let me = env.app.get("/me", &env.owner.cookie).await;
    assert_eq!(me.body.get("display_currency"), None, "{:?}", me.body);
    let r = env
        .app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "display_currency": "EUR" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
}

#[tokio::test]
async fn spend_budgets_are_set_in_the_org_currency_and_checked_in_dollars() {
    let (_ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    with_rates(&env).await;
    let limits = format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project);
    let budget = |amount: &str, period: &str| json!({ "level": "project", "kind": "spend_budget", "period": period, "amount": amount });
    let r = env
        .app
        .post(&limits, budget("11.014", "total"), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["currency"], "USD");
    let dollars = r.body["id"].as_str().unwrap().to_owned();

    // At 1.1014 dollars to the euro, $11.014 is €10.
    assert_eq!(set_currency(&env, "EUR").await.status, StatusCode::OK);
    let r = env
        .app
        .get(&format!("{limits}/{dollars}"), &env.owner.cookie)
        .await;
    assert_eq!(
        (
            &r.body["amount"],
            &r.body["currency"],
            &r.body["description"]
        ),
        (
            &json!("10"),
            &json!("EUR"),
            &json!("total spend budget of 10 EUR")
        ),
        "{:?}",
        r.body
    );
    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log?action=limit.update", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(
        audit.body["data"][0]["after"]["description"],
        "total spend budget of 10 EUR"
    );

    // €0.0003 is $0.00033: a third request of $0.0001 still fits, which $0.0003 wouldn't allow.
    let r = env
        .app
        .post(&limits, budget("0.0003", "month"), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["currency"], "EUR");
    let monthly = r.body["id"].as_str().unwrap().to_owned();
    for _ in 0..3 {
        let r = call(&env, "/v1/messages", Some(&env.key), true, Some(capped())).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    env.app.state.in_flight.settled().await;
    // $0.0003 used is €0.000272.
    let r = env
        .app
        .get(&format!("{limits}/{monthly}"), &env.owner.cookie)
        .await;
    assert_eq!(r.body["used"], "0.000272", "{:?}", r.body);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(capped())).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(
        r.text().contains("monthly spend budget of 0.0003 EUR"),
        "{}",
        r.text()
    );
}

async fn by_day(env: &Env, base: &str) -> Vec<(String, String, String, String)> {
    let r = env
        .app
        .get(
            &format!("{base}?from=2026-11-01&to=2026-11-16"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["key"].as_str().unwrap().to_owned(),
                r["cost"].as_str().unwrap().to_owned(),
                r["converted_cost"].as_str().unwrap().to_owned(),
                r["currency"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn spend_is_read_in_the_org_currency_at_each_days_rate() {
    let (_ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    env.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&env.app.state.pg)
        .await
        .unwrap();
    with_rates(&env).await;
    // $10 on a day with rates of its own, on a Saturday (Friday's rates) and before the first
    // rates (the latest).
    for d in ["2026-11-12", "2026-11-14", "2026-11-01"] {
        spend_on(&env, d, rust_decimal::Decimal::TEN).await;
    }
    let base = format!("/orgs/{}/projects/{}/usage", env.owner.org_id, env.project);
    let row = |day: &str, converted: &str, code: &str| {
        (
            day.to_owned(),
            "10".to_owned(),
            converted.to_owned(),
            code.to_owned(),
        )
    };
    assert_eq!(
        by_day(&env, &base).await,
        [
            row("2026-11-01", "10", "USD"),
            row("2026-11-12", "10", "USD"),
            row("2026-11-14", "10", "USD"),
        ],
        "in US dollars nothing is converted"
    );
    assert_eq!(set_currency(&env, "GBP").await.status, StatusCode::OK);
    assert_eq!(
        by_day(&env, &base).await,
        [
            // 0.8345 / 1.1014 pounds a dollar, the latest.
            row("2026-11-01", "7.576721", "GBP"),
            // 0.83 / 1.08.
            row("2026-11-12", "7.685185", "GBP"),
            // Friday's 0.84 / 1.09.
            row("2026-11-14", "7.706422", "GBP"),
        ]
    );
    let by_model = env
        .app
        .get(
            &format!("{base}?from=2026-11-01&to=2026-11-16&group_by=model"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(by_model.body["data"][0]["converted_cost"], "22.968328");

    // The request log and the timeline read $10 an hour ago at the latest rate.
    let at = env.app.clock.now().replace_nanosecond(0).unwrap() - time::Duration::hours(1);
    let (mut logged, _) = crate::logs::row(uuid(&env.owner.org_id), uuid(&env.project), at, 1);
    logged.cost = 10_000_000_000_000;
    let id = logged.request_id;
    kaeso_store::clickhouse::logs::insert_logs(
        env.app.state.clickhouse.as_ref().unwrap(),
        &[logged],
    )
    .await
    .unwrap();
    let rfc = |t: time::OffsetDateTime| {
        t.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let project = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let list = env
        .app
        .get(
            &format!(
                "{project}/logs?from={}",
                rfc(at - time::Duration::minutes(1))
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.body);
    let entry = &list.body["data"][0];
    assert_eq!(
        (&entry["cost"], &entry["converted_cost"], &entry["currency"]),
        (&json!("10"), &json!("7.576721"), &json!("GBP")),
        "{entry}"
    );
    let one = env
        .app
        .get(
            &format!(
                "{project}/logs/{}",
                kaeso_core::id::RequestId::from_uuid(id)
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(one.body["converted_cost"], "7.576721", "{:?}", one.body);
    // £7 is $9.24 at the latest rate, so the $10 request counts; £8 ($10.56) leaves it out.
    for (min, found) in [("7", 1), ("8", 0)] {
        let r = env
            .app
            .get(
                &format!(
                    "{project}/logs?from={}&min_cost={min}",
                    rfc(at - time::Duration::minutes(1))
                ),
                &env.owner.cookie,
            )
            .await;
        assert_eq!(
            r.body["data"].as_array().unwrap().len(),
            found,
            "{min}: {:?}",
            r.body
        );
    }
    let timeline = env
        .app
        .get(
            &format!(
                "/orgs/{}/usage/timeline?from={}&to={}&step=hour",
                env.owner.org_id,
                rfc(at - time::Duration::hours(1)),
                rfc(at + time::Duration::hours(1))
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(timeline.status, StatusCode::OK, "{:?}", timeline.body);
    let bucket = &timeline.body["data"][0];
    assert_eq!(
        (&bucket["converted_cost"], &bucket["currency"]),
        (&json!("7.576721"), &json!("GBP")),
        "{bucket}"
    );

    // A CSV export names the currency of its second cost column.
    let req = axum::http::Request::builder()
        .uri(format!(
            "/platform/v1{project}/logs/export?format=csv&from={}",
            rfc(at - time::Duration::minutes(1))
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let text = std::str::from_utf8(&body).unwrap();
    let header: Vec<&str> = text.lines().next().unwrap().split(',').collect();
    let at_column = |name: &str| header.iter().position(|h| *h == name).unwrap();
    let line: Vec<&str> = text.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(line[at_column("cost_usd")], "10", "{text}");
    assert_eq!(line[at_column("cost_gbp")], "7.576721", "{text}");

    // A live shared link reads the same pounds.
    let r = env
        .app
        .call(
            Method::PUT,
            &format!("/orgs/{}/security/shared-links", env.owner.org_id),
            Some(json!({ "allow": "anyone", "max_days": 90 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    spend_on(
        &env,
        &env.app.clock.now().date().to_string(),
        rust_decimal::Decimal::TEN,
    )
    .await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/shared-links", env.owner.org_id),
            json!({ "kind": "analytics", "project": env.project, "audience": "anyone", "days": 7, "expires_in_days": 30, "metric": "cost", "show_costs": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let token = r.body["url"]
        .as_str()
        .unwrap()
        .rsplit_once("/share/")
        .unwrap()
        .1
        .to_owned();
    let shared = env
        .app
        .call(Method::GET, &format!("/share/{token}"), None, None)
        .await;
    assert_eq!(shared.status, StatusCode::OK, "{:?}", shared.body);
    assert_eq!(shared.body["currency"], "GBP");
    let today = shared.body["days"].as_array().unwrap().last().unwrap();
    assert_eq!(
        (&today["cost"], &today["converted_cost"]),
        (&json!("10"), &json!("7.576721")),
        "{today}"
    );
}
