use kaeso_kpp::{RequestKind, Usage};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::catalogue::{Fields, UnitPrice};

mod unpriced;

pub use unpriced::{Unpriced, unpriced};

const MILLION: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);
const BILLION: Decimal = Decimal::from_parts(1_000_000_000, 0, 0, false, 0);
const TRILLION: Decimal = Decimal::from_parts(3_567_587_328, 232, 0, false, 0);
const RAW_SCALE: u32 = 12;

pub const COST_RAW_MAX: i128 = 10_i128.pow(38) - 1;

// Anthropic bills a one-hour cache write at twice the input price, and so does Claude through
// other providers: https://platform.claude.com/docs/en/about-claude/pricing
const ONE_HOUR_WRITE_FACTOR: Decimal = Decimal::from_parts(2, 0, 0, false, 0);

pub fn one_hour_cache_write_price(prices: &Fields) -> Option<Decimal> {
    prices.cache_write_1h_price.or_else(|| {
        prices
            .input_price
            .and_then(|p| p.checked_mul(ONE_HOUR_WRITE_FACTOR))
    })
}

pub fn prices_for_prompt(prices: &Fields, prompt_tokens: u64) -> Fields {
    let tier = prices
        .price_tiers
        .iter()
        .flatten()
        .filter(|t| prompt_tokens > t.context_above)
        .max_by_key(|t| t.context_above);
    let Some(tier) = tier else {
        return prices.clone();
    };
    Fields {
        input_price: tier.input_price.or(prices.input_price),
        output_price: tier.output_price.or(prices.output_price),
        cache_read_price: tier.cache_read_price.or(prices.cache_read_price),
        cache_write_price: tier.cache_write_price.or(prices.cache_write_price),
        cache_write_1h_price: tier.cache_write_1h_price.or(if tier.input_price.is_some() {
            None
        } else {
            prices.cache_write_1h_price
        }),
        price_tiers: None,
        ..prices.clone()
    }
}

pub fn unpriced_model(
    prices: &Fields,
    set_by_hand: &Fields,
    kind: RequestKind,
    model: &str,
) -> Option<Unpriced> {
    let priced = |price: Option<Decimal>, by_hand: Option<Decimal>| {
        by_hand.is_some() || price.is_some_and(|p| p > Decimal::ZERO)
    };
    let input = priced(prices.input_price, set_by_hand.input_price);
    let output = priced(prices.output_price, set_by_hand.output_price);
    let by_unit = set_by_hand.unit_prices.is_some()
        || prices.unit_prices.as_ref().is_some_and(|u| !u.is_empty());
    let known = match kind {
        RequestKind::Generate => input && output,
        RequestKind::Embed => input,
        RequestKind::Image => {
            by_unit || output || prices.output_image_price.is_some_and(|p| p > Decimal::ZERO)
        }
        RequestKind::Speech => {
            by_unit || output || prices.output_audio_price.is_some_and(|p| p > Decimal::ZERO)
        }
        RequestKind::Transcription => {
            by_unit || input || prices.input_audio_price.is_some_and(|p| p > Decimal::ZERO)
        }
        _ => true,
    };
    (!known).then(|| Unpriced {
        param: "model",
        feature: model.to_owned(),
    })
}

