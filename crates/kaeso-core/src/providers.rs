use std::sync::Arc;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_kpp::DialectId;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::provider::{self, AccountSignIn, AuthStyle, Provider, ProviderId};

pub const MODELS_DEV_PROVIDERS: &str = include_str!("../data/models-dev-providers.txt");

pub const GENERATION_DIALECTS: [DialectId; 3] = [
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::AnthropicMessages,
];

pub const EMBEDDINGS_DIALECTS: [DialectId; 1] = [DialectId::OpenAiEmbeddings];

pub const COUNTING_DIALECTS: [DialectId; 2] =
    [DialectId::OpenAiResponses, DialectId::AnthropicMessages];

pub const MAX_ID_LEN: usize = 40;

pub const MAX_NAME_LEN: usize = 100;

pub const MAX_BASE_URL_LEN: usize = 2048;

pub const METRIC_LABEL: &str = "custom";

pub fn is_reserved(id: &str) -> bool {
    provider::provider(id).is_some() || MODELS_DEV_PROVIDERS.lines().any(|line| line == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomAuth {
    Bearer,
    XApiKey,
    None,
}

impl CustomAuth {
    pub const ALL: [CustomAuth; 3] = [CustomAuth::Bearer, CustomAuth::XApiKey, CustomAuth::None];

    pub const fn as_str(self) -> &'static str {
        match self {
            CustomAuth::Bearer => "bearer",
            CustomAuth::XApiKey => "x_api_key",
            CustomAuth::None => "none",
        }
    }

    pub fn parse(s: &str) -> Option<CustomAuth> {
        CustomAuth::ALL.into_iter().find(|a| a.as_str() == s)
    }

    pub const fn style(self) -> Option<AuthStyle> {
        match self {
            CustomAuth::Bearer => Some(AuthStyle::Bearer),
            CustomAuth::XApiKey => Some(AuthStyle::XApiKey),
            CustomAuth::None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    IdTooLong,
    Name,
    BaseUrlLength,
    BaseUrlScheme,
    BaseUrlHost,
    BaseUrlCredentials,
    BaseUrlQuery,
    Generation,
    Embeddings,
    CountTokens,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub uuid: uuid::Uuid,
    pub id: ProviderId,
    pub name: String,
    pub base_url: String,
    pub auth: CustomAuth,
    pub generation: DialectId,
    pub embeddings: Option<DialectId>,
    pub count_tokens: Option<DialectId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Draft", into = "Draft")]
pub struct CustomProvider {
    uuid: uuid::Uuid,
    id: ProviderId,
    name: String,
    base_url: Url,
    auth: CustomAuth,
    generation: DialectId,
    embeddings: Option<DialectId>,
    count_tokens: Option<DialectId>,
    dialects: Vec<DialectId>,
}

impl CustomProvider {
    pub fn new(d: Draft) -> Result<CustomProvider, Problem> {
        if d.id.as_str().len() > MAX_ID_LEN {
            return Err(Problem::IdTooLong);
        }
        let name = d.name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_NAME_LEN
            || name.chars().any(char::is_control)
        {
            return Err(Problem::Name);
        }
        let base_url = base_url(&d.base_url)?;
        if !GENERATION_DIALECTS.contains(&d.generation) {
            return Err(Problem::Generation);
        }
        if d.embeddings
            .is_some_and(|e| !EMBEDDINGS_DIALECTS.contains(&e))
        {
            return Err(Problem::Embeddings);
        }
        if d.count_tokens
            .is_some_and(|c| c != d.generation || !COUNTING_DIALECTS.contains(&c))
        {
            return Err(Problem::CountTokens);
        }
        let dialects = std::iter::once(d.generation).chain(d.embeddings).collect();
        Ok(CustomProvider {
            uuid: d.uuid,
            id: d.id,
            name: name.to_owned(),
            base_url,
            auth: d.auth,
            generation: d.generation,
            embeddings: d.embeddings,
            count_tokens: d.count_tokens,
            dialects,
        })
    }

    pub fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn auth(&self) -> CustomAuth {
        self.auth
    }

    pub fn generation(&self) -> DialectId {
        self.generation
    }

    pub fn embeddings(&self) -> Option<DialectId> {
        self.embeddings
    }

    pub fn count_tokens(&self) -> Option<DialectId> {
        self.count_tokens
    }

    pub fn dialects(&self) -> &[DialectId] {
        &self.dialects
    }

    pub fn draft(&self) -> Draft {
        Draft {
            uuid: self.uuid,
            id: self.id.clone(),
            name: self.name.clone(),
            base_url: self.base_url.as_str().to_owned(),
            auth: self.auth,
            generation: self.generation,
            embeddings: self.embeddings,
            count_tokens: self.count_tokens,
        }
    }
}

impl TryFrom<Draft> for CustomProvider {
    type Error = String;
    fn try_from(d: Draft) -> Result<Self, Self::Error> {
        let id = d.id.clone();
        CustomProvider::new(d).map_err(|p| format!("custom provider `{id}`: {p:?}"))
    }
}

impl From<CustomProvider> for Draft {
    fn from(p: CustomProvider) -> Draft {
        p.draft()
    }
}

impl Serialize for Draft {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = s.serialize_struct("CustomProvider", 8)?;
        out.serialize_field("uuid", &self.uuid)?;
        out.serialize_field("id", &self.id)?;
        out.serialize_field("name", &self.name)?;
        out.serialize_field("base_url", &self.base_url)?;
        out.serialize_field("auth", &self.auth)?;
        out.serialize_field("generation", &self.generation)?;
        out.serialize_field("embeddings", &self.embeddings)?;
        out.serialize_field("count_tokens", &self.count_tokens)?;
        out.end()
    }
}

impl<'de> Deserialize<'de> for Draft {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Stored {
            uuid: uuid::Uuid,
            id: ProviderId,
            name: String,
            base_url: String,
            auth: CustomAuth,
            generation: DialectId,
            embeddings: Option<DialectId>,
            count_tokens: Option<DialectId>,
        }
        let s = Stored::deserialize(d)?;
        Ok(Draft {
            uuid: s.uuid,
            id: s.id,
            name: s.name,
            base_url: s.base_url,
            auth: s.auth,
            generation: s.generation,
            embeddings: s.embeddings,
            count_tokens: s.count_tokens,
        })
    }
}

fn base_url(text: &str) -> Result<Url, Problem> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_BASE_URL_LEN {
        return Err(Problem::BaseUrlLength);
    }
    let url = Url::parse(text).map_err(|_| Problem::BaseUrlHost)?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(Problem::BaseUrlScheme);
    }
    if url.host().is_none() {
        return Err(Problem::BaseUrlHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Problem::BaseUrlCredentials);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Problem::BaseUrlQuery);
    }
    Ok(url)
}

