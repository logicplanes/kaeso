---
title: OpenAI Chat Completions
description: OpenAI Chat Completions 格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 5
nav: OpenAI Chat
---

在这里查阅 OpenAI Chat Completions 格式的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_chat::OpenAiChat`（`crates/kaeso-codecs/src/openai_chat/`）
- **端点：**`POST /v1/chat/completions`（JSON 和 SSE），以及所有 OpenAI 编解码器共用的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_chat/`
- **其他服务商：** [Chat Completions 服务商](openai-chat-providers.md)介绍编解码器如何适配使用此格式的每个服务商

**来源。** OpenAI OpenAPI 规范的提交 `e95c0fe615f45a19e8878af923a729948445a6bd`（下文的 schema 名称出自这里）、[创建接口参考](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[流式事件参考](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events)和[错误码指南](https://developers.openai.com/api/docs/guides/error-codes)。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：所述条目的 `Extras::fields`，绑定到 `openai_chat`。它们只在本方言内使用；其他方言会以 `Unsupported` 拒绝（[Extras](cross-format.md#extras)）。嵌套的键使用键路径（`image_url.detail`）。
- **形式标记**：编解码器私有的 `Extras::form` 标记。它只关乎呈现，其他方言会忽略它。`verbatim` 标记保留已声明键的 `null` 和 `[]` 写法，以及值为 `null` 的未声明键。
- **Unsupported**：解码或 `check()` 以 `ErrorKind::Unsupported { feature }` 失败；括号中给出功能名。

**数字。** 类型化的数值字段经过 `u32`、`u64`、`i64`（`seed`）或 `f64`。超出其范围的值会导致解码失败，在请求中为 `InvalidRequest`，在回复中为 `ProviderError`，而不会被改动。未知字段逐字节保留，但名称无法由 Kaeso 承载的键除外（[见下文](#kaeso-无法承载的键)）。

## 请求：`CreateChatCompletionRequest`

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `GenerateRequest.model`（类型化） | |
| `messages` | `system` + `messages`（类型化） | 见[消息](#消息chatcompletionrequestmessage)。 |
| `temperature`、`top_p` | `sampling.temperature`、`sampling.top_p`（类型化） | 写成整数（`1`）时：形式标记 `integers`，即以整数写出的键的列表。只要值仍是整数，编码器就把这样的键写成整数，因此 `temperature: 1` 写回时仍是 `1`，而不是 `1.0`；值改变后写出新值。来自其他方言时：`f64` 写法（`1.0`）。 |
| `max_completion_tokens` | `sampling.max_output_tokens`（类型化） | 来自任何源方言的请求都写成这种形式，除非服务商的配置要求 `max_tokens`（[服务商配置](openai-chat-providers.md#服务商配置)）。`CreateChatCompletionRequest` 没有为它或 `max_tokens` 规定最小值，因此任何非负整数都会保留，包括 `0`；负数为 `InvalidRequest`。 |
| `max_tokens`（已弃用） | `sampling.max_output_tokens`（类型化） | 形式标记 `max_tokens: "max_tokens"`。两者都存在且值相同：形式标记 `"both"`。值不同时以 `max_completion_tokens` 为准，`max_tokens` 成为 extra `max_tokens (differs from max_completion_tokens)`，发回时写作 `max_tokens`；当其他方言拒绝它时，这个键名说明了冲突所在。 |
| `stop` | `sampling.stop`（类型化） | 字符串写法：形式标记 `stop: "string"`；`[]`：形式标记 `"array"`。超过四个序列：`Unsupported`（`stop`）。 |
| `seed` | `sampling.seed`（类型化） | 服务商的配置如此规定时，以 `random_seed` 发送（[服务商配置](openai-chat-providers.md#服务商配置)）。 |
| `frequency_penalty`、`presence_penalty` | `sampling.*`（类型化） | 整数写法与 `temperature` 相同（形式标记 `integers`）。 |
| `n` | 为 `1` 时记为形式标记 `n` | `n > 1`：`Unsupported`（`n`，见[采样参数](cross-format.md#采样参数)）；`0`：`InvalidRequest`。 |
| `reasoning_effort` | `reasoning.effort`（类型化） | 来自其他方言时（[推理强度与预算](reasoning.md#推理强度与预算)）：显式的推理强度保持不变；Anthropic `thinking.disabled` 变为 `none`；预算变为 `low`（低于 4,096）、`medium`（4,096 到 16,383）或 `high`（16,384 及以上）。取值为 `none` 到 `max`；未知值保留为 `ReasoningEffort::Other`，对 Chat 应用原样透传，而对来自其他方言的请求，`check()` 会拒绝规范枚举之外的值（`reasoning.effort`）。 |
| `response_format` `{type: text}` | `output = Text`（类型化） | 形式标记 `response_format_text`；未知键为 extras `response_format.<key>`。 |
| `response_format` `{type: json_object}` | `output = JsonObject`（类型化） | 未知键为 extras `response_format.<key>`。 |
| `response_format` `{type: json_schema, json_schema: {name, description, schema, strict}}` | `output = JsonSchema`（类型化） | 未知键为 extras（嵌套的为 `json_schema.<key>`）。缺少 `schema`：`{}` + 形式标记 `schema_absent`；缺少 `json_schema` 对象：`InvalidRequest`。来自其他方言、没有名称的 schema 以 `name: "response"` 发送（[结构化输出](cross-format.md#结构化输出)）。 |
| `response_format` 其他类型 | extras `response_format` | 仅限同一方言。 |
| `tools[]` `{type: function, function: FunctionObject}` | `ToolDef::Function`（类型化：`name`、`description`、原始 `parameters`、`strict`） | 未知键为 extras（嵌套的为 `function.<key>`）。来自其他方言、不符合 `[a-zA-Z0-9_-]{1,64}` 的名称会被确定性地改写（`EgressCtx::tool_names`）。 |
| `tools[]` 其他类型（`custom`） | `ToolDef::Unknown` | 仅限同一方言。 |
| `tool_choice` `"auto"`、`"none"`、`"required"` | `ToolChoice::Auto`、`None`、`Required`（类型化） | |
| `tool_choice` `{type: function, function: {name}}` | `ToolChoice::Tool(name)`（类型化） | 带有未知键时：`ToolChoice::Unknown`。 |
| `tool_choice` `allowed_tools`、`custom`、其他字符串 | `ToolChoice::Unknown` | 仅限同一方言。 |
| `parallel_tool_calls` | `parallel_tool_calls`（类型化） | |
| `safety_identifier` | `user`（类型化） | KPP `user` 是用于滥用监测的最终用户 ID，这正是 `safety_identifier` 的含义。 |
| `user`（已弃用） | 没有 `safety_identifier` 时为类型化的 `user`（形式标记 `user`）；否则为 extra `user (sent alongside safety_identifier)`，发回时写作 `user` | 来自其他方言的 KPP `user` 以 `safety_identifier` 发送。配置中没有最终用户字段的服务商两者都收不到（[服务商配置](openai-chat-providers.md#服务商配置)）。 |
| `stream` | `stream`（类型化） | `false`：形式标记 `stream_false`。 |
| `stream_options.include_usage` | `StreamMode::On { include_usage }`（类型化） | 没有 `stream_options`：形式标记 `stream_options: "absent"`；没有 `include_usage`：形式标记 `"no_include_usage"`；其他键（`include_obfuscation`）为 extras `stream_options.<key>`；`stream` 关闭时：整个对象记为形式标记 `stream_options_off`。来自其他方言的请求总是要求 `include_usage: true`。网关需要用量，因此会在每个流式请求上设置 `include_usage: true`，并在 `EncodeCtx` 中保留应用自己的请求；随后应用一侧的流编码器会对没有请求用量的应用隐藏用量 chunk。 |
| `audio`、`functions` 和 `function_call`（已弃用，顶层）、`logit_bias`、`logprobs`、`metadata`、`modalities`、`moderation`、`prediction`、`prompt_cache_key`、`prompt_cache_options`、`prompt_cache_retention`、`service_tier`、`store`、`top_logprobs`、`verbosity`、`web_search_options` | extras | KPP 中没有对应项；跨方言时被拒绝（[Extras](cross-format.md#extras)、[采样参数](cross-format.md#采样参数)），但 `prompt_cache_key` 在 [Extras 允许列表](cross-format.md#extras)中：OpenAI Responses 会收到它，其他格式会丢弃它；`prompt_cache_options` 和 `prompt_cache_retention` 则用于设置 Anthropic 保留缓存的时长（[缓存提示](cross-format.md#缓存提示)）。`null` 值记为形式标记。例如发往 Anthropic 时，`functions` 以 `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. `` 失败。 |
| 未知键 | extras | 以 `header:` 开头或包含 `.` 的键会被拒绝（[Kaeso 无法承载的键](#kaeso-无法承载的键)）。 |
| 请求头 `openai-beta` | extras `header:openai-beta` | 只在请求发往 Chat 时转发；在 [Extras 允许列表](cross-format.md#extras)中，因此其他方言会丢弃它。 |

### 消息（`ChatCompletionRequestMessage`）

| 线路 | KPP | 说明 |
|---|---|---|
| 开头的 `system` 和 `developer` 消息 | `GenerateRequest.system` 中的块 | 每条消息的第一个块带有形式标记 `message: {role, content}`，用来保留消息边界、角色以及字符串或数组写法；消息的 `name` 和未知键是该块上的 extras `message.<key>`。第一个部分为未知类型的开头消息仍是一条 `Role::System` 消息，并结束开头的这一段。来自其他方言时：一条 `system` 消息。 |
| 后续的 `system` 和 `developer` 消息 | `Message { role: System }`（[系统消息](cross-format.md#系统消息)） | `developer`：形式标记 `role: "developer"`。 |
| `user` | `Message { role: User }` | `name` 和未知键：消息的 extras。 |
| `assistant` | `Message { role: Assistant }`：先是内容部分，然后是 `refusal`，最后是 `tool_calls` | 没有 `content`：形式标记 `content: "absent"`；`null`：verbatim；`name`、`audio`、`function_call`：extras。`annotations` 不在请求 schema 中，但 SDK 会随每个回复把它发回：`[]` 按 verbatim 保留（形式标记），使对话可以转到另一个服务商；非空值成为第一个文本块的 `TextBlock.citations`，绑定到 Chat（没有文本时：extras `annotations`），其他方言会拒绝它（`citations`），Chat 则以 `annotations` 发回。来自其他方言时，文本块进入 `content`，第一个拒绝进入 `refusal`，工具调用进入 `tool_calls`，因为 Chat 无法将它们交错排列；推理会被丢弃（[推理](reasoning.md)）；没有留下任何 Chat 能显示的内容的轮次（例如只有推理）以 `content: ""` 发送，这样对话保留其轮次，也满足 Chat 关于没有 `tool_calls` 的助手消息必须有内容的规则。 |
| 助手 `refusal` | `Block::Refusal` | |
| 助手 `function_call`（已弃用） | 消息的 extras `function_call` | `null` 按 verbatim 保留。按收到时的样子写回；其他方言会拒绝它（`messages[i].function_call`）。 |
| 助手的 `reasoning_content`、`reasoning`（字符串）和 `reasoning_details`（OpenRouter 的 `reasoning.text`、`reasoning.summary`、`reasoning.encrypted` 条目列表） | 只要其中任一字段存在，就在内容之前生成 `Block::Reasoning`，取第一个可读字段的文本，没有则不含文本（`reasoning_details`：`text` 作为文本，`summary` 作为摘要），三者按原样保存在形式标记 `replayed` 中 | 应用会发回 Chat 提供商流式发给它们的推理（Grok CLI、Qwen Code、opencode）。Chat 提供商按原样收回这些字段，`thinking_parts()` 除外（[Chat 提供商](openai-chat-providers.md)）；其他方言丢弃可读推理（[推理](reasoning.md)）。无论其值如何，请求都不会因它们被拒绝；`null` 按原样保留。 |
| `tool` 消息 | `Role::User` 消息中的 `Block::ToolResult`；连续的 tool 消息共用一条 KPP 消息 | `tool_call_id` 变为 `call_id`；内容部分变为嵌套的文本块；未知键是工具结果的 extras。来自其他方言的 `is_error` 会被丢弃，因为 Chat 在内容中报告错误（[工具错误](cross-format.md#工具错误)）。来自其他方言时，一条用户消息中的工具结果变为 `tool` 消息，后面跟一条包含其余块的 `user` 消息。 |
| `function` 消息（已弃用） | `Message { role: User }`，其中一个 `Block::Unknown` 包含整条消息，另加消息的形式标记 `role: "function"` | 按收到时的样子写回；其他方言会拒绝它（`` `function` message ``）。 |
| `content` 为字符串或数组 | 形式标记 `content: "array"`（消息或工具结果） | 单个纯文本块默认写成字符串。空数组无效（`minItems: 1`）。 |
| 没有 `role` 或角色为其他值的消息 | `InvalidRequest`（`param` 为 `messages[i]`） | |

旧版函数调用没有工具调用 ID，因此它的轮次不会成为 KPP 的工具调用和工具结果。在本方言内它们改为原样往返，顶层参数 `functions` 和 `function_call` 作为 extras。对应的 fixture 是 `fixtures/openai_chat/requests/legacy-functions.json`。

其他方言以 `Unsupported` 拒绝这一切，各部分的名称如下：

- 顶层参数为 `functions` 和 `function_call`；
- 助手的 `function_call` 为 `messages[i].function_call`；
- `function` 消息为 `` `function` message ``，提示为 `Remove the message, or send the request to a model served through its original format.`

### 内容部分

| 线路 | KPP | 说明 |
|---|---|---|
| `text` | `Block::Text` | 未知键：块的 extras。 |
| `prompt_cache_breakpoint`（文本、图像和文件部分） | `cache: CacheHint { ttl: None }`，绑定到 Chat | 两者都标记可复用前缀的结尾；TTL 来自 `prompt_cache_options`。只有来自 Chat 的提示会被发回；来自其他方言的提示会被丢弃（[缓存提示](cross-format.md#缓存提示)）。`mode` 不是 `explicit` 时：形式标记 `mode`。 |
| `image_url.url`（`https:`） | `Block::Image { source: Url }` | |
| `image_url.url`（`data:<type>[;params];base64,<data>`，RFC 2397，协议名和 `base64` 大小写均可） | `Block::Image { source: Base64 }` | 不是 `data:<type>;base64` 的头部，例如带参数（`;name=a.png`）或大写，会在形式标记 `data_url` 中原样保留，`file.file_data` 也一样。不是 base64 或没有媒体类型的 data URL 仍为 `Url`。 |
| `image_url.detail` | extras `image_url.detail` | 在 [Extras 允许列表](cross-format.md#extras)中：对其他方言静默丢弃。 |
| `file.file_data`（`data:` URL） | `Block::Document { source: Base64 }` | 裸 base64 被视为媒体类型 `application/pdf`（OpenAI 文件输入指南中唯一的类型），带形式标记 `bare_file_data`，写回时仍为裸值。 |
| `file.file_id` | `Block::Document { source: FileId }`，绑定到 OpenAI 的文件存储（[文件](cross-format.md#文件)） | 同时有 `file_data` 时：extras `file.file_id`。 |
| `file.filename` | extras `file.filename` | 这是 OpenAI 要求随内联 `file_data` 一起提供的文件名，而不是展示给模型的标题，因此从不成为 `DocumentBlock::title`。在 [Extras 允许列表](cross-format.md#extras)中。来自其他方言的内联文档会得到 `document.<subtype>`（例如 `document.pdf`），因为 OpenAI [文件输入指南](https://developers.openai.com/api/docs/guides/file-inputs)中的每个示例都随 `file_data` 发送文件名。 |
| `input_audio` | `Unsupported`（`input_audio`） | 音频输入不属于 KPP。 |
| `refusal`（助手） | `Block::Refusal` + 形式标记 `part` | |
| 其他类型，以及用户消息之外的 `image_url` 和 `file` 部分、助手消息之外的 `refusal` 部分 | `Block::Unknown` | 仅限同一方言。 |

对于来自其他方言的 KPP，`check()` 会拒绝：

- 文本文档、文档 URL，以及文档的 `title`、`context` 和 `citations`；
- 图像文件 ID，以及用户消息之外的图像和文档；
- 助手消息之外的拒绝和引用；
- 其他方言的引用（通过 `check_foreign`）；
- `top_k`（[采样参数](cross-format.md#采样参数)）；
- 未知的推理模式，以及规范枚举之外的推理强度。

预算和 `disabled` 会被转换（[推理强度与预算](reasoning.md#推理强度与预算)）。以下提示字段则会被丢弃：

- 推理块（[推理](reasoning.md)）；
- 其他方言的缓存提示（[缓存提示](cross-format.md#缓存提示)）；
- `ToolResult::is_error`，其信息由错误内容承载（[工具错误](cross-format.md#工具错误)）；
- 其他方言的 `reasoning.summary` 和 `reasoning.include_encrypted`，因为 Chat 没有请求它们的字段（[可读推理与摘要](reasoning.md#可读推理与摘要)）；
- OpenAI Responses 的 `web_search` 工具和 `web_search_call` 项，因为 Chat 没有托管网页搜索（[省略的工具](cross-format.md#省略的工具)）；
- OpenAI Responses 的 `text.verbosity`，除非服务商配置接受 `verbosity`（[条件条目](cross-format.md#条件条目)）。

OpenAI Responses 的 `custom_tool_call` 和 `local_shell_call` 项及其输出会变为工具调用和 `tool` 消息（[Codex 工具历史](cross-format.md#codex-工具历史)）。

编解码器无法读取的请求体为 `InvalidRequest`，错误会指明字段及其应有的形式并设置 `param`，而不会给出解析器位置：`` The request body is not a valid OpenAI Chat Completions request: `max_completion_tokens` must be an integer from 0 to 4294967295. ``

### Kaeso 无法承载的键

名称以 `header:` 开头或包含 `.` 的未知键无法承载，因为 KPP 用这种写法表示请求头和键路径。凡是编解码器逐键读取对象的地方都会拒绝它：

- 请求体、`stream_options`、类型为 `text`、`json_object` 或 `json_schema` 的 `response_format` 及其 `json_schema`；
- 函数工具及其 `function`，以及具名的 `tool_choice` 及其 `function`；
- `system`、`developer`、`user`、`assistant` 和 `tool` 消息；
- `text`、`refusal`、`image_url` 和 `file` 部分及其 `image_url`、`file` 和 `prompt_cache_breakpoint` 对象；
- 函数工具调用及其 `function`。

此时请求为 `InvalidRequest`，例如请求体中的 `` The request body is not a valid OpenAI Chat Completions request: the field `header:authorization` has a name Kaeso cannot carry; remove it. ``，或消息中的 `` The request contains an invalid OpenAI Chat Completions user message: the field `a.b` has a name Kaeso cannot carry; remove it. ``。超过 64 字节的名称会被截断并以 `…` 结尾。

整体保留的对象，例如未知部分、工具和工具选择、自定义（custom）工具调用、`function` 消息和其他类型的 `response_format`，不会被深入检查。在回复和流 chunk 中，同一规则会产生 `ProviderError`（在流中为一个 `Error` 事件）。

### 工具调用（`ChatCompletionMessageToolCall`）

| 线路 | KPP | 说明 |
|---|---|---|
| `id`、`function.name` | `ToolCallBlock.id`、`.name` | Kaeso 改写过的名称在解码回复时映射回原名。回复中没有 `id` 或 `id` 为 `""` 的调用会得到 `call_kaeso_<回复 ID>_<n>`（ID 中的字母和数字，`n` 为其位置），使其他格式的应用能够回答它；形式标记 `id` 为 Chat 应用保留收到的写法（缺失时为 `null`）。流中同样处理，`n` 为 KPP 块索引，形式标记为 `tc`。 |
| `function.arguments` | 原始的 `ToolCallBlock.arguments` | 字符串中的 JSON 文本，逐字节保留。不是有效 JSON 或带有首尾空白的文本，其字面值保留在形式标记 `arguments` 中；此时 KPP 对空白文本存 `{}`，否则把文本存为 JSON 字符串。按照 KPP 的约定，`ToolCallBlock.arguments` 中的 JSON 字符串表示服务商发送的是非 JSON 文本，因此对来自其他方言的调用，以字符串的内容作为 `arguments` 发送；对象、数组和其他 JSON 值则以其 JSON 文本发送。 |
| 未知键 | extras（嵌套的为 `function.<key>`） | |
| `type: custom` | `Block::Unknown` | 在 `tool_calls` 中发回。 |
| 缺少 `type`，但有 `function` 对象 | `Block::ToolCall`，形式标记 `no_type` | `ChatCompletionMessageToolCall` 要求 `type`，但某些 OpenAI 兼容服务商会省略它；只有函数调用才有 `function` 对象，因此该调用能到达每个应用。写回时不带 `type`。 |

## 回复：`CreateChatCompletionResponse`

| 线路字段 | KPP | 说明 |
|---|---|---|
| `id`、`model` | `Response.id`、`.model` | 来自其他格式时，保留服务商的 ID 不变（[回复 ID](cross-format.md#回复-id)）。 |
| `object` | 无 | 始终为 `chat.completion`；其他值或没有该字段：形式标记 `object`。 |
| `created`、`service_tier`、`system_fingerprint`、`metadata`、`moderation` | extras | extras 中没有 `created` 的回复（例如来自其他方言的回复）取 `EncodeCtx::created_at`，没有时为 `0`。 |
| `choices` | `choices[0]` | 多于一个选项（choice）：`Unsupported`（`n`）；没有选项：`ProviderError`；`index` 不为 0：extras `choice.index`。带有 `error` 对象且没有 `choices` 的 200 响应体（OpenRouter 在服务商于 200 之后失败时发送，见 [Errors and debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging)）会像流错误一样解码为该错误。 |
| `choices[0].logprobs` | extras `choice.logprobs` | `null`：verbatim。 |
| `choices[0].finish_reason` | `Response.stop` | `stop` ⇄ `EndTurn`，`length` ⇄ `MaxTokens`，`tool_calls` ⇄ `ToolUse`，`content_filter` ⇄ `ContentFilter`，其他值（例如 `function_call`）⇄ `Other`。来自其他方言时：`StopSequence` 和 `PauseTurn` 变为 `stop`；`Refusal` 变为 `content_filter`，因为回答被拦下了。其他方言自有的值会变为最接近的 Chat 值：`max_tokens`、`max_output_tokens` 和 `model_context_window_exceeded` 变为 `length`，`tool_use` 变为 `tool_calls`，`refusal` 变为 `content_filter`，规范枚举之外的其他值变为 `stop`。非流式回复缺少规范要求的 `finish_reason`：`ProviderError`。 |
| `message.content` | `Block::Text` | 来自其他方言时，所有文本块不加分隔符地拼接起来，因为 Chat 的回复中没有内容部分。不加分隔符，是因为 Anthropic 会围绕引用把一个回答拆成多个文本块，而同样不带分隔符的流式增量必须拼出相同的 `content`。 |
| 以部分数组形式出现的 `message.content`（Mistral，不在 OpenAI 规范中） | 恰好是 `{type: "text", text}` 的部分为 `Block::Text`；配置使用 `thinking_parts()` 时，`thinking` 部分为 `Block::Reasoning`（[Chat 提供商](openai-chat-providers.md)）；其他部分为 `Block::Unknown`。没有该配置时：形式标记 `content_parts` | 没有 `thinking_parts()` 时，写回为同样的数组。有该配置时，Chat 应用得到字符串 `content`（没有文本时为 `null`）和 `reasoning_content`，从不得到数组。其他方言得到文本和推理内容；未知部分只以数组形式写回给 Chat 应用（[回复](cross-format.md#回复)）。来源：Mistral 的 `AssistantMessage.content`（`ContentChunk[]`，https://docs.mistral.ai/openapi.yaml）以及录制文件 `fixtures/openai_chat/providers/mistral/responses/recorded-reasoning-effort-high.json`；Chat 应用收到的内容见 `fixtures/openai_chat/providers/mistral/for-chat-apps/`。 |
| `message.reasoning_content`、`message.reasoning`，以及配置在 `reasoning_fields` 中列出的其他键 | 首先是 `Block::Reasoning`，形式 `reasoning_field` | 仅在列出该键的配置下（[服务商配置](openai-chat-providers.md#服务商配置)）；否则为 extras `choice.message.<key>`。对 Chat 应用写回同一个键。 |
| `message.annotations` | `TextBlock.citations`（原始，绑定到 Chat） | 没有内容时：extras `choice.message.annotations`。其他方言的引用会被省略（[回复](cross-format.md#回复)）。 |
| `message.refusal` | `Block::Refusal` | |
| `message.tool_calls` | `Block::ToolCall` 或 `Block::Unknown` | 与请求中相同。 |
| `message.audio`（`id`、`data`、`expires_at`、`transcript`）、`message.function_call`、未知键 | extras `choice.message.<key>` | 音频对象连同 `transcript` 和 `expires_at` 完整保留。 |
| 推理、其他方言的未知块 | 无 | 在 Chat 回复中省略（[回复](cross-format.md#回复)）。网关会为下一轮保存可回放的推理，以它之前对应的工具调用为键（[由网关保存的推理](reasoning.md#由网关保存的推理)，`kaeso_codecs::continuity`）。 |

来自其他方言的回复按 OpenAI 的方式写出空值：`content: null`、`refusal: null`、`annotations: []` 和 `logprobs: null`。

### 用量（`CompletionUsage`）

KPP 字段遵循[用量](cross-format.md#用量)中的约定。

| 线路 | KPP |
|---|---|
| `prompt_tokens − cached_tokens − cache_write_tokens` | `input_tokens` |
| `prompt_tokens_details.cached_tokens` | `cache_read_tokens`，至多为 `prompt_tokens`；`cache_write_tokens` 至多为剩余部分 |
| `prompt_tokens_details.cache_write_tokens` | `cache_write_tokens` |
| `completion_tokens` | `output_tokens`（包括推理；若 `total_tokens` 等于 `prompt_tokens + completion_tokens + reasoning_tokens`，即 SpaceX 的计法，则再加上推理 Token）；与 `prompt_tokens` 和 `total_tokens` 一样，`null` 视为缺失（0），并保留在 `raw` 中 |
| `completion_tokens_details.reasoning_tokens` | `reasoning_tokens` |
| `total_tokens` | 推导得出（`prompt_tokens + completion_tokens`）；不单独存储，保留在 `raw` 中 |
| `prompt_tokens_details.audio_tokens`、`text_tokens`、`image_tokens`，`completion_tokens_details.audio_tokens`、`text_tokens`、`accepted_prediction_tokens`、`rejected_prediction_tokens` | 只在 `raw` 中（类型化总数的细分） |
| 整个对象 | `raw`，只要类型化字段仍与之相符就原样发回；否则以类型化字段为准，原始对象补充其余的键 |

来自其他方言时：`prompt_tokens = input + cache_read + cache_write`，`prompt_tokens_details.cached_tokens = cache_read`，`cache_write_tokens` 不为零时也一并写入，KPP 有推理计数时再写入 `completion_tokens_details.reasoning_tokens`。

## 流：`CreateChatCompletionStreamResponse`

裸的 `data:` 帧，每帧一个 JSON chunk，以 `data: [DONE]` 结束。

| 线路 | KPP 事件 | 说明 |
|---|---|---|
| 第一个 chunk | `ResponseStart { id, model }` | `created`、`service_tier`、`system_fingerprint`：extras；`object` 不是 `chat.completion.chunk` 或没有该字段时：形式标记 `object`。第一个 chunk 没有内容且 `id` 或 `model` 为空或缺失时，开始事件以及其后没有内容的 chunk 会被暂缓输出，直到两个值都已知或某个 chunk 带有内容（块增量或回复增量）；随后开始事件带着届时已知的值发出，每个值取自第一个给出非空值的 chunk。 |
| 每个 chunk 中的 `id`、`object`、`created`、`model`、`service_tier`、`system_fingerprint` | 无 | 外层字段会重复出现。某个 chunk 的外层字段与编码器根据 `ResponseStart` 重建的不同时，形式标记 `chunk.env` 保留该 chunk 的值，`chunk.env_absent` 保留它缺少的键，因此每个 chunk（包括第一个）都按收到时的样子写回。一致的 chunk 跟随 `ResponseStart`，因此在那里改写 `id` 或 `model` 的网关会一并改写它们。 |
| `choices[0].delta.role` | 形式标记 `chunk.role` | 来自其他方言时，回复一开始流就以一个角色 chunk 开头：`delta: {"role": "assistant", "content": "", "refusal": null}`，即 OpenAI 自己在文本回复中发送的第一个 chunk，这样模型思考时应用也能收到流的消息（[保活消息](cross-format.md#保活消息)）。 |
| `choices[0].delta.content` | 一次 `BlockStart(Text)`，然后是 `BlockDelta(Text)` | 每一片都保留，包括 `""`。 |
| 以部分数组形式出现的 `choices[0].delta.content`（Mistral） | 恰好是 `{type: "text", text}` 的每个部分为 `BlockDelta(Text)`；配置使用 `thinking_parts()` 时，`thinking` 部分的每段文本为 `BlockDelta(ReasoningText)`，形式标记 `rf`，该部分的其他键（`closed`、`signature`）放在其最后一个 delta 的形式标记 `thinking` 中，与回复的推理块保留它们的方式相同。其他每个部分（`reference`）以及 `thinking` 数组中每个不是文本的块都变为带形式标记 `part`（`content` 或 `thinking`）的 `BlockStart(Unknown)`，后跟它的 `BlockStop`，与回复把它们保留为 `Block::Unknown` 相同；带有其他键但没有可读文本的 `thinking` 部分以同样方式整体保留。没有该配置时，其他部分为块的 extras `delta.content`，形式标记 `cs` 记录每个部分的位置 | 没有 `thinking_parts()` 时，Chat 应用收回同样的数组。有该配置时，它们得到字符串 `content` 和 `reasoning_content`，不含可读内容的块不带 `content`；保留下来的部分与回复中一样只留在记录的回复里。其他方言得到文本和推理内容并省略其余部分。Mistral 以这种方式流式发送思考内容，回答的第一个 token 可能与之在同一数组中（`fixtures/openai_chat/providers/mistral/streams/recorded-reasoning-effort-high-stream.sse`）。单独出现的引用和 `thinking` 中的引用遵循 Mistral 的 OpenAPI 文档（`ReferenceChunk`、`ThinkChunk`），见构造的 `fixtures/openai_chat/providers/mistral/streams/reference-chunks.sse`。 |
| `choices[0].delta.reasoning_content`、`delta.reasoning`，以及配置在 `reasoning_fields` 中列出的其他键 | 一次 `BlockStart(Reasoning)`，然后是 `BlockDelta(ReasoningText)`；形式 `chunk.rf` | 仅在列出该键的配置下；否则为数据块 extras `delta.<key>`。文本、拒绝或工具调用开始时，推理块结束。Chat 应用在同一个数据块中收回该键。 |
| `choices[0].delta.refusal` | 一次 `BlockStart(Refusal)`，然后是 `BlockDelta(Refusal)` | |
| `choices[0].delta.tool_calls[]`，新的 `index` | `BlockStart(ToolCall { id, name })` + 针对 `arguments` 的 `BlockDelta(ToolArguments)` | KPP 块索引按块开始的顺序排列；Chat 的 `index` 映射到 KPP 索引。改写过的名称会映射回原名。结构差异（缺少 `id`、`type`、`function`、`name` 或 `arguments`，或出现意外的 `index`）：形式标记 `tc`。未知键：extras `tool_call.<key>` 和 `tool_call.function.<key>`。`type` 不是 `function`：`BlockStart(Unknown)`，其后续条目为 `BlockDelta(Unknown)`。条目没有非空的 `id` 或 `name`（一些兼容 OpenAI 的服务商在后面的 chunk 中才发送）：`BlockStart` 及其后的每个事件都会被暂缓输出，直到两者都已到达、该块结束，或流结束或中断；之后头部带有后到的 `id` 和 `name`，名称映射回原名。第一个条目中为 `""` 的 `id` 或 `name` 由形式标记 `tc` 保留。 |
| `choices[0].delta.tool_calls[]`，已知的 `index` | `BlockDelta(ToolArguments)` | 重复或后到的 `id`、`type` 或 `name`：形式标记 `tc`。调用已有 `id` 时出现另一个非空 `id`，表示复用该索引的新调用：先前的块结束，新的 `BlockStart` 开始，带形式标记 `tc.index`。 |
| `choices[0].finish_reason` | 对每个打开的块发出 `BlockStop`，然后是 `ResponseDelta { stop }` | 映射与回复中相同。`""` 不是结束原因，按 `null` 处理：形式标记 `chunk.cv`。 |
| `usage`（`choices: []` 的 chunk） | `ResponseDelta { usage }` | 用量按[用量](#用量completionusage)处理，保留原始对象。`choices` 为空或缺失：形式标记 `chunk.ch`。 |
| `finish_reason` 之前的 chunk 上的 `usage` | 暂存，形式标记 `chunk.u` | 运行中的用量：基于 vLLM 的提供商在 `stream_options.continuous_usage_stats` 下会在每个 chunk 上发送。KPP 只在最后一个块结束后报告用量，因此解码器保留最新的一份：结束 chunk 自身没有用量时，随它的 `ResponseDelta` 一起发出；结束之后再没有用量时，在 `ResponseStop` 之前作为 `ResponseDelta { usage }` 发出。形式标记 `chunk.u` 保存每个 chunk 的对象，因此请求了用量的 Chat 应用会原样收到每个 chunk；未请求用量的应用收不到用量，只含用量的 chunk 也不会发给它。fixture 是 `fixtures/openai_chat/streams/usage-every-chunk.sse`。 |
| `usage: null`、`logprobs: null`、`finish_reason: null`、`content: null`、`refusal: null` | 形式标记 `chunk.kv`、`cv`、`dv` | |
| `choices[0].logprobs`、选项中的未知键 | 该 chunk 第一个事件上的 extras `choice.<key>` | |
| `delta.audio`、`delta.function_call`、delta 中的未知键 | extras `delta.<key>` | 第一个之后完全没有类型化内容的 chunk 是一个 `Extra`（`{"event": null, "data": <chunk>}`）。 |
| `obfuscation` | 形式标记 `chunk.obf` | 事件来自其他方言时不发送给应用。 |
| `moderation`、chunk 中的未知键 | 该 chunk 第一个事件上的 extras | |
| `data: [DONE]` | `ResponseStop`（在仍打开的块的 `BlockStop` 之后） | 在 `finish_reason` 之后结束但没有 `[DONE]` 的流：带形式标记 `no_done` 的 `ResponseStop`；更早结束的流，无论有没有 `[DONE]`：`Error`，因为回复被截断了。带有 SSE 字段的 `[DONE]` 帧是一个 `Extra`，流随后按没有 `[DONE]` 的流结束。 |
| `data: {"error": {…}}` | `Error`，像不带状态码的错误体一样解码 | 解码器停止。编码为同样的帧；之后没有 `[DONE]`。chunk 同时带有 `choices` 时也是如此，OpenRouter 就是这样发送流中错误的（[errors and debugging](https://openrouter.ai/docs/api/reference/errors-and-debugging)）；fixture 是 `fixtures/openai_chat/streams/error-in-chunk.sse`。 |
| 没有 `error` 对象的 `choices[0].finish_reason: "error"` | `Error`（`ProviderError`） | 不是已完成的回复。该 chunk 保存在 `KppError.raw` 中，并原样发回给 Chat 应用。 |
| 一些兼容 OpenAI 的服务商发送的、带数据的 `event: error` 帧 | `Error` | 像不带状态码的错误体一样解码；编码为 `data:` 帧。没有数据时，这样的帧是一个 `Extra`。 |
| 不是 JSON 的 `data:` 帧、Kaeso 无法读取的 chunk | `Error`（`ProviderError`） | 解码器停止。 |
| SSE 注释、没有数据或数据为空的帧、其他具名事件、`[DONE]` 之后的帧 | `Extra`（`{"comment"}` 或 `{"event", "data", "text", "id", "retry", "other"}`，原帧没有数据时不含 `data`，数据不是 JSON 时带有 `text: true`） | 仅限同一方言；按收到时的样子写回。 |
| 多个选项、`index` 不为 0 的选项 | 带 `Unsupported`（`n`）的 `Error` | |

chunk 分组：每个 chunk 的第一个事件带有形式标记 `chunk`，其余事件带有 `in_chunk`，这样编码器会把它们重新写成一帧。该标记包含 `n`（来自该 chunk 的事件数），另加上面的结构标记。

一旦到达不属于当前 chunk 的事件（其他方言的事件、`[DONE]` 或错误），打开的 chunk 就会被写出。因此被丢弃的事件绝不会让某个 chunk 落到 `[DONE]` 之后。`[DONE]` 或错误之后不再写出任何 chunk。Chat 无法显示的块的工具参数会被省略。来自其他方言、参数始终为空白的工具调用（例如 Claude 对无参数工具的调用）会在其块结束时多收到一个带 `arguments: "{}"` 的 chunk（[工具调用参数](cross-format.md#工具调用参数)）。

SSE 帧的处理遵循规范（[服务器发送事件](codec-design.md#服务器发送事件)）：包含 NUL 的 `id`，或不是能放进 `u64` 的 ASCII 数字的 `retry`，算作未知字段。OpenAI 不会发送 SSE `id`、`retry` 和未知字段。chunk 带有这些字段时，它们以 `{"id", "retry", "other": [[name, value], …]}` 的形式保留在形式标记 `chunk.sse` 中。该 chunk 与其他 chunk 一样解码，这些字段会写回给 Chat 应用。

没有 `data:` 行的帧（例如单独的 `retry: 3000` 或 `event: ping`）按规范不是事件。它成为一个原始 JSON 中没有 `data` 键的 `Extra`，写回时也没有数据行。具名事件和 `[DONE]` 之后的帧完整保留，只回放给 Chat 应用。

OpenAI 在工具调用的第一个 chunk 中发送其 `id`、`type` 和 `function.name`，但 schema 允许它们出现在后面的 chunk 中。一些兼容 OpenAI 的服务商也确实在那里发送 `id` 和 `name`，而且之前的第一个 chunk 是 `choices: []`，`id` 和 `model` 为空。

Anthropic 和 Responses 在块开始时就要给出两者，因此解码器按上文暂缓输出开始事件和该调用，直到知道这些值。事件的顺序从不改变。保持在本格式内的流会原样回放每个 chunk。对应的 fixture 是 `fixtures/openai_chat/streams/late-envelope-and-tool-header.sse`。

来自其他方言的事件各自成为一个 chunk，排在开头的角色 chunk 之后，带有 `logprobs: null` 和 `finish_reason`。工具索引按工具调用开始的顺序分配。推理、引用、不透明数据、未知块和其他方言的 extras 会被省略（[回复](cross-format.md#回复)）。

在这些被省略的内容到达期间，编码器会按节奏写入 SSE 注释（`: keepalive`），SSE 规范和 SDK 的解析器都会忽略它们。其他方言的每个额外事件（例如 Anthropic `ping`）对应一条，每 1,024 字节隐藏文本对应一条（[保活消息](cross-format.md#保活消息)）。`StreamEncoder::keepalive` 为网关提供同样的帧，用于上游静默的情况。

只有应用请求了用量时才会写入用量（`EncodeCtx::stream` 带 `include_usage`）。此时每个 chunk 都有 `usage: null`，最后一个 `choices: []` 的 chunk 携带用量。来自其他方言时，这是最新的用量；后面的报告没有输入计数时，输入计数取自 `ResponseStart`。

## 错误（`ErrorResponse`）

错误体为 `{error: {message, type, param, code}}`。

| 信号 | `ErrorKind` | 对来自其他地方的 KPP 错误的编码 |
|---|---|---|
| `code: context_length_exceeded` | `ContextTooLong` | 400 `invalid_request_error`，code `context_length_exceeded` |
| `type` 或 `code` 为 `insufficient_quota`、计费相关的错误码 | `Billing` | 429 `insufficient_quota`，code `insufficient_quota` |
| `code` `server_is_overloaded` 或 `service_unavailable_error` | `Overloaded` | 见 503 |
| `code` `content_policy_violation`、`content_filter`、`bio_policy`、`cyber_policy`、`misalignment_policy_violation` 或 `image_content_policy_violation` | `ContentFiltered` | 400 `invalid_request_error`，code `content_policy_violation` |
| 400、409、422 | `InvalidRequest`（编码时还有 `Unsupported`） | 400 `invalid_request_error` |
| 401 | `Authentication` | 401 `invalid_request_error` |
| 402 | `Billing` | 429 `insufficient_quota`，code `insufficient_quota` |
| 403 | `PermissionDenied` | 403 `request_forbidden` |
| 404 | `NotFound` | 404 `invalid_request_error` |
| 413 | `RequestTooLarge` | 413 `invalid_request_error` |
| 429 | `RateLimited` | 429 `rate_limit_error`，code `rate_limit_exceeded` |
| 503 | `Overloaded` | 503 `service_unavailable_error`，code `server_is_overloaded` |
| 408、504、`code` `vector_store_timeout` | `Timeout` | 504 `server_error`，code `timeout` |
| 其他 | `ProviderError` | 其 5xx 状态码（503 除外）或 500，`server_error` |
| `error.misalignment`（`Error` schema 中的 `MisalignmentErrorDetailsResource`）、`error` 中的未知键 | 保留在 `KppError.raw` 中（整个错误体） | 只有错误来自 Chat 时才会再次发送。 |

表中列出的错误码（位于 `code` 或 `type` 中）优先于状态码。按状态码确定的类别所对应的错误码是 `rate_limit_exceeded`、`rate_limit_error`、`slow_down`、`invalid_request_error`、`invalid_prompt`、图像相关的错误码以及 `server_error`。它们只在没有 HTTP 状态码的错误中起决定作用：流错误，或 200 回复体中的错误。

对这类错误，400 到 599 之间的数字 `code` 像状态码一样决定类别。OpenRouter 就是这样在 `code` 中发送 HTTP 状态码的，但错误本身仍然没有状态码。

兼容 OpenAI 的服务商（例如 DeepSeek）用 HTTP 402 表示余额不足（[DeepSeek 错误码](https://api-docs.deepseek.com/quick_start/error_codes)），因此无论其 `code` 为何，它都是 `Billing`。

Responses 和 Embeddings 编解码器使用同一张错误码表（`openai_responses::error`），因此一个 OpenAI 格式的错误在每个 OpenAI API 中都有相同的类别、`type` 和 `code`。OpenAI SDK 按状态码选择异常类，所以 `type` 只是附加信息。

`retry-after-ms` 或 `retry-after` 变为 `retry_after`。写出时为以整秒计、向上取整的 `retry-after`，延迟带有不足一秒的部分时，再加上向上取整的 `retry-after-ms`。`param` 为字符串时变为 `KppError.param`。从 Chat 解码的错误会以其原始状态码重新编码，错误体带有 `error` 对象时也沿用原始错误体。

Kaeso 无法作为 `ErrorResponse` 读取的错误体只按 HTTP 状态码确定类别（见上表），并保留原始错误体。没有状态码时（例如在流中），类别为 `ProviderError`。这类错误体包括代理返回的 HTML 429、纯文本 503、`{"error": "rate limited"}`，以及带有 Kaeso 无法承载的键的错误体。

字符串形式的 `error` 成为错误消息。另外三种形式也会被读取：`{"object": "error", "message", "type", "param", "code"}`（Mistral、vLLM），其字符串 `message` 即消息，`type` 和 `code`（数字视为状态码）参与确定类别，`param` 变为 `KppError.param`，流中没有状态码时以 `raw_status_code` 作为状态码；`detail` 下的校验错误，即位于顶层或 `message` 中的 `{loc, msg}` 列表（Mistral 的 `HTTPValidationError`），合成一条消息，每项为 `<msg> (`<path>`)`，以 `; ` 连接，路径按请求的写法给出（去掉 FastAPI 写出的联合成员：`str`、`list[…]`，以及索引后的角色或块类型），只有一项时也放入 `param`；以及字符串 `detail`、`detail` 对象中的 `message` 或单独的字符串 `message`（也包括与 `error: null` 并存时），引用为 `` The provider returned an error (HTTP 404): Not Found. ``。在流中，带有 `object: "error"` 的帧以该错误结束流。否则消息为 `` The provider returned an error (HTTP 503) that is not in the OpenAI error format: <body> ``。错误体在 500 字节后截断，在流中消息没有 HTTP 部分。可读但没有消息的错误体得到 `The provider returned an error without a message.`

编解码器不从文本推断含义。若服务商对错误密钥的回复没有认证状态码，就在 `crates/kaeso-core/src/provider.rs` 的 `refused_key_messages` 中列出该消息。网关随后会把这类错误报告为 `Authentication`（例如 SpaceX 以 HTTP 400 返回的 `Incorrect API key provided`）。同样，若服务商的上下文窗口错误没有自己的错误码，就在 `context_window_messages` 中列出其消息的一部分，网关会把它报告为 `ContextTooLong`（例如 Mistral 的 `too large for model with`）。

## 模型

`GET /v1/models` 和 `GET /v1/models/{model}` 实现在 `crates/kaeso-codecs/src/openai_models.rs` 中，由 Chat、Responses 和 Embeddings 编解码器共用，因为所有 OpenAI API 都以同一种结构列出模型。fixture 位于 `fixtures/openai_chat/models/`，三个编解码器都会对它们做往返测试。来源：规范中的 `Model` 和 `ListModelsResponse` schema，以及 `listModels` 和 `retrieveModel` 操作，其 `x-oaiMeta` 示例被用作 fixture。应用的请求不带 `anthropic-version` 请求头时即为 OpenAI 格式（`dispatch::models_dialect`，见[模型列表](codec-design.md#模型列表)）；网关随后用 `openai_chat` 编码。

### 查询参数

`listModels` 没有记录任何查询参数。每个参数经百分号解码后，作为解码方言的 extra `query.<name>` 保留：值为字符串，重复出现时为数组。格式错误的转义为 `InvalidRequest`。

OpenAI 不分页，因此网关调用 `ModelList::page` 时不设默认上限，OpenAI 应用总是拿到完整列表。路径段 `{model}` 以百分号编码的形式到达（微调模型的 ID 含有 `:`），由网关的路由器解码。

### `Model` ⇄ `kaeso_kpp::ModelInfo`

| 线路字段 | KPP | KPP 没有值时的编码 |
|---|---|---|
| `id` | `id` | （必填） |
| `object`（`"model"`，可选） | 形式标记 `object`，仅在不同时记录 | `"model"` |
| `created`（Unix 秒） | `created_at` | `0` |
| `owned_by` | `owned_by` | `"kaeso"`：所有者未知（例如由 Anthropic 列出的模型），而模型由 Kaeso 网关提供。 |
| `shutdown_date`（`YYYY-MM-DD` 或 `null`） | `retires_at`（该日期的 00:00 UTC） | `null` |
| 未知键 | extras 字段 | |

网关从自己的目录列出模型：把 `owned_by` 设为服务商 ID，并在目录中有发布日期时把 `created_at` 设为该日期。

收到时缺失的字段，或在默认值为具体值的地方收到 `null` 的字段，会记录在形式标记 `none` 中并按同样方式写出；`shutdown_date` 在规范中是可选的。`object` 在规范中是必填的，但 OpenRouter 的模型列表（[`GET /api/v1/models`](https://openrouter.ai/docs/guides/overview/models)）在列表和每个模型中都省略了它，因此按同样方式读取（fixture `providers/openrouter/models/list-recorded.json`）。不是 `full-date` 的 `shutdown_date` 为 `ProviderError`。三个 OpenAI 方言中任一方言的 extras 都会被三个编码器采用。

### `ListModelsResponse` ⇄ `ModelList`

| 线路字段 | KPP |
|---|---|
| `object`（`"list"`，可选） | 形式标记 `object`，仅在不同时记录；缺失或 `null` 记录在形式标记 `none` 中 |
| `data` | `models` |
| 未知键 | extras 字段 |
| 不是对象而是模型的裸数组（Together AI 以此形式发送） | `models`，形式标记 `bare_list` |
| 以 `models` 代替 `data`，每个模型以 `slug` 标识（`GET /v1/models` 对 ChatGPT 套餐访问令牌的响应，见 [Models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)） | `models`，每个 `slug` 作为 `id`，其他字段作为 extras 字段；形式标记 `plan_list` |

对 OpenAI 应用而言会有信息损失：`display_name`、`lifecycle`、`deprecated_at`、`context_window`、`max_output_tokens`、`has_more` 和其他方言的 extras（Anthropic 的 `capabilities`、`line`）没有对应字段，会被省略（[回复](cross-format.md#回复)）。不在 UTC 午夜的 `retires_at` 会截为其 UTC 日期，9999 年之后的则写为 `shutdown_date: null`。`created` 是整数，按存储的值写出，因此毫秒值仍是毫秒值。

`encode_model_request` 构建不带请求头和请求体的 `GET /v1/models/{model}`（规范中的 `retrieveModel`）；密钥由网关添加。`encode_models_request` 构建 `GET /v1/models`（规范中的 `listModels`，它不接受查询参数，因此带有 `limit`、`after_id`、`before_id`、`lifecycle` 或其他参数的查询为 `Unsupported`）。在 `decode_models` 和 `decode_model` 中，非 2xx 状态码交给 `decode_error` 处理。ID 为 `.` 或 `..` 时结果为 `InvalidRequest`，因为 URL 解析器即使在百分号编码后也会去掉这样的路径段。
