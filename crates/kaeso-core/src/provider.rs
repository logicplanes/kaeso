use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use kaeso_kpp::DialectId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const ID_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

pub const ID_RULE: &str = "lowercase letters and digits, with single hyphens between them";

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderIdError {
    #[error("`{0}` is not a provider id: {ID_RULE}")]
    Malformed(String),
    #[error("`{0}` is not a provider Kaeso knows; the providers are {known}", known = ids())]
    Unknown(String),
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProviderId(String);

impl ProviderId {
    pub const fn is_valid(id: &str) -> bool {
        let mut rest = id.as_bytes();
        let mut after_hyphen = true;
        while let [b, tail @ ..] = rest {
            match *b {
                b'a'..=b'z' | b'0'..=b'9' => after_hyphen = false,
                b'-' if !after_hyphen => after_hyphen = true,
                _ => return false,
            }
            rest = tail;
        }
        !after_hyphen
    }

    pub fn parse(id: &str) -> Result<ProviderId, ProviderIdError> {
        if ProviderId::is_valid(id) {
            Ok(ProviderId(id.to_owned()))
        } else {
            Err(ProviderIdError::Malformed(id.to_owned()))
        }
    }

    pub fn from_env_segment(segment: &str) -> Result<ProviderId, ProviderIdError> {
        let malformed = || ProviderIdError::Malformed(segment.to_owned());
        if segment.bytes().any(|b| b.is_ascii_lowercase() || b == b'-') {
            return Err(malformed());
        }
        ProviderId::parse(&segment.to_ascii_lowercase().replace('_', "-")).map_err(|_| malformed())
    }

    pub fn env_segment(&self) -> String {
        self.0.to_ascii_uppercase().replace('-', "_")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn provider(&self) -> Option<&'static Provider> {
        known(&self.0)
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ProviderId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for ProviderId {
    type Err = ProviderIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ProviderId::parse(s)
    }
}

impl Serialize for ProviderId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = std::borrow::Cow::<str>::deserialize(d)?;
        ProviderId::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for ProviderId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(format!("A provider id: {ID_RULE}.")))
            .pattern(Some(ID_PATTERN))
            .examples(["openai"])
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for ProviderId {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthStyle {
    Bearer,
    XApiKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub default_base_url: &'static str,
    pub auth: AuthStyle,
    pub generation: DialectId,
    pub count_tokens: Option<DialectId>,
    pub embeddings: Option<DialectId>,
    pub dialects: &'static [DialectId],
    pub refused_key_messages: &'static [&'static str],
}

pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "openai",
        name: "OpenAI",
        default_base_url: "https://api.openai.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiResponses,
        count_tokens: Some(DialectId::OpenAiResponses),
        embeddings: Some(DialectId::OpenAiEmbeddings),
        dialects: &[
            DialectId::OpenAiResponses,
            DialectId::OpenAiChat,
            DialectId::OpenAiEmbeddings,
        ],
        refused_key_messages: &[],
    },
    Provider {
        id: "anthropic",
        name: "Anthropic",
        default_base_url: "https://api.anthropic.com",
        auth: AuthStyle::XApiKey,
        generation: DialectId::AnthropicMessages,
        count_tokens: Some(DialectId::AnthropicMessages),
        embeddings: None,
        dialects: &[DialectId::AnthropicMessages],
        refused_key_messages: &[],
    },
    Provider {
        id: "deepseek",
        name: "DeepSeek",
        default_base_url: "https://api.deepseek.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
    },
    Provider {
        id: "xai",
        name: "SpaceX",
        default_base_url: "https://api.x.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiResponses,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiResponses, DialectId::OpenAiChat],
        refused_key_messages: &["Incorrect API key provided"],
    },
    Provider {
        id: "togetherai",
        name: "Together AI",
        default_base_url: "https://api.together.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
    },
];

const _: () = {
    let mut rest = PROVIDERS;
    while let [p, tail @ ..] = rest {
        assert!(
            ProviderId::is_valid(p.id),
            "every provider id follows ID_PATTERN"
        );
        rest = tail;
    }
};

pub fn lookup(id: &str) -> Result<&'static Provider, ProviderIdError> {
    if !ProviderId::is_valid(id) {
        return Err(ProviderIdError::Malformed(id.to_owned()));
    }
    known(id).ok_or_else(|| ProviderIdError::Unknown(id.to_owned()))
}

static BY_ID: LazyLock<Vec<&'static Provider>> = LazyLock::new(|| {
    let mut by_id: Vec<&'static Provider> = PROVIDERS.iter().collect();
    by_id.sort_by_key(|p| p.id);
    by_id.dedup_by_key(|p| p.id);
    by_id
});

fn known(id: &str) -> Option<&'static Provider> {
    BY_ID
        .binary_search_by(|p| p.id.cmp(id))
        .ok()
        .and_then(|i| BY_ID.get(i).copied())
}

pub fn provider(id: &str) -> Option<&'static Provider> {
    lookup(id).ok()
}

impl Provider {
    pub fn provider_id(&self) -> ProviderId {
        ProviderId(self.id.to_owned())
    }

    pub fn refuses_key(&self, message: &str) -> bool {
        self.refused_key_messages
            .iter()
            .any(|refusal| message.starts_with(refusal))
    }
}