// A built-in provider that a later release adds under an id an organization already uses
// stays hidden inside that organization: its keys were made for the custom endpoint, and
// sending them to the new built-in provider would hand them to a third party.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderRef {
    BuiltIn(&'static Provider),
    Custom(Arc<CustomProvider>),
}

impl ProviderRef {
    pub fn id(&self) -> &str {
        match self {
            ProviderRef::BuiltIn(p) => p.id,
            ProviderRef::Custom(c) => c.id.as_str(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ProviderRef::BuiltIn(p) => p.name,
            ProviderRef::Custom(c) => &c.name,
        }
    }

    pub fn generation(&self) -> DialectId {
        match self {
            ProviderRef::BuiltIn(p) => p.generation,
            ProviderRef::Custom(c) => c.generation,
        }
    }

    pub fn count_tokens(&self) -> Option<DialectId> {
        match self {
            ProviderRef::BuiltIn(p) => p.count_tokens,
            ProviderRef::Custom(c) => c.count_tokens,
        }
    }

    pub fn embeddings(&self) -> Option<DialectId> {
        match self {
            ProviderRef::BuiltIn(p) => p.embeddings,
            ProviderRef::Custom(c) => c.embeddings,
        }
    }

    pub fn dialects(&self) -> &[DialectId] {
        match self {
            ProviderRef::BuiltIn(p) => p.dialects,
            ProviderRef::Custom(c) => &c.dialects,
        }
    }

    pub fn auth(&self) -> Option<AuthStyle> {
        match self {
            ProviderRef::BuiltIn(p) => Some(p.auth),
            ProviderRef::Custom(c) => c.auth.style(),
        }
    }

    pub fn chat(&self) -> ChatProfile {
        match self {
            ProviderRef::BuiltIn(p) => p.chat,
            ProviderRef::Custom(_) => ChatProfile::OPENAI,
        }
    }

    pub fn responses(&self) -> ResponsesProfile {
        match self {
            ProviderRef::BuiltIn(p) => p.responses,
            ProviderRef::Custom(_) => ResponsesProfile::compatible("OpenAI"),
        }
    }

    pub fn account(&self) -> Option<AccountSignIn> {
        self.built_in().and_then(|p| p.account)
    }

    pub fn refuses_key(&self, message: &str) -> bool {
        self.built_in().is_some_and(|p| p.refuses_key(message))
    }

    pub fn reasoning_beyond_output_limit(&self) -> bool {
        self.built_in()
            .is_some_and(|p| p.reasoning_beyond_output_limit)
    }

    pub fn built_in(&self) -> Option<&'static Provider> {
        match self {
            ProviderRef::BuiltIn(p) => Some(p),
            ProviderRef::Custom(_) => None,
        }
    }

