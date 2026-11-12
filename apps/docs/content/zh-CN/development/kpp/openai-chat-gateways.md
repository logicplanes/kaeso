---
title: Chat Completions 网关
description: 把 Chat Completions 转发给其他服务商的网关和路由器增加了什么，以及编解码器如何适配它们。
order: 6
nav: 网关
---

把请求转发给其他服务商的网关和路由器使用 [OpenAI Chat Completions](openai-chat.md)，但各有差异。在修改它们的配置之前，请先在这里查看每个网关增加的内容；它们的配置在[服务商配置](openai-chat-providers.md#服务商配置)表中，其他服务商见 [Chat Completions 服务商](openai-chat-providers.md)。

它们的 fixture 位于 `fixtures/openai_chat/providers/<id>/`（[fixtures](fixtures.md)）。

## OpenRouter

来源：OpenRouter 的 [API 参考](https://openrouter.ai/docs/api_reference/overview)、[流式传输](https://openrouter.ai/docs/api_reference/streaming)、[错误与调试](https://openrouter.ai/docs/api_reference/errors-and-debugging)和[推理 Token](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/openrouter/` 中的实时录制（均使用 `:free` 模型）。

| OpenRouter 字段 | KPP | 说明 |
|---|---|---|
| `provider`、`models`、`transforms`、`usage`、`reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。 |
| `message.reasoning_details`、`delta.reasoning_details` | extras | OpenRouter 结构化形式的同一推理；Chat 应用原样收回，其他方言从 `reasoning` 得到文本。 |
| 回复中的 `provider`、`native_finish_reason` | extras | 发往其他方言时省略。 |
| `usage.cost`、`is_byok`、`cost_details` 及额外的 Token 明细 | 用量的 `raw` | Kaeso 按目录中的价格计算费用，而不是按 `usage.cost`。 |
| 最后一个流式数据块，它在 `usage` 旁重复带 `finish_reason` 的 choice | `Usage` | 按 `choices: []` 的用量数据块读取。 |
| `: OPENROUTER PROCESSING` 注释 | `Extra` | 与其他 SSE 注释一样，仅限同一方言。 |
| `error.metadata.raw`，即上游服务商自己的错误，与笼统的 `message` `Provider returned error` 并列 | `KppError.message` | 通过配置的 `error_detail`，Anthropic Messages 和 Responses 应用以及请求日志会得到 `Provider returned error: <原因>`，例如 `google/gemma-4-31b-it:free is temporarily rate-limited upstream…`。Chat 应用收到 OpenRouter 发来的原样错误体。 |
| 200 之后出现的 `error` 对象，位于回复体中或与 `choices` 并列的数据块中 | `KppError` | 见[上方各行](openai-chat.md#流createchatcompletionstreamresponse)；数字 `code` 像状态码一样决定类别。 |
| `GET /api/v1/models`：没有 `object`，模型也没有 `object` 和 `owned_by`，带 `total_count` 和 `links` | `ModelList`、extras | `GET /api/v1/models/{model}` 对每个模型都返回 404，因此模型检查依赖模型列表，而 OpenRouter 提供该列表时不检查密钥。 |

## Vercel

来源：Vercel 的 [Chat Completions](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions)、[reasoning](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/reasoning)和[advanced configuration](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/advanced) 页面（2026-10-29 阅读），以及 `fixtures/openai_chat/providers/vercel/` 中的实时录制。Chat Completions 是 Vercel 的 `generation` 方言，它列出的每个模型都支持。

| Vercel 字段 | KPP | 说明 |
|---|---|---|
| `providerOptions`（`gateway` 下的路由，例如 `order`、`only`、`sort`、`models`、`byok`、`tags`、`user`）、`models`、`provider`、`reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同（`requests/gateway-options`）。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`，与 OpenRouter 相同。 |
| `message.reasoning_details`、`delta.reasoning_details` | extras | OpenRouter 结构化形式的同一推理；对于既非 OpenAI 也非 Anthropic 的模型，`format` 为 `"unknown"`。 |
| 消息中和最后一个数据块 delta 中的 `provider_metadata`（`gateway.routing`、`cost`、`marketCost`、`gatewayCost`、`generationId`），以及顶层的 `generationId` | extras | 发往其他方言时省略。录制中的 `clientSessionId` 是客户端指纹，已替换为零。 |
| `usage.cost`、`market_cost`、`gateway_cost`、`surcharge_cost`、`reporting_tag_write_cost`、`is_byok`、`cost_details`、`cache_creation_input_tokens` 及额外的 Token 明细 | 用量的 `raw` | Kaeso 按目录中的价格计算费用。`usage.cost` 只是推理费用：`requests/gateway-options` 中的标签产生了 0.00015 美元的附加费，只有 `gateway_cost` 包含它。 |
| 最后一个流式数据块，同时带 `finish_reason`、`provider_metadata` 和 `usage` | `Usage` | 按 `choices: []` 的用量数据块读取。 |
| OpenAI 错误结构中的错误，带 Vercel 自己的 `type`（`model_not_found`、`no_providers_available`、`authentication_error`）、作为对象的 `param`，以及与 `error` 并列的 `providerMetadata` | `KppError` | 类别取决于状态码：401 表示密钥错误，403 表示账户无权调用该模型（`errors/recorded-restricted-model`，免费套餐账户调用 Claude），404 表示模型未知。Chat 应用收到 Vercel 发来的原样错误体。 |
| `GET /v1/models`：OpenAI 的结构，每个模型带 Vercel 自己的字段（`name`、`type`、`context_window`、`max_tokens`、`pricing`、`tags`、`supported_parameters`） | `ModelList`、extras | 模型列表对错误的密钥返回 401（`errors/list-invalid-api-key`），但模型检查首先询问的 `GET /v1/models/{model}` 不检查密钥就会回答。 |

## Standard Compute

来源：Standard Compute 的 [Chat Completions](https://docs.standardcompute.com/api/chat-completions)、[模型和路由](https://docs.standardcompute.com/models)、[错误](https://docs.standardcompute.com/api/errors)和[模型列表](https://docs.standardcompute.com/api/models)页面，阅读于 2026-11-12，以及 `fixtures/openai_chat/providers/standardcompute/` 中的实时录制，其中大部分通过智能路由（`standardcompute`）。

| Standard Compute 字段 | KPP | 说明 |
|---|---|---|
| 智能路由回复中的 `model: "StandardCompute"` | `Response.model` | 回复从不说明由哪个模型作答；Standard Compute 的请求日志会说明。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`，与 OpenRouter 相同。 |
| `message.reasoning_details`、`delta.reasoning_details` | extras | 以 OpenRouter 结构化形式表示的同一推理，包括 Gemini 模型作为 `reasoning.encrypted` 项的思考签名；Chat 应用会原样收到，并在之后的轮次中发回。 |
| `native_finish_reason`、`refusal: null`、`system_fingerprint: null`、`service_tier` | extras | 发往其他方言时省略。 |
| `usage` 中的 `cache_write_tokens`、`audio_tokens`、`video_tokens`、`image_tokens` | `raw` 用量 | |
| 最后一个流式分块，在 `usage` 旁重复结束选项 | `Usage` | 按带 `choices: []` 的用量分块读取。 |
| 不带 `code` 的 401 `error`；来自模型自身提供商、带数字 `code` 的错误 | `KppError` | 类型取决于状态码；带 `code: "budget_exhausted"` 的 402 `insufficient_quota` 为 `Billing`。 |
| 未知模型的 400 `model_not_found` | 类型为 `InvalidRequest` 的 `KppError` | 从不返回 404，因此不会把目录中的模型标记为不可用。 |
| `GET /v1/models`：`StandardCompute`、`context_length`、`architecture`、`supported_parameters`、`anthropic_family_tier`、`is_family_default` | `ModelList`、extras | 无需密钥即可获取。某个密钥的列表可能缺少该密钥仍可调用的模型，并把路由器称为 `StandardCompute`，并把回复中的 `anthropic/claude-haiku-5.5` 写作 `anthropic/claude-haiku-5-5`，因此模型检查比较 ID 时不区分大小写，也不区分 `.` 和 `-`，找不到的模型判定为无法确定。`GET /v1/models/{model}` 对每个模型都返回 404。 |

## Cloudflare AI Gateway

来源：Cloudflare 的 [REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/)、[Unified Billing](https://developers.cloudflare.com/ai-gateway/features/unified-billing/) 和[模型目录](https://developers.cloudflare.com/ai/models/)页面，阅读于 2026-11-12，以及 `fixtures/openai_chat/providers/cloudflare-ai-gateway/` 中来自 Workers AI 模型的实时录制（第三方模型在没有额度时返回 402）。

| Cloudflare 字段 | KPP | 说明 |
|---|---|---|
| `/client/v4/accounts/{account_id}/ai/v1/…` | `EgressCtx::base_path` | 对于 Chat Completions、Responses 和 Messages，密钥的账户取代 `/v1`；密钥的网关写入 `cf-aig-gateway-id`。 |
| `message.reasoning` 和 `reasoning_content`、`delta.reasoning` 和 `reasoning_content` | `Block::Reasoning` | 两个字段中是相同的文本；配置读取 `reasoning`，Chat 应用会原样收到 `reasoning_content`。 |
| 每个流式分块中的 `usage`，以及最后一个包含全部用量的分块 | `Usage` | 每个分块只统计自身的 Token；`finish_reason` 之后的最后一个分块包含总数。 |
| `usage.neurons` | `raw` 用量 | Workers AI 自己的单位；Kaeso 按 Token 计价。 |
| `refusal`、`annotations`、`audio`、`function_call`、`tool_calls: []`、`token_ids` | extras | 为 Chat 应用往返保留。 |
| `{"errors": [{"code", "message"}], "success": false}` | `KppError` | Cloudflare 自己的封装，用于被拒绝的令牌（401，代码 10000）、余额不足（402，代码 2021）、未知模型（404，代码 7003）或已停用模型（410，代码 7010）、图片链接（400，代码 6004）以及被拒绝的推理强度（400，代码 8001）。类型取决于状态码；消息沿用 Cloudflare 的。 |
| 带 402 的 Messages 错误 `{"type": "error", "error": {"type": "invalid_request_error"}}` | 类型为 `Billing` 的 `KppError` | 无论类型如何，402 都是计费错误。 |
| 来自 Workers AI 模型的 Responses 流 | 不使用 | 返回 `$error: Error: Network connection lost.` 或 Workers AI 自己的 `{"response"}` 事件，而不是 Responses 事件（实时），因此 Kaeso 通过 Chat Completions 流式传输 Workers AI 模型。 |
