---
title: Railway
description: 用 Deploy on Railway 按钮或基础设施文件在 Railway 上部署 Kaeso，使用 Railway Postgres 和 Redis。
order: 2
---

**Deploy on Railway** 按钮部署 Kaeso 的 [Railway 模板](https://railway.com/deploy/cmVV60)。仓库中的 `.railway/railway.ts` 以[基础设施即代码](https://docs.railway.com/infrastructure-as-code)描述同一个项目，供 Railway CLI 使用。两者都运行一个公开边缘、一个私有网关和控制台、一个 Postgres 服务和一个 Redis 服务，全部固定为同一版本。Railway 旧的 `railway.json` 和 `railway.toml` 从 2026 年 12 月 1 日起不再生效，因此仓库中没有它们。

## Railway 运行的内容

| 服务 | 镜像 | 可被谁访问 |
| --- | --- | --- |
| `kaeso-edge` | 控制台镜像，运行 `kaeso-edge` | 所有人，通过它的 Railway 域名或您自己的域名 |
| `kaeso-gateway` | 网关镜像 | 边缘服务，通过 Railway 私有网络 |
| `kaeso-console` | 控制台镜像 | 边缘服务，通过 Railway 私有网络 |
| `kaeso-postgres` | Railway Postgres | 网关 |
| `kaeso-valkey` | Railway Redis | 网关 |

Railway 的私有网络只有 IPv6，因此网关在 `[::]:8080` 上监听，并信任来自唯一本地地址范围的边缘服务。Railway 的健康检查访问 `PORT` 中的端口，因此每个服务都把它设为自己监听的端口。每个镜像都关闭了自动更新，服务因此停留在部署时的版本上。

## 用按钮部署

1. 打开 [Deploy on Railway](https://railway.com/new/template/cmVV60?utm_medium=integration&utm_source=button&utm_campaign=kaeso) 并登录 Railway。
2. 在 `kaeso-gateway` 下填写以下变量：

   | 变量 | 填写内容 |
   | --- | --- |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；网关把设置链接发送到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器和 Kaeso 邮件的发件人（[电子邮件](../email.md)）；没有 SMTP 服务器，网关不会启动 |
   | `KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD` | 服务器的用户名和密码，如果需要 |
   | `KAESO_SMTP_PORT` | `587`，除非您的服务器使用其他端口 |
   | `KAESO_CLICKHOUSE_URL` | 用于请求日志的、带用户名和密码的 ClickHouse HTTP URL；留空则关闭请求日志 |

   其余由模板填写：`KAESO_MASTER_KEYS` 填入新的主密钥，由 Railway 的 `secret()` 函数生成的 32 个随机字节；`KAESO_PUBLIC_URL` 填入边缘服务的 Railway 域名。

3. 部署，并等待五个服务全部在线。网关会把一次性设置链接发送到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。
4. 把 `kaeso-gateway` 变量中的 `KAESO_MASTER_KEYS` 复制到密码管理器。Kaeso 用它加密提供商密钥，没有它就无法读取数据库备份（[备份和主密钥](../backups.md)）。
5. 要使用您自己的域名，在 `kaeso-edge` 的 **Settings** 下添加它，把 `kaeso-gateway` 的 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名，并重新部署网关。

> [!NOTE]
> 网关镜像没有 shell，因此 `railway ssh` 无法在其中运行 `kaeso setup-link`。如果没有收到设置邮件，网关的日志会写明 SMTP 错误。修正 SMTP 变量并重新部署 `kaeso-gateway`：在还没有用户时，它每次启动都会发送新的链接。

## 用 CLI 部署

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
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器（[电子邮件](../email.md)）；没有它，网关不会启动 |
   | `KAESO_CLICKHOUSE_URL` | 用于请求日志的、带用户名和密码的 ClickHouse HTTP URL；留空则关闭请求日志 |

4. 从 `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts` 把该版本的 `.railway/railway.ts` 放入您自己的文件夹，在那里安装 Railway 的 TypeScript 包并应用该文件：

   ```bash
   npm install railway
   railway config apply
   ```

5. 为 `kaeso-edge` 分配域名：在其 **Settings** 下生成 Railway 域名或添加您自己的域名，端口为 8080。
6. 把 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名，并重新部署 `kaeso-gateway`。网关会把一次性设置链接发送到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。

## 更新和回滚

发布新版本会修改镜像标签。对于用按钮创建的项目，在 `kaeso-gateway`、`kaeso-console` 和 `kaeso-edge` 各自的 **Settings** 中修改镜像标签，先改网关。对于用文件创建的项目，以之前的方式应用新版本的 `railway.ts`；Railway 重新部署有变化的服务。两种情况下，网关都会在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，在服务的 **Deployments** 列表中重新部署较早的部署。

## 依据

该文件遵循 2026 年 11 月 6 日阅读的 Railway TypeScript 编写参考：`defineRailway`、`project`、`service`、`image`、`postgres`、`redis`、来自 `db.env` 和 `cache.env` 的服务环境、通过 `ctx.shared` 引用的共享变量，以及用于私有地址的 `${{service.RAILWAY_PRIVATE_DOMAIN}}`。Railway 把该功能标记为实验性，其 API 仍可能变化。

2026 年 11 月 9 日，该文件被应用到一个空的 Railway 项目，模板被部署到另一个项目，两者都使用第八个候选版本。两次部署中，五个服务都已在线，边缘服务在 Railway 域名上提供控制台和 API，第一个用户完成设置并登录。第一次运行发现，没有 `PORT` 时网关和控制台无法通过 Railway 的健康检查；文件和模板现在都设置了它。测试使用的是占位 SMTP 服务器，因此设置链接来自通过 SSH 隧道运行的 `kaeso setup-link`，而不是邮件。邮件投递、流式回复和更新尚未在 Railway 上测试。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：在 Railway 上备份 Postgres，并把主密钥保存在平台之外。
- [健康状态和指标](../monitoring.md)：边缘服务和网关应答的健康检查。
