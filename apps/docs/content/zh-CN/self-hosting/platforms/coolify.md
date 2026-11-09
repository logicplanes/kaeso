---
title: Coolify
description: 用 Compose 文件在 Coolify 上部署 Kaeso，使用 Coolify 的代理和自动生成的密码。
order: 8
---

仓库中的 `deploy/coolify/compose.yaml` 是一个用于 [Coolify](https://coolify.io/docs/knowledge-base/docker/compose) 的 Docker Compose 文件。它在您的 Coolify 服务器上运行 Coolify 代理后的公开边缘、私有网关和控制台，以及 Compose 包中的 PostgreSQL、ClickHouse 和 Valkey，全部固定为同一版本。Coolify 会生成所有密码和主密钥。

## 文件运行的内容

| 服务 | 镜像 | 谁能访问 |
| --- | --- | --- |
| `edge` | 控制台镜像，运行 `kaeso-edge` | 所有人，通过 Coolify 代理，使用您为它设置的域名 |
| `gateway` | 网关镜像 | 边缘服务 |
| `console` | 控制台镜像 | 边缘服务 |
| `postgres`、`clickhouse`、`valkey` | 与 Compose 包相同的版本 | 网关 |

每个服务都保留 Compose 包的加固设置：不允许获取新权限，除数据存储需要的权能外去除所有 Linux 权能，Kaeso 的服务使用只读根文件系统，并设置内存和进程限制。网关以 `kaeso` 身份登录 PostgreSQL，该用户拥有自己的数据库，但不是超级用户。Valkey 从不逐出键，因此 Valkey 满了时会拒绝写入，而不是丢弃限额计数器。

Coolify 会填写名称以 `SERVICE_` 开头的值：`SERVICE_URL_EDGE_8080` 是边缘服务在 8080 端口上的地址，`SERVICE_PASSWORD_…` 是随机密码，`SERVICE_REALBASE64_32_MASTERKEY` 是 32 个随机字节，即主密钥。

## 部署

1. 在 Coolify 中打开一个项目，添加类型为 **Docker Compose Empty** 的资源。粘贴包含最新版本的 `prod` 分支中的文件：`https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/coolify/compose.yaml`。
2. 在 `edge` 服务下把域名设为 `https://` 加您的域名，例如 `https://kaeso.example.com`，并把它的 DNS 记录指向服务器。Kaeso 需要 `https` 地址。
3. 如果有 SMTP 服务器，在 **Environment Variables** 下填写（[电子邮件](../email.md)）：

   | 变量 | 填写内容 |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp` 表示发送邮件；默认值 `log` 只记录每封邮件的类型 |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；使用 SMTP 时，网关把设置链接发送到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器 |

4. 部署，并等待所有服务都健康。
5. 把 **Environment Variables** 中的 `SERVICE_REALBASE64_32_MASTERKEY` 复制到密码管理器；主密钥是 `k1:` 加上这个值。Kaeso 用它加密提供商密钥，没有它就无法读取数据库备份（[备份和主密钥](../backups.md)）。
6. 打开设置链接创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。使用 SMTP 时，链接会发送到 `KAESO_SETUP_EMAIL`。否则在服务器上的终端或 Coolify 的服务器终端中输出它：

   ```bash
   docker exec $(docker ps --quiet --filter name=gateway-) kaeso setup-link
   ```

网关镜像没有 shell，因此 Coolify 中网关容器的终端无法运行该命令。如果服务器上有多个 Kaeso 资源，请把 `docker ps` 为本资源列出的网关容器交给 `docker exec`。

## 更新和回滚

发布新版本会修改 `compose.yaml` 中的镜像标签。在 Coolify 中打开该资源的 Compose 文件，把 `edge`、`gateway` 和 `console` 的标签改为新版本，然后部署；Coolify 会保留已生成的值。网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，改回较早的标签；与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 限制

- Kaeso 的更新器在这里不运行，因此控制台无法更新 Kaeso；每个版本都需要您在 Coolify 中部署。
- Coolify 的代理终止 TLS。网关会在每个流中写入保活数据，因此长时间的流不会因代理的空闲超时而中断（[长时间的流](../domains-and-tls.md#长时间的流)）。

## 依据

该文件遵循 Coolify 关于 Docker Compose 和贡献服务的页面，以及 Coolify 源代码生成魔法变量的方式，均于 2026 年 11 月 9 日阅读。`docker compose config` 接受该文件。2026 年 11 月 9 日，它在本地 Docker 中运行，Coolify 生成的值由手动填写：所有服务都变为健康，边缘服务把 API 发送到网关、把其他所有路径发送到控制台，`kaeso setup-link` 输出了链接，`kaeso` 用户不是 PostgreSQL 超级用户。它尚未在 Coolify 上部署。请先在一台临时服务器上部署，并检查设置链接、流式回复以及请求日志中的客户端地址（[已验证的内容](index.md#已验证的内容)）。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 PostgreSQL，并把主密钥保存在服务器之外。
- [健康状态和指标](../monitoring.md)：各服务应答的健康检查。
