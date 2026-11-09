---
title: CapRover
description: 用仓库中的模板在 CapRover 上把 Kaeso 部署为一键应用。
order: 10
---

仓库中的 `deploy/caprover/kaeso.yml` 是一个 [CapRover 一键应用](https://github.com/caprover/one-click-apps)。它在您的 CapRover 服务器上创建六个应用：CapRover 的 nginx 后的公开边缘、私有网关和控制台，以及 Compose 包中的 PostgreSQL、ClickHouse 和 Valkey，全部固定为同一版本。

## 模板创建的内容

| 应用 | 镜像 | 谁能访问 |
| --- | --- | --- |
| `kaeso` | 控制台镜像，运行 `kaeso-edge` | 所有人，通过 nginx，使用该应用的域名 |
| `kaeso-gateway` | 网关镜像 | 边缘服务 |
| `kaeso-console` | 控制台镜像 | 边缘服务 |
| `kaeso-postgres`、`kaeso-clickhouse`、`kaeso-valkey` | 与 Compose 包相同的版本 | 网关 |

这些名称取决于您填写的应用名称，这里是 `kaeso`。只有边缘服务是 Web 应用；其他应用只能在 CapRover 的覆盖网络中访问。网关以 `kaeso` 身份登录 PostgreSQL，该用户拥有自己的数据库，但不是超级用户。Valkey 的数据保持在 512 MB 以内，从不逐出键，因此 Valkey 满了时会拒绝写入，而不是丢弃限额计数器。

CapRover 的 nginx 会追加调用方的 `X-Forwarded-For`，因此边缘服务读取由 nginx 设置的 `X-Real-IP`。模板为边缘服务开启了 WebSocket 支持，这样 nginx 会用 HTTP/1.1 与它通信。网关只信任私有地址，并把指标保留在回环地址上。

## 部署

1. 生成主密钥并保存到密码管理器：

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. 在 CapRover 中打开 **Apps**，再打开 **One-Click Apps/Databases**，选择列表末尾的 **>> TEMPLATE <<**。
3. 粘贴包含最新版本的 `prod` 分支中的模板：`https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/caprover/kaeso.yml`。
4. 填写表单：

   | 字段 | 填写内容 |
   | --- | --- |
   | App name | `kaeso`，或您自己的名称 |
   | Public URL | 表单填好的、该应用在根域名下的地址，或您自己的域名，例如 `https://kaeso.example.com` |
   | Master key | 第 1 步的密钥 |
   | 密码 | 表单填好的随机密码 |
   | Mail transport | `smtp` 表示发送邮件；默认值 `log` 只记录每封邮件的类型 |
   | Setup email、SMTP host、port、user、password、sender | 您的 SMTP 服务器（如果有，见[电子邮件](../email.md)） |

5. 部署，并等待应用创建完成。网关会不断重启，直到 PostgreSQL 和 Valkey 应答。
6. 打开应用 `kaeso`，开启 **HTTPS** 和 **Force HTTPS**；如果 Public URL 使用了您的域名，再连接该域名。
7. 打开设置链接创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。使用 SMTP 时，链接会发送到 Setup email。否则在服务器上输出它：

   ```bash
   docker exec $(docker ps --quiet --filter name=srv-captain--kaeso-gateway) kaeso setup-link
   ```

## 更新和回滚

发布新版本会修改 `kaeso.yml` 中的镜像标签。在 `kaeso-gateway`、`kaeso-console` 和 `kaeso` 这三个应用中分别打开 **Deployment**，按镜像名称用新标签部署，例如 `ghcr.io/logicplanes/kaeso-gateway:{{version}}`，先部署网关。网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，用同样的方式部署较早的标签；与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 限制

- CapRover 只读取一键应用中的部分 Compose 字段。健康检查、去除的权能、只读根文件系统和内存限制都不会作用到这些应用上，因此它们以镜像的默认设置运行。
- nginx 默认等待回复 60 秒。流会发送保活数据，但耗时更长的非流式请求会得到 `504`。如需更长时间，请在边缘应用的 nginx 配置中调高 `proxy_read_timeout`。
- Kaeso 的更新器在这里不运行，因此控制台无法更新 Kaeso；每个版本都需要您在 CapRover 中部署。

## 依据

该模板遵循 CapRover 的一键应用指南、其源代码读取的 Compose 字段以及它的 nginx 配置，均于 2026 年 11 月 9 日阅读。它能按 YAML 解析，所用的每个字段都是 CapRover 会读取的。它尚未在 CapRover 上部署。请先在一台临时服务器上部署，并检查设置链接、流式回复以及请求日志中的客户端地址（[已验证的内容](index.md#已验证的内容)）。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 PostgreSQL，并把主密钥保存在服务器之外。
- [域名和 TLS](../domains-and-tls.md)：长时间的流，以及 Kaeso 前面的代理需要满足的条件。
