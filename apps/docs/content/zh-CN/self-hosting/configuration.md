---
title: 配置
description: 网关如何读取其设置，以及它的每一项设置。
order: 3
---

网关从 `KAESO_*` 环境变量读取设置，也可以辅以一个 TOML 文件。在 Compose 安装中：

- `compose.yaml` 读取的变量写在 `.env` 中，[.env 文件](installation.md#env-文件)列出了它们。
- 其他所有设置写在 `compose.override.yaml` 中，见[本地修改](installation.md#本地修改)。只写在 `.env` 中的设置不会传给网关。
- `compose.yaml` 已经设置了数据库 URL、主密钥文件和 JSON 日志格式，并根据 `KAESO_DOMAIN` 推导出 `KAESO_PUBLIC_URL`。

## 设置的读取方式

- **环境变量。** 每项设置都有一个，例如 `KAESO_LOG_LEVEL`。空变量视为未设置。
- **TOML 文件。** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` 从文件读取设置，下文的键以表的形式书写（先写 `[postgres]`，再写 `url = "…"`）。环境变量优先于文件，不是设置项的键视为错误。每个版本都附带 [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml)，其中每项非机密设置都填了默认值或示例值，可直接作为起点。
- **从文件读取机密。** 每项机密设置都可以改为从 `<VARIABLE>_FILE` 指定的文件读取，例如用于 Docker secret 的 `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys`。末尾的换行符会被忽略；同时设置变量及其 `_FILE` 视为错误。
- **启动时检查。** 缺少设置或设置无效时，网关拒绝启动，消息中会指明该设置，但从不输出机密。

| 类型 | 取值 |
| --- | --- |
| text | 文本、URL、地址（`0.0.0.0:8080`）、时长（`10s`、`5m`、`24h`）、大小（`32MiB`、`1MB` 或字节数）以及列出的选项 |
| bool | `true` 或 `false`；也可以是 `1` 和 `0`、`yes` 和 `no`、`on` 和 `off` |
| integer | 大于或等于零的整数 |

## 设置

### 监听

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | text | `0.0.0.0:8080` | 模型 API、Platform API 和健康检查的地址。 |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | text | `0.0.0.0:9100` | Prometheus 指标监听器的地址。切勿将其公开路由。 |
| `KAESO_PUBLIC_URL` | `public_url` | text | 必填 | 人和应用访问本安装所用的 URL，例如 `https://kaeso.example.com`：只含源，不带路径。必须是 `https`；开发时可以在回环地址上使用 `http`，例如 `http://localhost`。 |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | 以逗号分隔的反向代理网络，信任来自这些网络的 `X-Forwarded-For`，用于日志中的客户端地址、登录限流和 API 密钥限流。在 TOML 文件中写 `trusted_proxies = ""` 表示不信任任何代理；空变量则保留默认值。 |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | text | `30s` | 收到关闭信号后，进行中的请求和流还可以继续运行多久。届时仍未结束的请求会以应用所用格式的错误（503）结束，并照常计费和记录。 |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | text | `30s` | 客户端发送一个请求的请求头最多可用多久，以及一个连接在两个请求之间最多可空闲多久，超时后网关会关闭该连接；1 秒到 10 分钟。正在应答的请求（例如长流）不会因此被中断。 |

### 浏览器应用

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | text | — | 以逗号分隔的 Web 应用源，这些应用可以从浏览器调用模型 API，例如 `https://app.example.com,http://localhost:5173`：协议、主机和端口，不带路径。只有这些源会收到 CORS 响应头，且只在 `/v1` 上；未列出任何源时，Kaeso 完全不发送 CORS 响应头。`*` 以及不是源的值会使网关在启动时停止。请参阅[从浏览器调用](../api/model-api.md#从浏览器调用)。 |

### 数据库

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | text，机密 | 必填 | PostgreSQL 连接 URL。 |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | integer | `20` | 每个网关实例的最大 PostgreSQL 连接数，至少为 1。后台任务另外使用各自的短期连接持有锁。 |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | text，机密 | 必填 | ClickHouse HTTP URL，包含用户和密码，例如 `http://kaeso:secret@clickhouse:8123`。 |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | text | `kaeso` | 存放请求日志的 ClickHouse 数据库，由字母、数字和 `_` 组成；不存在时自动创建。 |
| `KAESO_VALKEY_URL` | `valkey.url` | text，机密 | 必填 | Valkey（或 Redis）URL，需要密码时包含密码，例如 `redis://:secret@valkey:6379`。 |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | text | `kaeso:` | 每个 Valkey 键和频道的前缀，用于在多个安装之间共用一个 Valkey。 |

### 主密钥

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | text，机密 | 必填 | 以 `id:base64,…` 形式给出的主密钥，当前使用的密钥排在最前。用 `kaeso master-key generate` 生成。参见[备份和主密钥](backups.md)。 |

### 电子邮件

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | text | — | SMTP 服务器。除非 `mail.transport` 为 `log`，否则必填。 |
| `KAESO_SMTP_PORT` | `smtp.port` | integer | `587` | SMTP 端口。 |
| `KAESO_SMTP_TLS` | `smtp.tls` | text | `starttls` | `starttls`、`tls`（隐式 TLS，通常为 465 端口）或 `none`（仅用于本地邮件捕获工具）。 |
| `KAESO_SMTP_USER` | `smtp.user` | text | — | SMTP 用户名，服务器需要时填写。设置了密码时必填。 |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | text，机密 | — | SMTP 密码。 |
| `KAESO_SMTP_FROM` | `smtp.from` | text | `Kaeso <kaeso@localhost>` | Kaeso 发送的每封邮件的发件人。Compose 文件将其设为 `Kaeso <kaeso@` 加上该安装的域名。 |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | text | `smtp` | `smtp`，或 `log`：只记录本应发送一封邮件及其类型，从不记录收件人或内容。用于试用 Kaeso 和开发。 |

### 登录

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | bool | `true` | 允许用电子邮件和密码登录。 |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | bool | `true` | 允许通过电子邮件发送的登录链接登录。 |
| `KAESO_AUTH_OIDC` | `auth.oidc` | bool | `true` | 允许通过组织的 OIDC 身份提供方登录。 |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | bool | `true` | 允许 OIDC 发现和 Token 请求访问私有地址和回环地址。如果不希望组织管理员通过网关访问内部网络，请在安装中关闭此项。 |

### API 密钥查找失败

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | integer | `100` | 每个客户端地址在每个窗口内允许的 API 密钥查找失败次数，模型 API 和 Platform API 合并计算；超过后，未缓存的密钥会被以 429 拒绝，直到窗口结束。`0` 关闭限流。参见[API 密钥限流](#api-密钥限流)。 |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | text | `1m` | `key_failures.per_address` 的窗口，1s 到 1h。被封锁的地址最多等待这么长时间。 |

### models.dev

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | bool | `true` | 定期获取 models.dev 数据。无法访问互联网的安装请关闭此项；届时使用版本内置的快照。 |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | text | `https://models.dev/api.json` | 获取 models.dev 数据的地址。 |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | text | `24h` | 获取 models.dev 数据的频率。 |

### 服务商调用

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | text | `https://api.openai.com` | OpenAI API 的基础 URL。 |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | text | `https://api.anthropic.com` | Anthropic API 的基础 URL。 |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | text | `10s` | 连接服务商的超时时间，包括 TCP 和 TLS；最长 24h。 |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | text | `600s` | 非流式服务商请求的总超时时间；最长 24h。 |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | text | `60s` | 流式：等待服务商返回第一个字节的时间；最长 24h。 |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | text | `300s` | 流式：两个数据块之间允许的最长静默时间；最长 24h。 |

单个模型的超时时间在项目目录中的该模型上设置；参见[模型和别名](../console/models-and-aliases.md)。

### 发往应用的流

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | text | `15s` | 当流式回复处于打开状态、且这段时间内没有向应用写入任何内容时，写入一条应用的 SDK 会忽略的保活消息（OpenAI 格式为 SSE 注释，Anthropic 为 `ping` 事件），使带有空闲超时的代理保持连接。`0s` 关闭保活；否则为 1s 到 300s。参见[长时间的流](domains-and-tls.md#长时间的流)。 |

### 推理连续性

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | text | `1h` | 对于使用 OpenAI Chat Completions、而其模型以另一种格式调用的应用，网关将工具调用之前的推理保留多久（Chat Completions 没有存放推理的字段），以便应用的下一轮请求将其带回。保存在 Valkey 中，使用主密钥加密，只有发出该请求的 API 密钥才能读取；从不记录到日志。`0s` 关闭此功能；否则为 1m 到 24h。参见[格式转换](../concepts/format-translation.md)。 |

### 请求体与响应体

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | text | `32MiB` | 模型 API 的最大请求体。 |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | text | `1MiB` | Platform API 的最大请求体。 |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | text | `128MiB` | Kaeso 从服务商读取的非流式响应体的最大大小；超出时请求以 502 失败。 |

### 请求日志

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | text | `256MiB` | 等待写入 ClickHouse 的请求日志行所占的内存；超过后丢弃最旧的行。 |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | text | `1s` | 将缓冲的日志行写入 ClickHouse 的频率。 |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | integer | `5000` | 等待的缓冲日志行达到此数量时立即写入，至少为 1。 |

### 实例默认值

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | text | `instance_admins` | 新安装中谁可以创建组织：`instance_admins` 或 `everyone`。实例管理员之后可以在实例设置中修改，还可以在那里允许某些电子邮件域名的用户创建。 |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | text | `invitation_only` | 新安装中谁可以在没有邀请的情况下创建账户：`invitation_only` 或 `everyone`。实例管理员之后可以在实例设置中修改，还可以在那里只允许某些电子邮件域名。 |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | integer | `90` | 新组织请求日志元数据的默认保留期限，以天计；至少为 1，且不超过下方的最大值。 |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | integer | `30` | 新组织请求日志内容的默认保留期限，以天计；至少为 1，不得长于元数据的保留期限，且不超过下方的最大值。 |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | integer | `3650` | 组织可以选择的最长元数据保留期限，以天计；最多 36500。 |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | integer | `3650` | 组织可以选择的最长内容保留期限，以天计；最多 36500。 |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | bool | `true` | 新安装中是否可以删除组织。实例管理员之后可以在实例设置中修改。 |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | integer | `7` | 已删除的组织在被永久移除之前保持可恢复和只读状态的天数，1 到 90。 |

### 日志

| 变量 | TOML 键 | 类型 | 默认值 | 作用 |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | text | `info,sqlx=warn` | 日志级别或过滤器，例如 `info` 或 `info,kaeso_server=debug`。 |
| `KAESO_LOG_FORMAT` | `log.format` | text | `json` | `json` 表示结构化日志，`text` 用于开发。 |

## 服务商设置

每个服务商都有自己的设置：TOML 键为 `providers.<id>.<field>`，变量为 `KAESO_PROVIDERS_<ID>_<FIELD>`，其中 ID 改为大写，每个 `-` 换成 `_`。Kaeso 调用两个服务商：`openai` 和 `anthropic`，各有一个字段 `base_url`（见[服务商调用](#服务商调用)）。

- 服务商 ID 由小写字母和数字组成，中间以单个连字符分隔（`^[a-z0-9]+(-[a-z0-9]+)*$`）。凡出现 ID 的地方都适用同一规则，服务商密钥和目录模型也不例外。
- 如果 `KAESO_PROVIDERS_…` 变量或 `[providers.…]` 表中的 ID 违反该规则，或指向 Kaeso 不调用的服务商，网关会在启动时停止，消息中会指明该项并列出所有服务商。

## API 密钥限流

网关最近没有见过的 API 密钥，每个都要在 PostgreSQL 中查找一次。未知密钥会被记住 30 秒，因此持续发送旧密钥的应用，每 30 秒、每个网关实例只产生一次查找；但每个请求都编造新密钥的客户端，每个请求都会产生一次查找。为此，网关在 Valkey 中按客户端地址统计查找失败的次数，模型 API 和 Platform API 合并计算。在 `KAESO_KEY_FAILURES_WINDOW`（默认 1 分钟）内失败达到 `KAESO_KEY_FAILURES_PER_ADDRESS`（默认 100）次后，该地址会被封锁到窗口结束：来自它的每个未缓存的密钥都会收到 429 和 `Retry-After`，以该 API 自己的格式返回，不查询数据库。

- 只有格式正确但找不到对应密钥的查找才计数。格式错误的值、已停用或已过期的密钥，以及 30 秒内重复出现的未知密钥都不计数。
- 有效密钥从不计数。同一实例在最近 30 秒内用过的密钥直接从内存应答，即使来自被封锁的地址；其他有效密钥需等到窗口结束。
- IPv6 地址按 /64 计数。
- 位于反向代理之后时，只有代理在 `KAESO_TRUSTED_PROXIES` 中，客户端地址才取自 `X-Forwarded-For`。否则所有客户端看起来都是该代理，共用它的额度。
- 无法连接 Valkey 期间，失败不计数。

每次封锁都会使 `kaeso_key_failure_blocks_total` 加一，并记录带有该地址的日志 `too many failed API key lookups from one address`；参见[健康状态和指标](monitoring.md)。

## 说明

- **实例默认值**只用于初始化新安装的实例设置。此后，实例管理员在**设置** › **实例** › **设置**中或通过 Platform API（`instance/settings`）修改账户和组织的创建、组织的删除及其宽限期，以及保留期限的默认值和最大值；以保存的值为准。
- 在这里关闭的**登录方式**无法在控制台中开启，控制台按配置显示它们的状态。
- **开发。** 当 `KAESO_PUBLIC_URL` 是回环地址上的 `http` URL（例如 `http://localhost:8080`）时，会话 Cookie 不带 `Secure` 也不带 `__Host-` 前缀，因此控制台可以在本机通过普通 HTTP 使用。其他任何 `http` URL 都会被拒绝。
