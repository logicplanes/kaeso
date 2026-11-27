use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use kaeso_codecs::anthropic_messages::{FixedValue, MINIMAX_VIDEO_MEDIA_TYPES, MessagesProfile};
use kaeso_codecs::gemini::GeminiProfile;
use kaeso_codecs::openai_chat::{ChatProfile, OptionDefault};
use kaeso_codecs::openai_decisions::DecisionsProfile;
use kaeso_codecs::openai_embeddings::EmbeddingsProfile;
use kaeso_codecs::openai_images::ImagesProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_codecs::openai_speech::SpeechProfile;
use kaeso_codecs::openai_transcriptions::TranscriptionsProfile;
use kaeso_codecs::openai_videos::VideosProfile;
use kaeso_codecs::system_one::SystemOneProfile;
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
    /// Azure's `api-key` header.
    ApiKey,
    /// fal's `Authorization: Key …`; fal reads `Bearer` as one of its own short-lived tokens
    /// (https://fal.ai/docs/documentation/model-apis/authentication/key-based).
    Key,
    /// `Authorization: Token <key>`, as Deepgram takes its API keys
    /// (https://developers.deepgram.com/reference/authentication).
    Token,
}

impl AuthStyle {
    pub const fn header_name(self) -> &'static str {
        match self {
            AuthStyle::Bearer | AuthStyle::Key | AuthStyle::Token => "authorization",
            AuthStyle::XApiKey => "x-api-key",
            AuthStyle::XGoogApiKey => "x-goog-api-key",
            AuthStyle::XiApiKey => "xi-api-key",
            AuthStyle::ApiKey => "api-key",
        }
    }

    pub fn header_value(self, key: &str) -> String {
        match self {
            AuthStyle::Bearer => format!("Bearer {key}"),
            AuthStyle::XApiKey
            | AuthStyle::XGoogApiKey
            | AuthStyle::XiApiKey
            | AuthStyle::ApiKey => key.to_owned(),
            AuthStyle::Key => format!("Key {key}"),
            AuthStyle::Token => format!("Token {key}"),
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
    /// An AWS access key (id, secret and optional session token) that signs requests with
    /// Signature Version 4.
    AwsAccessKey,
    /// The gateway's own AWS identity, optionally assuming a role, that signs requests with
    /// Signature Version 4.
    AwsRole,
    /// A Microsoft Entra ID service principal (tenant, client id and client secret) that Kaeso
    /// exchanges for access tokens.
    AzureServicePrincipal,
}

impl AuthMethod {
    pub const ALL: [AuthMethod; 7] = [
        AuthMethod::ApiKey,
        AuthMethod::ChatGpt,
        AuthMethod::Grok,
        AuthMethod::GoogleServiceAccount,
        AuthMethod::AwsAccessKey,
        AuthMethod::AwsRole,
        AuthMethod::AzureServicePrincipal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AuthMethod::ApiKey => "api_key",
            AuthMethod::ChatGpt => "chatgpt",
            AuthMethod::Grok => "grok",
            AuthMethod::GoogleServiceAccount => "google_service_account",
            AuthMethod::AwsAccessKey => "aws_access_key",
            AuthMethod::AwsRole => "aws_role",
            AuthMethod::AzureServicePrincipal => "azure_service_principal",
        }
    }

    pub fn parse(s: &str) -> Option<AuthMethod> {
        AuthMethod::ALL.into_iter().find(|m| m.as_str() == s)
    }

    pub const fn is_account(self) -> bool {
        matches!(self, AuthMethod::ChatGpt | AuthMethod::Grok)
    }

    pub const fn is_secret_key(self) -> bool {
        matches!(
            self,
            AuthMethod::ApiKey
                | AuthMethod::GoogleServiceAccount
                | AuthMethod::AwsAccessKey
                | AuthMethod::AzureServicePrincipal
        )
    }

    /// Whether requests are signed with AWS Signature Version 4.
    pub const fn is_aws(self) -> bool {
        matches!(self, AuthMethod::AwsAccessKey | AuthMethod::AwsRole)
    }
}

// The hosts QwenCloud's result links point at: DashScope's OSS buckets, such as
// `dashscope-result-sgp.oss-ap-southeast-1.aliyuncs.com` in its image, speech and video references.
const DASHSCOPE_RESULTS: &[&str] = &["dashscope-*.aliyuncs.com"];
const REPLICATE_OUTPUTS: &[&str] = &["replicate.delivery", "*.replicate.delivery"];

/// Where and how Kaeso sends rerank requests to a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RerankEgress {
    pub dialect: DialectId,
    /// The dialect's path starts at the root of the host, not under the base URL's path, as
    /// Cohere serves its Rerank API beside the OpenAI-compatible path Kaeso uses.
    pub from_root: bool,
}

impl RerankEgress {
    pub const fn of(dialect: DialectId) -> RerankEgress {
        RerankEgress {
            dialect,
            from_root: false,
        }
    }

    const fn at_root(self) -> RerankEgress {
        RerankEgress {
            from_root: true,
            ..self
        }
    }
}

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

/// Where and how Kaeso sends decision requests to a provider, for the models `models` matches.
/// The profile of the egress dialect is the one that applies; the other stays at its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionsEgress {
    /// `openai_decisions`, `system_one` or `gemini_decisions`.
    pub dialect: DialectId,
    /// How a provider of TypeSafe's System One format takes decision requests.
    pub system_one: SystemOneProfile,
    /// How a provider of OpenAI's Decisions format takes decision requests.
    pub openai: DecisionsProfile,
    /// The dialect's paths start at the root of the host, as for [`ImagesEgress::from_root`].
    pub from_root: bool,
    /// The auth header the decisions API takes when it differs from the provider's.
    pub auth: Option<AuthStyle>,
    /// The model ids this API serves, as for [`ImagesEgress::models`].
    pub models: &'static str,
}

impl DecisionsEgress {
    const ANY: DecisionsEgress = DecisionsEgress {
        dialect: DialectId::OpenAiDecisions,
        system_one: SystemOneProfile::GENERIC,
        openai: DecisionsProfile::GENERIC,
        from_root: false,
        auth: None,
        models: "*",
    };

    /// OpenAI's Decisions API in the form `profile` describes.
    pub const fn openai(profile: DecisionsProfile) -> DecisionsEgress {
        DecisionsEgress {
            openai: profile,
            ..DecisionsEgress::ANY
        }
    }

    /// TypeSafe's System One API in the form `profile` describes.
    pub const fn system_one(profile: SystemOneProfile) -> DecisionsEgress {
        DecisionsEgress {
            dialect: DialectId::SystemOne,
            system_one: profile,
            ..DecisionsEgress::ANY
        }
    }

    /// Google's Gemini Decisions API, which only Google AI Studio serves.
    pub const fn gemini() -> DecisionsEgress {
        DecisionsEgress {
            dialect: DialectId::GeminiDecisions,
            ..DecisionsEgress::ANY
        }
    }

    pub const fn for_models(self, models: &'static str) -> DecisionsEgress {
        DecisionsEgress { models, ..self }
    }

    pub fn serves(&self, model: &str) -> bool {
        glob(self.models, model)
    }

    /// The most questions one request may ask, as the profile of the egress dialect says.
    pub fn max_questions(&self) -> Option<usize> {
        match self.dialect {
            DialectId::SystemOne => self.system_one.max_questions,
            DialectId::OpenAiDecisions => self.openai.max_questions,
            _ => None,
        }
    }

    /// Whether the provider answers box questions.
    pub fn box_questions(&self) -> bool {
        self.dialect == DialectId::SystemOne && self.system_one.box_questions
    }
}

/// An API a provider serves its generation models through besides its `generation` one, such as
/// DeepSeek's Anthropic-compatible Messages API at `/anthropic/v1/messages`. A request that
/// arrives in this dialect is sent to it as it came, instead of being translated into the
/// provider's `generation` dialect, while the model's catalogue entry names no egress dialect and
/// the provider's base URL is its default. A model whose entry names this dialect is sent here
/// whatever the app speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeDialect {
    pub dialect: DialectId,
    /// What takes the place of `/v1` in the dialect's paths, when it isn't the provider's own
    /// [`Provider::api_root`].
    pub api_root: Option<&'static str>,
    /// The auth header the dialect takes, when it isn't the provider's own.
    pub auth: Option<AuthStyle>,
    /// The models served in this dialect, as patterns where `*` stands for any run of
    /// characters; empty for every generation model of the provider.
    pub models: &'static [&'static str],
    /// Whether the dialect's token count (`/v1/messages/count_tokens`) answers there.
    pub counts_tokens: bool,
    /// Whether `api_root` starts at the root of the base URL's host instead of under its path,
    /// as QwenCloud's `/apps/anthropic/v1` does beside its `/compatible-mode/v1`.
    pub from_root: bool,
}

impl NativeDialect {
    pub const fn of(dialect: DialectId) -> NativeDialect {
        NativeDialect {
            dialect,
            api_root: None,
            auth: None,
            models: &[],
            counts_tokens: false,
            from_root: false,
        }
    }

    pub const fn from_host_root(mut self, api_root: &'static str) -> NativeDialect {
        self.api_root = Some(api_root);
        self.from_root = true;
        self
    }

    pub const fn at(mut self, api_root: &'static str) -> NativeDialect {
        self.api_root = Some(api_root);
        self
    }

    pub const fn with_auth(mut self, auth: AuthStyle) -> NativeDialect {
        self.auth = Some(auth);
        self
    }

    pub const fn for_models(mut self, models: &'static [&'static str]) -> NativeDialect {
        self.models = models;
        self
    }

    pub const fn counting_tokens(mut self) -> NativeDialect {
        self.counts_tokens = true;
        self
    }

    pub fn serves(&self, model: &str) -> bool {
        self.models.is_empty() || self.models.iter().any(|m| glob(m, model))
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

/// A provider whose API path names the account the key belongs to, such as Cloudflare's
/// `/client/v4/accounts/{account_id}/ai/v1`. Each key carries its account id and, optionally, the
/// gateway its requests go through, sent in a header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountScope {
    /// Where the console points for the account id.
    pub account_page: KeyPage,
    /// The API's path under the base URL, `{account}` standing for the account id; it replaces
    /// the `/v1` of each dialect's paths.
    pub path: &'static str,
    /// The header that names the gateway, sent when a key names one.
    pub gateway_header: &'static str,
}

impl AccountScope {
    /// The account's API path under the base URL.
    pub fn base_path(&self, account: &str) -> String {
        self.path.replace("{account}", account)
    }

    /// Cloudflare account ids are 32 lowercase hexadecimal characters.
    pub fn is_account_id(id: &str) -> bool {
        id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }

    /// A gateway id: 1 to 64 letters, digits, hyphens and underscores, starting with a letter or
    /// digit, which fits in a header and a path segment unchanged.
    pub fn is_gateway_id(id: &str) -> bool {
        (1..=64).contains(&id.len())
            && id.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    }
}

/// A provider whose keys each call one region, every region on a host of its own, such as
/// Amazon Bedrock's `bedrock-runtime.{region}.amazonaws.com`. Each key carries its region; a
/// model whose catalogue entry names no egress dialect is served there only through the
/// provider's own API, which Kaeso doesn't speak, and is refused before sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regional {
    /// Where the console points for the regions.
    pub regions_page: KeyPage,
    pub regions: &'static [&'static str],
    /// The host of a region, `{region}` standing for it.
    pub host: &'static str,
    /// What takes the place of `/v1` on the host, for each dialect Kaeso sends there.
    pub api_roots: &'static [(DialectId, &'static str)],
    /// The auth header each dialect takes when it differs from the provider's.
    pub auth: &'static [(DialectId, AuthStyle)],
}

impl Regional {
    pub fn serves(&self, region: &str) -> bool {
        self.regions.contains(&region)
    }

    pub fn base_url(&self, region: &str) -> String {
        format!("https://{}", self.host.replace("{region}", region))
    }

    pub fn api_root(&self, dialect: DialectId) -> Option<&'static str> {
        self.api_roots
            .iter()
            .find(|(d, _)| *d == dialect)
            .map(|(_, root)| *root)
    }

    pub fn auth(&self, dialect: DialectId) -> Option<AuthStyle> {
        self.auth
            .iter()
            .find(|(d, _)| *d == dialect)
            .map(|(_, auth)| *auth)
    }
}

/// A provider whose keys each belong to one resource with an address of its own, such as an
/// Azure resource's `https://{resource}.openai.azure.com`. Each key carries that endpoint, which
/// takes the place of the provider's base URL for the key's requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEndpoint {
    /// Where the console points for the endpoint.
    pub page: KeyPage,
    /// The host suffixes an endpoint may end in, each with its leading dot.
    pub hosts: &'static [&'static str],
    /// The path under the endpoint that the dialects' `/v1/…` paths go under, such as Azure's
    /// `/openai`, so that `/v1/chat/completions` is sent to `/openai/v1/chat/completions`.
    pub path: &'static str,
    /// The paths an endpoint may be pasted with, longest first; Kaeso keeps the endpoint without
    /// them.
    pub pasted_paths: &'static [&'static str],
    /// An endpoint the console shows as an example.
    pub example: &'static str,
}

/// Why an endpoint isn't one a [`KeyEndpoint`] takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointProblem {
    /// It isn't a URL, or is longer than 2048 characters.
    NotUrl,
    /// It isn't `https`.
    NotHttps,
    /// It holds a user name or password.
    UserInfo,
    /// It has a query or a fragment.
    Query,
    /// Its host isn't one of the provider's.
    Host,
    /// It has a path other than the ones Kaeso drops.
    Path,
}

impl KeyEndpoint {
    /// The endpoint as Kaeso keeps it, `https://` and the lowercase host, from what someone pasted.
    pub fn parse(&self, input: &str) -> Result<String, EndpointProblem> {
        let input = input.trim();
        if input.len() > 2048 {
            return Err(EndpointProblem::NotUrl);
        }
        let url = url::Url::parse(input).map_err(|_| EndpointProblem::NotUrl)?;
        if url.scheme() != "https" {
            return Err(EndpointProblem::NotHttps);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(EndpointProblem::UserInfo);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(EndpointProblem::Query);
        }
        let Some(url::Host::Domain(host)) = url.host() else {
            return Err(EndpointProblem::Host);
        };
        let resource = self
            .hosts
            .iter()
            .find_map(|suffix| host.strip_suffix(suffix))
            .ok_or(EndpointProblem::Host)?;
        let plain =
            |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.');
        if resource.is_empty()
            || url.port().is_some()
            || !host.bytes().all(plain)
            || resource.starts_with(['-', '.'])
        {
            return Err(EndpointProblem::Host);
        }
        let path = url.path().trim_end_matches('/');
        if !path.is_empty()
            && !self
                .pasted_paths
                .iter()
                .any(|p| path.eq_ignore_ascii_case(p))
        {
            return Err(EndpointProblem::Path);
        }
        Ok(format!("https://{host}"))
    }

