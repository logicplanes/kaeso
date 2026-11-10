use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use kaeso_codecs::anthropic_messages::{FixedValue, MINIMAX_VIDEO_MEDIA_TYPES, MessagesProfile};
use kaeso_codecs::gemini::GeminiProfile;
use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::openai_images::ImagesProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_codecs::openai_speech::SpeechProfile;
use kaeso_codecs::openai_transcriptions::TranscriptionsProfile;
use kaeso_codecs::openai_videos::VideosProfile;
use kaeso_codecs::{ToolPatterns, ToolSchemaRoot};
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
    XGoogApiKey,
    /// ElevenLabs' `xi-api-key` header.
    XiApiKey,
}

impl AuthStyle {
    pub const fn header_name(self) -> &'static str {
        match self {
            AuthStyle::Bearer => "authorization",
            AuthStyle::XApiKey => "x-api-key",
            AuthStyle::XGoogApiKey => "x-goog-api-key",
            AuthStyle::XiApiKey => "xi-api-key",
        }
    }

    pub fn header_value(self, key: &str) -> String {
        match self {
            AuthStyle::Bearer => format!("Bearer {key}"),
            AuthStyle::XApiKey | AuthStyle::XGoogApiKey | AuthStyle::XiApiKey => key.to_owned(),
        }
    }
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
    GoogleServiceAccount,
}

impl AuthMethod {
    pub const ALL: [AuthMethod; 4] = [
        AuthMethod::ApiKey,
        AuthMethod::ChatGpt,
        AuthMethod::Grok,
        AuthMethod::GoogleServiceAccount,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AuthMethod::ApiKey => "api_key",
            AuthMethod::ChatGpt => "chatgpt",
            AuthMethod::Grok => "grok",
            AuthMethod::GoogleServiceAccount => "google_service_account",
        }
    }

    pub fn parse(s: &str) -> Option<AuthMethod> {
        AuthMethod::ALL.into_iter().find(|m| m.as_str() == s)
    }

    pub const fn is_account(self) -> bool {
        matches!(self, AuthMethod::ChatGpt | AuthMethod::Grok)
    }

    pub const fn is_secret_key(self) -> bool {
        matches!(self, AuthMethod::ApiKey | AuthMethod::GoogleServiceAccount)
    }
}

// The hosts QwenCloud's result links point at: DashScope's OSS buckets, such as
// `dashscope-result-sgp.oss-ap-southeast-1.aliyuncs.com` in its image, speech and video references.
const DASHSCOPE_RESULTS: &[&str] = &["dashscope-*.aliyuncs.com"];

/// Where and how Kaeso sends image requests to a provider, for the models `models` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImagesEgress {
    pub dialect: DialectId,
    pub profile: ImagesProfile,
    /// The dialect's paths start at the root of the host, not under the base URL's path, as
    /// Z.ai and MiniMax serve their image APIs beside the Anthropic-format path Kaeso uses.
    pub from_root: bool,
    /// The auth header the image API takes when it differs from the provider's.
    pub auth: Option<AuthStyle>,
    /// The model ids this API serves, `*` standing for any run of characters.
    pub models: &'static str,
    /// The hosts Kaeso may fetch the provider's image links from, for an app that asks for the
    /// bytes of a provider that answers with links only.
    pub fetch_hosts: &'static [&'static str],
    /// Whether rewriting the prompt (`prompt_extend`) doubles the price per image.
    pub extended_prompt_doubles: bool,
}

impl ImagesEgress {
    /// OpenAI's Images API with OpenAI's own profile, as a custom provider's server serves it.
    pub const OPENAI_COMPATIBLE: ImagesEgress = ImagesEgress::ANY;

    const ANY: ImagesEgress = ImagesEgress {
        dialect: DialectId::OpenAiImages,
        profile: ImagesProfile::OPENAI,
        from_root: false,
        auth: None,
        models: "*",
        fetch_hosts: &[],
        extended_prompt_doubles: false,
    };

    const fn openai(profile: ImagesProfile) -> ImagesEgress {
        ImagesEgress {
            profile,
            ..ImagesEgress::ANY
        }
    }

    const fn gemini() -> ImagesEgress {
        ImagesEgress {
            dialect: DialectId::Gemini,
            ..ImagesEgress::ANY
        }
    }

    const fn dashscope() -> ImagesEgress {
        ImagesEgress {
            dialect: DialectId::DashScope,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            fetch_hosts: DASHSCOPE_RESULTS,
            ..ImagesEgress::ANY
        }
    }

    // ElevenLabs answers a finished image generation with a signed link on Google Cloud
    // Storage (`content_url` in its image and video guide).
    const fn elevenlabs() -> ImagesEgress {
        ImagesEgress {
            dialect: DialectId::ElevenLabs,
            fetch_hosts: &["storage.googleapis.com"],
            ..ImagesEgress::ANY
        }
    }

    const fn for_models(self, models: &'static str) -> ImagesEgress {
        ImagesEgress { models, ..self }
    }

    const fn fetching(self, fetch_hosts: &'static [&'static str]) -> ImagesEgress {
        ImagesEgress {
            fetch_hosts,
            ..self
        }
    }

    const fn doubling_with_extended_prompt(self) -> ImagesEgress {
        ImagesEgress {
            extended_prompt_doubles: true,
            ..self
        }
    }

    pub fn serves(&self, model: &str) -> bool {
        glob(self.models, model)
    }
}

/// Where and how Kaeso sends speech requests to a provider, for the models `models` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechEgress {
    pub dialect: DialectId,
    /// How a provider that speaks a form of OpenAI's speech API takes it.
    pub profile: SpeechProfile,
    /// The dialect's paths start at the root of the host, as for [`ImagesEgress::from_root`].
    pub from_root: bool,
    /// The auth header the speech API takes when it differs from the provider's.
    pub auth: Option<AuthStyle>,
    /// The model ids this API serves, as for [`ImagesEgress::models`].
    pub models: &'static str,
    /// The hosts Kaeso may fetch a whole reply's audio link from, as for
    /// [`ImagesEgress::fetch_hosts`].
    pub fetch_hosts: &'static [&'static str],
}

impl SpeechEgress {
    /// OpenAI's speech API with OpenAI's own profile, as a custom provider's server serves it.
    pub const OPENAI_COMPATIBLE: SpeechEgress = SpeechEgress::ANY;

    const ANY: SpeechEgress = SpeechEgress {
        dialect: DialectId::OpenAiSpeech,
        profile: SpeechProfile::OPENAI,
        from_root: false,
        auth: None,
        models: "*",
        fetch_hosts: &[],
    };

    const fn openai(profile: SpeechProfile) -> SpeechEgress {
        SpeechEgress {
            profile,
            ..SpeechEgress::ANY
        }
    }

    const fn gemini() -> SpeechEgress {
        SpeechEgress {
            dialect: DialectId::Gemini,
            ..SpeechEgress::ANY
        }
    }

    const fn dashscope() -> SpeechEgress {
        SpeechEgress {
            dialect: DialectId::DashScope,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            fetch_hosts: DASHSCOPE_RESULTS,
            ..SpeechEgress::ANY
        }
    }

    pub fn serves(&self, model: &str) -> bool {
        glob(self.models, model)
    }
}

/// Where and how Kaeso sends transcription requests to a provider, for the models `models`
/// matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranscriptionsEgress {
    pub dialect: DialectId,
    pub profile: TranscriptionsProfile,
    pub from_root: bool,
    pub auth: Option<AuthStyle>,
    /// The model ids this API serves, as for [`ImagesEgress::models`].
    pub models: &'static str,
}

