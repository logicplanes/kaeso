use std::collections::BTreeMap;

use rust_decimal::{Decimal, RoundingStrategy};
use time::Date;

pub const USD: &str = "USD";
pub const EUR: &str = "EUR";

// Enough for the weakest currency the ECB lists (about 20,000 rupiah to a euro) to keep nine
// significant digits.
pub const RATE_PLACES: u32 = 12;

// A billionth of a dollar, the precision spend is counted in.
pub const PRICE_PLACES: u32 = 9;

pub fn is_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Day {
    pub date: Option<Date>,
    pub per_eur: BTreeMap<String, Decimal>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RateBook {
    days: BTreeMap<Date, BTreeMap<String, Decimal>>,
}

impl RateBook {
    pub fn new(rows: impl IntoIterator<Item = (Date, String, Decimal)>) -> RateBook {
        let mut days: BTreeMap<Date, BTreeMap<String, Decimal>> = BTreeMap::new();
        for (date, code, per_eur) in rows {
            if per_eur > Decimal::ZERO {
                days.entry(date)
                    .or_default()
                    .insert(EUR.to_owned(), Decimal::ONE);
                days.entry(date).or_default().insert(code, per_eur);
            }
        }
        // A day without the dollar can't give dollar rates.
        days.retain(|_, rates| rates.contains_key(USD));
        RateBook { days }
    }

    pub fn is_empty(&self) -> bool {
        self.days.is_empty()
    }

    pub fn latest_day(&self) -> Option<Date> {
        self.days.keys().next_back().copied()
    }

    pub fn first_day(&self) -> Option<Date> {
        self.days.keys().next().copied()
    }

    pub fn currencies(&self) -> Vec<String> {
        self.days
            .values()
            .next_back()
            .map(|rates| rates.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn latest(&self, code: &str) -> Option<Decimal> {
        let (_, rates) = self.days.iter().next_back()?;
        usd_rate(rates, code)
    }

    // The newest day that lists `code`: the ECB stops publishing a currency now and then.
    pub fn last(&self, code: &str) -> Option<Decimal> {
        self.days
            .values()
            .rev()
            .find_map(|rates| usd_rate(rates, code))
    }

    pub fn on(&self, code: &str, day: Date) -> Option<(Date, Decimal)> {
        let (date, rates) = self.days.range(..=day).next_back()?;
        usd_rate(rates, code).map(|r| (*date, r))
    }

    pub fn convert(&self, usd: Decimal, code: &str, day: Date) -> Option<Decimal> {
        if code == USD {
            return Some(usd);
        }
        let rate = self
            .on(code, day)
            .map(|(_, r)| r)
            .or_else(|| self.last(code))?;
        usd.checked_mul(rate)
    }

    pub fn from_usd(&self, usd: Decimal, code: &str) -> Option<Decimal> {
        if code == USD {
            return Some(usd);
        }
        usd.checked_mul(self.last(code)?)
    }

    pub fn to_usd(&self, amount: Decimal, code: &str) -> Option<Decimal> {
        if code == USD {
            return Some(amount);
        }
        let rate = self.last(code)?;
        Some(
            amount
                .checked_div(rate)?
                .round_dp_with_strategy(PRICE_PLACES, RoundingStrategy::MidpointNearestEven)
                .normalize(),
        )
    }
}

fn usd_rate(per_eur: &BTreeMap<String, Decimal>, code: &str) -> Option<Decimal> {
    let usd = *per_eur.get(USD)?;
    let other = *per_eur.get(code)?;
    if usd <= Decimal::ZERO {
        return None;
    }
    Some(
        (other / usd)
            .round_dp_with_strategy(RATE_PLACES, RoundingStrategy::MidpointNearestEven)
            .normalize(),
    )
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("the file has no day of rates")]
    Empty,
    #[error("`{0}` is not a date")]
    Date(String),
    #[error("the rate of {0} is not a positive number")]
    Rate(String),
    #[error("the day {0} has no US dollar rate")]
    NoDollar(String),
}

// The ECB writes single quotes in `eurofxref-daily.xml` and double quotes in the history file.
pub fn parse_ecb(xml: &str) -> Result<Vec<Day>, ParseError> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let mut days: Vec<Day> = Vec::new();
    for tag in xml.split('<').filter(|t| t.starts_with("Cube ")) {
        if let Some(time) = attribute(tag, "time") {
            let date = Date::parse(time, &format).map_err(|_| ParseError::Date(time.into()))?;
            days.push(Day {
                date: Some(date),
                per_eur: BTreeMap::new(),
            });
        } else if let (Some(code), Some(rate)) =
            (attribute(tag, "currency"), attribute(tag, "rate"))
        {
            let Some(day) = days.last_mut() else {
                continue;
            };
            let value: Decimal = rate
                .parse()
                .ok()
                .filter(|v: &Decimal| *v > Decimal::ZERO)
                .ok_or_else(|| ParseError::Rate(code.into()))?;
            if is_code(code) {
                day.per_eur.insert(code.to_owned(), value);
            }
        }
    }
    if days.is_empty() {
        return Err(ParseError::Empty);
    }
    if let Some(day) = days.iter().find(|d| !d.per_eur.contains_key(USD)) {
        return Err(ParseError::NoDollar(
            day.date.map(|d| d.to_string()).unwrap_or_default(),
        ));
    }
    Ok(days)
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{name}="))? + name.len() + 1;
    let rest = tag.get(at..)?;
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let rest = rest.get(1..)?;
    rest.get(..rest.find(quote)?)
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::date;

    use super::*;

    const DAILY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <gesmes:Envelope><Cube>\n\t<Cube time='2026-11-16'>\n\
        \t\t<Cube currency='USD' rate='1.1014'/>\n\t\t<Cube currency='JPY' rate='175.78'/>\n\
        \t\t<Cube currency='GBP' rate='0.83450'/>\n\t\t<Cube currency='IDR' rate='20110.00'/>\n\
        \t</Cube></Cube></gesmes:Envelope>";

    const HISTORY: &str = "<Cube><Cube time=\"2026-11-16\"><Cube currency=\"USD\" rate=\"1.1014\"/>\
        <Cube currency=\"GBP\" rate=\"0.83450\"/></Cube><Cube time=\"2026-11-13\">\
        <Cube currency=\"USD\" rate=\"1.0900\"/><Cube currency=\"GBP\" rate=\"0.8400\"/></Cube></Cube>";

    fn book(xml: &str) -> RateBook {
        RateBook::new(parse_ecb(xml).unwrap().into_iter().flat_map(|d| {
            let date = d.date.unwrap();
            d.per_eur.into_iter().map(move |(c, r)| (date, c, r))
        }))
    }

    #[test]
    fn both_files_are_read_whatever_their_quotes() {
        let daily = parse_ecb(DAILY).unwrap();
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].date, Some(date!(2026 - 11 - 16)));
        assert_eq!(daily[0].per_eur["JPY"], dec!(175.78));
        assert_eq!(daily[0].per_eur.len(), 4);
        let history = parse_ecb(HISTORY).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].per_eur["USD"], dec!(1.09));
    }

    #[test]
    fn a_broken_file_is_refused() {
        assert_eq!(
            parse_ecb("<html>Maintenance</html>"),
            Err(ParseError::Empty)
        );
        assert_eq!(
            parse_ecb("<Cube time='2026-13-01'>"),
            Err(ParseError::Date("2026-13-01".into()))
        );
        assert_eq!(
            parse_ecb("<Cube time='2026-11-16'><Cube currency='USD' rate='-1'/>"),
            Err(ParseError::Rate("USD".into()))
        );
        assert_eq!(
            parse_ecb("<Cube time='2026-11-16'><Cube currency='GBP' rate='0.8'/>"),
            Err(ParseError::NoDollar("2026-11-16".into()))
        );
    }

    #[test]
    fn dollar_rates_are_cross_rates_of_the_same_day() {
        let b = book(DAILY);
        // EUR→GBP / EUR→USD and 1 / EUR→USD, to twelve places.
        assert_eq!(b.latest("GBP"), Some(dec!(0.757672053750)));
        assert_eq!(b.latest("EUR"), Some(dec!(0.907935355003)));
        assert_eq!(b.latest("USD"), Some(dec!(1)));
        assert_eq!(b.latest("IDR"), Some(dec!(18258.579989104776)));
        assert_eq!(b.latest("CHF"), None);
        assert_eq!(
            b.currencies(),
            ["EUR", "GBP", "IDR", "JPY", "USD"].map(String::from)
        );
    }

    #[test]
    fn a_day_takes_the_rates_of_the_last_working_day_and_older_days_the_latest() {
        let b = book(HISTORY);
        assert_eq!(b.first_day(), Some(date!(2026 - 11 - 13)));
        assert_eq!(b.latest_day(), Some(date!(2026 - 11 - 16)));
        // A Saturday reads Friday's rates.
        let saturday = b.on("GBP", date!(2026 - 11 - 14)).unwrap();
        assert_eq!(saturday.0, date!(2026 - 11 - 13));
        assert_eq!(
            b.convert(dec!(10), "GBP", date!(2026 - 11 - 14)),
            Some(dec!(7.706422018350))
        );
        // A day before the first rates uses the latest.
        assert_eq!(b.on("GBP", date!(2026 - 11 - 01)), None);
        assert_eq!(
            b.convert(dec!(10), "GBP", date!(2026 - 11 - 01)),
            b.latest("GBP").map(|r| r * dec!(10))
        );
        assert_eq!(b.convert(dec!(10), "CHF", date!(2026 - 11 - 16)), None);
        assert_eq!(
            b.convert(dec!(10), "USD", date!(2026 - 11 - 01)),
            Some(dec!(10))
        );
    }

    #[test]
    fn a_currency_the_ecb_stopped_listing_keeps_its_last_rate() {
        let b = book(
            "<Cube time='2026-11-16'><Cube currency='USD' rate='1.1014'/></Cube>\
             <Cube time='2026-11-13'><Cube currency='USD' rate='1.0900'/>\
             <Cube currency='RUB' rate='109.00'/></Cube>",
        );
        assert_eq!(b.latest("RUB"), None);
        assert_eq!(b.last("RUB"), Some(dec!(100)));
        assert_eq!(b.from_usd(dec!(2), "RUB"), Some(dec!(200)));
        assert_eq!(b.to_usd(dec!(250), "RUB"), Some(dec!(2.5)));
        assert_eq!(
            b.convert(dec!(1), "RUB", date!(2026 - 11 - 16)),
            Some(dec!(100))
        );
        assert_eq!(b.from_usd(dec!(2), "USD"), Some(dec!(2)));
        assert_eq!(b.from_usd(dec!(2), "CHF"), None);
    }

    #[test]
    fn a_price_in_another_currency_becomes_dollars_to_a_billionth() {
        let b = book(DAILY);
        // €2.50 per million tokens at 1.1014 dollars to the euro.
        assert_eq!(b.to_usd(dec!(2.50), "EUR"), Some(dec!(2.7535)));
        // ¥100 at 175.78 yen to the euro: 0.626578677 dollars, rounded half to even.
        assert_eq!(b.to_usd(dec!(100), "JPY"), Some(dec!(0.626578678)));
        assert_eq!(b.to_usd(dec!(3), "USD"), Some(dec!(3)));
        assert_eq!(b.to_usd(dec!(3), "CHF"), None);
        assert_eq!(RateBook::default().to_usd(dec!(3), "EUR"), None);
        assert_eq!(RateBook::default().to_usd(dec!(3), "USD"), Some(dec!(3)));
    }

    #[test]
    fn an_amount_too_large_to_convert_has_no_value_in_the_other_currency() {
        let b = book(DAILY);
        assert_eq!(b.to_usd(Decimal::MAX, "EUR"), None);
        assert_eq!(b.from_usd(Decimal::MAX, "IDR"), None);
        assert_eq!(b.convert(Decimal::MAX, "IDR", date!(2026 - 11 - 16)), None);
        assert_eq!(b.to_usd(Decimal::MAX, "USD"), Some(Decimal::MAX));
    }

    #[test]
    fn prices_set_in_euros_are_costed_in_dollars_and_unknown_without_a_rate() {
        use crate::catalogue::{Fields, PriceTier, UnitPrice};
        let b = book(DAILY);
        let entered = Fields {
            currency: Some("EUR".into()),
            input_price: Some(dec!(2.50)),
            output_price: Some(dec!(10)),
            price_tiers: Some(vec![PriceTier {
                context_above: 200_000,
                input_price: Some(dec!(5)),
                ..PriceTier::default()
            }]),
            unit_prices: Some(vec![UnitPrice {
                per: "image".into(),
                of: "output".into(),
                usd: dec!(0.04),
                ..UnitPrice::default()
            }]),
            context_limit: Some(128_000),
            ..Fields::default()
        };
        let usd = entered.clone().in_usd(&b);
        assert_eq!(usd.currency, None);
        assert_eq!(usd.input_price, Some(dec!(2.7535)));
        assert_eq!(usd.output_price, Some(dec!(11.014)));
        assert_eq!(usd.price_tiers.unwrap()[0].input_price, Some(dec!(5.507)));
        assert_eq!(usd.unit_prices.unwrap()[0].usd, dec!(0.044056));
        assert_eq!(usd.context_limit, Some(128_000));
        let unknown = entered.in_usd(&RateBook::default());
        assert_eq!((unknown.input_price, unknown.unit_prices), (None, None));
        assert_eq!(unknown.context_limit, Some(128_000));
        let plain = Fields {
            input_price: Some(dec!(3)),
            ..Fields::default()
        };
        assert_eq!(plain.clone().in_usd(&RateBook::default()), plain);
    }

    #[test]
    fn codes_are_three_capitals() {
        assert!(is_code("EUR"));
        for bad in ["eur", "EURO", "E1R", ""] {
            assert!(!is_code(bad), "{bad}");
        }
    }
}
