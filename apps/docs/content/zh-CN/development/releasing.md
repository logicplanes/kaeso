---
title: 发布
description: 版本如何编号、一个版本发布哪些内容，以及维护者如何发布版本。
order: 9
---

Kaeso 作为一个整体发布：网关、控制台、KPP 的 crate 和 UI 库的包共用同一个版本号，一次发布就会发布所有这些内容。贡献者从不创建标签或版本；每个版本都由维护者通过仓库的 Release 工作流发布。

## 版本号

版本号遵循[语义化版本](https://semver.org)。如果某个版本需要运行 Kaeso 的人手动执行某个步骤，例如一个无法自动运行的迁移，它就是一个主版本。版本的 Git 标签是 `v` 加上版本号，例如 `v0.1.0`。

同一个版本的网关和控制台始终配套使用：该版本的 `compose.yaml` 将两个镜像都固定在这个版本，控制台也只适配其对应网关的 Platform API。

## 一个版本发布什么

| 位置 | 内容 |
| --- | --- |
| GitHub releases | 版本 `vX.Y.Z`，附带来自 `CHANGELOG.md` 的发布说明和文件 `compose.yaml`，即[安装](../self-hosting/installation.md)时下载的文件 |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` 和 `ghcr.io/logicplanes/kaeso-console`，以版本号作为标签 |
| crates.io | `kaeso-kpp` 和 `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens`，通过 pnpm 发布 |

docs.kaeso.app 上的文档和 ui.kaeso.app 上的展示站点从同一个仓库发布，描述的是最新版本。

## 变更日志

仓库根目录下的 `CHANGELOG.md` 遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)。每个会被人注意到的变更都要在同一个拉取请求中于 `[Unreleased]` 下添加一行：从使用、运行 Kaeso 或基于 Kaeso 构建的人的角度，写明变更了什么。发布时，`[Unreleased]` 会变为该版本号及其日期，这一节就成为该版本的发布说明。`[Unreleased]` 下没有内容的发布会被拒绝。

## 发布一个版本

1. **准备好 `dev`。** [完成标准](testing.md#完成变更之前)中的每项检查都通过，并且 `CHANGELOG.md` 写明了该版本的所有变更。
2. **刷新模型数据**，让全新安装一开始就有最新的价格和模型，并提交结果：

```bash
cargo xtask models-dev
```

3. **检查包。** 运行 `pnpm build` 之后，`pnpm release:check` 会打包 npm 包并检查其内容，例如字体许可证文件。
4. **运行 Release 工作流。** 在仓库的 **Actions** 中选择 **Release**，然后选择 **Run workflow**，并输入版本号，例如 `0.2.0`。它必须比上一个版本更新。

工作流会再次运行检查，设置版本号，为版本打标签，并发布上表中的所有内容。完成之后，该版本的 `compose.yaml` 就是[升级](../self-hosting/upgrades.md)时下载的文件。