impl TranscriptionsEgress {
    /// OpenAI's transcriptions API with OpenAI's own profile, as a custom provider's server
    /// serves it.
    pub const OPENAI_COMPATIBLE: TranscriptionsEgress = TranscriptionsEgress::ANY;

    const ANY: TranscriptionsEgress = TranscriptionsEgress {
        dialect: DialectId::OpenAiTranscriptions,
        profile: TranscriptionsProfile::OPENAI,
        from_root: false,
        auth: None,
        models: "*",
    };

    const fn openai(profile: TranscriptionsProfile) -> TranscriptionsEgress {
        TranscriptionsEgress {
            profile,
            ..TranscriptionsEgress::ANY
        }
    }

    const fn gemini() -> TranscriptionsEgress {
        TranscriptionsEgress {
            dialect: DialectId::Gemini,
            ..TranscriptionsEgress::ANY
        }
    }

    const fn dashscope() -> TranscriptionsEgress {
        TranscriptionsEgress {
            dialect: DialectId::DashScope,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..TranscriptionsEgress::ANY
        }
    }

    const fn for_models(self, models: &'static str) -> TranscriptionsEgress {
        TranscriptionsEgress { models, ..self }
    }

    pub fn serves(&self, model: &str) -> bool {
        glob(self.models, model)
    }
}

/// Where and how Kaeso sends video jobs to a provider, for the models `models` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideosEgress {
    pub dialect: DialectId,
    /// How a provider that speaks a form of OpenAI's Videos API takes it.
    pub profile: VideosProfile,
    /// The dialect's paths start at the root of the host, as for [`ImagesEgress::from_root`].
    pub from_root: bool,
    /// The auth header the video API takes when it differs from the provider's.
    pub auth: Option<AuthStyle>,
    /// The model ids this API serves, `*` standing for any run of characters.
    pub models: &'static str,
}

impl VideosEgress {
    /// OpenAI's Videos API with OpenAI's own profile, as a custom provider's server serves it.
    pub const OPENAI_COMPATIBLE: VideosEgress = VideosEgress::of(DialectId::OpenAiVideos);

    const fn of(dialect: DialectId) -> VideosEgress {
        VideosEgress {
            dialect,
            profile: VideosProfile::OPENAI,
            from_root: false,
            auth: None,
            models: "*",
        }
    }

    const fn rooted(dialect: DialectId) -> VideosEgress {
        VideosEgress {
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..VideosEgress::of(dialect)
        }
    }

    const fn for_models(self, models: &'static str) -> VideosEgress {
        VideosEgress { models, ..self }
    }

    pub fn serves(&self, model: &str) -> bool {
        glob(self.models, model)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelRoute {
    pub pattern: &'static str,
    pub dialect: DialectId,
    pub counts_tokens: bool,
}

impl ModelRoute {
    pub fn matches(&self, model: &str) -> bool {
        glob(self.pattern, model)
    }
}

/// Whether `host` is one of `hosts`, each a pattern where `*` stands for any run of characters.
pub fn fetchable(hosts: &[&str], host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    hosts.iter().any(|pattern| glob(pattern, &host))
}

// `*` stands for any run of characters, every other character for itself.
fn glob(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return text.is_empty();
    };
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let Some(last) = parts.pop() else {
        return rest.is_empty();
    };
    for part in parts {
        match rest.find(part) {
            Some(at) => rest = rest.get(at + part.len()..).unwrap_or(""),
            None => return false,
        }
    }
    rest.ends_with(last)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceAccountSignIn {
    pub key_page: KeyPage,
    pub locations: &'static [&'static str],
    pub default_location: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountSignIn {
    pub method: AuthMethod,
    pub product: &'static str,
    pub usage_url: Option<&'static str>,
    pub experimental: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Where Kaeso sends image requests, by model, the first match winning; empty for a
    /// provider that offers no image API.
    pub images: &'static [ImagesEgress],
    /// Where Kaeso sends speech requests, by model, as for `images`.
    pub speech: &'static [SpeechEgress],
    /// Where Kaeso sends transcriptions, by model, as for `images`.
    pub transcriptions: &'static [TranscriptionsEgress],
    /// Where Kaeso sends video jobs, by model; empty for a provider that offers no video API.
    pub videos: &'static [VideosEgress],
    pub dialects: &'static [DialectId],
    pub refused_key_messages: &'static [&'static str],
    pub context_window_messages: &'static [&'static str],
    pub reasoning_beyond_output_limit: bool,
    pub end_user_limit: Option<usize>,
    pub chat: ChatProfile,
    pub responses: ResponsesProfile,
    pub messages: MessagesProfile,
    pub gemini: GeminiProfile,
    pub account: Option<AccountSignIn>,
    pub routes: &'static [ModelRoute],
    pub service_account: Option<ServiceAccountSignIn>,
}

