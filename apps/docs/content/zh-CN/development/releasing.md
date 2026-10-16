---
title: 发布
description: 版本如何编号、一个版本发布哪些内容，以及维护者如何发布版本。
order: 9
---

Kaeso 作为一个整体发布：网关、控制台、KPP 的 crate 和 UI 库的包共用同一个版本号，一次发布就会发布所有这些内容。贡献者从不创建标签或版本；每个版本都由维护者通过仓库的 Release 工作流发布，文档则通过 Pages 工作流发布。

## 版本号

版本号遵循[语义化版本](https://semver.org)。如果某个版本需要运行 Kaeso 的人手动执行某个步骤，例如一个无法自动运行的迁移，它就是一个主版本。预发布版本会加上后缀，例如 `0.2.0-rc.1`。版本的 Git 标签是 `v` 加上版本号，例如 `v0.1.0`。

同一个版本的网关和控制台始终配套使用：该版本的 `compose.yaml` 将两个镜像都固定在这个版本，控制台也只适配其对应网关的 Platform API。

## 分支

| 分支 | 用途 |
| --- | --- |
| `dev` | 默认分支。所有拉取请求都提交到这里，版本也从这里发布 |
| `prod` | 最新的稳定版本。只有 Release 工作流会移动它，使其指向非预发布版本的提交 |
| `hotfix/*` | 针对最新版本的修复，从 `prod` 创建分支，并从该分支本身发布 |

热修复版本发布后，`prod` 上会有一个 `dev` 没有的提交；请将 `prod` 合并到 `dev`，否则下一次从 `dev` 发布会被拒绝。

## 一个版本发布什么

| 位置 | 内容 |
| --- | --- |
| GitHub releases | 版本 `vX.Y.Z`，标题为“Kaeso X.Y.Z”，附带来自 `CHANGELOG.md` 的发布说明和以下文件：`compose.yaml`，即[安装](../self-hosting/installation.md)时下载的文件；`kaeso.example.toml`；`env.example`；两个 npm 包的 tarball 和两个 `.crate` 文件 |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` 和 `ghcr.io/logicplanes/kaeso-console`，支持 `linux/amd64` 和 `linux/arm64`，附带构建来源证明和软件物料清单（SBOM） |
| crates.io | `kaeso-kpp` 和 `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens`，附带来源证明 |

| 镜像标签 | 适用于 |
| --- | --- |
| 版本号，例如 `0.2.0` 或 `0.2.0-rc.1` | 每个版本 |
| `MAJOR.MINOR`，例如 `0.2`，以及 `latest` | 每个非预发布版本 |
| `MAJOR`，例如 `1` | 从 `1.0.0` 起的每个非预发布版本 |

在 npm 上，正式版本使用 `latest` 标签，预发布版本使用 `next` 标签。预发布版本在 GitHub 上会被标记为预发布，永远不会成为最新版本，因此 `releases/latest/download/compose.yaml` 始终指向最新的稳定版本。

## 变更日志

仓库根目录下的 `CHANGELOG.md` 遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)。每个会被人注意到的变更都要在同一个拉取请求中于 `[Unreleased]` 下添加一行：从使用、运行 Kaeso 或基于 Kaeso 构建的人的角度，写明变更了什么。

发布时会在 `## [Unreleased]` 下方插入标题 `## [X.Y.Z] - YYYY-MM-DD`，这样此前积累在那里的内容就成为该版本的一节。文件末尾的 `[Unreleased]` 链接随后改为与新标签比较，并新增一个指向该版本的链接。这一节就是该版本的发布说明。`[Unreleased]` 下没有内容的发布会被拒绝，除非 `CHANGELOG.md` 中已经有该版本的一节。

## 发布一个版本

1. **准备好 `dev`。** [完成标准](testing.md#完成变更之前)中的每项检查都通过，包括 `pnpm release:check`，并且 `CHANGELOG.md` 写明了该版本的所有变更。
2. **刷新模型数据**，让全新安装一开始就有最新的价格和模型，并提交结果：

```bash
cargo xtask models-dev
```

3. **运行 Release 工作流。** 在仓库的 **Actions** 中选择 **Release**，然后选择 **Run workflow**。选择分支（`dev` 或某个 `hotfix/*` 分支），并输入版本号，例如 `0.2.0` 或 `0.2.0-rc.1`；开头的 `v` 会被去掉。
4. **发布文档。** 选择 **Pages**，然后选择 **Run workflow**，并输入该版本的标签，例如 `v0.2.0`。默认值 `prod` 是最新的稳定版本；发布预发布版本后，请输入其标签来发布对应的文档。

## Release 工作流

`.github/workflows/release.yml` 按顺序运行以下作业。写入仓库或发布内容的作业都在 `release` 环境中运行。

| 作业 | 作用 |
| --- | --- |
| Validate | 拒绝 `dev` 或 `hotfix/*` 以外的分支、不符合语义化版本的版本号、已存在的标签、不比最新标签更新的版本，以及空的 `[Unreleased]` |
| Checks | 运行完整的 CI 工作流，无论改动了什么，所有作业都会运行 |
| Tag | 用 `cargo set-version` 设置每个 crate 的版本，并更新 `Cargo.lock`、`fuzz/Cargo.lock` 和每个 `package.json` 中的版本；重新生成 `openapi.json`；在 `deploy/compose.yaml` 中固定两个镜像；移动变更日志。随后将“Release X.Y.Z”提交到该分支；如果 `prod` 上有该分支没有的提交则中止；创建标签 `vX.Y.Z`，并同时推送分支和标签；版本不是预发布版本时也一并推送 `prod` |
| Image | 从标签为每个平台构建每个镜像，附带完整的构建来源证明和 SBOM |
| Image manifest | 将两个平台合并到上述镜像标签下，并证明构建的来源 |
| Pack npm packages | 构建这些库，并用该版本号运行包检查 |
| npm | 发布 `@logicplanes/kaeso-tokens` 和 `@logicplanes/kaeso-ui` |
| crates.io | 发布 `kaeso-kpp` 和 `kaeso-codecs` |
| GitHub release | 下载已发布的 tarball 和 crate，并创建附带文件和发布说明的版本 |

npm 和 crates.io 作业会跳过已经发布的版本，因此某个发布作业失败时，重新运行该次运行中失败的作业即可完成发布。

## Pages 工作流

`.github/workflows/pages.yml` 只有一个输入：要发布的标签或分支。它会安装工作区，用 `pnpm build:packages` 构建这些库，用 `pnpm --filter kaeso-docs build` 构建本站（包括体积预算检查），并将 `apps/docs/dist` 部署到 GitHub Pages 上的 docs.kaeso.app。文档描述的是它发布时所基于的版本。
