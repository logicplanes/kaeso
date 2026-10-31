---
title: 安装
description: 安装指定版本、在自己的电脑上试用 Kaeso、修改其服务并运行其命令。
order: 3
---

[快速开始](../get-started/quick-start.md)会在服务器上安装最新版本。本页介绍可以换一种做法的地方，以及在实例上运行的命令。

## 安装指定版本

快速开始下载的是最新稳定版本的 `compose.yaml`。要安装其他版本，从该版本的标签下载此文件：

```bash
curl -fsSLO \
  https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml
```

每个版本还附带两个示例文件：[`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) 包含 `.env` 的常用变量，[`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) 包含网关的每项设置。

## 在自己的电脑上试用

在 `.env` 中设置 `KAESO_DOMAIN=localhost`，启动 Kaeso 后打开 `https://localhost`。Caddy 用自己的本地证书颁发机构签发证书，浏览器会请您确认一次。没有邮件服务器时，再设置 `KAESO_MAIL_TRANSPORT=log`（[邮件](email.md#不用邮件服务器试用-kaeso)）。

curl 和 SDK 不信任 Caddy 的本地证书颁发机构。从容器中复制出它的根证书：

```bash
docker compose cp \
  caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
```

然后把证书交给调用 Kaeso 的客户端：

```bash tab="curl"
curl --cacert caddy-root.crt https://localhost/v1/models \
  -H "Authorization: Bearer $KAESO_API_KEY"
```

```bash tab="Python"
SSL_CERT_FILE=caddy-root.crt python app.py
```

```bash tab="TypeScript"
NODE_EXTRA_CA_CERTS=caddy-root.crt node app.mjs
```

## 修改服务

把您对服务的修改写进 `compose.yaml` 旁边的 `compose.override.yaml`。Compose 会合并这两个文件，而升级只替换 `compose.yaml`，因此您的修改会保留。例如，让网关记录更详细的日志：

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
```

用 `docker compose up -d --wait` 应用修改。Compose 只会重建设置有变化的服务。[配置](configuration.md)列出了网关的每项设置。

## 选择请求日志的保存方式

请求日志保存在 ClickHouse 中。`compose.yaml` 自带的 ClickHouse 是推荐的方式：它不需要您做任何事，并把日志保存在同一台服务器上。用量、限额、预算和预算提醒都不依赖它，因为 Kaeso 把用量保存在 PostgreSQL 中。ClickHouse 是所有服务中占用内存最多的（[系统要求](requirements.md#内存和镜像)），因此实例也可以使用您已有的 ClickHouse，或者完全不保存请求日志。

| 请求日志 | `.env` 中的 `KAESO_CLICKHOUSE_URL` | 自带的 `clickhouse` 服务 |
| --- | --- | --- |
| 保存在自带的 ClickHouse 中（默认） | 不设置 | 运行 |
| 保存在您自己的 ClickHouse 中 | 您的 ClickHouse 的 URL | 关闭 |
| 关闭 | 设置为空 | 关闭 |

网关在启动时读取该设置。要恢复默认方式，删除下面的两处修改，然后运行 `docker compose up -d --wait`。

### 关闭请求日志

1. 在 `.env` 中添加值为空的 `KAESO_CLICKHOUSE_URL=`。请保留其中的 `CLICKHOUSE_PASSWORD`：即使自带的 ClickHouse 已关闭，Compose 仍会检查它。
2. 把自带的 ClickHouse 放进一个您从不启用的配置文件（profile），让 Compose 不再启动它：

   ```yaml title="compose.override.yaml"
   services:
     clickhouse:
       profiles: [logs]
   ```

3. 用新设置启动网关，然后删除仍在运行的 ClickHouse 容器。它的卷 `kaeso_clickhouse` 会保留之前的日志，直到您删除它。

   ```bash
   docker compose up -d --wait
   docker compose rm -sf clickhouse
   ```

之后控制台不再显示**请求日志**，组织的**日志记录**页面会说明请求日志已关闭。Platform API 对日志端点返回 `404`，`/readyz` 把 `clickhouse` 报告为 `off`。日志关闭期间处理的请求永远不会被记录，即使之后重新开启日志也是如此。

### 使用您自己的 ClickHouse

网关只需要 ClickHouse 的 HTTP 接口（明文或 TLS），以及一个可以创建数据库及其表的用户。把带有该用户和密码的 URL 写进 `.env`。其中除字母和数字以外的字符都要进行百分号编码：

```bash
KAESO_CLICKHOUSE_URL=https://kaeso:secret@clickhouse.example.com:8443
```

然后用与[上面](#关闭请求日志)相同的 `compose.override.yaml` 和命令关闭自带的 ClickHouse。网关会创建数据库 `KAESO_CLICKHOUSE_DATABASE`（默认为 `kaeso`），并在其中运行迁移。请像备份您的其他数据一样备份这个 ClickHouse。

## 运行 Kaeso 的命令

网关镜像同时也是 Kaeso 的命令行。在运行中的网关容器里执行命令，命令会使用网关的设置：

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| 命令 | 作用 |
| --- | --- |
| `kaeso setup-link` | 在实例还没有用户时打印设置链接。链接仅可使用一次，24 小时内有效，新链接会让旧链接失效 |
| `kaeso user sign-in-link <email>` | 为有效用户打印一次性登录链接，15 分钟内有效（[重新登录](sign-in.md#重新登录)） |
| `kaeso user reset-2fa <email>` | 移除用户的双重验证和恢复码，结束其会话并发邮件通知。实例审计日志会记录此操作 |
| `kaeso sso verify-domain <domain>` | 无需 TXT 记录即可将单点登录的邮箱域名标记为已验证，适用于公共 DNS 无法解析的域名。有多个组织列出该域名时，用 `--org <org>` 指定组织。组织的审计日志会记录此操作 |
| `kaeso instance admin-sso <org\|none>` | 按 `org_…` 这样的 ID 选择其单点登录可管理实例的组织，或用 `none` 不选，适用于没有实例管理员能在控制台中更改它的情况（见[实例管理员](sign-in.md#实例管理员)）。该组织需要已连接单点登录。实例审计日志会记录此操作 |
| `kaeso master-key generate` | 以 `id:base64` 格式打印一个新主密钥 |
| `kaeso master-key rewrap` | 用第一个主密钥重新加密已保存的机密，并报告哪些密钥 ID 仍在使用（[轮换主密钥](backups.md#轮换主密钥)） |
| `kaeso migrate` | 运行 PostgreSQL 迁移，请求日志开启时也运行 ClickHouse 迁移，然后退出 |
| `kaeso serve` | 运行网关。这是镜像的默认命令 |

## 下一步

- [配置](configuration.md)：查找每项设置及其所在位置。
- [域名和 TLS](domains-and-tls.md)：获取证书，或在自己的代理后面运行 Kaeso。
- [备份和主密钥](backups.md)：备份数据库并妥善保管主密钥。
