mod duration;
mod load;
mod settings;
mod size;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use kaeso_core::provider::ProviderId;
use kaeso_update::settings as refused;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use url::Url;

pub use crate::http::cors::AllowedOrigins;

pub use load::{ConfigError, MapSource, ProcessSource, Source, load, unknown_variables};

/// `kaeso config check`: the settings load and the master keys parse, without a database. Gives
/// the variables that name no setting, or the error the gateway would stop with.
pub fn check(source: &dyn Source) -> Result<Vec<String>, String> {
    let config = load(source).map_err(|e| format!("configuration: {e}"))?;
    kaeso_crypto::KeyRing::parse(&config.master_keys).map_err(|e| e.to_string())?;
    Ok(unknown_variables(source))
}
pub use settings::{
    Kind, PROVIDER_ENV_PREFIX, PROVIDER_FIELDS, ProviderField, ProviderVarError, REQUIRED, Setting,
    parse_provider_env, provider_env, provider_key, setting, settings,
};
pub use size::ByteSize;

const MAX_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(24 * 3600);

const MAX_RETENTION_DAYS: u32 = 36_500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub metrics_listen: SocketAddr,
    pub public_url: Url,
    #[serde(deserialize_with = "networks")]
    pub trusted_proxies: Vec<ipnet::IpNet>,
    #[serde(deserialize_with = "duration::deserialize")]
    pub shutdown_grace: Duration,
    pub server: ServerConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    pub postgres: PostgresConfig,
    pub clickhouse: ClickHouseConfig,
    pub valkey: ValkeyConfig,
    pub master_keys: SecretString,
    #[serde(default)]
    pub setup: SetupConfig,
    pub smtp: SmtpConfig,
    pub mail: MailConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub legal: LegalConfig,
    pub search_engines: bool,
    pub key_failures: KeyFailuresConfig,
    pub kpmr: KpmrConfig,
    pub exchange_rates: ExchangeRatesConfig,
    pub updates: UpdatesConfig,
    pub updater: UpdaterConfig,
    pub providers: ProvidersConfig,
    pub custom_providers: CustomProvidersConfig,
    #[serde(default)]
    pub mcp_servers: McpServersConfig,
    #[serde(default)]
    pub mcp_portal: McpPortalConfig,
    #[serde(default)]
    pub webhooks: WebhooksConfig,
    #[serde(default)]
    pub guardrails: GuardrailsConfig,
    #[serde(default)]
    pub aws: AwsConfig,
    pub upstream: UpstreamConfig,
    pub streams: StreamsConfig,
    pub continuity: ContinuityConfig,
    pub response_cache: ResponseCacheConfig,
    pub body_limits: BodyLimitsConfig,
    pub logs: LogsConfig,
    pub otel: OtelConfig,
    pub instance: InstanceConfig,
    pub log: LogConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    #[serde(deserialize_with = "duration::deserialize")]
    pub header_read_timeout: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub body_read_timeout: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub body_read_deadline: Duration,
    pub max_connections: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorsConfig {
    #[serde(default)]
    pub allowed_origins: String,
    #[serde(default)]
    pub allowed_headers: String,
    #[serde(skip)]
    pub origins: AllowedOrigins,
    #[serde(skip)]
    pub headers: crate::http::cors::ExtraHeaders,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresConfig {
    pub url: SecretString,
    pub pool_size: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickHouseConfig {
    pub url: Option<SecretString>,
    pub database: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValkeyConfig {
    pub url: SecretString,
    pub key_prefix: String,
    #[serde(default)]
    pub ca: Option<String>,
}

impl ValkeyConfig {
    pub fn ca(&self) -> Option<&str> {
        self.ca.as_deref().map(str::trim).filter(|c| !c.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtpTls {
    Starttls,
    Tls,
    None,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmtpConfig {
    pub host: Option<String>,
    pub port: u16,
    pub tls: SmtpTls,
    pub user: Option<String>,
    pub password: Option<SecretString>,
    pub from: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupConfig {
    #[serde(default)]
    email: Option<String>,
}

impl SetupConfig {
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref().filter(|e| !e.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailTransport {
    Smtp,
    Log,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailConfig {
    pub transport: MailTransport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub password: bool,
    pub magic_link: bool,
    #[serde(default)]
    pub demo_user: Option<String>,
    pub oidc_allow_private_networks: bool,
}

impl AuthConfig {
    pub fn demo_user(&self) -> Option<&str> {
        self.demo_user
            .as_deref()
            .map(str::trim)
            .filter(|address| !address.is_empty())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegalConfig {
    #[serde(default)]
    imprint_url: Option<String>,
    #[serde(default)]
    privacy_url: Option<String>,
}

impl LegalConfig {
    pub fn imprint_url(&self) -> Option<&str> {
        set(self.imprint_url.as_deref())
    }

    pub fn privacy_url(&self) -> Option<&str> {
        set(self.privacy_url.as_deref())
    }
}

fn set(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFailuresConfig {
    pub per_address: u32,
    #[serde(deserialize_with = "duration::deserialize")]
    pub window: Duration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KpmrConfig {
    pub sync: bool,
    pub url: Url,
    pub interval: KpmrInterval,
    #[serde(default, deserialize_with = "kpmr_keys")]
    pub public_keys: Vec<kaeso_core::kpmr::TrustedKey>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRatesConfig {
    pub url: Url,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KpmrInterval {
    Auto,
    Every(Duration),
}

impl<'de> Deserialize<'de> for KpmrInterval {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.trim() == "auto" {
            return Ok(KpmrInterval::Auto);
        }
        duration::parse(&s).map(KpmrInterval::Every).map_err(|_| {
            serde::de::Error::custom(format!("`{s}` is not `auto` or a duration such as 1h"))
        })
    }
}

fn kpmr_keys<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<kaeso_core::kpmr::TrustedKey>, D::Error> {
    let s = String::deserialize(d)?;
    s.split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| kaeso_core::kpmr::TrustedKey::parse(k).map_err(serde::de::Error::custom))
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatesConfig {
    pub check: bool,
    pub channel: kaeso_update::policy::Channel,
    pub mode: kaeso_update::schedule::Mode,
    pub releases_url: Url,
    pub installed_with: InstalledWith,
}

/// How Kaeso was installed, which decides the commands that update it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, serde::Serialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum InstalledWith {
    /// The installer or Docker Compose.
    #[default]
    Compose,
    /// The Helm chart.
    Helm,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdaterConfig {
    pub url: Url,
    pub token: Option<SecretString>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomProvidersConfig {
    pub enabled: bool,
    #[serde(default, deserialize_with = "networks")]
    pub allowed_networks: Vec<ipnet::IpNet>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhooksConfig {
    #[serde(default, deserialize_with = "networks")]
    pub allowed_networks: Vec<ipnet::IpNet>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServersConfig {
    #[serde(default, deserialize_with = "networks")]
    pub allowed_networks: Vec<ipnet::IpNet>,
    #[serde(
        deserialize_with = "duration::deserialize",
        default = "fifteen_minutes"
    )]
    pub sync_interval: Duration,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardrailsConfig {
    #[serde(default, deserialize_with = "networks")]
    pub allowed_networks: Vec<ipnet::IpNet>,
}

const fn fifteen_minutes() -> Duration {
    Duration::from_secs(15 * 60)
}

impl Default for McpServersConfig {
    fn default() -> McpServersConfig {
        McpServersConfig {
            allowed_networks: Vec::new(),
            sync_interval: fifteen_minutes(),
        }
    }
}

/// What Amazon Bedrock keys that sign in as the gateway's own AWS identity may do.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsConfig {
    /// Let instance admins add Bedrock keys that use the gateway's own AWS credentials (its
    /// environment, web identity, container or instance role).
    #[serde(default)]
    pub ambient_credentials: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpPortalConfig {
    #[serde(default = "four_mib")]
    pub response_limit: ByteSize,
    #[serde(deserialize_with = "duration::deserialize", default = "sixty_seconds")]
    pub call_timeout: Duration,
}

const fn four_mib() -> ByteSize {
    ByteSize(4 << 20)
}

const fn sixty_seconds() -> Duration {
    Duration::from_secs(60)
}

impl Default for McpPortalConfig {
    fn default() -> McpPortalConfig {
        McpPortalConfig {
            response_limit: four_mib(),
            call_timeout: sixty_seconds(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub base_url: Url,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct ProvidersConfig(BTreeMap<ProviderId, ProviderConfig>);

impl ProvidersConfig {
    pub fn get(&self, id: &str) -> Option<&ProviderConfig> {
        self.0.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProviderId, &ProviderConfig)> {
        self.0.iter()
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamConfig {
    #[serde(deserialize_with = "duration::deserialize")]
    pub connect_timeout: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub request_timeout: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub first_byte_timeout: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub stream_idle_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamsConfig {
    #[serde(deserialize_with = "duration::deserialize")]
    pub keepalive_interval: Duration,
    #[serde(deserialize_with = "duration::deserialize")]
    pub max_duration: Duration,
    pub max_bytes: ByteSize,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityConfig {
    #[serde(deserialize_with = "duration::deserialize")]
    pub reasoning_ttl: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseCacheConfig {
    pub enabled: bool,
    pub max_entry_size: ByteSize,
    pub memory_share: u8,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyLimitsConfig {
    pub model_api: ByteSize,
    pub images: ByteSize,
    pub audio: ByteSize,
    pub decisions: ByteSize,
    pub in_flight: ByteSize,
    pub platform_api: ByteSize,
    pub provider_response: ByteSize,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogsConfig {
    pub buffer_size: ByteSize,
    #[serde(deserialize_with = "duration::deserialize")]
    pub flush_interval: Duration,
    pub flush_rows: u32,
}

/// The export of traces to an OpenTelemetry collector over OTLP/HTTP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtelConfig {
    #[serde(default, deserialize_with = "optional_url")]
    pub endpoint: Option<Url>,
    pub headers: Option<SecretString>,
    pub service_name: String,
    pub content: bool,
    #[serde(deserialize_with = "ratio")]
    pub sample_ratio: f64,
    pub buffer_size: u32,
    #[serde(deserialize_with = "duration::deserialize")]
    pub flush_interval: Duration,
}

impl OtelConfig {
    /// Where spans are sent: the endpoint's `/v1/traces`, or the endpoint itself when it names it.
    pub fn traces_url(&self) -> Option<Url> {
        let endpoint = self.endpoint.as_ref()?;
        if endpoint.path().ends_with("/v1/traces") {
            return Some(endpoint.clone());
        }
        let mut url = endpoint.clone();
        let path = format!("{}/v1/traces", endpoint.path().trim_end_matches('/'));
        url.set_path(&path);
        Some(url)
    }

    /// The headers of `headers`, as `name=value` pairs separated by commas; a value may be
    /// percent-encoded, as `OTEL_EXPORTER_OTLP_HEADERS` allows.
    pub fn header_map(&self) -> Result<reqwest::header::HeaderMap, String> {
        let mut map = reqwest::header::HeaderMap::new();
        let Some(headers) = &self.headers else {
            return Ok(map);
        };
        for (at, pair) in headers
            .expose_secret()
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .enumerate()
        {
            let (name, value) = pair
                .split_once('=')
                .ok_or_else(|| "must be `name=value` pairs separated by commas".to_owned())?;
            // The text before `=` is not quoted: in a pair written wrongly, such as
            // `Authorization: Bearer <token>=`, it holds the collector's token.
            let name =
                reqwest::header::HeaderName::from_bytes(name.trim().as_bytes()).map_err(|_| {
                    format!(
                        "has a pair, number {}, whose name is not a header name",
                        at + 1
                    )
                })?;
            let value = percent_encoding::percent_decode_str(value.trim()).decode_utf8_lossy();
            let mut value = reqwest::header::HeaderValue::from_str(&value)
                .map_err(|_| format!("has a value for `{name}` that a header can't carry"))?;
            value.set_sensitive(true);
            map.append(name, value);
        }
        Ok(map)
    }
}

// A URL, or none for an empty value, as the example file writes an unset one.
fn optional_url<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Url>, D::Error> {
    let text = Option::<String>::deserialize(d)?;
    match text.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(url) => Url::parse(url).map(Some).map_err(serde::de::Error::custom),
    }
}

// A ratio as text, such as "0.25", or as a TOML number.
fn ratio<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(f64),
        Text(String),
    }
    let value = match Raw::deserialize(d)? {
        Raw::Number(n) => n,
        Raw::Text(t) => t
            .trim()
            .parse::<f64>()
            .map_err(|_| serde::de::Error::custom("must be a number from 0 to 1"))?,
    };
    if !(0.0..=1.0).contains(&value) {
        return Err(serde::de::Error::custom("must be a number from 0 to 1"));
    }
    Ok(value)
}

pub use kaeso_core::accounts::{AccountCreation, OrgCreation};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConfig {
    pub org_creation: OrgCreation,
    pub account_creation: AccountCreation,
    pub metadata_retention_days: u32,
    pub content_retention_days: u32,
    pub max_metadata_retention_days: u32,
    pub max_content_retention_days: u32,
    pub org_deletion: bool,
    pub org_deletion_grace_days: u32,
    pub timezone: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Json,
    Text,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    pub level: String,
    pub format: LogFormat,
}

impl Config {
    pub fn is_local_development(&self) -> bool {
        self.public_url.scheme() == "http" && is_loopback_host(&self.public_url)
    }

    pub fn public_origin(&self) -> String {
        self.public_url.origin().ascii_serialization()
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |key: &str, reason: String| Err(ConfigError::invalid(key, reason));
        match self.public_url.scheme() {
            "https" => {}
            "http" if is_loopback_host(&self.public_url) => {}
            _ => {
                return invalid(
                    "public_url",
                    "must be an https URL (http is only allowed for localhost during development)"
                        .into(),
                );
            }
        }
        match self.kpmr.url.scheme() {
            "https" => {}
            "http" if is_loopback_host(&self.kpmr.url) => {}
            _ => {
                return invalid(
                    "kpmr.url",
                    "must be an https URL (http is only allowed for localhost)".into(),
                );
            }
        }
        match self.exchange_rates.url.scheme() {
            "https" => {}
            "http" if is_loopback_host(&self.exchange_rates.url) => {}
            _ => {
                return invalid(
                    "exchange_rates.url",
                    "must be an https URL (http is only allowed for localhost)".into(),
                );
            }
        }
        if !self.exchange_rates.url.path().ends_with('/') {
            return invalid(
                "exchange_rates.url",
                "must end with `/`: the folder that holds the ECB's files".into(),
            );
        }
        if self.public_url.path() != "/"
            || self.public_url.query().is_some()
            || self.public_url.fragment().is_some()
            || !self.public_url.username().is_empty()
            || self.public_url.password().is_some()
        {
            return invalid(
                "public_url",
                "must be the bare origin of the install, without a path, query or user".into(),
            );
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(600))
            .contains(&self.server.header_read_timeout)
        {
            return invalid(
                "server.header_read_timeout",
                "must be between 1s and 10m".into(),
            );
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(600))
            .contains(&self.server.body_read_timeout)
        {
            return invalid(
                "server.body_read_timeout",
                "must be between 1s and 10m".into(),
            );
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(3600))
            .contains(&self.server.body_read_deadline)
        {
            return invalid(
                "server.body_read_deadline",
                "must be between 1s and 1h".into(),
            );
        }
        if self.server.max_connections == 0 {
            return invalid("server.max_connections", "must be at least 1".into());
        }
        if self.postgres.pool_size == 0 {
            return invalid("postgres.pool_size", "must be at least 1".into());
        }
        if self.clickhouse.database.is_empty()
            || !self
                .clickhouse
                .database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return invalid(
                "clickhouse.database",
                "must be a name of letters, digits and `_`".into(),
            );
        }
        if self.mail.transport == MailTransport::Smtp && self.smtp.host.is_none() {
            return invalid(
                "smtp.host",
                "is required to send sign-in links and alerts; set it, or set KAESO_MAIL_TRANSPORT=log for development".into(),
            );
        }
        if self.smtp.password.is_some() && self.smtp.user.is_none() {
            return invalid("smtp.user", "is required when smtp.password is set".into());
        }
        if self.smtp.user.is_some() && self.smtp.password.is_none() {
            return invalid(refused::SMTP_PASSWORD.key, refused::smtp_user_alone());
        }
        if self.valkey.ca().is_some() && !self.valkey.url.expose_secret().starts_with("rediss://") {
            return invalid(
                "valkey.ca",
                "needs a rediss:// URL in valkey.url, since it verifies a TLS connection".into(),
            );
        }
        if let Some(endpoint) = &self.otel.endpoint
            && (!matches!(endpoint.scheme(), "http" | "https")
                || endpoint.query().is_some()
                || endpoint.fragment().is_some())
        {
            return invalid(
                "otel.endpoint",
                "must be the http or https address of an OTLP/HTTP collector, such as `http://otel-collector:4318`, without a query".into(),
            );
        }
        if let Err(reason) = self.otel.header_map() {
            return invalid("otel.headers", reason);
        }
        if self.otel.service_name.trim().is_empty() {
            return invalid("otel.service_name", "must not be empty".into());
        }
        if self.otel.buffer_size == 0 {
            return invalid("otel.buffer_size", "must be at least 1".into());
        }
        if !(Duration::from_millis(100)..=Duration::from_secs(300))
            .contains(&self.otel.flush_interval)
        {
            return invalid("otel.flush_interval", "must be between 100ms and 5m".into());
        }
        if let Some(email) = self.setup.email() {
            if self.mail.transport != MailTransport::Smtp {
                return invalid(
                    "setup.email",
                    "needs mail.transport smtp, so the setup link can be sent".into(),
                );
            }
            if !email.contains('@') || email.contains(['\n', '\r', ',']) {
                return invalid("setup.email", "must be one email address".into());
            }
        }
        for (key, value) in [
            ("legal.imprint_url", self.legal.imprint_url()),
            ("legal.privacy_url", self.legal.privacy_url()),
        ] {
            if let Some(value) = value
                && !Url::parse(value).is_ok_and(|u| u.scheme() == "https" && u.host().is_some())
            {
                return invalid(key, "must be an https URL".into());
            }
        }
        for (key, url) in [
            ("updates.releases_url", &self.updates.releases_url),
            ("updater.url", &self.updater.url),
        ] {
            if !matches!(url.scheme(), "http" | "https") {
                return invalid(key, "must be an http or https URL".into());
            }
        }
        if self
            .updater
            .token
            .as_ref()
            .is_some_and(|t| t.expose_secret().trim().len() < kaeso_update::protocol::TOKEN_MIN_LEN)
        {
            return invalid(
                "updater.token",
                format!(
                    "must be at least {} characters; generate one with `openssl rand -hex 32`",
                    kaeso_update::protocol::TOKEN_MIN_LEN
                ),
            );
        }
        for (id, p) in self.providers.iter() {
            if !matches!(p.base_url.scheme(), "http" | "https") {
                return invalid(
                    &format!("providers.{id}.base_url"),
                    "must be an http or https URL".into(),
                );
            }
        }
        for (key, value) in [
            ("upstream.connect_timeout", self.upstream.connect_timeout),
            ("upstream.request_timeout", self.upstream.request_timeout),
            (
                "upstream.first_byte_timeout",
                self.upstream.first_byte_timeout,
            ),
            (
                "upstream.stream_idle_timeout",
                self.upstream.stream_idle_timeout,
            ),
            ("logs.flush_interval", self.logs.flush_interval),
        ] {
            if value.is_zero() {
                return invalid(key, "must be longer than zero".into());
            }
        }
        if let KpmrInterval::Every(every) = self.kpmr.interval
            && every < refused::KPMR_SHORTEST
        {
            return invalid(
                refused::KPMR_INTERVAL.key,
                refused::kpmr_interval_too_short(every),
            );
        }
        if self.mcp_servers.sync_interval < refused::MCP_SHORTEST {
            return invalid(
                refused::MCP_SYNC_INTERVAL.key,
                refused::mcp_sync_interval_too_short(self.mcp_servers.sync_interval),
            );
        }
        for (key, value) in [
            ("upstream.connect_timeout", self.upstream.connect_timeout),
            ("upstream.request_timeout", self.upstream.request_timeout),
            (
                "upstream.first_byte_timeout",
                self.upstream.first_byte_timeout,
            ),
            (
                "upstream.stream_idle_timeout",
                self.upstream.stream_idle_timeout,
            ),
        ] {
            if value > MAX_UPSTREAM_TIMEOUT {
                return invalid(key, "must be at most 24h".into());
            }
        }
        let keepalive = self.streams.keepalive_interval;
        if !keepalive.is_zero()
            && !(Duration::from_secs(1)..=Duration::from_secs(300)).contains(&keepalive)
        {
            return invalid(
                "streams.keepalive_interval",
                "must be 0s (off) or between 1s and 300s; proxies commonly close idle connections after 60s".into(),
            );
        }
        let ttl = self.continuity.reasoning_ttl;
        if !ttl.is_zero()
            && !(Duration::from_secs(60)..=Duration::from_secs(24 * 3600)).contains(&ttl)
        {
            return invalid(
                "continuity.reasoning_ttl",
                "must be 0s (off) or between 1m and 24h".into(),
            );
        }
        if !(1..=90).contains(&self.response_cache.memory_share) {
            return invalid(
                "response_cache.memory_share",
                "must be between 1 and 90 (percent of Valkey's maxmemory)".into(),
            );
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(3600)).contains(&self.key_failures.window)
        {
            return invalid("key_failures.window", "must be between 1s and 1h".into());
        }
        for (key, size) in [
            ("body_limits.model_api", self.body_limits.model_api),
            ("body_limits.images", self.body_limits.images),
            ("body_limits.audio", self.body_limits.audio),
            ("body_limits.decisions", self.body_limits.decisions),
            ("body_limits.in_flight", self.body_limits.in_flight),
            ("body_limits.platform_api", self.body_limits.platform_api),
            (
                "body_limits.provider_response",
                self.body_limits.provider_response,
            ),
            ("logs.buffer_size", self.logs.buffer_size),
            ("mcp_portal.response_limit", self.mcp_portal.response_limit),
            (
                "response_cache.max_entry_size",
                self.response_cache.max_entry_size,
            ),
        ] {
            if size.0 == 0 {
                return invalid(key, "must be larger than zero".into());
            }
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(3600))
            .contains(&self.mcp_portal.call_timeout)
        {
            return invalid(
                "mcp_portal.call_timeout",
                "must be between 1s and 1h".into(),
            );
        }
        if self.logs.flush_rows == 0 {
            return invalid("logs.flush_rows", "must be at least 1".into());
        }
        let i = &self.instance;
        if i.content_retention_days > i.metadata_retention_days {
            return invalid(
                "instance.content_retention_days",
                "must not be longer than instance.metadata_retention_days (content is never kept longer than metadata)".into(),
            );
        }
        for (key, days) in [
            (
                "instance.metadata_retention_days",
                i.metadata_retention_days,
            ),
            ("instance.content_retention_days", i.content_retention_days),
        ] {
            if days == 0 {
                return invalid(key, "must be at least 1 day".into());
            }
        }
        for (key, days) in [
            (
                "instance.max_metadata_retention_days",
                i.max_metadata_retention_days,
            ),
            (
                "instance.max_content_retention_days",
                i.max_content_retention_days,
            ),
        ] {
            if days > MAX_RETENTION_DAYS {
                return invalid(key, format!("must be at most {MAX_RETENTION_DAYS} days"));
            }
        }
        if i.metadata_retention_days > i.max_metadata_retention_days {
            return invalid(
                "instance.metadata_retention_days",
                "must not exceed instance.max_metadata_retention_days".into(),
            );
        }
        if i.content_retention_days > i.max_content_retention_days {
            return invalid(
                "instance.content_retention_days",
                "must not exceed instance.max_content_retention_days".into(),
            );
        }
        if !(1..=90).contains(&i.org_deletion_grace_days) {
            return invalid(
                "instance.org_deletion_grace_days",
                "must be between 1 and 90 days".into(),
            );
        }
        if crate::time_zone::parse(&i.timezone).is_none() {
            return invalid(
                "instance.timezone",
                "must be an IANA time zone such as `Europe/Berlin` or `UTC`".into(),
            );
        }
        if i.account_creation == AccountCreation::AllowedDomains {
            return invalid(
                "instance.account_creation",
                "must be `invitation_only` or `everyone`; allowed domains are set in the instance settings".into(),
            );
        }
        if i.org_creation == OrgCreation::AllowedDomains {
            return invalid(
                "instance.org_creation",
                "must be `instance_admins` or `everyone`; allowed domains are set in the instance settings".into(),
            );
        }
        Ok(())
    }
}

fn networks<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<ipnet::IpNet>, D::Error> {
    let s = String::deserialize(d)?;
    s.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| {
            n.parse::<ipnet::IpNet>()
                .or_else(|_| n.parse::<std::net::IpAddr>().map(ipnet::IpNet::from))
                .map_err(|_| {
                    serde::de::Error::custom(format!("`{n}` is not a network such as 10.0.0.0/8"))
                })
        })
        .collect()
}

pub(crate) fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod check_tests {
    use super::*;

    fn settings(master_keys: &str) -> MapSource {
        MapSource::new()
            .set("KAESO_PUBLIC_URL", "https://kaeso.example.com")
            .set("KAESO_POSTGRES_URL", "postgres://kaeso@db/kaeso")
            .set("KAESO_VALKEY_URL", "redis://valkey:6379")
            .set("KAESO_MASTER_KEYS", master_keys)
            .set("KAESO_MAIL_TRANSPORT", "log")
    }

    #[test]
    fn the_check_loads_the_settings_and_parses_the_master_keys_without_a_database() {
        use secrecy::ExposeSecret as _;
        let key = kaeso_crypto::generate_master_key();
        let valid = settings(key.expose_secret()).set("KAESO_LISTN", "0.0.0.0:8080");
        assert_eq!(check(&valid), Ok(vec!["KAESO_LISTN".to_owned()]));
        let bad_key = check(&settings("k1:AAAA")).unwrap_err();
        assert!(!bad_key.is_empty());
        let missing = check(&MapSource::new()).unwrap_err();
        assert!(missing.starts_with("configuration: "), "{missing}");
    }
}
