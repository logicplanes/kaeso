---
title: 自托管
description: 运行、配置和维护 Kaeso 安装。
order: 2
---

一个 Kaeso 安装是运行在同一个域名上的一组容器。每个 Kaeso 版本附带的 Compose 文件会启动所有这些容器；[快速开始](../get-started/quick-start.md)是得到一个运行中安装的最快途径。

| 服务 | 镜像 | 作用 | 可从何处访问 |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | 为域名终止 TLS（证书由它自动获取和续期），并路由请求 | 互联网，80 和 443 端口 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | 提供模型 API（`/v1`）和 Platform API（`/platform`） | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | 提供 Web 界面（其他所有路径） | Caddy |
| `postgres` | `postgres:18.6-alpine` | PostgreSQL：用户、组织、密钥、加密的服务商密钥、目录、限额、用量、审计日志 | 仅网关 |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | ClickHouse：请求日志和用量汇总 | 仅网关 |
| `valkey` | `valkey/valkey:9.1.2-alpine` | Valkey：速率和预算计数器以及共享缓存 | 仅网关 |

网关和控制台镜像的版本与所用 `compose.yaml` 所属的版本一致。数据库位于外部无法访问的内部网络中，其数据保存在命名的 Docker 卷中（`postgres`、`clickhouse`、`valkey`，以及存放证书的 `caddy_data`）。网关的指标监听端口 9100 不经 Caddy 路由。所有容器都不能获得额外权限，网关和控制台不带任何 Linux capabilities 运行，网关的文件系统为只读；Caddy 只保留绑定 80 和 443 端口的权限，并发送 `Strict-Transport-Security`。网关以 `kaeso` 身份登录 PostgreSQL，该用户拥有 Kaeso 数据库，但不是超级用户；ClickHouse 的 `kaeso` 用户不能管理用户和权限；Valkey 要求每个连接提供其密码。第三方镜像按版本和摘要固定。

## 多个网关

网关实例自身不保存任何状态。要运行多个实例，将它们放在同一个代理后面，并指向同一个 PostgreSQL、ClickHouse 和 Valkey：缓存通过 Valkey 在各实例之间失效，后台任务在 PostgreSQL 锁的保护下每次只在一个实例上运行，每个实例各自缓冲并写入自己的请求日志行和用量。

## 页面

| 页面 | 内容 |
| --- | --- |
| [系统要求](requirements.md) | 服务器规格、请求日志所需磁盘、网络 |
| [安装](installation.md) | 安装包含的文件、首次启动时发生的事情、本地修改、命令行 |
| [配置](configuration.md) | 设置的读取方式、每个 `KAESO_*` 设置，以及 API 密钥限流 |
| [域名和 TLS](domains-and-tls.md) | 域名、证书、自己的反向代理、长时间的流 |
| [电子邮件](email.md) | SMTP、Kaeso 发送的邮件、语言、测试 |
| [升级](upgrades.md) | 迁移到新版本 |
| [备份和主密钥](backups.md) | 需要备份的内容、恢复、轮换主密钥 |
| [健康状态和指标](monitoring.md) | 健康检查、Prometheus 指标和日志 |
| [故障排除](troubleshooting.md) | 常见问题及其解决方法 |
