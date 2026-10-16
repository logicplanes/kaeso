use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use time::{Date, Month, OffsetDateTime, Time};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    RequestRate,
    TokenRate,
    TokenBudget,
    SpendBudget,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::RequestRate => "request_rate",
            Kind::TokenRate => "token_rate",
            Kind::TokenBudget => "token_budget",
            Kind::SpendBudget => "spend_budget",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        [
            Kind::RequestRate,
            Kind::TokenRate,
            Kind::TokenBudget,
            Kind::SpendBudget,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }

    pub fn is_rate(self) -> bool {
        matches!(self, Kind::RequestRate | Kind::TokenRate)
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
        Kind::RequestRate | Kind::TokenRate | Kind::TokenBudget if !l.amount.fract().is_zero() => {
            return Err(invalid("amount", "The amount must be a whole number."));
        }
        _ => {}
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
            subject && l.model.is_none_or(|m| m == model)
        })
        .collect()
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
}
