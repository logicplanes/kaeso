---
title: Anthropic Messages
description: Anthropic Messages 格式的每个字段如何映射到 KPP 并映射回来。
order: 4
---

编解码器：`kaeso_codecs::anthropic_messages::AnthropicMessages`（`crates/kaeso-codecs/src/anthropic_messages/`）。端点：`POST /v1/messages`（JSON 和 SSE）以及 `POST /v1/messages/count_tokens`。

来源：Anthropic 的 OpenAPI 规范 `anthropic-465bff21a179090915396565d1ae8f705cf8596e2ec920eb121072f25b8a7d68.yml`（`anthropic-sdk-python/.stats.yml` 中最后一个 `openapi_spec_url`，SDK 提交 `41aa7674`）、`anthropic-sdk-python@b4b7916d`（1.13.0）中的 SDK 类型，以及 [Claude 文档](https://platform.claude.com/docs/en/)（`api/messages/create`、`api/messages/count_tokens`、`api/errors`、`api/versioning`、`build-with-claude/streaming`、`…/thinking`、`…/extended-thinking`、`…/effort`、`…/prompt-caching`、`…/structured-outputs`、`…/citations`、`…/pdf-support`、`…/vision`、`agents-and-tools/tool-use/*`、`…/handling-stop-reasons`）。SDK 类型比规范新的地方以 SDK 类型为准，只在 SDK 类型中出现的字段会注明这一点。下文的 schema 名称均指规范中的名称。

## 表格的读法

- **KPP**：wire 值映射到的类型化字段。
- **fields**：原样保存在 `Extras::fields` 中。这些字段是语义字段：只会写回 Anthropic；发往其他方言时会以 `Unsupported` 拒绝（[Extras](cross-format.md#extras)），除非[允许列表](extras-allowlist.md)允许将其丢弃；在发给其他方言的回复中会被省略（[回复](cross-format.md#回复)）。嵌套对象使用带点的键路径（`metadata.x`）。
- **form**：`Extras::form` 中编解码器私有的形式标记，其他方言会忽略它。`verbatim` 指 `wire::Ext` 的通用机制（`null`、`false` 和 `[]` 的写法）。
- **raw**：完整保存在 `RawItem`、`Citation` 或 `RawUsage` 中。
- “自有”指 extras 或来源为 `anthropic_messages` 的数据；“外来”指来自其他方言的数据。

类型化的数值字段按 `u32` 或 `u64` 读取，`temperature` 和 `top_p` 按 JSON 数字读取，KPP 以 `f64` 保存它们。超出其范围的值不会被修改，而是解码失败：在请求中为 `InvalidRequest`（`` The request body is not a valid Anthropic Messages request: `top_k` must be a non-negative integer. ``），在服务商回复中为 `ProviderError`。

Kaeso 逐字段读取的每个对象都会把未知键保存为 fields，但以 `header:` 开头或包含 `.` 的键除外：这样的键会被误认为转发的请求头或嵌套的键路径，因此整个对象会被拒绝（[Wire 结构体](index.md#wire-结构体)）。在请求中为 `InvalidRequest`（`` The request body is not a valid Anthropic Messages request: the field `a.b` has a name Kaeso cannot carry; remove it. ``），在回复、流或模型列表中为 `ProviderError`。这些对象包括：请求体、消息、`metadata`、`output_config`、`thinking`、`tool_choice`、函数工具、[内容块](#内容块)表格中的已知块类型及其 `source`、`cache_control` 和 `citations` 对象、回复、Token 计数、流事件及其增量、模型信息和模型列表。完整保存的内容不做检查：未知的块和工具、`input_schema`、工具输入、引用、`usage` 和 `output_config.format`。顶层带有这种键的错误响应体被视为不是 `ErrorResponse` 的响应体（[错误](#错误)）。

## 请求

Schema `CreateMessageParams` 和 `CountMessageTokensParams`。

| Wire 字段 | KPP | 说明 |
|---|---|---|
| `model` | `model` | |
| `messages[]`（`InputMessage`） | `messages[]` | 未知键 → 消息的 `extras.fields`。 |
| `messages[].output_config`、`.clear_at`（系统消息，beta；SDK `BetaMessageParam`） | 消息的 `extras.fields` | 其他方言：为 `{}` 或重复请求中 `output_config.effort` 的 `output_config` 会被丢弃，因为它不改变任何内容（Claude Code 会在其系统消息上发送它）；其他任何值以及 `clear_at` → `Unsupported`（[Extras](cross-format.md#extras)）。 |
| `messages[].role` `user` / `assistant` | `Role::User` / `Role::Assistant` | |
| `messages[].role` `system` | `Role::System` | 对话中间的系统消息（SDK `MessageParam.role`；[对话中间的系统消息](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)）。原样发回 Anthropic，发往 OpenAI 时成为位置靠后的 `system` 或 `developer` 消息（[系统消息](cross-format.md#系统消息)）。其他角色 → `InvalidRequest`。 |
| 字符串形式的 `messages[].content` | 一个 `Block::Text` | form `content: "string"`；外来消息总是以数组发送。 |
| 数组形式的 `messages[].content` | `blocks` | `[]` → form `content: "array"`。 |
| 字符串形式的 `system` | `system: [Text]` | form `system: "string"`。 |
| 数组形式的 `system`（`RequestTextBlock[]`） | `system` | `[]` → form `system: "array"`。 |
| `max_tokens` | `sampling.max_output_tokens` | 生成请求必需：缺少时 `check()` 返回 `InvalidRequest`（参数 `max_tokens`）。 |
| `temperature`、`top_p`、`top_k` | `sampling.temperature`、`.top_p`、`.top_k` | 在规范中已弃用，SDK 1.13.0 中也没有，但仍会映射。`temperature` 和 `top_p` 保留其写法：写成整数（`1`）的键会列在 form `integers` 中，只要值仍是整数，就会再次写成整数，因此 `temperature: 1` 返回时仍是 `1`，而不是 `1.0`；改变后的值按新值写出。来自其他方言的请求使用 Anthropic 自己的写法（`1.0`）（[采样参数](cross-format.md#采样参数)）。 |
| `stop_sequences` | `sampling.stop` | `[]` → verbatim。 |
| `stream: true` | `stream = On { include_usage: true }` | Anthropic 总是报告用量。`stream: false` → verbatim。 |
| `metadata.user_id` | `user` | `null` → verbatim（`verbatim:metadata`）；其他键 → fields `metadata.*`；存在 `metadata` 对象 → form `objects: ["metadata"]`。 |
| `thinking.type` `enabled` / `disabled` / `adaptive` | `reasoning.mode` `Enabled` / `Disabled` / `Adaptive` | `between_tools`（仅 SDK 中有）和将来的类型 → `ReasoningMode::Other`。 |
| `thinking.budget_tokens` | `reasoning.budget_tokens` | |
| `thinking.display` 及其他 `thinking` 键 | `reasoning.extras.fields` | 没有对应的 KPP 字段。`display` 在[允许列表](extras-allowlist.md)中，因此发往其他方言时会被丢弃；OpenAI Responses 会收到 `reasoning.summary: "auto"`，除非其值为 `omitted`（[可读推理与摘要](cross-format.md#可读推理与摘要)）。其他键在那里 → `Unsupported`。 |
| `output_config.effort` | `reasoning.effort` | `low`、`medium`、`high`、`xhigh`、`max`；未知值 → `ReasoningEffort::Other`。 |
| `output_config.format` `{type: json_schema, schema}` | `output = JsonSchema { schema }` | `name`、`description` 和 `strict` 为 `None`。`format` 的未知键 → 规范的 `extras`。其他格式类型，以及 Kaeso 无法逐字段读取的 `json_schema` 格式（没有 `schema`，或带有它无法承载的键）→ fields `output_config.format`。 |
| 其他 `output_config` 键（例如 `task_budget`，beta） | fields `output_config.*` | 存在 `output_config` → form `objects: ["output_config"]`；值为 `null` 的成员 → `verbatim:output_config`。`task_budget` 会在模型面前放一个倒计时，因此属于语义字段：其他方言 → `Unsupported`，并提示在 Claude Code 中设置 `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`。 |
| `context_management`（beta，`BetaContextManagementConfigParam`） | fields `context_management` | 其他方言：当它只包含 `edits` 且每项编辑都是 `clear_thinking_*` 时丢弃，因为它只管理 Anthropic 的思考块，而思考块从不跨方言（[推理](cross-format.md#推理)）；Claude Code 在每个请求中都会发送带 `keep: "all"` 的 `clear_thinking_20251015`。其他编辑（`clear_tool_uses_*`、`compact_*`）会改变模型看到的内容 → `Unsupported`，并附带 `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` 的提示。 |
| `tool_choice` `auto` / `any` / `none` / `tool{name}` | `tool_choice` `Auto` / `Required` / `None` / `Tool(name)` | 其他类型、没有 `name` 的 `tool`，以及带有 `name` 的 `auto`、`any` 或 `none` → `ToolChoice::Unknown`（raw，仅限自有）。未知键 → fields `tool_choice.*`。 |
| `tool_choice.disable_parallel_tool_use` | `parallel_tool_calls = Some(!value)` | |
| `type` 缺失、为 `null` 或 `custom` 的 `tools[]`（`Tool`） | `ToolDef::Function` | `type` 的写法 → `verbatim`。 |
| `tools[].name`、`.description`、`.input_schema`、`.strict`、`.cache_control` | `name`、`description`、`parameters`（raw）、`strict`、`cache` | |
| `tools[].eager_input_streaming`、`.allowed_callers`、`.defer_loading`、`.input_examples` | 工具的 `extras.fields` | 其他方言 → `Unsupported`；错误消息会提示 Claude Code 用户设置 `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`，这样就不会再发送这些 beta 工具字段。 |
| 任何其他类型的 `tools[]`（服务器工具，例如 `web_search_20250305` 或 `advisor_20260301`） | `ToolDef::Unknown`（raw） | 仅限同一方言。对于其他方言，`Unsupported` 消息会在 `Input tag` 后注明类型（`Input tag 'advisor_20260301' found using 'type' is not a function tool, …`），这是 API 自己的措辞，因此 Claude Code 会去掉 advisor 工具后重试（[网关协议](https://code.claude.com/docs/en/llm-gateway-protocol)，“Automatic retry and error forwarding”）。 |
| `cache_control`（顶层，自动缓存） | fields `cache_control` | 只是提示字段；KPP 没有请求级别的缓存提示。它在[允许列表](extras-allowlist.md)中，因此发往其他方言时会被丢弃。 |
| `container`、`inference_geo`、`service_tier`、SDK `diagnostics`、`user_profile_id`、`workspace_id`、未知键 | `extras.fields` | |
| 请求头 `anthropic-beta` | `extras` 请求头 `header:anthropic-beta` | 出站时只转发给 Anthropic。 |
| 请求头 `anthropic-version` | 不为 `2023-06-01` 时存为 `extras` 请求头 | 总是发送固定的版本，除非转发了更新的版本（[Anthropic 版本](cross-format.md#anthropic-版本)）。 |

### 内容块

以下规则同样适用于请求和回复。

| Wire 块 | KPP | 说明 |
|---|---|---|
| `text`（`text`、`citations`、`cache_control`） | `Block::Text` | `citations[]` → `Citation { origin, raw }`；`null` 和 `[]` → verbatim。 |
| `image` `source.type: base64`（`media_type`、`data`） | `Image { source: Base64 }` | |
| `image` `source.type: url` | `Image { source: Url }` | |
| `image` `source.type: file`（`file_id`） | `Image { source: FileId { origin } }` | 只能用于 Anthropic（`cross::file_id_usable`，[文件](cross-format.md#文件)）。 |
| `image.transformations`、未知键 | 块的 `extras.fields` | source 的未知键 → `source.*`。`source.type` 不属于以上类型的图像 → `Block::Unknown`。 |
| `document` `source.type: base64`（PDF）、`url` 或 `file` | `Document { source: Base64 / Url / FileId }` | |
| `document` `source.type: text`（`text/plain`） | `Document { source: Text }` | |
| `document` `source.type: content`（自定义内容） | `Block::Unknown` | 仅限同一方言。 |
| `document.title`、`.context` | `title`、`context` | `null` → verbatim。 |
| `document.citations.enabled` | `citations` | `{enabled: false}` → form `citations: "disabled"`；`{}` → `"empty"`；`null` → verbatim。 |
| 任意块上的 `cache_control`（`CacheControlEphemeral`） | `cache = CacheHint { ttl }` | `ttl` `5m` → `FiveMinutes`，`1h` → `OneHour`，其他值 → `Other`；缺失 → `None`。`ephemeral` 以外的 `type` → 提示的 `extras.fields.type`。 |
| `tool_use`（`id`、`name`、`input`、`cache_control`） | `Block::ToolCall`（`arguments` 为 raw） | `caller` 和 `toolset_name` → 块的 `extras.fields`；但 `caller: {type: "direct"}` 表示普通调用，按 verbatim 保留，使该调用可以发往其他方言。 |
| `tool_result`（`tool_use_id`、`content`、`is_error`、`cache_control`） | `Block::ToolResult` | 字符串形式的 `content` → form `content: "string"`；`[]` → `"array"`；`is_error: false` → verbatim。`toolset_name` → fields。嵌套在其他 `tool_result` 中超过 4 层的 `tool_result` 会被拒绝：在请求中为 `InvalidRequest`，在回复或流中为 `ProviderError`。 |
| `thinking`（`thinking`、`signature`） | `Block::Reasoning { text: [thinking], opaque: [AnthropicThinkingSignature] }` | `thinking: ""`（省略显示）保持为 `[""]`。`signature: ""` 不是不透明数据，而是变为 form `empty_signature`，因此该块永远不会被视为已签名的思考，并以 `""` 写回；缺失的 `signature` 保持缺失。若 `signature` 是推理数据块（`kpp:reasoning:v1:…`，即 Kaeso 交给应用的其他方言的推理，见[推理数据块](cross-format.md#推理数据块)），则解码为其中的原始块，包括来源；此时会忽略 `thinking` 文本。 |
| `redacted_thinking`（`data`） | `Block::Reasoning { opaque: [AnthropicRedactedThinking] }` | |
| 任何其他类型（`server_tool_use`、`web_search_tool_result`、`web_fetch_tool_result`、`code_execution_tool_result`、`search_result`、`container_upload`、`tool_reference`、将来的类型） | `Block::Unknown`（raw） | 仅限同一方言。`tool_reference`（MCP 工具搜索）发往其他方言被拒绝时，错误消息会提示在 Claude Code 中取消设置 `ENABLE_TOOL_SEARCH`，这样在网关之后工具搜索默认是关闭的。 |

### 来自其他方言的请求

对于并非来自 Anthropic Messages 的 KPP 请求，`encode_request` 会写出以下内容。

| KPP | Wire | 说明 |
|---|---|---|
| `system` | 以文本块数组形式的 `system` | 系统提示中的其他块 → `Unsupported`。 |
| `messages[].blocks` | 数组形式的 `content` | 空文本块会被丢弃，因为 Anthropic 不接受它们。丢弃推理或空文本后变空的消息会被省略。 |
| 并非来自 Anthropic 或来自另一个 Anthropic 服务商的 `Block::Reasoning` | 丢弃 | [推理](cross-format.md#推理)。这也包括应用以推理数据块发回的其他方言的推理：它只会发往自己的服务商。 |
| 没有非空签名的 `thinking` | 自有块（从 Anthropic Messages 读取）：按收到时的原样写出，缺失的 `signature` 保持缺失，`""`（form `empty_signature`）保持为 `""`；其他块丢弃 | 只包含这种自有块的助手轮次会保留。并非从 Anthropic Messages 读取的未签名 Anthropic 推理（由代码构建，没有 Anthropic extras）会被丢弃，因为 Anthropic 会拒绝未签名的思考块。`redacted_thinking` 会被重放。 |
| 带 base64 `data:` URL 的 `Source::Url`（来自其他方言） | `source: {type: base64, media_type, data}` | 不是 base64 的 `data:` URL → `Unsupported`；媒体类型的检查方式与 base64 相同。 |
| 文本、图像或文档以外的 `ToolResult` 内容（以及自有的未知块） | `Unsupported` | `RequestToolResultBlock.content`。 |
| `Block::Refusal` | `Unsupported`（`refusal block`） | |
| `Text.cache`、任何 `CacheHint` | `cache_control {type: ephemeral, ttl?}` | 外来提示的 extras 会被丢弃。 |
| 不匹配 `^[a-zA-Z0-9_-]+$` 的 `ToolCall.id` 或 `ToolResult.call_id` | 改写（其他字符变为 `_`，再追加 `_` 和 16 位十六进制 FNV-1a 哈希），并记录在 `EgressCtx::tool_ids` 中 | 在回复和流中映射回原值。 |
| 不匹配 `^[a-zA-Z0-9_-]{1,128}$` 的工具名称 | 以同样方式改写并截短到 128 个字符，记录在 `EgressCtx::tool_names` 中 | 应用于 `tools[].name`、`tool_choice.name` 和 `tool_use.name`。 |
| `FunctionTool.parameters: None` | `input_schema: {"type":"object","properties":{}}` | 在 OpenAI 中，没有参数即表示参数列表为空。 |
| 没有 `tool_choice` 的 `parallel_tool_calls: Some(false)` | `tool_choice: {type: auto, disable_parallel_tool_use: true}` | 不会添加到 `none`，因为它的 schema 中没有这个键。 |
| 没有模式或预算的 `reasoning.effort` `low` 到 `max` | `thinking: {type: adaptive}` 加 `output_config.effort`；在不支持自适应思考的模型上（`EgressCtx::adaptive_thinking = Some(false)`），按[推理强度与预算](cross-format.md#推理强度与预算)的表格得到 `thinking: {type: enabled, budget_tokens}`（`low` 2,048，`medium` 8,192，`high`、`xhigh` 和 `max` 24,576，并调整为至少 1,024 且小于 `max_tokens`；当 `max_tokens` 为 1,024 或更小时，将其增加 1,024，预算为 1,024） | [推理](cross-format.md#推理)。 |
| 没有推理强度、模式或预算的 `reasoning.summary`，且 `EgressCtx::reasoning = Some(true)` | 不带推理强度的 `thinking: {type: adaptive}`（模型的默认深度）；在不支持自适应思考的模型上使用 `medium` 预算 | OpenAI 的推理模型默认会推理，因此请求摘要的应用期望得到推理（[可读推理与摘要](cross-format.md#可读推理与摘要)）。未知模型或不推理的模型：不发送任何内容。 |
| 思考开启时的 `reasoning.summary`（任何值） | `thinking.display: "summarized"` | Claude Opus 4.7 及更高版本默认为 `omitted`（思考文本为空）。不适用于 Token 计数。对于会在工具调用之间写简短进度说明的模型，这些说明会与摘要混在一起返回；从不发送 `display: "updates"`。 |
| 为其他方言开启思考时的 `sampling.temperature`、`.top_p`、`.top_k` | 省略 | Anthropic 在思考开启时会拒绝它们，或限制 `top_p`（[思考与采样](cross-format.md#思考与采样)）。没有推理设置时，它们按收到时的原样发送，即使发往默认会思考的模型也是如此；对于目录条目注明 `temperature: false` 的模型，网关会拒绝非默认的 `temperature`。 |
| 来自其他方言、带 `enabled` 思考的 `tool_choice` `Required` 或 `Tool` | 不带 `thinking` 发送（也不带因思考而提高的 `max_tokens`） | Anthropic 会拒绝这种组合，而且强制调用本来也不包含思考。自适应思考会保留强制工具。 |
| `reasoning.effort` `none` | 不发送 `thinking`，不发送推理强度 | 这是 Anthropic 的默认行为。因此，在没有 `thinking` 时也会思考的模型（Claude Opus 5 和 5.5、Sonnet 5 和 5.5、Haiku 5.5 以及 Fable）仍会以其默认推理强度思考。思考开启时的 `none`、`minimal` 以及未知的推理强度 → `Unsupported`。 |
| `reasoning.budget_tokens`（没有模式） | `thinking: {type: enabled, budget_tokens}` | |
| 没有预算的 `reasoning.mode = Enabled`；带 `Adaptive` 或 `Disabled` 的预算 | `Unsupported` | 这里不做推理强度与预算之间的转换。 |
| 来自其他方言的 `reasoning.include_encrypted` | 丢弃 | 它只说明要返回哪些推理（Codex 总是发送它）；Anthropic 总是返回已签名的思考。 |
| `thinking` `enabled`，且最后一条消息（不计其后的系统消息）返回工具结果，而最后一个助手轮次发起了工具调用，却没有以可重放的思考块（已签名的 `thinking` 或 `redacted_thinking`）开头（例如该轮次来自另一个服务商） | 请求不带 `thinking` 和 `output_config.effort` 发送 | Anthropic 会拒绝这样的请求（“a final assistant message must start with a thinking block”）；见[没有思考的工具轮次](cross-format.md#没有思考的工具轮次)。`adaptive` 思考会保留：自适应思考的文档说明它不需要开头的思考块。除此之外不做任何记录。这种情况很少见，因为 Claude 产生的思考会从 Responses 和 Anthropic 应用以[推理数据块](cross-format.md#推理数据块)的形式返回，从 Chat 应用则经由[网关的存储](cross-format.md#由网关保存的推理)返回，所以该轮次会以可重放的思考开头。该规则同样适用于来自 Anthropic 应用的请求。 |
| `output = JsonObject` | `Unsupported`（`json_object output`） | [结构化输出](cross-format.md#结构化输出)。 |
| `output = JsonSchema` | `output_config.format` | 丢弃 `name`；`description` 和 `strict: false` → `Unsupported`。 |
| `sampling.seed`、`.frequency_penalty`、`.presence_penalty` | `Unsupported` | [采样参数](cross-format.md#采样参数)；不适用于 Token 计数，因为计数时会省略采样参数。 |
| 超出 0 到 1 范围的 `sampling.temperature` | `Unsupported` | Anthropic 的范围是 0 到 1（OpenAI 允许到 2）；不会重新缩放。不适用于 Token 计数。 |
| `user` | `metadata.user_id` | |
| `Role::System` | 对从 Anthropic Messages 解码的请求，写为 `role: "system"` 消息；来自其他方言时为 `Unsupported`（`system message`） | [系统消息](cross-format.md#系统消息)。 |
| JPEG、PNG、GIF 或 WebP 以外的 base64 图像；文本形式的图像来源 | `Unsupported` | `Base64ImageSource`。 |
| `application/pdf` 以外的 base64 文档；`text/plain` 以外的文本文档 | `Unsupported` | `Base64PDFSource`、`PlainTextSource`。 |
| 不是对象的工具调用参数；不是对象的工具参数定义 | `Unsupported` | `input` 和 `input_schema` 都是对象。 |
| 来自其他方言的 `Request::CountTokens` | 不带 `max_tokens`、采样参数、`metadata` 和 `stream` 的 `/v1/messages/count_tokens` | 它们不改变输入 Token 数，而且该端点会拒绝它们。Anthropic 应用自己的 Token 计数请求按收到时的原样发送。 |
| `check()` 在生成请求中会拒绝的 `Request::CountTokens` 输出和推理设置（JSON 模式、schema 描述、`strict: false`、推理强度 `minimal` 等） | 省略，不拒绝 | 只拒绝会改变所计数输入的内容。 |
| `Request::Embed` | `Unsupported` | |
| 外来 extras、未知的块、工具和工具选择、引用、外来文件 ID | `Unsupported` | `cross::check_foreign`（[请求](cross-format.md#请求)）。 |

## 回复

Schema `Message`。

| Wire 字段 | KPP | 说明 |
|---|---|---|
| `id`、`model` | `id`、`model` | 来自其他格式时，保留服务商的 ID 不变（[回复 ID](cross-format.md#回复-id)）。 |
| `content[]` | `blocks` | 与[内容块](#内容块)相同；工具 ID 和名称通过 `EgressCtx` 映射回原值。 |
| `stop_reason` `end_turn` / `max_tokens` / `stop_sequence` / `tool_use` / `pause_turn` / `refusal` | `stop` `EndTurn` / `MaxTokens` / `StopSequence(s)` / `ToolUse` / `PauseTurn` / `Refusal` | |
| `stop_reason` `model_context_window_exceeded` | `MaxTokens` 加 form `spelling:stop_reason` | 输出被上下文窗口截断；Anthropic 应用会收到原样的值。 |
| 未知的 `stop_reason` 值 | `StopReason::Other(value)` | |
| `stop_reason: null` 或缺失 | `EndTurn` 加 form `spelling:stop_reason` | |
| `stop_sequence` | 在 `StopSequence(s)` 中 | 任何其他写法（缺失，或在其他停止原因下有值）→ form `spelling:stop_sequence`。 |
| `stop_details`（`RefusalStopDetails`）、`container`、`diagnostics`、`type`、`role`、未知键 | `extras.fields` | |
| `usage.input_tokens` | `usage.input_tokens` | 已经不包含缓存读取和写入（[用量](cross-format.md#用量)）。 |
| `usage.cache_read_input_tokens` | `usage.cache_read_tokens` | `null` → 0。 |
| `usage.cache_creation_input_tokens` | `usage.cache_write_tokens` | `null` → 0。 |
| `usage.cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` | `usage.cache_write_breakdown` | |
| `usage.output_tokens` | `usage.output_tokens` | 包含思考。 |
| `usage.output_tokens_details.thinking_tokens` | `usage.reasoning_tokens` | |
| 整个 `usage`（含 `server_tool_use`、`service_tier`、`inference_geo`、`iterations` 和未知键） | `usage.raw`（绑定到其来源） | 对 Anthropic 回复会以相同的键和值重新写出。 |

来自其他方言的回复（`encode_response`）会得到 `type: "message"`、`role: "assistant"`、`stop_sequence: null`（或相应的序列）以及 `stop_details: null`。用量由类型化字段构建：`cache_creation` 来自细分数据或为 `null`，已知 `reasoning_tokens` 时会写出 `output_tokens_details`。

- **停止原因。** `ContentFilter` 变为 `refusal`，因为 Anthropic 没有内容过滤类的停止原因。若 `Other(s)` 是 Anthropic 文档中的值，则写为 `s`；否则 `length`、`max_output_tokens` 和 `max_completion_tokens` 变为 `max_tokens`，`function_call` 和 `tool_calls` 变为 `tool_use`，`content_filter` 变为 `refusal`，其余一律变为 `end_turn`。自有的 `Other` 值按收到时的原样写出。
- **拒绝回复。** `Refusal` 块变为 `text` 块。
- **有损的工具参数。** 对于不是有效 JSON 的工具调用参数（例如被输出上限截断），KPP 会将其保存为 JSON 字符串。这样的调用会以 `input: {}` 写出，因为 `tool_use.input` 必须是对象，所以应用会丢失这段不完整的文本；网关日志中的 KPP 回复仍保留着它。带有此类参数的请求会被 `check()` 拒绝。
- **推理**：来自其他方言的推理会变为 `thinking` 块（[推理数据块](cross-format.md#推理数据块)）：先是其摘要，然后是推理文本，作为 `thinking`（段落之间以空行分隔），并以原始块的推理数据块作为 `signature`。Anthropic 永远不会看到这个块：请求解码器会恢复原始块，而原始块只会发回它自己的服务商。
- **省略的内容**（[回复](cross-format.md#回复)）：并非从 Anthropic Messages 读取、没有签名的 Anthropic 推理（自有的未签名思考按收到时的原样写出）；图像、文档和工具结果块，它们不属于 `Message` 的内容联合类型；以及外来的不透明数据、引用、extras 和未知块。

## Token 计数

Schema `CountMessageTokensResponse`。

| Wire 字段 | KPP | 说明 |
|---|---|---|
| `input_tokens` | `CountTokensResponse::input_tokens` | 所有输入 Token；该端点只报告一个总数。 |
| 未知键 | `extras.fields` | |

## 流

Schema `MessageStreamEvent`。

| SSE 事件 | KPP | 说明 |
|---|---|---|
| `message_start` | `ResponseStart { id, model, usage }` | `message` 的其他键（`type`、`role`、`content: []`、`stop_reason`、`stop_sequence`、`stop_details` 等）→ fields `message.*`。 |
| `content_block_start` `text` | `BlockStart(Text)` | 初始文本和引用 → 带 form `in_start` 的 `BlockDelta`。 |
| `content_block_start` `tool_use` | `BlockStart(ToolCall { id, name })` | ID 和名称映射回原值；非空的初始 `input` → 带 `in_start` 的 `ToolArguments` 增量。 |
| `content_block_start` `thinking` | `BlockStart(Reasoning { origin })` | 初始思考和签名 → 带 `in_start` 的增量。 |
| `content_block_start` `redacted_thinking` | `BlockStart(Reasoning { opaque: [AnthropicRedactedThinking] })` | |
| 其他类型的 `content_block_start` | `BlockStart(Unknown)`（raw） | |
| 与默认值不同的起始块（`{type:text,text:""}`、`{type:tool_use,id,name,input:{}}`、`{type:thinking,thinking:"",signature:""}`、`{type:redacted_thinking,data}`） | form `content_block`（原样的块） | |
| 与 KPP 索引不同的 `content_block_start.index` | form `index` | Anthropic 按 0、1、…… 为块编号，所以这只会出现在不寻常的流中。 |
| `text_delta` | `Delta::Text` | |
| `input_json_delta`（`partial_json`） | `Delta::ToolArguments(partial_json)` | 一个块的 `partial_json` 片段拼接起来就是参数。在 `Unknown` 块（例如 `server_tool_use`）上：`Delta::Unknown`（raw）。 |
| `thinking_delta` | `Delta::ReasoningText` | |
| `signature_delta` | `Delta::Opaque(AnthropicThinkingSignature)` | |
| `citations_delta` | `Delta::Citation` | |
| 未知的增量类型 | `Delta::Unknown`（raw） | |
| `content_block_stop` | `BlockStop` | |
| `message_delta.delta.stop_reason` / `.stop_sequence` | `ResponseDelta.stop` | 不寻常的写法 → form `delta:spelling:*`。 |
| `message_delta.delta.stop_details`、`.container`、未知键 | fields `delta.*` | |
| `message_delta.usage`（`MessageDeltaUsage`） | `ResponseDelta.usage`（累计值） | 它省略的计数以及缓存写入的细分数据会从 `message_start` 补全；`raw` 是该增量自己的对象。 |
| `message_stop` | `ResponseStop` | |
| `error` | `StreamEvent::Error`（类型按[错误表](#错误)确定，没有状态码） | 解码器停止。 |
| `ping`、未知事件 | `StreamEvent::Extra { event, data }` | 仅限同一方言。不是 JSON 的数据按文本保留。帧的 `id`、`retry` 和其他 SSE 字段保留在该 extra 中。 |
| 没有 `data:` 行的帧（例如单独的 `retry: 3000` 或 `event: ping`），无论事件名称是什么 | 没有 `data` 键的 `StreamEvent::Extra` | 按 SSE 规范它不是事件，因此永远不会成为错误或生命周期事件。写回时不带 `data:` 行。 |
| SSE 注释 | `StreamEvent::Extra { comment }` | |
| 与数据中 `type` 不同的 `event:` 名称；`id:`、`retry:` 及其他 SSE 字段 | form `event` / `sse` | `sse` 为 `{"id", "retry", "other": [[name, value], …]}`，并写回到同一帧上。包含 NUL 的 `id`，以及不是可放入 `u64` 的 ASCII 数字的 `retry`，按规范不算 `id` 或 `retry`；它们与其他字段一起保留（[服务器发送事件](index.md#服务器发送事件)）。 |
| 顺序错误：在 `message_start` 之前出现块或消息事件、第二个 `message_start`、`message_stop` 之后的生命周期事件、块未关闭时的 `message_stop`、同一块索引开始两次、针对从未开始或已经结束的块的增量或停止事件、出现在错误类型块上的已知增量类型（例如 `tool_use` 上的 `text_delta`；未知块上的任何增量都可以）、数据中没有 `type` 的已知事件 | 最终的 `StreamEvent::Error`（`ProviderError`）；之前的事件会保留 | `ping`、注释和未知事件可以出现在 `message_stop` 之后。 |
| 没有 `message_stop` 或在块中间结束的流；数据不是 JSON 的已知事件 | 最终的 `StreamEvent::Error`（`ProviderError`） | Anthropic 索引中的空缺不算错误：块按开始顺序获得 KPP 索引（form `index`）。 |

编码来自其他方言的事件：

- **一次一个块。** 交错的块按开始顺序缓冲，`ResponseDelta` 或 `ResponseStop` 会关闭所有打开的块。因此，一个直到结束才关闭文本块的 OpenAI 流，会在最后才送出它的工具调用。多个外来的 `ResponseDelta` 会合并为一个 `message_delta`。
- **缺少 `ResponseStart`** 时，会生成一个 ID 为空、模型为请求中模型的 `message_start`。
- **拒绝回复**变为文本块。
- **工具调用参数**：来自其他方言的参数会被收集，在块结束时作为一个 `input_json_delta` 写出，但前提是它们构成一个 JSON 对象；否则不写出，输入保持为 `{}`。这与回复一样是有损的。Anthropic 自己的流按收到时的原样重放。
- **推理块**：来自其他方言的推理块变为 `thinking` 块（[推理数据块](cross-format.md#推理数据块)）：摘要和推理文本作为 `thinking_delta`，各部分之间有一个空行；不透明增量被收集起来，块结束时写出一个 `signature_delta`，其中是整个块的推理数据块。
- **省略的内容**（[回复](cross-format.md#回复)）：外来的引用增量和未知增量、未知块以及 `Extra` 事件，与完整回复一致。在它们到达期间，编码器会按节奏写出 `event: ping` 帧（`{"type": "ping"}`，与 Anthropic 发送的相同）：每个外来的额外事件一帧，每 1,024 字节隐藏文本一帧（[保活消息](cross-format.md#保活消息)）。`StreamEncoder::keepalive` 会把同样的帧交给网关。
- **停止原因：**`StopReason::Other` 的映射方式与回复相同。
- **错误：**KPP `Error` 变为 `event: error`，其内容与[错误](#错误)中的响应体相同：从 Anthropic 解码的错误按收到时的原样写出，其他错误为 `{"type":"error","error":{type,message}}`。

## 错误

Schema `ErrorResponse`。解码错误（无论是请求还是服务商的响应体）都会注明字段及其问题，而不是解析器的位置。

| HTTP | `error.type` | `ErrorKind`（解码） | 由 `ErrorKind` 编码 |
|---|---|---|---|
| 400 | `invalid_request_error` | `InvalidRequest`；消息以 `prompt is too long` 开头时为 `ContextTooLong` | `InvalidRequest`、`ContextTooLong`、`ContentFiltered`、`Unsupported` |
| 401 | `authentication_error` | `Authentication` | `Authentication` |
| 402 | `billing_error` | `Billing` | `Billing` |
| 403 | `permission_error` | `PermissionDenied` | `PermissionDenied` |
| 404 | `not_found_error` | `NotFound` | `NotFound` |
| 409 | `conflict_error` | `InvalidRequest`（保留状态码 409） | 无 |
| 413 | `request_too_large` | `RequestTooLarge` | `RequestTooLarge` |
| 429 | `rate_limit_error` | `RateLimited` | `RateLimited` |
| 500 | `api_error` | `ProviderError` | `ProviderError`（保留 5xx 状态码）、未知类型 |
| 504 | `timeout_error` | `Timeout` | `Timeout` |
| 529 | `overloaded_error` | `Overloaded` | `Overloaded` |

- `error.message` → `message`；整个响应体（含 `request_id` 和 `error.details`）→ `raw`；HTTP 状态码 → `status`；`retry-after-ms` 或 `retry-after` → `retry_after`，写回时为 `retry-after`，向上取整为整秒。
- 提示过长与其他错误一样是 400 `invalid_request_error`，只有消息能区分它。如果 Anthropic 改变措辞，该错误会被解码为 `InvalidRequest`。
- 未知的 `error.type`，或不是 `ErrorResponse` 的响应体（例如 Cloudflare 的 HTML 413、`type` 不是 `error` 的 JSON，或顶层带有 Kaeso 无法承载的键的 JSON），按状态码确定类型：400、409 和 422 为 `InvalidRequest`，401 为 `Authentication`，402 为 `Billing`，403 为 `PermissionDenied`，404 为 `NotFound`，408 和 504 为 `Timeout`，413 为 `RequestTooLarge`，429 为 `RateLimited`，503 和 529 为 `Overloaded`，其他任何状态码以及没有状态码的流错误为 `ProviderError`。文档中的 `error.type` 总是优先于状态码。状态码和原始响应体（不是 JSON 时以字符串保存）都会保留，这类响应体会以 Anthropic 的错误结构重新编码。
- 从 Anthropic 解码的错误会以其状态码从 `raw` 重新编码，因此 Anthropic 应用看到的错误不变；其他错误则构建为 `{"type":"error","error":{"type","message"}}`。
- 其他响应头（例如 `anthropic-ratelimit-*` 和 `request-id`）不属于 `KppError`。

## 模型

`GET /v1/models` 和 `GET /v1/models/{model_id}`。代码：`anthropic_messages/models.rs`；fixture：`crates/kaeso-codecs/fixtures/anthropic_messages/models/`。来源：[list models](https://platform.claude.com/docs/en/api/models/list.md) 和 [get a model](https://platform.claude.com/docs/en/api/models/retrieve.md) 页面，SDK 的 `types/model_info.py`、`model_list_params.py`、`model_capabilities.py` 和 `model_line.py`，以及规范中的 `ModelInfo` 和 `ListResponse_ModelInfo_`。规范中的结构没有 `lifecycle`、`deprecated_at`、`retires_at` 和 `line`，也没有 `lifecycle` 查询参数；以文档和 SDK 为准，编码器原本会填入默认值的每个字段都按可选字段解码，并记录其缺失（form `none`），因此两种结构都能往返。应用的请求带有 `anthropic-version` 时被视为 Anthropic 请求（`dispatch::models_dialect`，见[模型列表](index.md#模型列表)）。

### 查询参数

`decode_models_query` 将查询字符串转换为 `ModelsQuery`。

| 参数 | KPP | 说明 |
|---|---|---|
| `limit` | `limit` | 只能是数字（不带符号），1 到 1000，只出现一次；其他情况 → `InvalidRequest`（参数 `limit`）。缺失时，网关会把文档规定的默认值 20 传给 `ModelList::page`。 |
| `after_id`、`before_id` | `after_id`、`before_id` | 非空，各出现一次；两者同时出现 → `InvalidRequest`（参数 `before_id`）。不在经过过滤和去重的列表中的 ID → `ModelList::page` 返回 `InvalidRequest`（参数 `after_id` 或 `before_id`），而不是返回一个空页，那样会悄无声息地结束应用的分页；被 `lifecycle` 过滤掉的游标也视为未知。 |
| `lifecycle[]`（SDK 发送的形式：`Querystring(array_format="brackets")`）、`lifecycle`（重复出现） | `lifecycle` | `active`、`deprecated`、`retired`；最多 3 个值（`maxItems: 3`）。其他任何值，包括逗号分隔的 `active,deprecated` → `InvalidRequest`（参数 `lifecycle`）。空列表 → `ModelList::page` 的默认值（`active` 和 `deprecated`）。 |
| 任何其他参数（例如 `client.beta.models` 发送的 `beta=true`） | extras `query.<name>` | 一个字符串，重复出现时为字符串数组。 |

名称和值都会进行百分号解码（`+` 表示空格）；转义格式错误或结果不是 UTF-8 → `InvalidRequest`。路径段 `{model_id}` 同样以百分号编码的形式到达；由网关的路由器解码，因为编解码器从不接触路径。请求头（`anthropic-beta`、`anthropic-workspace-id`）不属于查询。

### 模型信息

wire 上的 `ModelInfo` 与 KPP 中的 `kaeso_kpp::ModelInfo`。

| Wire 字段 | KPP | KPP 没有值时编码为 |
|---|---|---|
| `id` | `id` | （必需） |
| `type`（`"model"`） | form `type`，仅在不同时 | `"model"` |
| `display_name` | `display_name` | `id` |
| `created_at`（RFC 3339） | `created_at`（Unix 秒） | `"1970-01-01T00:00:00Z"`：文档说明，如果发布日期未知，`created_at` 可能被设为纪元时间 |
| `lifecycle` | `lifecycle`（文档未列出的值为 `Other(s)`） | `"active"`（文档规定的默认值） |
| `deprecated_at`、`retires_at`（RFC 3339 或 `null`） | `deprecated_at`、`retires_at` | `null` |
| `max_input_tokens`、`max_tokens`（或 `null`） | `context_window`、`max_output_tokens` | `null` |
| `capabilities`（`ModelCapabilities` 或 `null`） | extras 字段 `capabilities` | `null` |
| `line`（`ModelLine` 或 `null`） | extras 字段 `line` | `null` |
| 未知键 | extras 字段 | |

`capabilities` 和 `line` 之所以放在 extras 中，是因为 `ModelCapabilities` 是一棵庞大且增长迅速的 Anthropic 功能名称树（批处理、引用、推理强度级别、思考类型、服务器工具、上下文管理策略），在其他方言中没有对应项；而 `line` 是 Anthropic 的产品系列（`opus`、`sonnet` 等），文档明确说不要从 `id` 推断。KPP 只会为 Anthropic 应用透传它们，而 extras 已经逐字节地做到了这一点。

**日期。** RFC 3339 的 `date-time` 会转换为 Unix 秒（应用时区偏移，截去秒的小数部分，`T` 和 `Z` 大小写均可），并写回为 `YYYY-MM-DDTHH:MM:SSZ`。如果收到的写法无法以这种方式还原（`.117Z`、`-07:00`、`t`），就保存在 form `spelling` 中，只要它仍表示同一秒，就会再次写出。无法解析的日期和 1970 年之前的日期 → `ProviderError`。任何一分钟内都接受 `:60` 秒；它被视为下一分钟的第一秒，并保留其写法。`9999-12-31T23:59:59Z` 之后的 KPP 时间无法用年份为四位数的 RFC 3339 表示；当兼容 OpenAI 的 `created` 以毫秒为单位时就会出现这种情况。此时 `created_at` 取纪元时间，即文档规定的未知值，`deprecated_at` 和 `retires_at` 取 `null`。

**缺失或 `null`。** 收到时缺失的字段，或在上述默认值为某个值的地方收到 `null` 的字段，会记录在 form `none` 中并以同样的方式写出，因此没有 `lifecycle` 的较旧规范结构也能往返。Anthropic 模型上缺失的 `capabilities` 和 `line` 仍保持缺失。

### 模型列表

wire 上的 `ListResponse[ModelInfo]` 与 KPP 中的 `ModelList`。

| Wire 字段 | KPP | 编码为 |
|---|---|---|
| `data` | `models` | |
| `has_more` | `has_more` | 未设置时为 `false`（缺失或 `null` 记录在 form `none` 中） |
| `first_id`、`last_id` | 由 `models` 推导 | 第一个和最后一个模型 ID，空页时为 `null`。收到的值若与推导出的值不同（文档示例发送的是占位符 `"first_id"` 和 `"last_id"`），会连同收到它时对应的 ID 一起保存在 form `first_id` 或 `last_id` 中，并且只有当页面仍以该模型开头或结尾时才会再次写出。 |
| 未知键 | extras 字段 | |

对 Anthropic 应用有损的部分：`owned_by`（OpenAI）在这里没有对应字段，来自其他方言的 extras 会被省略（[回复](cross-format.md#回复)）。来自其他方言的模型会得到上述默认值（`display_name` 为 ID，`lifecycle: "active"`，`capabilities: null`，`line`、`max_*` 和日期为 `null`）。

在出站一侧，`decode_models` 和 `decode_model` 接收服务商的响应；非 2xx 状态码交给 `decode_error`。解码出的 extras 带有来源 `anthropic_messages`，但没有服务商，因为这些函数不接收 `EgressCtx`。

## 往返

没有例外：每个请求、Token 计数请求、回复、Token 计数回复、模型列表、流和错误的 fixture 都能往返（`tests/`、`models_tests.rs`），而且每个流 fixture 在解码再编码之后，累积起来也与其配对的回复一致。
