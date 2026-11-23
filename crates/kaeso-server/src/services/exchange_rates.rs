use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use kaeso_core::currency::{self, RateBook};
use kaeso_store::StoreError;
use kaeso_store::pg::exchange_rates as store;
use rust_decimal::Decimal;
use time::Date;
use url::Url;

use crate::state::AppState;

pub const DAILY: &str = "eurofxref-daily.xml";
pub const HISTORY: &str = "eurofxref-hist-90d.xml";

const AFTER_SUCCESS: time::Duration = time::Duration::hours(6);
const AFTER_FAILURE: time::Duration = time::Duration::minutes(15);
// Each gateway of an install reads the rates again after this, so all take the ones one fetched.
const RELOAD: Duration = Duration::from_secs(600);
const MAX_FILE: usize = 1 << 20;

#[derive(Debug)]
pub struct Rates {
    book: RwLock<Arc<RateBook>>,
    loaded_at: RwLock<Option<std::time::Instant>>,
    reloading: AtomicBool,
}

impl Default for Rates {
    fn default() -> Self {
        Rates {
            book: RwLock::new(Arc::new(RateBook::default())),
            loaded_at: RwLock::new(None),
            reloading: AtomicBool::new(false),
        }
    }
}

impl Rates {
    fn set(&self, book: RateBook) {
        *self.book.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(book);
        *self
            .loaded_at
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(std::time::Instant::now());
    }

    fn stale(&self) -> bool {
        self.loaded_at
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none_or(|at| at.elapsed() >= RELOAD)
    }
}

// Never waits: a request goes on with the rates it has while stale ones are read again.
pub fn book(state: &AppState) -> Arc<RateBook> {
    let rates = &state.exchange_rates;
    if rates.stale()
        && !rates.reloading.swap(true, Ordering::AcqRel)
        && let Ok(runtime) = tokio::runtime::Handle::try_current()
    {
        let state = state.clone();
        runtime.spawn(async move {
            if let Err(e) = load(&state).await {
                tracing::warn!(error = %e, "cannot read the exchange rates; the gateway keeps the ones it has");
            }
            state
                .exchange_rates
                .reloading
                .store(false, Ordering::Release);
        });
    }
    rates
        .book
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub async fn load(state: &AppState) -> Result<Arc<RateBook>, StoreError> {
    let book = RateBook::new(store::all(&state.pg).await?);
    state.exchange_rates.set(book);
    Ok(state
        .exchange_rates
        .book
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone())
}

pub async fn due(state: &AppState) -> Result<bool, StoreError> {
    let status = store::status(&state.pg).await?;
    if !status.enabled {
        return Ok(false);
    }
    let wait = if status.error.is_some() {
        AFTER_FAILURE
    } else {
        AFTER_SUCCESS
    };
    Ok(status
        .checked_at
        .is_none_or(|at| state.clock.now() - at >= wait))
}

pub async fn sync(state: &AppState) -> Result<crate::jobs::Ran, StoreError> {
    let now = state.clock.now();
    let first = store::all(&state.pg).await?.is_empty();
    let file = if first { HISTORY } else { DAILY };
    let fetched = match fetch(&state.config.exchange_rates.url, file).await {
        Ok(xml) => currency::parse_ecb(&xml).map_err(|e| format!("{file}: {e}")),
        Err(e) => Err(e),
    };
    let days = match fetched {
        Ok(days) => days,
        Err(error) => {
            tracing::warn!(%error, "cannot fetch the ECB reference rates; the gateway keeps the ones it has");
            store::checked(&state.pg, now, Some(&error)).await?;
            return Ok(crate::jobs::Ran::worked(format!("failed: {error}")));
        }
    };
    let mut added = 0;
    for day in &days {
        let Some(date) = day.date else { continue };
        let rates: Vec<_> = day.per_eur.iter().map(|(c, r)| (c.clone(), *r)).collect();
        if store::insert_day(&state.pg, date, &rates).await? > 0 {
            added += 1;
        }
    }
    store::checked(&state.pg, now, None).await?;
    let book = load(state).await?;
    Ok(crate::jobs::Ran::of(
        format!(
            "{added} new days, latest {}",
            book.latest_day().map(|d| d.to_string()).unwrap_or_default()
        ),
        added > 0,
    ))
}

async fn fetch(base: &Url, file: &str) -> Result<String, String> {
    let url = base.join(file).map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(url.clone()).send().await.map_err(|e| {
        let mut message = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(c) = cause {
            message = format!("{message}: {c}");
            cause = c.source();
        }
        message
    })?;
    if !resp.status().is_success() {
        return Err(format!("{url} answered {}", resp.status()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_FILE {
            return Err(format!("{url} sent more than 1 MiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| format!("{url} sent no text"))
}

pub fn known(state: &AppState, code: &str) -> bool {
    code == currency::USD || book(state).latest(code).is_some()
}

pub async fn status(state: &AppState) -> Result<store::Status, StoreError> {
    store::status(&state.pg).await
}

#[derive(Clone, Debug)]
pub struct Money {
    book: Arc<RateBook>,
    pub code: String,
}

impl Money {
    pub fn of(state: &AppState, code: &str) -> Money {
        Money {
            book: book(state),
            code: code.to_owned(),
        }
    }

    pub async fn of_org(state: &AppState, org: uuid::Uuid) -> Result<Money, StoreError> {
        let code = kaeso_store::pg::orgs::get(&state.pg, org)
            .await?
            .map_or_else(|| currency::USD.to_owned(), |o| o.currency);
        Ok(Money::of(state, &code))
    }

    pub fn is_usd(&self) -> bool {
        self.code == currency::USD
    }

    // Spend is converted at the rate of the day it was spent, never at today's.
    pub fn on(&self, usd: Decimal, day: Date) -> Option<Decimal> {
        self.book.convert(usd, &self.code, day)
    }

    pub fn now(&self, usd: Decimal) -> Option<Decimal> {
        self.book.from_usd(usd, &self.code)
    }

    pub fn to_usd(&self, amount: Decimal) -> Option<Decimal> {
        self.book.to_usd(amount, &self.code)
    }

    pub fn text_on(&self, usd: Decimal, day: Date) -> Option<String> {
        self.on(usd, day).map(|v| self.text(v))
    }

    // Dollars keep every decimal they were costed with; a conversion is an estimate to six.
    pub fn text(&self, amount: Decimal) -> String {
        if self.is_usd() {
            amount.normalize().to_string()
        } else {
            amount_text(amount)
        }
    }
}

pub fn amount_text(v: Decimal) -> String {
    v.round_dp(6).normalize().to_string()
}
