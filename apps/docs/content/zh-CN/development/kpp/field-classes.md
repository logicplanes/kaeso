---
title: 字段类别
description: 每个 KPP 字段要么是目标必须遵守的语义字段，要么是目标可以忽略的提示字段。
order: 2
---

每个 KPP 字段要么是**语义字段**，要么是**提示字段**（参见[设计原则](index.md#设计原则)）。

- **语义字段：** 目标必须遵守。如果目标方言或模型无法表达它，编解码器的 `check()` 会返回 `Unsupported` 并注明该字段，什么都不会发送。
- **提示字段：** 目标可以忽略。目标没有对应字段时，编解码器会静默丢弃它。

下文中的“丢弃”始终指*仅在目标无法表达时*丢弃；能够表达某个提示字段的目标总会收到它。

## `GenerateRequest`

| 字段 | 类别 | 说明 |
| --- | --- | --- |
| `model` | 语义 | 由网关在编码前解析。 |
| `system` | 语义 | |
| `messages` | 语义 | |
| `tools`（`Function`） | 语义 | 其 `cache` 是提示字段，与块相同（[缓存提示](cross-format.md#缓存提示)）。 |
| `tools`（`Unknown`） | 语义 | 只有其来源方言能够表达。 |
| `tool_choice` | 语义 | `Unknown` 只有其来源方言能够表达。 |
| `parallel_tool_calls` | 语义 | |
| `output` | 语义 | `JsonObject` 在 Anthropic 中没有对应项，返回 `Unsupported`，不做模拟（[结构化输出](cross-format.md#结构化输出)）。 |
| `reasoning.mode`、`.effort`、`.budget_tokens` | 语义 | 推理强度与预算按[推理强度与预算](cross-format.md#推理强度与预算)中的表格相互转换。 |
| `reasoning.summary` | **提示** | 只控制是否返回摘要。Anthropic：`thinking.display: "summarized"`（[可读推理与摘要](cross-format.md#可读推理与摘要)）。 |
| `reasoning.include_encrypted` | **提示** | 只控制是否返回加密的推理；仅限 OpenAI Responses（[可读推理与摘要](cross-format.md#可读推理与摘要)）。 |
| `sampling.max_output_tokens` | 语义 | |
| `sampling.temperature`、`.top_p` | 语义 | 它们的写法（`1` 还是 `1.0`）属于形式，只在同一方言内保留（[采样参数](cross-format.md#采样参数)）。 |
| `sampling.top_k` | 语义 | 仅限 Anthropic；其他目标返回 `Unsupported`（[采样参数](cross-format.md#采样参数)）。 |
| `sampling.stop` | 语义 | OpenAI Chat Completions 最多接受四个；OpenAI Responses 没有该参数，返回 `Unsupported`（[采样参数](cross-format.md#采样参数)）。 |
| `sampling.seed` | 语义 | 仅限 OpenAI Chat Completions；其他目标返回 `Unsupported`（[采样参数](cross-format.md#采样参数)）。 |
| `sampling.frequency_penalty`、`.presence_penalty` | 语义 | 仅限 OpenAI Chat Completions；其他目标返回 `Unsupported`（[采样参数](cross-format.md#采样参数)）。 |
| `user` | **提示** | 用于滥用监控的最终用户 ID，从不改变输出。 |
| `stream` | 语义 | |
| `extras.fields` | 语义 | 只写回来源方言；发往其他方言时返回 `Unsupported`，除非该字段在 [Extras 允许列表](cross-format.md#extras)中。 |
| `extras.form` | **提示** | 用于呈现的形式标记，其他方言会忽略。 |

## `Message`

| 字段 | 类别 | 说明 |
| --- | --- | --- |
| `role` | 语义 | 对话开始之后的 `System` 消息发往 Anthropic 时返回 `Unsupported`，除非请求来自 Anthropic Messages（[系统消息](cross-format.md#系统消息)）。 |
| `blocks` | 语义 | 块的顺序保持不变。 |
| `extras` | 与请求相同 | |

## `Block`

| 变体 / 字段 | 类别 | 说明 |
| --- | --- | --- |
| `Text.text` | 语义 | |
| `Text.citations` | 语义 | 保持原始数据并绑定来源；发往其他方言时返回 `Unsupported`。 |
| `Text.cache`、`Image.cache`、`Document.cache`、`ToolCall.cache`、`ToolResult.cache` | **提示** | 发往 OpenAI 时丢弃，因为 OpenAI 会自行缓存（[缓存提示](cross-format.md#缓存提示)）。 |
| `Image.source` | 语义 | `FileId` 只能用于签发它的服务商（[文件](cross-format.md#文件)）。OpenAI Chat Completions 只接受 URL 或 base64 形式的图片。 |
| `Document.source` | 语义 | 文本来源仅限 Anthropic。OpenAI Chat Completions 也不接受 URL 来源。 |
| `Document.title`、`.context` | 语义 | 仅限 Anthropic；发往 OpenAI 时返回 `Unsupported`。 |
| `Document.citations` | 语义 | 仅限 Anthropic；OpenAI 无法引用文档，返回 `Unsupported`。 |
| `ToolCall.*` | 语义 | ID 和名称保持不变；目标因字符或长度不接受时，会以固定方式改写，并在返回时还原。 |
| `ToolResult.call_id`、`.content` | 语义 | |
| `ToolResult.is_error` | 对没有错误标志的目标是**提示** | 发往 OpenAI Chat 和 Responses 时丢弃，由内容承载错误文本（[工具错误](cross-format.md#工具错误)）。 |
| `Reasoning.summary`、`.text` | **提示** | 在发往其他方言的请求中，可读推理会被丢弃（[推理](cross-format.md#推理)）。在回复中，它会与原始块一起放在[推理数据块](cross-format.md#推理数据块)中展示。 |
| `Reasoning.opaque` | 对其来源是语义 | 逐字节回放给其来源方言和服务商；从不发往其他地方，在那里整个块都会被丢弃（[推理](cross-format.md#推理)）。 |
| `Reasoning.origin` | — | 决定是否回放（[推理](cross-format.md#推理)）。没有签名的 Anthropic `thinking` 块只有在从 Anthropic Messages 读到时才会发回 Anthropic。 |
| `Refusal.text` | 语义 | Anthropic 没有拒绝块：发往它的请求返回 `Unsupported`，给 Anthropic 应用的回复中则变为文本块。 |
| `Unknown` | 语义 | 仅限来源方言。OpenAI Chat 的 `function` 消息作为一个这样的块完整保存（[请求](cross-format.md#请求)）。 |
| 每个块的 `extras` | 与请求相同 | 允许列表按块类型生效（[Extras 允许列表](cross-format.md#extras)）。 |

## `CacheHint`

| 字段 | 类别 | 说明 |
| --- | --- | --- |
| `ttl` | **提示** | |
| `extras` | **提示** | 随提示一起丢弃。 |

## `EmbedRequest`

| 字段 | 类别 | 说明 |
| --- | --- | --- |
| `model`、`input`、`dimensions`、`encoding` | 语义 | |
| `user` | **提示** | |
| `extras` | 与 `GenerateRequest` 相同 | |

## 回复与流事件

回复和流事件从一个服务商流向一个应用，其中没有提示字段：应用的方言无法表达的字段，要么可以推导出来，例如把停止原因映射到最接近的对应值，要么放在 Extras 中，以便在同一方言内回放。每个方言的映射页面都列出了这些映射：[Anthropic Messages](anthropic-messages.md)、[OpenAI Chat Completions](openai-chat.md)、[OpenAI Responses](openai-responses.md)。

在应用的格式能够承载时，推理会到达其他方言的应用，并经过包装，以便之后能够原样返回（[推理数据块](cross-format.md#推理数据块)，`cross::reasoning_reply_policy`）。未知块只会到达其自身方言的应用。其余内容都不会出现在应用中（[回复](cross-format.md#回复)，`cross::reply_policy`），流中则以[保活消息](cross-format.md#保活消息)帧代替。网关会记录完整的 KPP 回复。
