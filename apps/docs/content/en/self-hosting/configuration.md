---
title: Configuration
description: Every setting of a Kaeso install, where it goes and how the gateway reads it.
order: 4
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
| `KAESO_UPDATER_TOKEN`, `COMPOSE_PROFILES` | no | The token the gateway and the updater share, and `updater` to start the updater ([the updater](upgrades.md#the-updater)). |
| `KAESO_DIR` | with the updater | The install's folder, the only folder the updater's Docker proxy allows bind mounts from. |
| `DOCKER_SOCKET` | no | The path of Docker's socket for the updater, when it isn't `/var/run/docker.sock`. |
| `KAESO_TRUSTED_PROXIES` | no | The networks of proxies whose `X-Forwarded-For` the gateway believes, `127.0.0.0/8,::1/128` and `EDGE_SUBNET` by default. Set it to your own proxy's address ([your own reverse proxy](domains-and-tls.md#use-your-own-reverse-proxy)). |
| `EDGE_SUBNET` | no | The subnet of the `edge` network, `172.30.83.0/24` by default, whose `X-Forwarded-For` the gateway believes. Change it when the server already uses that range. |
| `CADDY_MEMORY`, `GATEWAY_MEMORY`, … and `CADDY_PIDS`, `GATEWAY_PIDS`, …, `VALKEY_MAXMEMORY` | no | The services' memory and process limits, and the memory Valkey keeps its data in ([container limits](#container-limits)). |

Use only letters and digits in the passwords, as `openssl rand -hex` writes them, because they go into connection URLs. PostgreSQL keeps the passwords it had when its volume was created, so changing `POSTGRES_PASSWORD` later locks the gateway out. ClickHouse and Valkey take their password from `.env` at every start.

## Container limits

Every service in `compose.yaml` has a memory limit and a process limit. The defaults suit the [minimum server](requirements.md#server); raise one in `.env` when a service needs more:

| Service | Memory | Processes |
| --- | --- | --- |
| `caddy` | `CADDY_MEMORY`, `512m` | `CADDY_PIDS`, `256` |
| `gateway` | `GATEWAY_MEMORY`, `2g` | `GATEWAY_PIDS`, `1024` |
| `console` | `CONSOLE_MEMORY`, `256m` | `CONSOLE_PIDS`, `128` |
| `postgres` | `POSTGRES_MEMORY`, `2g` | `POSTGRES_PIDS`, `512` |
| `clickhouse` | `CLICKHOUSE_MEMORY`, `4g` | `CLICKHOUSE_PIDS`, `4096` |
| `valkey` | `VALKEY_MEMORY`, `1g` | `VALKEY_PIDS`, `128` |
| `updater` | `UPDATER_MEMORY`, `256m` | `UPDATER_PIDS`, `128` |
| `docker-proxy` | `DOCKER_PROXY_MEMORY`, `128m` | `DOCKER_PROXY_PIDS`, `64` |

Memory takes Docker's units, such as `768m` or `3g`. Docker stops and restarts a service that reaches its memory limit. ClickHouse sizes its own memory use to the limit.

Valkey keeps its data within `VALKEY_MAXMEMORY`, `512mb` by default, which stays below `VALKEY_MEMORY` so Valkey is never stopped for memory. It never evicts a key to make room, so a rate or budget counter is never dropped without notice.

When Valkey is full, it refuses new keys. Until there is room again, the gateway serves from PostgreSQL instead of its cache. It lets requests under rate limits and soft budgets through, and answers `503` under hard budgets. Most of Valkey's memory holds [reasoning continuity](#reasoning-continuity), which `continuity.reasoning_ttl` bounds.

Docker rotates each service's log at 10 MB and keeps five files.

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

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_LISTEN`<br>`listen` | text | `0.0.0.0:8080` | Address of the model API, the Platform API and the health checks. |
| `KAESO_METRICS_LISTEN`<br>`metrics_listen` | text | `127.0.0.1:9100` | Address of the Prometheus metrics. Never route it publicly. The gateway image sets `0.0.0.0:9100`, so a port published from the container reaches it; [health and metrics](monitoring.md#metrics) shows how. |
| `KAESO_PUBLIC_URL`<br>`public_url` | text | required | The address people and apps use, such as `https://kaeso.example.com`, without a path. It must be `https`, except `http` on a loopback address for development. |
| `KAESO_TRUSTED_PROXIES`<br>`trusted_proxies` | text | `127.0.0.0/8,::1/128` | Comma-separated networks of proxies whose `X-Forwarded-For` the gateway believes. `trusted_proxies = ""` in the TOML file trusts none; an empty variable keeps the default. |
| `KAESO_SHUTDOWN_GRACE`<br>`shutdown_grace` | text | `30s` | How long requests and streams in flight may run after a shutdown signal. Those still open then end with 503 and are charged and logged. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT`<br>`server.header_read_timeout` | text | `30s` | How long a client may take to send a request's headers, or leave a connection idle between requests; 1s to 10m. It never cuts off a reply in progress. |
| `KAESO_SERVER_BODY_READ_TIMEOUT`<br>`server.body_read_timeout` | text | `30s` | How long the gateway waits for the next part of a request body before it refuses the request with 400; 1s to 10m. |
| `KAESO_SERVER_BODY_READ_DEADLINE`<br>`server.body_read_deadline` | text | `60s` | How long the gateway waits for a whole request body, however steadily its parts arrive, before it refuses the request with 400; 1s to 1h. An 8 MiB body, the model API's default limit, arrives in time at 1.2 Mbit/s; raise it for apps on slower links. |
| `KAESO_SERVER_MAX_CONNECTIONS`<br>`server.max_connections` | integer | `4096` | Connections the gateway serves at once, from apps or a proxy in front. Further ones wait in the operating system's queue until one closes. The default matches Linux's default listen queue, `net.core.somaxconn`. |

On a loopback `http` address, such as `http://localhost:8080`, the session cookie goes without `Secure` and the `__Host-` prefix, so the console works over plain HTTP on your computer.

### Browser apps

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS`<br>`cors.allowed_origins` | text | — | Comma-separated origins of web apps that may call the model API from a browser, such as `https://app.example.com,http://localhost:5173`. Only these get CORS headers, only on `/v1`, and `*` stops the gateway at startup. |

Without this setting, Kaeso sends no CORS headers. The [model API](../api/model-api.md) explains how a browser app calls Kaeso.

### Databases

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_POSTGRES_URL`<br>`postgres.url` | text, secret | required | PostgreSQL connection URL. |
| `KAESO_POSTGRES_POOL_SIZE`<br>`postgres.pool_size` | integer | `20` | Most PostgreSQL connections per gateway instance, at least 1. Background jobs hold their locks on a few connections besides these. |
| `KAESO_CLICKHOUSE_URL`<br>`clickhouse.url` | text, secret | required | ClickHouse HTTP URL with user and password, such as `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE`<br>`clickhouse.database` | text | `kaeso` | ClickHouse database for request logs, of letters, digits and `_`. The gateway creates it when it is missing. |
| `KAESO_VALKEY_URL`<br>`valkey.url` | text, secret | required | Valkey (or Redis) URL with the password, such as `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX`<br>`valkey.key_prefix` | text | `kaeso:` | Prefix of every Valkey key and channel, to share one Valkey between installs that trust each other. |

### Master keys

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_MASTER_KEYS`<br>`master_keys` | text, secret | required | Master keys as `id:base64,…`, the active key first. `kaeso master-key generate` makes one ([backups and master keys](backups.md)). |

### Email

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_SMTP_HOST`<br>`smtp.host` | text | — | SMTP server. Required unless `mail.transport` is `log`. |
| `KAESO_SMTP_PORT`<br>`smtp.port` | integer | `587` | SMTP port. |
| `KAESO_SMTP_TLS`<br>`smtp.tls` | text | `starttls` | `starttls`, `tls` for implicit TLS (usually port 465), or `none` for a local mail catcher only. |
| `KAESO_SMTP_USER`<br>`smtp.user` | text | — | SMTP user name, if the server needs one. Required with a password. |
| `KAESO_SMTP_PASSWORD`<br>`smtp.password` | text, secret | — | SMTP password. |
| `KAESO_SMTP_FROM`<br>`smtp.from` | text | `Kaeso <kaeso@localhost>` | Sender of every email. `compose.yaml` sets `Kaeso <kaeso@` and your domain. |
| `KAESO_MAIL_TRANSPORT`<br>`mail.transport` | text | `smtp` | `smtp`, or `log` to only log the kind of each email, never its recipient or content. |

### Sign-in

These settings turn passwords and sign-in links on or off for the whole install. The console shows them, and a method turned off here can't be turned on there. Single sign-on has no install setting: it works for an organization once its admin connects an identity provider ([sign-in and single sign-on](sign-in.md)).

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD`<br>`auth.password` | bool | `true` | Allow sign-in with email and password. |
| `KAESO_AUTH_MAGIC_LINK`<br>`auth.magic_link` | bool | `true` | Allow sign-in with a link sent by email. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS`<br>`auth.oidc_allow_private_networks` | bool | `false` | Allow OIDC requests to private and loopback addresses, for an identity provider on your own network. Turn it on only where organization admins may reach your internal network through the gateway. Link-local and cloud metadata addresses, such as `169.254.169.254`, stay refused. |

### API key failures

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS`<br>`key_failures.per_address` | integer | `100` | Failed API key lookups allowed per client address and window, on both APIs together. `0` turns the throttle off ([API key throttling](#api-key-throttling)). |
| `KAESO_KEY_FAILURES_WINDOW`<br>`key_failures.window` | text | `1m` | The window of `key_failures.per_address`, 1s to 1h. A blocked address waits at most this long. |

### models.dev

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC`<br>`models_dev.sync` | bool | `true` | Fetch model data from models.dev. Without it, the gateway uses the data built into the release. |
| `KAESO_MODELS_DEV_URL`<br>`models_dev.url` | text | `https://models.dev/api.json` | Where the model data comes from. It must be `https`, except `http` on a loopback address. Kaeso follows at most 3 redirects, each on the same host and over `https`. |
| `KAESO_MODELS_DEV_INTERVAL`<br>`models_dev.interval` | text | `24h` | How often the gateway fetches it. |

### Updates

The first three settings only seed a new install. After that, instance admins change them under **Instance** › **Updates** ([upgrades](upgrades.md)).

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_UPDATES_CHECK`<br>`updates.check` | bool | `true` | Look for new Kaeso releases every 5 minutes. |
| `KAESO_UPDATES_CHANNEL`<br>`updates.channel` | text | `stable` | The releases offered: `stable`, or `prerelease` for pre-releases too. |
| `KAESO_UPDATES_MODE`<br>`updates.mode` | text | `manual` | How a new install takes updates through the updater: `manual`, where an instance admin starts each one, or `automatic`, in the update window. |
| `KAESO_UPDATES_RELEASES_URL`<br>`updates.releases_url` | text | `https://update.kaeso.app/v1/releases.json` | Where the list of releases comes from: update.kaeso.app, or a mirror that answers like GitHub's releases API. |
| `KAESO_UPDATER_URL`<br>`updater.url` | text | `http://updater:8081` | Address of the updater. |
| `KAESO_UPDATER_TOKEN`<br>`updater.token` | text, secret | — | The token the gateway and the updater share, at least 32 characters. Without it, the gateway never calls the updater. |

### Provider calls

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL`<br>`providers.openai.base_url` | text | `https://api.openai.com` | Base URL of the OpenAI API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL`<br>`providers.anthropic.base_url` | text | `https://api.anthropic.com` | Base URL of the Anthropic API. |
| `KAESO_PROVIDERS_DEEPSEEK_BASE_URL`<br>`providers.deepseek.base_url` | text | `https://api.deepseek.com` | Base URL of the DeepSeek API. |
| `KAESO_PROVIDERS_XAI_BASE_URL`<br>`providers.xai.base_url` | text | `https://api.x.ai` | Base URL of the SpaceX API. |
| `KAESO_PROVIDERS_TOGETHERAI_BASE_URL`<br>`providers.togetherai.base_url` | text | `https://api.together.ai` | Base URL of the Together AI API. |
| `KAESO_PROVIDERS_GROQ_BASE_URL`<br>`providers.groq.base_url` | text | `https://api.groq.com/openai` | Base URL of the Groq API. |
| `KAESO_PROVIDERS_OPENROUTER_BASE_URL`<br>`providers.openrouter.base_url` | text | `https://openrouter.ai/api` | Base URL of the OpenRouter API. |
| `KAESO_PROVIDERS_MISTRAL_BASE_URL`<br>`providers.mistral.base_url` | text | `https://api.mistral.ai` | Base URL of the Mistral API. |
| `KAESO_PROVIDERS_VERCEL_BASE_URL`<br>`providers.vercel.base_url` | text | `https://ai-gateway.vercel.sh` | Base URL of the Vercel API. |
| `KAESO_PROVIDERS_COHERE_BASE_URL`<br>`providers.cohere.base_url` | text | `https://api.cohere.ai/compatibility` | Base URL of the Cohere API. |
| `KAESO_PROVIDERS_OLLAMA_CLOUD_BASE_URL`<br>`providers.ollama-cloud.base_url` | text | `https://ollama.com` | Base URL of the Ollama Cloud API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT`<br>`upstream.connect_timeout` | text | `10s` | Time to connect to a provider, TCP and TLS; at most 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT`<br>`upstream.request_timeout` | text | `600s` | Total time of a provider request that doesn't stream; at most 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT`<br>`upstream.first_byte_timeout` | text | `60s` | Streaming: time to the provider's first byte; at most 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`<br>`upstream.stream_idle_timeout` | text | `300s` | Streaming: longest silence between two chunks; at most 24h. |

A model in a project's catalogue can set its own timeouts ([models and aliases](../console/models-and-aliases.md)).

The gateway calls providers through the proxy in `HTTPS_PROXY` (or `HTTP_PROXY` for `http` base URLs, or `ALL_PROXY`) when its environment sets one, except for the hosts in `NO_PROXY`. The proxy sees the provider keys of `http` calls, so use `https` base URLs through it. Custom providers are the exception: the gateway calls them directly ([custom providers](#custom-providers)).

### Custom providers

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_CUSTOM_PROVIDERS_ENABLED`<br>`custom_providers.enabled` | bool | `true` | Let organization admins add their own providers, such as a self-hosted vLLM or an internal proxy ([custom providers](../console/custom-providers.md)). `false` refuses requests to the ones they added and keeps them. |
| `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS`<br>`custom_providers.allowed_networks` | text | — | Comma-separated private, loopback or other non-public networks custom providers may reach, such as `10.0.0.0/8` or `127.0.0.1/32`. Plain `http` reaches only these. |

An organization admin chooses a custom provider's base URL, so the gateway guards where it connects. For every connection, it resolves the host's name and checks each address. It refuses private, loopback, link-local and cloud metadata addresses unless `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` lists their network. A name that later resolves to another address is checked again on the next connection. Link-local and cloud metadata addresses, such as `169.254.169.254`, stay refused even when a listed network covers them.

The gateway also follows no redirects from a custom provider, and calls it without the proxy in `HTTPS_PROXY`, because a proxy would resolve the name past these checks. Allow a network only where every organization admin may reach the servers in it through the gateway. For a vLLM on the same host as the gateway's containers, list the Docker network, such as `172.16.0.0/12`.

### Streams to apps

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL`<br>`streams.keepalive_interval` | text | `15s` | After this long without output, write a keepalive into an open stream so proxies keep the connection ([long streams](domains-and-tls.md#long-streams)). `0s` turns keepalives off; otherwise 1s to 300s. |
| `KAESO_STREAMS_MAX_DURATION`<br>`streams.max_duration` | text | `0s` | Longest time a streamed reply may run before the gateway ends it with an error event. `0s` sets no limit. |
| `KAESO_STREAMS_MAX_BYTES`<br>`streams.max_bytes` | text | `0` | Most bytes, such as `64MiB`, a streamed reply may write to the app before the gateway ends it with an error event. `0` sets no limit. |

### Reasoning continuity

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL`<br>`continuity.reasoning_ttl` | text | `1h` | How long the gateway keeps a model's reasoning before a tool call for an OpenAI Chat Completions app whose model speaks another format. `0s` turns it off; otherwise 1m to 24h. |

[Format translation](../concepts/format-translation.md#chat-completions) explains how the gateway keeps it.

### Request and response bodies

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API`<br>`body_limits.model_api` | text | `8MiB` | Largest request body of the model API. It fits an image at Anthropic's 5 MB limit as base64; raise it for large PDFs. |
| `KAESO_BODY_LIMITS_IN_FLIGHT`<br>`body_limits.in_flight` | text | `512MiB` | Memory the model API and the Playground may use at once to translate requests, estimated from each body as received and from its messages, blocks and tools. A request holds its estimate while it's decoded and encoded, then until it ends only what stays allocated: its decoded request with its messages, blocks, tools and token ids, a second copy of it while the provider may still be asked to count its input tokens, the body sent to the provider until it answers, and its raw body while content is logged. One key holds at most a quarter, and a request larger than that takes the quarter. A request that finds no room within 10 seconds gets 503 with `Retry-After: 10`. |
| `KAESO_BODY_LIMITS_PLATFORM_API`<br>`body_limits.platform_api` | text | `1MiB` | Largest request body of the Platform API, Playground requests included. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE`<br>`body_limits.provider_response` | text | `128MiB` | Largest response body the gateway reads from a provider when it doesn't stream. A larger one fails the request with 502. |

### Request logs

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE`<br>`logs.buffer_size` | text | `256MiB` | Memory for request log rows that wait for ClickHouse. Beyond it, the oldest rows are dropped. |
| `KAESO_LOGS_FLUSH_INTERVAL`<br>`logs.flush_interval` | text | `1s` | How often the gateway writes waiting rows to ClickHouse. |
| `KAESO_LOGS_FLUSH_ROWS`<br>`logs.flush_rows` | integer | `5000` | Write waiting rows as soon as this many wait, at least 1. |

### Instance defaults

These settings only seed a new install. After that, instance admins change them under **Instance** › **General**, the retention under **Instance** › **Logging**, or through the Platform API. The stored values win.

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION`<br>`instance.org_creation` | text | `instance_admins` | Who may create organizations: `instance_admins` or `everyone`. |
| `KAESO_INSTANCE_ACCOUNT_CREATION`<br>`instance.account_creation` | text | `invitation_only` | Who may create an account without an invitation: `invitation_only` or `everyone`. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS`<br>`instance.metadata_retention_days` | integer | `90` | Days new organizations keep request log metadata, at least 1 and at most the maximum below. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS`<br>`instance.content_retention_days` | integer | `30` | Days new organizations keep request log content, at least 1 and never longer than metadata. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS`<br>`instance.max_metadata_retention_days` | integer | `3650` | Longest metadata retention an organization may choose, at most 36500 days. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS`<br>`instance.max_content_retention_days` | integer | `3650` | Longest content retention an organization may choose, at most 36500 days. |
| `KAESO_INSTANCE_ORG_DELETION`<br>`instance.org_deletion` | bool | `true` | Whether organizations can be deleted. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS`<br>`instance.org_deletion_grace_days` | integer | `7` | Days a deleted organization stays restorable and read-only before it is removed for good, 1 to 90. |
| `KAESO_INSTANCE_TIMEZONE`<br>`instance.timezone` | text | `UTC` | The IANA time zone of a new install, such as `Europe/Berlin`: the console shows every time in it and automatic updates follow it. The setup page sets it to the first user's time zone; instance admins change it later in the instance settings. |

In the console, instance admins can also allow account and organization creation for some email domains only.

### Logging

| Variable<br>TOML key | Type | Default | What it does |
| --- | --- | --- | --- |
| `KAESO_LOG_LEVEL`<br>`log.level` | text | `info,sqlx=warn` | Log level or filter, such as `info` or `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT`<br>`log.format` | text | `json` | `json` for structured logs, `text` for development. |

## Provider settings

Each provider has its own settings, named after its id: the TOML key `providers.<id>.<field>` and the variable `KAESO_PROVIDERS_<ID>_<FIELD>`, with the id in upper case and each `-` as `_`. Every provider Kaeso calls has one field, `base_url` ([provider calls](#provider-calls)).

A provider id is lowercase letters and digits with single hyphens between them. A provider variable or table with any other id, or with the id of a provider Kaeso doesn't call, stops the gateway at startup. The message names it and lists the providers.

## API key throttling

The gateway counts failed API key lookups per client address in Valkey, so a client that guesses keys can't keep PostgreSQL busy. After `KAESO_KEY_FAILURES_PER_ADDRESS` failures within `KAESO_KEY_FAILURES_WINDOW`, the address is blocked until the window ends. Every key from it that isn't in memory then gets 429 with `Retry-After`, in the API's own format.

- Only well-formed keys that match no key count. A malformed value, a disabled or expired key, and an unknown key repeated within 30 seconds don't.
- Valid keys never count. A key the instance used in the last 30 seconds still works from a blocked address; other valid keys wait for the window to end.
- IPv6 addresses count per /64.
- Behind a proxy, the client address comes from `X-Forwarded-For` only when the proxy is in `KAESO_TRUSTED_PROXIES`. Otherwise every client shares the proxy's allowance.
- Docker Desktop, Colima and rootless Docker pass published ports through a proxy of their own. Caddy then sees every client at one address, and all clients share one allowance. To keep the limit per client, run Kaeso on Docker Engine with rootful networking.
- While Valkey can't be reached, failures aren't counted.

Each block adds to `kaeso_key_failure_blocks_total` and logs `too many failed API key lookups from one address` with the address ([health and metrics](monitoring.md)).
