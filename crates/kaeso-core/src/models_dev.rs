use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::macros::datetime;

pub const EMBEDDED: &str = include_str!("../data/models-dev.json");

pub const EMBEDDED_FETCHED_AT: OffsetDateTime = datetime!(2026-10-11 20:44:44 UTC);

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
    #[serde(default, deserialize_with = "price")]
    pub input: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub output: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub cache_read: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub cache_write: Option<Decimal>,
}

pub fn price<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => parse_decimal(&n.to_string())
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(Value::String(s)) => parse_decimal(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected a price, got {other}"
        ))),
    }
}

const MAX_PRICE: u32 = 1_000_000;

fn parse_decimal(s: &str) -> Result<Decimal, String> {
    Decimal::from_str(s)
        .or_else(|_| Decimal::from_scientific(s))
        .map_err(|_| format!("`{s}` is not a decimal number"))
        .and_then(|d| {
            if d.is_sign_negative() {
                Err(format!("`{s}` is negative"))
            } else if d > Decimal::from(MAX_PRICE) {
                Err(format!("`{s}` is above {MAX_PRICE} USD per million tokens"))
            } else {
                Ok(d.normalize())
            }
        })
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

pub fn parse(json: &[u8], providers: &[&str]) -> Result<Parsed, ParseError> {
    let doc: BTreeMap<String, Value> =
        serde_json::from_slice(json).map_err(|e| ParseError::Shape(e.to_string()))?;
    let mut snapshot = Snapshot::default();
    let mut skipped = Vec::new();
    for id in providers {
        let Some(raw) = doc.get(*id) else { continue };
        let name = raw.get("name").and_then(Value::as_str).map(str::to_owned);
        let Some(models) = raw.get("models").and_then(Value::as_object) else {
            return Err(ParseError::Shape(format!("provider `{id}` has no models")));
        };
        let mut parsed = BTreeMap::new();
        for (model_id, m) in models {
            match serde_json::from_value::<Model>(m.clone()) {
                Ok(model) if model.id == *model_id => {
                    parsed.insert(model_id.clone(), model);
                }
                _ => skipped.push(format!("{id}/{model_id}")),
            }
        }
        snapshot.providers.insert(
            (*id).to_owned(),
            Provider {
                id: (*id).to_owned(),
                name,
                models: parsed,
            },
        );
    }
    if snapshot.providers.is_empty() {
        return Err(ParseError::NoProviders(providers.join(", ")));
    }
    Ok(Parsed { snapshot, skipped })
}

pub fn embedded() -> Result<Snapshot, ParseError> {
    let ids: Vec<&str> = crate::provider::PROVIDERS.iter().map(|p| p.id).collect();
    Ok(parse(EMBEDDED.as_bytes(), &ids)?.snapshot)
}

impl Snapshot {
    pub fn model(&self, provider: &str, model: &str) -> Option<&Model> {
        self.providers.get(provider)?.models.get(model)
    }

    pub fn model_count(&self) -> usize {
        self.providers.values().map(|p| p.models.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn the_shipped_snapshot_parses_completely() {
        let ids: Vec<&str> = crate::provider::PROVIDERS.iter().map(|p| p.id).collect();
        let parsed = parse(EMBEDDED.as_bytes(), &ids).unwrap();
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
        let parsed = parse(doc, &["openai", "anthropic"]).unwrap();
        assert_eq!(parsed.snapshot.providers.len(), 1);
        let good = parsed.snapshot.model("openai", "good").unwrap();
        assert_eq!(good.cost.as_ref().unwrap().input, Some(dec!(1.25)));
        assert_eq!(good.cost.as_ref().unwrap().output, Some(dec!(10)));
        assert_eq!(parsed.skipped, ["openai/bad", "openai/mismatch"]);
        assert!(parse(b"[]", &["openai"]).is_err());
        assert!(parse(br#"{"x": {"models": {}}}"#, &["openai"]).is_err());
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
