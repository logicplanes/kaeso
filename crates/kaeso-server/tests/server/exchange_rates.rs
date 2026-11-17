#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::uuid;
use common::v1::{Env, call, chat};
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
    assert_eq!(r.body["error"]["param"], "display_currency");

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

#[tokio::test]
async fn spend_is_shown_in_the_chosen_currency_at_each_days_rate() {
    let (_ecb, url) = Ecb::start().await;
    let env = env_with(&url).await;
    run_job(&env).await;
    env.app.clock.advance(time::Duration::hours(7));
    run_job(&env).await;
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let day = |d: &str| time::Date::parse(d, &format).unwrap();
    // $10 on a day with rates of its own, on a Saturday (Friday's rates) and before the first
    // rates (the latest).
    for d in ["2026-11-12", "2026-11-14", "2026-11-01"] {
        kaeso_store::pg::limits::add_usage(
            &env.app.state.pg,
            &[kaeso_store::pg::limits::UsageRow {
                day: day(d),
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
                requests: 1,
                errors: 0,
                input_tokens: 1,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
                cost: rust_decimal::Decimal::TEN,
                media: Default::default(),
            }],
        )
        .await
        .unwrap();
    }
    let base = format!("/orgs/{}/projects/{}/usage", env.owner.org_id, env.project);
    let by_day = env
        .app
        .get(
            &format!("{base}?from=2026-11-01&to=2026-11-16&currency=GBP"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(by_day.status, StatusCode::OK, "{:?}", by_day.body);
    let rows: Vec<(String, String, String)> = by_day.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["key"].as_str().unwrap().to_owned(),
                r["cost"].as_str().unwrap().to_owned(),
                r["converted_cost"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            // 0.8345 / 1.1014 pounds a dollar, the latest.
            ("2026-11-01".into(), "10".into(), "7.576721".into()),
            // 0.83 / 1.08.
            ("2026-11-12".into(), "10".into(), "7.685185".into()),
            // Friday's 0.84 / 1.09.
            ("2026-11-14".into(), "10".into(), "7.706422".into()),
        ]
    );
    let by_model = env
        .app
        .get(
            &format!("{base}?from=2026-11-01&to=2026-11-16&group_by=model&currency=GBP"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(by_model.body["data"][0]["converted_cost"], "22.968328");
    let plain = env
        .app
        .get(
            &format!("{base}?from=2026-11-01&to=2026-11-16"),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(plain.body["data"][0]["converted_cost"], Value::Null);
    let unknown = env
        .app
        .get(&format!("{base}?currency=CHF"), &env.owner.cookie)
        .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.body["error"]["param"], "currency");

    // A person's choice is kept on their account.
    let r = env
        .app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "display_currency": "GBP" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["display_currency"], "GBP");
    let me = env.app.get("/me", &env.owner.cookie).await;
    assert_eq!(me.body["display_currency"], "GBP");
    let r = env
        .app
        .call(
            Method::PATCH,
            "/me",
            Some(json!({ "display_currency": "USD" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.body["display_currency"], "USD");
}
