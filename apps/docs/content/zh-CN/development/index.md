---
title: 开发
description: Kaeso 的源代码如何组织，以及如何构建、测试和修改它。
order: 6
---

这些页面面向参与开发 Kaeso 本身的人。所有内容都在同一个仓库 [github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso) 中，并以同一个版本号一起发布。要得到一个可运行的检出副本，请从[本地环境搭建](local-setup.md)开始。

## 仓库

| 路径 | 内容 | 构建工具 |
| --- | --- | --- |
| `crates/` | 网关（`kaeso` 及各个 `kaeso-*` crate）、KPP（`kaeso-kpp`、`kaeso-codecs`）、SDK 一致性服务器（`kpp-conformance`）和仓库任务（`xtask`） | Rust、Cargo |
| `apps/console` | Web 控制台 | TypeScript、React |
| `apps/docs` | 本文档，部署在 docs.kaeso.app | Markdown、TypeScript |
| `apps/showcase` | UI 库的展示站点 ui.kaeso.app：每个组件的每种状态 | Astro、React |
| `packages/ui`、`packages/tokens` | 组件库和设计令牌，所有界面都基于它们构建 | TypeScript、React |
| `e2e/` | 用 OpenAI 和 Anthropic 的官方 Python SDK 测试 KPP、网关和全新安装 | Python |
| `fuzz/` | 每个 KPP 解码器的模糊测试目标 | Rust |
| `deploy/` | Dockerfile、生产用的 `compose.yaml` 和开发环境栈 | Docker |
| `openapi.json` | Platform API 文档，由网关代码生成 | |

Cargo 构建 Rust 工作区，pnpm 构建 TypeScript 工作区（`pnpm-workspace.yaml`）。两者都在仓库根目录下运行。

## 一个版本发布什么

| 产物 | 构建来源 |
| --- | --- |
| 镜像 `ghcr.io/logicplanes/kaeso-gateway` 和 `ghcr.io/logicplanes/kaeso-console` | `deploy/gateway.Dockerfile`、`deploy/console.Dockerfile` |
| 附在 GitHub 版本上的 `compose.yaml`、`kaeso.example.toml` 和 `env.example` | 两个镜像都固定为该版本的 `deploy/compose.yaml`、`deploy/kaeso.example.toml`、`deploy/.env.example` |
| crates.io 上的 crate `kaeso-kpp` 和 `kaeso-codecs` | `crates/kaeso-kpp`、`crates/kaeso-codecs` |
| npm 包 `@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens` | `packages/ui`、`packages/tokens` |

文档和展示站点从该版本的标签构建，作为静态网站部署。完整流程见[发布](releasing.md)。

## 页面

| 页面 | 内容 |
| --- | --- |
| [参与贡献](contributing.md) | Issue、拉取请求，以及每个变更都要遵守的规则 |
| [本地环境搭建](local-setup.md) | 前置条件、开发环境栈、示例数据，以及从源代码运行各个部分 |
| [架构](architecture/index.md) | 网关、控制台、文档站点和邮件如何协同工作 |
| [KPP 内部实现](kpp/index.md) | 规范格式、编解码器，以及逐字段说明的每种 wire 格式 |
| [测试](testing.md) | 每个测试套件、如何运行，以及 CI 的要求 |
| [翻译](translations.md) | 每个界面如何使用英语、德语和简体中文，以及术语表 |
| [编写文档](documentation.md) | 本站页面如何编写、链接和翻译 |
| [依赖与许可证](dependencies.md) | 每个直接依赖、引入它的原因，以及许可证政策 |
| [性能](performance.md) | 打包预算、编解码器基准测试和模糊测试结果 |
| [发布](releasing.md) | 版本号、变更日志、Release 工作流及其发布的内容 |
