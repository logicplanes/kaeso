---
title: 故障排除
description: 安装中的常见问题及其解决方法。
order: 9
---

先查看网关的日志。日志会指明相关的设置、请求或服务。

```bash
docker compose logs --tail 100 gateway
```

## 网关无法启动

网关在启动时检查其设置，出错时会停止并给出指明该设置的消息。此时 `docker compose up -d --wait` 会因网关容器出错而中止，`docker compose ps` 会显示它在不断重启。

- 检查 `.env` 是否设置了 `KAESO_DOMAIN`、`POSTGRES_PASSWORD`、`POSTGRES_SUPERUSER_PASSWORD`、`CLICKHOUSE_PASSWORD` 和 `VALKEY_PASSWORD`，以及 `compose.yaml` 旁边是否存在 `master-keys`。缺少其中任何一个变量时，Compose 本身就会拒绝启动并指明缺少的那一个。
- 关于 `KAESO_SMTP_HOST` 的消息表示邮件无处可发：添加 SMTP 服务器，或者设置 `KAESO_MAIL_TRANSPORT=log`，在没有邮件服务器的情况下试用 Kaeso。参见[电子邮件](email.md)。
- 检查 `compose.override.yaml` 中的每个值是否都是[配置](configuration.md)所允许的。

如果消息指明了一个未配置的主密钥 ID，说明数据库中有用某个密钥加密的机密，而 `master-keys` 中缺少该密钥。将该密钥加回去；参见[备份和主密钥](backups.md#主密钥)。

如果消息说数据库已被更新版本的 Kaeso 迁移过，说明一个较旧的镜像正在针对较新的数据库启动。请启动较新的版本，或者恢复升级前所做的备份。

如果 PostgreSQL 或 ClickHouse 拒绝了网关的密码，说明数据库创建之后 `.env` 中的密码被修改过；数据库会保留创建时的密码。将旧密码改回 `.env` 中。

## 没有设置链接，或链接已过期

设置链接只在安装中还没有用户时输出，只能使用一次，有效期为 24 小时。输出一个新链接：

```bash
docker compose exec gateway kaeso setup-link
```

如果 Kaeso 已经设置完成，该命令会说明这一点；请改为在自己的域名上登录。

## 网站没有证书

Caddy 需要域名的 DNS 记录指向该服务器，并且 80 和 443 端口处于开放状态。`docker compose logs caddy` 会显示证书颁发机构的应答。参见[域名和 TLS](domains-and-tls.md#证书)。

## 有人丢失了第二重验证因素

实例管理员可以在**设置** › **实例** › **用户**中，在该用户的侧滑面板里用**重置双重验证**重置它。如果没有能够登录的实例管理员，可以在服务器上重置：

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

它会删除该用户的验证器和恢复码，结束其会话，向其发送邮件，并记录到实例的审计日志中。在要求双重验证的地方，该用户会在下次登录时重新设置双重验证。

## 请求失败

错误以应用调用时所用的格式返回，并说明该怎么做。[错误](../api/errors.md)页面列出了所有错误。最常见的有：

| 状态 | 原因 | 处理方法 |
| --- | --- | --- |
| 401 | 密钥缺失、未知、已停用、已归档或已过期 | 检查应用发送的密钥，以及它在 **API 密钥**中的状态 |
| 403 | 密钥所属的成员或密钥本身不允许使用该模型 | 扩大该成员或密钥的模型访问范围 |
| 404 | 该模型不在项目的目录中 | 在**模型**中添加它，或者调用目录中已有的模型 |
| 400 | 该模型的服务商还没有设置服务商密钥 | 在**服务商**中添加一个，或者为项目指定一个 |
| 429 | 速率限制或预算，或者来自同一地址的未知密钥过多 | 消息会指明是哪个限额；等待 `Retry-After` 指定的时间，或者提高限额 |
| 502、504 | 无法连接服务商、服务商未及时响应，或其响应超过 `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | 查看服务商的状态；请求日志会显示其错误 |
| 503 | Valkey 停止运行导致无法检查硬预算，或者网关正在重启 | 检查 [`/readyz`](monitoring.md#健康检查) 和 `docker compose logs valkey` |

在 OpenAI 格式中，网关自身的部分 503 错误会以 500 返回。

## 流在一分钟后中断

应用与 Kaeso 之间的代理或负载均衡器关闭了空闲连接。流处于静默状态时，网关每 15 秒写入一条保活消息；如果途经的某个代理更早关闭连接，请在 [`compose.override.yaml`](installation.md#本地修改) 中调低 `KAESO_STREAMS_KEEPALIVE_INTERVAL`。如果使用自己的代理，请为 `/v1/` 关闭响应缓冲。参见[长时间的流](domains-and-tls.md#长时间的流)。

## 请求日志缺失或延迟

日志条目会在请求之后几秒出现。如果一直没有出现，可能是 ClickHouse 未接受写入：`/readyz` 报告 `degraded`，指标 `kaeso_log_buffer_rows` 持续增长。日志行在内存中等待，最多占用 `KAESO_LOGS_BUFFER_SIZE`，之后最旧的行会被丢弃。查看 `docker compose logs clickhouse`。

## 有人报告了请求 ID

当网关出现调用方无法自行解决的故障时，Platform API 会返回 500，控制台会提示 *网关出现问题；请将请求 req_… 报告给管理员。* 请求引起的每一行[日志](monitoring.md#日志)都带有其 ID，因此 `docker compose logs gateway | grep req_…` 可以找到带有原因的 `internal error` 行。

## Kaeso 无法连接数据库

当 PostgreSQL 停止运行或拒绝连接时，Platform API 会返回 503，控制台会提示 *Kaeso 暂时无法连接数据库，请稍后重试。*，[`/readyz`](monitoring.md#健康检查) 也会返回 `not_ready`，其中 `postgres` 为 `down`。请检查 `docker compose ps postgres` 和 `docker compose logs postgres`：常见原因是磁盘已满或密码已更改。数据库恢复后请求会立即恢复正常，无需重启任何服务。当 Valkey 或 ClickHouse 无法连接而请求又需要用到它们时，Platform API 同样返回 503；只有请求日志和用量会改为提示暂时不可用。

## 收不到邮件

在**设置** › **实例** › **设置** › **发送测试邮件**中发送一封测试邮件，并在网关的日志中查看 SMTP 服务器的应答。检查 `.env` 中的 `KAESO_SMTP_HOST`、端口、`KAESO_SMTP_TLS` 和凭据，以及 SMTP 服务器是否允许从 `KAESO_SMTP_FROM` 中的地址发信。如果 `KAESO_MAIL_TRANSPORT` 为 `log`，邮件只会被记录到日志。参见[电子邮件](email.md)。
