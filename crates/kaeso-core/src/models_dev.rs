use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use time::OffsetDateTime;
use time::macros::datetime;

use crate::decimal;
use crate::provider::{ProviderId, known_ids};

pub const EMBEDDED: &str = include_str!("../data/models-dev.json");

pub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(2026-10-30 03:29:11 UTC);

pub const DEFAULT_URL: &str = "https://models.dev/api.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modalities {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    #[serde(default)]
    pub context: Option<u64>,
    #[serde(default)]
    pub input: Option<u64>,
    #[serde(default)]
    pub output: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    #[serde(default, deserialize_with = "listed_price")]
    pub input: Option<Decimal>,
    #[serde(default, deserialize_with = "listed_price")]
    pub output: Option<Decimal>,
    #[serde(default, deserialize_with = "listed_price")]
    pub cache_read: Option<Decimal>,
    #[serde(default, deserialize_with = "listed_price")]
    pub cache_write: Option<Decimal>,
}

pub fn price<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    decimal::deserialize_with(d, parse_decimal)
}

fn listed_price<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    let Some(raw) = Option::<Box<RawValue>>::deserialize(d)? else {
        return Ok(None);
    };
    let text = raw.get().trim();
    if text == "null" {
        return Ok(None);
    }
    let number = if text.starts_with('"') {
        serde_json::from_str::<String>(text).map_err(serde::de::Error::custom)?
    } else if text.starts_with(|c: char| c == '-' || c.is_ascii_digit()) {
        text.to_owned()
    } else {
        return Err(serde::de::Error::custom(format!(
            "expected a price, got {text}"
        )));
    };
    parse_decimal(&number)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

const MAX_PRICE: u32 = 1_000_000;

