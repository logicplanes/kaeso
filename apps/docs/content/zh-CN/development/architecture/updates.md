---
title: 更新
description: 网关如何得知新版本，版本为更新器附带了什么，以及更新器如何校验并应用版本。
order: 5
---

```text
GitHub releases API ◀── gateway (update_check job, instance/updates/*) ──▶ PostgreSQL
                              │  bearer token, internal network "updates"
                              ▼
                           updater ──▶ github.com (release.json, compose.yaml), api.github.com (attestations), ghcr.io
                              │  internal network "docker"
                              ▼
                         docker-proxy ──▶ /var/run/docker.sock
                              │
updater ──▶ maintenance volume ◀── Caddy (updating page, JSON 503s)
```

## 网关

`services/updates.rs` 和 `update_check` 任务负责检查新版本：该任务每五分钟询问下一次检查是否到期（`update_settings.next_check_at`），一次检查会带着 `If-None-Match` 获取 `releases_url?per_page=50`。列表有变化时，会在一个事务中替换 `update_releases`；对每个比正在运行的版本更新的版本，获取并保存其 `release.json`，或保存无法获取的原因。下一次检查在 12 小时后再加上一小时内的随机时间；失败后至少一小时后，如果 GitHub 的 `Retry-After` 或 `x-ratelimit-reset` 要求，则更晚。重复的失败只在 debug 级别记录。

`kaeso_update::policy::plan` 决定提供哪个版本：渠道中比正在运行的版本更新的最新版本，以及它是否需要手动更新：新的主版本、途经版本（包括预发布版本）的清单中有 `manual: true`，或者根本没有清单。`instance/updates` 下的 Platform API 只供实例管理员使用；每次更改都会记录审计（`instance.update_check`、`instance.update_settings`、`instance.update_start`、`instance.update_finish`），`apply` 要求在最近 15 分钟内登录过。

`update_jobs` 保存每次更新；一个唯一索引保证同一时间只有一行处于 `running`。某行运行期间，读取它时会向更新器询问其状态，并由最先读取它的网关（通常是切换后的新网关）记录一次结束，写入 `instance.update_finish`。更新器不认识的任务，或一小时内没有响应的任务，会以 `interrupted` 或 `updater_unreachable` 结束。

## 版本附带的内容

发布工作流构建三个镜像 `kaeso-gateway`、`kaeso-console` 和 `kaeso-updater`，用 `actions/attest-build-provenance` 证明它们的构建来源，并附上 `release.json`（`scripts/release-manifest.mjs`），同样为其证明来源：

```json
{
  "schema": 1,
  "version": "1.4.2",
  "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
  "manual": false,
  "updater_protocol": 1,
  "images": {
    "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": "sha256:…" },
    "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": "sha256:…" },
    "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": "sha256:…" }
  },
  "compose": { "name": "compose.yaml", "sha256": "…" }
}
```

当发布工作流的 **manual** 输入如此指定，或 `compose.yaml` 与上一个版本相比除镜像版本外还有其他差异时，`manual` 为真。`kaeso_update::manifest` 解析它，拒绝其他模式版本，并检查每个名称、摘要和提交。

## 更新器

`crates/kaeso-updater` 是一个监听 8081 端口的小型 axum 服务器：`GET /healthz` 无需令牌；`GET /v1/status`、`POST /v1/updates`（`{ "id", "version" }`）和 `GET /v1/updates/{id}` 需要带有共享令牌的 `Authorization: Bearer`。它同一时间只运行一个更新，并把它保存在 `/state/job.json` 中，每一步之后以原子方式写入。

一次更新（`engine.rs`）：

