pub mod capability;
pub mod resolve;
pub mod route;

use kaeso_kpp::DialectId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::kpmr::{self, price};
use crate::providers::{OrgProviders, ProviderRef};

pub const MODALITIES: &[&str] = &["text", "image", "pdf", "audio", "video"];

pub const EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "The fields of a catalogue entry. Every field is optional: an override\n\
    sets only what it changes, and the effective entry is the override merged\n\
    over KPMR's values."
    )
)]
pub struct Fields {
    #[doc = "A display name."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[doc = "Accepted inputs: `text`, `image`, `pdf`, `audio`, `video`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_modalities: Option<Vec<String>>,
    #[doc = "Produced outputs."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<String>>,
    #[doc = "The context window in tokens."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<u64>,
    #[doc = "The output limit in tokens."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_limit: Option<u64>,
    #[doc = "Whether it reasons."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[doc = "Whether it calls tools."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<bool>,
    #[doc = "Whether it supports structured output (JSON Schema)."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<bool>,
    #[doc = "Whether it accepts `temperature`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<bool>,
    #[doc = "USD per million input tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "3.00"))]
    pub input_price: Option<Decimal>,
    #[doc = "USD per million output tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "15.00"))]
    pub output_price: Option<Decimal>,
    #[doc = "USD per million cache-read tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_read_price: Option<Decimal>,
    #[doc = "USD per million cache-write tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_write_price: Option<Decimal>,
    #[doc = "USD per million cache-write tokens kept for an hour; absent means twice\n\
        the input price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_write_1h_price: Option<Decimal>,
    #[doc = "Prices for long prompts, by prompt length in tokens, lowest threshold first;\n\
        the highest tier a prompt exceeds replaces the base prices."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_tiers: Option<Vec<PriceTier>>,
    #[doc = "USD per million image input tokens; absent means the input price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "8.00"))]
    pub input_image_price: Option<Decimal>,
    #[doc = "USD per million image output tokens; when set, `output_price` is the price\n\
        of text output only."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "30.00"))]
    pub output_image_price: Option<Decimal>,
    #[doc = "USD per million cached image input tokens; absent means the cache-read price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_read_image_price: Option<Decimal>,
    #[doc = "USD per million audio input tokens; absent means the input price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "2.50"))]
    pub input_audio_price: Option<Decimal>,
    #[doc = "USD per million audio output tokens; absent means the output price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "12.00"))]
    pub output_audio_price: Option<Decimal>,
    #[doc = "Prices per image, megapixel, second, minute, hour or million characters, for\n\
        models a provider bills by unit; they add to the token prices."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_prices: Option<Vec<UnitPrice>>,
    #[doc = "The kpp dialect Kaeso sends: `anthropic_messages`, `openai_responses`,\n\
        `openai_chat` or `openai_embeddings`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "openai_responses"))]
    pub egress_dialect: Option<DialectId>,
    #[doc = "Anthropic models: whether adaptive thinking is accepted."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_thinking: Option<bool>,
    #[doc = "Anthropic models: whether a replayed thinking block is checked against the\n\
        conversation that produced it (preserved thinking)."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_binding: Option<bool>,
    #[doc = "Whether it accepts a forced tool choice (`tool_choice` `required`, `any`, `tool`\n\
        or a named function); absent means it does."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_tool_choice: Option<bool>,
    #[doc = "Supported effort levels."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    #[doc = "A per-model request timeout in seconds, instead of the instance's."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
    #[doc = "Release date, `YYYY-MM-DD`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[doc = "What it answers as a decision model; absent for a model that makes no decisions."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisions: Option<DecisionFields>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "What a decision model answers at `/v1/decisions`, `/v1/systemone` and\n\
    `/v1beta/decisions`: its question types and its limits."
    )
)]
pub struct DecisionFields {
    #[doc = "Whether it answers decision questions only and generates no text."]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub only: bool,
    #[doc = "The question types it answers: `predicate`, `choice`, `score`, `box`; absent\n\
        means the first three."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_types: Option<Vec<String>>,
    #[doc = "The most questions one request may ask."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_questions: Option<u32>,
    #[doc = "The most options one choice question may have."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_options: Option<u32>,
    #[doc = "The most levels one score question may have."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_levels: Option<u32>,
    #[doc = "The most images one request may show."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images: Option<u32>,
}

