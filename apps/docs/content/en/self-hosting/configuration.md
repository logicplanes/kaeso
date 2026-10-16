---
title: Configuration
description: How the gateway reads its settings, and every setting it has.
order: 3
---

The gateway is configured with `KAESO_*` environment variables, optionally backed by a TOML file. In the Compose install, put them in `.env` or in `compose.override.yaml`, as [installation](installation.md#local-changes) shows. The Compose file already sets the database URLs, the master key file and the JSON log format, and derives the public URL from `KAESO_DOMAIN`.

## How settings are read

- **Environment variables.** Each setting has a variable, such as `KAESO_LOG_LEVEL`. An empty variable counts as unset.
- **A file.** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` reads settings from a TOML file, with the TOML keys below written as tables (`[postgres]`, then `url = "…"`). An environment variable wins over the file. A key that is not a setting is an error. The repository's [`deploy/kaeso.example.toml`](https://github.com/logicplanes/kaeso/blob/dev/deploy/kaeso.example.toml) sets every setting that is not a secret to its default, so it is a valid starting point.
- **Secrets from files.** Every setting marked secret can be read instead from a file named in `<VARIABLE>_FILE`, such as `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys` for Docker secrets. A trailing newline is ignored. Setting both the variable and its `_FILE` is an error.
- **Checked at startup.** The gateway checks every setting when it starts and refuses to start with a message that names the setting. It never prints secret values.

| Type | Values |
| --- | --- |
| text | Text, URLs, addresses (`0.0.0.0:8080`), durations (`10s`, `5m`, `24h`), sizes (`32MiB`, `1MB`, or a number of bytes) and the listed choices |
| bool | `true` or `false`; also `1` and `0`, `yes` and `no`, `on` and `off` |
| integer | A whole number, zero or more |

## Settings

### Listening

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | text | `0.0.0.0:8080` | Address of the model API, the Platform API and the health checks. |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | text | `0.0.0.0:9100` | Address of the Prometheus metrics listener. Never route it publicly. |
| `KAESO_PUBLIC_URL` | `public_url` | text | required | The URL people and apps use to reach this install, such as `https://kaeso.example.com`: the bare origin, without a path. Must be `https`, except `http://localhost` for development. |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | Comma-separated networks of reverse proxies whose `X-Forwarded-For` is believed, for client addresses in logs, sign-in throttling and the API key throttle. Empty trusts no proxy. |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | text | `30s` | How long requests and streams in flight may run after a shutdown signal. Those still open then end with an error in the app's format (503) and are charged and logged. |

### Databases

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | text, secret | required | PostgreSQL connection URL. |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | integer | `20` | Maximum PostgreSQL connections per gateway instance, at least 1. |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | text, secret | required | ClickHouse HTTP URL, with user and password, such as `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | text | `kaeso` | ClickHouse database for request logs, made of letters, digits and `_`; created when missing. |
| `KAESO_VALKEY_URL` | `valkey.url` | text, secret | required | Valkey (or Redis) URL, such as `redis://valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | text | `kaeso:` | Prefix of every Valkey key and channel, for sharing one Valkey between installs. |

### Master keys

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | text, secret | required | Master keys as `id:base64,…`, the active key first. Generate one with `kaeso master-key generate`. See [backups and master keys](backups.md). |

### Email

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | text | — | SMTP server. Required unless `mail.transport` is `log`. |
| `KAESO_SMTP_PORT` | `smtp.port` | integer | `587` | SMTP port. |
| `KAESO_SMTP_TLS` | `smtp.tls` | text | `starttls` | `starttls`, `tls` (implicit TLS, usually port 465) or `none` (only for a local mail catcher). |
| `KAESO_SMTP_USER` | `smtp.user` | text | — | SMTP user name, if the server needs one. Required when a password is set. |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | text, secret | — | SMTP password. |
| `KAESO_SMTP_FROM` | `smtp.from` | text | `Kaeso <kaeso@localhost>` | Sender of every email Kaeso sends. The Compose file sets `Kaeso <kaeso@` and your domain. |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | text | `smtp` | `smtp`, or `log` to only log that an email would have been sent (recipient and kind, never the content). For trying Kaeso and for development. |

### Sign-in

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | bool | `true` | Allow sign-in with email and password. |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | bool | `true` | Allow sign-in with an email magic link. |
| `KAESO_AUTH_OIDC` | `auth.oidc` | bool | `true` | Allow sign-in through an organization's OIDC identity provider. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | bool | `true` | Allow OIDC discovery and token requests to private and loopback addresses. Turn it off on installs where organization admins should not reach your internal network through the gateway. |

### API key failures

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | integer | `100` | Failed API key lookups allowed per client address and window, on the model API and the Platform API together; beyond it, keys that are not cached are refused with 429 until the window ends. `0` turns the throttle off. See [API key throttling](#api-key-throttling). |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | text | `1m` | The window of `key_failures.per_address`, 1s to 1h. A blocked address waits at most this long. |

### models.dev

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | bool | `true` | Fetch models.dev periodically. Turn it off for installs without internet access; the snapshot built into the release is used. |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | text | `https://models.dev/api.json` | Where models.dev data is fetched from. |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | text | `24h` | How often models.dev is fetched. |

### Provider calls

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | text | `https://api.openai.com` | Base URL of the OpenAI API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | text | `https://api.anthropic.com` | Base URL of the Anthropic API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | text | `10s` | Connect timeout to providers, TCP and TLS. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | text | `600s` | Total timeout of a provider request that doesn't stream. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | text | `60s` | Streaming: time to the first byte from the provider. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | text | `300s` | Streaming: longest silence between two chunks. |

Timeouts for a single model are set on the model in a project's catalogue; see [models and aliases](../console/models-and-aliases.md).

### Streams to apps

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | text | `15s` | While a streamed reply is open and nothing was written to the app for this long, write a keepalive the app's SDK ignores (an SSE comment for the OpenAI formats, a `ping` event for Anthropic), so proxies with idle timeouts keep the connection. `0s` turns keepalives off; otherwise 1s to 300s. See [long streams](domains-and-tls.md#long-streams). |

### Reasoning continuity

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | text | `1h` | How long the gateway keeps the reasoning that came before a tool call for an OpenAI Chat Completions app whose model is called in another format (Chat Completions has no field for it), so the app's next turn carries it back. Held in Valkey, encrypted under the master key and readable only with the API key that made the request; never logged. `0s` turns it off; otherwise 1m to 24h. See [format translation](../concepts/format-translation.md). |

### Request bodies

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | text | `32MiB` | Largest request body of the model API. |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | text | `1MiB` | Largest request body of the Platform API. |

### Request logs

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | text | `256MiB` | Memory for request log rows waiting for ClickHouse; the oldest rows are dropped beyond it. |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | text | `1s` | How often buffered log rows are written to ClickHouse. |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | integer | `5000` | Write buffered log rows as soon as this many are waiting. |

### Instance defaults

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | text | `instance_admins` | Who may create organizations on a new install: `instance_admins` or `everyone`. Instance admins change it later in the instance settings, where they can also allow users on some email domains. |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | text | `invitation_only` | Who may create an account without an invitation on a new install: `invitation_only` or `everyone`. Instance admins change it later in the instance settings, where they can also allow only some email domains. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | integer | `90` | Default request log metadata retention of new organizations, in days. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | integer | `30` | Default request log content retention of new organizations, in days; never longer than the metadata retention. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | integer | `3650` | Longest metadata retention an organization may choose, in days. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | integer | `3650` | Longest content retention an organization may choose, in days. |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | bool | `true` | Whether organizations can be deleted on a new install. Instance admins change it later in the instance settings. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | integer | `7` | Days a deleted organization stays restorable and read-only before it is removed for good, 1 to 90. |

### Logging

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | text | `info,sqlx=warn` | Log level or filter, such as `info` or `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT` | `log.format` | text | `json` | `json` for structured logs, `text` for development. |

## API key throttling

A request with an API key that the gateway hasn't seen recently costs a PostgreSQL lookup. Unknown keys are remembered for 30 seconds, so an app that keeps sending a rotated key costs one lookup per 30 seconds and gateway instance; a client that makes up a new key for every request would cost one per request. So the gateway counts failed lookups per client address, on the model API and the Platform API together, in Valkey. After `KAESO_KEY_FAILURES_PER_ADDRESS` failures (100) within `KAESO_KEY_FAILURES_WINDOW` (1 minute), the address is blocked until the window ends: every key from it that is not cached gets 429 with `Retry-After`, in the API's own format, without a database query.

- Only lookups that find no key count. Malformed values, disabled or expired keys, and an unknown key repeated within 30 seconds don't.
- Valid keys never count. A key used in the last 30 seconds on an instance is answered from memory, even from a blocked address; a valid key that wasn't used for longer waits for the window to end.
- IPv6 addresses count per /64.
- Behind a reverse proxy, the client address comes from `X-Forwarded-For` only when the proxy is in `KAESO_TRUSTED_PROXIES`. With an untrusted proxy in front, every client looks like the proxy and shares its allowance, so list your proxies.
- When Valkey can't be reached, failures are not counted.

The metric `kaeso_key_failure_blocks_total` counts blocks, and the log line `too many failed API key lookups from one address` names the address; see [health and metrics](monitoring.md).

## Notes

- **Instance defaults** only seed the instance settings of a new install. After that, instance admins change who may create accounts and organizations, whether organizations can be deleted and their grace period, and the retention defaults and maximums under **Settings** › **Instance** › **Settings** in the console, or through the Platform API (`instance/settings`); the stored values win.
- **Sign-in methods** turned off here can't be turned on in the console, which shows them as the gateway's configuration sets them.
- **Development.** With `KAESO_PUBLIC_URL=http://localhost:…` the session cookie is sent without `Secure` and without the `__Host-` prefix, so the console works over plain HTTP on your machine. Any other `http` URL is refused.
