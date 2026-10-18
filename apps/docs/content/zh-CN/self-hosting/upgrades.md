---
title: 升级
description: 了解新版本，手动更新或通过更新器一键更新，以及更新期间会发生什么。
order: 7
---

版本遵循语义化版本规范，Kaeso 的所有组成部分都使用该版本的版本号。每个版本的发布说明位于[发布页面](https://github.com/logicplanes/kaeso/releases)，实例管理员也可以在控制台的**设置** › **实例** › **更新**中查看。每个版本的 `compose.yaml` 固定了该版本的网关、控制台和更新器镜像。

更新有两种方式：[手动更新](#手动更新)，始终可用；以及在控制台中[一键更新](#一键更新)，需要先添加可选的更新器。

## 新版本

网关每天两次通过 GitHub 的发布 API 检查新版本，失败后至少等待一小时。请求中除了 `User-Agent` 里的网关版本外，不包含任何关于此安装的信息；[更新](../development/architecture/updates.md#网关)详细说明了这项检查。

在**设置** › **实例** › **更新**中，实例管理员可以看到正在运行的版本、最新版本及其之后所有版本的发布说明、网关上次检查的时间，以及检查失败时的原因。**立即检查**会马上检查，即使自动检查已关闭。设置侧边栏中**更新**旁的**新**标记是唯一的提醒；Kaeso 不会因为新版本发送邮件或显示横幅。

| 设置 | 默认值 | 作用 |
| --- | --- | --- |
| 检查新版本（`updates.check`） | 开启 | 无法联网的安装请关闭。 |
| 版本（`updates.channel`） | 仅稳定版 | `prerelease` 也会提供候选版本。稳定渠道从不提供预发布版本，但正在运行预发布版本的安装会获得其后的稳定版本。 |
| 一键更新（`updates.apply`） | 关闭 | 实例管理员能否通过更新器在控制台中应用更新。 |

新安装时，这三项设置取自 `KAESO_UPDATES_CHECK`、`KAESO_UPDATES_CHANNEL` 和 `KAESO_UPDATES_APPLY`；之后由实例管理员在控制台中更改，每次更改都会记录在实例审计日志中。开启一键更新需要在最近 15 分钟内登录过。

### 无法联网的安装

关闭**检查新版本**，或在首次启动前在 [`compose.override.yaml`](installation.md#本地修改) 中设置 `KAESO_UPDATES_CHECK=false`。此时页面只显示正在运行的版本，并按下文手动更新。如果您有按 GitHub 发布 API 格式响应的镜像，可以用 `KAESO_UPDATES_RELEASES_URL` 让网关指向它；此时网关只从该镜像自身的地址获取版本清单。

## 哪些版本可以一键更新

只有同时满足以下条件的版本才能一键更新；否则控制台会显示其发布说明和指向本页的链接：

- 不是新的主版本。需要手动步骤的迁移会使版本成为主版本。
- 途经的版本都不要求手动操作。如果某个版本的 `compose.yaml` 除镜像版本外还有其他变化，或维护者如此标记，该版本就要求手动操作。每个 `release.json` 都写明截至该版本最后一个要求手动操作的版本，因此即使网关从未见过那个版本，网关和更新器也会拒绝跨越它的更新。
- 该版本附带 `release.json`，即更新器读取的清单：从第一个带更新器的版本起，每个版本都有。

## 手动更新

1. 阅读发布说明，看看是否有需要处理的事项。
2. [备份](backups.md) PostgreSQL，也备份 ClickHouse，以便日后仍能[降级](#降级)。
3. 下载新版本的 `compose.yaml`，覆盖旧文件。`.env`、`master-keys` 和 `compose.override.yaml` 中的设置保持不变。

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. 启动新版本：

```bash
docker compose pull
docker compose up -d --wait
```

`--wait` 会等新的网关和控制台报告 healthy 后才返回，有一个达不到时命令失败，因此执行升级的脚本或人员会立即发现启动失败。

网关在启动时于锁的保护下迁移 PostgreSQL 和 ClickHouse，因此多个实例可以同时启动，只有一个执行迁移。迁移只向前进行且按顺序运行，因此升级时可以跳过版本。Kaeso 0.1.0 通过一组初始迁移创建完整的数据库结构；之后的版本在其后添加各自的迁移。

无法一键更新时，控制台会显示这些命令，并带上确切的版本号。

## 一键更新

更新器是 `compose.yaml` 中的可选服务，位于 Compose 配置文件 `updater` 中，并带有其 Docker 套接字代理 `docker-proxy`。它是 Kaeso 中唯一与 Docker 通信的部分。

### 开启

在安装所在的文件夹中，为网关和更新器设置一个共享令牌，并开启该配置文件：

```bash
echo "KAESO_UPDATER_TOKEN=$(openssl rand -hex 32)" >> .env
echo "COMPOSE_PROFILES=updater" >> .env
docker compose up -d --wait
```

然后在**设置** › **实例** › **更新**中开启**一键更新**。如果更新器未安装、没有响应或对某个版本来说过旧，页面会说明。更新器出现之前的 Kaeso 版本需要手动更新一次。

如果不想把令牌放在 `.env` 中，可以把它写入一个文件，并在 `compose.override.yaml` 中作为 [Compose secret](https://docs.docker.com/compose/how-tos/use-secrets/) 交给两个服务；两者都读取 `KAESO_UPDATER_TOKEN_FILE`：

```yaml
services:
  gateway:
    environment:
      KAESO_UPDATER_TOKEN_FILE: /run/secrets/updater_token
    secrets: [updater_token]
  updater:
    environment:
      KAESO_UPDATER_TOKEN_FILE: /run/secrets/updater_token
    secrets: [updater_token]
secrets:
  updater_token:
    file: ./updater-token
```

更新器需要 Docker Engine 25 或更高版本：它使用 Docker API 1.44，代理也只放行这个版本；引擎较旧时，页面会显示更新器没有响应。它通过其代理挂载 `/var/run/docker.sock`；如果 Docker 的套接字在其他位置（例如 rootless Docker），请在 `.env` 中设置 `DOCKER_SOCKET`。

### 更新过程

**更新到 …** 会要求确认，确认框会写明版本并提醒先备份。网关记录这次更新，在实例审计日志中写入 `instance.update_start`，并只把版本号交给更新器。随后控制台会跟进以下步骤：

1. **校验签名。**在下载任何镜像之前，更新器会获取 `release.json`、`compose.yaml`，并按 `release.json` 中的摘要获取每个镜像的索引，然后使用内置于更新器的信任根，通过 Sigstore 将每一项与 Kaeso 在 `prod` 上的发布工作流的[构建来源证明](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations)进行校验（[每项校验](../development/architecture/updates.md#更新器)）。任何一项校验失败时，都不会安装任何内容，Kaeso 继续运行。
2. **下载。** 更新器按摘要拉取网关、控制台和更新器镜像，并检查每个镜像自身的版本标签是否为该版本。
3. **切换。** Caddy 以[更新页面](#kaeso-更新期间)作答。更新器停止控制台，用其原有设置和新镜像重新启动它并等待其健康检查；然后对网关做同样的操作。
4. **迁移。** 新网关在启动时迁移数据库；更新器最多给它的健康检查 10 分钟时间，只要网关在运行且没有重启，失败的检查就不计入，因此耗时较长的迁移不会被误认为启动失败。
5. **恢复在线。** 更新器把新版本写入 `compose.yaml`，这样之后的 `docker compose up -d --wait` 会启动同样的镜像；它为镜像打上版本标签，删除旧容器并结束维护。

整个更新大约需要一分钟。同一时间只运行一个更新。结果会作为 `instance.update_finish` 记录在实例审计日志和页面的历史中：

| 结果 | 含义 |
| --- | --- |
| **已更新** | 新版本正在运行。如果更新器无法写入 `compose.yaml`，或该文件在更新期间被修改，结果会说明这一点：请手动把 `compose.yaml` 中 Kaeso 镜像的标签改为新版本。 |
| **未安装** | 没有任何更改。 |
| **已回退** | 新版本没有启动，之前的版本已重新运行。 |
| **回退失败** | 新版本没有启动，之前的版本也没能重新启动。 |
| **需要处理** | 需要有人检查：新网关没有启动但可能已经迁移了数据库，因此更新器保留了它和新控制台（参见[回退](#回退)）；或者更新未能完成清理，例如删除维护标记或 `…-previous` 容器。详情会说明是哪种情况以及该怎么做。 |

### 回退

切换之前，更新器会比较正在运行的版本与新版本最新的 PostgreSQL 和 ClickHouse 迁移，两者都取自各自的 `release.json`。

如果新控制台没有变为健康状态，或者新网关没有变为健康状态而两个版本的迁移相同，更新器会删除新容器并重新启动之前的容器：更新状态为**已回退**，Kaeso 运行之前的版本。

如果新网关没有变为健康状态，而两个版本的迁移不同，或者正在运行的版本没有 `release.json`，新网关可能已经迁移了数据库，而之前的网关[无法在其上运行](#降级)。此时更新器会保留新网关和使用其 API 的新控制台，将之前的网关和控制台以 `kaeso-gateway-1-previous` 和 `kaeso-console-1-previous` 的名称保持停止状态，并以**需要处理**结束。这时可以查明新网关无法启动的原因（`docker compose logs gateway`），修复后把 `compose.yaml` 中的镜像标签改为新版本；或者恢复备份并运行旧版本，参见[降级](#降级)。完成后请删除这两个 `…-previous` 容器；只要其中一个存在，更新器就不会开始新的更新。

如果更新器在更新期间重启，它会最多等待 Docker 两分钟；之后，新网关变为健康时它会完成该更新，否则按上述方式回退，Docker 没有响应时则以**需要处理**结束。

### 更新器本身

更新器也会拉取并校验它自己的新镜像，并把自己的版本与其他镜像一起写入 `compose.yaml`，但它不会在运行期间替换自己。在安装所在文件夹中下一次运行 `docker compose up -d --wait` 时，它会切换到新版本。

### 安全模型

| 组件 | 能做什么 |
| --- | --- |
| 网关 | 请求更新器按版本号应用某个版本，并读取进度。它从不获得 Docker 访问权限，也无法指定镜像、选项或容器。 |
| 更新器 | 在其容器内监听 `0.0.0.0:8081`，并连接三个网络：只有网关与它共享的 `updates`、只有代理与它共享的 `docker`，以及只有它使用、用于访问 github.com、api.github.com 和 ghcr.io 的 `egress`。除健康检查外，每个请求都必须携带令牌：至少 32 个字符，以恒定时间比较，从不记录或返回。它只替换自身 Compose 项目中的网关和控制台容器，使用它们原有的设置和它按摘要固定并校验过的镜像；它只修改 `compose.yaml` 中的镜像版本，如果该文件与版本中的文件有任何其他差异，它不会改动。 |
| `docker-proxy` | 位于内部网络 `docker` 中的 [socket-proxy](https://github.com/wollomatic/socket-proxy)。它只响应更新器，并且只放行更新器调用的 Docker API 路径：列出和检查容器，检查、拉取和标记 Kaeso 镜像，以及创建、启动、停止、重命名和删除容器。它只按方法和路径过滤，不检查请求的内容。 |

代理限制了更新器出错时可能造成的影响，但并不能让更新器变得无害：更新器创建的容器按其设置可以挂载主机的任何部分或以特权模式运行。掌握了更新器，或掌握了其令牌并能进入 `updates` 网络的人，就以 root 身份掌握了主机。因此更新器是可选的，它丢弃了除写入 `compose.yaml` 所需之外的所有能力，使用只读文件系统，并且只与网关和代理共享其网络。没有 `updater` 配置文件时，Kaeso 中没有任何部分能访问 Docker。

更新器校验的是透明日志条目带有签名时间戳的证明。如果发布工作流将来生成只带包含证明和时间戳机构签名的证明包，更新器会拒绝它们并说明原因，这些版本需要手动更新，直到能校验它们的更新器发布为止。

## Kaeso 更新期间

在更新器切换版本期间，以及网关或控制台没有响应的任何时候（包括手动更新时），Caddy 会代为作答：

| 请求 | 响应 |
| --- | --- |
| 页面 | `503`，并返回一个页面：根据浏览器语言使用英语、德语或简体中文，根据系统设置使用浅色或深色。更新器工作期间它会显示当前步骤，每 5 秒检查一次，Kaeso 恢复后返回控制台。它不会从其他地方加载任何内容。 |
| `/platform/v1/*` | `503`，返回 Platform API 的错误，类型为 `unavailable` |
| `/v1/messages`，以及带有 `anthropic-version` 请求头的其他 `/v1/*` | `529`，返回 Anthropic 的 `overloaded_error`，与服务商过载时网关的响应相同 |
| 其他 `/v1/*` | `503`，返回 OpenAI 的 `service_unavailable_error` |

每个响应都带有 `Retry-After: 30`，OpenAI 和 Anthropic 的 SDK 重试时会遵循它。消息会说明 Kaeso 是正在更新还是暂时无法访问。

## 保持不变的名称

以下名称在每个版本中都相同，因此下载新的 `compose.yaml` 并运行 `docker compose pull` 后数据依然保留：

- 下载地址 `https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml`；
- 镜像 `ghcr.io/logicplanes/kaeso-gateway`、`ghcr.io/logicplanes/kaeso-console` 和 `ghcr.io/logicplanes/kaeso-updater`，标签为该版本的版本号；
- Compose 项目 `kaeso` 及其服务 `caddy`、`gateway`、`console`、`postgres`、`clickhouse` 和 `valkey`，以及 `updater` 配置文件中的 `updater` 和 `docker-proxy`；
- 存放数据的卷 `kaeso_postgres`、`kaeso_clickhouse` 和 `kaeso_valkey`，存放 TLS 证书的卷 `kaeso_caddy_data` 和 `kaeso_caddy_config`，以及存放维护标记和更新器进度的卷 `kaeso_maintenance` 和 `kaeso_updater`。

请在安装所在的文件夹中运行 Compose，不要使用 `-p` 或 `COMPOSE_PROJECT_NAME`，这样它会使用项目名 `kaeso`：换成其他项目名时，Compose 会从新的空卷开始。

## 降级

迁移无法撤销。PostgreSQL 数据库若已被更新的版本迁移过，网关会拒绝启动并说明原因。ClickHouse 数据库若已被更新的版本迁移过，网关仍会启动，但不写入请求日志，并记录 `ClickHouse migrations failed; request logs are not written`。

要回到旧版本，先恢复升级前的 PostgreSQL 备份；如果新版本迁移过 ClickHouse，再恢复 [ClickHouse 备份](backups.md#clickhouse)，然后启动旧版本的 `compose.yaml`。
