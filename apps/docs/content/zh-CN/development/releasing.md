---
title: 发布
description: 版本如何编号、一个版本发布哪些内容，以及维护者如何发布版本。
order: 9
---

Kaeso 作为一个整体发布：网关、控制台、KPP 的 crate 和 UI 库的包共用同一个版本号，一次发布就会发布所有这些内容。贡献者从不创建标签或版本；每个版本都由维护者从 `prod` 通过仓库的 Release 工作流发布，文档则通过 Pages 工作流发布。

## 版本号

版本号遵循[语义化版本](https://semver.org)。如果某个版本需要运行 Kaeso 的人手动执行某个步骤，例如一个无法自动运行的迁移，它就是一个主版本。预发布版本会加上后缀，例如 `0.2.0-rc.1`。版本的 Git 标签是 `v` 加上版本号，例如 `v0.1.0`。

同一个版本的网关和控制台始终配套使用：该版本的 `compose.yaml` 将两个镜像都固定在这个版本，控制台也只适配其对应网关的 Platform API。

## 分支

| 分支 | 用途 |
| --- | --- |
| `dev` | 默认分支。所有拉取请求都提交到这里，维护者也可以直接推送到它 |
| `prod` | 最新的版本，或即将成为版本的提交。维护者推送到它来发布版本，且始终是快进推送；Release 工作流只在这里运行 |
| `hotfix/X.Y.Z` | 针对最新版本的修复，从该版本的标签创建分支，并推送到 `prod` |

`dev` 和 `prod` 都不能被删除或强制推送，`v*` 标签也不能被移动或删除。每次发布后，Release 工作流都会把该版本带回 `dev`，因此下一次把 `dev` 推送到 `prod` 时又是快进推送。

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

发布时会在 `## [Unreleased]` 下方插入标题 `## [X.Y.Z] - YYYY-MM-DD`，这样此前积累在那里的内容就成为该版本的一节。文件末尾的 `[Unreleased]` 链接随后改为与新标签比较，并新增一个指向该版本的链接。这一节就是该版本的发布说明。`[Unreleased]` 下没有内容的发布会被拒绝，除非 `CHANGELOG.md` 中已经有该版本的一节；对于正式版本，已有其预发布版本的一节也可以。

在同一版本的预发布版本之后发布正式版本时（例如在 `0.2.0-rc.1` 和 `0.2.0-rc.2` 之后发布 `0.2.0`），会把这些预发布版本的各节以及 `[Unreleased]` 下的新内容合并成一节 `## [0.2.0] - YYYY-MM-DD`：每个标题（例如 `### Added`）及其下每个主题（例如 `#### Model API`）中的条目分别合并在一起，最新的在前，预发布版本的各节及其链接则被移除。这些修改由 `scripts/changelog.mjs` 完成，GitHub 上的发布说明就是它写出的那一节。这样它的发布说明就列出了该版本自上一个正式版本以来的全部变更。

## 发布一个版本

1. **准备好 `dev`。** [完成标准](testing.md#完成变更之前)中的每项检查都通过，包括 `pnpm release:check`，`dev` 上的 CI 为绿色，并且 `CHANGELOG.md` 写明了该版本的所有变更。
2. **刷新模型数据**，让全新安装一开始就有最新的价格和模型，并把结果提交到 `dev`：

```bash
cargo xtask models-dev
```

3. **把 `dev` 推送到 `prod`。** 这是一次快进推送。如果推送被拒绝，说明 `dev` 还没有上一个版本：请先将 `prod` 合并到 `dev`，做法见第 5 步。

```bash
git switch dev
git pull --ff-only
git push origin dev:prod
```

4. **在 `prod` 上运行 Release 工作流**，并输入版本号，例如 `0.2.0` 或 `0.2.0-rc.1`；开头的 `v` 会被去掉。在仓库的 **Actions** 中选择 **Release**，然后选择 **Run workflow** 并选中分支 `prod`；也可以在终端中运行：

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
```

5. **确认 `dev` 已包含该版本。** 该次运行的 **Sync dev** 作业会把版本带回 `dev`。如果这个作业失败，版本本身已经发布完成，其摘要会说明原因；此时请手动将 `prod` 合并到 `dev`，解决冲突，用 `git commit --no-edit` 完成合并，然后推送 `dev`：

```bash
git fetch origin
git switch dev
git pull --ff-only
git merge --no-ff -m "Merge release 0.2.0 into dev" origin/prod
git push origin dev
```

6. **发布文档**：用 Pages 工作流和该版本的标签，例如 `v0.2.0`。发布预发布版本后这一步是可选的，因为站点随后描述的是该预发布版本。

```bash
gh workflow run pages.yml --repo logicplanes/kaeso --ref v0.2.0
```

## 预发布版本

预发布版本（例如 `0.2.0-rc.1`）和其他版本一样从 `prod` 发布。它在 npm 上使用 `next` 标签，在 GitHub 上被标记为预发布，镜像标签只有它的版本号，因此 `latest` 仍指向最新的稳定版本。之后 `prod` 上就是这个预发布版本，下一个版本是另一个预发布版本或正式版本。每个预发布版本在变更日志中都有自己的一节，直到正式版本把它们合并成一节（[变更日志](#变更日志)），因此正式版本不需要自己的新条目。

## 热修复

热修复只修正最新版本，不带上此后在 `dev` 上积累的内容。只要 `prod` 仍停留在该版本，就可以这样做。

1. **从该版本的标签创建分支**，例如为热修复 `0.2.1` 从 `v0.2.0` 创建：

```bash
git fetch origin --tags
git switch -c hotfix/0.2.1 v0.2.0
```

2. **修复问题**：先写一个能复现该缺陷的测试，在 `CHANGELOG.md` 的 `[Unreleased]` 下添加一行，提交，并运行各项检查。
3. **把该分支推送到 `prod`**，这是一次快进推送，因为 `prod` 停留在该版本的提交上：

```bash
git push origin hotfix/0.2.1:prod
```

4. **在 `prod` 上运行 Release 工作流**，并输入热修复的版本号：

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.1
```

**Sync dev** 会把热修复合并到 `dev`。如果 `dev` 的 `[Unreleased]` 下已有条目，`CHANGELOG.md` 会发生冲突：请按[发布一个版本](#发布一个版本)中的做法手动合并，保留 `dev` 在 `[Unreleased]` 下的条目，并把热修复的那一节放在它们下面。

## Release 工作流

`.github/workflows/release.yml` 依次运行 Validate、Prepare、Checks 和 Tag；标签推送后，Sync dev 和各发布作业随即开始。写入仓库或发布内容的作业都在 `release` 环境中运行，该环境只接受 `prod` 上的运行。只有 Tag 和 Sync dev 能够推送：它们不构建任何内容，只运行 git，并使用该次运行自己的令牌推送，因此它们的推送不会再次触发 CI。

| 作业 | 作用 |
| --- | --- |
| Validate | 拒绝在 `prod` 以外的分支上运行、不符合语义化版本的版本号、已存在的标签、不比最新标签更新的版本，以及 `[Unreleased]` 下没有内容、且既没有该版本一节也没有（对于正式版本而言）其预发布版本一节的 `CHANGELOG.md` |
| Prepare | 用 `cargo set-version` 设置每个 crate 的版本，并更新 `Cargo.lock`、`fuzz/Cargo.lock` 和每个 `package.json` 中的版本；重新生成 `openapi.json`；在 `deploy/compose.yaml` 中固定两个镜像；移动变更日志，并把正式版本的各预发布版本的节合并到它自己的一节中。它只能读取仓库，不持有任何 Git 凭据，并把这些修改作为补丁交给 Tag |
| Checks | 运行完整的 CI 工作流，无论改动了什么，所有作业都会运行 |
| Tag | 应用补丁，拒绝新建、删除或重命名文件的补丁，也拒绝修改 Prepare 所写文件以外任何文件的补丁。补丁修改了文件时，将“Release X.Y.Z”提交到 `prod`；没有修改任何文件时（例如 `0.1.0`），标签直接打在 `prod` 的提交上。如果 `prod` 在运行开始后发生了移动则中止；创建标签 `vX.Y.Z`，并将 `prod` 和标签一起推送，要么都成功，要么都不生效 |
| Sync dev | 把版本带回 `dev`：如果 `dev` 没有新提交，就快进；否则在能干净合并时创建合并提交“Merge release X.Y.Z into dev”。如果发生冲突、版本修改了 `dev` 没有的工作流文件，或 `dev` 在此期间发生了移动，它会失败，并在摘要中说明如何手动合并 |
| Image | 从标签为每个平台构建每个镜像，附带完整的构建来源证明和 SBOM |
| Image manifest | 将两个平台合并到上述镜像标签下，把镜像的 `org.opencontainers.image` 标签作为注解复制到合并后的索引上，并证明构建的来源 |
| Pack npm packages | 构建这些库，并用该版本号运行包检查 |
| npm | 发布 `@logicplanes/kaeso-tokens` 和 `@logicplanes/kaeso-ui` |
| crates.io | 发布 `kaeso-kpp` 和 `kaeso-codecs` |
| GitHub release | 下载已发布的 tarball 和 crate，并创建附带文件和发布说明的版本 |

npm 和 crates.io 作业通过可信发布（trusted publishing）进行身份验证，因此仓库中不保存任何注册表令牌。它们会跳过已经发布的版本，因此某个发布作业失败时，重新运行该次运行中失败的作业即可完成发布。

## Pages 工作流

`.github/workflows/pages.yml` 在要发布的版本的标签上运行（启动工作流时选择该标签作为 ref），不是从版本标签启动的运行会被拒绝。它会安装工作区，用 `pnpm build:packages` 构建这些库，用 `pnpm --filter kaeso-docs build` 构建本站（包括体积预算检查），并将 `apps/docs/dist` 部署到 GitHub Pages 上的 docs.kaeso.app。文档描述的是它发布时所基于的版本。