pub fn cost(usage: &Usage, prices: &Fields) -> Decimal {
    let prices = &prices_for_prompt(prices, usage.total_input_tokens());
    let one_hour = usage
        .cache_write_breakdown
        .as_ref()
        .map_or(0, |b| b.ephemeral_1h.min(usage.cache_write_tokens));
    // Image tokens are a share of each count, billed at the image price when the model has one.
    let image_in = usage
        .input_image_tokens
        .unwrap_or(0)
        .min(usage.input_tokens);
    let image_out = usage
        .output_image_tokens
        .unwrap_or(0)
        .min(usage.output_tokens);
    let image_cached = usage
        .cache_read_image_tokens
        .unwrap_or(0)
        .min(usage.cache_read_tokens);
    // Audio tokens are a share of each count too, billed at the audio price when there is one.
    let audio_in = usage
        .input_audio_tokens
        .unwrap_or(0)
        .min(usage.input_tokens - image_in);
    let audio_out = usage
        .output_audio_tokens
        .unwrap_or(0)
        .min(usage.output_tokens - image_out);
    let total = [
        (usage.input_tokens - image_in - audio_in, prices.input_price),
        (image_in, prices.input_image_price.or(prices.input_price)),
        (audio_in, prices.input_audio_price.or(prices.input_price)),
        (
            usage.output_tokens - image_out - audio_out,
            prices.output_price,
        ),
        (image_out, prices.output_image_price.or(prices.output_price)),
        (audio_out, prices.output_audio_price.or(prices.output_price)),
        (
            usage.cache_read_tokens - image_cached,
            prices.cache_read_price,
        ),
        (
            image_cached,
            prices.cache_read_image_price.or(prices.cache_read_price),
        ),
        (
            usage.cache_write_tokens.saturating_sub(one_hour),
            prices.cache_write_price,
        ),
        (one_hour, one_hour_cache_write_price(prices)),
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

/// What a media request took and produced, for the prices per unit.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaUnits {
    pub input_images: u64,
    pub output_images: u64,
    /// The pixels of one output image, when its size is known.
    pub output_pixels: Option<u64>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    /// The characters of text a request asked to speak.
    #[serde(default)]
    pub characters: u64,
    /// The length of the audio a request sent, in milliseconds.
    #[serde(default)]
    pub input_ms: u64,
    /// The length of the audio or video a request made, in milliseconds.
    #[serde(default)]
    pub output_ms: u64,
    /// The videos a request made.
    #[serde(default)]
    pub output_videos: u64,
    /// Whether made video has sound, for prices that depend on it.
    #[serde(default)]
    pub audio: Option<bool>,
    /// The audio format or transcript form, as the request names it, for the request log.
    #[serde(default)]
    pub format: Option<String>,
    /// Whether the provider rewrote the prompt at a price that doubles the price per image, as
    /// QwenCloud's Z-Image does with `prompt_extend`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub extended_prompt: bool,
}

const MS_PER_SECOND: Decimal = Decimal::from_parts(1_000, 0, 0, false, 0);
const MS_PER_MINUTE: Decimal = Decimal::from_parts(60_000, 0, 0, false, 0);
const MS_PER_HOUR: Decimal = Decimal::from_parts(3_600_000, 0, 0, false, 0);

impl MediaUnits {
    fn count(&self, per: &str, of: &str) -> Option<Decimal> {
        let ms = match of {
            "input" => self.input_ms,
            _ => self.output_ms,
        };
        let duration = |per_unit: Decimal| (ms > 0).then(|| Decimal::from(ms) / per_unit);
        match (per, of) {
            ("image", "input") => Some(Decimal::from(self.input_images)),
            ("image", "output") => Some(Decimal::from(
                self.output_images
                    .saturating_mul(if self.extended_prompt { 2 } else { 1 }),
            )),
            ("megapixel", "output") => {
                let pixels = self.output_pixels?.checked_mul(self.output_images)?;
                Some(Decimal::from(pixels) / MILLION)
            }
            ("video", "output") if self.output_videos > 0 => {
                Some(Decimal::from(self.output_videos))
            }
            ("million_characters", "input") if self.characters > 0 => {
                Some(Decimal::from(self.characters) / MILLION)
            }
            ("second", _) => duration(MS_PER_SECOND),
            ("minute", _) => duration(MS_PER_MINUTE),
            ("hour", _) => duration(MS_PER_HOUR),
            _ => None,
        }
    }

    fn condition(&self, name: &str) -> Option<&str> {
        match name {
            "size" => self.size.as_deref(),
            "quality" => self.quality.as_deref(),
            "resolution" => self.resolution.as_deref(),
            _ => None,
        }
    }

    /// The unit the request log names these units by, with what went in and what came out:
    /// images, characters of speech, milliseconds of audio, or videos and their milliseconds.
    pub fn logged(&self) -> (&'static str, u64, u64) {
        if self.output_videos > 0 {
            ("video", self.output_videos, self.output_ms)
        } else if self.input_images > 0 || self.output_images > 0 || self.size.is_some() {
            ("image", self.input_images, self.output_images)
        } else if self.characters > 0 {
            ("character", self.characters, self.output_ms)
        } else {
            ("millisecond", self.input_ms, self.output_ms)
        }
    }
}

enum Fit {
    Exact(usize),
    Unknown,
    No,
}

fn fit(price: &UnitPrice, units: &MediaUnits) -> Fit {
    let mut named = 0;
    let mut unknown = false;
    for (name, wanted) in [
        ("size", &price.size),
        ("quality", &price.quality),
        ("resolution", &price.resolution),
    ] {
        let Some(wanted) = wanted else { continue };
        match units.condition(name) {
            Some(have) if have.eq_ignore_ascii_case(wanted) => named += 1,
            Some(_) => return Fit::No,
            None => unknown = true,
        }
    }
    if let Some(wanted) = price.audio {
        match units.audio {
            Some(have) if have == wanted => named += 1,
            _ => return Fit::No,
        }
    }
    if let Some(wanted) = price.duration {
        match units.output_ms {
            0 => unknown = true,
            ms if ms.div_ceil(1000) == u64::from(wanted) => named += 1,
            _ => return Fit::No,
        }
    }
    if unknown {
        Fit::Unknown
    } else {
        Fit::Exact(named)
    }
}

/// The price per unit KPMR or an override lists for one kind of unit: the one whose
/// conditions match the request most closely. When the request names no value for a
/// condition, the lowest listed price applies, since a provider's default is its lowest tier.
pub fn unit_price<'a>(
    prices: &'a [UnitPrice],
    units: &MediaUnits,
    per: &str,
    of: &str,
) -> Option<&'a UnitPrice> {
    let candidates = prices.iter().filter(|p| p.per == per && p.of == of);
    let mut exact: Option<(usize, &UnitPrice)> = None;
    let mut unknown: Option<&UnitPrice> = None;
    for p in candidates {
        match fit(p, units) {
            Fit::Exact(n) if exact.is_none_or(|(m, _)| n > m) => exact = Some((n, p)),
            Fit::Unknown if unknown.is_none_or(|u| p.usd < u.usd) => unknown = Some(p),
            _ => {}
        }
    }
    exact.map(|(_, p)| p).or(unknown)
}

