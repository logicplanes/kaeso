---
title: 发布
description: 版本号和变更日志的规则，以及维护者如何制作、发布和部署一个版本。
order: 11
---

Kaeso 作为一个整体发布。网关、控制台、KPP 的 crate 和 UI 库的软件包共用一个版本号，Release 工作流运行一次就会发布全部内容。只有维护者制作版本；贡献者从不创建标签或版本。

## 版本号

版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。需要运行 Kaeso 的人手动操作的版本（例如无法自动运行的迁移）是主版本。预发布版本会加上后缀，例如 `0.2.0-rc.1`。Git 标签是 `v` 加版本号，例如 `v0.2.0`。

同一版本的网关和控制台是配套的。该版本的 `compose.yaml` 把两个镜像固定在这个版本以及该版本构建出的摘要，控制台也只认它所属网关的 Platform API。

## 仓库中的版本号

根目录的 `package.json` 保存版本号，只有发布才会修改它，使用的命令是 `node scripts/version.mjs set <version>`。这条命令把版本号写入每个必须写明它的文件，例如其他 `package.json` 文件、各 crate 的清单、`openapi.json` 和 `deploy/compose.yaml`。`node scripts/version.mjs files` 会列出这些文件。

其他地方都读取版本号，而不是写死它。页面写 `\{{version}}`，网关读取 `env!("CARGO_PKG_VERSION")`，生成的文件读取各自的 `package.json`。`node scripts/version.mjs check` 是 `pnpm test` 的一部分，只要当前版本号出现在其他任何地方就会失败，并指出文件和行号。

## 分支

