---
title: Google Cloud
description: 在 Google Cloud 上运行 Kaeso：一台运行 Compose 软件包的 Compute Engine 服务器，或带负载均衡器、Cloud SQL 和 Memorystore 的 Cloud Run。
order: 7
---

仓库中的 `deploy/gcp/` 包含两套 Terraform 配置，OpenTofu 也能应用。快速服务器是在一台虚拟机上运行 Compose 软件包，与您自己的服务器相同。托管配置是在 Cloud Run 上运行 Kaeso 并使用 Google 的数据存储，适合一台机器不够用的安装。

## 快速服务器

`deploy/gcp/quick-server/` 创建一台运行 Ubuntu 24.04 的 Compute Engine 实例、一个静态地址、允许 HTTP、HTTPS 和 HTTP/3 的防火墙规则、来自您选定地址范围的 SSH，以及可选的 Cloud DNS 记录。首次启动时，实例运行安装程序：安装 Docker、写入设置，并启动带 Caddy、内置 ClickHouse 和更新器的 Compose 软件包（[安装](../installation.md)）。[Open in Cloud Shell](https://shell.cloud.google.com/cloudshell/editor?cloudshell_git_repo=https://github.com/logicplanes/kaeso&cloudshell_git_branch=prod&cloudshell_workspace=deploy/gcp/quick-server) 会把包含最新版本的 `prod` 分支克隆到 Cloud Shell，并以此文件夹作为工作区；下面的命令在那里用 `terraform` 代替 `tofu` 运行，因为 Cloud Shell 没有安装 `tofu`。

```bash
cd deploy/gcp/quick-server
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com -var 'ssh_source_ranges=["203.0.113.0/24"]'
```

| 变量 | 含义 |
| --- | --- |
| `project`、`region`、`zone` | 实例运行的位置；默认 `europe-west3-a` |
| `domain` | Kaeso 应答的域名；未给出 `dns_zone` 时，把它的 A 记录指向输出 `public_ip` |
| `kaeso_version` | 要安装的版本；留空则安装最新的稳定版本 |
| `machine_type`、`disk_size_gb` | 默认 `e2-medium` 和 40 GB（[系统要求](../requirements.md)） |
| `ssh_source_ranges` | 谁可以访问 SSH，`kaeso setup-link` 和手动升级需要它 |
| `dns_zone` | 域名所在的 Cloud DNS 托管区，用于创建记录 |

实例启动且证书签发后，运行输出 `setup_link` 打印一次性设置链接。更新与任何 Compose 安装相同：在控制台中、自动或手动（[升级](../upgrades.md)）。

## 托管生产环境

`deploy/gcp/managed/` 创建：

- 一个 VPC，含一个子网、一个无服务器 VPC 连接器和专用服务访问；
- 仅有私有地址的 Cloud SQL for PostgreSQL 17，以及 `noeviction` 的 Memorystore for Valkey，只能通过 VPC 访问；
- Secret Manager 机密：主密钥、PostgreSQL URL、SMTP 密码和 ClickHouse URL，由网关的服务账号读取；
- 网关（默认 2 CPU、4 GB，至少保持一个实例预热，请求超时一小时）和控制台的 Cloud Run 服务，只能通过负载均衡器访问；
- 一个全局外部 Application Load Balancer，带托管证书，HTTP 重定向到 HTTPS，路径规则把 `/v1/*`、`/v1beta/*`、`/v2/*`、`/platform/*`、`/mcp`、`/mcp/*`、`/oauth/*` 和 `/.well-known/oauth-*` 发往网关、其余发往控制台，后端超时一小时以支持长时间的流；
- 可选的 Cloud DNS 记录。

```bash
cd deploy/gcp/managed
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com \
  -var "master_keys=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  -var setup_email=owner@example.com -var smtp_host=smtp.example.com -var smtp_user=kaeso -var smtp_password=secret \
  -var 'smtp_from=Kaeso <kaeso@example.com>'
```

| 变量 | 含义 |
| --- | --- |
| `project`、`region`、`name` | 一切运行的位置和每个资源的名称前缀 |
| `domain`、`dns_zone` | 域名，以及用于创建记录的 Cloud DNS 托管区 |
| `master_keys` | 主密钥；配置把它保存在 Secret Manager 中，您在密码管理器中保留一份副本 |
| `setup_email` | 在第一个用户创建之前，网关把一次性设置链接发送到的地址 |
| `clickhouse_url` | 带用户名和密码的 ClickHouse HTTP URL，例如 ClickHouse Cloud；留空则关闭请求日志 |
| `smtp_…` | 您的 SMTP 服务器（[电子邮件](../email.md)） |
| `gateway_cpu`、`gateway_memory`、`gateway_min_instances`、`gateway_max_instances` | 网关的规格和运行的实例数；它们通过 Valkey 共享限额、缓存和路由冷却 |
| `database_tier`、`database_disk_gb`、`cache_node_type` | 数据存储的规格 |

Memorystore 只通过私有网络访问，不用密码也不用 TLS，因为网关无法出示 Google 的 IAM 令牌或其私有证书颁发机构。网关信任来自负载均衡器代理范围和 Cloud Run 自身前端的 `X-Forwarded-For`，并把指标保留在回环地址上。域名指向负载均衡器地址后，托管证书才会签发，最长需要一小时。

## 更新和回滚

发布新版本会修改 `main.tf` 中的镜像标签。应用新配置；Cloud Run 创建新修订版并在其健康后切换流量，网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要主动回滚，应用上一版本的配置，或在 Cloud Run 中把流量切回上一个修订版；如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 依据

两套配置都通过 Google 提供商的 `tofu fmt` 和 `tofu validate`，CI 会运行它们。2026-11-07，快速服务器配置已应用到一个位于 `europe-west3` 的新项目，并安装 第 8 个候选版本。设置链接、HTTPS 健康与就绪检查、控制台和 API 路由、流式回复、ClickHouse 请求日志，以及替换伪造 `X-Forwarded-For` 均通过（[已验证的内容](index.md#已验证的内容)）。托管配置只通过了验证；使用前请先在一次性项目中应用。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 Cloud SQL，并把主密钥保存在 Google Cloud 之外。
- [健康状态和指标](../monitoring.md)：Cloud Run 和负载均衡器使用的健康检查。
