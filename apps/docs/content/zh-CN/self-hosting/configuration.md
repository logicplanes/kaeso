---
title: 配置
description: Kaeso 实例的每项设置、它们的位置，以及网关如何读取它们。
order: 4
---

实例的设置保存在 `compose.yaml` 旁边。[第一张表](#env-中的设置)中的变量写在 `.env` 里，网关的其他设置都写在 `compose.override.yaml` 里：

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_STREAMS_KEEPALIVE_INTERVAL: 10s
```

用 `docker compose up -d --wait` 应用修改。只写在 `.env` 里的网关设置永远到不了网关，因为 Compose 只传递第一张表中的变量。

## .env 中的设置

`compose.yaml` 从 `.env` 读取这些变量，并由此得出网关的数据库 URL、主密钥文件和公开 URL。

| 变量 | 必填 | 设置内容 |
| --- | --- | --- |
| `KAESO_DOMAIN` | 是 | Caddy 提供服务并为其申请证书的域名。公开 URL 为 `https://` 加该域名。 |
| `POSTGRES_PASSWORD` | 是 | PostgreSQL 用户 `kaeso` 的密码，网关以该用户登录。 |
| `POSTGRES_SUPERUSER_PASSWORD` | 是 | PostgreSQL 超级用户 `postgres` 的密码。Kaeso 从不使用它。 |
| `CLICKHOUSE_PASSWORD` | 是 | ClickHouse 用户 `kaeso` 的密码。 |
| `VALKEY_PASSWORD` | 是 | Valkey 要求每个连接提供的密码。 |
| `KAESO_PUBLIC_URL` | 否 | 公开 URL 不是 `https://` 加域名时使用，例如在您自己的代理后面。 |
| `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 除非只记录邮件，否则需要 `KAESO_SMTP_HOST` | 网关发送邮件的方式（[电子邮件](#电子邮件)）。 |
| `KAESO_MAIL_TRANSPORT` | 否 | 设为 `log` 时只记录邮件，不发送。 |
| `KAESO_UPDATER_TOKEN`、`COMPOSE_PROFILES` | 否 | 网关和更新器共享的令牌，以及用于启动更新器的 `updater`（[更新器](upgrades.md#更新器)）。 |
| `KAESO_DIR` | 使用更新器时 | 实例文件夹，也是更新器的 Docker 代理唯一允许绑定挂载的来源。 |
| `DOCKER_SOCKET` | 否 | Docker 套接字不在 `/var/run/docker.sock` 时，供更新器使用的路径。 |
| `EDGE_SUBNET` | 否 | `edge` 网络的子网，默认为 `172.30.83.0/24`，网关信任来自该子网的 `X-Forwarded-For`。服务器已在使用这个网段时请修改它。 |
| `CADDY_MEMORY`、`GATEWAY_MEMORY` 等，`CADDY_PIDS`、`GATEWAY_PIDS` 等，以及 `VALKEY_MAXMEMORY` | 否 | 各服务的内存和进程数上限，以及 Valkey 存放数据的内存（[容器限制](#容器限制)）。 |

密码中只用字母和数字，就像 `openssl rand -hex` 生成的那样，因为它们会写进连接 URL。PostgreSQL 保留卷创建时的密码，之后修改 `POSTGRES_PASSWORD` 会让网关无法登录。ClickHouse 和 Valkey 每次启动时都从 `.env` 读取密码。

## 容器限制

`compose.yaml` 中的每个服务都有内存上限和进程数上限。默认值适合[最低配置的服务器](requirements.md#服务器)；某个服务需要更多资源时，在 `.env` 中调高对应的值：

| 服务 | 内存 | 进程数 |
| --- | --- | --- |
| `caddy` | `CADDY_MEMORY`，`512m` | `CADDY_PIDS`，`256` |
| `gateway` | `GATEWAY_MEMORY`，`2g` | `GATEWAY_PIDS`，`1024` |
| `console` | `CONSOLE_MEMORY`，`256m` | `CONSOLE_PIDS`，`128` |
| `postgres` | `POSTGRES_MEMORY`，`2g` | `POSTGRES_PIDS`，`512` |
| `clickhouse` | `CLICKHOUSE_MEMORY`，`4g` | `CLICKHOUSE_PIDS`，`4096` |
| `valkey` | `VALKEY_MEMORY`，`1g` | `VALKEY_PIDS`，`128` |
| `updater` | `UPDATER_MEMORY`，`256m` | `UPDATER_PIDS`，`128` |
| `docker-proxy` | `DOCKER_PROXY_MEMORY`，`128m` | `DOCKER_PROXY_PIDS`，`64` |

内存使用 Docker 的单位，例如 `768m` 或 `3g`。达到内存上限的服务会被停止并重新启动。ClickHouse 会按上限自行调整内存用量。

Valkey 把数据保持在 `VALKEY_MAXMEMORY` 之内，默认 `512mb`，低于 `VALKEY_MEMORY`，因此 Valkey 不会因内存不足被停止。它从不为腾出空间而驱逐键，因此速率或预算计数器不会在无提示的情况下丢失。Valkey 满时会拒绝新键：网关改为从 PostgreSQL 而不是缓存读取，放行受速率限制和软预算约束的请求，并对受硬预算约束的请求返回 `503`，直到重新有空间。Valkey 的大部分内存用于[推理连续性](#推理连续性)，其上限由 `continuity.reasoning_ttl` 决定。

Docker 会在每个服务的日志达到 10 MB 时轮转，并保留五个文件。

## 网关读取设置的方式

- **环境变量。** 每项设置都有一个，例如 `KAESO_LOG_LEVEL`。空变量视为未设置。
- **TOML 文件。** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` 从文件读取设置，使用下表中的 TOML 键。环境变量优先于文件，不是设置的键会报错。[`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) 设置了除机密以外的每项设置。
- **从文件读取机密。** 每个机密都可以改从 `<VARIABLE>_FILE` 指定的文件读取，例如 `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys`。末尾的换行符会被忽略，两者同时设置会报错。
- **启动时检查。** 设置缺失或无效时，网关不会启动。错误消息会指明该设置，但从不打印机密。

| 类型 | 取值 |
| --- | --- |
| 文本 | 文本、URL、地址（`0.0.0.0:8080`）、时长（`10s`、`5m`、`24h`）、大小（`32MiB`、`1MB` 或字节数）以及列出的选项 |
| 布尔 | `true` 或 `false`；也接受 `1` 和 `0`、`yes` 和 `no`、`on` 和 `off` |
| 整数 | 零或正整数 |

## 设置

### 监听

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | 文本 | `0.0.0.0:8080` | 模型 API、Platform API 和健康检查的地址。 |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | 文本 | `0.0.0.0:9100` | Prometheus 指标的地址。切勿公开转发。 |
| `KAESO_PUBLIC_URL` | `public_url` | 文本 | 必填 | 用户和应用使用的地址，例如 `https://kaeso.example.com`，不带路径。必须是 `https`，开发时可在回环地址上使用 `http`。 |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | 文本 | `127.0.0.0/8,::1/128` | 以逗号分隔的代理网段，网关信任来自这些代理的 `X-Forwarded-For`。TOML 文件中的 `trusted_proxies = ""` 不信任任何代理；空变量保留默认值。 |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | 文本 | `30s` | 收到关闭信号后，进行中的请求和流还能运行多久。届时仍未结束的以 503 结束，并照常计费和记录。 |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | 文本 | `30s` | 客户端发送请求头、或在两次请求之间保持连接空闲的最长时间；1s 到 10m。它从不中断正在进行的回复。 |
| `KAESO_SERVER_BODY_READ_TIMEOUT` | `server.body_read_timeout` | 文本 | `30s` | 网关等待请求体下一部分的最长时间，超时后以 400 拒绝请求；1s 到 10m。 |

在 `http` 回环地址上（例如 `http://localhost:8080`），会话 Cookie 不带 `Secure` 和 `__Host-` 前缀，因此控制台可以在您的电脑上通过普通 HTTP 使用。

### 浏览器应用

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | 文本 | — | 以逗号分隔的 Web 应用的源，这些应用可以从浏览器调用模型 API，例如 `https://app.example.com,http://localhost:5173`。只有它们获得 CORS 响应头，且仅限 `/v1`；设为 `*` 时网关不会启动。 |

没有这项设置时，Kaeso 不发送任何 CORS 响应头。[模型 API](../api/model-api.md) 介绍了浏览器应用如何调用 Kaeso。

### 数据库

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | 文本，机密 | 必填 | PostgreSQL 连接 URL。 |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | 整数 | `20` | 每个网关实例的 PostgreSQL 最大连接数，至少为 1。后台任务另用几条连接持有锁。 |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | 文本，机密 | 必填 | 带用户和密码的 ClickHouse HTTP URL，例如 `http://kaeso:secret@clickhouse:8123`。 |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | 文本 | `kaeso` | 存放请求日志的 ClickHouse 数据库，由字母、数字和 `_` 组成。不存在时网关会创建它。 |
| `KAESO_VALKEY_URL` | `valkey.url` | 文本，机密 | 必填 | 带密码的 Valkey（或 Redis）URL，例如 `redis://:secret@valkey:6379`。 |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | 文本 | `kaeso:` | 每个 Valkey 键和频道的前缀，便于相互信任的多个安装共用一个 Valkey。 |

### 主密钥

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | 文本，机密 | 必填 | 格式为 `id:base64,…` 的主密钥，当前使用的排在最前。用 `kaeso master-key generate` 生成（[备份和主密钥](backups.md)）。 |

### 电子邮件

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | 文本 | — | SMTP 服务器。除非 `mail.transport` 为 `log`，否则必填。 |
| `KAESO_SMTP_PORT` | `smtp.port` | 整数 | `587` | SMTP 端口。 |
| `KAESO_SMTP_TLS` | `smtp.tls` | 文本 | `starttls` | `starttls`，隐式 TLS 用 `tls`（通常是端口 465），`none` 仅用于本地邮件捕获工具。 |
| `KAESO_SMTP_USER` | `smtp.user` | 文本 | — | SMTP 用户名，服务器需要时填写。设置密码时必填。 |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | 文本，机密 | — | SMTP 密码。 |
| `KAESO_SMTP_FROM` | `smtp.from` | 文本 | `Kaeso <kaeso@localhost>` | 每封邮件的发件人。`compose.yaml` 设为 `Kaeso <kaeso@` 加您的域名。 |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | 文本 | `smtp` | `smtp`，或 `log`：只记录每封邮件的类型，从不记录收件人或内容。 |

### 登录

这些设置为整个实例开启或关闭密码和登录链接。控制台会显示它们，在这里关闭的方式无法在控制台中开启。单点登录没有实例设置：组织管理员连接身份提供商后，它就对该组织生效（[登录和单点登录](sign-in.md)）。

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | 布尔 | `true` | 允许用邮箱和密码登录。 |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | 布尔 | `true` | 允许用邮件中的链接登录。 |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | 布尔 | `false` | 允许向私有地址和回环地址发出 OIDC 请求，用于部署在您自己网络中的身份提供商。仅在允许组织管理员借网关访问您内部网络的情况下开启。链路本地地址和云元数据地址（例如 `169.254.169.254`）始终被拒绝。 |

### API 密钥查找失败

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | 整数 | `100` | 每个客户端地址在每个时间窗口内允许的 API 密钥查找失败次数，两个 API 合计。`0` 关闭限流（[API 密钥限流](#api-密钥限流)）。 |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | 文本 | `1m` | `key_failures.per_address` 的时间窗口，1s 到 1h。被封锁的地址最多等待这么久。 |

### models.dev

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | 布尔 | `true` | 从 models.dev 获取模型数据。关闭后，网关使用版本内置的数据。 |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | 文本 | `https://models.dev/api.json` | 模型数据的来源。 |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | 文本 | `24h` | 网关获取数据的间隔。 |

### 更新

前三项设置只用于初始化新实例。之后由实例管理员在**设置** › **实例** › **更新**中修改（[升级](upgrades.md)）。

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_UPDATES_CHECK` | `updates.check` | 布尔 | `true` | 每 5 分钟检查一次 Kaeso 新版本。 |
| `KAESO_UPDATES_CHANNEL` | `updates.channel` | 文本 | `stable` | 提供哪些版本：`stable`，或 `prerelease` 同时提供预发布版本。 |
| `KAESO_UPDATES_MODE` | `updates.mode` | 文本 | `manual` | 新实例通过更新器获取更新的方式：`manual`，由实例管理员逐次启动；或 `automatic`，在更新窗口内自动安装。 |
| `KAESO_UPDATES_RELEASES_URL` | `updates.releases_url` | 文本 | `https://update.kaeso.app/v1/releases.json` | 版本列表的来源：update.kaeso.app，或以 GitHub Releases API 格式响应的镜像。 |
| `KAESO_UPDATER_URL` | `updater.url` | 文本 | `http://updater:8081` | 更新器的地址。 |
| `KAESO_UPDATER_TOKEN` | `updater.token` | 文本，机密 | — | 网关和更新器共享的令牌，至少 32 个字符。没有它，网关从不调用更新器。 |

### 服务商调用

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | 文本 | `https://api.openai.com` | OpenAI API 的基础 URL。 |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | 文本 | `https://api.anthropic.com` | Anthropic API 的基础 URL。 |
| `KAESO_PROVIDERS_DEEPSEEK_BASE_URL` | `providers.deepseek.base_url` | 文本 | `https://api.deepseek.com` | DeepSeek API 的基础 URL。 |
| `KAESO_PROVIDERS_XAI_BASE_URL` | `providers.xai.base_url` | 文本 | `https://api.x.ai` | SpaceX API 的基础 URL。 |
| `KAESO_PROVIDERS_TOGETHERAI_BASE_URL` | `providers.togetherai.base_url` | 文本 | `https://api.together.ai` | Together AI API 的基础 URL。 |
| `KAESO_PROVIDERS_GROQ_BASE_URL` | `providers.groq.base_url` | 文本 | `https://api.groq.com/openai` | Groq API 的基础 URL。 |
| `KAESO_PROVIDERS_OPENROUTER_BASE_URL` | `providers.openrouter.base_url` | 文本 | `https://openrouter.ai/api` | OpenRouter API 的基础 URL。 |
| `KAESO_PROVIDERS_MISTRAL_BASE_URL` | `providers.mistral.base_url` | 文本 | `https://api.mistral.ai` | Mistral API 的基础 URL。 |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | 文本 | `10s` | 连接服务商的时间，含 TCP 和 TLS；最多 24h。 |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | 文本 | `600s` | 非流式服务商请求的总时间；最多 24h。 |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | 文本 | `60s` | 流式：等待服务商第一个字节的时间；最多 24h。 |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | 文本 | `300s` | 流式：两个数据块之间的最长静默时间；最多 24h。 |

项目目录中的模型可以设置自己的超时（[模型和别名](../console/models-and-aliases.md)）。

当网关的环境设置了代理时，网关会通过 `HTTPS_PROXY`（`http` 基础 URL 用 `HTTP_PROXY`，或 `ALL_PROXY`）中的代理调用服务商，`NO_PROXY` 中的主机除外。代理能看到 `http` 调用中的服务商密钥，因此通过代理时请使用 `https` 基础 URL。

### 发往应用的流

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | 文本 | `15s` | 流在这段时间内没有输出时，写入一条保活消息，让代理保持连接（[长时间的流](domains-and-tls.md#长时间的流)）。`0s` 关闭保活；否则为 1s 到 300s。 |
| `KAESO_STREAMS_MAX_DURATION` | `streams.max_duration` | 文本 | `0s` | 流式回复最长可运行的时间，超时后网关以错误事件结束它。`0s` 表示不限制。 |
| `KAESO_STREAMS_MAX_BYTES` | `streams.max_bytes` | 文本 | `0` | 流式回复最多可写给应用的字节数，例如 `64MiB`，超出后网关以错误事件结束它。`0` 表示不限制。 |

### 推理连续性

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | 文本 | `1h` | 当 OpenAI Chat Completions 应用调用的模型使用另一种格式时，网关保留该模型在工具调用前的推理内容的时间。`0s` 关闭此功能；否则为 1m 到 24h。 |

[格式转换](../concepts/format-translation.md#chat-completions)说明了网关如何保存这段推理。

### 请求体与响应体

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | 文本 | `8MiB` | 模型 API 请求体的上限。它能容纳一张达到 Anthropic 5 MB 上限、以 base64 编码的图片；处理大型 PDF 时请调高。 |
| `KAESO_BODY_LIMITS_IN_FLIGHT` | `body_limits.in_flight` | 文本 | `512MiB` | 模型 API 和 Playground 同时翻译请求时可使用的内存，根据收到的每个请求体及其消息、内容块和工具估算。请求在解码和编码期间占用其估算值，之后直到结束只保留仍在使用的部分：解码后的请求及其消息、内容块、工具和 Token ID，在仍可能请服务商统计其输入 Token 期间再保留一份副本，在服务商响应之前发送给它的请求体，以及在记录内容时的原始请求体。一个密钥最多占用四分之一，更大的请求占用这四分之一。10 秒内仍无空间的请求会得到 503 和 `Retry-After: 10`。 |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | 文本 | `1MiB` | Platform API 请求体的上限，包括 Playground 请求。 |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | 文本 | `128MiB` | 网关从服务商读取的非流式响应体的上限。超出时请求以 502 失败。 |

### 请求日志

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | 文本 | `256MiB` | 等待写入 ClickHouse 的请求日志行所占内存。超出时丢弃最旧的行。 |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | 文本 | `1s` | 网关把等待中的行写入 ClickHouse 的间隔。 |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | 整数 | `5000` | 等待的行数达到此值时立即写入，至少为 1。 |

### 实例默认值

这些设置只用于初始化新实例。之后由实例管理员在**设置** › **实例** › **设置**中或通过 Platform API 修改，保存的值优先。

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | 文本 | `instance_admins` | 谁可以创建组织：`instance_admins` 或 `everyone`。 |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | 文本 | `invitation_only` | 谁可以不经邀请创建账户：`invitation_only` 或 `everyone`。 |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | 整数 | `90` | 新组织保留请求日志元数据的天数，至少为 1，且不超过下面的上限。 |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | 整数 | `30` | 新组织保留请求日志内容的天数，至少为 1，且不长于元数据。 |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | 整数 | `3650` | 组织可选择的最长元数据保留期，最多 36500 天。 |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | 整数 | `3650` | 组织可选择的最长内容保留期，最多 36500 天。 |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | 布尔 | `true` | 是否可以删除组织。 |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | 整数 | `7` | 已删除的组织在被永久移除前保持可恢复和只读的天数，1 到 90。 |
| `KAESO_INSTANCE_TIMEZONE` | `instance.timezone` | 文本 | `UTC` | 新安装的 IANA 时区，例如 `Europe/Berlin`：控制台以该时区显示所有时间，自动更新也遵循该时区。设置页面会将其设为第一位用户的时区；实例管理员之后可在实例设置中更改。 |

在控制台中，实例管理员还可以只允许特定邮箱域名的用户创建账户和组织。

### 日志

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | 文本 | `info,sqlx=warn` | 日志级别或过滤器，例如 `info` 或 `info,kaeso_server=debug`。 |
| `KAESO_LOG_FORMAT` | `log.format` | 文本 | `json` | `json` 输出结构化日志，`text` 用于开发。 |

## 服务商设置

每个服务商都有以其 ID 命名的设置：TOML 键 `providers.<id>.<field>` 和变量 `KAESO_PROVIDERS_<ID>_<FIELD>`，其中 ID 为大写，每个 `-` 换成 `_`。Kaeso 调用的每个服务商都只有一个字段 `base_url`（[服务商调用](#服务商调用)）。

服务商 ID 由小写字母和数字组成，中间用单个连字符分隔。ID 不符合此规则、或指向 Kaeso 不调用的服务商的变量或表，会让网关在启动时停止。错误消息会指明它，并列出所有服务商。

## API 密钥限流

网关在 Valkey 中按客户端地址统计 API 密钥查找失败的次数，防止猜测密钥的客户端让 PostgreSQL 一直忙碌。在 `KAESO_KEY_FAILURES_WINDOW` 内失败达到 `KAESO_KEY_FAILURES_PER_ADDRESS` 次后，该地址在窗口结束前被封锁。此后来自该地址、且不在内存中的每个密钥都会收到 429 和 `Retry-After`，格式与所调用的 API 一致。

- 只有格式正确但不匹配任何密钥的值才计数。格式错误的值、已停用或已过期的密钥，以及 30 秒内重复出现的未知密钥都不计数。
- 有效密钥从不计数。本实例在过去 30 秒内用过的密钥，即使来自被封锁的地址也能使用；其他有效密钥需要等窗口结束。
- IPv6 地址按 /64 计数。
- 在代理后面时，只有当代理位于 `KAESO_TRUSTED_PROXIES` 中，客户端地址才取自 `X-Forwarded-For`。否则所有客户端共用代理的额度。
- Docker Desktop、Colima 和 rootless Docker 通过它们自己的代理转发发布的端口，因此 Caddy 看到的所有客户端都是同一个地址，所有客户端共用一份额度。如果需要按客户端限制，请在使用 root 网络模式的 Docker Engine 上运行 Kaeso。
- Valkey 无法访问期间，失败不计数。

每次封锁都会让 `kaeso_key_failure_blocks_total` 增加，并记录带有该地址的日志 `too many failed API key lookups from one address`（[健康状态和指标](monitoring.md)）。
