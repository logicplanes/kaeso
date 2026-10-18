---
title: 故障排除
description: 找到在实例上看到的消息或现象，并加以解决。
order: 11
---

找到您看到的消息或现象，然后按对应的方法解决。大多数问题都会出现在网关日志中，日志会指明涉及的设置、请求或服务：

```bash
docker compose logs --tail 100 gateway
```

## 网关不断重启

设置缺失或无效时，网关会停止，其容器随即重启。`docker compose up -d --wait` 因此在网关上失败。

1. 运行 `docker compose logs --tail 20 gateway`，找到以 `configuration:` 开头的那一行。它会指明是哪项设置，但从不显示其值。
2. 在 `compose.override.yaml` 中更正该设置；如果它属于 [.env 中的设置](configuration.md#env-中的设置)，就在 `.env` 中更正。
3. 再次运行 `docker compose up -d --wait`。

## Compose 要求在 .env 中设置变量

`.env` 缺少必需的变量，Compose 拒绝启动并指明该变量，例如 `set KAESO_DOMAIN in .env`。

1. 在 `.env` 中设置 `KAESO_DOMAIN`、`POSTGRES_PASSWORD`、`POSTGRES_SUPERUSER_PASSWORD`、`CLICKHOUSE_PASSWORD` 和 `VALKEY_PASSWORD`。
2. 确认 `master-keys` 位于 `compose.yaml` 旁边。
3. 再次运行 `docker compose up -d --wait`。

## KAESO_SMTP_HOST is required

网关没有可用来发送邮件的 SMTP 服务器。

1. 按[设置 SMTP](email.md#设置-smtp) 中的说明，把 SMTP 服务器写进 `.env`。
2. 或者，要在没有 SMTP 服务器的情况下试用 Kaeso，在 `.env` 中设置 `KAESO_MAIL_TRANSPORT=log`。
3. 运行 `docker compose up -d --wait`。

## Stored secrets are encrypted with master keys that are not configured

数据库中有机密是用 `master-keys` 中缺少的主密钥加密的。消息会指明该密钥 ID。

1. 在您保存的主密钥副本中找到该 ID 对应的密钥。
2. 把它加回 `master-keys`，放在当前使用的密钥之后（[轮换主密钥](backups.md#轮换主密钥)）。
3. 用 `docker compose up -d --wait --force-recreate gateway` 重新创建网关。

## The database was migrated by a newer version of Kaeso

较旧的网关正在已被较新版本迁移过的数据库上启动，而迁移无法撤销。

1. 重新启动较新版本的 `compose.yaml`。
2. 要回到较旧的版本，请恢复升级前的备份（[降级](upgrades.md#降级)）。

## PostgreSQL 拒绝网关的密码

PostgreSQL 的卷创建之后，`.env` 中的 `POSTGRES_PASSWORD` 被修改了。PostgreSQL 仍保留当时的密码。

1. 把原来的密码放回 `.env`。
2. 运行 `docker compose up -d --wait`。

## 网站没有证书

Caddy 无法为 `KAESO_DOMAIN` 获取证书，通常是 DNS 或防火墙的原因。

1. 运行 `docker compose logs caddy`，查看证书颁发机构的回应。
2. 确认域名的 DNS 记录指向服务器，且端口 80 和 443 已向互联网开放。
3. 运行 `docker compose restart caddy`。

## 没有设置链接，或链接已过期

设置链接仅可使用一次，24 小时内有效，并且只在实例还没有用户时可用。打印一个新链接：

```bash
docker compose exec gateway kaeso setup-link
```

如果 Kaeso 已经设置完成，命令会提示这一点；请直接在您的域名上登录。

## 收不到邮件

网关无法连接 SMTP 服务器，或服务器拒绝了邮件。

1. 在**设置** › **实例** › **设置**中选择**发送测试邮件**。
2. 在网关日志中查看 SMTP 服务器的回应。
3. 检查 `.env` 中的 `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS` 和凭据，并确认服务器允许以 `KAESO_SMTP_FROM` 发信。

设置 `KAESO_MAIL_TRANSPORT=log` 时，网关只记录邮件，从不发送（[邮件](email.md)）。

## 请求失败

错误以应用所调用的格式返回，消息会说明该怎么做。[错误](../api/errors.md)列出了所有错误，以下是最常见的几种：

| 状态码 | 原因 | 解决方法 |
| --- | --- | --- |
| 400 | 没有为模型的服务商设置服务商密钥 | 在**服务商**中添加一个，或为项目分配一个 |
| 401 | 密钥缺失、未知、已停用、已归档或已过期 | 检查应用发送的密钥，以及它在 **API 密钥**中的状态 |
| 403 | 该密钥或其成员无权使用该模型 | 扩大该成员或密钥的模型访问范围 |
| 404 | 模型不在项目的目录中 | 在**模型**中添加它，或调用目录中已有的模型 |
| 429 | 速率限制或预算，或同一地址的未知密钥过多 | 等待 `Retry-After`，或提高消息中指明的限额 |
| 502、504 | 无法连接服务商、服务商未及时响应，或返回的响应大于 `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | 查看服务商的状态；请求日志会显示其错误 |
| 503 | Valkey 不可用而无法检查硬性预算，或网关正在重启 | 检查 [`/readyz`](monitoring.md#健康检查) 和 `docker compose logs valkey` |

在 OpenAI 格式中，网关自身的部分 503 错误会以 500 返回。

## 流在一分钟后中断

应用和 Kaeso 之间的代理或负载均衡器关闭了空闲太久的连接。

1. 在 `compose.override.yaml` 中把 `KAESO_STREAMS_KEEPALIVE_INTERVAL` 调低到代理的空闲超时以下。默认值是 15 秒。
2. 在您自己的代理后面时，关闭 `/v1/` 的响应缓冲。

[长时间的流](domains-and-tls.md#长时间的流)介绍了保活机制。

## 请求日志缺失或延迟

日志条目会在请求后几秒内出现。如果一直没有出现，说明 ClickHouse 不接受写入：`/readyz` 报告 `degraded`，`kaeso_log_buffer_rows` 持续增长。

1. 运行 `docker compose logs clickhouse`，解决其中报告的问题。
2. 在网关日志中搜索 `ClickHouse migrations failed`。

日志行会在网关内存中等待，最多到 `KAESO_LOGS_BUFFER_SIZE`，之后最旧的行会被丢弃。

## Kaeso 无法连接数据库

控制台和 Platform API 返回 503，并显示“Kaeso 暂时无法连接数据库，请稍后重试。”PostgreSQL 已停止或拒绝连接，[`/readyz`](monitoring.md#健康检查) 返回 `not_ready`。

1. 运行 `docker compose ps postgres` 和 `docker compose logs postgres`。
2. 解决问题的根源。最常见的是磁盘已满或密码被修改。

数据库一恢复，请求就会重新正常工作，无需重启。当请求需要 Valkey 或 ClickHouse 却无法连接时，Platform API 也会返回同样的 503。

## 有人丢失了双重验证

实例管理员可以在**设置** › **实例** › **用户**中，在该用户的侧滑面板里选择**重置双重验证**。如果没有能登录的实例管理员，就在服务器上重置：

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

无论哪种方式，该用户的身份验证器应用和恢复码都会被移除，会话随之结束，并会收到一封邮件。在要求双重验证的地方，用户下次登录时需要重新设置。

## 无人能通过单点登录登录

在没有密码的实例上，身份提供商出故障后就没有任何登录途径。不要重新开启密码登录：请为一名实例管理员打印一次性登录链接，见[重新登录](sign-in.md#重新登录)。

## 有人报告了请求 ID

控制台显示“网关出现问题；请将请求 req_… 报告给管理员。”网关以调用方无法解决的方式失败了，原因记录在它的日志中。

1. 在日志中搜索该 ID：`docker compose logs gateway | grep req_…`。
2. 阅读带有原因的 `internal error` 那一行。
