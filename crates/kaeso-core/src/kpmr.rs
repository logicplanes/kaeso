use std::collections::BTreeMap;
use std::sync::LazyLock;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use kaeso_kpp::DialectId;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use time::OffsetDateTime;

use crate::decimal;
use crate::provider::{ProviderId, known_ids};

pub const EMBEDDED: &str = include_str!("../data/kpmr.json");

pub const DEFAULT_URL: &str = "https://kpmr.kaeso.app/v1/catalogue";

pub const FORMAT: u32 = 1;

pub const MEDIA_TYPE: &str = "application/vnd.kaeso.kpmr+json; version=1";

pub const SIGNATURE_HEADER: &str = "kpmr-signature";

pub const PRICE_UNIT: &str = "usd_per_million_tokens";

pub const BUILT_IN_KEY_ID: &str = "kpmr-2026-10";

pub const BUILT_IN_PUBLIC_KEY: &str = "EQI7HVO5opUSzyPkqSLOOkOPFoYH+QywDUYDHaQUnd8=";

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
pub struct Pricing {
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default, deserialize_with = "price")]
    pub input: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub output: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub cache_read: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub cache_write: Option<Decimal>,
    #[serde(default, deserialize_with = "price")]
    pub cache_write_1h: Option<Decimal>,
}

impl Pricing {
    fn prices(&self) -> [Option<Decimal>; 5] {
        [
            self.input,
            self.output,
            self.cache_read,
            self.cache_write,
            self.cache_write_1h,
        ]
    }
}

pub fn price<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    decimal::deserialize_with(d, parse_decimal)
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
pub struct Capabilities {
    #[serde(default)]
    pub tool_call: Option<bool>,
    #[serde(default)]
    pub forced_tool_choice: Option<bool>,
    #[serde(default)]
    pub structured_output: Option<bool>,
    #[serde(default)]
    pub temperature: Option<bool>,
    #[serde(default)]
    pub attachments: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reasoning {
    #[serde(default)]
    pub efforts: Option<Vec<String>>,
    #[serde(default)]
    pub adaptive_thinking: Option<bool>,
    #[serde(default)]
    pub thinking_binding: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gateway {
    #[serde(default, deserialize_with = "known_dialect")]
    pub egress_dialect: Option<DialectId>,
}

// A dialect only a newer gateway knows must not make this one skip the whole model.
fn known_dialect<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DialectId>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.and_then(|v| serde_json::from_value(v).ok()))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub modalities: Option<Modalities>,
    #[serde(default)]
    pub limits: Option<Limits>,
    #[serde(default)]
    pub pricing: Option<Pricing>,
    #[serde(default)]
    pub capabilities: Option<Capabilities>,
    #[serde(default)]
    pub reasoning: Option<Reasoning>,
    #[serde(default)]
    pub gateway: Option<Gateway>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub corrected: Vec<String>,
}

impl Model {
    pub fn is_deprecated(&self) -> bool {
        self.status.as_deref() == Some("deprecated")
    }

    pub fn egress_dialect(&self) -> Option<DialectId> {
        self.gateway.as_ref().and_then(|g| g.egress_dialect)
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
pub struct Catalogue {
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub generated_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub refresh_seconds: Option<u64>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    pub providers: BTreeMap<String, Provider>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub catalogue: Catalogue,
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a KPMR catalogue: {0}")]
    Shape(String),
    #[error("the KPMR catalogue is in format {0}; this gateway reads format 1")]
    Format(u64),
    #[error("the KPMR catalogue has none of the providers {0}")]
    NoProviders(String),
}

#[derive(Deserialize)]
struct Head<'a> {
    format: Option<u64>,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    generated_at: Option<OffsetDateTime>,
    #[serde(default)]
    refresh_seconds: Option<u64>,
    #[serde(default)]
    provider_ids: Vec<String>,
    #[serde(borrow)]
    providers: BTreeMap<String, &'a RawValue>,
}

#[derive(Deserialize)]
struct ProviderDoc<'a> {
    #[serde(default)]
    name: Option<Value>,
    #[serde(default, borrow)]
    models: Option<&'a RawValue>,
}