impl DecisionFields {
    fn from_kpmr(m: &kpmr::Model) -> Option<DecisionFields> {
        let only = m.is_decision_only();
        if !only && m.decisions.is_none() {
            return None;
        }
        let d = m.decisions.clone().unwrap_or_default();
        Some(DecisionFields {
            only,
            question_types: (!d.question_types.is_empty()).then_some(d.question_types),
            max_questions: d.max_questions,
            max_options: d.max_options,
            max_levels: d.max_levels,
            max_images: d.max_images,
        })
    }

    /// Whether it answers questions of the KPP type `kind`.
    pub fn answers(&self, kind: &str) -> bool {
        match &self.question_types {
            Some(types) => types.iter().any(|t| t == kind),
            None => kind != "box",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "The prices for every token of a request whose prompt is longer than\n\
    `context_above` tokens, output included. An absent price is the model's own\n\
    price."
    )
)]
pub struct PriceTier {
    #[doc = "The prompt length in tokens, cached tokens included, above which these\n\
        prices apply."]
    #[cfg_attr(feature = "openapi", schema(example = 272000))]
    pub context_above: u64,
    #[doc = "USD per million input tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "20.00"))]
    pub input_price: Option<Decimal>,
    #[doc = "USD per million output tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "75.00"))]
    pub output_price: Option<Decimal>,
    #[doc = "USD per million cache-read tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_read_price: Option<Decimal>,
    #[doc = "USD per million cache-write tokens."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_write_price: Option<Decimal>,
    #[doc = "USD per million cache-write tokens kept for an hour; absent means twice\n\
        this tier's input price, or the model's own one-hour price when the tier has\n\
        no input price."]
    #[serde(
        default,
        deserialize_with = "price",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub cache_write_1h_price: Option<Decimal>,
}

/// The units a price can be per, as KPMR names them.
pub const UNITS: &[&str] = &[
    "image",
    "megapixel",
    "second",
    "minute",
    "hour",
    "million_characters",
    "video",
];

const MAX_UNIT_PRICES: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "A price per unit of a request's input or output, for requests that match\n\
    every condition; an absent condition matches any request."
    )
)]
pub struct UnitPrice {
    #[doc = "The unit: `image`, `megapixel`, `second`, `minute`, `hour`,\n\
        `million_characters` or `video`."]
    #[cfg_attr(feature = "openapi", schema(example = "image"))]
    pub per: String,
    #[doc = "Whether the unit is of the input or the output: `input` or `output`."]
    #[cfg_attr(feature = "openapi", schema(example = "output"))]
    pub of: String,
    #[doc = "USD per unit."]
    #[serde(deserialize_with = "required_price")]
    #[cfg_attr(feature = "openapi", schema(value_type = String, example = "0.04"))]
    pub usd: Decimal,
    #[doc = "The image size it applies to, such as `1024x1024`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[doc = "The quality it applies to, such as `high`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[doc = "The resolution it applies to, such as `1K`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[doc = "Whether it applies to made video with sound (`true`) or without (`false`)."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    #[doc = "The fewest seconds a request is billed for, for prices per second, minute or\n\
        hour."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_seconds: Option<u64>,
    #[doc = "The length in whole seconds of the video it applies to, for prices per video."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[doc = "Whether the provider publishes the price only as an estimate."]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub estimate: bool,
}

fn required_price<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    price(d)?.ok_or_else(|| serde::de::Error::custom("a unit price needs `usd`"))
}

impl UnitPrice {
    fn from_kpmr(u: &kpmr::UnitPrice) -> UnitPrice {
        UnitPrice {
            per: u.per.clone(),
            of: u.of.clone(),
            usd: u.usd,
            size: u.size.clone(),
            quality: u.quality.clone(),
            resolution: u.resolution.clone(),
            audio: u.audio,
            minimum_seconds: u.minimum_seconds,
            duration: u.duration,
            estimate: u.estimate,
        }
    }
}

