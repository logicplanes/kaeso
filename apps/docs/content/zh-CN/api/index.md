---
title: API
description: 从应用中调用模型，并通过 HTTP 管理实例。
order: 4
---

每个实例都在其域名上提供两个 API。应用使用 API 密钥，通过 `/v1` 上的模型 API 调用模型；脚本通过 `/platform/v1` 上的 Platform API 管理实例。

- [模型 API](model-api.md)：用 OpenAI 和 Anthropic 的 SDK 调用模型，并查阅端点、请求头和模型名称。
- [错误](errors.md)：了解模型请求失败时在各格式下返回的内容及失败原因。
- [Platform API](platform-api.md)：查阅管理实例所需的身份验证、约定、错误和路由。
- [用 Platform API 实现自动化](automation.md)：通过脚本设置项目、模型和应用的密钥。
