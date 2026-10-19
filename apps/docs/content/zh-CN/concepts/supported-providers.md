---
title: 支持的服务商
description: 查阅 Kaeso 调用的每个服务商，包括其 ID、Kaeso 调用它所用的格式，以及密钥的获取位置。
order: 2
---

Kaeso 内置了以下服务商，按名称排列。如何使用服务商以及它们之间的差异，见[服务商](providers.md)。

## 各列说明

- **服务商。** 控制台显示的名称，后跟 ID。ID 用于在模型名称（如 `groq/llama-3.3-70b-versatile`）和[设置](../self-hosting/configuration.md#服务商设置)中指定服务商。
- **格式。** Kaeso 调用该服务商所用的 API 格式。模型使用第一种格式，除非其**发送格式**指定了另一种（[模型的参数](../console/models-and-aliases.md#更改模型的参数)）。**Embeddings** 表示该服务商提供嵌入模型，可用于 `/v1/embeddings`。**Token 计数**表示它可以为 `/v1/messages/count_tokens` 和 `/v1/responses/input_tokens` 计算 Token 数。
- **密钥。** 在哪里创建密钥，以及密钥的开头（[服务商密钥](../console/provider-keys.md)）。

## 服务商列表

| 服务商 | 格式 | 密钥 |
| --- | --- | --- |
| Anthropic `anthropic` | Messages、Token 计数 | console.anthropic.com，`sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | platform.deepseek.com，`sk-` |
| Groq `groq` | Chat Completions | console.groq.com，`gsk_` |
| Mistral `mistral` | Chat Completions、Embeddings | console.mistral.ai |
| OpenAI `openai` | Responses、Chat Completions、Embeddings、Token 计数 | platform.openai.com，`sk-` |
| OpenRouter `openrouter` | Chat Completions、Embeddings | openrouter.ai，`sk-or-v1-` |
| SpaceX `xai` | Responses、Chat Completions | console.x.ai，`xai-` |
| Together AI `togetherai` | Chat Completions、Embeddings | api.together.ai，`tgp_v1_` |

## 补充说明

关于部分服务商的其他须知事项：

- **Mistral。** 对于 Mistral 不接受的部分 Chat Completions 字段，Kaeso 会拒绝请求（[服务商配置](../development/kpp/openai-chat.md)）。
- **OpenRouter。** 模型 ID 包含模型开发者，例如 `anthropic/claude-sonnet-4.5`。调用时请在前面加上服务商，即 `openrouter/anthropic/claude-sonnet-4.5`，这样请求不会发往该开发者自己的服务商。
- **SpaceX。** 即 xAI 的 API，提供 Grok 等模型。
