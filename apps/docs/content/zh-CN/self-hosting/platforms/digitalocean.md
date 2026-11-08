---
title: DigitalOcean
description: 用应用规范在 DigitalOcean App Platform 上部署 Kaeso，使用开发用 PostgreSQL 和托管 Valkey。
order: 3
---

仓库中的 `.do/app.yaml` 是一份 [App Platform 应用规范](https://docs.digitalocean.com/products/app-platform/reference/app-spec/)。它把网关和控制台作为两个服务运行在应用的边缘之后，配一个开发用 PostgreSQL 数据库和一个托管 Valkey 数据库，全部固定为同一版本。

## 规范创建的内容

| 组件 | 类型 | 镜像 | 规格 |
| --- | --- | --- | --- |
| `kaeso-gateway` | 端口 8080 上的服务 | 网关镜像 | `apps-s-1vcpu-2gb` |
| `kaeso-console` | 端口 3000 上的服务 | 控制台镜像 | `apps-s-1vcpu-0.5gb` |
| `kaeso-postgres` | 开发用数据库，PostgreSQL 18 | DigitalOcean 提供 | 512 MB |
| `kaeso-valkey` | 托管 Valkey 数据库 | DigitalOcean 提供，需先创建 | 自选 |

应用的边缘终止 TLS 并按路径转发：`/v1`、`/v1beta`、`/platform`、`/mcp`、`/oauth` 和 `/.well-known` 到达网关，其他所有路径到达控制台，因此这里不运行 `kaeso-edge`。网关只信任来自私有网络（边缘所在）的 `X-Forwarded-For`，并把指标保留在回环地址上。托管 Valkey 使用 DigitalOcean 自有机构签发的证书进行 TLS 通信，规范把它作为 `KAESO_VALKEY_CA` 交给网关（[配置](../configuration.md#设置)）。

**Deploy to DO** 按钮只部署一个服务加一个开发用数据库，而 Kaeso 需要两个服务和一个 Valkey，因此改用 `doctl` 创建。

## 部署

1. 生成主密钥，并保存在密码管理器中：

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. 安装 [`doctl`](https://docs.digitalocean.com/reference/doctl/)，登录，并在应用所在区域创建 Valkey 数据库：

   ```bash
   doctl auth init
   doctl databases create kaeso-valkey --engine valkey --region fra1 --size db-s-1vcpu-1gb --num-nodes 1 --wait
   ```

3. 在控制面板中该数据库的 **Settings** 里，把 **Eviction policy** 设为 **No eviction**，这样数据库写满时会拒绝写入，而不是丢弃限额计数器。也可以通过 API 完成，需要数据库的 id：

   ```bash
   curl -X PUT -H "Authorization: Bearer $DIGITALOCEAN_TOKEN" -H "Content-Type: application/json" \
     -d '{"eviction_policy":"noeviction"}' "https://api.digitalocean.com/v2/databases/$DATABASE_ID/eviction_policy"
   ```

4. 从您要部署的版本中复制 `.do/app.yaml`，填入它留空的值：

   | 值 | 填写内容 |
   | --- | --- |
   | `KAESO_MASTER_KEYS` | 第 1 步的密钥 |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；网关会把设置链接发到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器（[邮件](../email.md)） |
   | `KAESO_CLICKHOUSE_URL` | 用于请求日志的 ClickHouse HTTP URL，含用户名和密码；留空则关闭请求日志 |

   `KAESO_PUBLIC_URL` 是 `${APP_URL}`，即应用的主地址，数据库地址来自数据库本身，所以这些都无需手动输入。App Platform 在首次收到 `SECRET` 值时会将其加密。

5. 创建应用并等待首次部署完成：

   ```bash
   doctl apps create --spec app.yaml --wait
   ```

6. 网关会把一次性设置链接发到 `KAESO_SETUP_EMAIL`；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。
7. 要使用自己的域名，在规范的 `domains` 下加入该域名并设 `type: PRIMARY`，用 `doctl apps update` 应用规范，再按应用 **Networking** 标签页的提示设置 DNS 记录。`${APP_URL}` 跟随主域名，网关会用新地址重新部署。

开发用数据库费用很低，但不保留备份。生产环境请创建托管 PostgreSQL 集群，并通过 `production: true` 和它的 `cluster_name` 让 `kaeso-postgres` 指向它（[备份和主密钥](../backups.md)）。

## 更新和回滚

发布新版本会修改 `.do/app.yaml` 中的镜像标签。应用新规范后，应用会部署两个服务；网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）：

```bash
doctl apps update $APP_ID --spec app.yaml --wait
```

要回滚，在应用的 **Activity** 标签页中，在某个较早的部署旁选择 **Rollback**；最近十次成功的部署会被保留。与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 依据

规范的结构、镜像、ingress、健康检查、环境变量和数据库字段，以及可绑定的 `${APP_URL}`、`${…DATABASE_URL}` 和 `${…CA_CERT}`，都遵循 2026 年 11 月 7 日阅读的应用规范参考以及环境变量、容器镜像和数据库页面。两个镜像已在本地 Docker 中按路径转发的边缘布局测试（[已验证的内容](index.md#已验证的内容)）。规范尚未从空账户部署到 App Platform；请先在一次性项目中部署，并检查设置邮件、一次超过一分钟的流式回复（因为边缘与网关之间使用 HTTP/1.1）、请求日志中的客户端地址以及一次更新。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：改用带备份的托管 PostgreSQL 集群，并把主密钥保存在平台之外。
- [健康状态和指标](../monitoring.md)：网关应答的健康检查。