impl PriceTier {
    fn from_kpmr(t: &kpmr::Tier) -> PriceTier {
        PriceTier {
            context_above: t.context_above,
            input_price: t.input,
            output_price: t.output,
            cache_read_price: t.cache_read,
            cache_write_price: t.cache_write,
            cache_write_1h_price: None,
        }
    }

    fn prices(&self) -> [Option<Decimal>; 5] {
        [
            self.input_price,
            self.output_price,
            self.cache_read_price,
            self.cache_write_price,
            self.cache_write_1h_price,
        ]
    }
}

const MAX_PRICE_TIERS: usize = 16;

fn is_valid_field(name: &str, value: &serde_json::Value) -> bool {
    let single = serde_json::Map::from_iter([(name.to_owned(), value.clone())]);
    serde_json::from_value::<Fields>(serde_json::Value::Object(single)).is_ok()
}

macro_rules! each_field {
    ($m:ident) => {
        $m!(
            display_name,
            input_modalities,
            output_modalities,
            context_limit,
            output_limit,
            reasoning,
            tool_call,
            structured_output,
            temperature,
            input_price,
            output_price,
            cache_read_price,
            cache_write_price,
            cache_write_1h_price,
            price_tiers,
            input_image_price,
            output_image_price,
            cache_read_image_price,
            input_audio_price,
            output_audio_price,
            unit_prices,
            egress_dialect,
            adaptive_thinking,
            thinking_binding,
            forced_tool_choice,
            efforts,
            timeout_seconds,
            release_date,
            decisions
        )
    };
}

pub const FIELD_NAMES: &[&str] = &[
    "display_name",
    "input_modalities",
    "output_modalities",
    "context_limit",
    "output_limit",
    "reasoning",
    "tool_call",
    "structured_output",
    "temperature",
    "input_price",
    "output_price",
    "cache_read_price",
    "cache_write_price",
    "cache_write_1h_price",
    "price_tiers",
    "input_image_price",
    "output_image_price",
    "cache_read_image_price",
    "input_audio_price",
    "output_audio_price",
    "unit_prices",
    "egress_dialect",
    "adaptive_thinking",
    "thinking_binding",
    "forced_tool_choice",
    "efforts",
    "timeout_seconds",
    "release_date",
    "decisions",
];

impl Fields {
    #[must_use]
    pub fn over(self, lower: &Fields) -> Fields {
        macro_rules! merge {
            ($($f:ident),*) => {
                Fields { $($f: self.$f.or_else(|| lower.$f.clone()),)* }
            };
        }
        each_field!(merge)
    }

    #[must_use]
    pub fn over_kpmr(self, mut kpmr: Fields) -> Fields {
        // A negotiated input price keeps its one-hour write at twice that price, not KPMR's list price.
        if self.input_price.is_some() {
            kpmr.cache_write_1h_price = None;
        }
        // KPMR's long-context prices would replace negotiated prices once a prompt is long.
        let sets_a_price = [
            self.input_price,
            self.output_price,
            self.cache_read_price,
            self.cache_write_price,
        ]
        .iter()
        .any(Option::is_some);
        if sets_a_price && self.price_tiers.is_none() {
            kpmr.price_tiers = None;
        }
        // The same holds for KPMR's image-token and unit prices: a negotiated price is the
        // price of the whole request unless the media prices are negotiated too.
        if sets_a_price {
            if self.input_image_price.is_none()
                && self.output_image_price.is_none()
                && self.cache_read_image_price.is_none()
            {
                kpmr.input_image_price = None;
                kpmr.output_image_price = None;
                kpmr.cache_read_image_price = None;
            }
            if self.input_audio_price.is_none() && self.output_audio_price.is_none() {
                kpmr.input_audio_price = None;
                kpmr.output_audio_price = None;
            }
            if self.unit_prices.is_none() {
                kpmr.unit_prices = None;
            }
        }
        // A one-hour price set by hand holds for long prompts too, where KPMR's tiers name none.
        let one_hour_by_hand = self.cache_write_1h_price;
        let mut merged = self.over(&kpmr);
        if let (Some(price), Some(tiers)) = (one_hour_by_hand, merged.price_tiers.as_mut()) {
            for tier in tiers {
                tier.cache_write_1h_price.get_or_insert(price);
            }
        }
        merged
    }

    pub fn is_empty(&self) -> bool {
        *self == Fields::default()
    }

