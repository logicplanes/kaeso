---
title: 托管平台
description: 用仓库中的模板在 Render、Railway、DigitalOcean、AWS 或 Google Cloud 上运行 Kaeso。
order: 12
---

Compose 软件包在您自己管理的一台服务器上运行 Kaeso。仓库中的模板则把同一个版本放到托管平台上运行：由平台终结 TLS、保存数据库并推出新版本。

```mermaid
flowchart LR
  accTitle: 托管平台上的 Kaeso
  accDescr: 应用和浏览器通过平台的 TLS 到达一个公开的边缘服务。边缘服务把 API、MCP 服务器及其登录发往私有网关，把其他所有路径发往私有控制台。网关使用平台的 PostgreSQL 和 Valkey，在开启请求日志时使用外部 ClickHouse，并调用模型服务商。
  clients[应用和浏览器]:::outside
  subgraph platform [平台]
    edge[公开边缘<br>平台的 TLS]
    gateway[私有网关]:::accent
    console[私有控制台]
    postgres[(PostgreSQL)]
    valkey[(Valkey)]
  end
  clickhouse[(ClickHouse)]:::outside
  providers[模型服务商]:::outside
  clients -- HTTPS --> edge
  edge -- /v1、/platform、/mcp 和 OAuth --> gateway
  edge -- 其他所有路径 --> console
  gateway --> postgres & valkey & clickhouse
  gateway -- HTTPS --> providers
```

## 两类路径

| 路径 | 是什么 | 在哪里 |
| --- | --- | --- |
| 平台模板 | 平台读取的一个文件：公开边缘、私有网关和控制台，以及平台的 PostgreSQL 和 Valkey | [Render](render.md)、[Railway](railway.md)、[DigitalOcean](digitalocean.md) |
| 快速服务器 | 一台虚拟机，运行 Compose 软件包、Caddy、内置 ClickHouse 和更新器，与您自己的服务器相同 | [AWS](aws.md)、[Google Cloud](gcp.md) |
| 托管生产环境 | 平台的容器、负载均衡器、PostgreSQL 和 Valkey，机密保存在平台的密钥存储中 | [AWS](aws.md)、[Google Cloud](gcp.md) |

AWS 和 Google Cloud 不是一键部署：您需要选择区域、域名及其证书、数据存储的规格、请求日志的去向、SMTP 服务器，以及主密钥的存放位置。

## 每条路径都保持的内容

- **同一来源。** 控制台和 API 在同一个域名上应答。`/v1/*`、`/platform/*`、`/mcp`、`/mcp/*`、`/oauth/*` 和 `/.well-known/oauth-*` 到达网关，其他所有路径到达控制台。在 Render 和 Railway 上由控制台镜像的第二个命令 `kaeso-edge` 负责，在 DigitalOcean 上由应用的 ingress 规则负责，在 AWS 和 Google Cloud 上由负载均衡器的路径规则负责。
- **流不被缓冲。** 边缘服务立即转发每个数据块，负载均衡器的空闲超时长于网关的保活间隔（[长时间的流](../domains-and-tls.md#长时间的流)）。
- **只相信平台代理给出的客户端地址。** 边缘服务只接受来自平台私有网络的 `X-Forwarded-For`，并只向网关传递一个地址；网关只信任边缘服务（[`KAESO_TRUSTED_PROXIES`](../configuration.md#设置)）。
- **健康检查**用 `/healthz` 检查进程，用 `/readyz` 检查 PostgreSQL、Valkey 和 ClickHouse（[健康状态和指标](../monitoring.md)）。指标端口永不公开。
- **更新器关闭。** 由平台推出和回滚新版本；版本的迁移只向前进行（[升级](../upgrades.md)）。
- **同一版本。** 网关、控制台和边缘服务固定为同一版本；发布新版本时会更新模板。

## 每条路径都需要的内容

| 需要 | 方式 |
| --- | --- |
| 主密钥 | `docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate`，保存在平台的密钥存储和密码管理器中（[备份和主密钥](../backups.md)） |
| 电子邮件 | 一个 SMTP 服务器；`KAESO_MAIL_TRANSPORT=log` 只用于试用（[电子邮件](../email.md)） |
| 第一个用户 | `KAESO_SETUP_EMAIL`：在第一个用户创建之前，网关每次启动都会把一次性设置链接发送到该地址，因为网关镜像没有 shell 可运行 `kaeso setup-link`（[配置](../configuration.md#设置)） |
| 请求日志 | 在 `KAESO_CLICKHOUSE_URL` 中填入外部 ClickHouse，例如 ClickHouse Cloud；不填则请求日志关闭，用量、限额和预算照常工作（[选择请求日志的保存方式](../installation.md#选择请求日志的保存方式)） |
| 公开地址 | `KAESO_PUBLIC_URL`，用户和应用使用的 `https` 地址 |

## 已验证的内容

2026 年 11 月 6 日，边缘服务、网关和控制台按此布局用发布镜像在本地 Docker 中完成了测试：路径转发、分块的流、伪造的客户端地址，以及网关不可用时各 API 的错误格式（`e2e/install/test_edge.py`）。Render 蓝图、Railway 文件和 DigitalOcean 应用规范对照各平台的规范做了检查，CloudFormation 模板通过 `cfn-lint` 检查，Terraform 通过 `tofu validate` 检查；CI 会重复每项检查。

五条路径都尚未从一个空账户部署到对应平台，因此每个页面都会说明其依据。[Render](render.md) 和 [Google Cloud](gcp.md) 可通过指向 `prod` 分支（包含最新版本）的链接打开；Railway 需要先发布模板，AWS 需要先把模板放入 S3 存储桶，而 DigitalOcean 的按钮只部署一个服务，所以其规范通过 `doctl` 应用。

## 下一步

- [Render](render.md)：用 Render Postgres 和 Key Value 部署蓝图。
- [Railway](railway.md)：用 Railway Postgres 和 Redis 应用基础设施文件。
- [DigitalOcean](digitalocean.md)：用规范创建应用，使用开发用 PostgreSQL 和托管 Valkey。
- [AWS](aws.md)：启动一台 EC2 服务器，或使用带 Application Load Balancer、RDS 和 ElastiCache 的 ECS Fargate。
- [Google Cloud](gcp.md)：启动一台 Compute Engine 服务器，或使用带负载均衡器、Cloud SQL 和 Memorystore 的 Cloud Run。
