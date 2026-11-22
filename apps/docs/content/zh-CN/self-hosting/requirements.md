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
| 内存 | 4 GB。ClickHouse 占用最多，可增长到其上限 `CLICKHOUSE_MEMORY`（[容器限制](configuration.md#容器限制)） |
| 磁盘 | 20 GB，另加请求日志所需空间 |

[更新器](upgrades.md#更新器)还需要 Docker Engine 25 或更高版本。

## 内存和镜像

保存请求日志的 ClickHouse 占用的内存比其他所有数据库加起来还多。以下数据于 2026-10-31 用 Docker 29.5.2 测得，分别在运行[网关基准测试](../development/benchmark.md)之前和之后：

| 服务 | 空闲时内存 | 基准测试后内存 | 镜像 |
| --- | --- | --- | --- |
| ClickHouse | 193 MiB | 256 MiB | 981 MB |
| PostgreSQL | 75 MiB | 75 MiB | 672 MB |
| Valkey | 15 MiB | 15 MiB | — |
| 网关 | — | — | 117 至 141 MB |

ClickHouse 于 2026-11-16 重新测量，分别在空闲时以及运行基准测试的基线、智能体会话和 1,000 个流之后 5 分钟。在它自己的诊断表中，它只写入查询日志（保留一周）和错误日志（保留一个月）；Kaeso 不读取其中任何一个。此前同样的运行结果为 461 MiB 和 697 MiB。从早期版本升级的实例会保留已写入的诊断表，这些表不再增长。

[关闭请求日志](installation.md#选择请求日志的保存方式)后，实例不再运行 ClickHouse。这在空闲时节省约 0.2 GB 内存，在负载之后节省约 0.25 GB，并省去 981 MB 的镜像。无论哪种方式，用量、限额和预算的工作方式都相同。

## 网络

实例需要一个 DNS 记录指向该服务器的域名，以及以下连接：

| 方向 | 端口或主机 | 用途 |
| --- | --- | --- |
| 入站 | TCP 端口 80 和 443，以及用于 HTTP/3 的 UDP 端口 443 | 证书和 HTTPS |
| 出站 | 您使用的每个服务商的主机（[支持的服务商](../concepts/supported-providers.md)），或您为其设置的基础 URL（[服务商设置](configuration.md#服务商设置)） | 您使用的服务商 |
| 出站 | `oauth2.googleapis.com` | Gemini Enterprise Agent Platform 服务账号密钥的访问令牌 |
| 出站 | `dashscope-*.aliyuncs.com` | QwenCloud 以链接返回的图片、`qwen3-tts-flash` 音频和 Wan 视频（[Images API](../api/images.md#服务商)） |
| 出站 | 服务商存放已完成视频的主机（按其参考文档所示）：SpaceX 为 `vidgen.x.ai`，MiniMax 为 `video-product.cdn.minimax.io` 和阿里云 OSS 存储桶（例如 `public-cdn-video-data-algeng.oss-cn-wulanchabu.aliyuncs.com`），Z.ai 为 `aigc-files.bigmodel.cn`。ElevenLabs 没有固定主机；OpenRouter 和 Together AI 从自己的主机提供文件，Veo 和 Gemini Omni 在回复中直接返回文件 | 下载已完成的视频（[Videos API](../api/videos.md#下载视频)） |
| 出站 | `kpmr.kaeso.app` | 来自 [KPMR](../kpmr/index.md) 的模型数据，大约每小时检查一次 |
| 出站 | `update.kaeso.app` | 检查 Kaeso 新版本，每 5 分钟一次 |
| 出站 | `www.ecb.europa.eu` | 欧洲央行的[货币](../concepts/currencies.md)参考汇率，每 6 小时一次（[汇率](configuration.md#汇率)） |
| 出站 | `acme-v02.api.letsencrypt.org`、`acme.zerossl.com` | Caddy 获取和续期证书 |
| 出站 | `ghcr.io`、`pkg-containers.githubusercontent.com` | 安装或更新时，Docker 拉取 Kaeso 的镜像 |
| 出站 | `registry-1.docker.io`、`auth.docker.io`、`production.cloudflare.docker.com` | Docker 拉取 PostgreSQL、ClickHouse、Valkey 和更新器套接字代理的镜像 |
| 出站 | `github.com`、`release-assets.githubusercontent.com` | 手动或由更新器下载版本的 `compose.yaml` 和 `release.json` |
| 出站 | `api.github.com` | 更新器和 `gh attestation verify` 获取版本的证明 |
| 出站 | 您的 SMTP 服务器 | 登录链接、邀请、提醒和安全通知（[邮件](email.md)） |
| 出站 | 您各组织的身份提供商 | 组织连接身份提供商后的单点登录（[登录](sign-in.md)） |
| 出站 | 您各组织的自定义服务商的基础 URL | 发往它们的请求，直接连接，绝不经过代理（[自定义服务商](configuration.md#自定义服务商)） |
| 出站 | 您各组织添加的 MCP 服务器的地址，通过 HTTPS | 它们的工具，包括登录和元数据请求；只有 `mcp_servers.allowed_networks` 列出的私有网络才可访问（[MCP 服务器](configuration.md#mcp-服务器)） |
| 出站 | 您各组织添加的 Webhook 和防护规则的地址 | 投递和检查；只有 `webhooks.allowed_networks` 或 `guardrails.allowed_networks` 列出的私有网络才可访问（[Webhook](configuration.md#webhook)、[防护规则](configuration.md#防护规则)） |
| 出站 | 设置了 `otel.endpoint` 时，您的追踪收集器 | 请求和工具调用的追踪（[追踪](monitoring.md#追踪)） |

没有互联网访问时，Kaeso 使用版本内置的模型数据，并需要手动更新。请按[无法联网的实例](upgrades.md#无法联网的实例)中的说明关闭出站检查。

## 请求日志所需磁盘

请求日志保存在 ClickHouse 中，关闭后不占用磁盘。每一百万个请求请预留以下磁盘空间：

| 记录内容 | 每一百万个请求 |
| --- | --- |
| 仅元数据：模型、状态、Token、费用和耗时 | 约 0.1 GB |
| 元数据和内容，按每个请求约 2.5 KB 文本计 | 约 0.65 GB |

内容会随提示词和回复的长度增长。保留期决定总量上限。每天一百万个请求时，按默认保留期约占 9 GB 元数据（90 天）和 16 GB 内容（30 天）。

编程智能体每一轮都会重新发送整个对话，因此同一会话的请求正文几乎相同。对于 Anthropic Messages、OpenAI Chat Completions、OpenAI Responses 或 Gemini 格式的请求，Kaeso 在每个项目中只存储一次其中的每条长消息、系统提示词和工具，请求正文引用它们而不是重复保存，因此一次会话的内容随轮数线性增长，而不是随轮数的平方增长。在一次 80 轮的测试会话中，2.5 MB 的请求正文在磁盘上占用 42 KB，其中前 40 轮占用一半。

每个组织在其**日志记录**页面上选择保留期，但不能超过实例的上限（[请求日志](../console/request-logs.md)）。实例的**系统**页面显示日志实际占用的空间。
