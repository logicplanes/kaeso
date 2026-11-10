---
title: Dokploy
description: 通过导入模板在 Dokploy 上部署 Kaeso，使用 Traefik 和自动生成的密码。
order: 9
---

仓库中的 `deploy/dokploy/` 包含一个 [Dokploy 模板](https://github.com/Dokploy/templates)：`docker-compose.yml` 和 `template.toml`。它在您的 Dokploy 服务器上运行 Dokploy 的 Traefik 后的公开边缘、私有网关和控制台，以及 Compose 包中的 PostgreSQL、ClickHouse 和 Valkey，全部固定为同一版本。模板会生成所有密码和主密钥。

## 模板运行的内容

| 服务 | 镜像 | 谁能访问 |
| --- | --- | --- |
| `edge` | 控制台镜像，运行 `kaeso-edge` | 所有人，通过 Traefik，使用模板的域名 |
| `gateway` | 网关镜像 | 边缘服务 |
| `console` | 控制台镜像 | 边缘服务 |
| `postgres`、`clickhouse`、`valkey` | 与 Compose 包相同的版本 | 网关 |

每个服务都保留 Compose 包的加固设置：不允许获取新权限，除数据存储需要的权能外去除所有 Linux 权能，Kaeso 的服务使用只读根文件系统，并设置内存和进程限制。网关以 `kaeso` 身份登录 PostgreSQL，该用户拥有自己的数据库，但不是超级用户。Valkey 从不逐出键，因此 Valkey 满了时会拒绝写入，而不是丢弃限额计数器。

`template.toml` 为边缘服务在 8080 端口上设置域名，并设置环境变量：随机密码来自 Dokploy 的 `hash` 辅助函数，主密钥来自它的 `base64` 辅助函数，即 32 个随机字节。在您添加 SMTP 服务器之前，邮件只会被记录。

## 部署

1. 在装有 `curl` 和 `jq` 的电脑上，把包含最新版本的 `prod` 分支中的模板转换为 Dokploy 可导入的 Base64 文本：

   ```bash
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/docker-compose.yml
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/template.toml
   jq -n --rawfile compose docker-compose.yml --rawfile config template.toml '{compose: $compose, config: $config}' | base64 | tr -d '\n'
   ```

2. 在 Dokploy 中打开一个项目，创建 **Compose** 服务。在 **Advanced** 下把文本粘贴到 **Import** 中并导入。Dokploy 会在应用之前显示 Compose 文件、环境变量、挂载和域名。
3. 在 **Domains** 下把主机改为您的域名，例如 `kaeso.example.com`，开启使用 Let's Encrypt 的 HTTPS，并把域名的 DNS 记录指向服务器。
4. 在 **Environment** 下把 `KAESO_PUBLIC_URL` 设为 `https://` 加该域名；如果有 SMTP 服务器，也一并填写（[电子邮件](../email.md)）：

   | 变量 | 填写内容 |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp` 表示发送邮件；默认值 `log` 只记录每封邮件的类型 |
   | `KAESO_SETUP_EMAIL` | 您的邮箱地址；使用 SMTP 时，网关把设置链接发送到这里 |
   | `KAESO_SMTP_HOST`、`KAESO_SMTP_PORT`、`KAESO_SMTP_TLS`、`KAESO_SMTP_USER`、`KAESO_SMTP_PASSWORD`、`KAESO_SMTP_FROM` | 您的 SMTP 服务器 |

5. 部署，并等待所有服务都健康。
6. 把 **Environment** 中的 `KAESO_MASTER_KEYS` 复制到密码管理器。Kaeso 用它加密提供商密钥，没有它就无法读取数据库备份（[备份和主密钥](../backups.md)）。
7. 打开设置链接创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。使用 SMTP 时，链接会发送到 `KAESO_SETUP_EMAIL`。否则在服务器上输出它，把 `kaeso-example` 换成该服务 **General** 标签页中的应用名称：

   ```bash
   docker exec kaeso-example-gateway-1 kaeso setup-link
   ```

网关镜像没有 shell，因此 Dokploy 中网关容器的终端无法运行该命令。

## 更新和回滚

发布新版本会修改 `docker-compose.yml` 中的镜像标签。在 Dokploy 中打开该服务的 Compose 文件，把 `edge`、`gateway` 和 `console` 的标签改为新版本，然后部署；环境变量保持不变。网关在启动时迁移数据库，只向前进行（[升级](../upgrades.md)）。要回滚，改回较早的标签；与任何安装一样，如果新版本的迁移是旧版本无法运行的，就只能保留新版本。

## 限制

- Kaeso 的更新器在这里不运行，因此控制台无法更新 Kaeso；每个版本都需要您在 Dokploy 中部署。
- Traefik 终止 TLS。网关会在每个流中写入保活数据，因此长时间的流不会因代理的空闲超时而中断（[长时间的流](../domains-and-tls.md#长时间的流)）。

## 依据

该模板遵循 Dokploy 模板仓库的格式，以及 Dokploy 导入模板和辅助函数的源代码，均于 2026 年 11 月 9 日阅读。`docker compose config` 接受带有模板环境变量的 Compose 文件。2026 年 11 月 9 日，它在本地 Docker 中运行，这些环境变量由手动填写：所有服务都变为健康，边缘服务把 API 发送到网关、把其他所有路径发送到控制台，网关完成了 ClickHouse 迁移，`kaeso setup-link` 输出了链接，`kaeso` 用户不是 PostgreSQL 超级用户。它尚未导入 Dokploy。请先在一台临时服务器上导入，并检查设置链接、流式回复以及请求日志中的客户端地址（[已验证的内容](index.md#已验证的内容)）。

## 下一步

- [配置](../configuration.md)：网关读取的每项设置及其作用。
- [备份和主密钥](../backups.md)：备份 PostgreSQL，并把主密钥保存在服务器之外。
- [健康状态和指标](../monitoring.md)：各服务应答的健康检查。