/// The cost of a media request's units at the model's prices per unit.
pub fn media_cost(units: &MediaUnits, prices: &Fields) -> Decimal {
    let Some(list) = prices.unit_prices.as_deref() else {
        return Decimal::ZERO;
    };
    let mut total = Decimal::ZERO;
    for per in crate::catalogue::UNITS {
        for of in ["input", "output"] {
            let (Some(count), Some(price)) =
                (units.count(per, of), unit_price(list, units, per, of))
            else {
                continue;
            };
            // Groq bills at least 10 seconds of audio a request, whatever its length.
            let count = match (price.minimum_seconds, *per) {
                (Some(min), "second" | "minute" | "hour") => {
                    let per_unit = match *per {
                        "second" => MS_PER_SECOND,
                        "minute" => MS_PER_MINUTE,
                        _ => MS_PER_HOUR,
                    };
                    let least = Decimal::from(min) * MS_PER_SECOND / per_unit;
                    count.max(least)
                }
                _ => count,
            };
            total = count
                .checked_mul(price.usd)
                .and_then(|c| total.checked_add(c))
                .unwrap_or(Decimal::MAX);
        }
    }
    total.normalize()
}

/// Whether a request's cost is known: its model has a price for everything it measured. A
/// request that measured nothing costs a known 0; one that measured units or tokens its model
/// has no price for, such as a Veo clip or the minutes a voice changer heard, has an unknown
/// cost, which is not 0.
pub fn is_priced(usage: &Usage, prices: Option<&Fields>, media: Option<&MediaUnits>) -> bool {
    let measured_tokens = usage.input_tokens > 0 || usage.output_tokens > 0;
    let measured_media = media.is_some_and(|m| {
        m.input_images > 0
            || m.output_images > 0
            || m.characters > 0
            || m.input_ms > 0
            || m.output_ms > 0
            || m.output_videos > 0
    });
    if !measured_tokens && !measured_media {
        return true;
    }
    let Some(prices) = prices else {
        return false;
    };
    cost_with_media(usage, prices, media) > Decimal::ZERO
}