fn parse_decimal(s: &str) -> Result<Decimal, String> {
    let d = decimal::parse_non_negative(s)?;
    if d > Decimal::from(MAX_PRICE) {
        Err(format!("`{s}` is above {MAX_PRICE} USD per million tokens"))
    } else {
        Ok(d)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub attachment: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub reasoning_options: Option<Vec<Value>>,
    #[serde(default)]
    pub tool_call: Option<bool>,
    #[serde(default)]
    pub structured_output: Option<bool>,
    #[serde(default)]
    pub temperature: Option<bool>,
    #[serde(default)]
    pub knowledge: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub modalities: Option<Modalities>,
    #[serde(default)]
    pub limit: Option<Limits>,
    #[serde(default)]
    pub cost: Option<Cost>,
    #[serde(default)]
    pub status: Option<String>,
    /// What the model makes, such as `embedding`; models.dev leaves it out for most
    /// generation models.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

impl Model {
    pub fn efforts(&self) -> Option<Vec<String>> {
        self.reasoning_options.as_ref()?.iter().find_map(|o| {
            if o.get("type")?.as_str()? != "effort" {
                return None;
            }
            Some(
                o.get("values")?
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
            )
        })
    }

    pub fn is_deprecated(&self) -> bool {
        self.status.as_deref() == Some("deprecated")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub models: BTreeMap<String, Model>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub providers: BTreeMap<String, Provider>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub snapshot: Snapshot,
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not models.dev data: {0}")]
    Shape(String),
    #[error("models.dev data has none of the providers {0}")]
    NoProviders(String),
}

#[derive(Deserialize)]
struct ProviderDoc<'a> {
    #[serde(default)]
    name: Option<Value>,
    #[serde(default, borrow)]
    models: Option<&'a RawValue>,
}

pub fn parse(json: &[u8], providers: &[ProviderId]) -> Result<Parsed, ParseError> {
    let doc: BTreeMap<String, &RawValue> =
        serde_json::from_slice(json).map_err(|e| ParseError::Shape(e.to_string()))?;
    let mut snapshot = Snapshot::default();
    let mut skipped = Vec::new();
    for id in providers {
        let Some(raw) = doc.get(id.as_str()) else {
            continue;
        };
        let no_models = || ParseError::Shape(format!("provider `{id}` has no models"));
        let provider: ProviderDoc<'_> = serde_json::from_str(raw.get()).map_err(|_| no_models())?;
        let name = provider
            .name
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_owned);
        let models: BTreeMap<String, &RawValue> = provider
            .models
            .and_then(|m| serde_json::from_str(m.get()).ok())
            .ok_or_else(no_models)?;
        let mut parsed = BTreeMap::new();
        for (model_id, m) in &models {
            match serde_json::from_str::<Model>(m.get()) {
                Ok(model) if model.id == *model_id => {
                    parsed.insert(model_id.clone(), model);
                }
                _ => skipped.push(format!("{id}/{model_id}")),
            }
        }
        snapshot.providers.insert(
            id.to_string(),
            Provider {
                id: id.to_string(),
                name,
                models: parsed,
            },
        );
    }
    if snapshot.providers.is_empty() {
        let ids: Vec<&str> = providers.iter().map(ProviderId::as_str).collect();
        return Err(ParseError::NoProviders(ids.join(", ")));
    }
    Ok(Parsed { snapshot, skipped })
}

pub fn embedded() -> Result<Snapshot, ParseError> {
    Ok(parse(EMBEDDED.as_bytes(), &known_ids())?.snapshot)
}

impl Snapshot {
    pub fn model(&self, provider: &str, model: &str) -> Option<&Model> {
        self.providers.get(provider)?.models.get(model)
    }

    pub fn model_count(&self) -> usize {
        self.providers.values().map(|p| p.models.len()).sum()
    }

    pub fn same_model(&self, id: &str) -> Option<&Model> {
        let wanted = id.to_lowercase();
        let tail = |id: &str| id.rsplit('/').next().unwrap_or(id).to_owned();
        let models = || self.providers.values().flat_map(|p| p.models.iter());
        models()
            .find(|(m, _)| m.to_lowercase() == wanted)
            .or_else(|| models().find(|(m, _)| tail(&m.to_lowercase()) == tail(&wanted)))
            .map(|(_, model)| model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn the_shipped_snapshot_parses_completely() {
        let parsed = parse(EMBEDDED.as_bytes(), &known_ids()).unwrap();
        assert!(parsed.skipped.is_empty(), "{:?}", parsed.skipped);
        assert_eq!(
            parsed.snapshot.providers.len(),
            crate::provider::PROVIDERS.len()
        );
        let sonnet = parsed
            .snapshot
            .model("anthropic", "claude-sonnet-4-5")
            .unwrap();
        let cost = sonnet.cost.as_ref().unwrap();
        assert_eq!(cost.input, Some(dec!(3)));
        assert_eq!(cost.cache_read, Some(dec!(0.3)));
        assert_eq!(sonnet.limit.as_ref().unwrap().output, Some(64_000));
        assert!(
            sonnet
                .modalities
                .as_ref()
                .unwrap()
                .input
                .contains(&"pdf".to_owned())
        );
        let gpt = parsed.snapshot.model("openai", "gpt-5").unwrap();
        assert_eq!(gpt.efforts().unwrap(), ["minimal", "low", "medium", "high"]);
        let together = &parsed.snapshot.providers["togetherai"].models;
        assert!(together.iter().any(|(id, model)| {
            id.contains('/')
                && model.cost.as_ref().and_then(|c| c.input).is_some()
                && model.limit.as_ref().and_then(|l| l.context).is_some()
        }));
    }

    #[test]
    fn a_model_another_server_names_is_found_by_its_id_or_the_part_after_its_owner() {
        let snapshot = embedded().unwrap();
        let sonnet = snapshot.same_model("claude-sonnet-4-5").unwrap();
        assert_eq!(sonnet.limit.as_ref().unwrap().output, Some(64_000));
        let llama = snapshot
            .same_model("meta-llama/Llama-3.3-70B-Instruct")
            .unwrap();
        assert_eq!(llama.tool_call, Some(true));
        assert!(snapshot.same_model("Llama-3.3-70B-Instruct").is_some());
        assert!(
            snapshot
                .same_model("research/llama-3.3-70b-instruct")
                .is_some()
        );
        assert!(snapshot.same_model("my-finetune-v7").is_none());
    }

    #[test]
    fn every_provider_in_the_shipped_snapshot_has_a_valid_known_id() {
        let doc: BTreeMap<String, Value> = serde_json::from_str(EMBEDDED).unwrap();
        for (key, provider) in &doc {
            let id = ProviderId::parse(key).unwrap();
            assert!(
                id.provider().is_some(),
                "{key} is not a provider Kaeso calls"
            );
            assert_eq!(
                provider.get("id").and_then(Value::as_str),
                Some(key.as_str())
            );
        }
        assert_eq!(doc.len(), known_ids().len());
    }

    #[test]
    fn the_readme_names_the_shipped_fetch_time() {
        let readme = include_str!("../data/README.md");
        let format = time::format_description::well_known::Rfc3339;
        let stamp = EMBEDDED_FETCHED_AT.format(&format).unwrap();
        assert!(
            readme.contains(&format!("| Fetched | {stamp} |")),
            "{stamp}"
        );
    }

    #[test]
    fn unknown_fields_are_ignored_and_bad_models_skipped() {
        let doc = br#"{
            "openai": {"id": "openai", "name": "OpenAI", "brand_new": 1, "models": {
                "good": {"id": "good", "future_field": {"x": 1}, "cost": {"input": 1.25, "output": "10"}},
                "bad": {"id": "bad", "cost": {"input": "cheap"}},
                "mismatch": {"id": "other"}
            }},
            "elsewhere": {"id": "elsewhere", "models": {}}
        }"#;
        let parsed = parse(doc, &known_ids()).unwrap();
        assert_eq!(parsed.snapshot.providers.len(), 1);
        let good = parsed.snapshot.model("openai", "good").unwrap();
        assert_eq!(good.cost.as_ref().unwrap().input, Some(dec!(1.25)));
        assert_eq!(good.cost.as_ref().unwrap().output, Some(dec!(10)));
        assert_eq!(parsed.skipped, ["openai/bad", "openai/mismatch"]);
        assert!(parse(b"[]", &known_ids()).is_err());
        let none = parse(br#"{"x": {"models": {}}}"#, &known_ids()).unwrap_err();
        assert_eq!(
            none.to_string(),
            format!(
                "models.dev data has none of the providers {}",
                crate::provider::ids()
            )
        );
    }

    #[test]
    fn prices_keep_every_digit_models_dev_sends() {
        let doc = br#"{"openai": {"models": {"m": {"id": "m", "cost": {
            "input": 0.30000000000000004441, "output": 12.3456789012345678901,
            "cache_read": 1.5E-7, "cache_write": 2e2}}}}}"#;
        let parsed = parse(doc, &known_ids()).unwrap();
        let cost = parsed
            .snapshot
            .model("openai", "m")
            .unwrap()
            .cost
            .clone()
            .unwrap();
        assert_eq!(cost.input, Some(dec!(0.30000000000000004441)));
        assert_eq!(cost.output, Some(dec!(12.3456789012345678901)));
        assert_eq!(cost.cache_read, Some(dec!(0.00000015)));
        assert_eq!(cost.cache_write, Some(dec!(200)));
        let stored = serde_json::to_value(&parsed.snapshot).unwrap();
        let back: Snapshot = serde_json::from_value(stored).unwrap();
        assert_eq!(back, parsed.snapshot);
        assert!(serde_json::from_str::<Cost>(r#"{"input": true}"#).is_err());
        assert!(
            serde_json::from_str::<Cost>(r#"{"input": null}"#)
                .unwrap()
                .input
                .is_none()
        );
    }

    #[derive(Debug, Deserialize)]
    struct Amount {
        #[serde(default, deserialize_with = "price")]
        amount: Option<Decimal>,
    }

    #[derive(Debug, Deserialize)]
    struct Wrapped {
        #[serde(flatten)]
        inner: Amount,
    }

    #[test]
    fn requests_send_prices_as_decimal_strings_or_whole_numbers() {
        let read = |json: &str| serde_json::from_str::<Amount>(json).map(|a| a.amount);
        assert_eq!(
            read(r#"{"amount": "0.000000123"}"#).unwrap(),
            Some(dec!(0.000000123))
        );
        assert_eq!(read(r#"{"amount": 50}"#).unwrap(), Some(dec!(50)));
        assert_eq!(read(r#"{"amount": null}"#).unwrap(), None);
        assert_eq!(read("{}").unwrap(), None);
        assert!(read(r#"{"amount": 0.5}"#).is_err());
        assert!(read(r#"{"amount": -1}"#).is_err());
        assert!(read(r#"{"amount": true}"#).is_err());
        for refused in ["+5", "5.", "1_000", " 5", "5 ", "-1", ""] {
            assert!(
                read(&format!(r#"{{"amount": "{refused}"}}"#)).is_err(),
                "{refused:?}"
            );
        }
        assert_eq!(read(r#"{"amount": ".5"}"#).unwrap(), Some(dec!(0.5)));
        let flat: Wrapped = serde_json::from_str(r#"{"amount": "1.5"}"#).unwrap();
        assert_eq!(flat.inner.amount, Some(dec!(1.5)));
        let from_value: Amount = serde_json::from_value(serde_json::json!({"amount": 7})).unwrap();
        assert_eq!(from_value.amount, Some(dec!(7)));
    }

    #[test]
    fn prices_never_go_through_floats() {
        let c: Cost =
            serde_json::from_str(r#"{"input": 0.1, "output": 1e-7, "cache_read": "0.30"}"#)
                .unwrap();
        assert_eq!(c.input, Some(dec!(0.1)));
        assert_eq!(c.output, Some(dec!(0.0000001)));
        assert_eq!(c.cache_read, Some(dec!(0.3)));
        assert!(serde_json::from_str::<Cost>(r#"{"input": -1}"#).is_err());
        assert!(serde_json::from_str::<Cost>(r#"{"input": 1e25}"#).is_err());
        assert!(serde_json::from_str::<Cost>(r#"{"input": "1000000"}"#).is_ok());
    }
}
