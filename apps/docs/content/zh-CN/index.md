---
title: 简介
description: Kaeso 是一个自行部署的开源 AI 网关。
---

Kaeso 是一个自行部署的开源 AI 网关：应用只需对接一个 API，服务商密钥、路由和用量始终由自己掌控。

应用使用 OpenAI 或 Anthropic 的官方 SDK 和 Kaeso API 密钥调用 Kaeso。Kaeso 检查密钥、模型和限额，用自己的服务商密钥将请求发送给服务商，并按应用请求的格式返回回复。每个请求都会被记录和计数，因此可以看到谁使用了哪个模型、花费了多少。

初次使用？[快速开始](get-started/quick-start.md)用 Docker Compose 运行 Kaeso，几分钟内即可发送第一个请求。

本文档描述的是 Kaeso 0.1.0。

## 功能概览

- **所有应用共用一个 API。** 支持 OpenAI 的 Chat Completions、Responses 和 Embeddings 格式以及 Anthropic 的 Messages 格式，应用现有的 SDK 可以继续使用。Chat Completions、Responses 和 Messages 都可以调用目录中来自 OpenAI 或 Anthropic 的任意聊天模型。
- **服务商密钥留在自己手中。** 应用获得的是 Kaeso API 密钥（`sk-kso-…`），可以为其设置限额、到期时间，也可以停用。服务商密钥使用只有本安装持有的主密钥加密。
- **组织和项目。** 成员、角色和权限、服务账号，以及每个项目的模型目录和别名。
- **限额和预算。** 为项目、成员或单个密钥设置请求速率和 Token 速率、Token 预算和支出预算。
- **请求日志和用量。** 每个请求的状态、Token、费用和耗时，选择保留时还包括其内容。
- **控制台。** 上述所有功能的 Web 界面，提供英语、德语和简体中文版本。

## 各部分如何配合

```text
app ──▶ gateway ──▶ OpenAI or Anthropic
          ▲
console ──┘
```

| 部分 | 说明 |
| --- | --- |
| 网关 | 应用调用的服务器。它在 `/v1` 下提供模型 API，在 `/platform/v1` 下提供 Platform API，状态保存在 PostgreSQL 中，请求日志保存在 ClickHouse 中，计数器保存在 Valkey 中。 |
| 控制台 | Web 界面，与网关使用同一个域名。它只使用 Platform API，因此脚本可以完成它能做的所有事情。 |
| KPP | Kaeso Provider Protocol：一种模型请求的规范格式，以及在它与各 API 格式之间转换的编解码器。网关在每个请求中都会用到它。 |

[请求流程](concepts/request-flow.md)说明了请求经过网关时发生的事情，[格式转换](concepts/format-translation.md)说明了 KPP 如何处理请求。

## 从哪里开始

| 想要 | 阅读 |
| --- | --- |
| 运行 Kaeso 并发送第一个请求 | [快速开始](get-started/quick-start.md) |
| 在代码中调用模型 | [调用模型](get-started/calling-models.md) |
| 在 Claude Code 或 Codex 中使用 Kaeso | [编程工具](get-started/coding-tools.md) |
| 规划和运维一个安装 | [自托管](self-hosting/index.md) |
| 熟悉控制台 | [使用控制台](console/index.md) |
| 在代码中调用 API | [API](api/index.md) |
| 参与 Kaeso 本身的开发 | [开发](development/index.md) |

Kaeso 采用 MIT 许可证。网关、KPP、控制台、UI 组件库和本文档的全部源代码都在 GitHub 上的同一个仓库中：[logicplanes/kaeso](https://github.com/logicplanes/kaeso)。
