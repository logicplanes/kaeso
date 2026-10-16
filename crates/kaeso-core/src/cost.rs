use kaeso_kpp::Usage;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::catalogue::Fields;

const MILLION: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);
const BILLION: Decimal = Decimal::from_parts(1_000_000_000, 0, 0, false, 0);

pub fn cost(usage: &Usage, prices: &Fields) -> Decimal {
    let p = |d: Option<Decimal>| d.unwrap_or(Decimal::ZERO);
    let total = Decimal::from(usage.input_tokens) * p(prices.input_price)
        + Decimal::from(usage.output_tokens) * p(prices.output_price)
        + Decimal::from(usage.cache_read_tokens) * p(prices.cache_read_price)
        + Decimal::from(usage.cache_write_tokens) * p(prices.cache_write_price);
    (total / MILLION).normalize()
}

pub fn nano_usd(cost: Decimal) -> i64 {
    (cost * BILLION).ceil().to_i64().unwrap_or(i64::MAX)
}

pub fn from_nano(nano: i64) -> Decimal {
    (Decimal::from(nano) / BILLION).normalize()
}

pub fn total_tokens(usage: &Usage) -> u64 {
    usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.cache_read_tokens)
        .saturating_add(usage.cache_write_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn costs_follow_d1() {
        let mut u = Usage::default();
        u.input_tokens = 1_000;
        u.output_tokens = 500;
        u.cache_read_tokens = 10_000;
        u.cache_write_tokens = 2_000;
        u.reasoning_tokens = Some(300);
        let prices = Fields {
            input_price: Some(dec!(3)),
            output_price: Some(dec!(15)),
            cache_read_price: Some(dec!(0.3)),
            cache_write_price: Some(dec!(3.75)),
            ..Fields::default()
        };
        assert_eq!(cost(&u, &prices), dec!(0.021));
        assert_eq!(nano_usd(dec!(0.021)), 21_000_000);
        assert_eq!(nano_usd(dec!(0.0000000001)), 1, "rounded up");
        assert_eq!(from_nano(21_000_000), dec!(0.021));
        assert_eq!(total_tokens(&u), 13_500);
        assert_eq!(cost(&u, &Fields::default()), Decimal::ZERO);
    }
}
