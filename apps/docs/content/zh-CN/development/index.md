---
title: 开发
description: Kaeso 的源代码如何组织，以及如何构建、测试和修改它。
order: 6
---

Kaeso 在一个仓库中公开开发：[github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso)。网关、KPP、控制台、UI 库和本文档都在其中，并以同一个版本号一起发布。这些页面面向参与开发 Kaeso 本身的人。

初次接触？[本地环境搭建](local-setup.md)会带你从全新克隆的代码开始，得到一个带有示例数据、测试全部通过的运行中的安装。

## 仓库

| 路径 | 内容 | 技术栈 |
| --- | --- | --- |
| `crates/` | Cargo 工作区：网关（`kaeso` 和各个 `kaeso-*` crate）、KPP（`kaeso-kpp`、`kaeso-codecs`）、SDK 一致性测试服务器（`kpp-conformance`）以及仓库任务（`xtask`） | Rust |
| `apps/console` | Web 控制台 | TypeScript、React |
| `apps/docs` | 本文档，部署在 docs.kaeso.app | Markdown、TypeScript |
| `apps/showcase` | UI 库的展示站点，部署在 [ui.kaeso.app](https://ui.kaeso.app) | Astro、React |
| `packages/ui`、`packages/tokens` | 组件库和设计令牌，所有界面都基于它们构建 | TypeScript、React |
| `e2e/` | 使用官方 OpenAI 和 Anthropic Python SDK 的端到端测试，以及安装测试 | Python |
| `fuzz/` | 每个 KPP 解码器的模糊测试目标 | Rust |
| `deploy/` | Dockerfile、生产环境的 `compose.yaml` 和开发环境栈 | Docker |
| `openapi.json` | Platform API 文档，由网关的代码生成 | |

Cargo 构建 Rust 工作区，pnpm 构建 TypeScript 工作区（`pnpm-workspace.yaml`）；两者都在仓库根目录下运行。

## 一个版本发布什么

| 产物 | 构建来源 |
| --- | --- |
| 镜像 `ghcr.io/logicplanes/kaeso-gateway` | `deploy/gateway.Dockerfile` |
| 镜像 `ghcr.io/logicplanes/kaeso-console` | `deploy/console.Dockerfile` |
| `compose.yaml`，附在 GitHub 版本中 | `deploy/compose.yaml`，固定了该版本的两个镜像 |
| crates.io 上的 crate `kaeso-kpp` 和 `kaeso-codecs` | `crates/kaeso-kpp`、`crates/kaeso-codecs` |
| npm 包 `@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens` | `packages/ui`、`packages/tokens` |

文档和展示站点以网站形式发布。[发布](releasing.md)介绍了如何发布一个版本。

## 页面

| 页面 | 内容 |
| --- | --- |
| [本地环境搭建](local-setup.md) | 前置条件、开发环境栈、示例数据，以及如何从源代码运行每个部分 |
| [架构](architecture/index.md) | 各部分如何配合：网关、控制台、文档站点和邮件 |
| [KPP 内部实现](kpp/index.md) | 规范格式、编解码器，以及逐字段说明的每种 wire 格式 |
| [测试](testing.md) | 每个测试套件、如何运行，以及完成变更之前必须通过哪些检查 |
| [翻译](translations.md) | 控制台、UI 库、邮件和文档如何支持英语、德语和中文 |
| [编写文档](documentation.md) | 本站的页面如何编写、链接和翻译 |
| [依赖与许可证](dependencies.md) | 每个依赖、引入它的原因，以及许可证政策 |
| [性能](performance.md) | 打包体积预算、编解码器基准测试和模糊测试结果 |
| [发布](releasing.md) | 版本号、发布工作流及其发布的内容 |
| [参与贡献](contributing.md) | 问题、拉取请求，以及每个变更都要遵守的规则 |