1. 从版本下载 `release.json` 和 `compose.yaml`，检查清单，找到自身 Compose 项目中的网关和控制台容器（根据自身容器的标签），从网关镜像的 `org.opencontainers.image.version` 读取正在运行的版本，拒绝更旧、相同或新的主版本，并检查本地 `compose.yaml` 换上新镜像版本后与版本中的文件逐字节相同；
2. 按摘要拉取三个镜像，并校验 `release.json` 和每个镜像（`verify.rs`）：从 `api.github.com/repos/logicplanes/kaeso/attestations/sha256:…` 获取的证明，每个都是 Sigstore bundle，用 [sigstore-rs](https://github.com/sigstore/sigstore-rs) 依据 `data/trusted_root.json` 中的信任根校验（到 Fulcio 的证书链、SCT、颁发者 `https://token.actions.githubusercontent.com` 的身份 `https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod`、DSSE 签名、透明日志条目），再加上更新器自己用 Rekor 密钥校验的日志签名条目时间戳，以及 in-toto 声明：SLSA provenance v1、主体的名称和摘要、工作流以及 `release.json` 中的提交。对镜像而言，主体是按摘要从 `ghcr.io` 获取的镜像索引；拉取的镜像的版本标签必须与该版本一致；
3. 写入维护标记，然后先对控制台、再对网关：停止容器，将其重命名为 `…-previous`，用旧容器的设置创建新容器（`clone.rs`：来自旧镜像的内容都会去掉，因此新镜像的环境、命令和健康检查生效）并启动它；
4. 等待每个健康检查：控制台 2 分钟，网关的迁移 10 分钟；崩溃、重启或 `unhealthy` 会立即失败；
5. 就地写入 `compose.yaml`，为镜像打标签，删除之前的容器并清除标记。

切换前的失败不会更改任何内容（`failed`）。切换后的失败会按相反顺序重新启动之前的容器并等待它们（`rolled_back`，或带有原因的 `rollback_failed`）。启动时，如果文件显示某个更新正在运行，会回退该更新；如果只剩清理工作，则会完成它。

更新器只通过 `http://docker-proxy:2375/v1.44/` 访问 Docker，`compose.yaml` 中该代理的允许列表写明了它使用的每个路径。

## 边缘

Caddy 从 `maintenance` 卷读取维护标记。标记存在期间，每个请求都会立即得到维护响应；否则在网关或控制台返回 `502`、`503` 或 `504` 时返回维护响应。`/kaeso-maintenance.json` 提供该标记 `{"state":"updating","step":"migrating"}`，使页面可以显示当前步骤而不暴露版本。页面以内联方式写在 `compose.yaml` 中，其 Content-Security-Policy 通过哈希只允许它自己的样式和脚本。`crates/kaeso-server/tests/edge.rs` 检查 JSON 响应与 `ApiError` 和 `kaeso_codecs::dispatch::encode_error` 对同一消息生成的内容一致、哈希匹配，并且页面不从其他地方加载任何内容。

## 测试

| 位置 | 内容 |
| --- | --- |
| `crates/kaeso-update` | 版本标签、渠道、包含主版本、手动步骤和预发布版本的计划（含属性测试）、清单、协议 |
| `crates/kaeso-server/tests/updates.rs` | 对接模拟 GitHub 和模拟更新器：条件请求、失败及其退避、无法联网的安装、权限、审计日志、单一更新锁、进度以及每种结束状态 |
| `crates/kaeso-updater/tests/verify.rs` | Kaeso 0.1.0 的真实证明可以通过校验；换成其他镜像、提交、工作流、签名或日志条目则不能 |
| `crates/kaeso-updater/tests/engine.rs`、`server.rs` | 对接模拟的 Docker API：完整的更新、每种拒绝、回退、回退失败、更新中途重启、令牌和 API |
| `e2e/install` | 启用 `updater` 配置文件的安装：网关能访问更新器，更新器只能通过代理访问 Docker，以及 Kaeso 不可用期间 Caddy 以各种格式给出的响应，通过官方 SDK 验证 |
| `apps/console` | 更新页面的每种状态，包括单元测试以及对接测试环境中的模拟版本和模拟更新器；三种语言、浅色和深色下的更新中页面，并用 axe 检查 |
