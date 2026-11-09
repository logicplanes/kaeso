use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use time::{Date, Month, OffsetDateTime, Time};
use uuid::Uuid;

use crate::catalogue::Fields;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    RequestRate,
    TokenRate,
    TokenBudget,
    SpendBudget,
    ToolCallRate,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::RequestRate => "request_rate",
            Kind::TokenRate => "token_rate",
            Kind::TokenBudget => "token_budget",
            Kind::SpendBudget => "spend_budget",
            Kind::ToolCallRate => "tool_call_rate",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        [
            Kind::RequestRate,
            Kind::TokenRate,
            Kind::TokenBudget,
            Kind::SpendBudget,
            Kind::ToolCallRate,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }

    pub fn is_rate(self) -> bool {
        matches!(
            self,
            Kind::RequestRate | Kind::TokenRate | Kind::ToolCallRate
        )
    }

    pub fn default_thresholds(self) -> Vec<u8> {
        if self.is_rate() {
            Vec::new()
        } else {
            vec![50, 80, 100]
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::RequestRate => "request rate",
            Kind::TokenRate => "token rate",
            Kind::TokenBudget => "token budget",
            Kind::SpendBudget => "spend budget",
            Kind::ToolCallRate => "tool call rate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Minute,
    Day,
    Month,
    Total,
}

impl Period {
    pub fn as_str(self) -> &'static str {
        match self {
            Period::Minute => "minute",
            Period::Day => "day",
            Period::Month => "month",
            Period::Total => "total",
        }
    }

    pub fn parse(s: &str) -> Option<Period> {
        [Period::Minute, Period::Day, Period::Month, Period::Total]
            .into_iter()
            .find(|p| p.as_str() == s)
    }

    pub fn start(self, now: OffsetDateTime) -> Option<Date> {
        let today = now.to_offset(time::UtcOffset::UTC).date();
        match self {
            Period::Day => Some(today),
            Period::Month => Date::from_calendar_date(today.year(), today.month(), 1).ok(),
            Period::Minute | Period::Total => None,
        }
    }

    pub fn resets_at(self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        let now = now.to_offset(time::UtcOffset::UTC);
        match self {
            Period::Minute => Some(now + time::Duration::MINUTE),
            Period::Day => now
                .date()
                .next_day()
                .map(|d| d.with_time(Time::MIDNIGHT).assume_utc()),
            Period::Month => {
                let (y, m) = match now.month() {
                    Month::December => (now.year() + 1, Month::January),
                    m => (now.year(), m.next()),
                };
                Date::from_calendar_date(y, m, 1)
                    .ok()
                    .map(|d| d.with_time(Time::MIDNIGHT).assume_utc())
            }
            Period::Total => None,
        }
    }

    pub fn instance(self, now: OffsetDateTime) -> String {
        match (self, self.start(now)) {
            (Period::Day, Some(d)) => format!("d{}", d),
            (Period::Month, Some(d)) => format!("m{:04}-{:02}", d.year(), u8::from(d.month())),
            _ => self.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Project,
    Member,
    Key,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Project => "project",
            Level::Member => "member",
            Level::Key => "key",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "id")]
pub enum Subject {
    Project(Uuid),
    User(Uuid),
    ServiceAccount(Uuid),
    Key(Uuid),
}

impl Subject {
    pub fn level(self) -> Level {
        match self {
            Subject::Project(_) => Level::Project,
            Subject::User(_) | Subject::ServiceAccount(_) => Level::Member,
            Subject::Key(_) => Level::Key,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limit {
    pub id: Uuid,
    pub subject: Subject,
    pub kind: Kind,
    pub model: Option<Uuid>,
    pub period: Period,
    pub amount: Decimal,
    pub hard: bool,
    pub thresholds: Vec<u8>,
}

impl Limit {
    pub fn counter(&self, now: OffsetDateTime) -> String {
        format!("lim:{}:{}", self.id, self.period.instance(now))
    }

    pub fn enforced(&self) -> bool {
        self.kind.is_rate() || self.hard
    }

    pub fn units(&self) -> i64 {
        use rust_decimal::prelude::ToPrimitive;
        match self.kind {
            Kind::SpendBudget => crate::cost::nano_usd(self.amount),
            _ => self.amount.trunc().to_i64().unwrap_or(i64::MAX),
        }
    }

    pub fn describe(&self) -> String {
        let period = match self.period {
            Period::Minute => "per-minute ",
            Period::Day => "daily ",
            Period::Month => "monthly ",
            Period::Total => "total ",
        };
        let amount = match self.kind {
            Kind::RequestRate => format!("{} requests per minute", self.amount.normalize()),
            Kind::TokenRate => format!("{} tokens per minute", self.amount.normalize()),
            Kind::TokenBudget => format!("{} tokens", self.amount.normalize()),
            Kind::SpendBudget => format!("{} USD", self.amount.normalize()),
            Kind::ToolCallRate => format!("{} tool calls per minute", self.amount.normalize()),
        };
        let what = if self.kind.is_rate() { "" } else { period };
        format!("{what}{} of {amount}", self.kind.label())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct LimitError {
    pub field: &'static str,
    pub message: String,
}

fn invalid(field: &'static str, message: impl Into<String>) -> LimitError {
    LimitError {
        field,
        message: message.into(),
    }
}

pub const MAX_COUNT: i64 = 1_000_000_000_000;
pub const MAX_SPEND_USD: i64 = 1_000_000_000;

pub fn validate(l: &Limit) -> Result<(), LimitError> {
    match (l.kind.is_rate(), l.period) {
        (true, Period::Minute) | (false, Period::Day | Period::Month | Period::Total) => {}
        (true, _) => return Err(invalid("period", "Rates are per `minute`.")),
        (false, _) => {
            return Err(invalid(
                "period",
                "Budgets are per `day`, `month` or `total`.",
            ));
        }
    }
    let (max, message) = if l.kind == Kind::SpendBudget {
        (
            MAX_SPEND_USD,
            "The amount must be positive and at most 1,000,000,000 USD.",
        )
    } else {
        (MAX_COUNT, "The amount must be positive and at most 10^12.")
    };
    if l.amount <= Decimal::ZERO || l.amount > Decimal::from(max) {
        return Err(invalid("amount", message));
    }
    match l.kind {
        Kind::SpendBudget if l.amount.scale() > 9 => {
            return Err(invalid("amount", "A spend budget has at most 9 decimals."));
        }
        Kind::RequestRate | Kind::TokenRate | Kind::TokenBudget | Kind::ToolCallRate
            if !l.amount.fract().is_zero() =>
        {
            return Err(invalid("amount", "The amount must be a whole number."));
        }
        _ => {}
    }
    if l.kind == Kind::ToolCallRate && l.model.is_some() {
        return Err(invalid(
            "model",
            "A tool call rate counts every tool; leave `model` out.",
        ));
    }
    if l.kind.is_rate() && !l.thresholds.is_empty() {
        return Err(invalid(
            "alert_thresholds",
            "Rate limits have no alerts; leave `alert_thresholds` out.",
        ));
    }
    if l.thresholds.len() > 10
        || l.thresholds.iter().any(|t| !(1..=100).contains(t))
        || l.thresholds.windows(2).any(|w| w.first() >= w.get(1))
    {
        return Err(invalid(
            "alert_thresholds",
            "Alert thresholds are up to 10 ascending percentages from 1 to 100.",
        ));
    }
    Ok(())
}

pub fn amount<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    crate::decimal::deserialize_with(d, crate::decimal::parse_non_negative)
}

pub fn applicable(
    limits: &[Limit],
    project: Uuid,
    member: Option<Subject>,
    key: Option<Uuid>,
    model: Uuid,
) -> Vec<&Limit> {
    limits
        .iter()
        .filter(|l| {
            let subject = l.subject == Subject::Project(project)
                || Some(l.subject) == member
                || key.is_some_and(|k| l.subject == Subject::Key(k));
            subject && l.kind != Kind::ToolCallRate && l.model.is_none_or(|m| m == model)
        })
        .collect()
}

// A tool call counts against the tool call rates of its project, its member and its key.
pub fn applicable_to_tools(
    limits: &[Limit],
    project: Uuid,
    member: Option<Subject>,
    key: Option<Uuid>,
) -> Vec<&Limit> {
    limits
        .iter()
        .filter(|l| {
            l.kind == Kind::ToolCallRate
                && (l.subject == Subject::Project(project)
                    || Some(l.subject) == member
                    || key.is_some_and(|k| l.subject == Subject::Key(k)))
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorstCase {
    pub tokens: i64,
    pub nano_usd: i64,
    pub bounded: bool,
}

fn asks_for_one_hour_cache(request: &kaeso_kpp::Request) -> bool {
    let kaeso_kpp::Request::Generate(g) = request else {
        return false;
    };
    let one_hour = |c: Option<&kaeso_kpp::CacheHint>| {
        c.is_some_and(|c| c.ttl == Some(kaeso_kpp::CacheTtl::OneHour))
    };
    let top_level = g
        .extras
        .fields
        .get("cache_control")
        .and_then(|v| v.to_value().ok())
        .is_some_and(|v| v.get("ttl").and_then(serde_json::Value::as_str) == Some("1h"));
    top_level
        || g.all_blocks().any(|b| one_hour(b.cache()))
        || g.tools.iter().any(|t| match t {
            kaeso_kpp::ToolDef::Function(f) => one_hour(f.cache.as_ref()),
            _ => false,
        })
}

impl WorstCase {
    pub fn of(
        request: &kaeso_kpp::Request,
        body_bytes: usize,
        prices: &Fields,
        provider: &crate::providers::ProviderRef,
    ) -> WorstCase {
        match request {
            kaeso_kpp::Request::Image(r) => return WorstCase::of_images(r, prices),
            kaeso_kpp::Request::Speech(r) => return WorstCase::of_speech(r, prices),
            kaeso_kpp::Request::Video(r) => return WorstCase::of_videos(r, prices),
            _ => {}
        }
        let context = prices.context_limit;
        let within = |n: u64| context.map_or(n, |c| n.min(c));
        let bytes = u64::try_from(body_bytes).unwrap_or(u64::MAX);
        let input = within(crate::estimate::tokens_of_bytes(bytes));
        let prices = &crate::cost::prices_for_prompt(prices, input);
        let limit_holds =
            !(provider.reasoning_beyond_output_limit() && prices.reasoning == Some(true));
        let output = match request {
            kaeso_kpp::Request::Generate(g) => g
                .sampling
                .max_output_tokens
                .filter(|_| limit_holds)
                .map(u64::from)
                .or(prices.output_limit)
                .or(context)
                .map(within),
            kaeso_kpp::Request::Embed(_) => Some(0),
            _ => return WorstCase::FREE,
        };
        let usage = kaeso_kpp::Usage::new(input, output.unwrap_or(0));
        let mut priced = prices.clone();
        let one_hour = asks_for_one_hour_cache(request)
            .then(|| crate::cost::one_hour_cache_write_price(prices))
            .flatten();
        priced.input_price = [
            prices.input_price,
            prices.cache_read_price,
            prices.cache_write_price,
            one_hour,
        ]
        .into_iter()
        .flatten()
        .max();
        let total = crate::cost::total_tokens(&usage);
        WorstCase {
            tokens: i64::try_from(total).unwrap_or(i64::MAX),
            nano_usd: crate::cost::nano_usd(crate::cost::cost(&usage, &priced)),
            bounded: output.is_some(),
        }
    }

    // The prompt's tokens at the dearest input price, every image at its dearest unit price,
    // and output tokens up to the model's output limit; without one, token-priced output has
    // no bound.
    fn of_images(r: &kaeso_kpp::ImageRequest, prices: &Fields) -> WorstCase {
        let count = u64::from(r.count.unwrap_or(1).max(1));
        let prompt = r.prompt.as_deref().map_or(0, str::len);
        let input = crate::estimate::tokens_of_bytes(u64::try_from(prompt).unwrap_or(u64::MAX));
        let units = crate::cost::MediaUnits {
            input_images: r.images.len() as u64,
            output_images: count,
            output_pixels: r
                .size
                .as_ref()
                .and_then(kaeso_kpp::ImageSize::dimensions)
                .and_then(|(w, h)| u64::from(w).checked_mul(u64::from(h))),
            ..crate::cost::MediaUnits::default()
        };
        let dearest = prices.unit_prices.as_ref().map(|list| {
            let mut top: Vec<crate::catalogue::UnitPrice> = Vec::new();
            for p in list {
                match top.iter_mut().find(|t| t.per == p.per && t.of == p.of) {
                    Some(t) if t.usd < p.usd => t.usd = p.usd,
                    Some(_) => {}
                    None => top.push(crate::catalogue::UnitPrice {
                        size: None,
                        quality: None,
                        resolution: None,
                        audio: None,
                        ..p.clone()
                    }),
                }
            }
            top
        });
        let output_price = prices.output_image_price.or(prices.output_price);
        let output = match (prices.output_limit, output_price) {
            (_, None) => Some(0),
            (limit, Some(_)) => limit.map(|l| l.saturating_mul(count)),
        };
        let usage = kaeso_kpp::Usage::new(input, output.unwrap_or(0));
        let mut priced = Fields {
            unit_prices: dearest,
            ..prices.clone()
        };
        priced.input_price = [prices.input_price, prices.input_image_price]
            .into_iter()
            .flatten()
            .max();
        priced.output_price = output_price;
        let cost = crate::cost::cost_with_media(&usage, &priced, Some(&units));
        WorstCase {
            tokens: i64::try_from(crate::cost::total_tokens(&usage)).unwrap_or(i64::MAX),
            nano_usd: crate::cost::nano_usd(cost),
            bounded: output.is_some(),
        }
    }

    // Every character at the dearest price per character, the text's tokens at the input
    // price, and audio tokens up to the model's output limit; without one, token-priced audio
    // has no bound. A sound effect or music asked for by length is priced by that length.
    fn of_speech(r: &kaeso_kpp::SpeechRequest, prices: &Fields) -> WorstCase {
        let characters = r.characters();
        let input = crate::estimate::tokens_of_bytes(characters);
        let units = crate::cost::MediaUnits {
            characters,
            output_ms: r.duration.map_or(0, crate::assembler::seconds_to_ms),
            ..crate::cost::MediaUnits::default()
        };
        let output_price = prices.output_audio_price.or(prices.output_price);
        let output = match (prices.output_limit, output_price) {
            (_, None) => Some(0),
            (limit, Some(_)) => limit,
        };
        let usage = kaeso_kpp::Usage::new(input, output.unwrap_or(0));
        let dearest = prices.unit_prices.as_ref().map(|list| {
            let mut top: Vec<crate::catalogue::UnitPrice> = Vec::new();
            for p in list {
                match top.iter_mut().find(|t| t.per == p.per && t.of == p.of) {
                    Some(t) if t.usd < p.usd => t.usd = p.usd,
                    Some(_) => {}
                    None => top.push(p.clone()),
                }
            }
            top
        });
        let mut priced = Fields {
            unit_prices: dearest,
            ..prices.clone()
        };
        priced.output_price = output_price;
        let cost = crate::cost::cost_with_media(&usage, &priced, Some(&units));
        WorstCase {
            tokens: i64::try_from(crate::cost::total_tokens(&usage)).unwrap_or(i64::MAX),
            nano_usd: crate::cost::nano_usd(cost),
            bounded: output.is_some(),
        }
    }

    // Every video at the dearest price per video or second of the model, whatever its
    // resolution, sound or length, and token-priced video up to the model's output limit. A
    // job without a length priced by the second, or by the token without a limit, has no bound.
    fn of_videos(r: &kaeso_kpp::VideoRequest, prices: &Fields) -> WorstCase {
        let count = u64::from(r.count.unwrap_or(1).max(1));
        let units = crate::cost::MediaUnits {
            output_videos: count,
            output_ms: r.seconds.map_or(0, |s| {
                u64::from(s).saturating_mul(1000).saturating_mul(count)
            }),
            ..crate::cost::MediaUnits::default()
        };
        let dearest = prices.unit_prices.as_ref().map(|list| {
            let mut top: Vec<crate::catalogue::UnitPrice> = Vec::new();
            for p in list {
                match top.iter_mut().find(|t| t.per == p.per && t.of == p.of) {
                    Some(t) if t.usd < p.usd => t.usd = p.usd,
                    Some(_) => {}
                    None => top.push(crate::catalogue::UnitPrice {
                        size: None,
                        quality: None,
                        resolution: None,
                        audio: None,
                        duration: None,
                        ..p.clone()
                    }),
                }
            }
            top
        });
        let per_time = dearest.as_ref().is_some_and(|top| {
            top.iter()
                .any(|p| matches!(p.per.as_str(), "second" | "minute" | "hour"))
        });
        let output = match (prices.output_limit, prices.output_price) {
            (_, None) => Some(0),
            (limit, Some(_)) => limit.map(|l| l.saturating_mul(count)),
        };
        let usage = kaeso_kpp::Usage::new(0, output.unwrap_or(0));
        let priced = Fields {
            unit_prices: dearest,
            ..prices.clone()
        };
        let cost = crate::cost::cost_with_media(&usage, &priced, Some(&units));
        WorstCase {
            tokens: i64::try_from(crate::cost::total_tokens(&usage)).unwrap_or(i64::MAX),
            nano_usd: crate::cost::nano_usd(cost),
            bounded: output.is_some() && (r.seconds.is_some() || !per_time),
        }
    }

    pub const FREE: WorstCase = WorstCase {
        tokens: 0,
        nano_usd: 0,
        bounded: true,
    };

    pub fn units(&self, kind: Kind) -> i64 {
        match kind {
            Kind::RequestRate | Kind::ToolCallRate => 0,
            Kind::TokenRate | Kind::TokenBudget => self.tokens,
            Kind::SpendBudget => self.nano_usd,
        }
    }
}

pub fn exceeds(child: &Limit, parent: &Limit) -> bool {
    child.kind == parent.kind
        && child.period == parent.period
        && (parent.model.is_none() || parent.model == child.model)
        && child.amount > parent.amount
}

pub fn crossed(thresholds: &[u8], before: i64, after: i64, amount: i64) -> Vec<u8> {
    if amount <= 0 {
        return Vec::new();
    }
    thresholds
        .iter()
        .copied()
        .filter(|t| {
            let line = i128::from(amount) * i128::from(*t);
            i128::from(before) * 100 < line && i128::from(after) * 100 >= line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    fn limit(subject: Subject, kind: Kind, period: Period, amount: Decimal) -> Limit {
        Limit {
            id: Uuid::now_v7(),
            subject,
            kind,
            model: None,
            period,
            amount,
            hard: true,
            thresholds: kind.default_thresholds(),
        }
    }

    #[derive(Debug, Deserialize)]
    struct Amount {
        #[serde(default, deserialize_with = "amount")]
        amount: Option<Decimal>,
    }

    fn parse_amount(json: &str) -> Result<Option<Decimal>, String> {
        serde_json::from_str::<Amount>(json)
            .map(|a| a.amount)
            .map_err(|e| e.to_string())
    }

    #[test]
    fn amounts_are_not_capped_like_prices() {
        assert_eq!(
            parse_amount(r#"{"amount": 20000000}"#),
            Ok(Some(dec!(20000000)))
        );
        assert_eq!(
            parse_amount(r#"{"amount": "1000000000000"}"#),
            Ok(Some(dec!(1000000000000)))
        );
        assert_eq!(parse_amount(r#"{"amount": "12.50"}"#), Ok(Some(dec!(12.5))));
        assert_eq!(parse_amount(r#"{"amount": null}"#), Ok(None));
        assert_eq!(parse_amount("{}"), Ok(None));
        assert!(
            parse_amount(r#"{"amount": 0.5}"#)
                .unwrap_err()
                .contains("decimal string")
        );
        for refused in ["-5", "+5", "5.", "1_000", " 5", "5 ", ""] {
            assert!(
                parse_amount(&format!(r#"{{"amount": "{refused}"}}"#))
                    .unwrap_err()
                    .contains("not a decimal"),
                "{refused:?}"
            );
        }
        assert!(parse_amount(r#"{"amount": -5}"#).is_err());
        assert!(
            parse_amount(r#"{"amount": "lots"}"#)
                .unwrap_err()
                .contains("not a decimal")
        );
        let budget = limit(
            Subject::Project(Uuid::nil()),
            Kind::TokenBudget,
            Period::Month,
            dec!(20000000),
        );
        assert!(validate(&budget).is_ok());
        let too_big = limit(
            Subject::Project(Uuid::nil()),
            Kind::TokenBudget,
            Period::Month,
            dec!(1000000000001),
        );
        assert_eq!(validate(&too_big).unwrap_err().field, "amount");
    }

    #[test]
    fn spend_budgets_stop_at_a_billion_dollars() {
        let p = Subject::Project(Uuid::nil());
        let spend = |amount: Decimal| limit(p, Kind::SpendBudget, Period::Total, amount);
        assert!(validate(&spend(dec!(1000000000))).is_ok());
        assert!(validate(&spend(dec!(999999999.999999999))).is_ok());
        let beyond = validate(&spend(dec!(1000000000.000000001))).unwrap_err();
        assert_eq!(beyond.field, "amount");
        assert_eq!(
            beyond.message,
            "The amount must be positive and at most 1,000,000,000 USD."
        );
        assert!(beyond.message.len() <= 90);
        assert_eq!(
            validate(&spend(dec!(0))).unwrap_err().message,
            beyond.message
        );
        let tokens = limit(p, Kind::TokenBudget, Period::Total, dec!(1000000000000));
        assert!(validate(&tokens).is_ok());
        let rate = limit(p, Kind::RequestRate, Period::Minute, dec!(1000000000000));
        assert!(validate(&rate).is_ok());
    }

    fn openai() -> &'static crate::providers::ProviderRef {
        static OPENAI: std::sync::LazyLock<crate::providers::ProviderRef> =
            std::sync::LazyLock::new(|| crate::provider::provider("openai").unwrap().into());
        &OPENAI
    }

    fn priced(input: &str, output: &str) -> Fields {
        Fields {
            input_price: Some(input.parse().unwrap()),
            output_price: Some(output.parse().unwrap()),
            output_limit: Some(64_000),
            ..Fields::default()
        }
    }

    fn generate(max_output: Option<u32>) -> kaeso_kpp::Request {
        let mut g = kaeso_kpp::GenerateRequest::new("m");
        g.sampling.max_output_tokens = max_output;
        kaeso_kpp::Request::Generate(g)
    }

    #[test]
    fn an_image_request_reserves_every_image_at_its_dearest_price() {
        use kaeso_kpp::{ImageInput, ImageOperation, ImageRequest, Request, Source};
        let unit = |of: &str, usd: Decimal, quality: Option<&str>| crate::catalogue::UnitPrice {
            per: "image".into(),
            of: of.into(),
            usd,
            quality: quality.map(str::to_owned),
            ..crate::catalogue::UnitPrice::default()
        };
        let per_image = Fields {
            unit_prices: Some(vec![
                unit("output", dec!(0.04), Some("low")),
                unit("output", dec!(0.08), Some("medium")),
                unit("input", dec!(0.01), None),
            ]),
            ..Fields::default()
        };
        let mut edit = ImageRequest::new("m", ImageOperation::Edit)
            .with_prompt("hat")
            .with_image(ImageInput::new(Source::base64("image/png", "AA==")));
        edit.count = Some(3);
        let worst = WorstCase::of(
            &Request::Image(edit.clone()),
            5_000_000,
            &per_image,
            openai(),
        );
        assert_eq!(worst.nano_usd, 250_000_000);
        assert!(worst.bounded);
        let tokens = Fields {
            input_price: Some(dec!(5)),
            input_image_price: Some(dec!(8)),
            output_image_price: Some(dec!(30)),
            output_limit: Some(10_000),
            ..Fields::default()
        };
        let worst = WorstCase::of(&Request::Image(edit.clone()), 5_000_000, &tokens, openai());
        assert_eq!(worst.nano_usd, 900_000_000 + 8_000);
        let unbounded = WorstCase::of(
            &Request::Image(edit),
            5_000_000,
            &Fields {
                output_limit: None,
                ..tokens
            },
            openai(),
        );
        assert!(!unbounded.bounded);
    }

    #[test]
    fn a_request_that_asks_for_one_hour_caching_reserves_its_write_price() {
        let mut prices = priced("3", "15");
        prices.cache_write_price = Some(dec!(3.75));
        let plain = WorstCase::of(&generate(Some(0)), 4000, &prices, openai());
        assert_eq!(plain.nano_usd, 1000 * 3750);
        let kaeso_kpp::Request::Generate(mut g) = generate(Some(0)) else {
            unreachable!()
        };
        let mut system = kaeso_kpp::TextBlock::new("You are terse.");
        system.cache = Some(kaeso_kpp::CacheHint::new(Some(
            kaeso_kpp::CacheTtl::OneHour,
        )));
        g.system.push(kaeso_kpp::Block::Text(system));
        let request = kaeso_kpp::Request::Generate(g);
        let one_hour = WorstCase::of(&request, 4000, &prices, openai());
        assert_eq!(one_hour.nano_usd, 1000 * 6000, "twice the input price");
        prices.cache_write_1h_price = Some(dec!(7));
        let listed = WorstCase::of(&request, 4000, &prices, openai());
        assert_eq!(
            listed.nano_usd,
            1000 * 7000,
            "the catalogue's one-hour price"
        );
    }

    #[test]
    fn a_long_request_reserves_at_its_long_context_prices() {
        let mut prices = priced("1", "5");
        prices.price_tiers = Some(vec![crate::catalogue::PriceTier {
            context_above: 999,
            input_price: Some(dec!(2)),
            output_price: Some(dec!(10)),
            ..crate::catalogue::PriceTier::default()
        }]);
        let short = WorstCase::of(&generate(Some(1000)), 3996, &prices, openai());
        assert_eq!(short.nano_usd, 999 * 1000 + 1000 * 5000);
        let long = WorstCase::of(&generate(Some(1000)), 4000, &prices, openai());
        assert_eq!(long.nano_usd, 1000 * 2000 + 1000 * 10_000);
    }

    #[test]
    fn a_reasoning_model_whose_provider_leaves_reasoning_out_of_the_limit_holds_its_own_output_limit()
     {
        let xai = &crate::providers::ProviderRef::from(crate::provider::provider("xai").unwrap());
        let mut prices = priced("1", "5");
        prices.reasoning = Some(true);
        let w = WorstCase::of(&generate(Some(60)), 4000, &prices, xai);
        assert_eq!(w.tokens, 1000 + 64_000);
        prices.reasoning = Some(false);
        assert_eq!(
            WorstCase::of(&generate(Some(60)), 4000, &prices, xai).tokens,
            1060
        );
        prices.reasoning = Some(true);
        assert_eq!(
            WorstCase::of(&generate(Some(60)), 4000, &prices, openai()).tokens,
            1060
        );
    }

    #[test]
    fn a_request_costs_at_most_its_input_and_its_output_limit() {
        let prices = priced("1", "5");
        let w = WorstCase::of(&generate(Some(1000)), 4000, &prices, openai());
        assert_eq!(
            w,
            WorstCase {
                tokens: 2000,
                nano_usd: 1000 * 1000 + 1000 * 5000,
                bounded: true
            }
        );
        assert_eq!(w.units(Kind::TokenBudget), 2000);
        assert_eq!(w.units(Kind::TokenRate), 2000);
        assert_eq!(w.units(Kind::SpendBudget), 6_000_000);
        assert_eq!(w.units(Kind::RequestRate), 0);

        let model_cap = WorstCase::of(&generate(None), 4000, &prices, openai());
        assert_eq!(model_cap.tokens, 65_000);
        assert!(model_cap.bounded);

        let mut cached = prices.clone();
        cached.cache_write_price = Some(dec!(1.25));
        assert_eq!(
            WorstCase::of(&generate(Some(0)), 4000, &cached, openai()).nano_usd,
            1250 * 1000
        );

        let windowed = Fields {
            context_limit: Some(1500),
            ..Fields::default()
        };
        let w = WorstCase::of(&generate(None), 40_000, &windowed, openai());
        assert_eq!((w.tokens, w.nano_usd, w.bounded), (3000, 0, true));

        let unknown = WorstCase::of(&generate(None), 400, &Fields::default(), openai());
        assert_eq!((unknown.tokens, unknown.bounded), (100, false));

        let embed = kaeso_kpp::Request::Embed(kaeso_kpp::EmbedRequest::new("e", Vec::new()));
        let w = WorstCase::of(&embed, 400, &prices, openai());
        assert_eq!((w.tokens, w.nano_usd, w.bounded), (100, 100_000, true));

        let count = kaeso_kpp::Request::CountTokens(kaeso_kpp::GenerateRequest::new("m"));
        assert_eq!(
            WorstCase::of(&count, 400, &prices, openai()),
            WorstCase::FREE
        );
    }

    #[test]
    fn the_largest_budget_fits_a_counter_many_times_over() {
        let p = Subject::Project(Uuid::nil());
        let cap = limit(
            p,
            Kind::SpendBudget,
            Period::Total,
            Decimal::from(MAX_SPEND_USD),
        );
        assert_eq!(cap.units(), 1_000_000_000_000_000_000);
        assert!(cap.units().checked_mul(9).is_some());
        assert!(cap.units() < (1_i64 << 62));
        let nano = limit(
            p,
            Kind::SpendBudget,
            Period::Total,
            dec!(999999999.999999999),
        );
        assert_eq!(nano.units(), 999_999_999_999_999_999);
        assert_eq!(
            crossed(&[50, 100], cap.units() - 1, cap.units(), cap.units()),
            vec![100]
        );
    }

    #[test]
    fn periods_roll_over_at_utc_boundaries() {
        let late = datetime!(2026-01-31 23:59:59 UTC);
        assert_eq!(Period::Day.instance(late), "d2026-01-31");
        assert_eq!(Period::Month.instance(late), "m2026-01");
        assert_eq!(
            Period::Day.resets_at(late),
            Some(datetime!(2026-02-01 0:00 UTC))
        );
        assert_eq!(
            Period::Month.resets_at(late),
            Some(datetime!(2026-02-01 0:00 UTC))
        );
        let next = datetime!(2026-02-01 00:00:00 UTC);
        assert_eq!(Period::Day.instance(next), "d2026-02-01");
        assert_eq!(Period::Month.instance(next), "m2026-02");
        let berlin = datetime!(2026-02-01 00:30 +01:00);
        assert_eq!(Period::Day.instance(berlin), "d2026-01-31");
        assert_eq!(
            Period::Month.resets_at(datetime!(2026-12-15 8:00 UTC)),
            Some(datetime!(2027-01-01 0:00 UTC))
        );
        assert_eq!(Period::Total.instance(late), "total");
        assert_eq!(Period::Total.resets_at(late), None);
    }

    #[test]
    fn limits_are_validated() {
        let p = Subject::Project(Uuid::nil());
        assert!(validate(&limit(p, Kind::RequestRate, Period::Minute, dec!(10))).is_ok());
        assert_eq!(
            validate(&limit(p, Kind::RequestRate, Period::Day, dec!(10)))
                .unwrap_err()
                .field,
            "period"
        );
        assert_eq!(
            validate(&limit(p, Kind::SpendBudget, Period::Minute, dec!(10)))
                .unwrap_err()
                .field,
            "period"
        );
        assert_eq!(
            validate(&limit(p, Kind::TokenBudget, Period::Day, dec!(1.5)))
                .unwrap_err()
                .field,
            "amount"
        );
        assert!(validate(&limit(p, Kind::SpendBudget, Period::Month, dec!(1.5))).is_ok());
        assert_eq!(
            validate(&limit(p, Kind::SpendBudget, Period::Month, dec!(0)))
                .unwrap_err()
                .field,
            "amount"
        );
        let mut l = limit(p, Kind::SpendBudget, Period::Month, dec!(5));
        l.thresholds = vec![80, 50];
        assert_eq!(validate(&l).unwrap_err().field, "alert_thresholds");
        let mut rate = limit(p, Kind::TokenRate, Period::Minute, dec!(5));
        assert!(rate.thresholds.is_empty());
        rate.thresholds = vec![50];
        assert_eq!(
            validate(&rate).unwrap_err().message,
            "Rate limits have no alerts; leave `alert_thresholds` out."
        );
        assert_eq!(Kind::TokenBudget.default_thresholds(), [50, 80, 100]);
        assert!(Kind::RequestRate.default_thresholds().is_empty());
        assert!(Kind::ToolCallRate.default_thresholds().is_empty());
    }

    #[test]
    fn three_levels_and_model_scopes_apply() {
        let (project, user, key, other_key, model, other_model) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let mut scoped = limit(Subject::Key(key), Kind::TokenBudget, Period::Day, dec!(100));
        scoped.model = Some(model);
        let mut elsewhere = limit(Subject::Key(key), Kind::TokenBudget, Period::Day, dec!(100));
        elsewhere.model = Some(other_model);
        let all = vec![
            limit(
                Subject::Project(project),
                Kind::RequestRate,
                Period::Minute,
                dec!(60),
            ),
            limit(
                Subject::User(user),
                Kind::SpendBudget,
                Period::Month,
                dec!(50),
            ),
            scoped.clone(),
            elsewhere,
            limit(
                Subject::Key(other_key),
                Kind::RequestRate,
                Period::Minute,
                dec!(1),
            ),
            limit(
                Subject::User(Uuid::now_v7()),
                Kind::RequestRate,
                Period::Minute,
                dec!(1),
            ),
        ];
        let got = applicable(&all, project, Some(Subject::User(user)), Some(key), model);
        assert_eq!(got.len(), 3);
        assert!(got.contains(&&scoped));
        let got = applicable(&all, project, Some(Subject::User(user)), None, model);
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|l| !matches!(l.subject, Subject::Key(_))));
    }

    #[test]
    fn children_never_exceed_parents() {
        let p = Subject::Project(Uuid::nil());
        let k = Subject::Key(Uuid::nil());
        let parent = limit(p, Kind::SpendBudget, Period::Month, dec!(50));
        let child = limit(k, Kind::SpendBudget, Period::Month, dec!(60));
        assert!(exceeds(&child, &parent));
        assert!(!exceeds(
            &limit(k, Kind::SpendBudget, Period::Month, dec!(50)),
            &parent
        ));
        assert!(
            !exceeds(&limit(k, Kind::SpendBudget, Period::Day, dec!(60)), &parent),
            "other periods are independent"
        );
        let mut scoped = child.clone();
        scoped.model = Some(Uuid::now_v7());
        assert!(
            exceeds(&scoped, &parent),
            "an all-models parent covers model limits"
        );
        let mut scoped_parent = parent.clone();
        scoped_parent.model = Some(Uuid::now_v7());
        assert!(
            !exceeds(&child, &scoped_parent),
            "a model parent does not cover other models"
        );
    }

    #[test]
    fn thresholds_cross_once() {
        assert_eq!(crossed(&[50, 80, 100], 0, 49, 100), Vec::<u8>::new());
        assert_eq!(crossed(&[50, 80, 100], 49, 50, 100), vec![50]);
        assert_eq!(crossed(&[50, 80, 100], 50, 120, 100), vec![80, 100]);
        assert_eq!(crossed(&[50, 80, 100], 100, 130, 100), Vec::<u8>::new());
        assert_eq!(crossed(&[50], 0, 10, 0), Vec::<u8>::new());
    }

    #[test]
    fn every_limit_has_its_own_counter() {
        let now = datetime!(2026-10-11 9:00 UTC);
        let user = Subject::User(Uuid::now_v7());
        let here = limit(user, Kind::SpendBudget, Period::Month, dec!(50));
        let there = limit(user, Kind::SpendBudget, Period::Month, dec!(50));
        assert_ne!(here.counter(now), there.counter(now));
        let rate = limit(user, Kind::RequestRate, Period::Minute, dec!(5));
        let other_rate = limit(user, Kind::RequestRate, Period::Minute, dec!(5));
        assert_ne!(rate.counter(now), other_rate.counter(now));
        assert_eq!(rate.counter(now), format!("lim:{}:minute", rate.id));
        assert_eq!(
            here.counter(now),
            here.counter(datetime!(2026-10-31 23:59:59 UTC))
        );
        assert_ne!(
            here.counter(now),
            here.counter(datetime!(2026-11-01 0:00 UTC))
        );
        let total = limit(user, Kind::TokenBudget, Period::Total, dec!(5));
        assert_eq!(total.counter(now), format!("lim:{}:total", total.id));
    }

    #[test]
    fn counters_and_descriptions() {
        let mut l = limit(
            Subject::Key(Uuid::nil()),
            Kind::SpendBudget,
            Period::Month,
            dec!(50),
        );
        assert_eq!(
            l.counter(datetime!(2026-10-11 9:00 UTC)),
            format!("lim:{}:m2026-10", l.id)
        );
        assert_eq!(l.units(), 50_000_000_000);
        assert_eq!(l.describe(), "monthly spend budget of 50 USD");
        l.kind = Kind::RequestRate;
        l.period = Period::Minute;
        l.amount = dec!(60);
        assert_eq!(l.describe(), "request rate of 60 requests per minute");
        assert!(l.enforced());
    }

    #[test]
    fn tool_call_rates_count_tools_only() {
        let project = Uuid::from_u128(1);
        let key = Uuid::from_u128(2);
        let mut tools = limit(
            Subject::Key(key),
            Kind::ToolCallRate,
            Period::Minute,
            dec!(30),
        );
        let requests = limit(
            Subject::Project(project),
            Kind::RequestRate,
            Period::Minute,
            dec!(30),
        );
        assert_eq!(
            tools.describe(),
            "tool call rate of 30 tool calls per minute"
        );
        assert!(validate(&tools).is_ok());
        let all = [tools.clone(), requests.clone()];
        let model = Uuid::from_u128(3);
        assert_eq!(
            applicable(&all, project, None, Some(key), model),
            [&requests]
        );
        assert_eq!(
            applicable_to_tools(&all, project, None, Some(key)),
            [&tools]
        );
        assert!(applicable_to_tools(&all, project, None, None).is_empty());
        tools.model = Some(model);
        assert_eq!(validate(&tools).map_err(|e| e.field), Err("model"));
        tools.model = None;
        tools.amount = dec!(1.5);
        assert_eq!(validate(&tools).map_err(|e| e.field), Err("amount"));
    }
}