// The locations that serve Gemini or Claude models, from the tables of
// https://docs.cloud.google.com/gemini-enterprise-agent-platform/resources/locations (2026-11-07).
const AGENT_PLATFORM_LOCATIONS: &[&str] = &[
    "global",
    "us",
    "eu",
    "us-central1",
    "us-east1",
    "us-east4",
    "us-east5",
    "us-south1",
    "us-west1",
    "us-west4",
    "northamerica-northeast1",
    "southamerica-east1",
    "europe-central2",
    "europe-north1",
    "europe-southwest1",
    "europe-west1",
    "europe-west2",
    "europe-west3",
    "europe-west4",
    "europe-west6",
    "europe-west8",
    "europe-west9",
    "asia-east1",
    "asia-east2",
    "asia-northeast1",
    "asia-northeast3",
    "asia-south1",
    "asia-southeast1",
    "australia-southeast1",
    "me-central1",
    "me-central2",
    "me-west1",
];

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
        images: &[ImagesEgress::openai(ImagesProfile::OPENAI)],
        speech: &[SpeechEgress::openai(SpeechProfile::OPENAI)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::OPENAI)],
        videos: &[],
        dialects: &[
            DialectId::OpenAiResponses,
            DialectId::OpenAiChat,
            DialectId::OpenAiEmbeddings,
        ],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // OpenAI's OpenAPI spec caps `safety_identifier` at 128 characters, in both formats.
        end_user_limit: Some(128),
        chat: ChatProfile::OPENAI,
        responses: ResponsesProfile::OPENAI,
        messages: MessagesProfile::compatible("OpenAI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        routes: &[],
        service_account: None,
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
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::AnthropicMessages],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Anthropic"),
        responses: ResponsesProfile::compatible("Anthropic"),
        messages: MessagesProfile::ANTHROPIC,
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("DeepSeek")
            .max_tokens()
            .system_role()
            .reasoning_fields(&["reasoning_content"])
            .tool_schema_root(ToolSchemaRoot::TypedObjectUnions)
            .tool_patterns(ToolPatterns::Compiled),
        responses: ResponsesProfile::compatible("DeepSeek"),
        messages: MessagesProfile::compatible("DeepSeek"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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
        images: &[ImagesEgress::openai(ImagesProfile::XAI)],
        speech: &[SpeechEgress::openai(SpeechProfile::XAI)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::XAI)],
        videos: &[VideosEgress::of(DialectId::XaiVideos)],
        dialects: &[DialectId::OpenAiResponses, DialectId::OpenAiChat],
        refused_key_messages: &["Incorrect API key provided"],
        context_window_messages: &[],
        reasoning_beyond_output_limit: true,
        end_user_limit: None,
        chat: ChatProfile::compatible("SpaceX")
            .reasoning_fields(&["reasoning_content"])
            .with_message_bookkeeping()
            .tool_schema_root(ToolSchemaRoot::ObjectUnions),
        responses: ResponsesProfile::compatible("SpaceX")
            .xai_web_search()
            .xai_input_items()
            .without_null_reasoning_content()
            .without_openai_encrypted_reasoning()
            .tool_schema_root(ToolSchemaRoot::ObjectUnions),
        messages: MessagesProfile::compatible("SpaceX"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        routes: &[],
        service_account: None,
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
        images: &[ImagesEgress::openai(ImagesProfile::TOGETHER)],
        speech: &[SpeechEgress::openai(SpeechProfile::TOGETHER)],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::TOGETHER,
        )],
        videos: &[VideosEgress {
            profile: VideosProfile::TOGETHER,
            ..VideosEgress::of(DialectId::OpenAiVideos)
        }],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Together AI")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        responses: ResponsesProfile::compatible("Together AI"),
        messages: MessagesProfile::compatible("Together AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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
        images: &[],
        speech: &[SpeechEgress::openai(SpeechProfile::GROQ)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::GROQ)],
        videos: &[],
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Groq")
            .system_role()
            .end_user_as_user()
            .without_cache_key()
            .reasoning_fields(&["reasoning"])
            .refusing(&["logit_bias", "logprobs", "prompt_cache_key", "top_logprobs"])
            .refusing_in_messages(&["name", "refusal"]),
        responses: ResponsesProfile::compatible("Groq"),
        messages: MessagesProfile::compatible("Groq"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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
        images: &[ImagesEgress::openai(ImagesProfile::OPENROUTER)],
        speech: &[SpeechEgress::openai(SpeechProfile::OPENROUTER)],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::OPENROUTER,
        )],
        videos: &[VideosEgress::of(DialectId::OpenRouterVideos)],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // OpenRouter's audio and video guides: Chat Completions takes "any format string the
        // provider supports" in `input_audio.format`, and videos as `video_url` parts.
        chat: ChatProfile::compatible("OpenRouter")
            .any_audio_format()
            .with_video_url()
            .reasoning_fields(&["reasoning"])
            .error_detail(&["error", "metadata", "raw"]),
        responses: ResponsesProfile::compatible("OpenRouter"),
        messages: MessagesProfile::compatible("OpenRouter"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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
        images: &[],
        speech: &[SpeechEgress::openai(SpeechProfile::MISTRAL)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::MISTRAL)],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &["too large for model with"],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Mistral")
            .max_tokens()
            .system_role()
            .random_seed()
            .without_end_user()
            .without_prefill()
            .greedy_top_p()
            .thinking_parts()
            .with_tool_parameters()
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
        responses: ResponsesProfile::compatible("Mistral"),
        messages: MessagesProfile::compatible("Mistral"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "vercel",
        name: "Vercel",
        key_page: KeyPage {
            url: "https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys",
            label: "AI Gateway → API Keys",
        },
        key_prefix: Some("vck_"),
        default_base_url: "https://ai-gateway.vercel.sh",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[ImagesEgress::openai(ImagesProfile::VERCEL)],
        speech: &[SpeechEgress::openai(SpeechProfile::VERCEL)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::VERCEL)],
        videos: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Vercel").reasoning_fields(&["reasoning"]),
        responses: ResponsesProfile::compatible("Vercel"),
        messages: MessagesProfile::compatible("Vercel"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "cohere",
        name: "Cohere",
        key_page: KeyPage {
            url: "https://dashboard.cohere.com/api-keys",
            label: "API Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.cohere.ai/compatibility",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[],
        speech: &[],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::OpenAiTranscriptions,
            profile: TranscriptionsProfile::COHERE,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..TranscriptionsEgress::ANY
        }],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Cohere")
            .stop_list()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "metadata",
                "modalities",
                "parallel_tool_calls",
                "prediction",
                "service_tier",
                "store",
                "top_logprobs",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("Cohere"),
        messages: MessagesProfile::compatible("Cohere"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "ollama-cloud",
        name: "Ollama Cloud",
        key_page: KeyPage {
            url: "https://ollama.com/settings/keys",
            label: "Settings → Keys",
        },
        key_prefix: None,
        default_base_url: "https://ollama.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Ollama Cloud")
            .max_tokens()
            .reasoning_fields(&["reasoning"])
            .refusing(&["logit_bias", "logprobs", "top_logprobs"]),
        responses: ResponsesProfile::compatible("Ollama Cloud"),
        messages: MessagesProfile::compatible("Ollama Cloud"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "moonshotai",
        name: "Moonshot",
        key_page: KeyPage {
            url: "https://platform.kimi.ai/console/api-keys",
            label: "API Keys",
        },
        key_prefix: Some("sk-"),
        default_base_url: "https://api.moonshot.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // Kimi's vision guide sends videos as `video_url` parts
        // (https://platform.moonshot.ai/docs/guide/use-kimi-vision-model).
        chat: ChatProfile::compatible("Moonshot")
            .with_video_url()
            .system_role()
            .partial_prefill()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "metadata",
                "modalities",
                "parallel_tool_calls",
                "service_tier",
                "store",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("Moonshot"),
        messages: MessagesProfile::compatible("Moonshot"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "alibaba",
        name: "QwenCloud",
        key_page: KeyPage {
            url: "https://home.qwencloud.com/api-keys",
            label: "API keys",
        },
        key_prefix: Some("sk-"),
        default_base_url: "https://ap-southeast-1.maas.qwencloudapi.com/compatible-mode",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        // QwenCloud's OpenAI-compatible image mode serves Qwen-Image 3.0 and 2.1 Pro only
        // (https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai);
        // its other image models answer on DashScope's own API.
        images: &[
            ImagesEgress::openai(ImagesProfile::QWENCLOUD)
                .for_models("qwen-image-3.0*")
                .fetching(DASHSCOPE_RESULTS),
            ImagesEgress::openai(ImagesProfile::QWENCLOUD)
                .for_models("qwen-image-2.1-pro*")
                .fetching(DASHSCOPE_RESULTS),
            ImagesEgress::dashscope().for_models("qwen-image-*"),
            ImagesEgress::dashscope().for_models("wan*"),
            // Z-Image costs twice as much per image when QwenCloud rewrites the prompt
            // (KPMR's note on `z-image-turbo`, from QwenCloud's pricing page).
            ImagesEgress::dashscope()
                .for_models("z-image*")
                .doubling_with_extended_prompt(),
            ImagesEgress::openai(ImagesProfile::QWENCLOUD).fetching(DASHSCOPE_RESULTS),
        ],
        speech: &[SpeechEgress::dashscope()],
        transcriptions: &[
            TranscriptionsEgress::dashscope().for_models("qwen-audio-*"),
            TranscriptionsEgress::dashscope().for_models("qwen3-asr-flash-filetrans*"),
            TranscriptionsEgress::dashscope().for_models("qwen3-asr-flash-realtime*"),
            TranscriptionsEgress {
                dialect: DialectId::OpenAiChat,
                ..TranscriptionsEgress::ANY
            },
        ],
        videos: &[VideosEgress::rooted(DialectId::DashScope).for_models("wan*")],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: true,
        end_user_limit: None,
        chat: ChatProfile::compatible("QwenCloud")
            .system_role()
            .thinking_switch()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "logit_bias",
                "metadata",
                "modalities",
                "prediction",
                "service_tier",
                "store",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("QwenCloud"),
        messages: MessagesProfile::compatible("QwenCloud"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "cerebras",
        name: "Cerebras",
        key_page: KeyPage {
            url: "https://cloud.cerebras.ai",
            label: "API Keys",
        },
        key_prefix: Some("csk-"),
        default_base_url: "https://api.cerebras.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Cerebras")
            .system_role()
            .leading_system_only()
            .end_user_as_user()
            .without_prefill()
            .with_tool_parameters()
            .reasoning_fields(&["reasoning"])
            .replay_reasoning_as_read()
            .refusing_in_messages(&["refusal"]),
        responses: ResponsesProfile::compatible("Cerebras"),
        messages: MessagesProfile::compatible("Cerebras"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "zai",
        name: "Z.ai",
        key_page: KeyPage {
            url: "https://z.ai/manage-apikey/apikey-list",
            label: "API Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.z.ai/api/anthropic",
        auth: AuthStyle::XApiKey,
        generation: DialectId::AnthropicMessages,
        count_tokens: None,
        embeddings: None,
        images: &[ImagesEgress {
            dialect: DialectId::OpenAiImages,
            profile: ImagesProfile::ZAI,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..ImagesEgress::ANY
        }],
        speech: &[],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::OpenAiTranscriptions,
            profile: TranscriptionsProfile::ZAI,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..TranscriptionsEgress::ANY
        }],
        videos: &[VideosEgress::rooted(DialectId::ZaiVideos)],
        dialects: &[DialectId::AnthropicMessages],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Z.ai"),
        responses: ResponsesProfile::compatible("Z.ai"),
        messages: MessagesProfile::compatible("Z.ai"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "minimax",
        name: "MiniMax",
        key_page: KeyPage {
            url: "https://platform.minimax.io/user-center/basic-information/interface-key",
            label: "API Keys",
        },
        key_prefix: Some("sk-api-"),
        default_base_url: "https://api.minimax.io/anthropic",
        auth: AuthStyle::XApiKey,
        generation: DialectId::AnthropicMessages,
        count_tokens: None,
        embeddings: None,
        images: &[ImagesEgress {
            dialect: DialectId::MinimaxImages,
            profile: ImagesProfile::OPENAI,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..ImagesEgress::ANY
        }],
        speech: &[SpeechEgress {
            dialect: DialectId::MinimaxSpeech,
            profile: SpeechProfile::OPENAI,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..SpeechEgress::ANY
        }],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::OpenAiTranscriptions,
            profile: TranscriptionsProfile::MINIMAX,
            from_root: true,
            auth: Some(AuthStyle::Bearer),
            ..TranscriptionsEgress::ANY
        }],
        videos: &[VideosEgress::rooted(DialectId::MinimaxVideos)],
        dialects: &[DialectId::AnthropicMessages],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("MiniMax"),
        responses: ResponsesProfile::compatible("MiniMax"),
        // The fields MiniMax's compatibility table lists as ignored, and its fixed sampling
        // (https://platform.minimax.io/docs/api-reference/text-anthropic-api).
        messages: MessagesProfile::compatible("MiniMax")
            .refusing(&[
                "top_k",
                "stop_sequences",
                "mcp_servers",
                "container",
                "context_management",
            ])
            .sampling_fixed_at(&[
                ("temperature", FixedValue::thousandths(1000)),
                ("top_p", FixedValue::thousandths(950)),
            ])
            .auto_tool_choice_only()
            .with_video(MINIMAX_VIDEO_MEDIA_TYPES),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "elevenlabs",
        name: "ElevenLabs",
        key_page: KeyPage {
            url: "https://elevenlabs.io/app/settings/api-keys",
            label: "API Keys",
        },
        key_prefix: Some("sk_"),
        default_base_url: "https://api.elevenlabs.io",
        auth: AuthStyle::XiApiKey,
        // ElevenLabs makes and transcribes audio; it writes no text, so a generation request
        // never reaches it past the catalogue's checks.
        generation: DialectId::ElevenLabs,
        count_tokens: None,
        embeddings: None,
        images: &[
            ImagesEgress::elevenlabs().for_models("gpt-image-*"),
            ImagesEgress::elevenlabs().for_models("gemini-*-image"),
            ImagesEgress::elevenlabs().for_models("bytedance-seedream-*"),
        ],
        speech: &[SpeechEgress {
            dialect: DialectId::ElevenLabs,
            profile: SpeechProfile::OPENAI,
            from_root: false,
            auth: None,
            ..SpeechEgress::ANY
        }],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::ElevenLabs,
            profile: TranscriptionsProfile::OPENAI,
            from_root: false,
            auth: None,
            ..TranscriptionsEgress::ANY
        }],
        videos: &[VideosEgress::of(DialectId::ElevenLabs)],
        dialects: &[DialectId::ElevenLabs],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("ElevenLabs"),
        responses: ResponsesProfile::compatible("ElevenLabs"),
        messages: MessagesProfile::compatible("ElevenLabs"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "google",
        name: "Google AI Studio",
        key_page: KeyPage {
            url: "https://aistudio.google.com/apikey",
            label: "API keys",
        },
        // AI Studio issues keys starting `AIza` and keys starting `AQ.`, so no one prefix fits.
        key_prefix: None,
        default_base_url: "https://generativelanguage.googleapis.com",
        auth: AuthStyle::XGoogApiKey,
        generation: DialectId::Gemini,
        count_tokens: Some(DialectId::Gemini),
        embeddings: Some(DialectId::GeminiEmbeddings),
        images: &[ImagesEgress::gemini()],
        speech: &[SpeechEgress::gemini()],
        transcriptions: &[TranscriptionsEgress::gemini()],
        videos: &[VideosEgress::of(DialectId::GeminiInteractions).for_models("gemini-omni-*")],
        dialects: &[DialectId::Gemini, DialectId::GeminiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        chat: ChatProfile::compatible("Google AI Studio"),
        responses: ResponsesProfile::compatible("Google AI Studio"),
        messages: MessagesProfile::compatible("Google AI Studio"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "gemini-agent-platform",
        name: "Gemini Enterprise Agent Platform",
        // Express mode API keys; a service account key is the other way in.
        key_page: KeyPage {
            url: "https://console.cloud.google.com/agent-platform/studio/settings/api-keys",
            label: "Settings → API keys",
        },
        key_prefix: None,
        default_base_url: "https://aiplatform.googleapis.com",
        auth: AuthStyle::XGoogApiKey,
        generation: DialectId::Gemini,
        count_tokens: Some(DialectId::Gemini),
        embeddings: None,
        images: &[ImagesEgress::gemini()],
        speech: &[SpeechEgress::gemini()],
        transcriptions: &[TranscriptionsEgress::gemini()],
        videos: &[
            VideosEgress::of(DialectId::Veo).for_models("veo-*"),
            VideosEgress::of(DialectId::GeminiInteractions).for_models("gemini-omni-*"),
        ],
        dialects: &[
            DialectId::Gemini,
            DialectId::AnthropicMessages,
            DialectId::OpenAiChat,
        ],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // The models-as-a-service endpoint ignores fields it doesn't list
        // (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/migrate/openai/overview).
        chat: ChatProfile::compatible("Gemini Enterprise Agent Platform")
            .max_tokens()
            .system_role()
            .without_end_user()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "logit_bias",
                "logprobs",
                "metadata",
                "parallel_tool_calls",
                "prediction",
                "prompt_cache_key",
                "service_tier",
                "store",
                "top_logprobs",
                "verbosity",
            ]),
        responses: ResponsesProfile::compatible("Gemini Enterprise Agent Platform"),
        // The Claude features Anthropic lists as unavailable on the platform
        // (https://platform.claude.com/docs/en/build-with-claude/claude-models-on-google-cloud/google-managed).
        messages: MessagesProfile::compatible("Gemini Enterprise Agent Platform")
            .refusing(&["mcp_servers", "container"])
            .raw_predict(),
        gemini: GeminiProfile::AGENT_PLATFORM,
        account: None,
        routes: &[
            ModelRoute {
                pattern: "claude-*",
                dialect: DialectId::AnthropicMessages,
                counts_tokens: true,
            },
            ModelRoute {
                pattern: "*/*",
                dialect: DialectId::OpenAiChat,
                counts_tokens: false,
            },
        ],
        service_account: Some(ServiceAccountSignIn {
            key_page: KeyPage {
                url: "https://console.cloud.google.com/iam-admin/serviceaccounts",
                label: "IAM & Admin → Service accounts",
            },
            locations: AGENT_PLATFORM_LOCATIONS,
            default_location: "global",
        }),
    },
    Provider {
        id: "fireworks-ai",
        name: "Fireworks AI",
        key_page: KeyPage {
            url: "https://app.fireworks.ai/settings/users/api-keys",
            label: "Settings → API Keys",
        },
        key_prefix: Some("fw_"),
        default_base_url: "https://api.fireworks.ai/inference",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // Fireworks' chat schema has no `safety_identifier`, only `user`, and takes `video_url`
        // parts for its video models (https://docs.fireworks.ai/guides/video-audio-inputs).
        chat: ChatProfile::compatible("Fireworks AI")
            .end_user_as_user()
            .with_video_url()
            .reasoning_fields(&["reasoning_content"]),
        responses: ResponsesProfile::compatible("Fireworks AI"),
        messages: MessagesProfile::compatible("Fireworks AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "nebius",
        name: "Nebius Token Factory",
        key_page: KeyPage {
            url: "https://tokenfactory.nebius.com/project/api-keys",
            label: "API keys",
        },
        key_prefix: None,
        default_base_url: "https://api.tokenfactory.nebius.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // Nebius' OpenAPI document lists no `developer` role and no `safety_identifier`, and
        // takes `video_url` parts (https://api.tokenfactory.nebius.com/openapi.json).
        chat: ChatProfile::compatible("Nebius Token Factory")
            .system_role()
            .end_user_as_user()
            .with_video_url()
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        responses: ResponsesProfile::compatible("Nebius Token Factory"),
        messages: MessagesProfile::compatible("Nebius Token Factory"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "sambanova",
        name: "SambaNova",
        key_page: KeyPage {
            url: "https://cloud.sambanova.ai/apis",
            label: "API Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.sambanova.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: Some(DialectId::AnthropicMessages),
        embeddings: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // SambaNova's request schema declares no end-user or cache-key field and refuses
        // undeclared ones; it accepts both penalties without acting on them
        // (https://docs.sambanova.ai/docs/en/features/openai-compatibility).
        chat: ChatProfile::compatible("SambaNova")
            .without_end_user()
            .without_cache_key()
            .with_video_url()
            .reasoning_fields(&["reasoning"])
            .refusing(&["frequency_penalty", "presence_penalty"]),
        responses: ResponsesProfile::compatible("SambaNova"),
        messages: MessagesProfile::compatible("SambaNova"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "scaleway",
        name: "Scaleway",
        key_page: KeyPage {
            url: "https://console.scaleway.com/iam/api-keys",
            label: "IAM → API keys",
        },
        key_prefix: None,
        default_base_url: "https://api.scaleway.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[],
        speech: &[],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::SCALEWAY,
        )],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // Scaleway's reference lists no `developer` role and no end-user field, and its reasoning
        // guide shows `reasoning_content` in replies and `reasoning` in stream deltas
        // (https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/).
        chat: ChatProfile::compatible("Scaleway")
            .system_role()
            .without_end_user()
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        responses: ResponsesProfile::compatible("Scaleway"),
        messages: MessagesProfile::compatible("Scaleway"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
    },
    Provider {
        id: "nvidia",
        name: "NVIDIA",
        key_page: KeyPage {
            url: "https://build.nvidia.com/settings/api-keys",
            label: "API Keys",
        },
        key_prefix: Some("nvapi-"),
        default_base_url: "https://integrate.api.nvidia.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        end_user_limit: None,
        // The hosted NIM API's request schemas have only `max_tokens`, no end-user field and the
        // roles `system`, `context`, `user` and `assistant`, and its samples read thinking from
        // `reasoning_content` (https://docs.api.nvidia.com/nim/reference/llm-apis).
        chat: ChatProfile::compatible("NVIDIA")
            .max_tokens()
            .system_role()
            .without_end_user()
            .reasoning_fields(&["reasoning_content"]),
        responses: ResponsesProfile::compatible("NVIDIA"),
        messages: MessagesProfile::compatible("NVIDIA"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
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

    pub fn exceeds_context(&self, message: &str) -> bool {
        self.context_window_messages
            .iter()
            .any(|overflow| message.contains(overflow))
    }

    pub fn route(&self, model: &str) -> Option<DialectId> {
        self.routes
            .iter()
            .find(|r| r.matches(model))
            .map(|r| r.dialect)
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
    use kaeso_codecs::openai_responses::{InputItems, ReasoningReplay, WebSearchTool};
    use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
    use kaeso_kpp::{Request, RequestKind};
    use std::path::{Path, PathBuf};

    // Every Responses request fixture, from `requests/` and each provider's `requests/`.
    fn responses_requests() -> Vec<(PathBuf, Request)> {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-codecs/fixtures/openai_responses");
        let mut dirs = vec![root.join("requests")];
        for provider in std::fs::read_dir(root.join("providers")).unwrap() {
            dirs.push(provider.unwrap().path().join("requests"));
        }
        let mut out = Vec::new();
        for dir in dirs.into_iter().filter(|d| d.is_dir()) {
            for file in std::fs::read_dir(dir).unwrap() {
                let file = file.unwrap().path();
                if file.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let body = std::fs::read(&file).unwrap();
                if let Ok(req) = dispatch::decode_request(
                    DialectId::OpenAiResponses,
                    RequestKind::Generate,
                    &body,
                    &HeaderList::new(),
                ) {
                    out.push((file, req));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn sent_to(p: &Provider, req: &Request) -> Option<serde_json::Value> {
        let mut ctx = EgressCtx::new(req)
            .with_provider(p.id)
            .with_responses_profile(p.responses);
        dispatch::check(DialectId::OpenAiResponses, req, &ctx).ok()?;
        let enc = dispatch::encode_request(DialectId::OpenAiResponses, req, &mut ctx).ok()?;
        serde_json::from_slice(&enc.body).ok()
    }

    #[test]
    fn only_openai_takes_codex_agent_messages_and_a_tool_choice_without_tools() {
        let as_sent: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.responses.input_items == InputItems::AsSent)
            .map(|p| p.id)
            .collect();
        assert_eq!(as_sent, ["openai"]);
        let bare: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.responses.tool_choice_without_tools)
            .map(|p| p.id)
            .collect();
        assert_eq!(bare, ["openai"]);
        let listed: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| matches!(p.responses.input_items, InputItems::Only(_)))
            .map(|p| p.id)
            .collect();
        assert_eq!(listed, ["xai"]);
    }

    // A provider that lists the input item types it takes gets every recorded request with only
    // those types, or Kaeso refuses it before sending: an item type Codex adds fails here the day
    // its fixture is recorded, and not at the provider. No provider but OpenAI ever gets Codex's
    // agent messages.
    #[test]
    fn every_recorded_responses_request_keeps_to_the_item_types_each_provider_takes() {
        let requests = responses_requests();
        assert!(requests.len() > 50, "{} fixtures", requests.len());
        for p in PROVIDERS.iter().filter(|p| p.id != "openai") {
            let mut sent = 0;
            for (file, req) in &requests {
                let Some(body) = sent_to(p, req) else {
                    continue;
                };
                sent += 1;
                assert!(
                    !body.to_string().contains("\"agent_message\""),
                    "{}: {}",
                    p.id,
                    file.display()
                );
                let InputItems::Only(allowed) = p.responses.input_items else {
                    continue;
                };
                for item in body["input"].as_array().into_iter().flatten() {
                    let ty = item["type"].as_str().unwrap_or("message");
                    assert!(
                        allowed.contains(&ty),
                        "{} would refuse a `{ty}` input item from {}",
                        p.id,
                        file.display()
                    );
                }
            }
            assert!(sent > 30, "{}: only {sent} requests encoded", p.id);
        }
    }

    #[test]
    fn only_openai_takes_tool_namespaces_and_only_spacex_changes_search_and_reasoning() {
        let namespaces: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.responses.tool_namespaces)
            .map(|p| p.id)
            .collect();
        assert_eq!(namespaces, ["openai"]);
        let different: Vec<(&str, WebSearchTool, ReasoningReplay)> = PROVIDERS
            .iter()
            .filter(|p| {
                p.responses.web_search != WebSearchTool::AsSent
                    || p.responses.reasoning_replay != ReasoningReplay::AsSent
            })
            .map(|p| (p.id, p.responses.web_search, p.responses.reasoning_replay))
            .collect();
        assert_eq!(
            different,
            [(
                "xai",
                WebSearchTool::Xai,
                ReasoningReplay::WithoutNullContent
            )]
        );
    }

    #[test]
    fn only_deepseek_compiles_tool_patterns() {
        let compiling: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.chat.request.tool_patterns != ToolPatterns::AsSent)
            .map(|p| p.id)
            .collect();
        assert_eq!(compiling, ["deepseek"]);
    }

    #[test]
    fn only_deepseek_and_spacex_take_less_than_any_tool_schema_root() {
        let merging: Vec<(&str, ToolSchemaRoot, ToolSchemaRoot)> = PROVIDERS
            .iter()
            .filter(|p| {
                p.chat.request.tool_schema_root != ToolSchemaRoot::AsSent
                    || p.responses.tool_schema_root != ToolSchemaRoot::AsSent
            })
            .map(|p| {
                (
                    p.id,
                    p.chat.request.tool_schema_root,
                    p.responses.tool_schema_root,
                )
            })
            .collect();
        assert_eq!(
            merging,
            [
                (
                    "deepseek",
                    ToolSchemaRoot::TypedObjectUnions,
                    ToolSchemaRoot::AsSent
                ),
                (
                    "xai",
                    ToolSchemaRoot::ObjectUnions,
                    ToolSchemaRoot::ObjectUnions
                )
            ]
        );
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
            "openai, anthropic, deepseek, xai, togetherai, groq, openrouter, mistral, vercel, cohere, ollama-cloud, moonshotai, alibaba, cerebras, zai, minimax, elevenlabs, google, gemini-agent-platform, fireworks-ai, nebius, sambanova, scaleway, nvidia"
        );
    }

    #[test]
    fn zai_takes_its_key_as_x_api_key_on_its_anthropic_endpoint_and_counts_no_tokens() {
        let zai = provider("zai").unwrap();
        assert_eq!(
            (zai.auth, zai.generation, zai.dialects),
            (
                AuthStyle::XApiKey,
                DialectId::AnthropicMessages,
                &[DialectId::AnthropicMessages][..]
            )
        );
        assert_eq!(zai.default_base_url, "https://api.z.ai/api/anthropic");
        assert_eq!((zai.count_tokens, zai.embeddings), (None, None));
        assert_eq!(zai.key_prefix, None);
    }

    #[test]
    fn google_takes_its_key_as_x_goog_api_key_and_speaks_gemini_for_generation_counts_and_embeddings()
     {
        let google = provider("google").unwrap();
        assert_eq!(
            (google.name, google.auth, google.generation, google.dialects),
            (
                "Google AI Studio",
                AuthStyle::XGoogApiKey,
                DialectId::Gemini,
                &[DialectId::Gemini, DialectId::GeminiEmbeddings][..]
            )
        );
        assert_eq!(
            (google.count_tokens, google.embeddings),
            (Some(DialectId::Gemini), Some(DialectId::GeminiEmbeddings))
        );
        assert_eq!(
            google.default_base_url,
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(google.key_page.url, "https://aistudio.google.com/apikey");
        assert_eq!(google.key_prefix, None);
        assert_eq!(google.gemini, GeminiProfile::GOOGLE_AI_STUDIO);
        for key in ["AIzaSyA0bCdEf1234567", "AQ.Xq0TestAbCdEf1234"] {
            let redacted = crate::secret::redact_tokens(&format!("API key {key} not valid."));
            assert!(!redacted.contains(&key[4..]), "{redacted}");
        }
    }

    #[test]
    fn each_auth_style_names_its_header_and_carries_the_key() {
        assert_eq!(
            [
                AuthStyle::Bearer,
                AuthStyle::XApiKey,
                AuthStyle::XGoogApiKey
            ]
            .map(|s| (s.header_name(), s.header_value("k-1"))),
            [
                ("authorization", "Bearer k-1".to_owned()),
                ("x-api-key", "k-1".to_owned()),
                ("x-goog-api-key", "k-1".to_owned()),
            ]
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
                "requests/identifiers-metadata-store",
                "prompt_cache_key",
            ),
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
        let greedy =
            sent_with_profile("mistral", "providers/mistral/requests/greedy-sampling").unwrap();
        assert_eq!(greedy["top_p"], 1.0, "{greedy}");
    }

    #[test]
    fn ollama_cloud_gets_every_apps_output_limit_as_the_max_tokens_it_honours() {
        let recorded = sent_with_profile(
            "ollama-cloud",
            "providers/ollama-cloud/requests/max-completion-tokens-ignored",
        )
        .unwrap();
        assert_eq!(recorded["max_tokens"], 20, "{recorded}");
        assert!(
            recorded.get("max_completion_tokens").is_none(),
            "{recorded}"
        );

        let mut g = kaeso_kpp::GenerateRequest::new("gpt-oss:20b");
        g.extras.origin = Some(kaeso_kpp::Origin::new(DialectId::AnthropicMessages));
        g.messages = vec![kaeso_kpp::Message::user("Count to forty.")];
        g.sampling.max_output_tokens = Some(60);
        let req = kaeso_kpp::Request::Generate(g);
        let mut ctx = kaeso_codecs::EgressCtx::new(&req)
            .with_chat_profile(provider("ollama-cloud").unwrap().chat);
        let out =
            kaeso_codecs::dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx).unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(sent["max_tokens"], 60, "{sent}");
        assert!(sent.get("max_completion_tokens").is_none(), "{sent}");
    }

    #[test]
    fn ollama_cloud_refuses_the_fields_ollama_ignores_and_keeps_the_rest() {
        for (request, param) in [
            ("requests/logit-bias", "logit_bias"),
            ("requests/logprobs-top-logprobs-doc", "logprobs"),
        ] {
            assert_eq!(
                sent_with_profile("ollama-cloud", request).unwrap_err(),
                param,
                "{request}"
            );
        }
        let developer =
            sent_with_profile("ollama-cloud", "requests/plain-developer-string").unwrap();
        assert_eq!(developer["messages"][0]["role"], "developer");
        let named = sent_with_profile("ollama-cloud", "requests/system-name-multiturn").unwrap();
        assert!(named["messages"][0].get("name").is_some(), "{named}");
        let seed =
            sent_with_profile("ollama-cloud", "requests/stop-string-seed-penalties").unwrap();
        assert!(seed.get("seed").is_some(), "{seed}");
        let reasoning = sent_with_profile(
            "ollama-cloud",
            "providers/ollama-cloud/requests/reasoning-sent-back",
        )
        .unwrap();
        assert_eq!(
            reasoning["messages"][1]["reasoning"],
            "The user wants one number; 7 is a common pick."
        );
    }

    #[test]
    fn moonshot_ai_gets_system_messages_and_a_prefill_marked_partial() {
        let refused =
            sent_with_profile("openai", "providers/moonshotai/requests/developer-role").unwrap();
        assert_eq!(refused["messages"][0]["role"], "developer");
        let sent = sent_with_profile("moonshotai", "providers/moonshotai/requests/developer-role")
            .unwrap();
        assert_eq!(sent["messages"][0]["role"], "system");

        let mut g = kaeso_kpp::GenerateRequest::new("kimi-k2.6");
        g.extras.origin = Some(kaeso_kpp::Origin::new(DialectId::AnthropicMessages));
        g.messages = vec![
            kaeso_kpp::Message::user("Name three colours as a numbered list."),
            kaeso_kpp::Message::new(
                kaeso_kpp::Role::Assistant,
                vec![kaeso_kpp::Block::text("1. Red\n2.")],
            ),
        ];
        let req = kaeso_kpp::Request::Generate(g);
        let mut ctx = kaeso_codecs::EgressCtx::new(&req)
            .with_chat_profile(provider("moonshotai").unwrap().chat);
        let out =
            kaeso_codecs::dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx).unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&out.body).unwrap();
        let recorded: serde_json::Value = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/../kaeso-codecs/fixtures/openai_chat/providers/moonshotai/requests/partial.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(sent["messages"][1], recorded["messages"][1]);
    }

    #[test]
    fn moonshot_ai_refuses_the_fields_moonshot_ignores_and_keeps_the_rest() {
        assert_eq!(
            sent_with_profile(
                "moonshotai",
                "providers/moonshotai/requests/parallel-tool-calls-false"
            )
            .unwrap_err(),
            "parallel_tool_calls"
        );
        assert_eq!(
            sent_with_profile("moonshotai", "requests/identifiers-metadata-store").unwrap_err(),
            "metadata"
        );
        for kept in [
            "providers/moonshotai/requests/logprobs",
            "providers/moonshotai/requests/system-name-stop",
            "providers/moonshotai/requests/json-object-system",
            "providers/moonshotai/requests/partial",
            "providers/moonshotai/requests/thinking-disabled",
        ] {
            let sent = sent_with_profile("moonshotai", kept).unwrap();
            assert!(sent.get("parallel_tool_calls").is_none(), "{kept}");
        }
        let sent = sent_with_profile(
            "moonshotai",
            "providers/moonshotai/requests/thinking-disabled",
        )
        .unwrap();
        assert_eq!(sent["thinking"]["type"], "disabled");
        assert_eq!(sent["max_completion_tokens"], 20);
    }

    #[test]
    fn alibaba_refuses_the_fields_model_studio_ignores_and_keeps_the_rest() {
        for (request, param) in [
            ("requests/logit-bias", "logit_bias"),
            ("providers/alibaba/requests/store-metadata-fields", "store"),
            ("providers/alibaba/requests/prediction-field", "prediction"),
            ("providers/alibaba/requests/audio-fields", "audio"),
            (
                "providers/alibaba/requests/web-search-options-field",
                "web_search_options",
            ),
        ] {
            assert_eq!(
                sent_with_profile("alibaba", request).unwrap_err(),
                param,
                "{request}"
            );
        }
        let developer = sent_with_profile("alibaba", "requests/plain-developer-string").unwrap();
        assert_eq!(developer["messages"][0]["role"], "system");
        for request in [
            "providers/alibaba/requests/sampling",
            "providers/alibaba/requests/logprobs",
            "providers/alibaba/requests/structured-output",
            "providers/alibaba/requests/tools-required",
            "providers/alibaba/requests/functions-field",
            "providers/alibaba/requests/assistant-prefill",
            "providers/alibaba/requests/user-hints",
        ] {
            sent_with_profile("alibaba", request).unwrap_or_else(|p| panic!("{request}: {p}"));
        }
        let reasoning =
            sent_with_profile("alibaba", "providers/alibaba/requests/reasoning-sent-back").unwrap();
        assert_eq!(
            reasoning["messages"][1]["reasoning_content"],
            "The user wants one number; 7 is a common pick."
        );
    }

    #[test]
    fn alibaba_gets_an_anthropic_apps_thinking_as_enable_thinking_and_its_budget() {
        let alibaba = provider("alibaba").unwrap();
        assert!(alibaba.reasoning_beyond_output_limit);
        let send = |thinking: kaeso_kpp::ReasoningConfig| {
            let mut g = kaeso_kpp::GenerateRequest::new("qwen3.7-flash");
            g.extras.origin = Some(kaeso_kpp::Origin::new(DialectId::AnthropicMessages));
            g.messages = vec![kaeso_kpp::Message::user("What is 17 times 23?")];
            g.sampling.max_output_tokens = Some(4096);
            g.reasoning = Some(thinking);
            let req = kaeso_kpp::Request::Generate(g);
            let mut ctx = kaeso_codecs::EgressCtx::new(&req).with_chat_profile(alibaba.chat);
            let out = kaeso_codecs::dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx)
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(&out.body).unwrap()
        };
        let mut on = kaeso_kpp::ReasoningConfig::default();
        on.mode = Some(kaeso_kpp::ReasoningMode::Enabled);
        on.budget_tokens = Some(2048);
        let sent = send(on);
        assert_eq!(sent["enable_thinking"], true, "{sent}");
        assert_eq!(sent["thinking_budget"], 2048, "{sent}");
        assert_eq!(sent["max_completion_tokens"], 4096, "{sent}");
        assert!(sent.get("reasoning_effort").is_none(), "{sent}");
        let mut off = kaeso_kpp::ReasoningConfig::default();
        off.mode = Some(kaeso_kpp::ReasoningMode::Disabled);
        let sent = send(off);
        assert_eq!(sent["enable_thinking"], false, "{sent}");
        assert!(sent.get("thinking_budget").is_none(), "{sent}");
    }

    #[test]
    fn cerebras_gets_each_request_it_was_recorded_refusing_in_a_form_it_takes() {
        let cerebras = |request: &str| {
            sent_with_profile(
                "cerebras",
                &format!("providers/cerebras/requests/{request}"),
            )
        };
        for (request, param) in [
            ("prefill", "messages[1]"),
            ("assistant-refusal", "messages[1].refusal"),
        ] {
            assert_eq!(cerebras(request).unwrap_err(), param, "{request}");
        }
        let developer = cerebras("developer-role-qwen").unwrap();
        assert_eq!(developer["messages"][0]["role"], "system");
        let end_user = cerebras("safety-identifier-field").unwrap();
        assert!(end_user.get("safety_identifier").is_none(), "{end_user}");
        assert_eq!(end_user["user"], "8f14e45fceea167a5a36dedd4bea2543");
        let tool = cerebras("tool-without-parameters").unwrap();
        assert_eq!(
            tool["tools"][0]["function"]["parameters"],
            serde_json::json!({"type": "object", "properties": {}})
        );
        let replayed = cerebras("reasoning-content-sent-back").unwrap();
        assert_eq!(
            replayed["messages"][1]["reasoning"],
            "The user wants one number; 7 is a common pick."
        );
        assert!(replayed["messages"][1].get("reasoning_content").is_none());
        for kept in [
            "basic",
            "logprobs",
            "sampling",
            "user-hints",
            "logit-bias-honoured",
            "structured-output",
            "image",
            "reasoning-sent-back",
            "system-mid-conversation",
        ] {
            cerebras(kept).unwrap_or_else(|p| panic!("{kept}: {p}"));
        }
    }

    #[test]
    fn only_mistral_reads_its_context_window_message_as_context_too_long() {
        let message =
            "Prompt contains 65673 tokens, too large for model with 32768 maximum context length";
        for p in PROVIDERS {
            assert_eq!(p.exceeds_context(message), p.id == "mistral", "{}", p.id);
        }
        assert!(
            !provider("mistral")
                .unwrap()
                .exceeds_context("Invalid model: m")
        );
    }

    #[test]
    fn only_spacex_takes_the_model_ids_grok_cli_replays() {
        for p in PROVIDERS
            .iter()
            .filter(|p| p.dialects.contains(&DialectId::OpenAiChat))
        {
            let sent = sent_with_profile(p.id, "requests/grok-cli-model-id").unwrap();
            let kept = sent["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m.get("model_id").is_some());
            assert_eq!(kept, p.id == "xai", "{}", p.id);
        }
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
    fn every_provider_names_itself_in_its_messages_profile() {
        for p in PROVIDERS {
            assert_eq!(p.messages.provider, p.name, "{}", p.id);
        }
        assert_eq!(
            provider("anthropic").unwrap().messages,
            MessagesProfile::ANTHROPIC
        );
    }

    fn sent_to_minimax(request: &str) -> Result<(serde_json::Value, Vec<String>), String> {
        let path = format!(
            "{}/../kaeso-codecs/fixtures/anthropic_messages/{request}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let req = dispatch::decode_request(
            DialectId::AnthropicMessages,
            RequestKind::Generate,
            &std::fs::read(&path).unwrap(),
            &HeaderList::new(),
        )
        .unwrap();
        let mut ctx = EgressCtx::new(&req)
            .with_provider("minimax")
            .with_messages_profile(provider("minimax").unwrap().messages);
        dispatch::encode_request(DialectId::AnthropicMessages, &req, &mut ctx)
            .map(|out| (serde_json::from_slice(&out.body).unwrap(), ctx.left_out))
            .map_err(|e| e.param.unwrap_or_default())
    }

    #[test]
    fn minimax_refuses_what_its_compatibility_table_lists_as_ignored_and_takes_its_own_examples() {
        for (request, param) in [
            ("requests/sampling-stop-sequences", "temperature"),
            ("requests/sampling-top-p", "top_p"),
            ("requests/tool-choice-any-no-parallel", "tool_choice"),
            (
                "requests/context-management-clear-tool-uses",
                "context_management",
            ),
        ] {
            assert_eq!(sent_to_minimax(request).unwrap_err(), param, "{request}");
        }
        let (body, left_out) =
            sent_to_minimax("requests/claude-code-mid-conversation-system").unwrap();
        assert!(body.get("context_management").is_none(), "{body}");
        assert_eq!(left_out, ["context_management"]);
        for documented in [
            "deep-thinking",
            "tool-use-auto",
            "image-url-adaptive",
            "image-url-stream",
            "video-url",
        ] {
            let request = format!("providers/minimax/requests/{documented}");
            let (body, left_out) =
                sent_to_minimax(&request).unwrap_or_else(|p| panic!("{documented}: {p}"));
            assert!(left_out.is_empty(), "{documented}");
            assert_eq!(body["output_config"]["effort"], "max", "{documented}");
        }
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
    fn gemini_enterprise_agent_platform_takes_express_keys_and_service_account_keys() {
        let p = provider("gemini-agent-platform").unwrap();
        assert_eq!(p.name, "Gemini Enterprise Agent Platform");
        assert_eq!(
            (p.auth, p.generation, p.count_tokens, p.embeddings),
            (
                AuthStyle::XGoogApiKey,
                DialectId::Gemini,
                Some(DialectId::Gemini),
                None
            )
        );
        assert_eq!(
            p.route("claude-opus-5-5@default"),
            Some(DialectId::AnthropicMessages)
        );
        assert_eq!(p.route("xai/grok-4.7"), Some(DialectId::OpenAiChat));
        assert_eq!(p.route("gemini-3.5-flash"), None);
        assert!(p.messages.raw_predict);
        assert!(p.routes.iter().all(|r| p.dialects.contains(&r.dialect)));
        assert_eq!(p.default_base_url, "https://aiplatform.googleapis.com");
        assert!(p.gemini.role_on_every_content);
        let sa = p.service_account.unwrap();
        assert_eq!(sa.default_location, "global");
        assert!(sa.locations.iter().all(|l| crate::google::is_location(l)));
        for l in [
            "global",
            "us",
            "eu",
            "us-east5",
            "europe-west1",
            "asia-southeast1",
        ] {
            assert!(sa.locations.contains(&l), "{l}");
        }
        assert!(
            PROVIDERS
                .iter()
                .filter(|p| p.id != "gemini-agent-platform")
                .all(|p| p.service_account.is_none())
        );
    }

    #[test]
    fn route_patterns_match_like_globs() {
        let r = |pattern| ModelRoute {
            pattern,
            dialect: DialectId::AnthropicMessages,
            counts_tokens: true,
        };
        assert!(r("claude-*").matches("claude-opus-5-5@default"));
        assert!(!r("claude-*").matches("xclaude-opus"));
        assert!(r("*/*").matches("meta/llama-4-maverick-17b-128e-instruct-maas"));
        assert!(!r("*/*").matches("gemini-3.5-flash"));
        assert!(r("gemini-embedding-*").matches("gemini-embedding-2"));
        assert!(r("exact").matches("exact"));
        assert!(!r("exact").matches("exactly"));
        assert!(r("a*b*c").matches("aXbYc"));
        assert!(!r("a*b*c").matches("aXcYb"));
    }

    #[test]
    fn qwencloud_sends_each_media_model_to_the_api_that_serves_it() {
        let qwen = provider("alibaba").unwrap();
        let images = |m: &str| qwen.images.iter().find(|e| e.serves(m)).unwrap();
        for m in [
            "qwen-image-3.0-pro",
            "qwen-image-3.0",
            "qwen-image-2.1-pro",
            "flux",
        ] {
            assert_eq!(images(m).dialect, DialectId::OpenAiImages, "{m}");
            assert!(!images(m).from_root);
        }
        for m in [
            "qwen-image-2.1-turbo",
            "qwen-image-2.0-pro-2026-06-22",
            "wan2.7-image-pro",
            "wan2.6-image",
            "z-image-turbo",
        ] {
            assert_eq!(images(m).dialect, DialectId::DashScope, "{m}");
            assert!(images(m).from_root && !images(m).fetch_hosts.is_empty());
        }
        assert!(
            qwen.speech
                .iter()
                .all(|e| e.dialect == DialectId::DashScope)
        );
        let asr = |m: &str| {
            qwen.transcriptions
                .iter()
                .find(|e| e.serves(m))
                .unwrap()
                .dialect
        };
        assert_eq!(asr("qwen3-asr-flash"), DialectId::OpenAiChat);
        assert_eq!(asr("qwen3-asr-flash-2025-09-08"), DialectId::OpenAiChat);
        assert_eq!(asr("qwen3-asr-flash-filetrans"), DialectId::DashScope);
        assert_eq!(asr("qwen-audio-3.1-asr-flash"), DialectId::DashScope);
        assert!(fetchable(
            DASHSCOPE_RESULTS,
            "Dashscope-Result-SGP.oss-ap-southeast-1.aliyuncs.com"
        ));
        assert!(!fetchable(
            DASHSCOPE_RESULTS,
            "dashscope-result.aliyuncs.com.example.com"
        ));
        assert!(!fetchable(&[], "dashscope-result.aliyuncs.com"));
        let openai = provider("openai").unwrap();
        assert!(openai.images.iter().all(|e| e.fetch_hosts.is_empty()));
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
                "mistral",
                "vercel",
                "cohere",
                "ollama-cloud",
                "moonshotai",
                "alibaba",
                "cerebras",
                "zai",
                "minimax",
                "elevenlabs",
                "google",
                "gemini-agent-platform",
                "fireworks-ai",
                "nebius",
                "sambanova",
                "scaleway",
                "nvidia"
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