/// The cost of a request: its tokens, and its media units when it has any.
pub fn cost_with_media(usage: &Usage, prices: &Fields, media: Option<&MediaUnits>) -> Decimal {
    let tokens = cost(usage, prices);
    match media {
        Some(units) => tokens
            .checked_add(media_cost(units, prices))
            .unwrap_or(Decimal::MAX)
            .normalize(),
        None => tokens,
    }
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
    fn a_model_is_unpriced_without_a_positive_price_unless_one_was_set_by_hand() {
        let prices = |input_price: Option<Decimal>, output_price: Option<Decimal>| Fields {
            input_price,
            output_price,
            ..Fields::default()
        };
        let none = Fields::default();
        let generate = RequestKind::Generate;
        let priced = prices(Some(dec!(1)), Some(dec!(2)));
        assert_eq!(unpriced_model(&priced, &none, generate, "m"), None);
        for unpriced in [
            prices(None, Some(dec!(2))),
            prices(Some(dec!(1)), None),
            prices(Some(dec!(0)), Some(dec!(2))),
            prices(Some(dec!(1)), Some(dec!(0))),
        ] {
            let u = unpriced_model(&unpriced, &none, generate, "gpt-x").unwrap();
            assert_eq!((u.param, u.feature.as_str()), ("model", "gpt-x"));
        }
        let free = prices(Some(dec!(0)), Some(dec!(0)));
        assert_eq!(
            unpriced_model(&free, &free, generate, "m"),
            None,
            "an admin made it free"
        );
        let embed = prices(Some(dec!(1)), None);
        assert_eq!(unpriced_model(&embed, &none, RequestKind::Embed, "m"), None);
        assert!(unpriced_model(&none, &none, RequestKind::Embed, "m").is_some());
        assert_eq!(
            unpriced_model(&none, &none, RequestKind::CountTokens, "m"),
            None
        );
    }

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

    #[test]
    fn one_hour_cache_writes_cost_their_own_price() {
        let prices = Fields {
            input_price: Some(dec!(3)),
            output_price: Some(dec!(15)),
            cache_read_price: Some(dec!(0.3)),
            cache_write_price: Some(dec!(3.75)),
            cache_write_1h_price: Some(dec!(7)),
            ..Fields::default()
        };
        let mut u = Usage::default();
        u.cache_write_tokens = 3_000_000;
        assert_eq!(cost(&u, &prices), dec!(11.25));
        u.cache_write_breakdown = Some(kaeso_kpp::CacheWriteBreakdown::new(1_000_000, 2_000_000));
        assert_eq!(cost(&u, &prices), dec!(17.75));
        u.cache_write_breakdown = Some(kaeso_kpp::CacheWriteBreakdown::new(0, 9_000_000));
        assert_eq!(cost(&u, &prices), dec!(21));
    }

    #[test]
    fn the_one_hour_write_price_is_kpmrs_or_an_overrides_else_twice_the_input_price() {
        let kpmr = Fields {
            input_price: Some(dec!(3)),
            cache_write_price: Some(dec!(3.75)),
            cache_write_1h_price: Some(dec!(6.5)),
            ..Fields::default()
        };
        assert_eq!(one_hour_cache_write_price(&kpmr), Some(dec!(6.5)));
        let overridden = Fields {
            cache_write_1h_price: Some(dec!(9)),
            ..Fields::default()
        }
        .over(&kpmr);
        assert_eq!(one_hour_cache_write_price(&overridden), Some(dec!(9)));
        let unlisted = Fields {
            cache_write_1h_price: None,
            ..kpmr.clone()
        };
        assert_eq!(one_hour_cache_write_price(&unlisted), Some(dec!(6)));
        let mut u = Usage::default();
        u.cache_write_tokens = 1_000_000;
        u.cache_write_breakdown = Some(kaeso_kpp::CacheWriteBreakdown::new(0, 1_000_000));
        assert_eq!(cost(&u, &unlisted), dec!(6));
        let unpriced_input = Fields {
            cache_write_price: Some(dec!(3.75)),
            ..Fields::default()
        };
        assert_eq!(one_hour_cache_write_price(&unpriced_input), None);
    }

    fn tiered() -> Fields {
        Fields {
            input_price: Some(dec!(10)),
            output_price: Some(dec!(50)),
            cache_read_price: Some(dec!(1)),
            cache_write_price: Some(dec!(12.5)),
            cache_write_1h_price: Some(dec!(20)),
            price_tiers: Some(vec![
                crate::catalogue::PriceTier {
                    context_above: 272_000,
                    input_price: Some(dec!(20)),
                    output_price: Some(dec!(75)),
                    cache_read_price: None,
                    cache_write_price: Some(dec!(25)),
                    cache_write_1h_price: None,
                },
                crate::catalogue::PriceTier {
                    context_above: 512_000,
                    output_price: Some(dec!(90)),
                    ..crate::catalogue::PriceTier::default()
                },
            ]),
            ..Fields::default()
        }
    }

    #[test]
    fn a_prompt_longer_than_a_tier_pays_that_tiers_prices() {
        let prices = tiered();
        let at = prices_for_prompt(&prices, 272_000);
        assert_eq!(
            at, prices,
            "a prompt of exactly 272,000 tokens is not above it"
        );
        let below = prices_for_prompt(&prices, 1_000);
        assert_eq!(
            (below.input_price, below.output_price),
            (Some(dec!(10)), Some(dec!(50)))
        );
        let above = prices_for_prompt(&prices, 272_001);
        assert_eq!(above.input_price, Some(dec!(20)));
        assert_eq!(above.output_price, Some(dec!(75)));
        assert_eq!(
            above.cache_read_price,
            Some(dec!(1)),
            "a null price is the base price"
        );
        assert_eq!(above.cache_write_price, Some(dec!(25)));
        assert_eq!(
            one_hour_cache_write_price(&above),
            Some(dec!(40)),
            "twice the tier's input price, not the base list price"
        );
        let highest = prices_for_prompt(&prices, 600_000);
        assert_eq!(
            highest.output_price,
            Some(dec!(90)),
            "the highest tier exceeded"
        );
        assert_eq!(
            highest.input_price,
            Some(dec!(10)),
            "its null input is the base price"
        );
        assert_eq!(
            one_hour_cache_write_price(&highest),
            Some(dec!(20)),
            "without a tier input price the base one-hour price holds"
        );
        let mut by_hand = prices.clone();
        if let Some(tiers) = by_hand.price_tiers.as_mut() {
            tiers[0].cache_write_1h_price = Some(dec!(30));
        }
        assert_eq!(
            one_hour_cache_write_price(&prices_for_prompt(&by_hand, 300_000)),
            Some(dec!(30))
        );
        let untiered = Fields {
            price_tiers: None,
            ..prices.clone()
        };
        assert_eq!(prices_for_prompt(&untiered, 10_000_000), untiered);
    }

    #[test]
    fn every_token_of_a_long_request_pays_the_tier_not_only_those_past_the_threshold() {
        let prices = Fields {
            input_price: Some(dec!(1)),
            output_price: Some(dec!(2)),
            cache_read_price: Some(dec!(0.1)),
            cache_write_price: Some(dec!(1.25)),
            price_tiers: Some(vec![crate::catalogue::PriceTier {
                context_above: 2_000_000,
                input_price: Some(dec!(10)),
                output_price: Some(dec!(20)),
                cache_read_price: Some(dec!(1)),
                cache_write_price: Some(dec!(12.5)),
                cache_write_1h_price: None,
            }]),
            ..Fields::default()
        };
        let mut u = Usage::default();
        u.input_tokens = 1_000_000;
        u.cache_read_tokens = 1_000_000;
        u.cache_write_tokens = 1;
        u.output_tokens = 1_000_000;
        assert_eq!(
            cost(&u, &prices),
            dec!(10) + dec!(1) + dec!(0.0000125) + dec!(20),
            "2,000,001 prompt tokens: all of them and the output at the tier"
        );
        u.cache_write_tokens = 0;
        assert_eq!(cost(&u, &prices), dec!(1) + dec!(0.1) + dec!(2));
    }

    #[test]
    fn the_prompt_length_counts_cached_and_written_tokens_as_well() {
        let prices = tiered();
        let mut u = Usage::default();
        u.input_tokens = 100_000;
        u.cache_read_tokens = 100_000;
        u.cache_write_tokens = 72_000;
        u.output_tokens = 1_000_000;
        assert_eq!(cost(&u, &prices), dec!(52), "272,000 tokens: base prices");
        u.cache_write_tokens = 72_001;
        assert_eq!(
            cost(&u, &prices),
            dec!(78.900025),
            "272,001 tokens: the tier"
        );
        u.cache_write_breakdown = Some(kaeso_kpp::CacheWriteBreakdown::new(1, 72_000));
        assert_eq!(
            cost(&u, &prices),
            dec!(79.980025),
            "one-hour writes at twice 20"
        );
    }

    fn unit(per: &str, of: &str, usd: Decimal) -> UnitPrice {
        UnitPrice {
            per: per.into(),
            of: of.into(),
            usd,
            ..UnitPrice::default()
        }
    }

    #[test]
    fn image_tokens_are_billed_at_image_prices_and_fall_back_to_token_prices() {
        let prices = Fields {
            input_price: Some(dec!(5)),
            output_price: Some(dec!(10)),
            cache_read_price: Some(dec!(1.25)),
            input_image_price: Some(dec!(8)),
            output_image_price: Some(dec!(30)),
            cache_read_image_price: Some(dec!(2)),
            ..Fields::default()
        };
        let mut usage = Usage::new(1_000_000, 1_000_000);
        usage.cache_read_tokens = 1_000_000;
        usage.input_image_tokens = Some(400_000);
        usage.output_image_tokens = Some(1_000_000);
        usage.cache_read_image_tokens = Some(500_000);
        // 0.6 * 5 + 0.4 * 8 + 1 * 30 + 0.5 * 1.25 + 0.5 * 2
        assert_eq!(cost(&usage, &prices), dec!(37.825));
        let plain = Fields {
            input_image_price: None,
            output_image_price: None,
            cache_read_image_price: None,
            ..prices
        };
        assert_eq!(cost(&usage, &plain), dec!(16.25));
        usage.output_image_tokens = Some(5_000_000);
        assert_eq!(cost(&usage, &plain), dec!(16.25));
    }

    #[test]
    fn unit_prices_match_the_closest_conditions_and_add_to_tokens() {
        let mut low = unit("image", "output", dec!(0.04));
        low.resolution = Some("1K".into());
        low.quality = Some("low".into());
        let mut medium = unit("image", "output", dec!(0.05));
        medium.resolution = Some("1K".into());
        medium.quality = Some("medium".into());
        let mut two_k = unit("image", "output", dec!(0.08));
        two_k.resolution = Some("2K".into());
        let input = unit("image", "input", dec!(0.01));
        let prices = Fields {
            unit_prices: Some(vec![low, medium, two_k, input]),
            ..Fields::default()
        };
        let mut units = MediaUnits {
            input_images: 2,
            output_images: 3,
            quality: Some("Medium".into()),
            resolution: Some("1k".into()),
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&units, &prices), dec!(0.17));
        units.resolution = Some("2K".into());
        units.quality = None;
        assert_eq!(media_cost(&units, &prices), dec!(0.26));
        units.resolution = None;
        assert_eq!(media_cost(&units, &prices), dec!(0.14));
        units.resolution = Some("4K".into());
        assert_eq!(media_cost(&units, &prices), dec!(0.02));
        let usage = Usage::new(10, 0);
        let both = Fields {
            input_price: Some(dec!(1)),
            ..prices.clone()
        };
        assert_eq!(cost_with_media(&usage, &both, Some(&units)), dec!(0.02001));
        assert_eq!(cost_with_media(&usage, &both, None), dec!(0.00001));
    }

    #[test]
    fn megapixel_prices_use_the_output_size() {
        let prices = Fields {
            unit_prices: Some(vec![unit("megapixel", "output", dec!(0.04))]),
            ..Fields::default()
        };
        let units = MediaUnits {
            output_images: 2,
            output_pixels: Some(1024 * 1024),
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&units, &prices), dec!(0.08388608));
        let unsized_units = MediaUnits {
            output_images: 2,
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&unsized_units, &prices), Decimal::ZERO);
        let mut audio = unit("image", "output", dec!(1));
        audio.audio = Some(true);
        let audio_only = Fields {
            unit_prices: Some(vec![audio]),
            ..Fields::default()
        };
        assert_eq!(media_cost(&units, &audio_only), Decimal::ZERO);
        assert_eq!(media_cost(&units, &Fields::default()), Decimal::ZERO);
    }

    #[test]
    fn audio_tokens_are_billed_at_audio_prices_and_fall_back_to_token_prices() {
        let prices = Fields {
            input_price: Some(dec!(0.6)),
            output_price: Some(dec!(2)),
            output_audio_price: Some(dec!(12)),
            input_audio_price: Some(dec!(1)),
            ..Fields::default()
        };
        let mut usage = Usage::new(1_000_000, 2_000_000);
        usage.output_audio_tokens = Some(1_000_000);
        usage.input_audio_tokens = Some(500_000);
        assert_eq!(cost(&usage, &prices), dec!(14.8));
        let plain = Fields {
            input_audio_price: None,
            output_audio_price: None,
            ..prices
        };
        assert_eq!(cost(&usage, &plain), dec!(4.6));
    }

    #[test]
    fn characters_and_audio_lengths_are_billed_per_unit_with_a_minimum() {
        let speech = Fields {
            unit_prices: Some(vec![unit("million_characters", "input", dec!(15))]),
            ..Fields::default()
        };
        let units = MediaUnits {
            characters: 12,
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&units, &speech), dec!(0.00018));
        assert_eq!(units.logged(), ("character", 12, 0));
        let mut hourly = unit("hour", "input", dec!(0.111));
        hourly.minimum_seconds = Some(10);
        let groq = Fields {
            unit_prices: Some(vec![hourly]),
            ..Fields::default()
        };
        let short = MediaUnits {
            input_ms: 3_000,
            ..MediaUnits::default()
        };
        assert_eq!(
            media_cost(&short, &groq),
            dec!(0.111) * dec!(10) / dec!(3600)
        );
        let long = MediaUnits {
            input_ms: 7_200_000,
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&long, &groq), dec!(0.222));
        assert_eq!(long.logged(), ("millisecond", 7_200_000, 0));
        let per_minute = Fields {
            unit_prices: Some(vec![unit("minute", "output", dec!(0.12))]),
            ..Fields::default()
        };
        let sound = MediaUnits {
            output_ms: 30_000,
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&sound, &per_minute), dec!(0.06));
        assert_eq!(media_cost(&MediaUnits::default(), &groq), Decimal::ZERO);
        let mut with_sound = unit("second", "output", dec!(0.05));
        with_sound.audio = Some(true);
        let mut silent = unit("second", "output", dec!(0.025));
        silent.audio = Some(false);
        let video = Fields {
            unit_prices: Some(vec![with_sound, silent]),
            ..Fields::default()
        };
        let clip = |audio| MediaUnits {
            output_ms: 4_000,
            audio,
            ..MediaUnits::default()
        };
        assert_eq!(media_cost(&clip(Some(false)), &video), dec!(0.1));
        assert_eq!(media_cost(&clip(Some(true)), &video), dec!(0.2));
    }

    #[test]
    fn speech_and_transcription_models_are_priced_by_unit_or_audio_tokens() {
        let none = Fields::default();
        let audio_out = Fields {
            output_audio_price: Some(dec!(12)),
            ..Fields::default()
        };
        assert!(unpriced_model(&none, &none, RequestKind::Speech, "m").is_some());
        assert_eq!(
            unpriced_model(&audio_out, &none, RequestKind::Speech, "m"),
            None
        );
        let audio_in = Fields {
            input_audio_price: Some(dec!(2.5)),
            ..Fields::default()
        };
        assert!(unpriced_model(&none, &none, RequestKind::Transcription, "m").is_some());
        assert_eq!(
            unpriced_model(&audio_in, &none, RequestKind::Transcription, "m"),
            None
        );
    }

    #[test]
    fn image_models_are_priced_by_unit_or_by_output_tokens() {
        let none = Fields::default();
        let units = Fields {
            unit_prices: Some(vec![unit("image", "output", dec!(0.02))]),
            ..Fields::default()
        };
        let tokens = Fields {
            output_image_price: Some(dec!(30)),
            ..Fields::default()
        };
        assert!(unpriced_model(&none, &none, RequestKind::Image, "m").is_some());
        assert_eq!(unpriced_model(&units, &none, RequestKind::Image, "m"), None);
        assert_eq!(
            unpriced_model(&tokens, &none, RequestKind::Image, "m"),
            None
        );
    }

    #[test]
    fn a_cost_is_unknown_when_the_model_has_no_price_for_what_was_measured() {
        let units = |per: &str, of: &str| Fields {
            unit_prices: Some(vec![unit(per, of, dec!(0.12))]),
            ..Fields::default()
        };
        let none = Usage::default();
        assert!(
            is_priced(&none, None, None),
            "nothing measured costs a known 0"
        );
        let clip = MediaUnits {
            output_videos: 1,
            output_ms: 8000,
            ..MediaUnits::default()
        };
        assert!(!is_priced(&none, Some(&Fields::default()), Some(&clip)));
        assert!(!is_priced(&none, None, Some(&clip)));
        let heard_characters = MediaUnits {
            characters: 12,
            ..MediaUnits::default()
        };
        assert!(
            !is_priced(
                &none,
                Some(&units("minute", "input")),
                Some(&heard_characters)
            ),
            "a voice changer priced per input minute, with only characters measured"
        );
        let speech = units("million_characters", "input");
        assert!(is_priced(&none, Some(&speech), Some(&heard_characters)));
        assert!(!is_priced(
            &Usage::new(10, 5),
            Some(&Fields::default()),
            None
        ));
    }
}
