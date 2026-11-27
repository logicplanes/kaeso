---
title: 跨格式的采样参数
nav: 采样参数
description: 每种 API 格式接受哪些采样设置，以及 Kaeso 因其不提出任何要求而省略的值。
order: 3
---

采样设置决定模型如何选词：温度、top P 和 top K、惩罚参数、种子、停止序列和输出上限。每种 API 格式接受的设置各不相同。本页列出目标会拒绝的设置，以及 Kaeso 因其不提出任何要求而改为省略的值。

## 目标拒绝的设置

目标无法遵守的采样设置为 `Unsupported`，从不丢弃或重新缩放，惩罚值为 0 时除外（[中性值](#中性值)）：

| 目标 | 拒绝 |
| --- | --- |
| Anthropic | `seed`、频率惩罚和存在惩罚，以及超出 Anthropic 取值范围 0 到 1 的 `temperature` |
| OpenAI Chat Completions | `top_k`，以及超过四个的停止序列 |
| OpenAI Responses | `top_k`、`seed`、各项惩罚和停止序列，Responses 没有这些参数；以及低于 16 个 Token 的输出上限，即 `CreateResponse` 的最小值；除 `top_k` 和各项惩罚外，网关都会处理（[网关中的 OpenAI Responses](#网关中的-openai-responses)） |
| Gemini API | 超过五个停止序列（`GenerationConfig.stopSequences`） |
| Amazon Bedrock Converse | `top_k`、`seed` 和惩罚参数，它们属于各模型系列自己的字段 |

数字在同一方言内保持原来的写法：只要值未改变，`temperature: 1` 回来时仍是 `1`，`1.0` 仍是 `1.0`。跨方言时使用目标自己的写法，因此来自 Chat 的 `temperature: 1` 到达 Anthropic 时是 `1.0`。

KPP 没有 `n`：`n` 大于 1 的 Chat 请求在解码时就会被拒绝，与目标无关。Chat 的 `logprobs`、`logit_bias` 和 `top_logprobs` 是 extras，其他每个方言都会拒绝它们（[Extras](cross-format.md#extras)），除非它们取[中性值](#中性值)。

## 网关中的 OpenAI Responses

Responses 编解码器会拒绝停止序列、`seed` 和低于 16 个 Token 的输出上限。当网关把其他格式的请求发给通过 Responses 提供的模型（例如 Grok）时，会改为由自己处理：

- **停止序列。** 请求不带停止序列发送，Kaeso 在回复文本中出现第一个停止序列之前自行结束回复。打开的块会结束，回复按应用格式的方式结束：`finish_reason: "stop"`，或带有 `stop_sequence` 的 `stop_reason: "stop_sequence"`。可能是停止序列开头的文本会等待下一个增量，因此分散在多个增量中的停止序列同样会结束回复。推理内容和工具参数不会结束回复。`kaeso-adapted` 会注明应用中的字段名，例如 `stop`。
- **低于 16 个 Token 的输出上限。** Kaeso 发送 16，并在应用的上限处以该格式的长度原因结束回复。对于上限不计推理的服务商（例如 xAI），推理内容不计入。`kaeso-adapted` 会注明应用中的字段名，例如 `max_tokens`。Responses 应用自己设置的低于 16 的上限仍会被拒绝。
- **`seed`。** 会被省略，并在 `kaeso-left-out` 中注明：OpenAI 只尽力而为地遵循它，而 Responses 没有对应字段。

Kaeso 会以流的方式读取这类回复，即使应用没有要求流式输出，这样 Kaeso 结束回复时服务商也会停止生成。Kaeso 根据已读取的内容估算用量，与 [ChatGPT 套餐的输出上限](openai-responses.md#chatgpt-套餐用量)相同。

## 中性值

NextChat 等聊天应用在每个请求中都会发送 OpenAI 的采样字段，取值就是 API 在没有该字段时使用的值。这样的值不提出任何要求。若目标或其提供商的配置会拒绝该字段，Kaeso 会改为省略它，并按应用的写法在 `kaeso-left-out` 头中列出。其他取值仍像以前一样被拒绝。这些值见 `cross::NEUTRAL_VALUES`，另加两个惩罚参数。

| 字段 | 值 | 对谁省略 |
| --- | --- | --- |
| `frequency_penalty`、`presence_penalty`，Gemini 应用的 `generationConfig.frequencyPenalty` 和 `.presencePenalty` | `0` | Anthropic、Responses、Amazon Bedrock Converse，以及拒绝它们的 Chat 提供商 |
| `logprobs` | `false` | 其他所有格式，以及拒绝它的 Chat 提供商 |
| `top_logprobs` | `0` | 同上 |
| `logit_bias`、`metadata` | `{}` | 同上 |
| `store` | `false` | 同上；Responses 目标会收到 Kaeso 自己的 `store: false`，因此那里不列出 |
| `service_tier`，来自 Chat 和 Anthropic | `"auto"` | 同上，以及拒绝它的 Anthropic 提供商 |
| `verbosity` | `"medium"` | 同上 |
| `modalities` | `["text"]` | 拒绝它的 Chat 提供商；其他格式本来就输出文本 |

每个值都是该字段在 OpenAI 的 OpenAPI 文档和 Anthropic 的 API 参考中的 `default`；空的 `logit_bias` 或 `metadata` 与其默认值 `null` 含义相同。`service_tier: "default"` 不是中性值：它要求标准处理，而项目设置可能会选择其他层级。`web_search_options: {}` 也不是：它会让搜索模型进行搜索。以 `null` 发送的字段属于 form，其他格式永远不会收到；拒绝该字段的 Chat 提供商也不会收到这个 `null`。

Fixtures：`crates/kaeso-codecs/fixtures/openai_chat/requests/nextchat-defaults.json`（NextChat 的请求）、`crates/kaeso-codecs/fixtures/openai_chat/requests/neutral-defaults.json`。
