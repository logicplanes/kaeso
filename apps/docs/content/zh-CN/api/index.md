---
title: API
description: 从应用中调用模型，并通过 HTTP 管理实例。
order: 4
---

每个实例都在其域名上提供两个 API。应用使用 API 密钥，通过 `/v1`、`/v1beta` 和 `/v2` 上的模型 API 调用模型；脚本通过 `/platform/v1` 上的 Platform API 管理实例。智能体通过 `/mcp` 上的 MCP 服务器访问 Platform API，客户端通过 `/mcp/<项目 ID>` 上的 MCP 门户调用项目的工具。[各服务商的 API](../concepts/provider-apis.md) 列出每个模型 API 能调用哪些服务商。

## 下一步

- [模型 API](model-api.md)：用 OpenAI、Anthropic 和 Google Gen AI 的 SDK 调用模型，并查阅端点、请求头和模型名称。
- [图片 API](images.md)：使用 OpenAI 的 Images API，在所有能生成图片的服务商上生成和编辑图片。
- [音频 API](audio.md)：使用 OpenAI 的音频 API，在所有提供该功能的服务商上生成语音、转写或翻译音频。
- [视频 API](videos.md)：使用 OpenAI 的 Videos API，在每个提供视频生成的服务商处生成视频。
- [决策 API](decisions.md)：以 OpenAI、TypeSafe 或 Gemini 的决策格式向决策模型提出类型化问题。
- [ElevenLabs API](elevenlabs.md)：使用 ElevenLabs 官方 SDK 调用 ElevenLabs 的语音、音效、音乐和转写。
- [错误](errors.md)：了解模型请求失败时在各格式下返回的内容及失败原因。
- [Platform API](platform-api.md)：查阅管理实例所需的身份验证、约定、错误和路由。
- [用 Platform API 实现自动化](automation.md)：通过脚本设置项目、模型和应用的密钥。
- [MCP 服务器](mcp.md)：连接编程智能体，决定它们能做什么，并查阅其工具和登录方式。
- [MCP 门户 API](mcp-portal.md)：通过 MCP 调用项目的工具，并查阅门户的登录方式、版本和错误。
