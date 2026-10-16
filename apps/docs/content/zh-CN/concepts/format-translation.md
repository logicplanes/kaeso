---
title: 格式转换
description: KPP 如何在 OpenAI 和 Anthropic 格式之间传递请求，以及会拒绝哪些内容。
order: 2
---

KPP，即 Kaeso Provider Protocol，是模型请求的一种规范格式，每种 API 格式都有一个编解码器负责与之相互转换。网关对每个请求都使用它，因此采用其中任一格式的应用都可以调用任一服务商的模型。

| 格式 | 端点 | 流式 |
| --- | --- | --- |
| Anthropic Messages | `/v1/messages`、`/v1/messages/count_tokens` | 是 |
| OpenAI Chat Completions | `/v1/chat/completions` | 是 |
| OpenAI Responses | `/v1/responses`、`/v1/responses/input_tokens` | 是 |
| OpenAI Embeddings | `/v1/embeddings` | 否 |

请求带有 `anthropic-version` 时，`GET /v1/models` 和 `GET /v1/models/{id}` 以 Anthropic 的结构列出项目的模型，否则以 OpenAI 的结构列出。嵌入请求只发往 OpenAI 的嵌入模型。

## 三条规则

1. **格式相同，没有损失。** 以原格式发往服务商的请求会按发送时的原样到达。只有模型名称会改为服务商的模型 ID，并且流始终会请求用量，而应用只有在自己请求时才会看到用量。
2. **拒绝，而不丢弃。** 请求跨格式转换时，目标格式无法支持的字段会被拒绝，错误中会注明该字段。只有不改变模型输出内容的提示信息会被省略。
3. **签名的推理只回到原处。** 加密或签名的推理内容只会发回签发它的格式和服务商。

## 可以互通的内容

- 对话：系统指令、用户和助手轮次、文本、图像和文档。Anthropic 对话中间的系统消息到达 OpenAI 时，会成为一条位置靠后的系统消息。
- 工具：函数工具、工具选择、并行工具调用、工具调用和工具结果。
- 目标不接受的工具名称和调用 ID 会以固定方式改写，并在回复中映射回原值：其他字符变为 `_`，名称被截短以符合长度要求，并附加原值的哈希。OpenAI 接受最多 64 个由字母、数字、`_` 和 `-` 组成的名称；Anthropic 最多 128 个。
- 结构化输出：JSON schema。
- 采样：温度、Top P、最大 Token 数和停止序列，前提是目标支持。
- 最终用户的 ID：Anthropic 的 `metadata.user_id`，OpenAI 的 `safety_identifier`。
- 推理：推理强度和思考预算会相互转换。
- 流：逐个事件转换为应用的格式。

## 会被拒绝的内容

| 发往 | 拒绝并返回错误 |
| --- | --- |
| Anthropic | `seed`、频率惩罚和存在惩罚、大于 1 的温度、`json_object` 输出格式、带有描述或带有 `strict: false` 的 JSON schema、来自其他格式的对话中间的系统消息、历史中的拒绝回复、JPEG、PNG、GIF 和 WebP 以外的图像、PDF 和纯文本以外的文档、`minimal` 推理强度 |
| OpenAI Chat Completions | `top_k`、超过四个停止序列、文本文档、文档 URL、文档标题、上下文和引用、图像文件 ID、用户消息之外的图像和文档 |
| OpenAI Responses | `top_k`、`seed`、惩罚参数、停止序列、文本文档、文档标题、上下文和引用 |
| 任何服务商 | 多个 choice（`n`）；存储状态：`store: true`、`previous_response_id`、`conversation`、`background` |
| 其他格式 | KPP 中没有对应位置的应用格式字段，例如 Chat Completions 的 `logprobs`、`logit_bias` 和 `service_tier`；其他格式的内置工具（例如网页搜索）及其结果；引用；来自其他服务商的文件 ID |

当目标没有对应位置时，不改变模型输出内容的提示信息会被省略：缓存提示、beta 和版本请求头、图像细节、文件名、提示词缓存键、Anthropic 的 `thinking.display`、只清除先前思考内容的 Anthropic 上下文管理，以及 OpenAI 没有的工具结果错误标记；工具结果的文本仍会说明失败的内容。转换为 Responses 的请求带有 `store: false`，因此 OpenAI 不会保存任何内容。

## 推理

