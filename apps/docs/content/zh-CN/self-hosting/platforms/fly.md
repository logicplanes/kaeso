---
title: Fly.io
description: 在 Fly.io 上把 Kaeso 部署为私有网络中的四个应用，使用 Fly Managed Postgres。
order: 4
---

仓库中的 `deploy/fly/` 为每个应用提供一份 Fly Launch 配置。Fly.io 每个 `fly.toml` 运行一个应用，而 Kaeso 需要四个：一个公开边缘、一个私有网关和控制台，以及 Valkey。Fly Managed Postgres 保存数据库。四个应用全部固定为同一版本。

## 文件创建的内容

| 应用 | 文件 | 镜像 | 机器 |
| --- | --- | --- | --- |
| `…-edge` | `edge.toml` | 控制台镜像，运行 `kaeso-edge` | `shared-cpu-1x`，256 MB |
| `…-gateway` | `gateway.toml` | 网关镜像 | `shared-cpu-2x`，2 GB |
| `…-console` | `console.toml` | 控制台镜像 | `shared-cpu-1x`，256 MB |
| `…-valkey` | `valkey.toml` | Valkey 官方镜像，与 Compose 包相同的版本，使用 1 GB 的卷 | `shared-cpu-1x`，512 MB |

只有边缘服务有公开服务。其他应用没有，因此只能在组织的私有网络中通过 `<app>.internal` 访问。该网络使用 IPv6，所以网关在 `[::]:8080` 上监听。边缘服务读取由 Fly 代理设置的 `Fly-Client-IP`，不使用调用方可控制的 `X-Forwarded-For`。网关信任私有网络，并把指标保留在回环地址上。

Valkey 的数据保持在 256 MB 以内，从不逐出键，因此 Valkey 满了时会拒绝写入，而不是丢弃限额计数器。它运行在一台机器和一个卷上，Fly 不会复制该卷。Valkey 只保存计数器和缓存：网关会从 PostgreSQL 重建预算计数器，速率窗口重新开始（[备份和主密钥](../backups.md)）。

## 部署

Fly.io 没有适用于多应用项目的部署按钮，因此用 Fly 的命令行工具 `fly` 部署。

1. 把主密钥生成到 `KAESO_MASTER_KEYS` 中，并把 `echo` 输出的内容复制到密码管理器：

   ```bash
   export KAESO_MASTER_KEYS=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)
   echo $KAESO_MASTER_KEYS
   ```

2. 安装 [`fly`](https://fly.io/docs/flyctl/install/)，登录，并克隆包含最新版本的 `prod` 分支：

   ```bash
   fly auth login
   git clone --branch prod https://github.com/logicplanes/kaeso.git
   cd kaeso
   ```

3. 应用名称在整个 Fly.io 中唯一，因此请选择自己的前缀，然后创建四个应用：

   ```bash
   export KAESO_APP=kaeso-example
   fly apps create $KAESO_APP-edge
   fly apps create $KAESO_APP-gateway
   fly apps create $KAESO_APP-console
   fly apps create $KAESO_APP-valkey
   ```

4. 在应用所在区域创建 Managed Postgres 集群，并把它关联到网关。`fly mpg list` 会显示集群的 ID：

   ```bash
   fly mpg create --name $KAESO_APP --region fra --pg-major-version 17
   fly mpg attach $CLUSTER_ID --app $KAESO_APP-gateway --variable-name KAESO_POSTGRES_URL
   ```

5. 设置密钥，把示例换成您的 SMTP 服务器（[电子邮件](../email.md)）：

   ```bash
   export VALKEY_PASSWORD=$(openssl rand -hex 24)
   fly secrets set --stage --app $KAESO_APP-valkey VALKEY_PASSWORD=$VALKEY_PASSWORD
   fly secrets set --stage --app $KAESO_APP-edge KAESO_GATEWAY_UPSTREAM=$KAESO_APP-gateway.internal:8080 KAESO_CONSOLE_UPSTREAM=$KAESO_APP-console.internal:3000
   fly secrets set --stage --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://$KAESO_APP-edge.fly.dev \
     KAESO_VALKEY_URL=redis://:$VALKEY_PASSWORD@$KAESO_APP-valkey.internal:6379 KAESO_MASTER_KEYS="$KAESO_MASTER_KEYS" \
     KAESO_SETUP_EMAIL=owner@example.com KAESO_SMTP_HOST=smtp.example.com KAESO_SMTP_USER=kaeso \
     KAESO_SMTP_PASSWORD=secret KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
   ```

6. 先把 Valkey 和控制台各部署为一台机器，再部署网关和边缘服务：

   ```bash
   fly deploy --config deploy/fly/valkey.toml --app $KAESO_APP-valkey --ha=false
   fly deploy --config deploy/fly/console.toml --app $KAESO_APP-console --ha=false
   fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway
   fly deploy --config deploy/fly/edge.toml --app $KAESO_APP-edge
   ```

7. 网关会把一次性设置链接发送到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。网关镜像没有可运行 `kaeso setup-link` 的 shell。如果邮件没有送达，网关日志会写明 SMTP 错误；修正密钥后，网关会在下次启动时发送新链接。
8. 要使用自己的域名，给边缘服务添加证书，并按 `fly certs add` 的提示设置 DNS 记录。然后把网关的 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名：

   ```bash
   fly certs add kaeso.example.com --app $KAESO_APP-edge
   fly secrets set --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://kaeso.example.com
   ```

要保存请求日志，把网关的 `KAESO_CLICKHOUSE_URL` 设为带用户名和密码的 ClickHouse HTTP URL，例如 ClickHouse Cloud。没有它时请求日志关闭，用量、限额和预算照常工作（[选择请求日志的保存方式](../installation.md#选择请求日志的保存方式)）。

## 更新和回滚

发布新版本会修改 `edge.toml`、`gateway.toml` 和 `console.toml` 中的镜像标签。拉取 `prod` 分支，用第 6 步的命令先部署网关，再部署控制台和边缘服务。网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。机器停止时有 75 秒处理完剩余请求。要回滚，部署较早的镜像，例如 `fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway --image ghcr.io/logicplanes/kaeso-gateway:X.Y.Z`；与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 限制

- Fly Machines 没有只读根文件系统，也没有去除 Linux 权能的设置，因此各应用只以镜像自带的用户运行。
- Managed Postgres 提供 PostgreSQL 16 和 17；Compose 包运行 18。Kaeso 在 17 上运行，与 Google Cloud 上相同。
- 网关信任组织私有网络中的所有地址。如果该组织中的其他应用不应能设置网关记录的客户端地址，请把 Kaeso 放在单独的组织中运行。

## 依据

这些文件遵循 Fly.io 的应用配置参考、请求头和私有网络页面，以及 `fly mpg create` 和 `fly mpg attach` 的参考，均于 2026 年 11 月 9 日阅读。四个文件都能按 TOML 解析，并通过 [SchemaStore](https://www.schemastore.org/fly.json) 的 `fly.toml` 模式检查；该模式缺少 Fly 参考中记录的顶层 `[checks]` 部分。`fly config validate` 需要 Fly.io 账户，因此没有运行。这些应用尚未在 Fly.io 上部署。请先在一个临时组织中部署，并检查设置邮件、流式回复以及请求日志中的客户端地址（[已验证的内容](index.md#已验证的内容)）。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 Managed Postgres，并把主密钥保存在 Fly.io 之外。
- [健康状态和指标](../monitoring.md)：边缘服务和网关应答的健康检查。
