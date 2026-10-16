pub mod capability;
pub mod resolve;

use std::collections::BTreeMap;

use kaeso_kpp::DialectId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models_dev::{self, price};
use crate::provider::{self, Provider};

pub const MODALITIES: &[&str] = &["text", "image", "pdf", "audio", "video"];

pub const EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

pub const SUPPLEMENT: &str = include_str!("../../data/supplement.toml");

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "The fields of a catalogue entry. Every field is optional: an override\n\
    sets only what it changes, and the effective entry is the merge of the\n\
    override, models.dev and the supplement, in that order."
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
    #[doc = "USD per million input tokens, as a decimal string."]
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
    #[doc = "The kpp dialect Kaeso sends: `anthropic_messages`, `openai_responses`,\n\
        `openai_chat` or `openai_embeddings`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, example = "openai_responses"))]
    pub egress_dialect: Option<DialectId>,
    #[doc = "Anthropic models: whether adaptive thinking is accepted."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_thinking: Option<bool>,
    #[doc = "Supported effort levels."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    #[doc = "A per-model request timeout in seconds, instead of the instance's."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
    #[doc = "Release date, `YYYY-MM-DD`."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
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
            egress_dialect,
            adaptive_thinking,
            efforts,
            timeout_seconds,
            release_date
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
    "egress_dialect",
    "adaptive_thinking",
    "efforts",
    "timeout_seconds",
    "release_date",
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

    pub fn from_models_dev(m: &models_dev::Model) -> Fields {
        let cost = m.cost.clone().unwrap_or_default();
        let limit = m.limit.clone().unwrap_or_default();
        Fields {
            display_name: m.name.clone(),
            input_modalities: m.modalities.as_ref().map(|x| x.input.clone()),
            output_modalities: m.modalities.as_ref().map(|x| x.output.clone()),
            context_limit: limit.context,
            output_limit: limit.output,
            reasoning: m.reasoning,
            tool_call: m.tool_call,
            structured_output: m.structured_output,
            temperature: m.temperature,
            input_price: cost.input,
            output_price: cost.output,
            cache_read_price: cost.cache_read,
            cache_write_price: cost.cache_write,
            egress_dialect: None,
            adaptive_thinking: None,
            efforts: m.efforts(),
            timeout_seconds: None,
            release_date: m.release_date.clone(),
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
            FieldError::new("overrides", "An override has a value of the wrong type.")
        })?;
        Ok(())
    }

    pub fn validate(&self, provider: &Provider) -> Result<(), FieldError> {
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
        for (name, v) in [
            ("input_price", self.input_price),
            ("output_price", self.output_price),
            ("cache_read_price", self.cache_read_price),
            ("cache_write_price", self.cache_write_price),
        ] {
            if v.is_some_and(|p| p.is_sign_negative() || p > max_price || p.scale() > 9) {
                return Err(FieldError::new(
                    name,
                    format!(
                        "`{name}` must be 0 to 1,000,000 USD per million tokens, at most 9 decimals."
                    ),
                ));
            }
        }
        if let Some(d) = self.egress_dialect
            && !provider.dialects.contains(&d)
        {
            return Err(FieldError::new(
                "egress_dialect",
                format!(
                    "`{d}` is not a dialect of {}; use one of {}.",
                    provider.name,
                    provider
                        .dialects
                        .iter()
                        .map(|d| d.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if self.adaptive_thinking.is_some()
            && !provider.dialects.contains(&DialectId::AnthropicMessages)
        {
            return Err(FieldError::new(
                "adaptive_thinking",
                "Adaptive thinking applies only to Anthropic models.",
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Supplement {
    models: BTreeMap<(String, String), Fields>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementEntry {
    adaptive_thinking: Option<bool>,
    efforts: Option<Vec<String>>,
    egress_dialect: Option<DialectId>,
}

impl Supplement {
    pub fn parse(text: &str) -> Result<Supplement, String> {
        let doc: BTreeMap<String, BTreeMap<String, SupplementEntry>> =
            toml::from_str(text).map_err(|e| e.to_string())?;
        let mut models = BTreeMap::new();
        for (provider_id, entries) in doc {
            let provider = provider::provider(&provider_id)
                .ok_or_else(|| format!("unknown provider `{provider_id}`"))?;
            for (model, e) in entries {
                let fields = Fields {
                    adaptive_thinking: e.adaptive_thinking,
                    efforts: e.efforts,
                    egress_dialect: e.egress_dialect,
                    ..Fields::default()
                };
                fields
                    .validate(provider)
                    .map_err(|err| format!("{provider_id}.{model}: {err}"))?;
                models.insert((provider_id.clone(), model), fields);
            }
        }
        Ok(Supplement { models })
    }

    pub fn embedded() -> Result<Supplement, String> {
        Supplement::parse(SUPPLEMENT)
    }

    pub fn get(&self, provider: &str, model: &str) -> Option<&Fields> {
        self.models.get(&(provider.to_owned(), model.to_owned()))
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
    pub last_known: Option<models_dev::Model>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effective {
    pub id: uuid::Uuid,
    pub provider: &'static Provider,
    pub model_id: String,
    pub fields: Fields,
    pub dialect: DialectId,
}

impl Effective {
    pub fn is_embeddings(&self) -> bool {
        self.dialect == DialectId::OpenAiEmbeddings
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

pub fn effective(
    entry: &Entry,
    snapshot: &models_dev::Snapshot,
    supplement: &Supplement,
) -> Option<Effective> {
    let provider = provider::provider(&entry.provider)?;
    let upstream = snapshot
        .model(&entry.provider, &entry.model_id)
        .or(entry.last_known.as_ref())
        .map(Fields::from_models_dev)
        .unwrap_or_default();
    let supplemental = supplement
        .get(&entry.provider, &entry.model_id)
        .cloned()
        .unwrap_or_default();
    let fields = entry.overrides.clone().over(&upstream).over(&supplemental);
    let dialect = fields.egress_dialect.unwrap_or(provider.generation);
    Some(Effective {
        id: entry.id,
        provider,
        model_id: entry.model_id.clone(),
        fields,
        dialect,
    })
}

pub fn preview(
    provider: &str,
    model: &str,
    snapshot: &models_dev::Snapshot,
    supplement: &Supplement,
) -> Option<Fields> {
    let m = snapshot.model(provider, model)?;
    let supplemental = supplement.get(provider, model).cloned().unwrap_or_default();
    Some(Fields::from_models_dev(m).over(&supplemental))
}

#[cfg(test)]
mod tests;