| 来自应用 | 发往 | 转换为 |
| --- | --- | --- |
| 思考预算 | OpenAI | 推理强度：低于 4,096 个 Token 为 `low`，4,096 到 16,383 为 `medium`，16,384 及以上为 `high` |
| 关闭思考 | OpenAI | 推理强度 `none` |
| 从 `low` 到 `max` 的推理强度 | 支持自适应思考的 Claude | 采用该推理强度的自适应思考 |
| 从 `low` 到 `max` 的推理强度 | 不支持自适应思考的 Claude | 预算：`low` 为 2,048，`medium` 为 8,192，`high`、`xhigh` 和 `max` 为 24,576 个 Token |
| 推理强度 `none` | Claude | 不思考 |

- **推理强度和预算。** 同一请求中的推理强度优先于预算。当目录条目如此注明时，Claude 模型被视为不支持自适应思考；Claude 4.5 及更早版本不支持。预算会按 Anthropic 的规则调整：至少 1,024 个 Token，且小于 `max_tokens`。当 `max_tokens` 为 1,024 或更小时，预算为 1,024，`max_tokens` 增加 1,024，以便为回答保留空间。
- **思考和采样。** 当 Kaeso 为来自其他格式的请求开启 Claude 的思考时，会省略 `temperature`、`top_p` 和 `top_k`，因为 Anthropic 会拒绝将它们与思考一起使用。强制工具调用（`required` 或指定的工具）不使用预算思考，因为 Anthropic 会拒绝强制工具与预算思考同时使用；自适应思考则保留强制工具。Anthropic 应用自身的设置会按收到的原样发送。
- **来自其他地方的工具轮次。** 当预算思考开启，而上一个助手轮次的工具调用中没有 Claude 可以重放的思考内容时（例如这些调用由另一个服务商发出），该请求不使用思考。自适应思考保持开启。

## 轮次之间的推理

来自其他服务商的推理内容会以应用自身的格式返回给应用，原始内容包装在其中，以便应用将其发回。Responses 应用会收到一个在 `encrypted_content` 中包含原始内容的推理项；Anthropic 应用会收到一个在 `signature` 中包含原始内容的 `thinking` 块。应用将其发回时，KPP 会为写入它的服务商恢复原始内容。推理强度不是 `none` 的转换请求会向 OpenAI 请求其加密的推理内容，以便这些内容也能返回。

Chat Completions 没有用于推理内容的字段。当 Chat Completions 应用由 Claude 提供服务，或由通过 Responses 调用的 OpenAI 模型提供服务时，网关会保存每次工具调用之前的推理内容，并在应用发送工具结果时将其放回。这样模型在工具循环中能保持其思路。

- 推理内容在 Valkey 中保存 `KAESO_CONTINUITY_REASONING_TTL`（1 小时），以主密钥加密，且只能用发出该请求的 API 密钥读取。它从不记录到日志中。`0s` 会关闭此功能（[推理连续性](../self-hosting/configuration.md#推理连续性)）。
- 最后一次工具调用之后的推理内容不会保存，失败的流中的推理内容也不会保存。
- 当推理内容已不存在时（已过期、使用了其他密钥、无法连接 Valkey），请求会在没有它的情况下继续。`kaeso_reasoning_continuity_total` 统计已保存、已恢复、缺失和不可用的推理内容（[监控](../self-hosting/monitoring.md)）。

可读的推理内容遵循应用在其自身格式中的请求：

- 设置了 `reasoning.summary` 的 Responses 应用会以推理摘要的形式收到 Claude 的思考内容，以 `response.reasoning_summary_text.delta` 事件流式发送。网关会向 Claude 请求摘要形式的思考内容，否则 Claude Opus 4.7 及更高版本会隐藏这些内容。
- 开启思考的 Anthropic 应用会以思考文本的形式收到 OpenAI 模型的推理摘要。除非应用将 `thinking.display` 设为 `omitted`，否则网关会向 OpenAI 请求这些摘要。OpenAI 可能会拒绝向未经其验证的组织提供摘要；其错误会被传递给应用。
- Chat Completions 应用看不到推理内容。

目录注明模型是否进行推理（models.dev 的 `reasoning`）。对于进行推理的模型，不带推理强度的 `reasoning: {summary: "auto"}` 会以默认深度开启 Claude 的思考：自适应思考，或 8,192 个 Token 的预算。不带推理强度的 Anthropic 应用思考会为下一轮带回 OpenAI 的加密推理内容。对于不进行推理或未知是否进行推理的模型，从不开启推理，对此类模型请求推理强度、思考或预算的请求会被拒绝。
