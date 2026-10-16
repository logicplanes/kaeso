---
title: 安装
description: 安装包含的文件、首次启动、本地修改和命令行。
order: 2
---

[快速开始](../get-started/quick-start.md)是本页的简短版本。

## 文件

一个安装就是一个只有自己能读取的文件夹，其中包含以下文件：

| 文件 | 内容 |
| --- | --- |
| `compose.yaml` | 来自 Kaeso 版本的服务定义。升级时用新版本的文件替换它。 |
| `.env` | 自己的设置：域名、数据库密码、电子邮件。Docker Compose 从同一个文件夹中读取它。 |
| `master-keys` | 主密钥，作为 Docker secret `master_keys` 交给网关。 |
| `compose.override.yaml` | 可选：对服务的自定义修改，替换 `compose.yaml` 时会保留。 |

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

每个版本还附带 [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example)（列出 `.env` 的每个变量及示例值）和 [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml)（列出网关的每项设置）；与上面的 `curl` 一样，这些链接获取的是最新稳定版本的文件。要安装特定版本，请改为从该版本的标签下载其文件，例如 `https://github.com/logicplanes/kaeso/releases/download/v0.1.0/compose.yaml`。

网关在容器内以非特权用户运行，并在容器内读取 `master-keys`，因此该文件本身是可读的；文件夹则对服务器上的其他用户保持封闭。将 `.env` 和 `master-keys` 与数据库备份分开备份；参见[备份和主密钥](backups.md)。

## .env 文件

`compose.yaml` 从 `.env` 中读取以下变量：

| 变量 | 必填 | 作用 |
| --- | --- | --- |
| `KAESO_DOMAIN` | 是 | Caddy 提供服务并为其获取证书的域名。公开 URL 为 `https://` 加上该域名。 |
| `POSTGRES_PASSWORD` | 是 | PostgreSQL 的 `kaeso` 用户的密码，网关以该用户登录。该用户拥有 `kaeso` 数据库，但不是超级用户。 |
| `POSTGRES_SUPERUSER_PASSWORD` | 是 | PostgreSQL 超级用户 `postgres` 的密码，供自己管理数据库时使用。Kaeso 从不使用它，`docker compose exec postgres psql -U postgres` 无需它即可登录。 |
| `CLICKHOUSE_PASSWORD` | 是 | ClickHouse 的 `kaeso` 用户的密码。 |
| `VALKEY_PASSWORD` | 是 | Valkey 要求每个连接提供的密码。 |
| `KAESO_PUBLIC_URL` | 否 | 公开 URL 不是 `https://` 加 `KAESO_DOMAIN` 时使用，例如位于自己的代理之后。 |
| `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | `KAESO_SMTP_HOST` 必填，除非邮件只记录到日志 | 网关发送邮件的方式。参见[电子邮件](email.md)。 |
| `KAESO_MAIL_TRANSPORT` | 否 | 设为 `log` 时只记录邮件而不发送，用于试用 Kaeso。 |

这些密码会写入连接 URL，因此只使用字母和数字，`openssl rand -hex` 生成的正是这样的密码。PostgreSQL 和 ClickHouse 的密码在数据库首次于其卷中创建时生效；之后在 `.env` 中修改密码，并不会改变数据库已有的密码。Valkey 每次启动时都从 `.env` 读取其密码。

Compose 只从 `.env` 中读取这些变量。其他所有[设置](configuration.md)都写入 `compose.override.yaml`，如[本地修改](#本地修改)所示。

## 首次启动时发生的事情

```bash
docker compose up -d
docker compose logs gateway | grep setup_url
```

网关处于 healthy 状态后，`docker compose up -d` 才会返回。找到的那一行中的 `setup_url` 就是设置链接。

1. PostgreSQL、ClickHouse 和 Valkey 启动；Compose 会等待 PostgreSQL 和 Valkey 报告为 healthy。
2. 网关检查其设置，如果某项设置缺失或错误，就会停止并给出指明该设置的消息。它从不输出机密值。
3. 网关在提供任何服务之前将 PostgreSQL 迁移到自己的版本，并在后台迁移 ClickHouse，不断重试直到 ClickHouse 响应；在此之前 `/readyz` 报告 `degraded`。迁移在锁的保护下运行，因此只有一个网关实例执行迁移，其他实例等待。
4. 安装中还没有用户时，网关会在日志中输出一个设置链接。该链接只能使用一次，有效期为 24 小时。每次重启以及运行 `docker compose exec gateway kaeso setup-link` 都会输出一个新链接，并使旧链接失效。
5. 网关和控制台都处于 healthy 状态后，Caddy 启动并为 `KAESO_DOMAIN` 获取证书。

设置页面会创建第一个用户，该用户成为实例管理员和第一个组织的所有者，组织自带一个默认项目，该用户在其中是管理员。此后，其他人通过邀请加入，或者在实例允许的情况下自行注册；参见[实例管理](../console/instance-administration.md)。

## 在自己的电脑上试用

设置 `KAESO_DOMAIN=localhost`，然后打开 `https://localhost`。Caddy 会使用其本地证书颁发机构签发的证书提供服务，浏览器会要求确认接受一次。如果手头没有邮件服务器，在 `.env` 中添加 `KAESO_MAIL_TRANSPORT=log`。

curl 和 SDK 不信任 Caddy 的本地证书颁发机构。将它的证书从容器中复制出来并交给它们：

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

Python SDK 从 `SSL_CERT_FILE=caddy-root.crt` 读取它，TypeScript SDK 从 `NODE_EXTRA_CA_CERTS=caddy-root.crt` 读取。

## 本地修改

要修改或添加某个服务的设置，将其写入 `compose.yaml` 旁边的 `compose.override.yaml`。Docker Compose 会合并这两个文件，替换 `compose.yaml` 的升级也会保留这些修改。

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

所有设置都列在[配置](configuration.md)中。用 `docker compose up -d` 应用修改；Compose 只会重新创建设置发生变化的服务。

## 命令行

网关镜像同时也是它的命令行工具。在运行中的容器里执行这些命令，它们会使用网关的设置：

| 命令 | 作用 |
| --- | --- |
| `kaeso setup-link` | 在安装中还没有用户时输出一个新的设置链接；旧链接随即失效 |
| `kaeso master-key generate` | 以 `id:base64` 的形式输出一个新的主密钥 |
| `kaeso master-key rewrap` | 用第一个主密钥重新加密已保存的机密，并报告哪些密钥 ID 仍在使用；参见[轮换主密钥](backups.md#轮换主密钥) |
| `kaeso user reset-2fa <email>` | 删除用户的双重验证和恢复码，结束其会话，并向其发送邮件 |
| `kaeso migrate` | 运行 PostgreSQL 和 ClickHouse 迁移，然后退出 |
| `kaeso serve` | 运行网关；这是镜像的默认命令 |

例如：

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```
