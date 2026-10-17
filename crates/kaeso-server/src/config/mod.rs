mod load;
mod settings;
mod size;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use kaeso_core::provider::ProviderId;
use secrecy::SecretString;
use serde::Deserialize;
use url::Url;

pub use crate::http::cors::AllowedOrigins;

pub use load::{ConfigError, MapSource, ProcessSource, Source, load};
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
    #[serde(with = "humantime_serde")]
    pub shutdown_grace: Duration,
    pub server: ServerConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    pub postgres: PostgresConfig,
    pub clickhouse: ClickHouseConfig,
    pub valkey: ValkeyConfig,
    pub master_keys: SecretString,
    pub smtp: SmtpConfig,
    pub mail: MailConfig,
    pub auth: AuthConfig,
    pub key_failures: KeyFailuresConfig,
    pub models_dev: ModelsDevConfig,
    pub providers: ProvidersConfig,
    pub upstream: UpstreamConfig,
    pub streams: StreamsConfig,
    pub continuity: ContinuityConfig,
    pub body_limits: BodyLimitsConfig,
    pub logs: LogsConfig,
    pub instance: InstanceConfig,
    pub log: LogConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    #[serde(with = "humantime_serde")]
    pub header_read_timeout: Duration,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorsConfig {
    #[serde(default)]
    pub allowed_origins: String,
    #[serde(skip)]
    pub origins: AllowedOrigins,
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
    pub url: SecretString,
    pub database: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValkeyConfig {
    pub url: SecretString,
    pub key_prefix: String,
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
    pub oidc: bool,
    pub oidc_allow_private_networks: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFailuresConfig {
    pub per_address: u32,
    #[serde(with = "humantime_serde")]
    pub window: Duration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsDevConfig {
    pub sync: bool,
    pub url: Url,
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
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
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub first_byte_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub stream_idle_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamsConfig {
    #[serde(with = "humantime_serde")]
    pub keepalive_interval: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuityConfig {
    #[serde(with = "humantime_serde")]
    pub reasoning_ttl: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyLimitsConfig {
    pub model_api: ByteSize,
    pub platform_api: ByteSize,
    pub provider_response: ByteSize,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogsConfig {
    pub buffer_size: ByteSize,
    #[serde(with = "humantime_serde")]
    pub flush_interval: Duration,
    pub flush_rows: u32,
}

pub use kaeso_core::accounts::{AccountCreation, OrgCreation};

#[derive(Clone, Copy, Debug, Deserialize)]
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
            ("models_dev.interval", self.models_dev.interval),
        ] {
            if value.is_zero() {
                return invalid(key, "must be longer than zero".into());
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
        if !(Duration::from_secs(1)..=Duration::from_secs(3600)).contains(&self.key_failures.window)
        {
            return invalid("key_failures.window", "must be between 1s and 1h".into());
        }
        for (key, size) in [
            ("body_limits.model_api", self.body_limits.model_api),
            ("body_limits.platform_api", self.body_limits.platform_api),
            (
                "body_limits.provider_response",
                self.body_limits.provider_response,
            ),
            ("logs.buffer_size", self.logs.buffer_size),
        ] {
            if size.0 == 0 {
                return invalid(key, "must be larger than zero".into());
            }
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
        if i.metadata_retention_days == 0 || i.content_retention_days == 0 {
            return invalid(
                "instance.metadata_retention_days",
                "retention must be at least 1 day".into(),
            );
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

fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}