fn readable(model: &Model) -> bool {
    model.pricing.as_ref().is_none_or(|p| {
        p.unit.as_deref() == Some(PRICE_UNIT) || p.prices().iter().all(Option::is_none)
    })
}

pub fn parse(json: &[u8], providers: &[ProviderId]) -> Result<Parsed, ParseError> {
    let head: Head<'_> =
        serde_json::from_slice(json).map_err(|e| ParseError::Shape(e.to_string()))?;
    match head.format {
        Some(f) if f == u64::from(FORMAT) => {}
        Some(f) => return Err(ParseError::Format(f)),
        None => return Err(ParseError::Shape("it names no format".into())),
    }
    let mut catalogue = Catalogue {
        revision: head.revision,
        generated_at: head.generated_at,
        refresh_seconds: head.refresh_seconds,
        provider_ids: head.provider_ids,
        providers: BTreeMap::new(),
    };
    let mut skipped = Vec::new();
    for id in providers {
        let Some(raw) = head.providers.get(id.as_str()) else {
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
                Ok(model) if model.id == *model_id && readable(&model) => {
                    parsed.insert(model_id.clone(), model);
                }
                _ => skipped.push(format!("{id}/{model_id}")),
            }
        }
        catalogue.providers.insert(
            id.to_string(),
            Provider {
                id: id.to_string(),
                name,
                models: parsed,
            },
        );
    }
    if catalogue.providers.is_empty() {
        let ids: Vec<&str> = providers.iter().map(ProviderId::as_str).collect();
        return Err(ParseError::NoProviders(ids.join(", ")));
    }
    Ok(Parsed { catalogue, skipped })
}

pub fn embedded() -> Result<Catalogue, ParseError> {
    Ok(parse(EMBEDDED.as_bytes(), &known_ids())?.catalogue)
}

static EMBEDDED_PROVIDER_IDS: LazyLock<Vec<String>> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct Ids {
        #[serde(default)]
        provider_ids: Vec<String>,
    }
    serde_json::from_str::<Ids>(EMBEDDED)
        .map(|i| i.provider_ids)
        .unwrap_or_default()
});

pub fn embedded_provider_ids() -> &'static [String] {
    &EMBEDDED_PROVIDER_IDS
}

impl Catalogue {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedKey {
    pub id: String,
    key: VerifyingKey,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("`{0}` is not `<key id>:<base64 Ed25519 public key>`")]
    Form(String),
    #[error("the key `{0}` is not a base64 Ed25519 public key of 32 bytes")]
    Key(String),
}

impl TrustedKey {
    pub fn new(id: &str, public_key: &str) -> Result<TrustedKey, KeyError> {
        let id = id.trim();
        if id.is_empty() || id.contains([',', ' ', '=']) {
            return Err(KeyError::Form(format!("{id}:{public_key}")));
        }
        let bytes: [u8; 32] = STANDARD
            .decode(public_key.trim())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| KeyError::Key(id.to_owned()))?;
        let key = VerifyingKey::from_bytes(&bytes).map_err(|_| KeyError::Key(id.to_owned()))?;
        Ok(TrustedKey {
            id: id.to_owned(),
            key,
        })
    }

    pub fn parse(entry: &str) -> Result<TrustedKey, KeyError> {
        let (id, key) = entry
            .split_once(':')
            .ok_or_else(|| KeyError::Form(entry.to_owned()))?;
        TrustedKey::new(id, key)
    }

    pub fn built_in() -> Result<TrustedKey, KeyError> {
        TrustedKey::new(BUILT_IN_KEY_ID, BUILT_IN_PUBLIC_KEY)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("the catalogue is not signed")]
    Missing,
    #[error("the signature header is not `keyid=<id>, ed25519=<base64>`")]
    Malformed,
    #[error("the catalogue is signed with `{0}`, a key this gateway does not trust")]
    UnknownKey(String),
    #[error("the signature of `{0}` does not match the catalogue")]
    Mismatch(String),
}

