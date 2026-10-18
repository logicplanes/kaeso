---
title: 故障排除
description: 安装的常见问题及解决方法。
order: 11
---

先查看网关日志，它会指明涉及的设置、请求或服务：

```bash
docker compose logs --tail 100 gateway
```

## 网关无法启动

缺少设置或设置无效时，网关会停止并在消息中指明该设置；此时 `docker compose up -d --wait` 会因网关容器而失败，`docker compose ps` 显示它在反复重启。

| 消息内容 | 解决方法 |
| --- | --- |
| 缺少 `.env` 中的某个变量 | Compose 本身就会拒绝启动并指明该变量：设置 `KAESO_DOMAIN`、`POSTGRES_PASSWORD`、`POSTGRES_SUPERUSER_PASSWORD`、`CLICKHOUSE_PASSWORD` 和 `VALKEY_PASSWORD`，并确认 `compose.yaml` 旁边有 `master-keys` |
| 需要 `KAESO_SMTP_HOST` | 填写 SMTP 服务器，或设置 `KAESO_MAIL_TRANSPORT=log` 在没有邮件服务器的情况下试用 Kaeso；参见[电子邮件](email.md) |
| 其他设置无效 | 在 `compose.override.yaml` 中改正；[配置](configuration.md)列出了允许的值 |
| 某个主密钥 ID 未配置 | 数据库中有用 `master-keys` 中缺失的密钥加密的机密；把该密钥加回去，参见[主密钥](backups.md#主密钥) |
| 数据库已被更新版本的 Kaeso 迁移 | 旧镜像正在连接较新的数据库；启动较新的版本，或执行[降级](upgrades.md#降级) |
| PostgreSQL 或 ClickHouse 拒绝密码 | 数据库创建之后修改了 `.env` 中的密码，而数据库保留最初的密码；把旧密码改回去 |

## 没有设置链接，或链接已过期

设置链接只在安装还没有用户时存在，只能使用一次，有效期 24 小时。输出一个新链接：

```bash
docker compose exec gateway kaeso setup-link
```

如果 Kaeso 已经设置好，命令会说明这一点；请直接在自己的域名上登录。

## 无法登录没有密码的安装

在只通过单点登录登录的安装上，身份提供商故障，或设置会话在单点登录可用之前就已结束，都会导致无法登录。不要重新开启密码，而是用 `kaeso user sign-in-link` 为一位实例管理员输出一次性登录链接，见[重新登录](sign-in.md#重新登录)。

## 网站没有证书

Caddy 需要域名的 DNS 记录指向服务器，并且端口 80 和 443 处于开放状态。`docker compose logs caddy` 会显示证书颁发机构的答复；参见[证书](domains-and-tls.md#获取证书)。

## 有人丢失了第二重验证因素

实例管理员可在**设置** › **实例** › **用户**中，打开此人的侧滑面板，用**重置双重验证**重置。没有能登录的实例管理员时，在服务器上重置：

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

两种方式都会移除此人的身份验证器和恢复码，结束其会话，向其发送邮件，并记入实例审计日志。在要求双重验证的地方，此人会在下次登录时重新设置。

## 请求失败

错误以应用调用时所用的格式返回，并说明该怎么做；[错误](../api/errors.md)列出了全部错误。最常见的：

| 状态 | 原因 | 处理方法 |
| --- | --- | --- |
| 400 | 模型的服务商没有配置服务商密钥 | 在**服务商**中添加一个，或为项目分配一个 |
| 401 | 密钥缺失、未知、已停用、已归档或已过期 | 检查应用发送的密钥，以及它在 **API 密钥**中的状态 |
| 403 | 该密钥或其所属成员无权使用该模型 | 扩大成员或密钥的模型访问权限 |
| 404 | 该模型不在项目的目录中 | 在**模型**中添加，或调用目录中已有的模型 |
| 429 | 速率限制或预算，或同一地址的未知密钥过多 | 消息会指明是哪个限额；等待 `Retry-After`，或提高限额 |
| 502、504 | 无法连接服务商、服务商未及时响应，或其响应超过 `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | 检查服务商的状态；请求日志会显示其错误 |
| 503 | Valkey 宕机导致无法检查硬预算，或网关正在重启 | 检查 [`/readyz`](monitoring.md#健康检查) 和 `docker compose logs valkey` |

在 OpenAI 格式中，网关自身的部分 503 错误会以 500 返回。

## 流在一分钟后中断

应用和 Kaeso 之间的代理或负载均衡器关闭了空闲连接。流沉默期间，网关每 15 秒写入一次保活消息；如果路径上的代理更早关闭连接，请在 [`compose.override.yaml`](installation.md#修改服务) 中调低 `KAESO_STREAMS_KEEPALIVE_INTERVAL`。使用自己的代理时，还要为 `/v1/` 关闭响应缓冲。参见[长时间的流](domains-and-tls.md#长时间的流)。

## 请求日志缺失或延迟

日志条目会在请求后几秒出现。如果一直没有出现，可能是 ClickHouse 不接受写入：`/readyz` 报告 `degraded`，`kaeso_log_buffer_rows` 持续增长。日志行在内存中最多缓存到 `KAESO_LOGS_BUFFER_SIZE`，之后最旧的行会被丢弃。检查 `docker compose logs clickhouse`，并在网关日志中查找 `ClickHouse migrations failed`。

## 有人报告了请求 ID

当网关以调用方无法修复的方式出错时，Platform API 返回 500，控制台显示*网关出现问题；请将请求 req_… 报告给管理员。*一个请求的每一行[日志](monitoring.md#日志)都带有其 ID，因此 `docker compose logs gateway | grep req_…` 可以找到带有原因的 `internal error` 行。

## Kaeso 无法连接数据库

PostgreSQL 宕机或拒绝连接时，控制台和 Platform API 返回 503 和*Kaeso 暂时无法连接数据库，请稍后重试。*，[`/readyz`](monitoring.md#健康检查) 返回 `not_ready`，其中 `postgres` 为 `down`。检查 `docker compose ps postgres` 和 `docker compose logs postgres`；常见原因是磁盘已满或密码被修改。数据库恢复后，请求随即恢复正常，无需重启。

当请求需要 Valkey 或 ClickHouse 而无法连接时，Platform API 也返回同样的 503；只有请求日志和用量会改为说明它们暂时不可用。

## 收不到邮件

在**设置** › **实例** › **设置**中用**发送测试邮件**发送一封测试邮件，并在网关日志中查看 SMTP 服务器的答复。检查 `.env` 中的 `KAESO_SMTP_HOST`、端口、`KAESO_SMTP_TLS` 和凭据，并确认 SMTP 服务器允许从 `KAESO_SMTP_FROM` 中的地址发信。`KAESO_MAIL_TRANSPORT=log` 时，邮件只会记录到日志。参见[电子邮件](email.md)。
