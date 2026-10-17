---
title: API
description: Kaeso 部署提供的两个 API 及其调用方式。
order: 4
---

Kaeso 部署在其域名上提供两个 API：

| API | 路径 | 调用方 | 身份验证 |
| --- | --- | --- | --- |
| [模型 API](model-api.md) | `/v1` | 应用，通过 OpenAI 和 Anthropic 的官方 SDK | 具有模型调用权限的项目 API 密钥 |
| [Platform API](platform-api.md) | `/platform/v1` | 控制台、脚本和 CI，用于管理组织、项目、密钥、模型、限额和日志 | 控制台会话、具有平台访问权限的项目 API 密钥，或组织的平台密钥 |

[错误](errors.md)列出了模型请求失败时在各格式下返回的内容。Platform API 的错误及其 OpenAPI 文档见[其页面](platform-api.md)。
