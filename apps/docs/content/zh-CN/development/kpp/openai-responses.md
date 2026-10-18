---
title: OpenAI Responses
description: OpenAI Responses 格式的每个字段如何映射到 KPP，又如何映射回来。
order: 5
---

- **编解码器：**`kaeso_codecs::openai_responses::OpenAiResponses`（`crates/kaeso-codecs/src/openai_responses/`）
- **端点：**`POST /v1/responses`（JSON 和 SSE）、`POST /v1/responses/input_tokens`，以及所有 OpenAI 编解码器共用的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_responses/`

**来源。** OpenAI OpenAPI 规范的提交 `e95c0fe615f45a19e8878af923a729948445a6bd`（下文的 schema 名称出自这里）、[创建接口参考](https://developers.openai.com/api/reference/resources/responses/methods/create)、[流式事件参考](https://developers.openai.com/api/reference/resources/responses/streaming-events)、[输入 Token 计数参考](https://developers.openai.com/api/reference/resources/responses/subresources/input_tokens/methods/count)以及[错误码指南](https://developers.openai.com/api/docs/guides/error-codes)。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **Extras**：所指元素的 `Extras::fields`，绑定到 `openai_responses`。它们只在同一方言内有效：其他方言会以 `Unsupported` 拒绝（参见 [Extras](cross-format.md#extras)）。嵌套的键使用键路径（`text.verbosity`、`summary.0.x`）。
- **形式**：编解码器私有的 `Extras::form` 形式标记。它只关乎呈现方式，其他方言会忽略它。
- **Unsupported**：解码或 `check()` 以 `ErrorKind::Unsupported { feature }` 失败。

与其他编解码器不同，这个编解码器不使用 `wire::Ext`；下一节说明它如何以另一种方式保留原始写法。

## 原始写法如何保留

每个 wire 对象都有一个*规范*编码，它只由类型化的 KPP 字段和[形式标记](#形式标记)中列出的结构性形式标记构成。来自其他方言的请求和回复得到的正是这种编码。解码时，编解码器为解码出的内容构建规范编码，并把它与收到的 JSON 之间的每一处差异，作为覆盖项记录在该对象所属 KPP 元素的 extras 中：

- `fields["<path>"]`：规范编码不写出、或以另一个值写出的键。包括未知键，以及 KPP 不建模的已知键（`metadata`、`text.verbosity`、`detail: "low"` 等），逐字节保留。下文表格中称为 **Extras**。
- `form["set"]["<path>"]`：针对呈现方式的同类记录。包括 `null` 值、显式写出的默认值（`store: false`、`background: false`、`stream: false`、`tools: []`、`include: []`、`metadata: {}`、`text: {}`、`detail: "auto"`、`truncation: "disabled"`、`service_tier: "auto"`、`text.format: {type: text}`、`text.verbosity: "medium"`、`stream_options: {}`、`stream_options.include_obfuscation`、空的 `annotations`/`logprobs`/`summary`/`content`），键 `type`、`id`、`item_id`、`status`、`role`、`object`、`filename`、`obfuscation`、`sequence_number`，以及与规范值只有写法不同的数字（规范编码写 `1.0` 而原文是 `1`）。下文表格中称为**形式**。
- `form["unset"]`：规范编码会写出、但原始内容中没有的路径。

只有当 extras 来自 `openai_responses` 时，编码才会应用这些覆盖项。类型化字段始终优先。值按原样精确比较，数字也不例外（`1` 和 `1.0` 不同）。对于规范编码能还原的值，从不记录覆盖项，并有一个测试检查没有任何类型化的键被存为 extras 字段。每个覆盖项还会记住解码时规范编码在其路径上的值（`form["canon"]`）。如果某个类型化字段在解码后发生了变化（网关设置了 `max_output_tokens`、向只有 prompt 的请求添加了消息、设置了推理强度等），该路径上的规范值就会不同，过时的覆盖项随即被跳过。因此，写作 `1` 的数字只要值仍为 1，就会以 `1` 写回；值改变后则写出新值。

只有在规范编码于该路径上也有 `type` 时（part 和 item 类型的写法），以及输入消息中可选的 `type: "message"`，`type` 才只关乎呈现方式。规范编码中不存在、且带有其他 `type` 的对象（例如 `text.format: {type: "grammar", …}`）会整体保存为一个 extras 字段，因此其他方言会拒绝它。

键路径无法容纳为空、含有 `.` 或以 `header:`、`@`、`~` 开头的键，因为这些写法分别属于键路径、转发的请求头（`header:openai-beta`）和带作用域的覆盖项。含有这类键的对象会整体比较：如果它与规范编码不同，整个对象就作为一个 extras 字段存放在它自己的路径上（顶层的键则是整个请求体，路径为空）。这类键保留在请求体中：它们绝不会作为请求头发送，其他方言会像对待任何 extras 字段一样拒绝它们。与所有覆盖项一样，该字段只在其路径上的规范值不变时生效：如果该对象内的某个类型化字段发生变化（对于顶层的键，即请求的任何类型化字段，例如 `model`），该对象会按规范写出，这些键随之省略。

## 请求：`CreateResponse` / `TokenCountsBody`

| Wire 字段 | KPP | 说明 |
| --- | --- | --- |
| `model` | `model` 类型化 | 对 `/v1/responses` 是必填项；对 `/v1/responses/input_tokens` 是可选项（缺失时 KPP `model` 为空，编码时再次省略）。 |
| `input`（字符串） | 一条 `Role::User` 消息，含一个文本块，类型化 | 形式 `input: "string"`。其他方言总是得到 item 列表。 |
| `input`（item 列表） | `system` + `messages` 类型化 | 参见[输入项](#输入项inputitem)。没有 `input`（配合 `prompt` 时允许）：没有消息；形式 `unset input`。 |
| `instructions` | `system[0]` 文本块，类型化 | 来自其他方言时，单个文本块变为 `instructions`；多个文本块变为一个开头的 `developer` 消息 item，其中包含 `input_text` part。这样保留了块的边界，但不再区分 `instructions` 和系统消息（API 把两者都视为开发者指令）。`null`：形式。 |
| `tools[]` `{type: function, name, description, parameters, strict}`（`FunctionTool`） | `ToolDef::Function` 类型化（`parameters` 为原始 JSON） | 其他键（`defer_loading`、`allowed_callers`、`output_schema` 等）是该工具的 extras。来自其他方言的工具上的缓存提示会被丢弃（参见[缓存提示](cross-format.md#缓存提示)）。来自其他方言、不符合 `^[a-zA-Z0-9_-]{1,64}$` 的名称会以确定的方式改写（其他字符变为 `_`，再追加 `_` 和 16 位十六进制的 FNV-1a 哈希，总长不超过 64 个字符），记录在 `EgressCtx::tool_names` 中，并在解码时映射回原值。 |
| `tools[]` 其他类型（`web_search`、`file_search`、`mcp`、`custom` 等） | `ToolDef::Unknown` | 仅限同一方言。 |
| `tool_choice` `"auto"` / `"none"` / `"required"` | `ToolChoice::Auto` / `None` / `Required` 类型化 | |
| `tool_choice` `{type: function, name}` | `ToolChoice::Tool(name)` 类型化 | 带有其他键时：`ToolChoice::Unknown`。 |
| `tool_choice` `allowed_tools`、托管工具、`mcp`、`custom`、其他字符串 | `ToolChoice::Unknown` | 仅限同一方言。 |
| `parallel_tool_calls` | `parallel_tool_calls` 类型化 | |
| `text.format` `{type: text}` | `output = Text` | 省略是规范形式；显式写出的 `{type: text}` 是形式。 |
| `text.format` `{type: json_object}` | `output = JsonObject` 类型化 | |
| `text.format` `{type: json_schema, name, description, schema, strict}` | `output = JsonSchema` 类型化（`schema` 为原始 JSON） | 其他键：`JsonSchemaSpec` 的 extras。没有名称的 schema（Anthropic）以 `name: "response"` 发送。 |
| `text.format` 其他类型 | extras `text.format` | |
| `text.verbosity` | `"medium"` 是默认值，GPT-5 和 Codex 客户端每次请求都会发送：作为形式，这样此类请求可以发往其他方言。`low`/`high` 会改变输出：extras `text.verbosity`，其他方言会拒绝（参见 [Extras](cross-format.md#extras)）。 | `null`：形式。 |
| `reasoning.effort` | `reasoning.effort` 类型化 | 来自其他方言时（参见[推理](cross-format.md#推理)）：显式推理强度原样使用；Anthropic `thinking.disabled` → `none`；预算 → `low`（低于 4,096）、`medium`（4,096 到 16,383）、`high`（16,384 及以上），与[推理强度与预算](cross-format.md#推理强度与预算)一致。取值从 `none` 到 `max`；未知值为 `ReasoningEffort::Other`。 |
| `reasoning.summary` | `reasoning.summary` 类型化 | `auto`/`concise`/`detailed`/其他。开启了思考、且 `display` 不是 `omitted` 的 Anthropic Messages 请求会得到 `summary: "auto"`，因为 OpenAI 只有在被要求时才返回可读推理（参见[可读推理与摘要](cross-format.md#可读推理与摘要)）。不适用于 `/v1/responses/input_tokens`。OpenAI 要求部分组织先完成验证才能获得推理摘要；对于这类组织的服务商密钥，OpenAI 的错误会原样传给应用。 |
| `reasoning.mode`（`standard`/`pro`）、`reasoning.context`、`reasoning.generate_summary`（已弃用） | `ReasoningConfig` 的 extras | 与 KPP 的 `ReasoningMode`（Anthropic 思考的开或关）不是同一概念。 |
| `reasoning` `{}` | `reasoning` = 空配置 | 形式 `object`。 |
| `include[]` `"reasoning.encrypted_content"` | `reasoning.include_encrypted` 类型化 | 没有 `reasoning` 对象时：配置上的形式 `no_object`。来自其他方言的请求只要（经推理强度换算后）发送了 `none` 以外的推理强度，就总会请求它；当 `EgressCtx::reasoning = Some(true)` 时，没有推理强度的 Anthropic `adaptive` 或 `enabled` 思考也会请求它（参见[可读推理与摘要](cross-format.md#可读推理与摘要)）。在 `store: false` 下，这是在下一轮交还推理的唯一途径（参见[推理数据块](cross-format.md#推理数据块)）。 |
| `include[]` 其他值 | extras `include`（整个列表） | `message.output_text.logprobs`、`web_search_call.action.sources` 等。 |
| `max_output_tokens` | `sampling.max_output_tokens` 类型化 | `CreateResponse` 规定最小值为 16：更小的值为 `InvalidRequest`（`` The request body is not a valid OpenAI Responses request: `max_output_tokens` must be at least 16. ``，参数 `max_output_tokens`）；`TokenCountsBody` 没有该字段。来自其他方言、`sampling.max_output_tokens` 更小的请求在 `check()` 中为 `Unsupported`，从不调高，并注明应用自己的字段，例如 `max_completion_tokens`（[采样参数](cross-format.md#采样参数)）。 |
| `temperature`、`top_p` | `sampling.temperature`、`sampling.top_p` 类型化 | 规范写法是浮点数（`1.0`）；`1` 是带 `canon` 的形式 `set`（参见[原始写法如何保留](#原始写法如何保留)），因此只要值未变，就以 `1` 写回。其他方言得到 `1.0`。 |
| `safety_identifier` | `user` 类型化 | 来自其他方言的 KPP `user` 以 `safety_identifier` 发送。 |
| `user`（已弃用） | 没有 `safety_identifier` 时为 `user` 类型化（形式 `user_key`）；否则为 extras `user` | |
| `stream: true` | `StreamMode::On { include_usage: true }` | Responses 流总是报告用量。`false`/`null`：形式。 |
| `stream_options` | `include_obfuscation`：形式；其他键：extras | 混淆字段只是填充，不是内容。 |
| `store` | `false`/`null`：形式；`true`：**Unsupported**（`store`，参见[有状态的 Responses 功能](cross-format.md#有状态的-responses-功能)） | 来自其他方言的请求以 `store: false` 发送（参见 [Responses 的存储与 ID](cross-format.md#responses-的存储与-id)）。本方言的请求保留原有写法，因此省略 `store` 仍意味着 OpenAI 会存储该响应。形式 `native` 标记从本方言解码的请求。 |
| `previous_response_id`、`conversation` | `null`：形式；任何值：**Unsupported**（参见[有状态的 Responses 功能](cross-format.md#有状态的-responses-功能)） | |
| `background` | `false`/`null`：形式；`true`：**Unsupported**（参见[有状态的 Responses 功能](cross-format.md#有状态的-responses-功能)） | |
| `metadata`、`truncation`、`service_tier`、`top_logprobs`、`max_tool_calls`、`prompt_cache_key`、`prompt_cache_retention`、`prompt_cache_options`、`prompt`、`context_management`、`moderation`、`access_programs`、`personality`（仅计数）、未知键 | extras | 显式写出的默认值和 `null` 是形式（参见[原始写法如何保留](#原始写法如何保留)）。 |
| 请求头 `openai-beta` | extras `header:openai-beta` | 只转发给 Responses。 |

请求体不是 JSON 对象、含有重复的键、缺少 `model`（针对 `/v1/responses`）或某个已知字段类型错误时，结果是 `InvalidRequest`，并指明该字段（`` The request body is not a valid OpenAI Responses request: `max_output_tokens` must be a non-negative integer. ``，同时设置 `param`），从不给出解析器位置。

对于来自任何方言的请求，`check()` 还会拒绝：`sampling.top_k`、`seed`、`frequency_penalty`、`presence_penalty` 和停止序列，Responses API 没有这些参数（参见[采样参数](cross-format.md#采样参数)）；未知的推理模式，以其 wire 值命名；文本文档、文档的 `title`/`context`/`citations`，以及文本形式的图像来源；以及 `cross::check_foreign` 拒绝的一切（外来 extras、未知的块、工具和工具选择、外来文件 ID、外来引用；参见[请求](cross-format.md#请求)）。无法重放的推理块会被丢弃（参见[推理](cross-format.md#推理)），来自其他方言的缓存提示也会被丢弃（参见[缓存提示](cross-format.md#缓存提示)）。`ToolResult::is_error` 同样会被丢弃：它是提示字段，失败情况由内容本身描述（参见[工具错误](cross-format.md#工具错误)）。来自其他方言的推理配置以其推理强度发送：显式推理强度原样使用，预算按[推理强度表](cross-format.md#推理强度与预算)换算，`disabled` 作为 `none`；既没有推理强度也没有预算的 `adaptive` 或 `enabled` 发送时不带推理强度，模型因此使用默认推理强度。Embeddings 请求是 `Unsupported`。

### 输入项（`InputItem`）

| Wire | KPP | 说明 |
| --- | --- | --- |
| 开头的 `system` / `developer` 消息 item，只含文本，且除 `type`/`role` 外没有其他 item 数据 | `GenerateRequest.system` 的块 | 请求上的形式 `system_items`（`[{n, role, type, parts}]`）和 `instructions` 记录拆分方式。带有 `id`、`status`、图像或未知键的开头 item 仍是一条 `Role::System` 消息。 |
| `message` item，角色 `user` / `assistant` | `Message { role: User / Assistant }` 的内容块 | 支持 `EasyInputMessage`、`InputMessage` 和 `OutputMessage` 三种结构。item 的 `type`、`id`、`status` 和 `role` 的写法：消息上的形式；`phase` 和未知键：消息的 extras。一条 KPP 消息最多包含一个消息 item。 |
| `message` item，角色 `system` / `developer`（不在开头） | `Message { role: System }`（参见[系统消息](cross-format.md#系统消息)） | 规范角色为 `developer`；`system`：形式。 |
| `content` 为字符串或 part 列表 | 消息上的形式 `parts` | 对单个纯文本块而言，字符串是规范写法。空的 part 列表：形式 `empty_at`。 |
| `function_call`（`FunctionToolCall`） | 当前助手消息中的 `Block::ToolCall` | `call_id` → `id`，`name`，`arguments`（一个 JSON 字符串）→ 原始 JSON。不是 JSON 的参数按 KPP 约定保存为 JSON 字符串，并带形式 `args_string`；来自其他方言、本身是 JSON 字符串的 `arguments` 值以其文本发送。`id`、`status`：形式；`namespace`、`caller`、未知键：该块的 extras。来自其他方言、超过 64 个字符的调用 ID 会缩短为前 55 个字符加 `_` 和 FNV-1a 哈希的 8 位十六进制数，记录在 `EgressCtx::tool_ids` 中，并在解码时映射回原值。 |
| `function_call_output`（`FunctionCallOutputItemParam`） | 当前用户消息中的 `Block::ToolResult` | `call_id`；`output` 字符串 → 一个文本块，part 列表 → 文本、图像和文档块（本可用字符串表示时带形式 `parts`）。`id`、`status`：形式；`name`、`namespace`、未知键：extras。来自其他方言的 `is_error` 会被丢弃。来自其他方言的空内容：`output: ""`。 |
| `reasoning`（`ReasoningItem`） | 当前助手消息中的 `Block::Reasoning`（来源 `openai_responses`） | Kaeso 生成的 ID（`rs_kaeso_…`）不会保留（形式 `kaeso_id`）；这类 item 如果没有 `encrypted_content`，根本不会发给服务商，因为它既没有 `id`，也没有 OpenAI 能用的内容。`id` → 不透明的 `OpenAiReasoningItemId`，`encrypted_content` → 不透明的 `OpenAiEncryptedReasoning`，`summary[].text` → `summary`，`content[].text` → `text`；`status` 和各 part 的 `type`：形式；未知键：extras。只重放给同一服务商的 `openai_responses`（参见[推理](cross-format.md#推理)）。如果 item 的 `encrypted_content` 是一个推理数据块（`kpp:reasoning:v1:…`，即 Kaeso 交给应用的其他方言的推理，参见[推理数据块](cross-format.md#推理数据块)），无论 item 其余部分写了什么，都会解码为其中的原始块，连同来源（例如 Claude 的签名思考）。在 Kaeso 以 `store: false` 发送的请求中（来自其他方言的请求，参见 [Responses 的存储与 ID](cross-format.md#responses-的存储与-id)），没有加密内容的推理不会发送，因为 OpenAI 无法解析该 item ID。 |
| `item_reference` 及所有其他 item 类型（服务端工具的调用与输出、`compaction`、未来的类型） | 一条只含一个 `Block::Unknown` 的消息，形式 `raw_item` | 以 `_call` 结尾的类型以及 `mcp_list_tools`、`mcp_approval_request` 的角色为 `Assistant`；其余为 `User`。角色只对 KPP 校验有意义，因为这些 item 永远不会发往其他方言。仅限同一方言。 |
| 无法按其类型解析的 item（例如 `call_id: null` 的 `function_call_output`） | `Block::Unknown`（同上） | 无损保留；由服务商判断。 |

分组：连续的 `function_call` 和 `reasoning` item 并入当前助手消息，连续的 `function_call_output` item 并入当前用户消息，消息 item 则并入同一角色、尚未包含消息 item 的消息；system 和 developer item 从不并入已有消息。编码时，每条 KPP 消息会再拆开：每一段连续的内容块成为一个消息 item，其他每个块各成为一个 item。来自其他方言的消息，例如 Anthropic 的 `[thinking, text, tool_use, tool_use]`，会变成一个 `message` item 和两个 `function_call` item。其中的思考会被丢弃（参见[推理](cross-format.md#推理)），除非它是 Anthropic 应用放在[推理数据块](cross-format.md#推理数据块)中带回来的 OpenAI 推理。

消息 item 和函数调用 item 的 `id` 与 `status`（`msg_…`、`fc_…`）只关乎呈现方式，因此一段回传了输出 item 的对话仍然可以发往其他服务商。推理 item 的 ID 是语义字段（不透明的 `OpenAiReasoningItemId`），因为 OpenAI 会解析它们。

### 内容 part

| Wire part | KPP | 说明 |
| --- | --- | --- |
| `input_text`（用户、系统、开发者、工具输出） | `Block::Text` | |
| `output_text`（助手） | `Block::Text`，`annotations[]` → `citations`（原始 JSON，绑定来源） | 请求中的规范写法：`{type, text, annotations}`；输入历史中的 `logprobs`：extras。不是该角色默认的那种文本类型：形式。 |
| `refusal` | `Block::Refusal` | |
| `input_image` `image_url`（data URL `data:<type>;base64,…`） | `Block::Image`，`Source::Base64` | 带其他参数的 data URL 仍为 `Source::Url`。 |
| `input_image` `image_url`（URL）/ `file_id` | `Source::Url` / `Source::FileId`（绑定来源） | 两者都有或都没有：`Block::Unknown`。图片文件 ID 只能用于同一服务商的 OpenAI Responses，因为 OpenAI Chat Completions 不接受图片文件 ID。 |
| `input_image.detail`、`input_file.detail` | `"auto"`：形式；其他值：extras `detail` | 在图像上，`detail` 是 [Extras 允许列表](cross-format.md#extras)中的分辨率提示，发往其他方言时会被丢弃。在文件上，它控制 PDF 的渲染方式，其他方言会拒绝它。 |
| `input_file` `file_data` / `file_url` / `file_id` | `Block::Document`，`Source::Base64` / `Url` / `FileId` | 必须恰好有一个来源，且 `file_data` 必须是 data URL；否则为 `Block::Unknown`。 |
| `input_file.filename` | 形式 | KPP 没有文件名字段（`DocumentBlock::title` 是标题），这样 PDF 才能跨方言传递。OpenAI 要求 `file_data` 带有文件名：来自其他方言、以 `file_data` 发送的文档会得到 `filename: "document.<ext>"`，扩展名按媒体类型取 `pdf`、`txt`、`md`、`csv` 或 `json`，其他情况为 `bin`。 |
| 文本、图片或文件 part 上的 `prompt_cache_breakpoint`（`PromptCacheBreakpointConfig`） | 该块的 `cache: CacheHint`（形式 `breakpoint`） | 只会重新输出到本方言；来自其他方言的提示会被丢弃（参见[缓存提示](cross-format.md#缓存提示)）。 |
| 其他 part 类型 | `Block::Unknown` | |

## 回复：`Response`

| Wire 字段 | KPP | 说明 |
| --- | --- | --- |
| `id`、`model` | `Response.id`、`.model` 类型化 | 来自其他格式时，保留服务商的 ID 不变（[回复 ID](cross-format.md#回复-id)）。 |
| `status` `completed` | `stop = EndTurn`；当 `output` 含有 `function_call` 时为 `ToolUse` | |
| `status` `incomplete` + `incomplete_details.reason` `max_output_tokens` / `content_filter` / 其他 | `stop = MaxTokens` / `ContentFilter` / `Other(reason)` | 其他状态（`in_progress`、`cancelled`、`queued`）：`Other(status)`；确切的 `status` 和 `incomplete_details` 通过覆盖项还原。 |
| `status` `failed` + `error {code, message}` | `decode_response` 返回该错误 | 错误类型只取自 `error.code`（`ResponseErrorCode`），依据[错误表](#错误errorresponse)中的每个错误码以及表下列出的错误码，例如 `server_error` → `ProviderError`，`rate_limit_exceeded` → `RateLimited`，`vector_store_timeout` → `Timeout`，`invalid_prompt` → `InvalidRequest`，`cyber_policy` → `ContentFiltered`；其他错误码或缺少错误码 → `ProviderError`。没有消息时：`The provider reported that the response failed.` 整个对象保存在 `raw` 中。它不是错误响应体，因此 Responses 应用得到的是一个 `ErrorResponse`，带有该消息以及该错误类型对应的状态码、`type` 和 `code`。 |
| `output[]` `message`（角色 `assistant`） | 内容块（`output_text` → 带引用的 `Text`，`refusal` → `Refusal`，其他 part → `Unknown`） | item 边界：回复上的形式 `messages` = `[[第一个块, 块数], …]` 和 `blocks`（块数），另外每个解码出的块上都有形式 `item`（输出索引）。这些范围只在块列表未变时使用。如果网关删除或添加了块，就按块的 `item` 标记分组，这样 ID 和 item 覆盖项（作用域为 `@out<i>`）始终跟随其 item，也不会冒出多余的 item。part 的 `logprobs`：该块的 extras。 |
| `output[]` `function_call` | `Block::ToolCall` | `id`（`fc_…`）：块上的形式；发出时改写过的名称和 ID 会映射回原值。 |
| `output[]` `reasoning` | `Block::Reasoning` | 与输入项相同。 |
| `output[]` 其他类型（`web_search_call` 等） | `Block::Unknown` | |
| `usage`（`ResponseUsage`） | `usage` 类型化 | 按[用量](cross-format.md#用量)中的约定：`input_tokens` = wire 中的 `input_tokens` − `cached_tokens` − `cache_write_tokens`，因为 wire 中的 `input_tokens` 包含缓存读取和缓存写入；`cache_read_tokens` = `cached_tokens`；`cache_write_tokens`；`output_tokens`（包含推理）；`reasoning_tokens`；原对象存于 `raw`。编码时写出类型化字段，并叠加来自本方言的 `raw`。`null`：形式。 |
| `object`、`created_at`、`completed_at`、`error: null`、回显的请求参数（`instructions`、`tools`、`tool_choice`、`text`、`reasoning`、`temperature`、`top_p`、`store`、`truncation`、`metadata`、`service_tier` 等）、未知键 | 回复的 extras（覆盖项） | 形式 `native` 标记从本方言解码的回复。 |

来自其他方言的回复会写成一个完整的 `Response` 对象：

- `object: "response"`、`created_at`（`EncodeCtx::created_at`）、`background: false`、`completed_at`（完成时为同一时间，否则为 `null`）以及 `error: null`。
- 回显的请求参数取自应用的请求（`EncodeCtx::request`，按本编解码器发送时的方式编码），未设置的使用 OpenAI 的默认值：`instructions`、`max_output_tokens`、`max_tool_calls`、`prompt_cache_key`、`safety_identifier` 和 `user` 为 `null`，`parallel_tool_calls: true`，`reasoning: {effort, summary}`（未设置的为 `null`），`temperature: 1.0`，`top_p: 1.0`，`top_logprobs: 0`，`text: {format: {type: text}}`，`tool_choice: "auto"`，`tools: []`，`truncation: "disabled"`，`metadata: {}`。`previous_response_id: null`、`service_tier: "default"` 和 `store: false`（Kaeso 不存储任何内容）总会写出。
- 没有对应项的停止原因（`StopSequence`、`Refusal`、`PauseTurn`、`Other`）报告为 `completed`，这会丢失信息：Anthropic 的 `pause_turn` 要求应用继续，而 Responses 应用无从得知。回复为 `incomplete` 时，消息 item 也是 `incomplete`。
- 其他方言的推理变为一个推理 item（参见[推理数据块](cross-format.md#推理数据块)）：ID 为 `rs_kaeso_…`，其推理文本作为一个 `summary_text`，后接其摘要，原始块的推理数据块作为 `encrypted_content`。无论应用是否在 `include` 中请求，推理数据块都会发送，因为 Kaeso 不存储任何内容，而各 SDK 的 agent 循环会把输出 item 原样发回。
- 本方言无法呈现的回复数据会被省略（参见[回复](cross-format.md#回复)）：外来的 `Unknown` 块、外来的引用和 extras、图像、文档和工具结果。
- 消息 item 和函数调用 item 的 ID 为 `msg_kaeso_…` 和 `fc_kaeso_…`。请求解码时会识别这些由 Kaeso 生成的 ID，绝不会把它们发给服务商。

## 输入 Token 计数：`TokenCountsResource`

| Wire | KPP | 说明 |
| --- | --- | --- |
| `{object: "response.input_tokens", input_tokens}` | `CountTokensResponse.input_tokens` 类型化 | 其他键：extras。 |

请求是 `TokenCountsBody`，与 `CreateResponse` 一样解码为 `Request::CountTokens`；路径为 `/v1/responses/input_tokens`。`include`、`max_output_tokens`、`temperature`、`top_p`、`user`、`safety_identifier`、`stream` 和 `store` 不属于 `TokenCountsBody`。应用发送这些键时，它们保留为请求的 extras（仅限同一方言）；显式写出的默认值和 `null` 是形式，`store: true` 与 `/v1/responses` 一样会被拒绝。来自其他方言的 KPP 计数请求编码时不带这些键，因为它们不影响输入 Token 数。

## 流：`ResponseStreamEvent`

消息的每个内容 part、每个函数调用、每个推理 item 和每个其他 item 各对应一个 KPP 块。`sequence_number` 和 `obfuscation` 的偏差是形式。与数据中的 `type` 不同的 `event:` 名称是形式覆盖项 `~event`，已解码帧的 SSE 字段 `id`、`retry` 和未知字段是形式覆盖项 `~sse`（`{"id", "retry", "other": [[name, value], …]}`）；两者的作用域与事件相同，并会被写回。与规范事件的其他所有差异都是 extras 覆盖项，作用域由所附加的事件决定（`@open`、`@close`、`@done`、`@in_progress`、`@sdone`、`@spdone`、`@rdone`、`@rpdone`、`@text`、`@e<index>`、`@a<index>-…`）。

| SSE 事件 | KPP |
| --- | --- |
| `response.created` | `ResponseStart`（`id`、`model`，`usage` 不为 null 时也包括；形式 `native`） |
| `response.in_progress` | 附加到 `ResponseStart` |
| `response.output_item.added`（`message`） | 附加到第一个 part 的 `BlockStart`（`@open`） |
| `response.content_part.added`（消息：`output_text` / `refusal` / 其他） | `BlockStart` `Text` / `Refusal` / `Unknown`（形式 `part`；在另一个消息 item 尚未关闭时打开新消息 item，则带形式 `new_item`） |
| `response.output_text.delta` | `BlockDelta` `Text`；`logprobs` 为 extras |
| `response.output_text.annotation.added` | `BlockDelta` `Citation` |
| `response.refusal.delta` | `BlockDelta` `Refusal` |
| `response.output_text.done`、`response.refusal.done` | 附加到该 part 的 `BlockStop`（`@done`） |
| `response.content_part.done`（消息） | `BlockStop` |
| `response.output_item.done`（`message`） | 附加到下一个生命周期事件（`@close`） |
| `response.output_item.added`（`function_call`） | `BlockStart` `ToolCall`（`call_id`、`name`） |
| `response.function_call_arguments.delta` | `BlockDelta` `ToolArguments` |
| `response.function_call_arguments.done` | 附加到 `BlockStop`（`@done`） |
| `response.output_item.done`（`function_call`） | `BlockStop` |
| `response.output_item.added`（`reasoning`） | `BlockStart` `Reasoning`（item ID 作为不透明的 `OpenAiReasoningItemId`） |
| `response.reasoning_summary_part.added` | 文本为空的 `BlockDelta` `ReasoningSummary`（打开摘要 `summary_index`） |
| `response.reasoning_summary_text.delta` | `BlockDelta` `ReasoningSummary` |
| `response.reasoning_summary_text.done`、`response.reasoning_summary_part.done` | 附加到下一个摘要 part 或 `BlockStop`（`@sdone`、`@spdone`） |
| `response.content_part.added`（推理，`reasoning_text`） | 文本为空的 `BlockDelta` `ReasoningText`，形式 `new_part` |
| `response.reasoning_text.delta` | `BlockDelta` `ReasoningText` |
| `response.reasoning_text.done`、`response.content_part.done`（推理） | 附加到下一个 part 或 `BlockStop`（`@rdone`、`@rpdone`） |
| `response.output_item.done`（`reasoning`） | `BlockDelta` `Opaque`（如有 `encrypted_content`）+ `BlockStop` |
| `response.output_item.added` / `.done`（其他 item 类型） | `BlockStart` `Unknown` / `BlockDelta` `Unknown`（最终的 item）+ `BlockStop` |
| `response.completed`、`response.incomplete` | `ResponseDelta`（`stop`、`usage`）+ `ResponseStop`（最终回复对象的覆盖项，例如 `completed_at`） |
| `response.failed` | `StreamEvent::Error`（错误类型取自 `error.code`；事件存于 `raw`，原样重放） |
| `error`（`ResponseErrorEvent`，扁平的 `code`/`message`/`param`） | `StreamEvent::Error`（事件存于 `raw`，原样重放）。有嵌套的 `error` 对象时，从中读取 `code`、`message`、`param` 和 `type`，否则从事件本身读取。错误类型取自 `code` 和 `type`（忽略 `type: "error"`），与没有状态码时的[错误表](#错误errorresponse)相同，因此每个列出的错误码都起决定作用；否则为 `ProviderError`。没有消息时：`The provider reported an error in the middle of the stream.` |
| 内容之前没有 `response.created` | 合成一个 `ResponseStart`（id 和 model 为空，带形式 `skip`，因此不会被写回；最终的回复对象仍带有真实 ID） |
| 已知事件引用了流从未声明过的输出 item 或 part，或缺少必填字段 | `StreamEvent::Error`（`ProviderError`，``The provider sent a `<type>` event that refers to an output item or part it never announced, or lacks required fields.``）；之后不再解码任何内容 |
| `error` 紧接着 `response.failed` | 一个 `StreamEvent::Error`（两个事件都存于 `raw`，都会重放） |
| `response.completed`/`.incomplete`、`response.failed` 或 `error` 之后的任何内容 | 不解码 |
| 其他所有事件（`response.queued`、`response.web_search_call.*`、`response.file_search_call.*`、`response.mcp_*`、`response.image_generation_call.*`、`response.audio.*`、未来的事件、开始之后出现或没有可读 `response` 的 `response.created`）、SSE 注释、没有 `data:` 行的帧（例如单独的 `retry: 3000`）、数据为空或 JSON 数据没有 `type` 的帧 | `StreamEvent::Extra`（`{event, data}` / `{comment}`），仅限同一方言。没有数据的帧不含 `data` 键，写回时也没有 `data:` 行；空数据作为文本保留（`text: true`）；帧的 `id`、`retry` 和未知 SSE 字段保留为 `id`、`retry` 和 `other`。 |
| 不是 JSON 的数据 | `StreamEvent::Error`（`ProviderError`，`The provider sent a stream event that is not valid JSON: …`）；之后不再解码 |
| 已开始的流在 `response.completed`/`.incomplete`、`response.failed` 或 `error` 之前结束；流在事件中途结束；某行不是 UTF-8；帧超过 16 MiB | `StreamEvent::Error`（`ProviderError`：`The provider's stream ended before the response was complete.`、`the provider's event stream ended in the middle of an event`、`the provider's event stream contains a line that is not valid UTF-8`、`the provider sent a streamed event larger than Kaeso's limit of 16777216 bytes`） |
| 出现在不寻常位置的冗余事件 | `StreamEvent::Extra`，本应产生它们的生命周期事件带形式 `skip` |

为来自其他方言的 KPP 事件编码时，会产生完整的规范事件序列：

- `response.created` 和 `response.in_progress`，各自带有含回显字段的回复对象和 `usage: null`；
- 消息 item 的第一个 part 开始时的 `output_item.added`，以及每个 part 的 `BlockStart` 处的 `content_part.added`；
- 每个 `BlockStop` 处的 `.done` 事件和 `content_part.done`；
- 下一个非文本块开始或回复结束时，消息的 `output_item.done`；
- 摘要前后的 `reasoning_summary_*` part 事件，以及推理文本前后的 `content_part.*` 和 `reasoning_text.*`；
- 最后是 `response.completed`（`MaxTokens` 和 `ContentFilter` 时为 `response.incomplete`），带有所有输出 item 和用量。

`sequence_number` 从 0 开始计数。item ID 是生成的（`msg_kaeso_…`、`fc_kaeso_…`、`rs_kaeso_…`，由回复 ID 和输出索引得出）。不会生成 `obfuscation`。在 `ResponseStop` 时仍未关闭的块会先被关闭。没有 `ResponseStart` 的流会得到一个，其 ID 为 `resp_kaeso_<created_at>`，取自 `EncodeCtx::created_at`。其他方言的推理块变为 ID 为 `rs_kaeso_…` 的推理 item（参见[推理数据块](cross-format.md#推理数据块)）：推理文本作为摘要 0 的 `reasoning_summary_text` 流式输出（各摘要保留其索引），`output_item.done` 在 `encrypted_content` 中携带整个块的推理数据块。在 `ResponseStop` 或 `Error` 之后，编码器拒绝后续的生命周期事件。外来错误变为一个 `error` 事件，`code` 取自错误类型（参见[错误](#错误errorresponse)）。

其他方言发送、而本方言无法呈现的内容（额外事件，例如 Anthropic 的 `ping`，以及外来的未知块）会换来按节奏发送的 SSE 注释 `: keepalive`（参见[保活消息](cross-format.md#保活消息)）：每个额外事件一条，每 1,024 字节隐藏文本一条。`StreamEncoder::keepalive` 为网关提供同样的帧。流没有结束标记。

## 未单独列出的规范属性

所覆盖 schema 的其余所有属性，按名称列出：

| 属性 | KPP 目标 |
| --- | --- |
| `FunctionTool.async` | `FunctionTool` 的 extras（`async`） |
| `FunctionTool.defer_loading`、`.allowed_callers`、`.output_schema` | `FunctionTool` 的 extras |
| `FunctionToolCall.async`、`.namespace`、`.caller`（输入和输出 item） | `ToolCallBlock` 的 extras（`async` 等） |
| `FunctionToolCallOutputResource.created_by`（以及 `id`、`status`） | `created_by`：`ToolResultBlock` 的 extras；`id`、`status`：形式 |
| `Response.prompt_cache_diagnostics` | `Response` 的 extras（`prompt_cache_diagnostics`，整个对象） |
| `Response.prompt_cache_options`、`.moderation`、`.conversation`、`.output_text` | `Response` 的 extras |
| `ResponseUsage.input_tokens` | `usage.input_tokens` + `usage.cache_read_tokens` + `usage.cache_write_tokens`（参见[用量](cross-format.md#用量)） |
| `ResponseUsage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens` |
| `ResponseUsage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens` |
| `ResponseUsage.output_tokens` | `usage.output_tokens` |
| `ResponseUsage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens` |
| `ResponseUsage.total_tokens` | 推导得出（输入 + 输出）；原对象存于 `usage.raw`，对本方言优先使用 |
| 流事件的 `output_index` | 该 item 的 KPP 块（解码器状态；消息 part 按 `output_index` + `content_index` 定位）；按编码器自己的计数器写出，偏差是该事件的 `fields` 覆盖项 |
| 流事件的 `content_index` | 内容 part 的 KPP 块（消息 part）或推理文本 part（`new_part` 增量）；按编码器的计数器写出 |
| 流事件的 `item_id` | item ID（本方言的 ID 用形式覆盖项，否则使用生成的 ID） |
| 流事件的 `sequence_number` | 偏离 0、1、2… 时为形式覆盖项 |
| 流事件的 `obfuscation` | 增量上的形式覆盖项 |
| 流事件的 `logprobs`（`output_text.delta`/`.done`） | 增量 / 停止事件上的 `fields` 覆盖项 |
| `prompt.id`、`prompt.version`、`prompt.variables`（`Prompt`） | extras `prompt`（整个对象；可复用的 prompt 存储在服务端） |
| `prompt_cache_options.ttl`、`prompt_cache_options.mode` | extras `prompt_cache_options`（整个对象） |
| `context_management[].type`、`context_management[].compact_threshold` | extras `context_management`（整个列表） |
| `Error.misalignment`（`MisalignmentErrorDetailsResource`：`review_target`、`error_type`、`detailed_explanation`、`steer`） | `KppError.raw`（整个响应体），重放给 OpenAI 方言的应用 |
| `ResponseError.misalignment`（失败的回复，`response.failed`） | `KppError.raw`（整个回复或事件）；`response.failed` 事件会在 Responses 流中重放 |
| `ResponseError.code` / `.message` | `KppError.kind`（经由 `ResponseErrorCode`）/ `.message` |
| `Error.type`、`.code`、`.param`、`.message` | `KppError.kind`（结合状态码）、`.param`、`.message`；响应体存于 `raw` |

## 错误：`ErrorResponse`

`{"error": {"message", "type", "param", "code"}}` ⇄ `KppError`（`message`、`param`、`status`、取自 `retry-after-ms` / `retry-after` 的 `retry_after`，响应体存于 `raw`）。

| 解码：状态码 / `type` / `code` | `ErrorKind` |
| --- | --- |
| `code` `context_length_exceeded` | `ContextTooLong` |
| `type`/`code` `insufficient_quota`、`credit_balance_exhausted`、`organization_spend_limit_exceeded`、`project_spend_limit_exceeded`、`organization_usage_limit_exceeded`、`billing_hard_limit_reached`、`billing_not_active` | `Billing` |
| 503、`service_unavailable_error`、`server_is_overloaded` | `Overloaded` |
| `content_policy_violation`、`content_filter`、`image_content_policy_violation`、`bio_policy`、`cyber_policy`、`misalignment_policy_violation` | `ContentFiltered` |
| 400、409、422 | `InvalidRequest` |
| 401 | `Authentication` |
| 403 | `PermissionDenied` |
| 404 | `NotFound` |
| 413 | `RequestTooLarge` |
| 429 | `RateLimited` |
| 408、504、`vector_store_timeout` | `Timeout` |
| 其他状态码 | `ProviderError` |
| 不是 OpenAI 错误的响应体，无论状态码如何（例如代理返回的 HTML 429） | 按上面各行取该状态码对应的错误类型（代理返回的 429 为 `RateLimited`，502 为 `ProviderError`），消息为 ``The provider returned HTTP <status> with a body Kaeso could not read: <the body, cut to 300 bytes>``；保留状态码和原始响应体 |

含 `type` 和 `code` 的行优先于状态码。其他错误码在这里不起决定作用，由状态码决定：`rate_limit_exceeded`、`rate_limit_error` 和 `slow_down`（`RateLimited`），`server_error`（`ProviderError`），`invalid_prompt`、`invalid_request_error` 和图片相关错误码（以 `invalid_image` 或 `image_` 开头，或以 `_image` 结尾；`InvalidRequest`）。它们只在没有状态码的地方起决定作用：流中的 `error` 事件，以及失败回复的 `error.code`。没有消息的错误对象得到 `The provider returned HTTP <status> without a message.`

对于来自 OpenAI 方言（Chat、Responses、Embeddings）的错误，只要它们未被改动，编码时就重放其响应体和状态码；所谓未改动，是指从响应体重新解码出的错误类型与该错误的类型相同，且消息一致。其他错误得到：

| `ErrorKind` | 状态码 | `type` | `code` |
| --- | --- | --- | --- |
| `InvalidRequest`、`Unsupported` | 400 | `invalid_request_error` | `null` |
| `ContextTooLong` | 400 | `invalid_request_error` | `context_length_exceeded` |
| `ContentFiltered` | 400 | `invalid_request_error` | `content_policy_violation` |
| `Authentication` | 401 | `invalid_request_error` | `null` |
| `PermissionDenied` | 403 | `request_forbidden` | `null` |
| `NotFound` | 404 | `invalid_request_error` | `null` |
| `RequestTooLarge` | 413 | `invalid_request_error` | `null` |
| `RateLimited` | 429 | `rate_limit_error` | `rate_limit_exceeded` |
| `Billing` | 429 | `insufficient_quota` | `insufficient_quota` |
| `Overloaded` | 503 | `service_unavailable_error` | `server_is_overloaded` |
| `Timeout` | 504 | `server_error` | `timeout` |
| `ProviderError` | 503 以外的 5xx 状态码，否则为 500 | `server_error` | `null` |

`retry_after` 写为 `retry-after`（整秒，向上取整）；不是整秒时另加 `retry-after-ms`。在流中，外来错误为 `{"type": "error", "code": <code, or type when null>, "message", "param"}`，其中 `code` 是错误码，错误码为 `null` 时用 `type`。

## 形式标记

| 标记 | 所在位置 | 含义 |
| --- | --- | --- |
| `set`、`unset` | 任何元素 | 呈现方式的覆盖项（参见[原始写法如何保留](#原始写法如何保留)）。 |
| `input` | 请求 | `input` 是字符串。 |
| `system_items`、`instructions` | 请求 | `system` 如何拆分为 `instructions` 和开头的 system 与 developer item。 |
| `user_key` | 请求 | 终端用户 ID 来自 `user`。 |
| `object`、`no_object` | 推理配置 | `reasoning` 对象为空 / 不存在。 |
| `parts` | 消息、工具结果 | 内容是 part 列表，尽管用字符串就足够。 |
| `empty_at` | 消息 | 空消息 item 的块索引。 |
| `raw_item` | 消息 | 该消息是一个未知 item。 |
| `args_string` | 工具调用 | `arguments` 不是 JSON；原始值是一个 JSON 字符串。 |
| `breakpoint` | 缓存提示 | 它是一个显式的 `prompt_cache_breakpoint`。 |
| `native` | 请求 | 从本方言解码（不会添加 `store: false`）。 |
| `canon` | 任何元素 | 解码时覆盖项路径上的规范值，用于检查覆盖项是否过时。 |
| `native`、`messages`、`blocks` | 回复 | 从本方言解码；消息 item 的块范围；块数。 |
| `item` | 回复中的块 | 该块所属输出 item 的索引。 |
| `kaeso_id` | 推理块 | 其由 Kaeso 生成的 item ID 已被丢弃。 |
| `new_item`、`part`、`new_part`、`skip`、`~event`、`~sse` | 流事件 | 参见[流](#流responsestreamevent)。 |

## 模型

`GET /v1/models` 和 `GET /v1/models/{model}` 在所有 OpenAI API 中结构相同。本编解码器使用 [OpenAI Chat Completions](openai-chat.md) 页面所述的共享实现对其编码和解码（fixture 位于 `fixtures/openai_chat/models/`）。解码出的 extras 带有来源 `openai_responses`。