    pub fn custom(&self) -> Option<&CustomProvider> {
        match self {
            ProviderRef::BuiltIn(_) => None,
            ProviderRef::Custom(c) => Some(c),
        }
    }

    pub fn metric_label(&self) -> &'static str {
        match self {
            ProviderRef::BuiltIn(p) => p.id,
            ProviderRef::Custom(_) => METRIC_LABEL,
        }
    }
}

impl From<&'static Provider> for ProviderRef {
    fn from(p: &'static Provider) -> ProviderRef {
        ProviderRef::BuiltIn(p)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<CustomProvider>", into = "Vec<CustomProvider>")]
pub struct OrgProviders {
    custom: Vec<Arc<CustomProvider>>,
}

impl From<Vec<CustomProvider>> for OrgProviders {
    fn from(list: Vec<CustomProvider>) -> OrgProviders {
        OrgProviders::new(list)
    }
}

impl From<OrgProviders> for Vec<CustomProvider> {
    fn from(p: OrgProviders) -> Vec<CustomProvider> {
        p.custom.iter().map(|c| CustomProvider::clone(c)).collect()
    }
}

impl OrgProviders {
    pub fn new(mut list: Vec<CustomProvider>) -> OrgProviders {
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list.dedup_by(|a, b| a.id == b.id);
        OrgProviders {
            custom: list.into_iter().map(Arc::new).collect(),
        }
    }

    pub const fn built_in_only() -> OrgProviders {
        OrgProviders { custom: Vec::new() }
    }

    pub fn get(&self, id: &str) -> Option<ProviderRef> {
        self.custom_by_id(id)
            .map(|c| ProviderRef::Custom(Arc::clone(c)))
            .or_else(|| provider::provider(id).map(ProviderRef::BuiltIn))
    }

    pub fn custom(&self) -> &[Arc<CustomProvider>] {
        &self.custom
    }

    pub fn hidden_built_in(&self, id: &str) -> Option<&'static Provider> {
        self.custom_by_id(id).and(provider::provider(id))
    }

    fn custom_by_id(&self, id: &str) -> Option<&Arc<CustomProvider>> {
        self.custom
            .binary_search_by(|c| c.id.as_str().cmp(id))
            .ok()
            .and_then(|i| self.custom.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn draft(id: &str, base_url: &str) -> Draft {
        Draft {
            uuid: uuid::Uuid::now_v7(),
            id: ProviderId::parse(id).unwrap(),
            name: "Local vLLM".into(),
            base_url: base_url.into(),
            auth: CustomAuth::Bearer,
            generation: DialectId::OpenAiChat,
            embeddings: None,
            count_tokens: None,
        }
    }

    #[test]
    fn built_in_and_models_dev_ids_are_reserved() {
        for id in ["openai", "anthropic", "groq", "lmstudio", "302ai", "zai"] {
            assert!(is_reserved(id), "{id}");
        }
        for id in ["my-vllm", "internal-proxy", "ollama-local"] {
            assert!(!is_reserved(id), "{id}");
        }
        for p in provider::PROVIDERS {
            assert!(MODELS_DEV_PROVIDERS.lines().any(|l| l == p.id), "{}", p.id);
        }
    }

    #[test]
    fn a_custom_provider_names_its_dialects_and_keeps_to_the_ones_it_may_have() {
        let mut d = draft("my-vllm", "https://vllm.example.com/");
        d.embeddings = Some(DialectId::OpenAiEmbeddings);
        let p = CustomProvider::new(d.clone()).unwrap();
        assert_eq!(
            p.dialects(),
            [DialectId::OpenAiChat, DialectId::OpenAiEmbeddings]
        );
        assert_eq!(p.base_url().as_str(), "https://vllm.example.com/");

        let refused = |change: fn(&mut Draft)| {
            let mut d = draft("my-vllm", "https://vllm.example.com");
            change(&mut d);
            CustomProvider::new(d).unwrap_err()
        };
        assert_eq!(
            refused(|d| d.generation = DialectId::OpenAiEmbeddings),
            Problem::Generation
        );
        assert_eq!(
            refused(|d| d.embeddings = Some(DialectId::OpenAiChat)),
            Problem::Embeddings
        );
        assert_eq!(
            refused(|d| d.count_tokens = Some(DialectId::OpenAiChat)),
            Problem::CountTokens
        );
        assert_eq!(
            refused(|d| d.count_tokens = Some(DialectId::AnthropicMessages)),
            Problem::CountTokens
        );
        assert_eq!(refused(|d| d.name = "  ".into()), Problem::Name);
        assert_eq!(refused(|d| d.name = "a\u{7}b".into()), Problem::Name);
        assert_eq!(
            refused(|d| d.base_url = "ftp://x.example".into()),
            Problem::BaseUrlScheme
        );
        assert_eq!(
            refused(|d| d.base_url = "https://u:p@x.example".into()),
            Problem::BaseUrlCredentials
        );
        assert_eq!(
            refused(|d| d.base_url = "https://x.example/?a=1".into()),
            Problem::BaseUrlQuery
        );
        assert_eq!(
            refused(|d| d.base_url = "https://x.example/#a".into()),
            Problem::BaseUrlQuery
        );
        assert_eq!(
            refused(|d| d.base_url = "not a url".into()),
            Problem::BaseUrlHost
        );
        assert_eq!(
            refused(|d| d.base_url = String::new()),
            Problem::BaseUrlLength
        );
        assert_eq!(
            refused(|d| d.id = ProviderId::parse(&"a".repeat(41)).unwrap()),
            Problem::IdTooLong
        );

        let mut counting = draft("my-proxy", "https://proxy.example.com/anthropic");
        counting.generation = DialectId::AnthropicMessages;
        counting.count_tokens = Some(DialectId::AnthropicMessages);
        counting.auth = CustomAuth::XApiKey;
        let p = CustomProvider::new(counting).unwrap();
        let r = ProviderRef::Custom(Arc::new(p));
        assert_eq!(r.count_tokens(), Some(DialectId::AnthropicMessages));
        assert_eq!(r.auth(), Some(AuthStyle::XApiKey));
        assert_eq!(r.chat(), ChatProfile::OPENAI);
        assert_eq!(r.responses(), ResponsesProfile::compatible("OpenAI"));
        assert!(r.account().is_none() && !r.refuses_key("Incorrect API key provided"));
        assert_eq!(r.metric_label(), "custom");
    }

    #[test]
    fn a_local_server_without_auth_sends_no_key() {
        let mut d = draft("ollama-local", "http://localhost:11434");
        d.auth = CustomAuth::None;
        let r = ProviderRef::Custom(Arc::new(CustomProvider::new(d).unwrap()));
        assert_eq!(r.auth(), None);
        for a in CustomAuth::ALL {
            assert_eq!(CustomAuth::parse(a.as_str()), Some(a));
        }
        assert_eq!(CustomAuth::parse("basic"), None);
    }

    #[test]
    fn an_organizations_provider_wins_over_a_built_in_one_of_the_same_id_inside_it() {
        let org = OrgProviders::new(vec![
            CustomProvider::new(draft("my-vllm", "https://vllm.example.com")).unwrap(),
            CustomProvider::new(draft("groq", "https://groq-proxy.example.com")).unwrap(),
        ]);
        assert_eq!(org.get("my-vllm").unwrap().name(), "Local vLLM");
        let groq = org.get("groq").unwrap();
        assert!(groq.custom().is_some(), "the organization's own groq wins");
        assert_eq!(org.hidden_built_in("groq").map(|p| p.name), Some("Groq"));
        assert_eq!(org.hidden_built_in("my-vllm"), None);
        assert_eq!(
            org.get("openai").unwrap().built_in(),
            provider::provider("openai")
        );

        let other = OrgProviders::built_in_only();
        assert!(
            other.get("my-vllm").is_none(),
            "another organization never sees it"
        );
        assert_eq!(
            other.get("groq").unwrap().built_in(),
            provider::provider("groq")
        );
    }

    #[test]
    fn org_providers_read_back_from_their_cached_form_and_refuse_a_broken_one() {
        let org = OrgProviders::new(vec![
            CustomProvider::new(draft("my-vllm", "https://vllm.example.com/v1-proxy")).unwrap(),
        ]);
        let json = serde_json::to_value(&org).unwrap();
        assert_eq!(json[0]["id"], "my-vllm");
        assert_eq!(json[0]["generation"], "openai_chat");
        assert_eq!(
            serde_json::from_value::<OrgProviders>(json.clone()).unwrap(),
            org
        );
        let mut broken = json;
        broken[0]["base_url"] = "file:///etc/passwd".into();
        assert!(serde_json::from_value::<OrgProviders>(broken).is_err());
    }
}
