---
title: Railway
description: 用基础设施文件在 Railway 上部署 Kaeso，使用 Railway Postgres 和 Redis。
order: 2
---

仓库中的 `.railway/railway.ts` 以[基础设施即代码](https://docs.railway.com/infrastructure-as-code)描述一个 Railway 项目：一个公开边缘、一个私有网关和控制台、一个 Postgres 服务和一个 Redis 服务，全部固定为同一版本。Railway 旧的 `railway.json` 和 `railway.toml` 从 2026 年 12 月 1 日起不再生效，因此仓库中没有它们。

## 文件创建的内容

| 服务 | 镜像 | 可被谁访问 |
| --- | --- | --- |
| `kaeso-edge` | 控制台镜像，运行 `kaeso-edge` | 所有人，通过您分配的域名 |
| `kaeso-gateway` | 网关镜像 | 边缘服务，通过 Railway 私有网络 |
| `kaeso-console` | 控制台镜像 | 边缘服务，通过 Railway 私有网络 |
| `kaeso-postgres` | Railway Postgres | 网关 |
| `kaeso-valkey` | Railway Redis | 网关 |

Railway 的私有网络只有 IPv6，因此网关在 `[::]:8080` 上监听，并信任来自唯一本地地址范围的边缘服务。每个镜像都关闭了自动更新，服务因此停留在文件指定的版本上。

## 部署

1. 生成主密钥并保存到密码管理器：

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. 安装 Railway CLI，登录并创建一个空项目：

   ```bash
   railway login
   railway init
   ```

3. 在项目的 **Settings** 中的 **Shared Variables** 下添加以下变量：

   | 变量 | 填写内容 |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` 加边缘服务的域名；现在先填占位值，在第 6 步修正 |
   | `KAESO_MASTER_KEYS` | 第 1 步的密钥 |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；网关把设置链接发送到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器（[电子邮件](../email.md)） |
   | `KAESO_CLICKHOUSE_URL` | 用于请求日志的、带用户名和密码的 ClickHouse HTTP URL；留空则关闭请求日志 |

4. 从 `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts` 把该版本的 `.railway/railway.ts` 放入您自己的文件夹，并在该文件夹中应用：

   ```bash
   railway config apply
   ```

5. 为 `kaeso-edge` 分配域名：在其 **Settings** 下生成 Railway 域名或添加您自己的域名，端口为 8080。
6. 把 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名，并重新部署 `kaeso-gateway`。网关会把一次性设置链接发送到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。

## 更新和回滚

发布新版本会修改 `railway.ts` 中的镜像标签。以同样方式应用新文件；Railway 重新部署有变化的服务，网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，在服务的 **Deployments** 列表中重新部署较早的部署。

## 依据

该文件遵循 2026 年 11 月 6 日阅读的 Railway TypeScript 编写参考：`defineRailway`、`project`、`service`、`image`、`postgres`、`redis`、来自 `db.env` 和 `cache.env` 的服务环境、通过 `ctx.shared` 引用的共享变量，以及用于私有地址的 `${{service.RAILWAY_PRIVATE_DOMAIN}}`。Railway 把该功能标记为实验性，其 API 仍可能变化。三个镜像已按此布局在本地 Docker 中测试（[已验证的内容](index.md#已验证的内容)）。2026-11-07，已登录测试工作区的试用已过期；Railway 要求升级套餐后才能创建新项目并应用基础设施，因此无法从空项目部署。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：在 Railway 上备份 Postgres，并把主密钥保存在平台之外。
- [健康状态和指标](../monitoring.md)：边缘服务和网关应答的健康检查。