    pub fn set_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        macro_rules! set {
            ($($f:ident),*) => {
                $(if self.$f.is_some() { names.push(stringify!($f)); })*
            };
        }
        each_field!(set);
        names
    }

    pub fn from_kpmr(m: &kpmr::Model) -> Fields {
        let pricing = m.pricing.clone().unwrap_or_default();
        let limits = m.limits.clone().unwrap_or_default();
        let capabilities = m.capabilities.clone().unwrap_or_default();
        let reasoning = m.reasoning.clone().unwrap_or_default();
        Fields {
            display_name: m.name.clone(),
            input_modalities: m.modalities.as_ref().map(|x| x.input.clone()),
            output_modalities: m.modalities.as_ref().map(|x| x.output.clone()),
            context_limit: limits.context,
            output_limit: limits.output,
            reasoning: capabilities.reasoning,
            tool_call: capabilities.tool_call,
            structured_output: capabilities.structured_output,
            temperature: capabilities.temperature,
            input_price: pricing.input,
            output_price: pricing.output,
            cache_read_price: pricing.cache_read,
            cache_write_price: pricing.cache_write,
            cache_write_1h_price: pricing.cache_write_1h,
            price_tiers: (!pricing.tiers.is_empty())
                .then(|| pricing.tiers.iter().map(PriceTier::from_kpmr).collect()),
            input_image_price: pricing.input_image,
            output_image_price: pricing.output_image,
            cache_read_image_price: pricing.cache_read_image,
            input_audio_price: pricing.input_audio,
            output_audio_price: pricing.output_audio,
            unit_prices: (!pricing.per_unit.is_empty())
                .then(|| pricing.per_unit.iter().map(UnitPrice::from_kpmr).collect()),
            egress_dialect:
                m.egress_dialect()
                    .or((m.kind.as_deref() == Some("embedding"))
                        .then_some(DialectId::OpenAiEmbeddings)),
            adaptive_thinking: reasoning.adaptive_thinking,
            thinking_binding: reasoning.thinking_binding,
            forced_tool_choice: capabilities.forced_tool_choice,
            efforts: reasoning.efforts.map(|e| {
                e.into_iter()
                    .filter(|v| EFFORTS.contains(&v.as_str()))
                    .collect()
            }),
            timeout_seconds: None,
            release_date: m.release_date.clone(),
            decisions: DecisionFields::from_kpmr(m),
        }
    }

    pub fn patch(
        &mut self,
        patch: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), FieldError> {
        let mut doc = serde_json::to_value(&*self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        for (k, v) in patch {
            if !FIELD_NAMES.contains(&k.as_str()) {
                return Err(FieldError::new(
                    k,
                    format!("`{k}` is not a catalogue field."),
                ));
            }
            if v.is_null() {
                doc.remove(k);
            } else {
                doc.insert(k.clone(), v.clone());
            }
        }
        *self = serde_json::from_value(serde_json::Value::Object(doc)).map_err(|_| {
            let wrong = patch
                .iter()
                .find(|(k, v)| !v.is_null() && !is_valid_field(k, v));
            match wrong {
                Some((k, _)) => {
                    FieldError::new(k, format!("`{k}` has a value of the wrong type or form."))
                }
                None => FieldError::new("overrides", "An override has a value of the wrong type."),
            }
        })?;
        Ok(())
    }

    pub fn validate(&self, provider: &ProviderRef) -> Result<(), FieldError> {
        if let Some(n) = &self.display_name
            && (n.trim().is_empty() || n.chars().count() > 200)
        {
            return Err(FieldError::new(
                "display_name",
                "The display name must be 1 to 200 characters.",
            ));
        }
        for (name, list) in [
            ("input_modalities", &self.input_modalities),
            ("output_modalities", &self.output_modalities),
        ] {
            if let Some(list) = list {
                if list.is_empty() {
                    return Err(FieldError::new(
                        name,
                        format!("`{name}` must list at least one modality."),
                    ));
                }
                check_values(name, list, MODALITIES)?;
            }
        }
        if let Some(efforts) = &self.efforts {
            check_values("efforts", efforts, EFFORTS)?;
        }
        for (name, v) in [
            ("context_limit", self.context_limit),
            ("output_limit", self.output_limit),
        ] {
            if v == Some(0) {
                return Err(FieldError::new(name, format!("`{name}` must be positive.")));
            }
        }
        let max_price = Decimal::from(1_000_000u32);
        let tiers = self.price_tiers.as_deref().unwrap_or_default();
        let tier_prices = tiers
            .iter()
            .flat_map(PriceTier::prices)
            .map(|p| ("price_tiers", p));
        for (name, v) in [
            ("input_price", self.input_price),
            ("output_price", self.output_price),
            ("cache_read_price", self.cache_read_price),
            ("cache_write_price", self.cache_write_price),
            ("cache_write_1h_price", self.cache_write_1h_price),
            ("input_image_price", self.input_image_price),
            ("output_image_price", self.output_image_price),
            ("cache_read_image_price", self.cache_read_image_price),
            ("input_audio_price", self.input_audio_price),
            ("output_audio_price", self.output_audio_price),
        ]
        .into_iter()
        .chain(tier_prices)
        {
            if v.is_some_and(|p| p.is_sign_negative() || p > max_price || p.scale() > 9) {
                return Err(FieldError::new(
                    name,
                    format!(
                        "`{name}` must be 0 to 1,000,000 USD per million tokens, at most 9 decimals."
                    ),
                ));
            }
        }
        let rising = tiers
            .iter()
            .zip(tiers.iter().skip(1))
            .all(|(a, b)| a.context_above < b.context_above);
        if tiers.len() > MAX_PRICE_TIERS
            || !rising
            || tiers.first().is_some_and(|t| t.context_above == 0)
        {
            return Err(FieldError::new(
                "price_tiers",
                "`price_tiers` takes up to 16 tiers, their `context_above` positive and rising.",
            ));
        }
        let units = self.unit_prices.as_deref().unwrap_or_default();
        let unit_ok = |u: &UnitPrice| {
            UNITS.contains(&u.per.as_str())
                && matches!(u.of.as_str(), "input" | "output")
                && !u.usd.is_sign_negative()
                && u.usd <= max_price
                && u.usd.scale() <= 9
        };
        if units.len() > MAX_UNIT_PRICES || !units.iter().all(unit_ok) {
            return Err(FieldError::new(
                "unit_prices",
                "`unit_prices` takes up to 32 prices of 0 to 1,000,000 USD per known unit.",
            ));
        }
        if let Some(d) = self.egress_dialect
            && !provider.dialects().contains(&d)
        {
            return Err(FieldError::new(
                "egress_dialect",
                format!(
                    "`{d}` is not a dialect of {}; use one of {}.",
                    provider.name(),
                    provider
                        .dialects()
                        .iter()
                        .map(|d| d.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if self.adaptive_thinking.is_some()
            && !provider.dialects().contains(&DialectId::AnthropicMessages)
        {
            return Err(FieldError::new(
                "adaptive_thinking",
                "Adaptive thinking applies only to Anthropic models.",
            ));
        }
        if self.thinking_binding.is_some()
            && !provider.dialects().contains(&DialectId::AnthropicMessages)
        {
            return Err(FieldError::new(
                "thinking_binding",
                "Thinking binding applies only to Anthropic models.",
            ));
        }
        if self
            .timeout_seconds
            .is_some_and(|t| !(1..=3600).contains(&t))
        {
            return Err(FieldError::new(
                "timeout_seconds",
                "The timeout must be 1 to 3600 seconds.",
            ));
        }
        if let Some(d) = &self.decisions {
            validate_decisions(d, provider)?;
        }
        if let Some(d) = &self.release_date {
            let format = time::macros::format_description!("[year]-[month]-[day]");
            if time::Date::parse(d, &format).is_err() {
                return Err(FieldError::new(
                    "release_date",
                    "The release date must be a date, YYYY-MM-DD.",
                ));
            }
        }
        Ok(())
    }

    pub fn validate_custom(&self) -> Result<(), FieldError> {
        // A decision-only model generates nothing: it has no output limit or generation dialect,
        // and its price is per input token.
        if self.decisions.as_ref().is_some_and(|d| d.only) {
            let missing: Vec<&str> = [
                ("input_modalities", self.input_modalities.is_none()),
                ("input_price", self.input_price.is_none()),
            ]
            .into_iter()
            .filter_map(|(n, m)| m.then_some(n))
            .collect();
            return match missing.first() {
                None => Ok(()),
                Some(first) => Err(FieldError::new(
                    first,
                    format!("A custom model needs {}.", missing.join(", ")),
                )),
            };
        }
        // A video model has no context or output limit and no generation dialect either, and
        // providers such as Gemini Enterprise Agent Platform publish no price for theirs: it
        // needs its modalities, and its jobs are logged with an unknown cost until it has one.
        let videos_only = self
            .output_modalities
            .as_ref()
            .is_some_and(|o| o.iter().any(|m| m == "video") && !o.iter().any(|m| m == "text"));
        if videos_only {
            if self.input_modalities.is_none() {
                return Err(FieldError::new(
                    "input_modalities",
                    "A custom model needs input_modalities.",
                ));
            }
            return Ok(());
        }
        // An image, speech or transcription model has no context or output limit and no
        // generation dialect: it needs its modalities and a price, per unit or per token.
        let has = |list: &Option<Vec<String>>, m: &str| {
            list.as_ref().is_some_and(|l| l.iter().any(|x| x == m))
        };
        let makes_media = (has(&self.output_modalities, "image")
            || has(&self.output_modalities, "audio"))
            && !has(&self.output_modalities, "text");
        let transcribes = has(&self.input_modalities, "audio")
            && !has(&self.input_modalities, "text")
            && has(&self.output_modalities, "text");
        if makes_media || transcribes {
            let unpriced = self.unit_prices.is_none()
                && self.input_price.is_none()
                && self.output_price.is_none()
                && self.output_image_price.is_none();
            if self.input_modalities.is_none() || unpriced {
                let missing: Vec<&str> = [
                    ("input_modalities", self.input_modalities.is_none()),
                    ("unit_prices", unpriced),
                ]
                .into_iter()
                .filter_map(|(n, m)| m.then_some(n))
                .collect();
                return Err(FieldError::new(
                    missing.first().copied().unwrap_or("unit_prices"),
                    format!("A custom model needs {}.", missing.join(", ")),
                ));
            }
            return Ok(());
        }
        let missing: Vec<&str> = [
            ("input_modalities", self.input_modalities.is_none()),
            ("output_modalities", self.output_modalities.is_none()),
            ("context_limit", self.context_limit.is_none()),
            ("output_limit", self.output_limit.is_none()),
            ("input_price", self.input_price.is_none()),
            ("output_price", self.output_price.is_none()),
            ("egress_dialect", self.egress_dialect.is_none()),
        ]
        .into_iter()
        .filter_map(|(n, m)| m.then_some(n))
        .collect();
        match missing.first() {
            None => Ok(()),
            Some(first) => Err(FieldError::new(
                first,
                format!("A custom model needs {}.", missing.join(", ")),
            )),
        }
    }
}

fn validate_decisions(d: &DecisionFields, provider: &ProviderRef) -> Result<(), FieldError> {
    if !provider.has_decisions() {
        return Err(FieldError::new(
            "decisions",
            match provider.custom() {
                Some(c) => format!(
                    "The custom provider `{}` has no decisions API turned on; turn it on first.",
                    c.id()
                ),
                None => format!(
                    "{} offers no decisions API Kaeso can call; remove `decisions`.",
                    provider.name()
                ),
            },
        ));
    }
    if let Some(types) = &d.question_types {
        if types.is_empty() {
            return Err(FieldError::new(
                "decisions",
                "`decisions.question_types` must list at least one type.",
            ));
        }
        check_values("decisions", types, &kpmr::QUESTION_TYPES)?;
    }
    let limits = [d.max_questions, d.max_options, d.max_levels, d.max_images];
    if limits.contains(&Some(0)) {
        return Err(FieldError::new(
            "decisions",
            "The limits in `decisions` must be positive.",
        ));
    }
    Ok(())
}

fn check_values(name: &str, list: &[String], allowed: &[&str]) -> Result<(), FieldError> {
    for (i, v) in list.iter().enumerate() {
        if !allowed.contains(&v.as_str()) {
            return Err(FieldError::new(
                name,
                format!("`{v}` is not one of {}.", allowed.join(", ")),
            ));
        }
        if list.iter().take(i).any(|w| w == v) {
            return Err(FieldError::new(name, format!("`{v}` is listed twice.")));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{field}: {message}")]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    fn new(field: &str, message: impl Into<String>) -> FieldError {
        FieldError {
            field: field.to_owned(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable,
}

impl Availability {
    pub fn as_str(self) -> &'static str {
        match self {
            Availability::Available => "available",
            Availability::Unavailable => "unavailable",
        }
    }

    pub fn parse(text: &str) -> Option<Availability> {
        match text {
            "available" => Some(Availability::Available),
            "unavailable" => Some(Availability::Unavailable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: uuid::Uuid,
    pub provider: String,
    pub model_id: String,
    pub overrides: Fields,
    pub custom: bool,
    pub delisted: bool,
    pub last_known: Option<kpmr::Model>,
    #[serde(default)]
    pub availability: Option<Availability>,
    #[serde(default)]
    pub availability_checked_at: Option<time::OffsetDateTime>,
}

impl Entry {
    pub fn is_unavailable(&self) -> bool {
        self.availability == Some(Availability::Unavailable)
    }

    pub fn is_deprecated(&self, snapshot: &kpmr::Catalogue) -> bool {
        snapshot
            .model(&self.provider, &self.model_id)
            .or(self.last_known.as_ref())
            .is_some_and(kpmr::Model::is_deprecated)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effective {
    pub id: uuid::Uuid,
    pub provider: ProviderRef,
    pub model_id: String,
    pub fields: Fields,
    pub dialect: DialectId,
}

impl Effective {
    pub fn is_embeddings(&self) -> bool {
        EMBEDDINGS.contains(&self.dialect)
    }

    /// Whether the model answers decision questions, alone or as well as generating.
    pub fn decides(&self) -> bool {
        self.fields.decisions.is_some()
    }

    /// Whether the model answers decision questions only and generates nothing.
    pub fn decides_only(&self) -> bool {
        self.fields.decisions.as_ref().is_some_and(|d| d.only)
    }

    pub fn accepts(&self, modality: &str) -> bool {
        self.fields
            .input_modalities
            .as_ref()
            .is_some_and(|m| m.iter().any(|x| x == modality))
    }

    pub fn display_name(&self) -> &str {
        self.fields
            .display_name
            .as_deref()
            .unwrap_or(&self.model_id)
    }
}

const EMBEDDINGS: [DialectId; 2] = [DialectId::OpenAiEmbeddings, DialectId::GeminiEmbeddings];

pub fn effective(
    entry: &Entry,
    snapshot: &kpmr::Catalogue,
    providers: &OrgProviders,
) -> Option<Effective> {
    let provider = providers.get(&entry.provider)?;
    let fields = if provider.custom().is_some() {
        entry.overrides.clone().over_kpmr(Fields::default())
    } else {
        let mut upstream = snapshot
            .model(&entry.provider, &entry.model_id)
            .or(entry.last_known.as_ref())
            .map(Fields::from_kpmr)
            .unwrap_or_default();
        // KPMR names OpenAI's embeddings format for every embeddings model; a provider with
        // embeddings in another format gets them in its own. One that Kaeso calls without
        // embeddings keeps KPMR's, so the model stays an embeddings model that is refused
        // instead of being sent as a generation model.
        upstream.egress_dialect = upstream.egress_dialect.and_then(|d| {
            if provider.dialects().contains(&d) {
                Some(d)
            } else if EMBEDDINGS.contains(&d) {
                provider.embeddings().or(Some(d))
            } else {
                None
            }
        });
        entry.overrides.clone().over_kpmr(upstream)
    };
    let dialect = fields
        .egress_dialect
        .unwrap_or_else(|| provider.dialect_for(&entry.model_id));
    Some(Effective {
        id: entry.id,
        provider,
        model_id: entry.model_id.clone(),
        fields,
        dialect,
    })
}

pub fn preview(provider: &str, model: &str, snapshot: &kpmr::Catalogue) -> Option<Fields> {
    snapshot.model(provider, model).map(Fields::from_kpmr)
}

#[cfg(test)]
mod tests;
