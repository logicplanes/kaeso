---
title: 自托管
description: 运行、配置和维护 Kaeso 安装。
order: 2
---

一个 Kaeso 安装由一个域名后面的六个容器组成，由每个版本附带的 `compose.yaml` 启动。[快速开始](../get-started/quick-start.md)让安装跑起来，后续的一切都在这些页面中。

| 服务 | 镜像 | 职责 | 可访问来源 |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | 用自行申请和续期的证书终止 TLS，并转发请求 | 互联网，端口 80 和 443 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | 提供模型 API（`/v1`）和 Platform API（`/platform`） | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | 提供 Web 界面（其他所有路径） | Caddy |
| `postgres` | `postgres:18.6-alpine` | 用户、组织、密钥、加密的服务商密钥、目录、限额、用量、审计日志 | 仅网关 |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | 请求日志和用量汇总 | 仅网关 |
| `valkey` | `valkey/valkey:9.1.2-alpine` | 速率和预算计数器、共享缓存 | 仅网关 |

网关和控制台镜像带有所用 `compose.yaml` 对应版本的版本号；第三方镜像以版本和摘要固定。

## 隔离

- 数据库位于没有对外路由的内部网络中。它们的数据存放在命名卷 `postgres`、`clickhouse` 和 `valkey` 中；Caddy 把证书存放在 `caddy_data` 和 `caddy_config` 中。
- 任何容器都无法提升权限。网关和控制台不带 Linux capabilities 运行，网关的文件系统为只读。Caddy 只保留绑定端口 80 和 443 的权限，并发送 `Strict-Transport-Security`。
- 网关以 `kaeso` 身份登录 PostgreSQL，该用户拥有 Kaeso 数据库，但不是超级用户。ClickHouse 的 `kaeso` 用户无法管理用户和授权。Valkey 要求每个连接提供密码。
- Caddy 不转发网关的指标端口 9100。

## 多个网关

网关实例自身不保存状态。要运行多个实例，把它们放在同一个代理后面，并指向同一套 PostgreSQL、ClickHouse 和 Valkey。缓存通过 Valkey 在实例之间失效，后台任务在 PostgreSQL 锁的保护下每次只在一个实例上运行，每个实例各自缓冲并写入自己的请求日志行和用量。

## 页面

| 页面 | 内容 |
| --- | --- |
| [系统要求](requirements.md) | 服务器规格、网络、请求日志所需磁盘 |
| [安装](installation.md) | 安装包含的文件、首次启动、本地修改、命令行 |
| [配置](configuration.md) | 设置的读取方式、每一项 `KAESO_*` 设置、API 密钥限流 |
| [域名和 TLS](domains-and-tls.md) | 域名、证书、自己的反向代理、长时间的流 |
| [电子邮件](email.md) | SMTP、Kaeso 发送什么邮件以及使用哪种语言、测试 |
| [登录和单点登录](sign-in.md) | 登录方式、只用单点登录的安装、重新登录 |
| [升级](upgrades.md) | 迁移到新版本 |
| [备份和主密钥](backups.md) | 备份什么、恢复、轮换主密钥 |
| [健康状态和指标](monitoring.md) | 健康检查、Prometheus 指标、日志 |
| [故障排除](troubleshooting.md) | 常见问题及解决方法 |
