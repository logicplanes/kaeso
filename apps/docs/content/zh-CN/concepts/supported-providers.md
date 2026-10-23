---
title: 支持的服务商
description: 查阅 Kaeso 调用的每个服务商，包括其 ID、调用格式、主机以及密钥的获取位置。
order: 2
---

Kaeso 内置了以下服务商，按名称排列。如何使用服务商以及它们之间的差异，见[服务商](providers.md)。

## 各列说明

- **服务商。** 控制台显示的名称，后跟 ID。ID 用于在模型名称（如 `groq/llama-3.3-70b-versatile`）和[设置](../self-hosting/configuration.md#服务商设置)中指定服务商。
- **格式。** Kaeso 调用该服务商所用的 API 格式。模型使用第一种格式，除非其**发送格式**指定了另一种（[模型的参数](../console/models-and-aliases.md#更改模型的参数)）。**Embeddings** 表示该服务商提供嵌入模型，可用于 `/v1/embeddings`。**Token 计数**表示它可以为 `/v1/messages/count_tokens` 和 `/v1/responses/input_tokens` 计算 Token 数。
- **主机。** 服务商默认基础 URL 的主机，实例必须能够访问它。如果您[设置了基础 URL](../self-hosting/configuration.md#服务商设置)，则以该地址为准。
- **密钥。** 在哪里创建密钥，以及密钥的开头（[服务商密钥](../console/provider-keys.md)）。

## 服务商列表

| 服务商 | 格式 | 主机 | 密钥 |
| --- | --- | --- | --- |
| Anthropic `anthropic` | Messages、Token 计数 | `api.anthropic.com` | [platform.claude.com](https://platform.claude.com/settings/keys)，`sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com/api_keys)，`sk-` |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com](https://console.groq.com/keys)，`gsk_` |
| Mistral `mistral` | Chat Completions、Embeddings | `api.mistral.ai` | [console.mistral.ai](https://console.mistral.ai/api-keys)，无固定前缀 |
| OpenAI `openai` | Responses、Chat Completions、Embeddings、Token 计数 | `api.openai.com` | [platform.openai.com](https://platform.openai.com/api-keys)，`sk-` |
| OpenRouter `openrouter` | Chat Completions、Embeddings | `openrouter.ai` | [openrouter.ai](https://openrouter.ai/settings/keys)，`sk-or-v1-` |
| SpaceX `xai` | Responses、Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai/team/default/api-keys)，`xai-` |
| Together AI `togetherai` | Chat Completions、Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys)，`tgp_v1_` |

## 补充说明

关于部分服务商的其他须知事项。服务商会改名或拒绝的每个 Chat Completions 字段，见[服务商配置](../development/kpp/openai-chat.md#服务商配置)：

- **DeepSeek。** `developer` 消息会作为 `system` 消息发送给它。
- **Groq。** `developer` 消息会作为 `system` 消息发送给它；它拒绝部分字段，例如 `logit_bias`。
- **Mistral。** 部分字段以它自己的名称发送，例如 `seed` 改为 `random_seed`；它拒绝其他一些字段（例如 `logit_bias`），也拒绝以助手消息结尾的对话。
- **OpenAI。** 服务商密钥也可以是一个 ChatGPT 账户，其请求按 OpenAI 的相应规则使用该账户的 ChatGPT 套餐（[使用 ChatGPT 账户登录](../console/provider-keys.md#使用-chatgpt-账户登录)）。
- **OpenRouter。** 模型 ID 包含模型开发者，例如 `anthropic/claude-sonnet-4.5`。请以 `openrouter/anthropic/claude-sonnet-4.5` 的形式调用，这样请求不会发往该开发者自己的服务商（[请求如何指定模型](../console/models-and-aliases.md#请求如何指定模型)）。Kaeso 按目录中的价格记录费用，而不是按 OpenRouter 在 `usage.cost` 中报告的费用。OpenRouter 列出模型时不检查密钥，因此**检查可用性**无法发现错误的密钥；使用该密钥的第一个请求会报告密钥被拒绝。
- **SpaceX。** 即 xAI 的 API，提供 Grok 等模型。作为实验性选项，服务商密钥也可以是套餐包含 API 访问权限的 Grok 账户（[使用 Grok 账户登录](../console/provider-keys.md#使用-grok-账户登录)）。
