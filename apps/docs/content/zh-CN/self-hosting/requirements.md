---
title: 系统要求
description: 运行 Kaeso 的服务器需要具备的条件。
order: 1
---

## 服务器

- 装有 Docker Engine 和 Docker Compose 2.24 或更高版本的 Linux。镜像提供 `linux/amd64` 和 `linux/arm64` 两种架构。[一键更新](upgrades.md#一键更新)需要 Docker Engine 25 或更高版本，更新器使用其 API 版本 1.44。
- 至少 2 个 vCPU 和 4 GB 内存，其中 ClickHouse 约占 2 GB。
- 至少 20 GB 磁盘，另加请求日志所需的空间。

## 网络

- 一个 DNS 记录指向该服务器的域名。
- 端口 80 和 443 对互联网开放，用于申请证书和提供 HTTPS。UDP 端口 443 开放时，Caddy 也通过 HTTP/3 提供服务。
- 能以 HTTPS 访问所用的服务商（`api.openai.com`、`api.anthropic.com`）和 `models.dev`，网关每天从 `models.dev` 更新一次模型数据。无法访问 `models.dev` 的安装使用版本内置的模型数据；可在 [`compose.override.yaml`](installation.md#本地修改) 中设置 `KAESO_MODELS_DEV_SYNC=false` 关闭更新。
- 一台 SMTP 服务器，用于发送登录链接、邀请、预算提醒和安全通知。参见[电子邮件](email.md)。
- 能通过 HTTPS 访问 `api.github.com`（网关每天两次在此检查 Kaeso 新版本），以及为[一键更新](upgrades.md#一键更新)访问 `github.com`、`objects.githubusercontent.com` 和 `ghcr.io`。无法访问时请手动更新，并按[升级](upgrades.md#无法联网的安装)所述关闭检查。

## 请求日志所需磁盘

请求日志存放在 ClickHouse 中。以下数据测自 200,000 个合成请求，每个请求约含 2.5 KB 请求和回复文本，并在 ClickHouse 合并数据分片之后测量：

| 数据 | 每百万请求占用的磁盘 |
| --- | --- |
| 元数据：模型、状态、Token、费用、耗时 | 约 63 MB |
| 内容：请求和回复 | 约 545 MB，即每字节文本约 0.21 字节 |
| 用量汇总 | 最多 40 MB，通常远少于此 |

不记录内容时，每百万请求约需 0.1 GB；记录内容时约需 0.65 GB，内容部分按提示词和回复的实际大小换算。保留期限决定总量上限：每天一百万请求、按默认保留期限（元数据 90 天、内容 30 天）计算，约为 6 GB 元数据和 16 GB 内容。

每个组织在[日志记录](../console/request-logs.md#保留期)中设置自己的保留期限，不超过实例规定的最大值。组织的**日志记录**页面和实例的**系统**页面会显示实际占用。
