---
title: API
description: Kaeso 部署提供的两个 API 及其调用方式。
order: 4
---

Kaeso 部署在其域名下提供两个 API：

| API | 路径 | 用途 | 身份验证 |
| --- | --- | --- | --- |
| 模型 API | `/v1` | 供应用以 OpenAI 和 Anthropic 格式调用模型 | 具有模型调用权限的项目 API 密钥 |
| Platform API | `/platform/v1` | 供控制台和脚本管理组织、项目、密钥、模型、限额和日志 | 会话、具有平台访问权限的项目 API 密钥，或组织平台密钥 |

| 页面 | 内容 |
| --- | --- |
| [模型 API](model-api.md) | 端点、Base URL、密钥、模型名称、流式传输、响应头 |
| [错误](errors.md) | 模型请求失败时在各格式下返回的内容 |
| [Platform API](platform-api.md) | 约定、身份验证、错误、使用 curl 的完整示例、功能领域、角色和权限、试验场、OpenAPI 文档 |
