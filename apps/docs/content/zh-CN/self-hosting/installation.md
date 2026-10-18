---
title: 安装
description: 安装包含的文件、首次启动、本地修改和命令行。
order: 2
---

[快速开始](../get-started/quick-start.md)会一步步安装 Kaeso；本页说明它设置了什么，以及如何修改。

## 文件

一个安装就是一个只有自己能访问的文件夹，其中包含以下文件：

| 文件 | 内容 |
| --- | --- |
| `compose.yaml` | 来自 Kaeso 版本的服务定义。升级时用新版本的文件替换它；可选的[更新器](upgrades.md#一键更新)只修改其中的镜像版本。 |
| `.env` | 自己的设置：域名、数据库密码、电子邮件。Docker Compose 从同一个文件夹中读取它。 |
| `master-keys` | 主密钥，作为 Docker secret `master_keys` 交给网关。 |
| `compose.override.yaml` | 可选：对服务的自定义修改，替换 `compose.yaml` 时会保留。 |

快速开始会创建前三个文件。网关在容器内以非特权用户运行，因此 `master-keys` 必须可读；文件夹的权限会阻止服务器上的其他用户访问。将 `.env` 和 `master-keys` 的副本与数据库备份分开保存；参见[备份和主密钥](backups.md)。

快速开始下载的是最新稳定版本的 `compose.yaml`。要安装特定版本，请改从该版本的标签下载，例如 `https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml`。每个版本还附带 [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example)，列出 `.env` 的每个变量及示例值，以及 [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml)，列出网关的每项设置。

## .env 文件

`compose.yaml` 从 `.env` 中读取以下变量：

| 变量 | 必填 | 作用 |
| --- | --- | --- |
| `KAESO_DOMAIN` | 是 | Caddy 提供服务并为其申请证书的域名。公开 URL 为 `https://` 加上该域名。 |
| `POSTGRES_PASSWORD` | 是 | PostgreSQL 的 `kaeso` 用户的密码，网关以该用户登录。该用户拥有 `kaeso` 数据库，但不是超级用户。 |
| `POSTGRES_SUPERUSER_PASSWORD` | 是 | PostgreSQL 超级用户 `postgres` 的密码，供自己管理数据库时使用。Kaeso 从不使用它，`docker compose exec postgres psql -U postgres` 无需它即可登录。 |
| `CLICKHOUSE_PASSWORD` | 是 | ClickHouse 的 `kaeso` 用户的密码。 |
| `VALKEY_PASSWORD` | 是 | Valkey 要求每个连接提供的密码。 |
| `KAESO_PUBLIC_URL` | 否 | 公开 URL 不是 `https://` 加 `KAESO_DOMAIN` 时使用，例如位于自己的代理之后。 |
| `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | `KAESO_SMTP_HOST` 必填，除非邮件只记录到日志 | 网关发送邮件的方式。参见[电子邮件](email.md)。 |
| `KAESO_MAIL_TRANSPORT` | 否 | 设为 `log` 时只记录邮件而不发送，用于试用 Kaeso。 |
| `KAESO_UPDATER_TOKEN`、`COMPOSE_PROFILES=updater` | 否 | 网关与更新器共享的密钥，以及启动更新器的配置文件，用于[一键更新](upgrades.md#一键更新)。 |
| `DOCKER_SOCKET` | 否 | Docker 套接字不在 `/var/run/docker.sock` 时，供更新器的代理使用的位置。 |

密码只使用字母和数字（`openssl rand -hex` 生成的正是如此），因为它们会写入连接 URL。PostgreSQL 和 ClickHouse 在首次创建各自的卷时采用密码；之后在 `.env` 中修改，并不会改变数据库已有的密码。Valkey 每次启动时都从 `.env` 读取密码。

Compose 只传递这些变量。其他所有[设置](configuration.md)都写入 `compose.override.yaml`，见[本地修改](#本地修改)。

## 首次启动时发生的事情

启动安装，然后输出设置链接：

```bash
docker compose up -d --wait
docker compose exec gateway kaeso setup-link
```

使用 `--wait` 时，Compose 会等到每个服务都报告健康后才返回，有服务未能健康时则失败。Caddy 一开始提供服务就算健康，此时新域名的证书可能仍在申请中；参见[证书](domains-and-tls.md#证书)。不加 `--wait`，Compose 在容器创建后立即返回；位于[自己的反向代理](domains-and-tls.md#自己的反向代理)之后时，没有 Caddy 等待网关，这时运行 `kaeso setup-link` 还为时过早。

1. PostgreSQL、ClickHouse 和 Valkey 启动。网关等待 PostgreSQL 和 Valkey 报告健康。
2. 网关检查其设置。缺少设置或设置无效时，它会停止，消息中会指明该设置，但从不显示其值。
3. 网关在提供任何服务之前迁移 PostgreSQL，并在后台迁移 ClickHouse，不断重试直到 ClickHouse 响应；在此之前，`/readyz` 报告 `degraded`。迁移在锁的保护下运行，因此只有一个网关实例执行迁移，其他实例等待。
4. 安装还没有用户时，网关会在日志中说明尚未设置以及如何获取设置链接，但从不记录链接本身：任何能读取日志的人都可能借此接管安装。`kaeso setup-link` 在终端中输出链接。链接只能使用一次，有效期 24 小时，每输出一个新链接，旧链接即失效。
5. 网关和控制台健康后，Caddy 启动，一开始提供服务就报告健康，并在后台为 `KAESO_DOMAIN` 申请证书。

设置链接会创建第一个用户，该用户成为实例管理员、第一个组织的所有者以及其默认项目的管理员。其他人通过邀请加入，或在实例允许时自行注册；参见[实例管理](../console/instance-administration.md)。只通过单点登录登录的安装，请按[没有密码的安装](sign-in.md#没有密码的安装)中的说明设置。

## 在自己的电脑上试用

设置 `KAESO_DOMAIN=localhost`，然后打开 `https://localhost`。Caddy 使用其本地证书颁发机构签发的证书提供服务，浏览器会要求确认一次。手头没有邮件服务器时，在 `.env` 中加入 `KAESO_MAIL_TRANSPORT=log`。

curl 和各 SDK 不信任 Caddy 的本地证书颁发机构。从容器中复制其根证书并交给它们：

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

Python SDK 从 `SSL_CERT_FILE=caddy-root.crt` 读取它，TypeScript SDK 从 `NODE_EXTRA_CA_CERTS=caddy-root.crt` 读取。

## 本地修改

对服务的自定义修改写在 `compose.yaml` 旁边的 `compose.override.yaml` 中。Docker Compose 会合并这两个文件，替换 `compose.yaml` 的升级也会保留这些修改：

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

[配置](configuration.md)列出了每项设置。使用 `docker compose up -d --wait` 应用修改；Compose 只会重新创建设置有变化的服务。

## 命令行

网关镜像同时也是 Kaeso 的命令行。在运行中的容器里执行命令，它们会使用网关的设置：

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| 命令 | 作用 |
| --- | --- |
| `kaeso setup-link` | 在安装还没有用户时输出新的设置链接；旧链接随之失效 |
| `kaeso master-key generate` | 以 `id:base64` 格式输出一个新的主密钥 |
| `kaeso master-key rewrap` | 用第一个主密钥重新加密存储的机密，并报告哪些密钥 ID 仍在使用；参见[轮换主密钥](backups.md#轮换主密钥) |
| `kaeso user reset-2fa <email>` | 移除用户的双重验证和恢复码，结束其会话，发邮件通知本人，并记入实例审计日志 |
| `kaeso user sign-in-link <email>` | 为一个有效用户输出一次性登录链接，15 分钟内有效，用于其他方式都无法登录时；参见[重新登录](sign-in.md#重新登录) |
| `kaeso migrate` | 运行 PostgreSQL 和 ClickHouse 迁移，然后退出 |
| `kaeso serve` | 运行网关；镜像的默认命令 |
