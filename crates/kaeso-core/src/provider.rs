use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use kaeso_codecs::openai_chat::ChatProfile;
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
pub struct KeyPage {
    pub url: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AuthMethod {
    ApiKey,
    ChatGpt,
    Grok,
}

impl AuthMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            AuthMethod::ApiKey => "api_key",
            AuthMethod::ChatGpt => "chatgpt",
            AuthMethod::Grok => "grok",
        }
    }

    pub fn parse(s: &str) -> Option<AuthMethod> {
        [AuthMethod::ApiKey, AuthMethod::ChatGpt, AuthMethod::Grok]
            .into_iter()
            .find(|m| m.as_str() == s)
    }

    pub const fn is_account(self) -> bool {
        !matches!(self, AuthMethod::ApiKey)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountSignIn {
    pub method: AuthMethod,
    pub product: &'static str,
    pub usage_url: Option<&'static str>,
    pub experimental: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub key_page: KeyPage,
    pub key_prefix: Option<&'static str>,
    pub default_base_url: &'static str,
    pub auth: AuthStyle,
    pub generation: DialectId,
    pub count_tokens: Option<DialectId>,
    pub embeddings: Option<DialectId>,
    pub dialects: &'static [DialectId],
    pub refused_key_messages: &'static [&'static str],
    pub reasoning_beyond_output_limit: bool,
    pub tool_namespaces: bool,
    pub web_search_access: bool,
    pub null_reasoning_content: bool,
    pub chat: ChatProfile,
    pub account: Option<AccountSignIn>,
}

pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "openai",
        name: "OpenAI",
        key_page: KeyPage {
            url: "https://platform.openai.com/api-keys",
            label: "API keys",
        },
        key_prefix: Some("sk-"),
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
        reasoning_beyond_output_limit: false,
        tool_namespaces: true,
        web_search_access: true,
        null_reasoning_content: true,
        chat: ChatProfile::OPENAI,
        account: Some(AccountSignIn {
            method: AuthMethod::ChatGpt,
            product: "ChatGPT",
            usage_url: Some("https://chatgpt.com/settings/usage"),
            experimental: false,
        }),
    },
    Provider {
        id: "anthropic",
        name: "Anthropic",
        key_page: KeyPage {
            url: "https://platform.claude.com/settings/keys",
            label: "Settings → API keys",
        },
        key_prefix: Some("sk-ant-"),
        default_base_url: "https://api.anthropic.com",
        auth: AuthStyle::XApiKey,
        generation: DialectId::AnthropicMessages,
        count_tokens: Some(DialectId::AnthropicMessages),
        embeddings: None,
        dialects: &[DialectId::AnthropicMessages],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("Anthropic"),
        account: None,
    },
    Provider {
        id: "deepseek",
        name: "DeepSeek",
        key_page: KeyPage {
            url: "https://platform.deepseek.com/api_keys",
            label: "API keys",
        },
        key_prefix: Some("sk-"),
        default_base_url: "https://api.deepseek.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("DeepSeek")
            .max_tokens()
            .system_role()
            .reasoning_fields(&["reasoning_content"]),
        account: None,
    },
    Provider {
        id: "xai",
        name: "SpaceX",
        key_page: KeyPage {
            url: "https://console.x.ai/team/default/api-keys",
            label: "API Keys",
        },
        key_prefix: Some("xai-"),
        default_base_url: "https://api.x.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiResponses,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiResponses, DialectId::OpenAiChat],
        refused_key_messages: &["Incorrect API key provided"],
        reasoning_beyond_output_limit: true,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("SpaceX").reasoning_fields(&["reasoning_content"]),
        account: Some(AccountSignIn {
            method: AuthMethod::Grok,
            product: "Grok",
            usage_url: None,
            experimental: true,
        }),
    },
    Provider {
        id: "togetherai",
        name: "Together AI",
        key_page: KeyPage {
            url: "https://api.together.ai/settings/projects/~current/api-keys",
            label: "API Keys",
        },
        key_prefix: Some("tgp_v1_"),
        default_base_url: "https://api.together.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("Together AI")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        account: None,
    },
    Provider {
        id: "groq",
        name: "Groq",
        key_page: KeyPage {
            url: "https://console.groq.com/keys",
            label: "API Keys",
        },
        key_prefix: Some("gsk_"),
        default_base_url: "https://api.groq.com/openai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("Groq")
            .system_role()
            .reasoning_fields(&["reasoning"])
            .refusing(&["logit_bias", "logprobs", "top_logprobs"])
            .refusing_in_messages(&["name", "refusal"]),
        account: None,
    },
    Provider {
        id: "openrouter",
        name: "OpenRouter",
        key_page: KeyPage {
            url: "https://openrouter.ai/settings/keys",
            label: "API Keys",
        },
        key_prefix: Some("sk-or-v1-"),
        default_base_url: "https://openrouter.ai/api",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("OpenRouter")
            .reasoning_fields(&["reasoning"])
            .error_detail(&["error", "metadata", "raw"]),
        account: None,
    },
    Provider {
        id: "mistral",
        name: "Mistral",
        key_page: KeyPage {
            url: "https://console.mistral.ai/api-keys",
            label: "API keys",
        },
        key_prefix: None,
        default_base_url: "https://api.mistral.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        reasoning_beyond_output_limit: false,
        tool_namespaces: false,
        web_search_access: false,
        null_reasoning_content: false,
        chat: ChatProfile::compatible("Mistral")
            .max_tokens()
            .system_role()
            .random_seed()
            .without_end_user()
            .without_prefill()
            .thinking_parts()
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "moderation",
                "prompt_cache_options",
                "prompt_cache_retention",
                "store",
                "verbosity",
                "web_search_options",
            ])
            .refusing_in_messages(&["name", "refusal"]),
        account: None,
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
    fn only_openai_takes_responses_tool_namespaces() {
        let taking: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.tool_namespaces)
            .map(|p| p.id)
            .collect();
        assert_eq!(taking, ["openai"]);
    }

    #[test]
    fn only_openai_takes_web_search_access_and_null_reasoning_content() {
        let taking: Vec<(&str, bool, bool)> = PROVIDERS
            .iter()
            .filter(|p| p.web_search_access || p.null_reasoning_content)
            .map(|p| (p.id, p.web_search_access, p.null_reasoning_content))
            .collect();
        assert_eq!(taking, [("openai", true, true)]);
    }

    #[test]
    fn openai_and_xai_sign_in_with_an_account_and_methods_keep_their_names() {
        let accounts: Vec<(&str, AuthMethod, bool)> = PROVIDERS
            .iter()
            .filter_map(|p| p.account.map(|a| (p.id, a.method, a.experimental)))
            .collect();
        assert_eq!(
            accounts,
            [
                ("openai", AuthMethod::ChatGpt, false),
                ("xai", AuthMethod::Grok, true)
            ]
        );
        for method in [AuthMethod::ApiKey, AuthMethod::ChatGpt, AuthMethod::Grok] {
            assert_eq!(AuthMethod::parse(method.as_str()), Some(method));
        }
        assert_eq!(AuthMethod::parse("password"), None);
        assert!(AuthMethod::ChatGpt.is_account());
        assert!(AuthMethod::Grok.is_account());
        assert!(!AuthMethod::ApiKey.is_account());
    }

    #[test]
    fn every_provider_id_is_listed_once() {
        let mut ids: Vec<&str> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let listed = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), listed, "a provider id is listed twice");
    }

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
        assert_eq!(
            ids(),
            "openai, anthropic, deepseek, xai, togetherai, groq, openrouter, mistral"
        );
    }

    #[test]
    fn every_provider_is_found_by_its_own_id_and_no_other() {
        for p in PROVIDERS {
            assert_eq!(provider(p.id), Some(p));
        }
        assert_eq!(BY_ID.len(), PROVIDERS.len());
    }

    fn sent_with_profile(provider_id: &str, request: &str) -> Result<serde_json::Value, String> {
        use kaeso_codecs::dispatch;
        use kaeso_codecs::{EgressCtx, HeaderList};
        let path = format!(
            "{}/../kaeso-codecs/fixtures/openai_chat/{request}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let body = std::fs::read(&path).unwrap();
        let req = dispatch::decode_request(
            DialectId::OpenAiChat,
            kaeso_kpp::RequestKind::Generate,
            &body,
            &HeaderList::new(),
        )
        .unwrap();
        let profile = provider(provider_id).unwrap().chat;
        let mut ctx = EgressCtx::new(&req).with_chat_profile(profile);
        dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx)
            .map(|out| serde_json::from_slice(&out.body).unwrap())
            .map_err(|e| e.param.unwrap_or_default())
    }

    #[test]
    fn each_profile_setting_answers_the_request_the_provider_was_recorded_refusing() {
        for (provider_id, request, param) in [
            ("groq", "requests/logit-bias", "logit_bias"),
            ("groq", "requests/logprobs-top-logprobs-doc", "logprobs"),
            (
                "groq",
                "requests/tools-results-parallel-error",
                "messages[2].refusal",
            ),
        ] {
            assert_eq!(
                sent_with_profile(provider_id, request).unwrap_err(),
                param,
                "{provider_id} {request}"
            );
        }
        for (provider_id, request) in [
            ("deepseek", "requests/plain-developer-string"),
            ("groq", "requests/tools-single"),
        ] {
            let sent = sent_with_profile(provider_id, request).unwrap();
            let roles: Vec<&str> = sent["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|m| m["role"].as_str())
                .collect();
            assert!(
                !roles.contains(&"developer"),
                "{provider_id} {request}: {roles:?}"
            );
            assert!(
                roles.contains(&"system"),
                "{provider_id} {request}: {roles:?}"
            );
        }
    }

    #[test]
    fn deepseek_gets_every_apps_output_limit_as_max_tokens_which_it_honours() {
        let recorded = sent_with_profile(
            "deepseek",
            "providers/deepseek/requests/max-completion-tokens-limit",
        )
        .unwrap();
        assert_eq!(recorded["max_tokens"], 20, "{recorded}");
        assert!(
            recorded.get("max_completion_tokens").is_none(),
            "{recorded}"
        );

        let mut g = kaeso_kpp::GenerateRequest::new("deepseek-flash");
        g.extras.origin = Some(kaeso_kpp::Origin::new(DialectId::OpenAiResponses));
        g.messages = vec![kaeso_kpp::Message::user("Count to forty.")];
        g.sampling.max_output_tokens = Some(60);
        let req = kaeso_kpp::Request::Generate(g);
        let mut ctx = kaeso_codecs::EgressCtx::new(&req)
            .with_chat_profile(provider("deepseek").unwrap().chat);
        let out =
            kaeso_codecs::dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx).unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(sent["max_tokens"], 60, "{sent}");
        assert!(sent.get("max_completion_tokens").is_none(), "{sent}");
    }

    #[test]
    fn mistrals_profile_answers_each_request_mistral_was_recorded_refusing() {
        for (request, param) in [
            ("requests/logit-bias", "logit_bias"),
            ("providers/mistral/requests/store-field", "store"),
            (
                "providers/mistral/requests/function-call-field",
                "function_call",
            ),
            ("providers/mistral/requests/audio-field", "audio"),
            (
                "providers/mistral/requests/assistant-refusal-field",
                "messages[1].refusal",
            ),
            (
                "providers/mistral/requests/assistant-prefill",
                "messages[1]",
            ),
            ("requests/system-name-multiturn", "messages[0].name"),
        ] {
            assert_eq!(
                sent_with_profile("mistral", request).unwrap_err(),
                param,
                "{request}"
            );
        }
        let limit =
            sent_with_profile("mistral", "requests/max-completion-tokens-legacy-null").unwrap();
        assert!(limit.get("max_completion_tokens").is_none(), "{limit}");
        assert!(limit.get("max_tokens").is_some(), "{limit}");
        let seed = sent_with_profile("mistral", "requests/stop-string-seed-penalties").unwrap();
        assert!(
            seed.get("seed").is_none() && seed.get("random_seed").is_some(),
            "{seed}"
        );
        let user = sent_with_profile("mistral", "providers/mistral/requests/user-field").unwrap();
        assert!(user.get("user").is_none(), "{user}");
        let developer = sent_with_profile("mistral", "requests/plain-developer-string").unwrap();
        assert_eq!(developer["messages"][0]["role"], "system");
    }

    #[test]
    fn every_provider_has_what_people_see_of_it() {
        for p in PROVIDERS {
            assert!(!p.name.is_empty() && p.name.trim() == p.name, "{}", p.id);
            let page = p
                .key_page
                .url
                .strip_prefix("https://")
                .unwrap_or_else(|| panic!("{}: the key page is not https", p.id));
            assert!(
                page.split('/')
                    .next()
                    .is_some_and(|host| host.contains('.')),
                "{}: {page}",
                p.id
            );
            assert!(
                !p.key_page.label.is_empty() && p.key_page.label.trim() == p.key_page.label,
                "{}",
                p.id
            );
            if let Some(prefix) = p.key_prefix {
                assert!(
                    !prefix.is_empty() && !prefix.contains(char::is_whitespace),
                    "{}: {prefix:?}",
                    p.id
                );
                let leaked = format!("key {prefix}AbCd1234 refused");
                assert!(
                    !crate::secret::redact_tokens(&leaked).contains("AbCd1234"),
                    "{}: keys starting {prefix:?} are not redacted from provider errors",
                    p.id
                );
            }
            assert_eq!(p.dialects.first(), Some(&p.generation), "{}", p.id);
            for d in [p.count_tokens, p.embeddings].into_iter().flatten() {
                assert!(p.dialects.contains(&d), "{}: {d}", p.id);
            }
        }
    }

    #[test]
    fn every_provider_names_itself_in_its_chat_profile() {
        for p in PROVIDERS {
            assert_eq!(p.chat.provider, p.name, "{}", p.id);
        }
        assert_eq!(provider("openai").unwrap().chat, ChatProfile::OPENAI);
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
            [
                "openai",
                "anthropic",
                "deepseek",
                "xai",
                "togetherai",
                "groq",
                "openrouter",
                "mistral"
            ]
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
