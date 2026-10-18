---
title: 开发
description: 构建、测试和修改 Kaeso，从全新克隆到合并拉取请求。
order: 6
---

这些页面面向修改 Kaeso 本身的人。所有内容都在同一个仓库 [logicplanes/kaeso](https://github.com/logicplanes/kaeso) 中，并以同一个版本一起发布。

| 路径 | 内容 | 构建方式 |
| --- | --- | --- |
| `crates/` | 网关、更新器、KPP（`kaeso-kpp`、`kaeso-codecs`）、SDK 一致性服务器和仓库任务（`xtask`） | Rust、Cargo |
| `apps/console` | Web 控制台 | TypeScript、React |
| `apps/docs` | 本文档 | Markdown、TypeScript |
| `apps/showcase` | UI 库的展示站点 ui.kaeso.app | Astro、React |
| `apps/update` | 更新服务 update.kaeso.app：实例读取的版本源、镜像的版本清单、版本配图和版本页面 | TypeScript、React |
| `packages/ui`、`packages/tokens` | 组件库和设计令牌，所有界面都由它们构建 | TypeScript、React |
| `e2e/` | 用 OpenAI 和 Anthropic 的官方 Python SDK 测试 KPP、网关和全新实例 | Python |
| `fuzz/` | 每个 KPP 解码器的模糊测试目标 | Rust |
| `deploy/` | Dockerfile、自托管用的 `compose.yaml` 和开发栈 | Docker |
| `openapi.json` | Platform API 文档，由网关代码生成 | |

Cargo 构建 Rust 工作区，pnpm 构建 TypeScript 工作区，两者都在仓库根目录运行。

- [本地环境搭建](local-setup.md)：从全新克隆到带示例数据的控制台，并让测试全部通过。
- [参与贡献](contributing.md)：报告问题、提出变更，并让拉取请求顺利合并。
- [开发栈](stack.md)：更改端口、运行第二个栈，或从源码运行网关。
- [测试](testing.md)：运行变更必须通过的检查，并找到覆盖代码的测试套件。
- [翻译](translations.md)：用英语、德语和简体中文写出每一个词。
- [编写文档](documentation.md)：编写和翻译本站的页面。
- [架构](architecture/index.md)：了解网关、控制台、文档和更新器的构建方式。
- [KPP 内部实现](kpp/index.md)：修改在各 API 格式之间转换的编解码器。
- [发布](releasing.md)：确定版本号，发布并部署新版本。
