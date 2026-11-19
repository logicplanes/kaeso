use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use kaeso_core::currency::{self, RateBook};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};

use crate::platform::error::{ApiError, ApiResult};
use crate::services::exchange_rates as service;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A currency spend can be read in, and what one US dollar buys of it.")]
pub struct CurrencyRate {
    #[doc = "The ISO 4217 code."]
    #[schema(example = "EUR")]
    pub currency: String,
    #[doc = "Units of the currency one US dollar buys at the latest rates: the ECB's\n\
        euro rate of the currency divided by its euro rate of the dollar."]
    #[schema(value_type = String, example = "0.907935355003")]
    pub per_usd: Decimal,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The exchange rates the install holds.")]
pub struct ExchangeRates {
    #[doc = "Where the rates come from: `ecb`, the European Central Bank's euro foreign\n\
        exchange reference rates."]
    #[schema(example = "ecb")]
    pub source: &'static str,
    #[doc = "Whether the install fetches the rates each day; instance admins turn it off."]
    pub fetching: bool,
    #[doc = "The day of the latest rates; `null` while the install has none, when only\n\
        US dollars can be used."]
    #[schema(example = "2026-11-16")]
    pub day: Option<String>,
    #[doc = "The day of the oldest rates. Spend on earlier days is converted at the\n\
        latest rates."]
    #[schema(example = "2026-08-20")]
    pub first_day: Option<String>,
    #[doc = "When the install last fetched the rates."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    #[doc = "The currencies of the latest rates, US dollar and euro included, by code;\n\
        just the US dollar without rates."]
    pub rates: Vec<CurrencyRate>,
}

pub(crate) async fn view(state: &AppState) -> ApiResult<ExchangeRates> {
    let book = service::book(state);
    let status = service::status(state).await?;
    let mut rates: Vec<CurrencyRate> = book
        .currencies()
        .into_iter()
        .filter_map(|c| {
            book.latest(&c).map(|per_usd| CurrencyRate {
                currency: c,
                per_usd,
            })
        })
        .collect();
    if rates.is_empty() {
        rates.push(CurrencyRate {
            currency: currency::USD.to_owned(),
            per_usd: Decimal::ONE,
        });
    }
    Ok(ExchangeRates {
        source: "ecb",
        fetching: status.enabled,
        day: book.latest_day().map(|d| d.to_string()),
        first_day: book.first_day().map(|d| d.to_string()),
        checked_at: status.checked_at,
        rates,
    })
}

#[utoipa::path(
    get,
    path = "/me/exchange-rates",
    summary = "The exchange rates spend can be read in and prices entered in.",
    description = "The European Central Bank's euro reference rates of the latest working day \
    the install has, as units per US dollar. Kaeso stores and calculates in US dollars; the rates \
    convert prices entered in another currency and spend shown in one.",
    tag = "me",
    responses((status = 200, body = ExchangeRates))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<ExchangeRates>> {
    Ok(Json(view(&state).await?))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CurrencyParam {
    #[doc = "Also give each cost in this currency, an ISO 4217 code from\n\
        `exchange-rates`, as `converted_cost`: at the rate of the day it was spent, or\n\
        the latest rate for days before the install's first rates."]
    #[param(example = "EUR")]
    pub currency: Option<String>,
}

pub(crate) fn converter(
    state: &AppState,
    code: Option<&str>,
) -> ApiResult<Option<(Arc<RateBook>, String)>> {
    match code {
        None | Some(currency::USD) => Ok(None),
        Some(code) if service::known(state, code) => {
            Ok(Some((service::book(state), code.to_owned())))
        }
        Some(code) => Err(unknown("currency", code)),
    }
}

pub(crate) fn unknown(param: &str, code: &str) -> ApiError {
    ApiError::invalid(
        param,
        format!("Kaeso has no exchange rate for {code}; use a currency from `exchange-rates`."),
    )
}

pub(crate) fn convert(book: &RateBook, code: &str, usd: Decimal, day: Date) -> Option<String> {
    book.convert(usd, code, day)
        .map(|v| v.round_dp(6).normalize().to_string())
}