    /// The base URL of a key's requests: its endpoint and the provider's path.
    pub fn base(&self, endpoint: &str) -> String {
        format!("{}{}", endpoint.trim_end_matches('/'), self.path)
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
pub enum ModelListing {
    Complete,
    Partial,
    /// The listing names the models the provider offers, not the ones a key can call, such as
    /// Azure's, which lists what a resource may deploy rather than its deployments: a model check
    /// only shows that the key works.
    Catalogue,
}

impl ModelListing {
    // A partial listing that leaves a model out says nothing about it, hence `None`. Its
    // provider matches ids loosely: Standard Compute lists `anthropic/claude-haiku-5-5` and
    // answers it as `anthropic/claude-haiku-5.5`, so case and `.` against `-` don't count.
    pub fn serves<'a>(
        self,
        ids: impl IntoIterator<Item = &'a String>,
        model: &str,
    ) -> Option<bool> {
        let mut ids = ids.into_iter();
        match self {
            ModelListing::Complete => Some(ids.any(|id| id == model)),
            ModelListing::Partial => ids.any(|id| loosely_equal(id, model)).then_some(true),
            ModelListing::Catalogue => None,
        }
    }
}

fn loosely_equal(a: &str, b: &str) -> bool {
    let fold = |c: char| {
        if c == '.' {
            '-'
        } else {
            c.to_ascii_lowercase()
        }
    };
    a.len() == b.len() && a.chars().map(fold).eq(b.chars().map(fold))
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
    /// How the provider takes OpenAI embeddings requests, for a provider whose `embeddings` is
    /// `openai_embeddings`.
    pub embeddings_profile: EmbeddingsProfile,
    /// Where Kaeso sends rerank requests; `None` for a provider whose rerank API Kaeso doesn't
    /// call.
    pub rerank: Option<RerankEgress>,
    /// Where Kaeso sends image requests, by model, the first match winning; empty for a
    /// provider that offers no image API.
    pub images: &'static [ImagesEgress],
    /// Where Kaeso sends speech requests, by model, as for `images`.
    pub speech: &'static [SpeechEgress],
    /// Where Kaeso sends transcriptions, by model, as for `images`.
    pub transcriptions: &'static [TranscriptionsEgress],
    /// Where Kaeso sends video jobs, by model; empty for a provider that offers no video API.
    pub videos: &'static [VideosEgress],
    /// Where Kaeso sends decision requests, by model, as for `images`.
    pub decisions: &'static [DecisionsEgress],
    pub dialects: &'static [DialectId],
    /// The APIs besides `generation` that a request in their own dialect goes to as it came;
    /// each is also one of `dialects`.
    pub native: &'static [NativeDialect],
    pub refused_key_messages: &'static [&'static str],
    pub context_window_messages: &'static [&'static str],
    pub reasoning_beyond_output_limit: bool,
    pub listing: ModelListing,
    pub end_user_limit: Option<usize>,
    pub chat: ChatProfile,
    pub responses: ResponsesProfile,
    pub messages: MessagesProfile,
    pub gemini: GeminiProfile,
    pub account: Option<AccountSignIn>,
    pub routes: &'static [ModelRoute],
    pub service_account: Option<ServiceAccountSignIn>,
    pub account_scope: Option<AccountScope>,
    /// What takes the place of `/v1` in every dialect's paths for a provider whose API isn't
    /// under `/v1`: Chat Completions goes to `<base URL><api_root>/chat/completions`, the model
    /// listing to `<base URL><api_root>/models`. `Some("")` when the base URL itself ends at the
    /// API root, as Volcengine Ark's `…/api/v3`; `None` for `/v1`. A key's account scope
    /// ([`Provider::account_scope`]) goes first.
    pub api_root: Option<&'static str>,
    pub regional: Option<Regional>,
    pub key_endpoint: Option<KeyEndpoint>,
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