pub fn verify(
    body: &[u8],
    header: Option<&str>,
    keys: &[TrustedKey],
) -> Result<(), SignatureError> {
    let header = header.ok_or(SignatureError::Missing)?;
    let mut key_id = None;
    let mut signature = None;
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("keyid", v)) => key_id = Some(v.trim()),
            Some(("ed25519", v)) => signature = Some(v.trim()),
            _ => {}
        }
    }
    let (Some(key_id), Some(signature)) = (key_id, signature) else {
        return Err(SignatureError::Malformed);
    };
    let signature: [u8; 64] = STANDARD
        .decode(signature)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(SignatureError::Malformed)?;
    let key = keys
        .iter()
        .find(|k| k.id == key_id)
        .ok_or_else(|| SignatureError::UnknownKey(key_id.to_owned()))?;
    key.key
        .verify(body, &Signature::from_bytes(&signature))
        .map_err(|_| SignatureError::Mismatch(key_id.to_owned()))
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use rust_decimal_macros::dec;

    use super::*;

    fn catalogue(providers: &str) -> Vec<u8> {
        format!(
            r#"{{"format": 1, "revision": "r1", "generated_at": "2026-10-31T21:00:00Z", "refresh_seconds": 600,
            "sources": {{"models_dev": null}}, "provider_ids": ["anthropic", "openai", "zeta"], "providers": {providers}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn the_shipped_catalogue_parses_completely() {
        let parsed = parse(EMBEDDED.as_bytes(), &known_ids()).unwrap();
        assert!(parsed.skipped.is_empty(), "{:?}", parsed.skipped);
        let c = &parsed.catalogue;
        assert_eq!(c.providers.len(), crate::provider::PROVIDERS.len());
        assert!(c.generated_at.is_some() && c.revision.is_some());
        let sonnet = c.model("anthropic", "claude-sonnet-4-5").unwrap();
        let pricing = sonnet.pricing.as_ref().unwrap();
        assert_eq!(pricing.input, Some(dec!(3)));
        assert_eq!(pricing.cache_read, Some(dec!(0.3)));
        assert_eq!(pricing.cache_write_1h, Some(dec!(6)));
        assert_eq!(sonnet.limits.as_ref().unwrap().output, Some(64_000));
        assert!(
            sonnet
                .modalities
                .as_ref()
                .unwrap()
                .input
                .contains(&"pdf".to_owned())
        );
        let gpt = c.model("openai", "gpt-5").unwrap();
        assert_eq!(
            gpt.reasoning.as_ref().unwrap().efforts.as_deref().unwrap(),
            ["minimal", "low", "medium", "high"]
        );
        assert_eq!(gpt.pricing.as_ref().unwrap().cache_write_1h, None);
        let together = &c.providers["togetherai"].models;
        assert!(together.iter().any(|(id, model)| {
            id.contains('/')
                && model.pricing.as_ref().and_then(|p| p.input).is_some()
                && model.limits.as_ref().and_then(|l| l.context).is_some()
        }));
    }

    #[test]
    fn the_shipped_catalogue_lists_every_provider_id_and_only_providers_kaeso_calls() {
        let doc: Value = serde_json::from_str(EMBEDDED).unwrap();
        let providers = doc["providers"].as_object().unwrap();
        for (key, provider) in providers {
            let id = ProviderId::parse(key).unwrap();
            assert!(
                id.provider().is_some(),
                "{key} is not a provider Kaeso calls"
            );
            assert_eq!(provider["id"].as_str(), Some(key.as_str()));
        }
        assert_eq!(providers.len(), known_ids().len());
        let ids = embedded_provider_ids();
        assert!(ids.is_sorted() && ids.len() > 100, "{ids:?}");
        for p in crate::provider::PROVIDERS {
            assert!(ids.iter().any(|i| i == p.id), "{}", p.id);
        }
    }

    #[test]
    fn a_model_another_server_names_is_found_by_its_id_or_the_part_after_its_owner() {
        let c = embedded().unwrap();
        let sonnet = c.same_model("claude-sonnet-4-5").unwrap();
        assert_eq!(sonnet.limits.as_ref().unwrap().output, Some(64_000));
        let llama = c.same_model("meta-llama/Llama-3.3-70B-Instruct").unwrap();
        assert_eq!(llama.capabilities.as_ref().unwrap().tool_call, Some(true));
        assert!(c.same_model("Llama-3.3-70B-Instruct").is_some());
        assert!(c.same_model("research/llama-3.3-70b-instruct").is_some());
        assert!(c.same_model("my-finetune-v7").is_none());
    }

    #[test]
    fn the_readme_names_the_shipped_revision() {
        let readme = include_str!("../data/README.md");
        let revision = embedded().unwrap().revision.unwrap();
        assert!(
            readme.contains(&format!("| Revision | `{revision}` |")),
            "{revision}"
        );
    }

    #[test]
    fn unknown_fields_are_ignored_and_bad_models_skipped() {
        let doc = catalogue(
            r#"{
            "openai": {"id": "openai", "name": "OpenAI", "brand_new": 1, "models": {
                "good": {"id": "good", "future_field": {"x": 1}, "pricing": {"unit": "usd_per_million_tokens", "input": "1.25", "output": "10", "future": "1"},
                    "gateway": {"egress_dialect": "openai_videos"}, "reasoning": {"efforts": ["low"], "next": true}},
                "float": {"id": "float", "pricing": {"unit": "usd_per_million_tokens", "input": 1.25}},
                "bad": {"id": "bad", "pricing": {"unit": "usd_per_million_tokens", "input": "cheap"}},
                "euro": {"id": "euro", "pricing": {"unit": "eur_per_million_tokens", "input": "1"}},
                "mismatch": {"id": "other"}
            }},
            "elsewhere": {"id": "elsewhere", "models": {}}
        }"#,
        );
        let parsed = parse(&doc, &known_ids()).unwrap();
        assert_eq!(parsed.catalogue.providers.len(), 1);
        assert_eq!(parsed.catalogue.refresh_seconds, Some(600));
        assert_eq!(
            parsed.catalogue.provider_ids,
            ["anthropic", "openai", "zeta"]
        );
        let good = parsed.catalogue.model("openai", "good").unwrap();
        assert_eq!(good.pricing.as_ref().unwrap().input, Some(dec!(1.25)));
        assert_eq!(good.pricing.as_ref().unwrap().output, Some(dec!(10)));
        assert_eq!(good.egress_dialect(), None);
        assert_eq!(
            parsed.skipped,
            [
                "openai/bad",
                "openai/euro",
                "openai/float",
                "openai/mismatch"
            ]
        );
        let stored = serde_json::to_value(&parsed.catalogue).unwrap();
        let back: Catalogue = serde_json::from_value(stored).unwrap();
        assert_eq!(back, parsed.catalogue);
        assert!(parse(b"[]", &known_ids()).is_err());
        let none = parse(&catalogue(r#"{"x": {"models": {}}}"#), &known_ids()).unwrap_err();
        assert_eq!(
            none.to_string(),
            format!(
                "the KPMR catalogue has none of the providers {}",
                crate::provider::ids()
            )
        );
        let newer = parse(br#"{"format": 2, "providers": {}}"#, &known_ids()).unwrap_err();
        assert_eq!(
            newer.to_string(),
            "the KPMR catalogue is in format 2; this gateway reads format 1"
        );
        assert!(parse(br#"{"providers": {}}"#, &known_ids()).is_err());
    }

    #[test]
    fn prices_keep_every_digit_kpmr_sends() {
        let doc = catalogue(
            r#"{"openai": {"models": {"m": {"id": "m", "pricing": {"unit": "usd_per_million_tokens",
            "input": "0.30000000000000004441", "output": "12.3456789012345678901",
            "cache_read": "0.00000015", "cache_write": "200", "cache_write_1h": null}}}}}"#,
        );
        let parsed = parse(&doc, &known_ids()).unwrap();
        let p = parsed
            .catalogue
            .model("openai", "m")
            .unwrap()
            .pricing
            .clone()
            .unwrap();
        assert_eq!(p.input, Some(dec!(0.30000000000000004441)));
        assert_eq!(p.output, Some(dec!(12.3456789012345678901)));
        assert_eq!(p.cache_read, Some(dec!(0.00000015)));
        assert_eq!(p.cache_write, Some(dec!(200)));
        assert_eq!(p.cache_write_1h, None);
        assert!(serde_json::from_str::<Pricing>(r#"{"input": true}"#).is_err());
        assert!(serde_json::from_str::<Pricing>(r#"{"input": 0.1}"#).is_err());
        assert!(serde_json::from_str::<Pricing>(r#"{"input": "-1"}"#).is_err());
        assert!(serde_json::from_str::<Pricing>(r#"{"input": "1e25"}"#).is_err());
        assert!(serde_json::from_str::<Pricing>(r#"{"input": "1000000"}"#).is_ok());
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

    fn signing_key(seed: u8) -> (SigningKey, String) {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        (key, public)
    }

    #[test]
    fn only_a_catalogue_signed_by_a_trusted_key_verifies() {
        let (key, public) = signing_key(7);
        let trusted = [TrustedKey::parse(&format!("test-1:{public}")).unwrap()];
        let body = catalogue("{}");
        let sig = STANDARD.encode(key.sign(&body).to_bytes());
        let header = format!("keyid=test-1, ed25519={sig}");
        assert_eq!(verify(&body, Some(&header), &trusted), Ok(()));
        let mut changed = body.clone();
        changed.push(b' ');
        assert_eq!(
            verify(&changed, Some(&header), &trusted),
            Err(SignatureError::Mismatch("test-1".into()))
        );
        assert_eq!(verify(&body, None, &trusted), Err(SignatureError::Missing));
        assert_eq!(
            verify(
                &body,
                Some(&format!("keyid=other, ed25519={sig}")),
                &trusted
            ),
            Err(SignatureError::UnknownKey("other".into()))
        );
        for bad in [
            "keyid=test-1",
            "ed25519=abc",
            "keyid=test-1, ed25519=not-base64!",
        ] {
            assert_eq!(
                verify(&body, Some(bad), &trusted),
                Err(SignatureError::Malformed),
                "{bad}"
            );
        }
        let (other, _) = signing_key(8);
        let forged = STANDARD.encode(other.sign(&body).to_bytes());
        assert_eq!(
            verify(
                &body,
                Some(&format!("keyid=test-1, ed25519={forged}")),
                &trusted
            ),
            Err(SignatureError::Mismatch("test-1".into()))
        );
    }

    #[test]
    fn trusted_keys_are_an_id_and_a_32_byte_ed25519_key() {
        assert_eq!(TrustedKey::built_in().unwrap().id, BUILT_IN_KEY_ID);
        let (_, public) = signing_key(3);
        assert_eq!(
            TrustedKey::parse(&format!(" mirror :{public}")).unwrap().id,
            "mirror"
        );
        assert!(matches!(
            TrustedKey::parse("no-colon"),
            Err(KeyError::Form(_))
        ));
        assert!(matches!(
            TrustedKey::parse(&format!(":{public}")),
            Err(KeyError::Form(_))
        ));
        assert!(matches!(
            TrustedKey::parse("short:AAAA"),
            Err(KeyError::Key(_))
        ));
    }
}
