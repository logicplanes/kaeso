---
title: Render
description: 用蓝图在 Render 上部署 Kaeso，使用 Render Postgres 和 Key Value。
order: 1
---

仓库根目录的 `render.yaml` 是一个 [Render 蓝图](https://render.com/docs/infrastructure-as-code)。它创建一个公开边缘、一个私有网关和控制台、一个 Postgres 实例和一个 Key Value 实例，全部固定为同一版本。

## 蓝图创建的内容

| 服务 | 类型 | 镜像 | 套餐 |
| --- | --- | --- | --- |
| `kaeso-edge` | Web Service | 控制台镜像，运行 `kaeso-edge` | 0.5 CPU，512 MB |
| `kaeso-gateway` | Private Service | 网关镜像 | 2 CPU，4 GB |
| `kaeso-console` | Private Service | 控制台镜像 | 0.5 CPU，512 MB |
| `kaeso-valkey` | Key Value | Render 提供，`noeviction` | 256 MB |
| `kaeso-postgres` | Postgres | Render 提供，版本 18 | 1 CPU，2 GB |

只有边缘服务是公开的。Render 的私有网络承载从边缘到网关和控制台、从网关到 Postgres 和 Key Value 的请求，后两者的 `ipAllowList` 为空。每个服务都设置了 `autoDeployTrigger: off`，因此 Render 不会因提交而重新部署。在创建蓝图之前修改套餐或区域；之后 Render 会保留它们。

## 部署

1. 生成主密钥并保存到密码管理器：

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. 在 Render 中从仓库 `logicplanes/kaeso` 的 `prod` 分支创建 **Blueprint**，该分支包含最新版本；[Deploy to Render](https://render.com/deploy?repo=https://github.com/logicplanes/kaeso/tree/prod) 会打开已填好仓库和分支的该表单。Render 会显示蓝图需要的值：

   | 值 | 填写内容 |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` 加边缘服务的地址，例如 `https://kaeso-edge.onrender.com`，或设置好后的您自己的域名 |
   | `KAESO_MASTER_KEYS` | 第 1 步的密钥 |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；网关把设置链接发送到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器（[电子邮件](../email.md)） |
   | `KAESO_CLICKHOUSE_URL` | 用于请求日志的、带用户名和密码的 ClickHouse HTTP URL；留空则关闭请求日志 |

3. 等待所有服务上线。网关会把一次性设置链接发送到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。
4. 要使用自己的域名，在 `kaeso-edge` 的 **Settings** 中添加域名，按 Render 的提示设置 DNS 记录，把 `kaeso-gateway` 的 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名，然后重新部署网关。

网关在端口 8080 上监听，并把指标保留在回环地址上，因为 Render 把私有服务在所有接口上绑定的那一个端口当作其端口。公开流量在到达 Render 前经过 Cloudflare。边缘服务读取由 Cloudflare 覆盖的 `CF-Connecting-IP`，不使用调用方可控制的 `X-Forwarded-For`；网关只信任边缘服务。

## 更新和回滚

发布新版本会修改 `render.yaml` 中的镜像标签。在蓝图页面上，**Manual sync** 会按新版本重新部署各服务；网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，在每个服务上使用 **Rollback** 回到上一次部署；与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 依据

蓝图的结构、服务类型、套餐名称、`fromService` 和 `fromDatabase` 引用以及 `ipAllowList` 都遵循 2026 年 11 月 6 日阅读的 Render 蓝图规范。2026-11-07，蓝图从法兰克福的空白 Render 项目创建。它创建了全部五个资源，设置邮件送达，健康与就绪检查通过，流式回复进入请求日志，伪造的 `X-Forwarded-For` 被 Cloudflare 的 `CF-Connecting-IP` 地址替换（[已验证的内容](index.md#已验证的内容)）。实际测试发现并验证了转发地址问题的修复。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：在 Render 上备份 Postgres，并把主密钥保存在平台之外。
- [健康状态和指标](../monitoring.md)：边缘服务和网关应答的健康检查。
