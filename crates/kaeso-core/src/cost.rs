use kaeso_kpp::Usage;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::catalogue::Fields;

const MILLION: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);
const BILLION: Decimal = Decimal::from_parts(1_000_000_000, 0, 0, false, 0);
const TRILLION: Decimal = Decimal::from_parts(3_567_587_328, 232, 0, false, 0);
const RAW_SCALE: u32 = 12;

pub const COST_RAW_MAX: i128 = 10_i128.pow(38) - 1;

pub fn cost(usage: &Usage, prices: &Fields) -> Decimal {
    let total = [
        (usage.input_tokens, prices.input_price),
        (usage.output_tokens, prices.output_price),
        (usage.cache_read_tokens, prices.cache_read_price),
        (usage.cache_write_tokens, prices.cache_write_price),
    ]
    .into_iter()
    .try_fold(Decimal::ZERO, |sum, (tokens, price)| {
        Decimal::from(tokens)
            .checked_mul(price.unwrap_or(Decimal::ZERO))
            .and_then(|c| sum.checked_add(c))
    })
    .unwrap_or(Decimal::MAX);
    (total / MILLION).normalize()
}

pub fn nano_usd(cost: Decimal) -> i64 {
    cost.checked_mul(BILLION)
        .and_then(|n| n.ceil().to_i64())
        .unwrap_or(i64::MAX)
}

pub fn from_nano(nano: i64) -> Decimal {
    (Decimal::from(nano) / BILLION).normalize()
}

pub fn cost_raw(cost: Decimal) -> Option<i128> {
    cost.checked_mul(TRILLION)
        .and_then(|n| n.round().to_i128())
        .filter(|raw| (-COST_RAW_MAX..=COST_RAW_MAX).contains(raw))
}

pub fn saturated_cost_raw(cost: Decimal) -> i128 {
    cost_raw(cost).unwrap_or(if cost.is_sign_negative() {
        -COST_RAW_MAX
    } else {
        COST_RAW_MAX
    })
}

pub fn cost_from_raw(raw: i128) -> Decimal {
    Decimal::try_from_i128_with_scale(raw, RAW_SCALE)
        .map(|d| d.normalize())
        .unwrap_or(if raw < 0 { Decimal::MIN } else { Decimal::MAX })
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
    fn costs_are_per_million_tokens_and_round_up_to_nano_dollars() {
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
        let huge = Fields {
            input_price: Some(Decimal::MAX),
            output_price: Some(Decimal::MAX),
            ..Fields::default()
        };
        assert_eq!(cost(&u, &huge), (Decimal::MAX / MILLION).normalize());
        assert_eq!(nano_usd(Decimal::MAX), i64::MAX);
    }

    #[test]
    fn raw_costs_have_twelve_decimals_and_stay_within_the_column() {
        assert_eq!(TRILLION, Decimal::from(1_000_000_000_000_i64));
        assert_eq!(cost_raw(dec!(0.021)), Some(21_000_000_000));
        assert_eq!(cost_raw(dec!(0.0000000000005)), Some(0));
        assert_eq!(cost_from_raw(21_000_000_000), dec!(0.021));
        assert_eq!(
            cost_from_raw(cost_raw(dec!(123.456789012345)).unwrap()),
            dec!(123.456789012345)
        );
        assert_eq!(cost_raw(Decimal::MAX), None);
        assert_eq!(saturated_cost_raw(Decimal::MAX), COST_RAW_MAX);
        assert_eq!(saturated_cost_raw(Decimal::MIN), -COST_RAW_MAX);
        let largest = Decimal::MAX / TRILLION;
        assert!(cost_raw(largest).is_some_and(|r| r > 0 && r <= COST_RAW_MAX));
        assert_eq!(cost_from_raw(COST_RAW_MAX), Decimal::MAX);
        assert_eq!(cost_from_raw(-COST_RAW_MAX), Decimal::MIN);
        assert_eq!(cost_from_raw(i128::MAX), Decimal::MAX);
    }
}
