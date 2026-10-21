---
title: 系统要求
description: Kaeso 实例所需的服务器、网络访问和磁盘空间。
order: 1
---

Kaeso 运行在一台装有 Docker 的 Linux 服务器上。请按需要记录的请求数量来规划磁盘。

## 服务器

| 资源 | 最低要求 |
| --- | --- |
| 系统 | Linux，Docker Engine 和 Docker Compose 2.24 或更高版本，`linux/amd64` 或 `linux/arm64` |
| CPU | 2 个 vCPU |
| 内存 | 4 GB，其中 ClickHouse 约占 2 GB |
| 磁盘 | 20 GB，另加请求日志所需空间 |

[更新器](upgrades.md#更新器)还需要 Docker Engine 25 或更高版本。

## 网络

实例需要一个 DNS 记录指向该服务器的域名，以及以下连接：

| 方向 | 端口或主机 | 用途 |
| --- | --- | --- |
| 入站 | TCP 端口 80 和 443，以及用于 HTTP/3 的 UDP 端口 443 | 证书和 HTTPS |
| 出站 | 您使用的每个服务商的主机（[支持的服务商](../concepts/supported-providers.md)），或您为其设置的基础 URL（[服务商设置](configuration.md#服务商设置)） | 您使用的服务商 |
| 出站 | `models.dev` | 模型数据，每天刷新一次 |
| 出站 | `update.kaeso.app` | 检查 Kaeso 新版本，每 5 分钟一次 |
| 出站 | `acme-v02.api.letsencrypt.org`、`acme.zerossl.com` | Caddy 获取和续期证书 |
| 出站 | `ghcr.io`、`pkg-containers.githubusercontent.com` | 安装或更新时，Docker 拉取 Kaeso 的镜像 |
| 出站 | `registry-1.docker.io`、`auth.docker.io`、`production.cloudflare.docker.com` | Docker 拉取 Caddy、PostgreSQL、ClickHouse、Valkey 和更新器套接字代理的镜像 |
| 出站 | `github.com`、`release-assets.githubusercontent.com` | 手动或由更新器下载版本的 `compose.yaml` 和 `release.json` |
| 出站 | `api.github.com` | 更新器和 `gh attestation verify` 获取版本的证明 |
| 出站 | 您的 SMTP 服务器 | 登录链接、邀请、提醒和安全通知（[邮件](email.md)） |
| 出站 | 您各组织的身份提供商 | 组织连接身份提供商后的单点登录（[登录](sign-in.md)） |

没有互联网访问时，Kaeso 使用版本内置的模型数据，并需要手动更新。请按[无法联网的实例](upgrades.md#无法联网的实例)中的说明关闭出站检查。

## 请求日志所需磁盘

请求日志保存在 ClickHouse 中。每一百万个请求请预留以下磁盘空间：

| 记录内容 | 每一百万个请求 |
| --- | --- |
| 仅元数据：模型、状态、Token、费用和耗时 | 约 0.1 GB |
| 元数据和内容，按每个请求约 2.5 KB 文本计 | 约 0.65 GB |

内容会随提示词和回复的长度增长。保留期决定总量上限。每天一百万个请求时，按默认保留期约占 6 GB 元数据（90 天）和 16 GB 内容（30 天）。

每个组织在其**日志记录**页面上选择保留期，但不能超过实例的上限（[请求日志](../console/request-logs.md)）。实例的**系统**页面显示日志实际占用的空间。
