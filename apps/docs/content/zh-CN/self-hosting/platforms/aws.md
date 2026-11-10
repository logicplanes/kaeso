---
title: AWS
description: 在 AWS 上运行 Kaeso：一台运行 Compose 软件包的 EC2 服务器，或带 Application Load Balancer、RDS 和 ElastiCache 的 ECS Fargate。
order: 5
---

仓库中的 `deploy/aws/` 包含两个 CloudFormation 模板。快速服务器是在一台实例上运行 Compose 软件包，与您自己的服务器相同。托管堆栈是在 ECS Fargate 上运行 Kaeso 并使用 AWS 的数据存储，适合一台机器不够用的安装。

## 快速服务器

[在 AWS 上启动](https://console.aws.amazon.com/cloudformation/home#/stacks/create/review?templateURL=https%3A%2F%2Fkaeso-cloudformation.s3.amazonaws.com%2Flatest%2Fquick-server.yaml&stackName=kaeso) 会在 CloudFormation 中打开已发布的快速服务器模板。同一文件位于仓库的 `deploy/aws/quick-server.yaml`。每个版本都会把两个模板上传到 `https://kaeso-cloudformation.s3.amazonaws.com/vX.Y.Z/`，稳定版本还会上传到 `latest/`（[发布](../../development/releasing.md)）。

`deploy/aws/quick-server.yaml` 创建一台运行 Ubuntu 24.04 的 Graviton 实例、一个弹性 IP、一个允许 HTTP、HTTPS 和 HTTP/3 的安全组、来自您选定地址范围的 SSH，以及可选的 Route 53 记录。首次启动时，实例运行安装程序：安装 Docker、写入设置，并启动带 Caddy、内置 ClickHouse 和更新器的 Compose 软件包（[安装](../installation.md)）。

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/quick-server.yaml \
  --parameter-overrides Domain=kaeso.example.com KeyName=my-key AllowedSshCidr=203.0.113.0/24 HostedZoneId=Z0123456789
```

| 参数 | 含义 |
| --- | --- |
| `Domain` | Kaeso 应答的域名；未给出 `HostedZoneId` 时，把它的 A 记录指向输出 `PublicIp` |
| `KaesoVersion` | 要安装的版本；留空则安装最新的稳定版本 |
| `InstanceType`、`DiskSizeGb` | 默认 `t4g.medium` 和 40 GB（[系统要求](../requirements.md)） |
| `KeyName`、`AllowedSshCidr` | SSH 访问，`kaeso setup-link` 和手动升级需要它 |
| `HostedZoneId` | 域名所在的 Route 53 托管区，用于创建记录 |

堆栈完成且证书签发后，通过 SSH 运行输出 `SetupLink` 打印一次性设置链接。更新与任何 Compose 安装相同：在控制台中、自动或手动（[升级](../upgrades.md)）。

## 托管生产环境

`deploy/aws/managed.yaml` 创建：

- 一个 VPC，含两个公有子网、两个私有子网和一个 NAT 网关；
- 一个 Application Load Balancer，使用您的 ACM 证书，HTTP 重定向到 HTTPS，路径规则把 `/v1/*`、`/v1beta/*`、`/v2/*`、`/platform/*`、`/mcp`、`/mcp/*`、`/oauth/*` 和 `/.well-known/oauth-*` 发往网关、其余发往控制台，空闲超时为 300 秒以支持长时间的流；
- 一个 ECS 集群，含网关服务（默认 2 vCPU、4 GB，2 个任务）和控制台服务，运行在私有子网中的 Graviton Fargate 上；
- RDS PostgreSQL 18 和 ElastiCache Valkey（`noeviction`、TLS 和 AUTH 令牌），只有网关可访问；
- Secrets Manager 机密：主密钥、数据库密码、Valkey 令牌、SMTP 密码和 ClickHouse URL；
- CloudWatch 日志组，以及可选的 Route 53 别名。

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/managed.yaml --capabilities CAPABILITY_IAM \
  --parameter-overrides Domain=kaeso.example.com CertificateArn=arn:aws:acm:eu-central-1:123456789012:certificate/example \
  MasterKeys="$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  SetupEmail=owner@example.com SmtpHost=smtp.example.com SmtpUser=kaeso SmtpPassword=secret SmtpFrom="Kaeso <kaeso@example.com>"
```

| 参数 | 含义 |
| --- | --- |
| `Domain`、`CertificateArn` | 域名及其在堆栈所在区域的 ACM 证书；`HostedZoneId` 创建别名记录 |
| `MasterKeys` | 主密钥；堆栈把它保存在 Secrets Manager 中，您在密码管理器中保留一份副本 |
| `SetupEmail` | 在第一个用户创建之前，网关把一次性设置链接发送到的地址 |
| `ClickHouseUrl` | 带用户名和密码的 ClickHouse HTTP URL，例如 ClickHouse Cloud；留空则关闭请求日志 |
| `Smtp…` | 您的 SMTP 服务器（[电子邮件](../email.md)） |
| `GatewayCpu`、`GatewayMemory`、`GatewayCount` | 网关的规格和运行的任务数；它们通过 Valkey 共享限额、缓存和路由冷却 |
| `DatabaseInstanceClass`、`DatabaseStorageGb`、`CacheNodeType` | 数据存储的规格 |

网关只信任来自 VPC（负载均衡器节点所在）的 `X-Forwarded-For`，并把指标保留在回环地址上。负载均衡器对网关检查 `/readyz`，对控制台检查 `/healthz`；网关任务停止时有 75 秒时间处理完请求。

## 更新和回滚

发布新版本会修改 `managed.yaml` 中的镜像标签。用新模板更新堆栈；ECS 替换任务，并在失败时由断路器回滚，网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要主动回滚，部署上一版本的模板；如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 依据

两个模板都通过 `cfn-lint`，CI 会运行它。2026-11-07，已发布的快速服务器模板在空白 AWS 账户的 `us-east-1` 中创建了堆栈，并安装 第 8 个候选版本。首次启动暴露出所有权问题：模板在安装后更改了 `/opt/kaeso` 的所有权，导致网关无法读取主密钥文件。在测试实例上恢复所有权后，设置链接、HTTPS 健康与就绪检查、控制台和 API 路由、流式回复、ClickHouse 请求日志，以及替换伪造 `X-Forwarded-For` 均通过（[已验证的内容](index.md#已验证的内容)）。模板现在不再更改安装器拥有的文件，并有回归检查拒绝旧的 `chown`；修正后的模板尚未从空白账户重新部署。托管堆栈只通过了 lint；使用前请先在一次性账户中部署。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 RDS，并把主密钥保存在 AWS 之外。
- [健康状态和指标](../monitoring.md)：负载均衡器使用的健康检查。