| 分支 | 说明 | 谁来修改 |
| --- | --- | --- |
| `dev` | 默认分支，所有工作都合并到这里 | 只能通过拉取请求（[合并](contributing.md#合并)） |
| `prod` | 最新版本运行时所在的提交，或即将成为新版本的提交 | 维护者，始终以快进方式 |
| `hotfix/X.Y.Z` | 针对最新版本的修复，从其标签创建 | 维护者，并由其推送到 `prod` |

`dev` 和 `prod` 不能删除或强制推送，只有维护者能推送到 `prod`，`v*` 标签不能移动或删除。

## 变更日志

`CHANGELOG.md` 遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，拉取请求从不修改它。Release 工作流根据上一个版本以来合并的拉取请求写出每个版本的分节。每个拉取请求合并时的标题成为它的那一行，并附上拉取请求的链接，标签决定这一行放在哪里。合并时的标题就是该拉取请求在 `prod` 上的 squash 提交的主题行，因此合并后再编辑拉取请求不会改变任何内容。对于以其他方式合并的拉取请求，则取合并那一刻的标题。

带 `internal` 标签的拉取请求会被省略。Dependabot 的更新合并为一行“Dependencies updated”。带 `breaking` 标签的拉取请求还会在分节顶部单独列出。

要自己撰写版本说明，在 `dev` 上的 `CHANGELOG.md` 中把内容写在 `[Unreleased]` 下。发布时会先列出这些行，再为每个没有被其中任何一行以 `[#42](…/pull/42)` 形式链接的拉取请求添加一行。这适合说明需要比标题更详细的版本，也适合热修复，因为热修复的提交没有拉取请求。

如果仓库的 `release` 环境配置了机密 `LOG_AI_KEY`，工作流会通过维护者的 Kaeso 实例让模型根据已合并的拉取请求起草这一分节。模型按合并时的样子读取每个拉取请求：squash 提交的主题行和正文，或以其他方式合并的拉取请求的各个提交。只有当关联议题的作者对仓库有写权限时，模型才会看到该议题的标题。它会原样保留 `[Unreleased]` 下的内容。如果草稿未通过检查或请求失败，发布会改用标题。要在不修改任何文件的情况下预览下一个版本的说明：

```bash
pnpm release:notes --dry-run --since v{{version}} --head origin/dev
```

在同一版本的预发布之后发布稳定版本时，会把预发布的各分节合并为一个，因此稳定版本的说明涵盖自上一个稳定版本以来的全部内容。

GitHub 发布页显示该分节的简短版本：一句话、最多五个亮点、破坏性变更、贡献者，以及指向完整更新日志和比较视图的链接。模型在撰写分节时一并写出它。没有模型时，发布页列出该分节的前五行，每行只保留第一句。上面的预览会在说明之后输出 GitHub 发布页的内容。

### 说明中可以写什么

说明中的每一行，无论来自模型还是标题，都要通过这些检查：

- 不含地址：没有 URL、电子邮件地址或 IP 地址。
- 除 `kaeso.app` 及其子域名、`github.com/logicplanes/kaeso` 和 `ghcr.io/logicplanes` 上的 Kaeso 镜像外，不含主机名。`crates.io` 和 `models.dev` 可以不带路径出现。以常见顶级域名（例如 `.com` 或 `.example`）结尾的名称算作主机名，在代码片段中也一样。
- 不含 `| sh` 这样的 shell 管道，也不含 `curl -fsSL` 或 `wget` 这样的下载命令。

经过一次修正后仍未通过的草稿会改用标题。未通过检查的标题会让 **Prepare** 任务停止，并指出对应的拉取请求。请在 `dev` 上的 `[Unreleased]` 下为它写一行并链接该拉取请求，然后重新运行 Release。

### 打标签前的审阅

每个使用机密、写入仓库或发布内容的任务都在 `release` 环境中运行，因此都要等待维护者批准。批准 **Release notes** 后，模型开始起草。随后 **Prepare** 写出变更日志，并在其摘要中显示该分节和 GitHub 发布页，内容与发布时完全一致。之后 **Tag** 等待您的批准，所以请先阅读摘要。如果其中有任何问题，请拒绝 **Tag**，在 `dev` 上修正原因，然后重新运行 Release。

### 标签

| 标签 | 在变更日志中 |
| --- | --- |
| `added`、`changed`、`deprecated`、`removed`、`fixed`、`security` | 所在分节，例如 Added。没有类型标签时由标题的第一个词决定：Add、Fix、Remove 等，其他词归入 Changed |
| `breaking` | 还会作为破坏性变更列在顶部 |
| `internal` | 省略 |
| `area: gateway`、`area: kpp`、`area: console`、`area: ui library`、`area: documentation`、`area: self-hosting` | 分节中的主题。只有恰好一个领域标签时才会设置主题 |

**Labels** 工作流在每次推送时按 `.github/labeler.yml` 根据改动的文件设置领域标签和 `internal`。手动设置的标签只保留到下一次推送。当标题的第一个词选不中正确的分节时，维护者会设置类型标签；当升级需要使用者做出改动时，维护者会设置 `breaking`。

## 制作一个版本

1. 确认 `dev` 已就绪：CI 为绿色，每个已合并的拉取请求都有它那一行所需的标签，并且[完成变更之前](testing.md#完成变更之前)列出的检查全部通过。
2. 刷新模型数据，让全新实例以最新的模型和价格启动，并通过拉取请求把结果合并到 `dev`：

   ```bash
   cargo xtask models-dev
   ```

3. 以快进方式把 `dev` 推送到 `prod`：

   ```bash
   git switch dev
   git pull --ff-only
   git push origin dev:prod
   ```

4. 在 `prod` 上用版本号运行 Release 工作流。如果版本需要手动步骤，在工作流的输入中勾选 **manual**，这样更新器永远不会安装它：

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
   ```

   运行中的任务等待批准时，逐一批准。批准 **Tag** 之前，先阅读 **Prepare** 摘要中的说明（[打标签前的审阅](#打标签前的审阅)）。

5. 把版本合并回 `dev`。运行中的 **Sync dev** 作业会尝试这样做，但 `dev` 只接受拉取请求，所以它通常以警告结束，并给出需要运行的命令。手动合并即可，维护者对 `dev` 规则的豁免允许这样做：

   ```bash
   git fetch origin --tags
   git switch dev
   git pull --ff-only
   git merge --no-ff -m "Merge release 0.2.0 into dev" v0.2.0
   git push origin dev
   ```

6. 从该版本的标签部署网站（[部署网站](#部署网站)）。

如果运行因为 `prod` 还没有上一个版本而停止，先像第 5 步那样把它的标签合并到 `dev`，再重复第 3 步。如果某个发布作业失败，重新运行这次运行中失败的作业即可。每个作业都会跳过已经发布的内容；GitHub 版本作业会替换它已创建的版本中的资产和说明。

一个版本会发布：

| 位置 | 内容 |
| --- | --- |
| GitHub Releases | `vX.Y.Z` 及其说明，以及 `compose.yaml`、`kaeso.example.toml`、`env.example`、`release.json`、npm 压缩包和 `.crate` 文件 |
| GitHub Container Registry | 面向 `linux/amd64` 和 `linux/arm64` 的 `kaeso-gateway`、`kaeso-console` 和 `kaeso-updater`，带构建来源证明和软件物料清单（SBOM） |
| crates.io | `kaeso-kpp` 和 `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens` |

镜像以版本号作为标签；稳定版本还会打上 `MAJOR.MINOR` 和 `latest`，从 1.0.0 起还有 `MAJOR`。

### 提交与证明

运行从 `prod` 的提交开始。版本需要变化时，Tag 作业会在其上提交 `Release X.Y.Z`，并把这个提交标记为 `vX.Y.Z`。这个版本提交只修改 `CHANGELOG.md` 和 `node scripts/version.mjs files` 列出的文件。提交之前，Tag 作业会检查除 `CHANGELOG.md` 外的这些修改，与 `node scripts/version.mjs set` 在单独作业中写出的内容完全相同。

Tag 作业只推送标签：`prod` 会在下一次从 `dev` 推送时得到这个版本提交，因为 `dev` 在第 5 步中已经合并了它。每个镜像、软件包和资产都从该标签构建。

`gh attestation verify` 和更新器会检查镜像和 `release.json` 的证明。这些证明指明运行开始时的提交，因为 GitHub 是为这次运行签名的；`release.json` 也指明同一个提交。每个镜像内由 BuildKit 写入的构建来源，则把标签的提交指明为来源。因此，标签的提交要么就是被证明的提交，要么是紧挨其上的版本提交，且只修改版本号和更新日志。

## 制作预发布版本

在 `prod` 上用 `0.2.0-rc.1` 这样的版本号运行 Release 工作流。预发布版本在 npm 上使用 `next` 标签，镜像只使用其版本号作为标签，GitHub 也从不把它标记为最新版本。之后是否部署网站可自行决定。

## 制作热修复

热修复只修复最新版本，不包含 `dev` 上此后积累的内容。只要 `prod` 仍停留在该版本运行时所在的提交，就可以这样做。

1. 从该版本的标签创建分支：

   ```bash
   git fetch origin --tags
   git switch -c hotfix/0.2.1 v0.2.0
   ```

2. 用一个能重现缺陷的测试修复它，并在 `CHANGELOG.md` 的 `[Unreleased]` 下添加一行。这一行就是热修复的变更日志。
3. 把分支推送到 `prod`：

   ```bash
   git push origin hotfix/0.2.1:prod
   ```

4. 在 `prod` 上用热修复的版本号运行 Release 工作流。
5. 像[制作一个版本](#制作一个版本)中那样把热修复合并回 `dev`。如果 `CHANGELOG.md` 冲突，保留 `dev` 在 `[Unreleased]` 下的内容，并把热修复的分节放在其下方。

## 部署网站

docs.kaeso.app 上的文档和 ui.kaeso.app 上的展示站点都是静态网站，从版本的标签构建：

```bash
git fetch origin --tags
git checkout v0.2.0
pnpm install --frozen-lockfile && pnpm build:packages
pnpm --filter kaeso-docs build
pnpm --filter kaeso-showcase build
```

把 `apps/docs/dist` 和 `apps/showcase/dist` 原样上传。它们包含每种语言的每个页面、`robots.txt`、`sitemap.xml`，以及每种语言一个 `404.html`。托管服务需要用 `console.html` 响应 `/console`，并用对应语言的 `404.html` 响应未知地址。移动过的页面在旧地址上有一个跳转页面，`_redirects` 也为支持该文件的托管服务列出了相同的跳转。

### 内容安全策略

每次构建都会把网站的策略作为 `<meta>` 标签写入每个页面，也写入 `csp.txt` 和 `_headers`。还要把它作为 `Content-Security-Policy` 响应头发送，因为只有响应头才能加上 `frame-ancestors 'none'`。文档的响应头始终不变；它允许来自 `update.kaeso.app` 的图片，更新日志中各版本的配图就来自那里：

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: https://update.kaeso.app; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

展示站点的策略为它的每个内联脚本和样式列出一个哈希值，这些哈希会随它们变化。从要部署的那次构建的 `csp.txt` 中获取它的响应头。