pub fn known_ids() -> Vec<ProviderId> {
    PROVIDERS.iter().map(Provider::provider_id).collect()
}

pub fn ids() -> String {
    PROVIDERS
        .iter()
        .map(|p| p.id)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mvp_providers_use_their_own_apis() {
        let openai = provider("openai").unwrap();
        assert_eq!(
            (openai.auth, openai.generation),
            (AuthStyle::Bearer, DialectId::OpenAiResponses)
        );
        let anthropic = provider("anthropic").unwrap();
        assert_eq!(
            (anthropic.auth, anthropic.embeddings),
            (AuthStyle::XApiKey, None)
        );
        assert!(provider("gemini").is_none());
        assert_eq!(ids(), "openai, anthropic, deepseek, xai, togetherai");
    }

    #[test]
    fn every_provider_is_found_by_its_own_id_and_no_other() {
        for p in PROVIDERS {
            assert_eq!(provider(p.id), Some(p));
        }
        assert_eq!(BY_ID.len(), PROVIDERS.len());
    }

    #[test]
    fn provider_ids_are_lowercase_words_joined_by_single_hyphens() {
        for good in [
            "openai",
            "anthropic",
            "azure-openai",
            "together-ai",
            "302ai",
            "a",
            "x-1-y",
        ] {
            assert_eq!(ProviderId::parse(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "-",
            "OpenAI",
            "azure_openai",
            "azure--openai",
            "-openai",
            "openai-",
            "open.ai",
            "open ai",
            "open/ai",
            "ö",
            "openai\n",
        ] {
            assert_eq!(
                ProviderId::parse(bad),
                Err(ProviderIdError::Malformed(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_pattern_matches_exactly_the_ids_the_validator_accepts() {
        let pattern = regex::Regex::new(ID_PATTERN).unwrap();
        for id in [
            "openai",
            "azure-openai",
            "",
            "-a",
            "a-",
            "a--b",
            "a_b",
            "A",
            "a.b",
            "1-2",
            "a\n",
        ] {
            assert_eq!(pattern.is_match(id), ProviderId::is_valid(id), "{id:?}");
        }
    }

    #[test]
    fn a_provider_names_the_messages_with_which_it_refuses_a_key() {
        let xai = provider("xai").unwrap();
        assert!(xai.refuses_key(
            "Incorrect API key provided. You can obtain an API key from https://console.x.ai."
        ));
        assert!(!xai.refuses_key("Model grok-4.3 does not support parameter presencePenalty."));
        assert!(
            !provider("openai")
                .unwrap()
                .refuses_key("Incorrect API key provided: sk-…")
        );
    }

    #[test]
    fn lookup_tells_malformed_from_unknown_ids() {
        assert_eq!(lookup("anthropic").unwrap().name, "Anthropic");
        assert_eq!(
            lookup("together-ai").unwrap_err().to_string(),
            format!(
                "`together-ai` is not a provider Kaeso knows; the providers are {}",
                ids()
            )
        );
        assert_eq!(
            lookup("Together_AI").unwrap_err().to_string(),
            "`Together_AI` is not a provider id: lowercase letters and digits, with single hyphens between them"
        );
        assert_eq!(
            known_ids()
                .iter()
                .map(ProviderId::as_str)
                .collect::<Vec<_>>(),
            ["openai", "anthropic", "deepseek", "xai", "togetherai"]
        );
        assert_eq!(
            ProviderId::parse("openai").unwrap().provider(),
            provider("openai")
        );
        assert!(
            ProviderId::parse("together-ai")
                .unwrap()
                .provider()
                .is_none()
        );
    }

    #[test]
    fn environment_segments_map_back_to_exactly_one_id() {
        let id: ProviderId = "azure-openai".parse().unwrap();
        assert_eq!(id.env_segment(), "AZURE_OPENAI");
        assert_eq!(ProviderId::from_env_segment("AZURE_OPENAI").unwrap(), id);
        for bad in [
            "",
            "_OPENAI",
            "OPENAI_",
            "AZURE__OPENAI",
            "azure_openai",
            "AZURE-OPENAI",
            "OPEN.AI",
        ] {
            assert!(ProviderId::from_env_segment(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ids_serialize_as_strings_and_refuse_malformed_input() {
        let id: ProviderId = serde_json::from_str("\"together-ai\"").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"together-ai\"");
        assert_eq!(format!("{id} {id:?}"), "together-ai \"together-ai\"");
        let err = serde_json::from_str::<ProviderId>("\"together_ai\"").unwrap_err();
        assert!(err.to_string().contains("is not a provider id"), "{err}");
    }

    proptest::proptest! {
        #[test]
        fn every_valid_id_survives_the_environment_round_trip(id in "[a-z0-9]{1,8}(-[a-z0-9]{1,8}){0,3}") {
            let parsed = ProviderId::parse(&id).unwrap();
            let segment = parsed.env_segment();
            proptest::prop_assert!(segment.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'));
            proptest::prop_assert_eq!(ProviderId::from_env_segment(&segment).unwrap(), parsed);
        }

        #[test]
        fn the_validator_and_the_pattern_agree(id in "[a-zA-Z0-9_.-]{0,12}") {
            let pattern = regex::Regex::new(ID_PATTERN).unwrap();
            proptest::prop_assert_eq!(pattern.is_match(&id), ProviderId::is_valid(&id));
        }
    }
}
