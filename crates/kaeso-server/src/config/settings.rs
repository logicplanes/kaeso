#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Bool,
    Integer,
}

impl Kind {
    pub fn doc_name(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Bool => "bool",
            Kind::Integer => "integer",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub key: &'static str,
    pub env: &'static str,
    pub kind: Kind,
    pub default: Option<&'static str>,
    pub secret: bool,
    pub description: &'static str,
}

const fn s(
    key: &'static str,
    env: &'static str,
    kind: Kind,
    default: Option<&'static str>,
    secret: bool,
    description: &'static str,
) -> Setting {
    Setting {
        key,
        env,
        kind,
        default,
        secret,
        description,
    }
}

use Kind::{Bool, Integer, Text};

#[rustfmt::skip]
pub const SETTINGS: &[Setting] = &[
    s("listen", "KAESO_LISTEN", Text, Some("0.0.0.0:8080"), false, "Address of the model API, the Platform API and the health checks."),
    s("metrics_listen", "KAESO_METRICS_LISTEN", Text, Some("0.0.0.0:9100"), false, "Address of the Prometheus metrics listener. Never route it publicly."),
    s("public_url", "KAESO_PUBLIC_URL", Text, None, false, "The URL people and apps use to reach this install, e.g. `https://kaeso.example.com`. Required. Must be `https`, except `http://localhost` for development."),
    s("trusted_proxies", "KAESO_TRUSTED_PROXIES", Text, Some("127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7"), false, "Comma-separated networks of reverse proxies whose `X-Forwarded-For` is believed, for client IP addresses in logs, sign-in throttling and API key failure throttling. Empty trusts no proxy."),
    s("shutdown_grace", "KAESO_SHUTDOWN_GRACE", Text, Some("30s"), false, "How long in-flight requests and streams may run after a shutdown signal."),
    s("postgres.url", "KAESO_POSTGRES_URL", Text, None, true, "PostgreSQL connection URL. Required."),
    s("postgres.pool_size", "KAESO_POSTGRES_POOL_SIZE", Integer, Some("20"), false, "Maximum PostgreSQL connections per gateway instance."),
    s("clickhouse.url", "KAESO_CLICKHOUSE_URL", Text, None, true, "ClickHouse HTTP URL, with user and password, e.g. `http://kaeso:secret@clickhouse:8123`. Required."),
    s("clickhouse.database", "KAESO_CLICKHOUSE_DATABASE", Text, Some("kaeso"), false, "ClickHouse database for request logs; created when missing."),
    s("valkey.url", "KAESO_VALKEY_URL", Text, None, true, "Valkey (or Redis) URL, e.g. `redis://valkey:6379`. Required."),
    s("valkey.key_prefix", "KAESO_VALKEY_KEY_PREFIX", Text, Some("kaeso:"), false, "Prefix of every Valkey key and channel, for sharing one Valkey between installs."),
    s("master_keys", "KAESO_MASTER_KEYS", Text, None, true, "Master keys as `id:base64,...`, active key first. Required. Generate one with `kaeso master-key generate`."),
    s("smtp.host", "KAESO_SMTP_HOST", Text, None, false, "SMTP server. Required unless `mail.transport` is `log`."),
    s("smtp.port", "KAESO_SMTP_PORT", Integer, Some("587"), false, "SMTP port."),
    s("smtp.tls", "KAESO_SMTP_TLS", Text, Some("starttls"), false, "`starttls`, `tls` (implicit TLS, usually port 465) or `none` (only for a local mail catcher)."),
    s("smtp.user", "KAESO_SMTP_USER", Text, None, false, "SMTP user name, if the server needs one."),
    s("smtp.password", "KAESO_SMTP_PASSWORD", Text, None, true, "SMTP password."),
    s("smtp.from", "KAESO_SMTP_FROM", Text, Some("Kaeso <kaeso@localhost>"), false, "Sender of every email Kaeso sends."),
    s("mail.transport", "KAESO_MAIL_TRANSPORT", Text, Some("smtp"), false, "`smtp`, or `log` to only log that an email would have been sent (recipient and kind, never the content). For development."),
    s("auth.password", "KAESO_AUTH_PASSWORD", Bool, Some("true"), false, "Allow sign-in with email and password."),
    s("auth.magic_link", "KAESO_AUTH_MAGIC_LINK", Bool, Some("true"), false, "Allow sign-in with an email magic link."),
    s("auth.oidc", "KAESO_AUTH_OIDC", Bool, Some("true"), false, "Allow sign-in through an organization's OIDC identity provider."),
    s("auth.oidc_allow_private_networks", "KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS", Bool, Some("true"), false, "Allow OIDC discovery and token requests to private and loopback addresses. Turn off on hosted installs to prevent server-side request forgery."),
    s("key_failures.per_address", "KAESO_KEY_FAILURES_PER_ADDRESS", Integer, Some("100"), false, "Failed API key lookups (well-formed keys that match no key) allowed per client address and window, on the model API and the Platform API together; beyond it, keys that are not cached are refused with 429 until the window ends. IPv6 addresses count per /64. Valid keys never count. `0` turns the throttle off."),
    s("key_failures.window", "KAESO_KEY_FAILURES_WINDOW", Text, Some("1m"), false, "The window of `key_failures.per_address`: 1s to 1h."),
    s("models_dev.sync", "KAESO_MODELS_DEV_SYNC", Bool, Some("true"), false, "Fetch models.dev periodically. Off for installs without internet; the built-in snapshot is used."),
    s("models_dev.url", "KAESO_MODELS_DEV_URL", Text, Some("https://models.dev/api.json"), false, "Where models.dev data is fetched from."),
    s("models_dev.interval", "KAESO_MODELS_DEV_INTERVAL", Text, Some("24h"), false, "How often models.dev is fetched."),
    s("providers.openai.base_url", "KAESO_PROVIDERS_OPENAI_BASE_URL", Text, Some("https://api.openai.com"), false, "Base URL of the OpenAI API."),
    s("providers.anthropic.base_url", "KAESO_PROVIDERS_ANTHROPIC_BASE_URL", Text, Some("https://api.anthropic.com"), false, "Base URL of the Anthropic API."),
    s("upstream.connect_timeout", "KAESO_UPSTREAM_CONNECT_TIMEOUT", Text, Some("10s"), false, "Connect timeout to providers, TCP and TLS."),
    s("upstream.request_timeout", "KAESO_UPSTREAM_REQUEST_TIMEOUT", Text, Some("600s"), false, "Total timeout of a non-streaming provider request."),
    s("upstream.first_byte_timeout", "KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT", Text, Some("60s"), false, "Streaming: time to the first byte from the provider."),
    s("upstream.stream_idle_timeout", "KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT", Text, Some("300s"), false, "Streaming: longest silence between two chunks."),
    s("streams.keepalive_interval", "KAESO_STREAMS_KEEPALIVE_INTERVAL", Text, Some("15s"), false, "While a streamed reply is open and nothing was written to the app for this long, write a keepalive the app's SDK ignores (an SSE comment for OpenAI formats, a `ping` event for Anthropic), so proxies with idle timeouts keep the connection. `0s` turns keepalives off; otherwise 1s to 300s."),
    s("continuity.reasoning_ttl", "KAESO_CONTINUITY_REASONING_TTL", Text, Some("1h"), false, "How long the gateway keeps the reasoning that came before a tool call for an OpenAI Chat Completions app served by another provider (Chat has no field for it), so the app's next turn carries it back. Held in Valkey, encrypted under the master key and readable only with the API key that made the request; never logged. `0s` turns it off (Claude then answers tool results without its earlier thinking); otherwise 1m to 24h."),
    s("body_limits.model_api", "KAESO_BODY_LIMITS_MODEL_API", Text, Some("32MiB"), false, "Largest request body of the model API."),
    s("body_limits.platform_api", "KAESO_BODY_LIMITS_PLATFORM_API", Text, Some("1MiB"), false, "Largest request body of the Platform API."),
    s("logs.buffer_size", "KAESO_LOGS_BUFFER_SIZE", Text, Some("256MiB"), false, "Memory for request log rows waiting for ClickHouse; the oldest rows are dropped beyond it."),
    s("logs.flush_interval", "KAESO_LOGS_FLUSH_INTERVAL", Text, Some("1s"), false, "How often buffered log rows are written to ClickHouse."),
    s("logs.flush_rows", "KAESO_LOGS_FLUSH_ROWS", Integer, Some("5000"), false, "Write buffered log rows as soon as this many are waiting."),
    s("instance.org_creation", "KAESO_INSTANCE_ORG_CREATION", Text, Some("instance_admins"), false, "Who may create organizations on a new install: `instance_admins` or `everyone`. Instance admins change it later in the instance settings, where they can also allow users on some email domains."),
    s("instance.account_creation", "KAESO_INSTANCE_ACCOUNT_CREATION", Text, Some("invitation_only"), false, "Who may create an account without an invitation on a new install: `invitation_only` or `everyone`. Instance admins change it later in the instance settings, where they can also allow only some email domains."),
    s("instance.metadata_retention_days", "KAESO_INSTANCE_METADATA_RETENTION_DAYS", Integer, Some("90"), false, "Default log metadata retention of new organizations, in days."),
    s("instance.content_retention_days", "KAESO_INSTANCE_CONTENT_RETENTION_DAYS", Integer, Some("30"), false, "Default log content retention of new organizations, in days."),
    s("instance.max_metadata_retention_days", "KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS", Integer, Some("3650"), false, "Longest metadata retention an organization may choose, in days."),
    s("instance.max_content_retention_days", "KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS", Integer, Some("3650"), false, "Longest content retention an organization may choose, in days."),
    s("instance.org_deletion", "KAESO_INSTANCE_ORG_DELETION", Bool, Some("true"), false, "Whether organizations can be deleted on a new install. Instance admins change it later in the instance settings."),
    s("instance.org_deletion_grace_days", "KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS", Integer, Some("7"), false, "Days a deleted organization stays restorable and read-only before it is removed for good, 1 to 90."),
    s("log.level", "KAESO_LOG_LEVEL", Text, Some("info,sqlx=warn"), false, "Log level or filter, e.g. `info` or `info,kaeso_server=debug`."),
    s("log.format", "KAESO_LOG_FORMAT", Text, Some("json"), false, "`json` for structured logs, `text` for development."),
];

pub fn setting(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.key == key)
}

pub const REQUIRED: &[&str] = &[
    "public_url",
    "postgres.url",
    "clickhouse.url",
    "valkey.url",
    "master_keys",
];