// The regions with a `bedrock-runtime` endpoint in https://docs.aws.amazon.com/general/latest/gr/bedrock.html
// (2026-11-14).
const BEDROCK_REGIONS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-2",
    "ca-central-1",
    "sa-east-1",
    "eu-central-1",
    "eu-central-2",
    "eu-north-1",
    "eu-south-1",
    "eu-south-2",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-northeast-3",
    "ap-south-1",
    "ap-south-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "us-gov-east-1",
    "us-gov-west-1",
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::openai(ImagesProfile::OPENAI)],
        speech: &[SpeechEgress::openai(SpeechProfile::OPENAI)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::OPENAI)],
        videos: &[],
        decisions: &[DecisionsEgress::openai(DecisionsProfile::OPENAI).for_models("gpt-6-luna*")],
        dialects: &[
            DialectId::OpenAiResponses,
            DialectId::OpenAiChat,
            DialectId::OpenAiEmbeddings,
        ],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        // OpenAI's OpenAPI spec caps `safety_identifier` at 128 characters, in both formats.
        end_user_limit: Some(128),
        chat: ChatProfile::OPENAI,
        responses: ResponsesProfile::OPENAI,
        messages: MessagesProfile::compatible("OpenAI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::AnthropicMessages],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Anthropic"),
        responses: ResponsesProfile::compatible("Anthropic"),
        messages: MessagesProfile::ANTHROPIC,
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        // The Anthropic-compatible Messages API at `/anthropic/v1/messages`, every model, either auth
        // header; it ignores `top_k`, `container` and `mcp_servers`, which are refused, and
        // answers `type: "custom"` on a client tool with 422
        // (https://api-docs.deepseek.com/guides/anthropic_api, read 2026-11-27).
        native: &[NativeDialect::of(DialectId::AnthropicMessages).at("/anthropic/v1")],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("DeepSeek")
            .max_tokens()
            .system_role()
            .reasoning_fields(&["reasoning_content"])
            .tool_schema_root(ToolSchemaRoot::TypedObjectUnions)
            .tool_patterns(ToolPatterns::Compiled)
            .forced_tool_without_thinking(),
        responses: ResponsesProfile::compatible("DeepSeek"),
        messages: MessagesProfile::compatible("DeepSeek")
            .refusing(&["top_k", "container", "mcp_servers"])
            .without_custom_tool_type()
            .forced_tool_without_thinking(),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::openai(ImagesProfile::XAI)],
        speech: &[SpeechEgress::openai(SpeechProfile::XAI)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::XAI)],
        videos: &[VideosEgress::of(DialectId::XaiVideos)],
        decisions: &[],
        dialects: &[DialectId::OpenAiResponses, DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &["Incorrect API key provided"],
        context_window_messages: &[],
        reasoning_beyond_output_limit: true,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::openai(ImagesProfile::TOGETHER)],
        speech: &[SpeechEgress::openai(SpeechProfile::TOGETHER)],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::TOGETHER,
        )],
        videos: &[VideosEgress {
            profile: VideosProfile::TOGETHER,
            ..VideosEgress::of(DialectId::OpenAiVideos)
        }],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Together AI")
            .with_top_k()
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        responses: ResponsesProfile::compatible("Together AI"),
        messages: MessagesProfile::compatible("Together AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[SpeechEgress::openai(SpeechProfile::GROQ)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::GROQ)],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::openai(ImagesProfile::OPENROUTER)],
        speech: &[SpeechEgress::openai(SpeechProfile::OPENROUTER)],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::OPENROUTER,
        )],
        videos: &[VideosEgress::of(DialectId::OpenRouterVideos)],
        decisions: &[DecisionsEgress::system_one(SystemOneProfile::OPENROUTER)],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at `/api/v1/messages`, every model, with the bearer key
        // (https://openrouter.ai/docs/api/api-reference/anthropic-messages/create-a-message.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // OpenRouter's audio and video guides: Chat Completions takes "any format string the
        // provider supports" in `input_audio.format`, and videos as `video_url` parts.
        chat: ChatProfile::compatible("OpenRouter")
            .with_top_k()
            .any_audio_format()
            .with_video_url()
            .with_reasoning_object()
            .reasoning_fields(&["reasoning"])
            .error_detail(&["error", "metadata", "raw"]),
        responses: ResponsesProfile::compatible("OpenRouter"),
        messages: MessagesProfile::compatible("OpenRouter"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[SpeechEgress::openai(SpeechProfile::MISTRAL)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::MISTRAL)],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &["too large for model with"],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::openai(ImagesProfile::VERCEL)],
        speech: &[SpeechEgress::openai(SpeechProfile::VERCEL)],
        transcriptions: &[TranscriptionsEgress::openai(TranscriptionsProfile::VERCEL)],
        videos: &[],
        decisions: &[DecisionsEgress::openai(DecisionsProfile::GENERIC)],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at `/v1/messages`, every model of the gateway
        // (https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-compat.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Vercel")
            .with_reasoning_object()
            .reasoning_fields(&["reasoning"]),
        responses: ResponsesProfile::compatible("Vercel"),
        messages: MessagesProfile::compatible("Vercel"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        // Cohere's Rerank API is `POST /v2/rerank` at the root of its host, beside the
        // compatibility API (https://docs.cohere.com/reference/rerank).
        rerank: Some(RerankEgress::of(DialectId::CohereRerank).at_root()),
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
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        // The Messages API at `/v1/messages`, with the bearer key; it takes no `tool_choice`
        // (https://docs.ollama.com/api/anthropic-compatibility.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Ollama Cloud")
            .max_tokens()
            .with_reasoning_object()
            .reasoning_fields(&["reasoning"])
            .refusing(&["logit_bias", "logprobs", "top_logprobs"]),
        responses: ResponsesProfile::compatible("Ollama Cloud"),
        messages: MessagesProfile::compatible("Ollama Cloud").auto_tool_choice_only(),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        // The Messages API at `/anthropic/v1/messages`, for the Kimi K3 models only
        // (https://platform.kimi.ai/docs/api/messages.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)
            .at("/anthropic/v1")
            .for_models(&["kimi-k3*"])],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        // QwenCloud's OpenAI-compatible image mode serves Qwen-Image 3.0 and 2.1 Pro only
        // (https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai);
        // its other image models answer on DashScope's own API.
        rerank: None,
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
        decisions: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at the host's `/apps/anthropic/v1/messages`, for the Qwen models
        // (https://docs.qwencloud.com/api-reference/chat/anthropic.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)
            .from_host_root("/apps/anthropic/v1")
            .for_models(&["qwen*"])],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: true,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
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
        decisions: &[],
        dialects: &[DialectId::AnthropicMessages],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Z.ai"),
        responses: ResponsesProfile::compatible("Z.ai"),
        messages: MessagesProfile::compatible("Z.ai"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
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
        decisions: &[],
        dialects: &[DialectId::AnthropicMessages],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
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
        decisions: &[],
        dialects: &[DialectId::ElevenLabs],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("ElevenLabs"),
        responses: ResponsesProfile::compatible("ElevenLabs"),
        messages: MessagesProfile::compatible("ElevenLabs"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::gemini()],
        speech: &[SpeechEgress::gemini()],
        transcriptions: &[TranscriptionsEgress::gemini()],
        videos: &[VideosEgress::of(DialectId::GeminiInteractions).for_models("gemini-omni-*")],
        decisions: &[DecisionsEgress::gemini()],
        dialects: &[DialectId::Gemini, DialectId::GeminiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("Google AI Studio"),
        responses: ResponsesProfile::compatible("Google AI Studio"),
        messages: MessagesProfile::compatible("Google AI Studio"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress::gemini()],
        speech: &[SpeechEgress::gemini()],
        transcriptions: &[TranscriptionsEgress::gemini()],
        videos: &[
            VideosEgress::of(DialectId::Veo).for_models("veo-*"),
            VideosEgress::of(DialectId::GeminiInteractions).for_models("gemini-omni-*"),
        ],
        decisions: &[],
        dialects: &[
            DialectId::Gemini,
            DialectId::AnthropicMessages,
            DialectId::OpenAiChat,
        ],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[DecisionsEgress::system_one(SystemOneProfile::FIREWORKS)],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at `/inference/v1/messages`, serverless and on-demand models; no token
        // count (https://docs.fireworks.ai/tools-sdks/anthropic-compatibility.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // Fireworks' chat schema has no `safety_identifier`, only `user`, and takes `video_url`
        // parts for its video models (https://docs.fireworks.ai/guides/video-audio-inputs).
        chat: ChatProfile::compatible("Fireworks AI")
            .with_top_k()
            .end_user_as_user()
            .with_video_url()
            .reasoning_fields(&["reasoning_content"]),
        responses: ResponsesProfile::compatible("Fireworks AI"),
        messages: MessagesProfile::compatible("Fireworks AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // Nebius' OpenAPI document lists no `developer` role and no `safety_identifier`, and
        // takes `video_url` parts (https://api.tokenfactory.nebius.com/openapi.json).
        chat: ChatProfile::compatible("Nebius Token Factory")
            .with_top_k()
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        // The Messages API at `/v1/messages` and its token count, every language model
        // (https://docs.sambanova.ai/docs/en/features/messages, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages).counting_tokens()],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // SambaNova's request schema declares no end-user or cache-key field and refuses
        // undeclared ones; it accepts both penalties without acting on them
        // (https://docs.sambanova.ai/docs/en/features/openai-compatibility).
        chat: ChatProfile::compatible("SambaNova")
            .with_top_k()
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::SCALEWAY,
        )],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "huggingface",
        name: "Hugging Face",
        key_page: KeyPage {
            url: "https://huggingface.co/settings/tokens",
            label: "Settings → Access Tokens",
        },
        key_prefix: Some("hf_"),
        default_base_url: "https://router.huggingface.co",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::AnthropicMessages],
        // The Messages API at `/v1/messages`, every chat model of every inference provider
        // (https://huggingface.co/docs/inference-providers/guides/messages-api.md, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // The router's chat schema has only `max_tokens` and no end-user field, and it passes
        // messages on to the model's own provider, not every one of which takes `developer`
        // (https://huggingface.co/docs/inference-providers/tasks/chat-completion); thinking
        // comes in that provider's field, `reasoning_content` or `reasoning`.
        chat: ChatProfile::compatible("Hugging Face")
            .max_tokens()
            .system_role()
            .without_end_user()
            .reasoning_fields(&["reasoning_content", "reasoning"]),
        responses: ResponsesProfile::compatible("Hugging Face"),
        messages: MessagesProfile::compatible("Hugging Face"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
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
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "baseten",
        name: "Baseten",
        key_page: KeyPage {
            url: "https://app.baseten.co/settings/api_keys",
            label: "Organization settings → API keys",
        },
        key_prefix: None,
        default_base_url: "https://inference.baseten.co",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[DecisionsEgress::system_one(SystemOneProfile::BASETEN)],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // Model APIs' request schema has only `max_tokens`, the end user only as `user` and the
        // roles `system`, `user`, `assistant` and `tool`; thinking comes as `reasoning_content`
        // (https://docs.baseten.co/inference/model-apis/reasoning).
        chat: ChatProfile::compatible("Baseten")
            .with_top_k()
            .max_tokens()
            .system_role()
            .end_user_as_user()
            .reasoning_fields(&["reasoning_content"]),
        responses: ResponsesProfile::compatible("Baseten"),
        messages: MessagesProfile::compatible("Baseten"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "siliconflow",
        name: "SiliconFlow",
        key_page: KeyPage {
            url: "https://cloud.siliconflow.com/account/ak",
            label: "API Keys",
        },
        key_prefix: Some("sk-"),
        default_base_url: "https://api.siliconflow.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[SpeechEgress::openai(SpeechProfile::SILICONFLOW)],
        transcriptions: &[TranscriptionsEgress::openai(
            TranscriptionsProfile::SILICONFLOW,
        )],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        // `max_tokens` caps only the answer, `thinking_budget` the thinking before it
        // (https://docs.siliconflow.com/en/userguide/capabilities/reasoning).
        reasoning_beyond_output_limit: true,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // SiliconFlow's request schema has only `max_tokens`, no end-user field and the roles
        // `system`, `user` and `assistant`; its thinking models take `enable_thinking` and
        // `thinking_budget` and answer with `reasoning_content`
        // (https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions).
        chat: ChatProfile::compatible("SiliconFlow")
            .with_top_k()
            .max_tokens()
            .system_role()
            .without_end_user()
            .thinking_switch()
            .reasoning_fields(&["reasoning_content"]),
        responses: ResponsesProfile::compatible("SiliconFlow"),
        messages: MessagesProfile::compatible("SiliconFlow"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "standardcompute",
        name: "Standard Compute",
        key_page: KeyPage {
            url: "https://standardcompute.com/dashboard/api-keys",
            label: "API Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.stdcmpt.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // A key's listing leaves out models it can still name, such as regional variants, and
        // names the router `StandardCompute` where requests send `standardcompute`; ids match
        // without regard to case (https://docs.standardcompute.com/models, checked live).
        listing: ModelListing::Partial,
        end_user_limit: None,
        // Standard Compute passes Chat Completions on to the model's own provider through a
        // router that answers thinking as `reasoning`
        // (https://docs.standardcompute.com/api/chat-completions).
        chat: ChatProfile::compatible("Standard Compute").reasoning_fields(&["reasoning"]),
        responses: ResponsesProfile::compatible("Standard Compute"),
        messages: MessagesProfile::compatible("Standard Compute"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "cloudflare-ai-gateway",
        name: "Cloudflare AI Gateway",
        // A user or account API token with Account > Workers AI > Read, which every
        // `/accounts/{account_id}/ai/*` endpoint needs, for third-party models too
        // (https://developers.cloudflare.com/ai-gateway/usage/rest-api/#authentication). New tokens
        // start `cfut_` or `cfat_`; older ones have no prefix
        // (https://developers.cloudflare.com/fundamentals/api/get-started/token-formats/).
        key_page: KeyPage {
            url: "https://dash.cloudflare.com/profile/api-tokens",
            label: "API Tokens",
        },
        key_prefix: None,
        default_base_url: "https://api.cloudflare.com",
        auth: AuthStyle::Bearer,
        // The REST API serves Chat Completions, Responses and Anthropic Messages for third-party
        // and Workers AI models; Cloudflare deprecated the `/compat` endpoint for single-model
        // calls. Each model's page lists the formats it takes, which KPMR carries as the model's
        // egress dialect where it isn't Chat Completions.
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        // Workers AI's embedding models answer OpenAI's embeddings API under the account's
        // `/ai/v1`, and leave out `dimensions` and `usage` (recorded live); `/ai/v1/embeddings`
        // isn't among the AI Gateway REST API's paths, but answers through a gateway too
        // (https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/).
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::compatible("Cloudflare AI Gateway")
            .without_dimensions(),
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        // Workers AI's Clef models answer System One bodies at `/run/@cf/cloudflare/{model}`
        // beside the account's `/ai/v1`, through the gateway the key names
        // (https://developers.cloudflare.com/workers-ai/models/clef/,
        // https://developers.cloudflare.com/ai-gateway/usage/rest-api/#call-a-workers-ai-model).
        decisions: &[
            DecisionsEgress::system_one(SystemOneProfile::CLOUDFLARE_WORKERS_AI)
                .for_models("@cf/cloudflare/clef*"),
        ],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // There is no model listing in a model format; a model check asks Cloudflare's Workers AI
        // model search instead (services::cloudflare).
        listing: ModelListing::Complete,
        end_user_limit: None,
        // Workers AI models answer thinking as `reasoning` and the same text again as
        // `reasoning_content` (recorded live).
        chat: ChatProfile::compatible("Cloudflare AI Gateway")
            .reasoning_fields(&["reasoning", "reasoning_content"]),
        responses: ResponsesProfile::compatible("Cloudflare AI Gateway"),
        messages: MessagesProfile::compatible("Cloudflare AI Gateway"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: Some(AccountScope {
            account_page: KeyPage {
                url: "https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/",
                label: "Find account and zone IDs",
            },
            path: "/client/v4/accounts/{account}/ai/v1",
            gateway_header: "cf-aig-gateway-id",
        }),
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "perplexity",
        name: "Perplexity",
        // Keys are made in a project's API Keys page and start `pplx-`
        // (https://docs.perplexity.ai/docs/admin/api-key-management.md).
        key_page: KeyPage {
            url: "https://console.perplexity.ai/project/keys",
            label: "API Keys",
        },
        key_prefix: Some("pplx-"),
        default_base_url: "https://api.perplexity.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[DecisionsEgress::system_one(SystemOneProfile::PERPLEXITY)],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` lists the Agent API's models, such as `perplexity/sonar`, and none of
        // the ids Sonar chat completions take (https://docs.perplexity.ai/openapi.json).
        listing: ModelListing::Partial,
        end_user_limit: None,
        // Sonar chat completions at `/v1/sonar` take the fields of `ApiChatCompletionsRequest`
        // in https://docs.perplexity.ai/openapi.json (2026-11-12) and report usage in every
        // stream chunk; the official SDK lists tools, penalties and log probabilities too, so
        // the sources disagree and Kaeso refuses them. End users go in a header, not the body.
        chat: ChatProfile::compatible("Perplexity")
            .at_path("/v1/sonar")
            .max_tokens()
            .system_role()
            .without_end_user()
            .without_cache_key()
            .without_stream_options()
            .refusing(&[
                "audio",
                "frequency_penalty",
                "function_call",
                "functions",
                "logit_bias",
                "logprobs",
                "metadata",
                "modalities",
                "parallel_tool_calls",
                "prediction",
                "presence_penalty",
                "prompt_cache_key",
                "prompt_cache_retention",
                "seed",
                "service_tier",
                "store",
                "stream_mode",
                "tool_choice",
                "tools",
                "top_logprobs",
            ]),
        responses: ResponsesProfile::compatible("Perplexity"),
        messages: MessagesProfile::compatible("Perplexity"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "typesafe",
        name: "TypeSafe",
        // Keys are made in the console's keys page (https://docs.typesafe.ai/introduction/quickstart)
        // and have no documented prefix.
        key_page: KeyPage {
            url: "https://console.typesafe.ai/keys",
            label: "API keys",
        },
        key_prefix: None,
        default_base_url: "https://api.typesafe.ai",
        auth: AuthStyle::Bearer,
        // TypeSafe answers decision questions and writes no text, so its only format is System
        // One, as ElevenLabs' is its audio API; a generation request never reaches it past the
        // catalogue's checks, and the System One codec refuses one that does.
        generation: DialectId::SystemOne,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[DecisionsEgress::system_one(SystemOneProfile::TYPESAFE)],
        dialects: &[DialectId::SystemOne],
        // A refused key is a 401, a missing one a 403, both with `error_type:
        // "authentication_error"`, which the System One codec reads as a refused key.
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` lists "the models and aliases available to the authenticated account"
        // in TypeSafe's own shape (https://api.typesafe.ai/openapi.json, `ModelMetadataList`),
        // and there is no `GET /v1/models/{model}`; whether it lists every versioned id isn't
        // documented, so a model it leaves out is inconclusive, never unavailable.
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("TypeSafe"),
        responses: ResponsesProfile::compatible("TypeSafe"),
        messages: MessagesProfile::compatible("TypeSafe"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "deepinfra",
        name: "DeepInfra",
        // A key needs the `inference` scope, which every new key starts with
        // (https://docs.deepinfra.com/account/authentication.md).
        key_page: KeyPage {
            url: "https://deepinfra.com/dash/keys",
            label: "API keys",
        },
        key_prefix: None,
        // DeepInfra's guides write the base URL as `https://api.deepinfra.com/v1/openai`; its
        // OpenAPI document serves the same API at `/v1/…` under the host
        // (https://api.deepinfra.com/openapi.json).
        default_base_url: "https://api.deepinfra.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at `/anthropic/v1/messages` and its token count, every chat model
        // (https://docs.deepinfra.com/integrations/anthropic, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)
            .at("/anthropic/v1")
            .counting_tokens()],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // `OpenAIChatCompletionsIn` in DeepInfra's OpenAPI document has only `max_tokens`, the end
        // user as `user`, the roles `system`, `user`, `assistant` and `tool`, `video_url` parts
        // and `reasoning_content` on assistant messages, and none of the fields refused here,
        // which it would otherwise drop without a word (https://api.deepinfra.com/openapi.json).
        chat: ChatProfile::compatible("DeepInfra")
            .with_top_k()
            .max_tokens()
            .system_role()
            .end_user_as_user()
            .with_video_url()
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
                "prompt_cache_retention",
                "store",
                "top_logprobs",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("DeepInfra"),
        messages: MessagesProfile::compatible("DeepInfra"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "upstage",
        name: "Upstage",
        // Keys are made on the console's API keys page; the getting-started guide shows one
        // starting with `up_` only as an example (https://console.upstage.ai/docs/getting-started).
        key_page: KeyPage {
            url: "https://console.upstage.ai/api-keys",
            label: "API keys",
        },
        key_prefix: None,
        // Upstage's guides write the base URL as `https://api.upstage.ai/v1` and serve chat at
        // `/v1/chat/completions` and embeddings at `/v1/embeddings` under the host
        // (https://console.upstage.ai/api/chat). The Korea region, in closed beta, is another
        // host with the same paths, `https://kr.api.upstage.ai`
        // (https://console.upstage.ai/docs/guides/region-usage).
        default_base_url: "https://api.upstage.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        // Upstage counts tokens with a tokenizer it publishes, not an API
        // (https://console.upstage.ai/docs/guides/counting-tokens).
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        // The embeddings reference documents only `model` and `input`; every vector has 1,024
        // dimensions (https://console.upstage.ai/api/embeddings).
        embeddings_profile: EmbeddingsProfile::compatible("Upstage").without_dimensions(),
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` needs a key and isn't in the API reference; Upstage's own Claude Code
        // script reads ids from it (https://console.upstage.ai/claude-solar.sh). Neither its
        // shape nor whether it names every model is documented, so a model it leaves out is
        // inconclusive, never unavailable.
        listing: ModelListing::Partial,
        end_user_limit: None,
        // The chat request schema has only `max_tokens`, the roles `system`, `user`, `assistant`
        // and `tool`, no end-user field, and none of the fields refused here; whether Upstage
        // refuses an undeclared field isn't documented, so Kaeso refuses them rather than risk
        // them being dropped. Thinking comes as `reasoning` (https://console.upstage.ai/api/chat).
        chat: ChatProfile::compatible("Upstage")
            .max_tokens()
            .system_role()
            .without_end_user()
            .reasoning_fields(&["reasoning"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "logprobs",
                "metadata",
                "modalities",
                "prediction",
                "prompt_cache_retention",
                "seed",
                "service_tier",
                "stop",
                "store",
                "top_logprobs",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("Upstage"),
        messages: MessagesProfile::compatible("Upstage"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "ionos",
        name: "IONOS AI Model Hub",
        // An IONOS Cloud API token, a JWT made in the Data Center Designer's Token Manager, for a
        // user whose group may access the AI Model Hub; tokens expire after the time chosen at
        // creation (https://docs.ionos.com/cloud/ai/ai-model-hub/how-tos/access-management).
        key_page: KeyPage {
            url: "https://dcd.ionos.com",
            label: "Management → Token Manager",
        },
        key_prefix: None,
        // Every inference runs in Berlin (`de-txl`), the only region
        // (https://docs.ionos.com/cloud/ai/ai-model-hub/governance-and-compliance/data-handling).
        default_base_url: "https://openai.inference.de-txl.ionos.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` needs a token and lists every model the hub serves; there is no
        // `GET /v1/models/{model}` (the API reference's OpenAPI document).
        listing: ModelListing::Complete,
        end_user_limit: None,
        // `ChatCompletionRequest` in IONOS's API reference: `developer` works only on some
        // models ("Use `system` for the widest compatibility"), the end user is `user`, no
        // `prompt_cache_key` (caching is automatic), `video_url` parts, and
        // replies carry `reasoning` or, on some models, `reasoning_content` too; the fields
        // refused here aren't in the schema, which doesn't say whether it refuses them. No model
        // takes audio, which the catalogue's input types already refuse
        // (https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/chat-completions/create-chat-completion).
        chat: ChatProfile::compatible("IONOS AI Model Hub")
            .system_role()
            .end_user_as_user()
            .without_cache_key()
            .with_video_url()
            .reasoning_fields(&["reasoning", "reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "metadata",
                "modalities",
                "prediction",
                "prompt_cache_retention",
                "service_tier",
                "store",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("IONOS AI Model Hub"),
        messages: MessagesProfile::compatible("IONOS AI Model Hub"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "novita-ai",
        name: "Novita AI",
        // "Every key begins with the `sk_` prefix"; keys are made under Key Management
        // (https://docs.novita.ai/guides/llm-api-keys.md).
        key_page: KeyPage {
            url: "https://novita.ai/settings/key-management",
            label: "Key Management",
        },
        key_prefix: Some("sk_"),
        // Novita AI's OpenAI-compatible base URL is `https://api.novita.ai/openai`, and its API
        // reference serves chat, embeddings and models at `/openai/v1/…`
        // (https://docs.novita.ai/guides/llm-api.md).
        default_base_url: "https://api.novita.ai/openai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        // The embeddings reference has `input`, `model` and `encoding_format`, and no `dimensions`
        // (https://docs.novita.ai/api-reference/model-apis-llm-create-embeddings.md).
        embeddings_profile: EmbeddingsProfile::compatible("Novita AI").without_dimensions(),
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        // A refused key is a 401 `FAILED_TO_AUTH`, a missing one a 403 `INVALID_API_KEY`
        // (https://docs.novita.ai/api-reference/basic-error-code.md).
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        // `max_tokens` counts reasoning tokens too (https://docs.novita.ai/guides/LLM-FAQ.md).
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` lists only chat models; embedding models need an undocumented
        // `?model_type=embedding` (https://docs.novita.ai/api-reference/model-apis-llm-list-models.md),
        // so a model it leaves out is inconclusive, never unavailable.
        listing: ModelListing::Partial,
        end_user_limit: None,
        // Novita AI's chat reference has only `max_tokens`, the roles `system`, `user` and
        // `assistant` (its guides send `tool` too), no end-user field, `video_url` parts,
        // `enable_thinking`, and `separate_reasoning`, without which reasoning stays inside
        // `content`; it doesn't say whether it refuses undeclared fields, so the OpenAI fields it
        // leaves out are refused here instead of being dropped without a word
        // (https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md).
        chat: ChatProfile::compatible("Novita AI")
            .with_top_k()
            .max_tokens()
            .system_role()
            .without_end_user()
            .without_cache_key()
            .with_video_url()
            .thinking_switch()
            .separate_reasoning()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "metadata",
                "parallel_tool_calls",
                "prediction",
                "prompt_cache_key",
                "prompt_cache_retention",
                "service_tier",
                "store",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("Novita AI"),
        messages: MessagesProfile::compatible("Novita AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "volcengine",
        name: "Volcengine Ark",
        // Keys are made on the console's API Key 管理 page; Volcengine documents no key prefix
        // (https://docs.volcengine.com/docs/zh/ark/api-key.md).
        key_page: KeyPage {
            url: "https://ark.volcengine.com/region:cn-beijing/apiKey",
            label: "API Key 管理",
        },
        key_prefix: None,
        // Ark's API sits at `/api/v3`, not `/v1`: `…/api/v3/chat/completions`, `…/api/v3/models`
        // (https://docs.volcengine.com/docs/zh/ark/base-url-and-authentication.md). The base URL
        // ends at that root, so BytePlus ModelArk's `https://ark.ap-southeast.bytepluses.com/api/v3`
        // works as a base-URL override.
        default_base_url: "https://ark.cn-beijing.volces.com/api/v3",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        // Ark embeds only through `/embeddings/multimodal`, one vector for the whole input,
        // which isn't OpenAI's embeddings format
        // (https://docs.volcengine.com/docs/zh/ark/multimodal-vectorization-api.md).
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        // 400 `OutofContextError` (https://docs.volcengine.com/docs/zh/ark/error-codes.md).
        context_window_messages: &["exceed max message tokens"],
        // A Chat app's own `max_tokens` limits only the answer on most models
        // (https://docs.volcengine.com/docs/zh/ark/chat-api.md).
        reasoning_beyond_output_limit: true,
        // `GET /models` lists the base model versions an account sees, not its inference
        // endpoints (`ep-…`), which a request may name instead
        // (https://docs.volcengine.com/docs/zh/ark/models-api.md).
        listing: ModelListing::Partial,
        end_user_limit: None,
        // From Ark's Chat API reference (https://docs.volcengine.com/docs/zh/ark/chat-api.md,
        // updated 2026-11-12):
        // - `developer` messages are accepted since the 2026-09 schema upgrade
        //   (https://docs.volcengine.com/docs/zh/ark/api-schema-upgrade-notice.md);
        // - the output limit is sent as the app wrote it: Kaeso's own limit becomes
        //   `max_completion_tokens`, which counts thinking and answer like Anthropic's
        //   `max_tokens` and OpenAI's own field, while Ark's `max_tokens` leaves thinking out on
        //   most models; Ark refuses the two together;
        // - no end-user field (`user`, `safety_identifier`);
        // - thinking is switched with `thinking: {"type": "enabled" | "disabled"}` and graded by
        //   `reasoning_effort` (https://docs.volcengine.com/docs/zh/ark/deep-thinking.md);
        // - `video_url` parts, `reasoning_content` in replies and deltas, and on newer Seed
        //   models `encrypted_content`, which must go back unchanged in multi-turn tool calls;
        // - `service_tier` takes `auto`, `default`, `fast` and `flex`, not OpenAI's `scale` or
        //   `priority`; `web_search_options` works with Doubao Search only, and its
        //   `search_context_size` and `user_location` have no effect;
        // - none of the other refused fields; whether Ark refuses undeclared fields isn't
        //   documented, so Kaeso refuses them rather than risk Ark dropping them.
        chat: ChatProfile::compatible("Volcengine Ark")
            .without_end_user()
            .thinking_type()
            .with_video_url()
            .reasoning_fields(&["reasoning_content"])
            .encrypted_reasoning_field("encrypted_content")
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "metadata",
                "modalities",
                "n",
                "prediction",
                "prompt_cache_retention",
                "seed",
                "store",
                "web_search_options.search_context_size",
                "web_search_options.user_location",
            ])
            .refusing_values(&[("service_tier", &["priority", "scale"])]),
        responses: ResponsesProfile::compatible("Volcengine Ark"),
        messages: MessagesProfile::compatible("Volcengine Ark"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: Some(""),
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "ovhcloud",
        name: "OVHcloud AI Endpoints",
        // Keys belong to a Public Cloud project: Public Cloud → AI → AI Endpoints → API keys, with
        // no documented prefix; the catalogue page links the console's Public Cloud section
        // (https://www.ovhcloud.com/en/public-cloud/ai-endpoints/catalog/). Without a key AI Endpoints
        // still answers, at 2 requests a minute per IP and model
        // (https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-getting-started.md).
        key_page: KeyPage {
            url: "https://www.ovh.com/manager/#/public-cloud/",
            label: "Public Cloud → AI Endpoints → API keys",
        },
        key_prefix: None,
        // The documented base is `https://oai.endpoints.kepler.ai.cloud.ovh.net/v1`, and every
        // path is `/v1/…` under the host
        // (https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-catalog-api.md).
        default_base_url: "https://oai.endpoints.kepler.ai.cloud.ovh.net",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        // `Qwen3-Embedding-8B` takes `dimensions`, and `bge-m3`, whose size is fixed, answers it
        // with a 400, so nothing is dropped without a word.
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        // A wrong key is a 403 with only a `message`, "Forbidden: authentication failed.", on chat,
        // embeddings and the model listing alike; the Chat Completions codec quotes it.
        native: &[],
        refused_key_messages: &[
            "Forbidden: authentication failed.",
            "The provider returned an error (HTTP 403): Forbidden: authentication failed.",
        ],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // `ChatCompletionParameters` in AI Endpoints' OpenAPI document
        // (https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/gpt-oss-120b/openapi.json): the
        // `developer` role works on some models only (Mistral answers "Unknown message role"),
        // `user` is refused and no other end-user or cache-key field is declared, and replies
        // carry `reasoning`, which assistant messages take back. Undeclared fields such as
        // `service_tier` are refused by AI Endpoints itself with a 400, but its parser lets
        // through the fields refused here, which the schema doesn't declare; a Responses app's
        // cache key is left out.
        chat: ChatProfile::compatible("OVHcloud AI Endpoints")
            .system_role()
            .without_end_user()
            .without_cache_key()
            .reasoning_fields(&["reasoning"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "metadata",
                "modalities",
                "prediction",
                "prompt_cache_key",
                "store",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("OVHcloud AI Endpoints"),
        messages: MessagesProfile::compatible("OVHcloud AI Endpoints"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "stepfun-ai",
        name: "StepFun",
        // Keys are made on the Interface Keys page and have no documented prefix; the China
        // service (platform.stepfun.com, `https://api.stepfun.com`) has accounts and keys of its
        // own (https://platform.stepfun.ai/docs/en/quickstart/overview).
        key_page: KeyPage {
            url: "https://platform.stepfun.ai/interface-key",
            label: "Interface Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.stepfun.ai",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` answers only with a key and lists chat and speech models alike
        // (https://platform.stepfun.ai/docs/en/api-reference/models/list).
        listing: ModelListing::Complete,
        end_user_limit: None,
        // StepFun's Chat Completions reference lists only `max_tokens`, the roles `system`,
        // `user`, `assistant` and `tool`, no end-user or cache-key field, `video_url` parts and
        // usage in every stream chunk without `stream_options`; reasoning comes as `reasoning`,
        // or as `reasoning_content` with `reasoning_format: "deepseek-style"`. Whether it refuses
        // the fields it leaves out isn't documented, so they are refused here
        // (https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create).
        chat: ChatProfile::compatible("StepFun")
            .max_tokens()
            .system_role()
            .without_end_user()
            .without_cache_key()
            .without_stream_options()
            .with_video_url()
            .reasoning_fields(&["reasoning", "reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "logprobs",
                "metadata",
                "modalities",
                "parallel_tool_calls",
                "prediction",
                "presence_penalty",
                "prompt_cache_key",
                "prompt_cache_retention",
                "seed",
                "service_tier",
                "store",
                "top_logprobs",
                "web_search_options",
            ])
            .refusing_in_messages(&["name"]),
        responses: ResponsesProfile::compatible("StepFun"),
        messages: MessagesProfile::compatible("StepFun"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "tencent-tokenhub",
        name: "Tencent TokenHub",
        // Keys belong to one region and site and don't work in another; the console makes them
        // for the region chosen at the top of the page
        // (https://cloud.tencent.com/document/product/1823/130090).
        key_page: KeyPage {
            url: "https://console.cloud.tencent.com/tokenhub/apikey",
            label: "API Key management",
        },
        key_prefix: None,
        // The Guangzhou region, scheduled in mainland China. Its SDK base URL is
        // `https://tokenhub.tencentmaas.com/v1`; every path sits under `/v1` on the host
        // (https://cloud.tencent.com/document/product/1823/130078). Singapore and the
        // international site's regions are other hosts, set as the base URL.
        default_base_url: "https://tokenhub.tencentmaas.com",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        // Its embedding models answer vectors of a fixed size and take no `dimensions`
        // ("不支持自定义", https://cloud.tencent.com/document/product/1823/133515).
        embeddings_profile: EmbeddingsProfile::compatible("Tencent TokenHub").without_dimensions(),
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at `/v1/messages`, every model
        // (https://cloud.tencent.com/document/product/1823/130079, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages)],
        refused_key_messages: &[],
        context_window_messages: &[],
        // Reasoning and answer share the output limit
        // (https://cloud.tencent.com/document/product/1823/132252).
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        // TokenHub's Chat Completions reference has the roles `system`, `user`, `assistant` and
        // `tool`, both output limits, `safety_identifier`, `video_url` parts and
        // `reasoning_content`, and none of the fields refused here; it ignores undeclared
        // fields without a word ("会被静默忽略")
        // (https://cloud.tencent.com/document/product/1823/135872).
        chat: ChatProfile::compatible("Tencent TokenHub")
            .system_role()
            .with_video_url()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "metadata",
                "modalities",
                "prediction",
                "prompt_cache_retention",
                "service_tier",
                "store",
            ]),
        responses: ResponsesProfile::compatible("Tencent TokenHub"),
        messages: MessagesProfile::compatible("Tencent TokenHub"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "venice",
        name: "Venice",
        // Inference keys reach chat and embeddings; admin keys only the account routes
        // (https://docs.venice.ai/guides/getting-started/generating-api-key).
        key_page: KeyPage {
            url: "https://venice.ai/settings/api",
            label: "API settings",
        },
        key_prefix: None,
        // Venice's base URL is `https://api.venice.ai/api/v1`, so the API root sits one level
        // below the host (https://docs.venice.ai/api-reference/api-spec).
        default_base_url: "https://api.venice.ai/api",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /models` lists text models only unless asked for `?type=embedding`
        // (https://docs.venice.ai/api-reference/endpoint/models/list).
        listing: ModelListing::Partial,
        end_user_limit: None,
        // Venice's `ChatCompletionRequest` refuses undeclared top-level fields with 400
        // (https://docs.venice.ai/guides/getting-started/openai-migration), but discards `user`,
        // has no `safety_identifier`, and takes `metadata` and `store` without acting on them
        // (https://docs.venice.ai/swagger.yaml). It prepends a system prompt of its own unless
        // `venice_parameters.include_venice_system_prompt` is false
        // (https://docs.venice.ai/api-reference/api-spec#system-prompts).
        chat: ChatProfile::compatible("Venice")
            .without_end_user()
            .with_video_url()
            .any_audio_format()
            .reasoning_fields(&["reasoning_content"])
            .with_option_defaults(&[OptionDefault::new(
                "venice_parameters",
                "include_venice_system_prompt",
                "false",
            )])
            .refusing(&["metadata", "store"]),
        responses: ResponsesProfile::compatible("Venice"),
        messages: MessagesProfile::compatible("Venice"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        regional: None,
        api_root: None,
        key_endpoint: None,
    },
    Provider {
        id: "amazon-bedrock",
        name: "Amazon Bedrock",
        // Bedrock API keys are made under API keys in the Bedrock console: long-term keys start
        // `ABSK`, short-term ones `bedrock-api-key-`, so no one prefix fits
        // (https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html).
        key_page: KeyPage {
            url: "https://console.aws.amazon.com/bedrock/",
            label: "API keys",
        },
        key_prefix: None,
        // Each key's region picks its own runtime host; a base URL set to anything else, such as
        // a VPC endpoint, serves every key as it is.
        default_base_url: "https://bedrock-runtime.us-east-1.amazonaws.com",
        auth: AuthStyle::Bearer,
        // bedrock-runtime serves Chat Completions at `/openai/v1` and Anthropic Messages at
        // `/anthropic/v1` (https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html);
        // KPMR names the one each model takes, from AWS's API compatibility table.
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        // Converse serves the chat models that answer neither of the other two, such as Amazon
        // Nova and Meta Llama (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html).
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::BedrockConverse,
        ],
        // A refused key is a 401 on the Chat Completions route but a 403 `permission_error` on
        // the Messages route, with these messages for a malformed, an unsigned and an unknown key
        // (recorded live).
        native: &[],
        refused_key_messages: &[
            "Invalid API Key format",
            "Missing required parameters in the API Key",
            "Authentication failed",
        ],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // bedrock-runtime lists no models (`GET /openai/v1/models` is a 404
        // `UnknownOperationException`), so a model check is inconclusive (services::regional).
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("Amazon Bedrock"),
        responses: ResponsesProfile::compatible("Amazon Bedrock"),
        // The Claude features Anthropic lists as unavailable on bedrock-runtime
        // (https://platform.claude.com/docs/en/build-with-claude/claude-on-amazon-bedrock-legacy).
        messages: MessagesProfile::compatible("Amazon Bedrock")
            .refusing(&["mcp_servers", "container"]),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        // The Messages route takes the key as `x-api-key`
        // (https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html).
        api_root: None,
        regional: Some(Regional {
            regions_page: KeyPage {
                url: "https://docs.aws.amazon.com/general/latest/gr/bedrock.html",
                label: "Amazon Bedrock endpoints",
            },
            regions: BEDROCK_REGIONS,
            host: "bedrock-runtime.{region}.amazonaws.com",
            api_roots: &[
                (DialectId::OpenAiChat, "/openai/v1"),
                (DialectId::AnthropicMessages, "/anthropic/v1"),
                (DialectId::BedrockConverse, ""),
            ],
            auth: &[(DialectId::AnthropicMessages, AuthStyle::XApiKey)],
        }),
        key_endpoint: None,
    },
    Provider {
        id: "azure",
        name: "Azure",
        // A resource's KEY 1 or KEY 2, on its "Keys and Endpoint" page in the Azure portal, with
        // the endpoint beside it; keys have no prefix and differ in length between older and
        // newer resources (https://learn.microsoft.com/en-us/azure/ai-services/authentication).
        // A key may instead be a Microsoft Entra ID service principal, whose tokens go as a bearer
        // (`kaeso_core::azure`).
        key_page: KeyPage {
            url: "https://portal.azure.com/",
            label: "Keys and Endpoint",
        },
        key_prefix: None,
        // Never called while it is the default: each key's endpoint takes its place
        // (`key_endpoint`). An install may set it to send every Azure request elsewhere, such as
        // through an API gateway in front of Azure.
        default_base_url: "https://openai.azure.com",
        // The v1 API takes the key as `api-key` or as a bearer token; a bearer that looks like a
        // JWT is read as an Entra ID token and refused in another shape (recorded live), so the
        // key goes as `api-key` (https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models?view=rest-microsoft-foundry-v1).
        auth: AuthStyle::ApiKey,
        // `{endpoint}/openai/v1` serves Chat Completions, Responses and Embeddings without an
        // `api-version` (https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle).
        // Deployments that don't take the Responses API answer `400 Model not supported`
        // (https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints), so
        // Chat Completions is the default. Images and audio are preview-only on v1, and Claude
        // deployments answer only Anthropic's Messages API under `/anthropic`.
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
            DialectId::OpenAiEmbeddings,
        ],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /openai/v1/models` lists the models a resource may deploy, not its deployments.
        listing: ModelListing::Catalogue,
        end_user_limit: None,
        // The v1 OpenAPI document's `CreateChatCompletionRequest` has every field of OpenAI's,
        // `verbosity` and the `developer` role included (Azure/azure-rest-api-specs,
        // specification/ai/data-plane/OpenAI.v1/azure-v1-v1-generated.json). Azure answers a
        // field it doesn't know with 400 "Unrecognized request argument supplied", so nothing
        // is refused here.
        chat: ChatProfile::compatible("Azure").with_verbosity(),
        responses: ResponsesProfile::compatible("Azure"),
        messages: MessagesProfile::compatible("Azure"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        // The resource endpoint on the same page as the key; the v1 API answers under
        // `{endpoint}/openai/v1` on all three hosts, which Microsoft's SDKs accept as Azure's
        // (https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints).
        key_endpoint: Some(KeyEndpoint {
            page: KeyPage {
                url: "https://learn.microsoft.com/en-us/azure/ai-services/authentication",
                label: "Find the key and endpoint",
            },
            hosts: &[
                ".openai.azure.com",
                ".services.ai.azure.com",
                ".cognitiveservices.azure.com",
            ],
            path: "/openai",
            pasted_paths: &["/openai/v1", "/openai"],
            example: "https://example.openai.azure.com",
        }),
    },
    Provider {
        id: "voyage",
        name: "Voyage AI",
        // MongoDB sells Voyage AI's API as the Atlas Embedding and Reranking API: model API keys
        // (`al-…`) are made in Atlas under AI Model APIs
        // (https://www.mongodb.com/docs/voyageai/management/api-keys/). Keys from Voyage's own
        // dashboard (`pa-…`) work against `https://api.voyageai.com`, and keys scoped to a region
        // against `https://eu.ai.mongodb.com` or `https://us.ai.mongodb.com`, each as a base URL
        // of its own (https://www.mongodb.com/docs/voyageai/api-reference/overview/).
        key_page: KeyPage {
            url: "https://www.mongodb.com/docs/voyageai/management/api-keys/",
            label: "Model API keys",
        },
        key_prefix: None,
        default_base_url: "https://ai.mongodb.com",
        auth: AuthStyle::Bearer,
        // Voyage AI writes no text: its models embed and rerank, so a model without a kind of
        // its own is an embeddings model and a chat request never reaches it.
        generation: DialectId::OpenAiEmbeddings,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::VOYAGE,
        // `POST /v1/rerank`, with `top_k` and string documents
        // (https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json).
        rerank: Some(RerankEgress::of(DialectId::VoyageRerank)),
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // Voyage AI serves no `GET /v1/models` (404, recorded 2026-11-14).
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("Voyage AI"),
        responses: ResponsesProfile::compatible("Voyage AI"),
        messages: MessagesProfile::compatible("Voyage AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "jina",
        name: "Jina AI",
        // Jina AI's Search Foundation API, which Elastic runs since it bought Jina AI; keys
        // start with `jina_` and are made in Jina's API dashboard (https://docs.jina.ai/api.md).
        key_page: KeyPage {
            url: "https://jina.ai/api-dashboard/key-manager",
            label: "API key manager",
        },
        key_prefix: Some("jina_"),
        default_base_url: "https://api.jina.ai",
        auth: AuthStyle::Bearer,
        // Jina AI's models embed and rerank, so a model without a kind of its own is an
        // embeddings model and a chat request never reaches it.
        generation: DialectId::OpenAiEmbeddings,
        count_tokens: None,
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::JINA,
        // `POST /v1/rerank`, Cohere's v1 shape with `return_documents` true by default
        // (https://api.jina.ai/openapi.json, `RerankerV3Request`).
        rerank: Some(RerankEgress::of(DialectId::JinaRerank)),
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::OpenAiEmbeddings],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` lists every model as `jina-ai/<id>`, while requests name it without
        // the prefix; `GET /v1/models/{id}` finds it by the short id (recorded 2026-11-14).
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("Jina AI"),
        responses: ResponsesProfile::compatible("Jina AI"),
        messages: MessagesProfile::compatible("Jina AI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        regional: None,
        key_endpoint: None,
        api_root: None,
    },
    Provider {
        id: "fal",
        name: "fal",
        // A key with the `API` preset calls models; a team's keys belong to the team
        // (https://fal.ai/docs/documentation/model-apis/authentication/key-based).
        key_page: KeyPage {
            url: "https://fal.ai/dashboard/keys",
            label: "API keys",
        },
        key_prefix: None,
        // fal's queue (https://fal.ai/docs/documentation/model-apis/inference/queue); its
        // status and result URLs are followed only on `kaeso_codecs::fal::QUEUE_HOSTS`.
        default_base_url: "https://queue.fal.run",
        auth: AuthStyle::Key,
        // fal hosts image and video models here; it writes no text, so a generation request
        // never reaches it past the catalogue's checks.
        generation: DialectId::Fal,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[ImagesEgress {
            dialect: DialectId::Fal,
            ..ImagesEgress::ANY
        }],
        speech: &[],
        transcriptions: &[],
        videos: &[VideosEgress::of(DialectId::Fal)],
        decisions: &[],
        dialects: &[DialectId::Fal],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        listing: ModelListing::Complete,
        end_user_limit: None,
        chat: ChatProfile::compatible("fal"),
        responses: ResponsesProfile::compatible("fal"),
        messages: MessagesProfile::compatible("fal"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "replicate",
        name: "Replicate",
        // API tokens are 40 characters starting with `r8_`
        // (https://replicate.com/docs/topics/security/api-tokens).
        key_page: KeyPage {
            url: "https://replicate.com/account/api-tokens",
            label: "API tokens",
        },
        key_prefix: Some("r8_"),
        default_base_url: "https://api.replicate.com",
        auth: AuthStyle::Bearer,
        // Replicate runs image and video models here; Kaeso maps no text model of it, so a
        // generation request never reaches it past the catalogue's checks.
        generation: DialectId::Replicate,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        // Replicate answers with links on `replicate.delivery` and its subdomains, which it
        // deletes after an hour (https://replicate.com/docs/topics/predictions/output-files);
        // `b64_json` fetches them (`kaeso_codecs::replicate::OUTPUT_HOSTS`).
        images: &[ImagesEgress {
            dialect: DialectId::Replicate,
            ..ImagesEgress::ANY
        }
        .fetching(REPLICATE_OUTPUTS)],
        speech: &[],
        transcriptions: &[],
        videos: &[VideosEgress::of(DialectId::Replicate)],
        decisions: &[],
        dialects: &[DialectId::Replicate],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // The model check reads the official models collection, the models Kaeso maps; a model
        // it leaves out may still run.
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("Replicate"),
        responses: ResponsesProfile::compatible("Replicate"),
        messages: MessagesProfile::compatible("Replicate"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "deepgram",
        name: "Deepgram",
        // Keys are made in the Deepgram Console under a project's API Keys and work on every
        // regional host (https://developers.deepgram.com/reference/authentication,
        // https://developers.deepgram.com/reference/regional-endpoints).
        key_page: KeyPage {
            url: "https://console.deepgram.com",
            label: "API Keys",
        },
        key_prefix: None,
        default_base_url: "https://api.deepgram.com",
        auth: AuthStyle::Token,
        // Deepgram transcribes and speaks; it writes no text, so a generation request never
        // reaches it past the catalogue's checks.
        generation: DialectId::Deepgram,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[SpeechEgress {
            dialect: DialectId::Deepgram,
            ..SpeechEgress::ANY
        }],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::Deepgram,
            ..TranscriptionsEgress::ANY
        }],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::Deepgram],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // `GET /v1/models` lists each voice and model variant (`aura-2-thalia-en`,
        // `nova-3-general`), not the families Kaeso offers (`aura-2`, `nova-3`), and answers
        // without a key.
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("Deepgram"),
        responses: ResponsesProfile::compatible("Deepgram"),
        messages: MessagesProfile::compatible("Deepgram"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "cartesia",
        name: "Cartesia",
        // Keys (`sk_car_…`) are made at play.cartesia.ai; admin keys (`sk_car_admin_…`) are
        // refused on speech and transcription (https://docs.cartesia.ai/use-the-api/api-conventions).
        key_page: KeyPage {
            url: "https://play.cartesia.ai/keys",
            label: "API Keys",
        },
        key_prefix: Some("sk_car_"),
        default_base_url: "https://api.cartesia.ai",
        // "Cartesia accepts both `X-Api-Key` and `Authorization` headers"
        // (https://docs.cartesia.ai/get-started/authenticate-your-client-applications).
        auth: AuthStyle::XApiKey,
        // Cartesia speaks and transcribes; it writes no text, so a generation request never
        // reaches it past the catalogue's checks.
        generation: DialectId::Cartesia,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[SpeechEgress {
            dialect: DialectId::Cartesia,
            ..SpeechEgress::ANY
        }],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::Cartesia,
            ..TranscriptionsEgress::ANY
        }],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::Cartesia],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // Cartesia lists no models; the model check reads one voice, which only shows that the
        // key works.
        listing: ModelListing::Catalogue,
        end_user_limit: None,
        chat: ChatProfile::compatible("Cartesia"),
        responses: ResponsesProfile::compatible("Cartesia"),
        messages: MessagesProfile::compatible("Cartesia"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "qianfan",
        name: "Baidu Qianfan",
        // API keys (`bce-v3/ALTAK-…/…`) are made in the Baidu AI Cloud console under 安全认证 →
        // API Key (https://cloud.baidu.com/doc/qianfan-api/s/vmwbl43ut). No key prefix: the `/`
        // inside the key would leave its secret part out of redaction.
        key_page: KeyPage {
            url: "https://console.bce.baidu.com/iam/#/iam/apikey/list",
            label: "API Key",
        },
        key_prefix: None,
        // Qianfan's API sits at `/v2`, not `/v1`: `…/v2/chat/completions`, `…/v2/embeddings`,
        // `…/v2/models` (https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4); `/v1/…` and
        // `/v2/v1/…` answer 404 (recorded 2026-11-14). The base URL ends at that root, so the
        // international service's `https://api.baiduqianfan.ai/v1` works as a base-URL override.
        default_base_url: "https://qianfan.baidubce.com/v2",
        auth: AuthStyle::Bearer,
        generation: DialectId::OpenAiChat,
        count_tokens: None,
        // `/v2/embeddings` takes no `dimensions`; its models' sizes are fixed
        // (https://cloud.baidu.com/doc/qianfan-api/s/qmwbl45cf).
        embeddings: Some(DialectId::OpenAiEmbeddings),
        embeddings_profile: EmbeddingsProfile::compatible("Baidu Qianfan").without_dimensions(),
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[],
        videos: &[],
        decisions: &[],
        dialects: &[
            DialectId::OpenAiChat,
            DialectId::AnthropicMessages,
            DialectId::OpenAiEmbeddings,
        ],
        // The Messages API at the host's `/anthropic/v1/messages`, every chat model
        // (https://cloud.baidu.com/doc/qianfan-docs/s/6mh3e6gjp, read 2026-11-25).
        native: &[NativeDialect::of(DialectId::AnthropicMessages).from_host_root("/anthropic/v1")],
        refused_key_messages: &[],
        context_window_messages: &[],
        // `max_tokens` and `max_completion_tokens` both count the answer and the chain of thought
        // (https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4).
        reasoning_beyond_output_limit: false,
        // `GET /v2/models` needs a key and lists the account's models in Qianfan's own shape, not
        // the services an account deploys under names of its own
        // (https://cloud.baidu.com/doc/qianfan-api/s/Amwbl43sp).
        listing: ModelListing::Partial,
        end_user_limit: None,
        // From Qianfan's chat reference (https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4 and
        // the vision page https://cloud.baidu.com/doc/qianfan-api/s/ymwbl48ky, updated
        // 2026-11-11):
        // - roles `system`, `user`, `assistant` and `tool`; no `developer`;
        // - `user` names the end user;
        // - thinking is switched with `thinking: {"type": "enabled" | "disabled"}` and graded by
        //   `reasoning_effort` (https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv);
        // - `video_url` parts and `reasoning_content` in replies and deltas;
        // - none of the refused fields. Whether Qianfan refuses fields it doesn't know isn't
        //   documented, so Kaeso refuses them rather than risk Qianfan dropping them.
        chat: ChatProfile::compatible("Baidu Qianfan")
            .system_role()
            .end_user_as_user()
            .thinking_type()
            .with_video_url()
            .reasoning_fields(&["reasoning_content"])
            .refusing(&[
                "audio",
                "function_call",
                "functions",
                "logit_bias",
                "logprobs",
                "modalities",
                "prediction",
                "prompt_cache_key",
                "prompt_cache_retention",
                "service_tier",
                "store",
                "top_logprobs",
                "verbosity",
                "web_search_options",
            ]),
        responses: ResponsesProfile::compatible("Baidu Qianfan"),
        messages: MessagesProfile::compatible("Baidu Qianfan"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: Some(""),
        regional: None,
        key_endpoint: None,
    },
    Provider {
        id: "assemblyai",
        name: "AssemblyAI",
        // Keys are made in the dashboard under API Keys; AssemblyAI documents no prefix
        // (https://www.assemblyai.com/docs/api-reference/overview).
        key_page: KeyPage {
            url: "https://www.assemblyai.com/dashboard/home",
            label: "API Keys",
        },
        key_prefix: None,
        // The Sync API: one request per transcript, audio of up to 20 minutes and 256 MB, a
        // 30-second deadline. `https://sync.eu.assemblyai.com` keeps the audio in the EU
        // (https://www.assemblyai.com/docs/api-reference/sync-api/transcribe).
        default_base_url: "https://sync.assemblyai.com",
        // "Optionally prefixed with `Bearer `" (the Sync API's OpenAPI `Authorization`).
        auth: AuthStyle::Bearer,
        // AssemblyAI transcribes; it writes no text, so a generation request never reaches it
        // past the catalogue's checks.
        generation: DialectId::AssemblyAi,
        count_tokens: None,
        embeddings: None,
        embeddings_profile: EmbeddingsProfile::OPENAI,
        rerank: None,
        images: &[],
        speech: &[],
        transcriptions: &[TranscriptionsEgress {
            dialect: DialectId::AssemblyAi,
            ..TranscriptionsEgress::ANY
        }],
        videos: &[],
        decisions: &[],
        dialects: &[DialectId::AssemblyAi],
        native: &[],
        refused_key_messages: &[],
        context_window_messages: &[],
        reasoning_beyond_output_limit: false,
        // The Sync API lists no models and has no request that checks a key, so a model check
        // stays inconclusive.
        listing: ModelListing::Partial,
        end_user_limit: None,
        chat: ChatProfile::compatible("AssemblyAI"),
        responses: ResponsesProfile::compatible("AssemblyAI"),
        messages: MessagesProfile::compatible("AssemblyAI"),
        gemini: GeminiProfile::GOOGLE_AI_STUDIO,
        account: None,
        routes: &[],
        service_account: None,
        account_scope: None,
        api_root: None,
        regional: None,
        key_endpoint: None,
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

    /// The provider's own API for `dialect`, when it serves `model` there besides its
    /// `generation` dialect.
    pub fn native(&self, dialect: DialectId, model: &str) -> Option<&NativeDialect> {
        self.native
            .iter()
            .find(|n| n.dialect == dialect && n.serves(model))
    }

    /// What takes the place of `/v1` in `dialect`'s paths: the dialect's own root where it has
    /// one, else the provider's.
    pub fn api_root_for(&self, dialect: DialectId) -> Option<&'static str> {
        self.native
            .iter()
            .find(|n| n.dialect == dialect)
            .and_then(|n| n.api_root)
            .or(self.api_root)
    }

    /// The auth header `dialect` takes at the provider.
    pub fn auth_for(&self, dialect: DialectId) -> AuthStyle {
        self.native
            .iter()
            .find(|n| n.dialect == dialect)
            .and_then(|n| n.auth)
            .unwrap_or(self.auth)
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
    use kaeso_codecs::openai_chat::{EndUserField, OutputLimit};
    use kaeso_codecs::openai_responses::{InputItems, ReasoningReplay, WebSearchTool};
    use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
    use kaeso_kpp::{Request, RequestKind};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_native_dialect_is_one_of_the_providers_dialects_beside_its_generation() {
        for p in PROVIDERS {
            for n in p.native {
                assert!(p.dialects.contains(&n.dialect), "{}: {:?}", p.id, n.dialect);
                assert_ne!(p.generation, n.dialect, "{}", p.id);
                assert!(
                    p.native.iter().filter(|o| o.dialect == n.dialect).count() == 1,
                    "{}: one entry per dialect",
                    p.id
                );
                // A key's own host, region or account path decides where its requests go, so
                // native APIs stay with providers whose base URL alone does.
                assert!(
                    p.regional.is_none()
                        && p.key_endpoint.is_none()
                        && p.account_scope.is_none()
                        && p.account.is_none(),
                    "{}",
                    p.id
                );
                assert!(n.from_root <= n.api_root.is_some(), "{}", p.id);
            }
        }
    }

    #[test]
    fn deepseek_serves_anthropic_apps_at_its_own_messages_api() {
        let deepseek = PROVIDERS.iter().find(|p| p.id == "deepseek").unwrap();
        let native = deepseek
            .native(DialectId::AnthropicMessages, "deepseek-flash")
            .unwrap();
        assert!(!native.counts_tokens);
        assert_eq!(
            deepseek.api_root_for(DialectId::AnthropicMessages),
            Some("/anthropic/v1")
        );
        assert_eq!(deepseek.api_root_for(DialectId::OpenAiChat), None);
        assert_eq!(
            deepseek.auth_for(DialectId::AnthropicMessages),
            AuthStyle::Bearer
        );
        assert_eq!(
            deepseek.native(DialectId::OpenAiResponses, "deepseek-flash"),
            None
        );
        let moonshot = PROVIDERS.iter().find(|p| p.id == "moonshotai").unwrap();
        assert!(
            moonshot
                .native(DialectId::AnthropicMessages, "kimi-k3")
                .is_some()
        );
        assert!(
            moonshot
                .native(DialectId::AnthropicMessages, "kimi-k3.1-pro")
                .is_some()
        );
        assert_eq!(
            moonshot.native(DialectId::AnthropicMessages, "kimi-k2.6"),
            None
        );
        let qwen = PROVIDERS.iter().find(|p| p.id == "alibaba").unwrap();
        let native = qwen
            .native(DialectId::AnthropicMessages, "qwen3.7-plus")
            .unwrap();
        assert!(native.from_root);
        assert_eq!(native.api_root, Some("/apps/anthropic/v1"));
        assert_eq!(
            qwen.native(DialectId::AnthropicMessages, "wan2.6-t2v"),
            None
        );
    }

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
            "openai, anthropic, deepseek, xai, togetherai, groq, openrouter, mistral, vercel, cohere, ollama-cloud, moonshotai, alibaba, cerebras, zai, minimax, elevenlabs, google, gemini-agent-platform, fireworks-ai, nebius, sambanova, scaleway, huggingface, nvidia, baseten, siliconflow, standardcompute, cloudflare-ai-gateway, perplexity, typesafe, deepinfra, upstage, ionos, novita-ai, volcengine, ovhcloud, stepfun-ai, tencent-tokenhub, venice, amazon-bedrock, azure, voyage, jina, fal, replicate, deepgram, cartesia, qianfan, assemblyai"
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
    fn standard_compute_speaks_chat_completions_with_a_bearer_key_and_counts_no_tokens() {
        let sc = provider("standardcompute").unwrap();
        assert_eq!(
            (sc.name, sc.auth, sc.generation, sc.dialects),
            (
                "Standard Compute",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat][..]
            )
        );
        assert_eq!(sc.default_base_url, "https://api.stdcmpt.com");
        assert_eq!(
            (sc.count_tokens, sc.embeddings, sc.key_prefix),
            (None, None, None)
        );
        assert_eq!(
            sc.key_page.url,
            "https://standardcompute.com/dashboard/api-keys"
        );
        assert!(sc.images.is_empty() && sc.speech.is_empty() && sc.videos.is_empty());
        assert_eq!(sc.chat.reply.reasoning_fields, ["reasoning"]);
        assert_eq!(sc.listing, ModelListing::Partial);
        let partial: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.listing == ModelListing::Partial)
            .map(|p| p.id)
            .collect();
        assert_eq!(
            partial,
            [
                "standardcompute",
                "perplexity",
                "typesafe",
                "upstage",
                "novita-ai",
                "volcengine",
                "venice",
                "amazon-bedrock",
                "voyage",
                "jina",
                "replicate",
                "deepgram",
                "qianfan",
                "assemblyai"
            ]
        );
    }

    #[test]
    fn perplexity_takes_sonar_chat_at_its_own_path_and_refuses_what_its_api_reference_leaves_out() {
        let p = provider("perplexity").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Perplexity",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.perplexity.ai");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, None, Some("pplx-"))
        );
        assert_eq!(p.key_page.url, "https://console.perplexity.ai/project/keys");
        assert_eq!(p.listing, ModelListing::Partial);
        assert_eq!(p.chat.provider, "Perplexity");
        assert_eq!(p.chat.request.path, "/v1/sonar");
        assert!(!p.chat.request.stream_options && !p.chat.request.end_user);
        for field in ["tools", "tool_choice", "logprobs", "seed", "stream_mode"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        let listed = ["perplexity/sonar".to_owned()];
        assert_eq!(p.listing.serves(&listed, "sonar-pro"), None);
    }

    #[test]
    fn deepgram_transcribes_and_speaks_in_its_own_api_with_a_token_key() {
        let p = provider("deepgram").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Deepgram",
                AuthStyle::Token,
                DialectId::Deepgram,
                &[DialectId::Deepgram][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.deepgram.com");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, None, None)
        );
        assert_eq!(p.key_page.url, "https://console.deepgram.com");
        assert_eq!(p.listing, ModelListing::Partial);
        let r = crate::providers::ProviderRef::BuiltIn(p);
        for model in ["nova-3", "whisper-large", "anything"] {
            let t = r.transcriptions(model).unwrap();
            assert_eq!(
                (t.dialect, t.from_root, t.auth),
                (DialectId::Deepgram, false, None)
            );
        }
        for model in ["aura-2", "aura"] {
            let s = r.speech(model).unwrap();
            assert_eq!(
                (s.dialect, s.from_root, s.auth),
                (DialectId::Deepgram, false, None)
            );
            assert!(s.fetch_hosts.is_empty());
        }
        assert!(p.images.is_empty() && p.videos.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn deepinfra_serves_chat_and_embeddings_under_its_host_and_refuses_what_its_schema_leaves_out()
    {
        let p = provider("deepinfra").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "DeepInfra",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[
                    DialectId::OpenAiChat,
                    DialectId::AnthropicMessages,
                    DialectId::OpenAiEmbeddings
                ][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.deepinfra.com");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(p.key_page.url, "https://deepinfra.com/dash/keys");
        assert_eq!(p.listing, ModelListing::Complete);
        assert_eq!(p.chat.provider, "DeepInfra");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::MaxTokens);
        assert_eq!(p.chat.request.end_user_field, EndUserField::User);
        assert!(!p.chat.request.developer_role && p.chat.request.video_url);
        assert!(p.chat.request.cache_key && p.chat.request.stream_options);
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        for field in ["parallel_tool_calls", "top_logprobs", "logit_bias", "store"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn stepfun_serves_chat_under_its_host_and_refuses_what_its_chat_reference_leaves_out() {
        let p = provider("stepfun-ai").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "StepFun",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.stepfun.ai");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, None, None)
        );
        assert_eq!(
            (p.key_page.url, p.key_page.label),
            (
                "https://platform.stepfun.ai/interface-key",
                "Interface Keys"
            )
        );
        assert_eq!(p.listing, ModelListing::Complete);
        assert_eq!(p.chat.provider, "StepFun");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::MaxTokens);
        assert!(!p.chat.request.developer_role && p.chat.request.video_url);
        assert!(!p.chat.request.end_user && !p.chat.request.cache_key);
        assert!(!p.chat.request.stream_options);
        assert_eq!(
            p.chat.reply.reasoning_fields,
            ["reasoning", "reasoning_content"]
        );
        for field in [
            "parallel_tool_calls",
            "presence_penalty",
            "logprobs",
            "seed",
            "store",
        ] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(!p.chat.request.refused_fields.contains(&"tool_choice"));
        assert_eq!(p.chat.request.refused_message_fields, ["name"]);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn ovhcloud_serves_chat_and_embeddings_under_its_host_and_refuses_what_its_schema_leaves_out() {
        let p = provider("ovhcloud").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "OVHcloud AI Endpoints",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(
            p.default_base_url,
            "https://oai.endpoints.kepler.ai.cloud.ovh.net"
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(p.listing, ModelListing::Complete);
        let refusal = kaeso_codecs::dispatch::decode_error(
            DialectId::OpenAiChat,
            403,
            &HeaderList::new(),
            br#"{"message":"Forbidden: authentication failed.  Please generate a new one at https://example.com"}"#,
        );
        assert!(p.refuses_key(&refusal.message), "{}", refusal.message);
        assert_eq!(p.chat.provider, "OVHcloud AI Endpoints");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert!(!p.chat.request.developer_role && !p.chat.request.end_user);
        assert!(!p.chat.request.cache_key && !p.chat.request.video_url);
        assert!(p.chat.request.stream_options && !p.chat.request.verbosity);
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning"]);
        for field in ["metadata", "store", "prediction", "prompt_cache_key"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        for field in [
            "service_tier",
            "logit_bias",
            "top_logprobs",
            "parallel_tool_calls",
        ] {
            assert!(!p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn novita_ai_serves_chat_and_embeddings_under_openai_and_asks_for_reasoning_in_its_own_field() {
        let p = provider("novita-ai").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Novita AI",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.novita.ai/openai");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), Some("sk_"))
        );
        assert_eq!(p.key_page.url, "https://novita.ai/settings/key-management");
        assert_eq!(p.listing, ModelListing::Partial);
        assert_eq!(p.chat.provider, "Novita AI");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::MaxTokens);
        assert!(!p.chat.request.end_user && !p.chat.request.developer_role);
        assert!(p.chat.request.video_url && p.chat.request.thinking_switch);
        assert!(p.chat.request.separate_reasoning && !p.chat.request.cache_key);
        assert!(!p.embeddings_profile.dimensions);
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        for field in [
            "parallel_tool_calls",
            "prompt_cache_key",
            "store",
            "metadata",
        ] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn ionos_serves_chat_and_embeddings_from_berlin_with_system_messages_and_its_reasoning_fields()
    {
        let p = provider("ionos").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "IONOS AI Model Hub",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(
            p.default_base_url,
            "https://openai.inference.de-txl.ionos.com"
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(
            (p.key_page.url, p.key_page.label),
            ("https://dcd.ionos.com", "Management → Token Manager")
        );
        assert_eq!(p.chat.provider, "IONOS AI Model Hub");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert_eq!(p.chat.request.end_user_field, EndUserField::User);
        assert!(!p.chat.request.developer_role && p.chat.request.video_url);
        assert!(!p.chat.request.cache_key && p.chat.request.stream_options);
        assert_eq!(
            p.chat.reply.reasoning_fields,
            ["reasoning", "reasoning_content"]
        );
        for field in ["service_tier", "store", "metadata", "web_search_options"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(!p.chat.request.refused_fields.contains(&"logit_bias"));
        assert!(p.images.is_empty() && p.speech.is_empty() && p.transcriptions.is_empty());
    }

    #[test]
    fn upstage_serves_chat_and_embeddings_under_its_host_and_refuses_what_its_schema_leaves_out() {
        let p = provider("upstage").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Upstage",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.upstage.ai");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(p.key_page.url, "https://console.upstage.ai/api-keys");
        assert_eq!(p.listing, ModelListing::Partial);
        assert_eq!(p.chat.provider, "Upstage");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::MaxTokens);
        assert!(!p.chat.request.developer_role && !p.chat.request.end_user);
        assert!(!p.chat.request.video_url);
        assert!(p.chat.request.cache_key && p.chat.request.stream_options);
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning"]);
        for field in ["stop", "seed", "logprobs", "logit_bias", "store"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(
            !p.chat
                .request
                .refused_fields
                .contains(&"parallel_tool_calls")
        );
        assert_eq!(p.embeddings_profile.provider, "Upstage");
        assert!(!p.embeddings_profile.dimensions);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn volcengine_serves_chat_at_its_api_root_and_switches_thinking_with_its_thinking_object() {
        let p = provider("volcengine").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Volcengine Ark",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.api_root),
            ("https://ark.cn-beijing.volces.com/api/v3", Some(""))
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, None, None)
        );
        assert_eq!(p.listing, ModelListing::Partial);
        assert!(p.reasoning_beyond_output_limit);
        assert_eq!(p.chat.provider, "Volcengine Ark");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert!(p.chat.request.developer_role && !p.chat.request.end_user);
        assert!(p.chat.request.thinking_type && !p.chat.request.thinking_switch);
        assert!(p.chat.request.video_url && p.chat.request.cache_key);
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        assert_eq!(p.chat.reply.encrypted_reasoning, Some("encrypted_content"));
        for field in ["seed", "n", "store", "web_search_options.user_location"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert_eq!(
            p.chat.request.refused_values,
            [("service_tier", &["priority", "scale"][..])]
        );
        // Every other provider but Baidu Qianfan, whose API sits at `/v2`, sends its dialects'
        // usual `/v1` paths.
        assert_eq!(
            PROVIDERS
                .iter()
                .filter(|o| o.api_root.is_some())
                .map(|o| o.id)
                .collect::<Vec<_>>(),
            ["volcengine", "qianfan"]
        );

        let req = Request::Generate(
            kaeso_kpp::GenerateRequest::new("doubao-seed-2-1-pro-260915")
                .with_message(kaeso_kpp::Message::user("Hello")),
        );
        let mut ctx = EgressCtx::new(&req)
            .with_chat_profile(p.chat)
            .with_api_root(p.api_root.unwrap());
        let sent = dispatch::encode_request(DialectId::OpenAiChat, &req, &mut ctx).unwrap();
        assert_eq!(sent.path, "/chat/completions");
    }

    #[test]
    fn tencent_tokenhub_serves_chat_and_embeddings_under_v1_and_refuses_what_its_reference_leaves_out()
     {
        let p = provider("tencent-tokenhub").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Tencent TokenHub",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[
                    DialectId::OpenAiChat,
                    DialectId::AnthropicMessages,
                    DialectId::OpenAiEmbeddings
                ][..]
            )
        );
        assert_eq!(p.default_base_url, "https://tokenhub.tencentmaas.com");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(
            p.key_page.url,
            "https://console.cloud.tencent.com/tokenhub/apikey"
        );
        assert_eq!(p.listing, ModelListing::Complete);
        assert_eq!(p.chat.provider, "Tencent TokenHub");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert_eq!(
            p.chat.request.end_user_field,
            EndUserField::SafetyIdentifier
        );
        assert!(!p.chat.request.developer_role && p.chat.request.video_url);
        assert!(p.chat.request.cache_key && p.chat.request.stream_options);
        assert!(!p.chat.request.verbosity);
        assert_eq!(
            p.embeddings_profile,
            EmbeddingsProfile::compatible("Tencent TokenHub").without_dimensions()
        );
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        for field in ["logit_bias", "store", "service_tier", "modalities", "audio"] {
            assert!(p.chat.request.refused_fields.contains(&field), "{field}");
        }
        for field in ["parallel_tool_calls", "top_logprobs", "web_search_options"] {
            assert!(!p.chat.request.refused_fields.contains(&field), "{field}");
        }
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn venice_serves_chat_and_embeddings_under_its_api_root_without_its_own_system_prompt() {
        let p = provider("venice").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Venice",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[DialectId::OpenAiChat, DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.venice.ai/api");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(p.embeddings_profile, EmbeddingsProfile::OPENAI);
        assert_eq!(p.key_page.url, "https://venice.ai/settings/api");
        assert_eq!(p.listing, ModelListing::Partial);
        assert_eq!(p.chat.provider, "Venice");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert!(p.chat.request.developer_role && p.chat.request.video_url);
        assert!(!p.chat.request.end_user && p.chat.request.input_audio_formats.is_none());
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        assert_eq!(p.chat.request.refused_fields, ["metadata", "store"]);
        assert_eq!(
            p.chat.request.option_defaults,
            [OptionDefault::new(
                "venice_parameters",
                "include_venice_system_prompt",
                "false"
            )]
        );
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn amazon_bedrock_calls_each_keys_region_with_chat_completions_messages_and_converse_and_counts_no_tokens()
     {
        let p = provider("amazon-bedrock").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Amazon Bedrock",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[
                    DialectId::OpenAiChat,
                    DialectId::AnthropicMessages,
                    DialectId::BedrockConverse
                ][..]
            )
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix, p.listing),
            (None, None, None, ModelListing::Partial)
        );
        assert!(p.refuses_key("Invalid API Key format: Must start with pre-defined prefix"));
        assert!(p.refuses_key("Invalid API Key format: Delimiter ':' not found"));
        assert!(p.refuses_key("Missing required parameters in the API Key"));
        assert!(p.refuses_key("Authentication failed: Please make sure your API Key is valid."));
        assert!(!p.refuses_key("Too many requests, please wait before trying again."));
        assert_eq!(p.messages.refused_fields, ["mcp_servers", "container"]);
        assert!(p.routes.is_empty() && p.account_scope.is_none());
        let r = p.regional.unwrap();
        assert_eq!(r.regions.len(), 22);
        assert!(r.serves("eu-central-1") && r.serves("us-gov-west-1"));
        assert!(!r.serves("us-west-1") && !r.serves("global") && !r.serves("EU-CENTRAL-1"));
        assert_eq!(
            r.base_url("eu-central-1"),
            "https://bedrock-runtime.eu-central-1.amazonaws.com"
        );
        assert_eq!(p.default_base_url, r.base_url("us-east-1"));
        assert_eq!(r.api_root(DialectId::OpenAiChat), Some("/openai/v1"));
        assert_eq!(
            r.api_root(DialectId::AnthropicMessages),
            Some("/anthropic/v1")
        );
        assert_eq!(r.api_root(DialectId::BedrockConverse), Some(""));
        assert_eq!(r.api_root(DialectId::OpenAiResponses), None);
        assert_eq!(
            r.auth(DialectId::AnthropicMessages),
            Some(AuthStyle::XApiKey)
        );
        assert_eq!(r.auth(DialectId::OpenAiChat), None);
        let regional: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.regional.is_some())
            .map(|p| p.id)
            .collect();
        assert_eq!(regional, ["amazon-bedrock"]);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn azure_calls_each_keys_resource_with_an_api_key_header_and_sends_chat_as_openai_writes_it() {
        let p = provider("azure").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Azure",
                AuthStyle::ApiKey,
                DialectId::OpenAiChat,
                &[
                    DialectId::OpenAiChat,
                    DialectId::OpenAiResponses,
                    DialectId::OpenAiEmbeddings
                ][..]
            )
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        assert_eq!(p.listing, ModelListing::Catalogue);
        assert_eq!(p.chat.provider, "Azure");
        assert_eq!(p.chat.request.path, "/v1/chat/completions");
        assert_eq!(p.chat.request.output_limit, OutputLimit::AsWritten);
        assert_eq!(
            p.chat.request.end_user_field,
            EndUserField::SafetyIdentifier
        );
        assert!(p.chat.request.developer_role && p.chat.request.verbosity);
        assert!(p.chat.request.cache_key && p.chat.request.stream_options);
        assert!(p.chat.request.refused_fields.is_empty());
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
        let endpoint = p.key_endpoint.unwrap();
        assert_eq!(
            endpoint.base("https://example.openai.azure.com"),
            "https://example.openai.azure.com/openai"
        );
        let scoped: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.key_endpoint.is_some())
            .map(|p| p.id)
            .collect();
        assert_eq!(scoped, ["azure"]);
    }

    #[test]
    fn an_azure_endpoint_is_an_https_resource_host_and_loses_a_pasted_api_path() {
        let endpoint = provider("azure").unwrap().key_endpoint.unwrap();
        for (pasted, kept) in [
            (
                "https://example.openai.azure.com",
                "https://example.openai.azure.com",
            ),
            (
                " https://Example.OpenAI.Azure.com/ ",
                "https://example.openai.azure.com",
            ),
            (
                "https://example.openai.azure.com/openai/v1/",
                "https://example.openai.azure.com",
            ),
            (
                "https://example.openai.azure.com/openai",
                "https://example.openai.azure.com",
            ),
            (
                "https://example.services.ai.azure.com/openai/v1",
                "https://example.services.ai.azure.com",
            ),
            (
                "https://example.cognitiveservices.azure.com",
                "https://example.cognitiveservices.azure.com",
            ),
        ] {
            assert_eq!(endpoint.parse(pasted).as_deref(), Ok(kept), "{pasted}");
        }
        for (pasted, problem) in [
            ("example.openai.azure.com", EndpointProblem::NotUrl),
            ("http://example.openai.azure.com", EndpointProblem::NotHttps),
            (
                "https://user:pw@example.openai.azure.com",
                EndpointProblem::UserInfo,
            ),
            (
                "https://example.openai.azure.com/?api-version=v1",
                EndpointProblem::Query,
            ),
            (
                "https://example.openai.azure.com/#keys",
                EndpointProblem::Query,
            ),
            ("https://example.com", EndpointProblem::Host),
            ("https://openai.azure.com", EndpointProblem::Host),
            (
                "https://example.openai.azure.com.example.com",
                EndpointProblem::Host,
            ),
            (
                "https://example.openai.azure.com:8443",
                EndpointProblem::Host,
            ),
            ("https://192.0.2.1", EndpointProblem::Host),
            (
                "https://under_score.openai.azure.com",
                EndpointProblem::Host,
            ),
            ("https://.openai.azure.com", EndpointProblem::Host),
            (
                "https://example.openai.azure.com/anthropic",
                EndpointProblem::Path,
            ),
            (
                "https://example.services.ai.azure.com/api/projects/p",
                EndpointProblem::Path,
            ),
        ] {
            assert_eq!(endpoint.parse(pasted), Err(problem), "{pasted}");
        }
    }

    #[test]
    fn a_catalogue_listing_says_nothing_of_any_model() {
        let ids = ["gpt-5".to_owned()];
        assert_eq!(ModelListing::Catalogue.serves(&ids, "gpt-5"), None);
        assert_eq!(ModelListing::Catalogue.serves(&ids, "my-gpt"), None);
    }

    #[test]
    fn fal_makes_images_and_videos_through_its_queue_with_a_key_header() {
        let p = provider("fal").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            ("fal", AuthStyle::Key, DialectId::Fal, &[DialectId::Fal][..])
        );
        assert_eq!(p.default_base_url, "https://queue.fal.run");
        assert_eq!(
            (p.auth.header_name(), p.auth.header_value("k-1").as_str()),
            ("authorization", "Key k-1")
        );
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, None, None)
        );
        assert_eq!(p.key_page.url, "https://fal.ai/dashboard/keys");
        for model in kaeso_codecs::fal::IMAGE_MODELS {
            let images = p.images.iter().find(|e| e.serves(model)).unwrap();
            assert_eq!(images.dialect, DialectId::Fal, "{model}");
            assert!(!images.from_root && images.auth.is_none() && images.fetch_hosts.is_empty());
        }
        for model in kaeso_codecs::fal::VIDEO_MODELS {
            let videos = p.videos.iter().find(|e| e.serves(model)).unwrap();
            assert_eq!(videos.dialect, DialectId::Fal, "{model}");
        }
        assert!(p.speech.is_empty() && p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn replicate_makes_images_and_videos_as_predictions_and_fetches_its_hour_long_links() {
        let p = provider("replicate").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Replicate",
                AuthStyle::Bearer,
                DialectId::Replicate,
                &[DialectId::Replicate][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.key_prefix, p.listing),
            (
                "https://api.replicate.com",
                Some("r8_"),
                ModelListing::Partial
            )
        );
        assert_eq!(p.key_page.url, "https://replicate.com/account/api-tokens");
        assert_eq!((p.count_tokens, p.embeddings), (None, None));
        for model in kaeso_codecs::replicate::IMAGE_MODELS {
            let images = p.images.iter().find(|e| e.serves(model)).unwrap();
            assert_eq!(images.dialect, DialectId::Replicate, "{model}");
            assert_eq!(images.fetch_hosts, kaeso_codecs::replicate::OUTPUT_HOSTS);
            for host in ["replicate.delivery", "pbxt.replicate.delivery"] {
                assert!(fetchable(images.fetch_hosts, host), "{host}");
            }
            assert!(!fetchable(
                images.fetch_hosts,
                "replicate.delivery.example.com"
            ));
        }
        for model in kaeso_codecs::replicate::VIDEO_MODELS {
            let videos = p.videos.iter().find(|e| e.serves(model)).unwrap();
            assert_eq!(videos.dialect, DialectId::Replicate, "{model}");
        }
        assert!(p.speech.is_empty() && p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn cartesia_speaks_and_transcribes_in_its_own_dialect_with_a_key_header() {
        let p = provider("cartesia").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Cartesia",
                AuthStyle::XApiKey,
                DialectId::Cartesia,
                &[DialectId::Cartesia][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.key_prefix, p.listing),
            (
                "https://api.cartesia.ai",
                Some("sk_car_"),
                ModelListing::Catalogue
            )
        );
        assert_eq!(p.key_page.url, "https://play.cartesia.ai/keys");
        let speech = p.speech.iter().find(|e| e.serves("sonic-3.6")).unwrap();
        assert_eq!(speech.dialect, DialectId::Cartesia);
        let stt = p
            .transcriptions
            .iter()
            .find(|e| e.serves("ink-whisper"))
            .unwrap();
        assert_eq!(stt.dialect, DialectId::Cartesia);
        assert!(p.images.is_empty() && p.videos.is_empty() && p.embeddings.is_none());
    }

    #[test]
    fn voyage_embeds_and_reranks_at_mongodb_with_its_own_field_names_and_writes_no_text() {
        let p = provider("voyage").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Voyage AI",
                AuthStyle::Bearer,
                DialectId::OpenAiEmbeddings,
                &[DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(p.default_base_url, "https://ai.mongodb.com");
        assert_eq!(
            (p.count_tokens, p.embeddings, p.key_prefix),
            (None, Some(DialectId::OpenAiEmbeddings), None)
        );
        let e = p.embeddings_profile;
        assert_eq!(
            (e.provider, e.dimensions_field),
            ("Voyage AI", "output_dimension")
        );
        assert_eq!(e.dimension_sizes, [256, 512, 1024, 2048]);
        assert!(e.dimensions && !e.user && !e.float_encoding && !e.token_inputs);
        assert_eq!(
            p.rerank,
            Some(RerankEgress {
                dialect: DialectId::VoyageRerank,
                from_root: false
            })
        );
        assert_eq!(p.listing, ModelListing::Partial);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn jina_embeds_with_embedding_type_and_reranks_with_its_own_format_and_writes_no_text() {
        let p = provider("jina").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Jina AI",
                AuthStyle::Bearer,
                DialectId::OpenAiEmbeddings,
                &[DialectId::OpenAiEmbeddings][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.key_prefix),
            ("https://api.jina.ai", Some("jina_"))
        );
        assert_eq!(
            (p.count_tokens, p.embeddings),
            (None, Some(DialectId::OpenAiEmbeddings))
        );
        let e = p.embeddings_profile;
        assert_eq!(
            (e.provider, e.encoding_field, e.dimensions_field),
            ("Jina AI", "embedding_type", "dimensions")
        );
        assert!(e.dimensions && e.float_encoding && !e.user && !e.token_inputs);
        assert_eq!(p.rerank, Some(RerankEgress::of(DialectId::JinaRerank)));
        assert_eq!(p.listing, ModelListing::Partial);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
        assert!(p.transcriptions.is_empty() && p.decisions.is_empty());
    }

    #[test]
    fn qianfan_speaks_chat_completions_at_v2_with_its_own_roles_and_thinking_switch() {
        let p = provider("qianfan").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "Baidu Qianfan",
                AuthStyle::Bearer,
                DialectId::OpenAiChat,
                &[
                    DialectId::OpenAiChat,
                    DialectId::AnthropicMessages,
                    DialectId::OpenAiEmbeddings
                ][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.api_root, p.key_prefix),
            ("https://qianfan.baidubce.com/v2", Some(""), None)
        );
        assert_eq!(
            (p.count_tokens, p.embeddings),
            (None, Some(DialectId::OpenAiEmbeddings))
        );
        assert!(!p.embeddings_profile.dimensions);
        assert_eq!(p.listing, ModelListing::Partial);
        assert_eq!(p.chat.provider, "Baidu Qianfan");
        assert_eq!(p.chat.reply.reasoning_fields, ["reasoning_content"]);
        assert!(!p.chat.request.developer_role && p.chat.request.end_user);
        assert!(p.images.is_empty() && p.speech.is_empty() && p.videos.is_empty());
    }

    #[test]
    fn assemblyai_transcribes_through_its_sync_api_in_its_own_dialect() {
        let p = provider("assemblyai").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "AssemblyAI",
                AuthStyle::Bearer,
                DialectId::AssemblyAi,
                &[DialectId::AssemblyAi][..]
            )
        );
        assert_eq!(
            (p.default_base_url, p.key_prefix, p.listing),
            ("https://sync.assemblyai.com", None, ModelListing::Partial)
        );
        let stt = p
            .transcriptions
            .iter()
            .find(|e| e.serves("universal-3-6-pro"))
            .unwrap();
        assert_eq!(stt.dialect, DialectId::AssemblyAi);
        assert!(p.speech.is_empty() && p.images.is_empty() && p.embeddings.is_none());
    }

    #[test]
    fn a_partial_listing_matches_ids_without_case_or_dots_and_says_nothing_of_a_model_it_leaves_out()
     {
        let ids = ["StandardCompute".to_owned(), "z-ai/glm-5.3".to_owned()];
        assert_eq!(
            ModelListing::Partial.serves(&ids, "standardcompute"),
            Some(true)
        );
        assert_eq!(
            ModelListing::Partial.serves(&ids, "openai/gpt-5.6-luna:eu"),
            None
        );
        let listed = ["anthropic/claude-haiku-5-5".to_owned()];
        assert_eq!(
            ModelListing::Partial.serves(&listed, "anthropic/claude-haiku-5.5"),
            Some(true)
        );
        assert_eq!(
            ModelListing::Partial.serves(&listed, "anthropic/claude-haiku-5.5:eu"),
            None
        );
        assert_eq!(
            ModelListing::Complete.serves(&listed, "anthropic/claude-haiku-5.5"),
            Some(false)
        );
        assert_eq!(
            ModelListing::Complete.serves(&ids, "z-ai/glm-5.3"),
            Some(true)
        );
        assert_eq!(
            ModelListing::Complete.serves(&ids, "standardcompute"),
            Some(false)
        );
    }

    #[test]
    fn an_account_scope_takes_cloudflare_account_and_gateway_ids_only() {
        assert!(AccountScope::is_account_id(
            "0123456789abcdef0123456789abcdef"
        ));
        for bad in [
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef",
            "0123456789abcdef0123456789abcdeg",
            "",
        ] {
            assert!(!AccountScope::is_account_id(bad), "{bad}");
        }
        for good in ["default", "prod-eu", "Team_1", &"a".repeat(64)] {
            assert!(AccountScope::is_gateway_id(good), "{good}");
        }
        for bad in ["", "-x", "_x", "no spaces", "a/b", &"a".repeat(65)] {
            assert!(!AccountScope::is_gateway_id(bad), "{bad}");
        }
        let scoped: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.account_scope.is_some())
            .map(|p| p.id)
            .collect();
        assert_eq!(scoped, ["cloudflare-ai-gateway"]);
    }

    #[test]
    fn cloudflare_embeds_with_workers_ai_and_refuses_the_dimensions_it_ignores() {
        let p = provider("cloudflare-ai-gateway").unwrap();
        assert_eq!(p.embeddings, Some(DialectId::OpenAiEmbeddings));
        assert!(p.dialects.contains(&DialectId::OpenAiEmbeddings));
        assert_eq!(p.embeddings_profile.provider, "Cloudflare AI Gateway");
        assert!(!p.embeddings_profile.dimensions);
        let others: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| p.embeddings_profile != EmbeddingsProfile::OPENAI)
            .map(|p| p.id)
            .collect();
        assert_eq!(
            others,
            [
                "cloudflare-ai-gateway",
                "upstage",
                "novita-ai",
                "tencent-tokenhub",
                "voyage",
                "jina",
                "qianfan"
            ]
        );
    }

    #[test]
    fn each_auth_style_names_its_header_and_carries_the_key() {
        assert_eq!(
            [
                AuthStyle::Bearer,
                AuthStyle::XApiKey,
                AuthStyle::XGoogApiKey,
                AuthStyle::XiApiKey,
                AuthStyle::ApiKey,
                AuthStyle::Token
            ]
            .map(|s| (s.header_name(), s.header_value("k-1"))),
            [
                ("authorization", "Bearer k-1".to_owned()),
                ("x-api-key", "k-1".to_owned()),
                ("x-goog-api-key", "k-1".to_owned()),
                ("xi-api-key", "k-1".to_owned()),
                ("api-key", "k-1".to_owned()),
                ("authorization", "Token k-1".to_owned()),
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
        ] {
            assert_eq!(
                sent_with_profile("mistral", request).unwrap_err(),
                param,
                "{request}"
            );
        }
        let named = sent_with_profile("mistral", "requests/system-name-multiturn").unwrap();
        assert!(
            named["messages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("name").is_none()),
            "participant names are left out: {named}"
        );
        let store = sent_with_profile("mistral", "providers/mistral/requests/store-field").unwrap();
        assert!(
            store.get("store").is_none(),
            "store: false asks for nothing: {store}"
        );
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
            let sent = match sent_with_profile(p.id, "requests/grok-cli-model-id") {
                Ok(sent) => sent,
                Err(param) if p.chat.request.refused_fields.contains(&param.as_str()) => continue,
                Err(param) => panic!("{}: {param}", p.id),
            };
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
    fn each_decisions_provider_takes_its_own_format_and_the_rest_take_none() {
        let egress = |id: &str, model: &str| {
            provider(id)
                .unwrap()
                .decisions
                .iter()
                .find(|e| e.serves(model))
                .copied()
        };
        let openai = egress("openai", "gpt-6-luna").unwrap();
        assert_eq!(openai.dialect, DialectId::OpenAiDecisions);
        assert_eq!(openai.openai, DecisionsProfile::OPENAI);
        assert_eq!(openai.max_questions(), Some(400));
        assert!(!openai.box_questions());
        assert!(egress("openai", "gpt-6-luna-2026-09-22").is_some());
        assert!(egress("openai", "gpt-5.5").is_none());
        assert_eq!(
            egress("google", "gemini-decision-1").unwrap().dialect,
            DialectId::GeminiDecisions
        );
        assert_eq!(egress("google", "x").unwrap().max_questions(), None);
        let vercel = egress("vercel", "typesafe-ai/jev").unwrap();
        assert_eq!(
            (vercel.dialect, vercel.openai, vercel.max_questions()),
            (DialectId::OpenAiDecisions, DecisionsProfile::GENERIC, None)
        );
        for (id, profile) in [
            ("openrouter", SystemOneProfile::OPENROUTER),
            ("baseten", SystemOneProfile::BASETEN),
            ("fireworks-ai", SystemOneProfile::FIREWORKS),
            ("perplexity", SystemOneProfile::PERPLEXITY),
            ("typesafe", SystemOneProfile::TYPESAFE),
        ] {
            let e = egress(id, "any/model").unwrap();
            assert_eq!((e.dialect, e.system_one), (DialectId::SystemOne, profile));
            assert_eq!(e.box_questions(), profile.box_questions, "{id}");
            assert!(!e.from_root && e.auth.is_none(), "{id}");
        }
        assert_eq!(SystemOneProfile::PERPLEXITY.path, "/v1/decisions");
        let clef = egress("cloudflare-ai-gateway", "@cf/cloudflare/clef-flash").unwrap();
        assert_eq!(
            (clef.dialect, clef.system_one, clef.max_questions()),
            (
                DialectId::SystemOne,
                SystemOneProfile::CLOUDFLARE_WORKERS_AI,
                Some(64)
            )
        );
        assert!(!clef.box_questions() && !clef.from_root);
        assert!(egress("cloudflare-ai-gateway", "@cf/cloudflare/clef-omni").is_some());
        assert!(egress("cloudflare-ai-gateway", "typesafe/jev").is_none());
        assert!(egress("cloudflare-ai-gateway", "@cf/zai-org/glm-4.7-flash").is_none());
        let served = [
            "openai",
            "google",
            "openrouter",
            "vercel",
            "baseten",
            "fireworks-ai",
            "perplexity",
            "typesafe",
            "cloudflare-ai-gateway",
        ];
        for p in PROVIDERS {
            assert_eq!(!p.decisions.is_empty(), served.contains(&p.id), "{}", p.id);
            // Only a provider whose one API is a decisions API names its format, as its only one.
            let decision_formats: Vec<_> = p
                .dialects
                .iter()
                .filter(|d| {
                    matches!(
                        d,
                        DialectId::OpenAiDecisions
                            | DialectId::SystemOne
                            | DialectId::GeminiDecisions
                    )
                })
                .collect();
            assert!(
                decision_formats.is_empty()
                    || (p.dialects.len() == 1
                        && p.decisions.iter().all(|e| e.dialect == p.generation)),
                "{} lists a decision format as a generation format",
                p.id
            );
        }
    }

    #[test]
    fn typesafe_answers_decisions_in_system_one_and_nothing_else() {
        let p = provider("typesafe").unwrap();
        assert_eq!(
            (p.name, p.auth, p.generation, p.dialects),
            (
                "TypeSafe",
                AuthStyle::Bearer,
                DialectId::SystemOne,
                &[DialectId::SystemOne][..]
            )
        );
        assert_eq!(p.default_base_url, "https://api.typesafe.ai");
        assert_eq!(p.key_page.url, "https://console.typesafe.ai/keys");
        assert_eq!(
            (p.key_prefix, p.count_tokens, p.embeddings),
            (None, None, None)
        );
        assert!(
            p.images.is_empty()
                && p.speech.is_empty()
                && p.transcriptions.is_empty()
                && p.videos.is_empty()
        );
        let e = p.decisions.iter().find(|e| e.serves("jev-latest")).unwrap();
        assert_eq!(e.system_one, SystemOneProfile::TYPESAFE);
        assert!(e.box_questions() && e.max_questions().is_none());
        assert_eq!(e.system_one.path, "/v1/systemone");
        assert!(e.system_one.images_header && e.system_one.long_context.is_some());
        assert_eq!(p.listing, ModelListing::Partial);
        let listed = ["jev-latest".to_owned()];
        assert_eq!(p.listing.serves(&listed, "jev-latest"), Some(true));
        assert_eq!(p.listing.serves(&listed, "jev-1.13.0"), None);
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
                "huggingface",
                "nvidia",
                "baseten",
                "siliconflow",
                "standardcompute",
                "cloudflare-ai-gateway",
                "perplexity",
                "typesafe",
                "deepinfra",
                "upstage",
                "ionos",
                "novita-ai",
                "volcengine",
                "ovhcloud",
                "stepfun-ai",
                "tencent-tokenhub",
                "venice",
                "amazon-bedrock",
                "azure",
                "voyage",
                "jina",
                "fal",
                "replicate",
                "deepgram",
                "cartesia",
                "qianfan",
                "assemblyai"
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
