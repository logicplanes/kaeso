---
title: 系统要求
description: 服务器运行 Kaeso 所需的条件。
order: 1
---

## 服务器

- 装有 Docker Engine 和 Docker Compose 2.24 或更高版本的 Linux。
- 至少 2 个 vCPU 和 4 GB 内存，其中 ClickHouse 约占 2 GB。
- 至少 20 GB 磁盘，另加请求日志所需的空间。

镜像发布了 `linux/amd64` 和 `linux/arm64` 两种平台版本。

## 网络

- 一个 DNS 记录指向该服务器的域名。
- 向互联网开放 80 和 443 端口，用于获取证书和提供 HTTPS。UDP 443 端口开放时，Caddy 还会通过它提供 HTTP/3。
- 可以通过 HTTPS 访问所用的服务商（`api.openai.com`、`api.anthropic.com`）以及 `models.dev`，网关每天从这里刷新一次模型数据。无法访问 `models.dev` 的安装会使用版本内置的模型数据；在 [`compose.override.yaml`](installation.md#本地修改) 中用 `KAESO_MODELS_DEV_SYNC=false` 关闭刷新。
- 一个 SMTP 服务器，用于发送登录链接、邀请、预算提醒和安全通知。参见[电子邮件](email.md)。

## 请求日志所需磁盘

请求日志保存在 ClickHouse 中。使用网关的表结构和合成流量测量，共 200,000 个请求，每个请求约含 2.5 KB 的请求和回复文本，在 ClickHouse 合并其数据分片之后：

| 数据 | 每百万个请求占用的磁盘 |
| --- | --- |
| 元数据：模型、状态、Token、费用、耗时 | 约 63 MB |
| 内容：请求和回复，每个请求 2.5 KB 文本 | 约 545 MB，即每字节文本约 0.21 字节 |
| 用量汇总 | 最多 40 MB，通常远少于此 |

不记录内容时，按每百万个请求约 0.1 GB 规划；记录内容时约 0.65 GB，内容部分随提示和回复的大小按比例增减。保留期限决定总量的上限：以每天一百万个请求、元数据默认保留 90 天、内容默认保留 30 天计算，约为 6 GB 元数据和 16 GB 内容。

保留期限按组织在[日志记录](../console/request-logs.md#保留期)中设置，不能超过实例允许的最大值。组织的**日志记录**设置和实例的**系统**页面会显示实际数值。
