---
title: Configuration
description: Every setting of a Kaeso install, where it goes and how the gateway reads it.
order: 3
---

An install keeps its settings next to `compose.yaml`. The variables in [the first table](#settings-in-env) go in `.env`; every other gateway setting goes in `compose.override.yaml`:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_STREAMS_KEEPALIVE_INTERVAL: 10s
```

Apply a change with `docker compose up -d --wait`. A gateway setting written only in `.env` never reaches the gateway, because Compose passes on only the variables of the first table.

## Settings in .env

`compose.yaml` reads these variables from `.env` and derives the gateway's database URLs, master key file and public URL from them.

| Variable | Required | Sets |
| --- | --- | --- |
| `KAESO_DOMAIN` | yes | The domain Caddy serves and gets a certificate for. The public URL is `https://` and the domain. |
| `POSTGRES_PASSWORD` | yes | The password of PostgreSQL's `kaeso` user, which the gateway signs in as. |
| `POSTGRES_SUPERUSER_PASSWORD` | yes | The password of PostgreSQL's superuser `postgres`. Kaeso never uses it. |
| `CLICKHOUSE_PASSWORD` | yes | The password of ClickHouse's `kaeso` user. |
| `VALKEY_PASSWORD` | yes | The password Valkey requires on every connection. |
| `KAESO_PUBLIC_URL` | no | The public URL, when it isn't `https://` and the domain, such as behind your own proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, unless emails are only logged | How the gateway sends email ([email](#email)). |
| `KAESO_MAIL_TRANSPORT` | no | `log` to only log emails instead of sending them. |
| `KAESO_UPDATER_TOKEN`, `COMPOSE_PROFILES` | no | The token the gateway and the updater share, and `updater` to start the updater ([one-click updates](upgrades.md#one-click-updates)). |
| `DOCKER_SOCKET` | no | The path of Docker's socket for the updater, when it isn't `/var/run/docker.sock`. |

Use only letters and digits in the passwords, as `openssl rand -hex` writes them, because they go into connection URLs. PostgreSQL keeps the passwords it had when its volume was created, so changing `POSTGRES_PASSWORD` later locks the gateway out. ClickHouse and Valkey take their password from `.env` at every start.

## How the gateway reads settings

- **Environment variables.** Each setting has one, such as `KAESO_LOG_LEVEL`. An empty variable counts as unset.
- **A TOML file.** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` reads settings from a file, with the TOML keys below. An environment variable wins over the file, and a key that isn't a setting is an error. [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) sets every setting but the secrets.
- **Secrets from files.** Every secret can instead come from the file named in `<VARIABLE>_FILE`, such as `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys`. A trailing newline is ignored, and setting both is an error.
- **Checked at startup.** The gateway doesn't start when a setting is missing or invalid. Its message names the setting and never prints a secret.

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
| `KAESO_METRICS_LISTEN` | `metrics_listen` | text | `0.0.0.0:9100` | Address of the Prometheus metrics. Never route it publicly. |
| `KAESO_PUBLIC_URL` | `public_url` | text | required | The address people and apps use, such as `https://kaeso.example.com`, without a path. It must be `https`, except `http` on a loopback address for development. |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | Comma-separated networks of proxies whose `X-Forwarded-For` the gateway believes. `trusted_proxies = ""` in the TOML file trusts none; an empty variable keeps the default. |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | text | `30s` | How long requests and streams in flight may run after a shutdown signal. Those still open then end with 503 and are charged and logged. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | text | `30s` | How long a client may take to send a request's headers, or leave a connection idle between requests; 1s to 10m. It never cuts off a reply in progress. |

On a loopback `http` address, such as `http://localhost:8080`, the session cookie goes without `Secure` and the `__Host-` prefix, so the console works over plain HTTP on your computer.

### Browser apps

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | text | — | Comma-separated origins of web apps that may call the model API from a browser, such as `https://app.example.com,http://localhost:5173`. Only these get CORS headers, only on `/v1`, and `*` stops the gateway at startup. |

Without this setting, Kaeso sends no CORS headers. The [model API](../api/model-api.md) explains how a browser app calls Kaeso.

### Databases

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | text, secret | required | PostgreSQL connection URL. |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | integer | `20` | Most PostgreSQL connections per gateway instance, at least 1. Background jobs hold their locks on a few connections besides these. |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | text, secret | required | ClickHouse HTTP URL with user and password, such as `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | text | `kaeso` | ClickHouse database for request logs, of letters, digits and `_`. The gateway creates it when it is missing. |
| `KAESO_VALKEY_URL` | `valkey.url` | text, secret | required | Valkey (or Redis) URL with the password, such as `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | text | `kaeso:` | Prefix of every Valkey key and channel, to share one Valkey between installs. |

### Master keys

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | text, secret | required | Master keys as `id:base64,…`, the active key first. `kaeso master-key generate` makes one ([backups and master keys](backups.md)). |

### Email

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | text | — | SMTP server. Required unless `mail.transport` is `log`. |
| `KAESO_SMTP_PORT` | `smtp.port` | integer | `587` | SMTP port. |
| `KAESO_SMTP_TLS` | `smtp.tls` | text | `starttls` | `starttls`, `tls` for implicit TLS (usually port 465), or `none` for a local mail catcher only. |
| `KAESO_SMTP_USER` | `smtp.user` | text | — | SMTP user name, if the server needs one. Required with a password. |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | text, secret | — | SMTP password. |
| `KAESO_SMTP_FROM` | `smtp.from` | text | `Kaeso <kaeso@localhost>` | Sender of every email. `compose.yaml` sets `Kaeso <kaeso@` and your domain. |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | text | `smtp` | `smtp`, or `log` to only log the kind of each email, never its recipient or content. |

### Sign-in

These settings turn sign-in methods on or off for the whole install. The console shows them, and a method turned off here can't be turned on there ([sign-in and single sign-on](sign-in.md)).

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | bool | `true` | Allow sign-in with email and password. |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | bool | `true` | Allow sign-in with a link sent by email. |
| `KAESO_AUTH_OIDC` | `auth.oidc` | bool | `true` | Allow sign-in through an organization's OIDC identity provider. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | bool | `true` | Allow OIDC requests to private and loopback addresses. Turn it off where organization admins shouldn't reach your internal network through the gateway. |

### API key failures

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | integer | `100` | Failed API key lookups allowed per client address and window, on both APIs together. `0` turns the throttle off ([API key throttling](#api-key-throttling)). |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | text | `1m` | The window of `key_failures.per_address`, 1s to 1h. A blocked address waits at most this long. |

### models.dev

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | bool | `true` | Fetch model data from models.dev. Without it, the gateway uses the data built into the release. |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | text | `https://models.dev/api.json` | Where the model data comes from. |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | text | `24h` | How often the gateway fetches it. |

### Updates

The first three settings only seed a new install. After that, instance admins change them under **Settings** › **Instance** › **Updates** ([upgrades](upgrades.md)).

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_UPDATES_CHECK` | `updates.check` | bool | `true` | Look for new Kaeso releases twice a day. |
| `KAESO_UPDATES_CHANNEL` | `updates.channel` | text | `stable` | The releases offered: `stable`, or `prerelease` for pre-releases too. |
| `KAESO_UPDATES_APPLY` | `updates.apply` | bool | `false` | Let instance admins apply updates from the console through the updater. |
| `KAESO_UPDATES_RELEASES_URL` | `updates.releases_url` | text | `https://api.github.com/repos/logicplanes/kaeso/releases` | Where the list of releases comes from, in the form of GitHub's releases API. |
| `KAESO_UPDATER_URL` | `updater.url` | text | `http://updater:8081` | Address of the updater. |
| `KAESO_UPDATER_TOKEN` | `updater.token` | text, secret | — | The token the gateway and the updater share, at least 32 characters. Without it, the gateway never calls the updater. |

### Provider calls

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | text | `https://api.openai.com` | Base URL of the OpenAI API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | text | `https://api.anthropic.com` | Base URL of the Anthropic API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | text | `10s` | Time to connect to a provider, TCP and TLS; at most 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | text | `600s` | Total time of a provider request that doesn't stream; at most 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | text | `60s` | Streaming: time to the provider's first byte; at most 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | text | `300s` | Streaming: longest silence between two chunks; at most 24h. |

A model in a project's catalogue can set its own timeouts ([models and aliases](../console/models-and-aliases.md)).

### Streams to apps

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | text | `15s` | After this long without output, write a keepalive into an open stream so proxies keep the connection ([long streams](domains-and-tls.md#long-streams)). `0s` turns keepalives off; otherwise 1s to 300s. |

### Reasoning continuity

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | text | `1h` | How long the gateway keeps a model's reasoning before a tool call for an OpenAI Chat Completions app whose model speaks another format. `0s` turns it off; otherwise 1m to 24h. |

Chat Completions has no field for that reasoning, so the gateway holds it in Valkey until the app's next turn. It is encrypted under the master key, readable only with the API key that made the request, and never logged ([format translation](../concepts/format-translation.md)).

### Request and response bodies

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | text | `32MiB` | Largest request body of the model API. |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | text | `1MiB` | Largest request body of the Platform API. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | text | `128MiB` | Largest response body the gateway reads from a provider when it doesn't stream. A larger one fails the request with 502. |

### Request logs

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | text | `256MiB` | Memory for request log rows that wait for ClickHouse. Beyond it, the oldest rows are dropped. |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | text | `1s` | How often the gateway writes waiting rows to ClickHouse. |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | integer | `5000` | Write waiting rows as soon as this many wait, at least 1. |

### Instance defaults

These settings only seed a new install. After that, instance admins change them under **Settings** › **Instance** › **Settings**, or through the Platform API, and the stored values win.

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | text | `instance_admins` | Who may create organizations: `instance_admins` or `everyone`. |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | text | `invitation_only` | Who may create an account without an invitation: `invitation_only` or `everyone`. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | integer | `90` | Days new organizations keep request log metadata, at least 1 and at most the maximum below. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | integer | `30` | Days new organizations keep request log content, at least 1 and never longer than metadata. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | integer | `3650` | Longest metadata retention an organization may choose, at most 36500 days. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | integer | `3650` | Longest content retention an organization may choose, at most 36500 days. |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | bool | `true` | Whether organizations can be deleted. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | integer | `7` | Days a deleted organization stays restorable and read-only before it is removed for good, 1 to 90. |

In the console, instance admins can also allow account and organization creation for some email domains only.

### Logging

| Variable | TOML key | Type | Default | What it does |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | text | `info,sqlx=warn` | Log level or filter, such as `info` or `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT` | `log.format` | text | `json` | `json` for structured logs, `text` for development. |

## Provider settings

Each provider has its own settings, named after its id: the TOML key `providers.<id>.<field>` and the variable `KAESO_PROVIDERS_<ID>_<FIELD>`, with the id in upper case and each `-` as `_`. Kaeso calls two providers, `openai` and `anthropic`, and each has one field, `base_url` ([provider calls](#provider-calls)).

A provider id is lowercase letters and digits with single hyphens between them. A provider variable or table with any other id, or with the id of a provider Kaeso doesn't call, stops the gateway at startup. The message names it and lists the providers.

## API key throttling

The gateway counts failed API key lookups per client address in Valkey, so a client that guesses keys can't keep PostgreSQL busy. After `KAESO_KEY_FAILURES_PER_ADDRESS` failures within `KAESO_KEY_FAILURES_WINDOW`, the address is blocked until the window ends. Every key from it that isn't in memory then gets 429 with `Retry-After`, in the API's own format.

- Only well-formed keys that match no key count. A malformed value, a disabled or expired key, and an unknown key repeated within 30 seconds don't.
- Valid keys never count. A key the instance used in the last 30 seconds still works from a blocked address; other valid keys wait for the window to end.
- IPv6 addresses count per /64.
- Behind a proxy, the client address comes from `X-Forwarded-For` only when the proxy is in `KAESO_TRUSTED_PROXIES`. Otherwise every client shares the proxy's allowance.
- While Valkey can't be reached, failures aren't counted.

Each block adds to `kaeso_key_failure_blocks_total` and logs `too many failed API key lookups from one address` with the address ([health and metrics](monitoring.md)).
