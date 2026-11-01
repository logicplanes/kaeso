---
title: 跨格式转换
description: 请求或回复从一种 API 格式转到另一种格式时编解码器遵循的规则，以及这些规则的理由。
order: 2
---

应用使用的 API 格式可能与服务商不同。下面是这种情况下编解码器遵循的规则，以及每条规则的理由。保持在自身格式内的请求会被精确重现，这些规则都不适用。

| 来自其他格式的数据 | 在请求中 | 在回复中 |
| --- | --- | --- |
| extras 字段 | `Unsupported`，除非在[允许列表](#extras)中 | 省略 |
| 未知块、工具和工具选择 | `Unsupported`，[工具命名空间](#工具命名空间)、[工具搜索](#工具搜索)、[托管网页搜索](#托管网页搜索)和 [Codex 工具历史](#codex-工具历史)除外 | 省略未知块，网页搜索除外 |
| 引用 | `Unsupported` | 省略 |
| 服务商文件 ID | `Unsupported`，除非由目标服务商签发 | — |
| 可读推理 | 丢弃；[推理数据块](reasoning.md#推理数据块)会在其来源处恢复它 | 应用有推理形态时，以该形态展示 |
| 不透明推理 | 只回放给签发它的方言和服务商 | 包装在推理数据块中 |
| 缓存提示 | 目标会自行缓存时丢弃 | — |
| 目标没有的采样设置 | `Unsupported` | — |
| 应用格式中没有的停止原因 | — | 映射为该格式中最接近的一个 |

## 请求

`cross::check_foreign` 遍历请求，并以 `Unsupported` 拒绝目标无法使用的内容：

- **来自其他方言的 extras 字段**，除非该字段在[允许列表](#extras)中，或者凭其取值可以跨格式（`cross::is_droppable_value`）。含有字段却没有来源的 extras 则以 `InvalidRequest` 拒绝，因为没有谁能把它们写回。
- **来自其他方言的未知块、工具和工具选择。** 没有 `type` 但有 `role` 的未知块保存的是一整条消息，例如 OpenAI Chat 的 `function` 消息，它被称为 `` `<role>` message ``，提示为“Remove the message, or send the request to a model served through its original format.”。
- **目标服务商无法解析的服务商文件 ID**（[文件](#文件)）。
- **来自其他方言的引用**：没有哪个目标说明过早先轮次中的引用会起什么作用，而且各格式的引用之间无法映射。

以下内容不会被拒绝，因为编码器会把它们丢弃：目标无法回放的推理块（[推理](reasoning.md)），以及目标会自行缓存时的缓存提示（[缓存提示](#缓存提示)）。

`check()` 也会拒绝目标根本无法表达的内容。Anthropic 需要 `max_tokens`，因此对它而言，没有 `sampling.max_output_tokens` 的生成请求属于 `InvalidRequest`，错误中注明应用自己表示该上限的字段，而不会得到一个编造的值。

拒绝时注明的是应用发送的字段，而不是目标的字段：写在 `param` 中，输出上限的拒绝还写在消息中。`cross::app_field` 读取请求的来源，对 OpenAI Chat 还读取请求带的是 `max_completion_tokens` 还是 `max_tokens`（两者都发送时为 `max_completion_tokens`）。字段名遵循应用的格式：

| 被拒绝的设置 | 来自 Anthropic | 来自 Chat | 来自 Responses |
| --- | --- | --- | --- |
| 发往 OpenAI Responses 时低于 16 的输出上限 | `max_tokens` | `max_completion_tokens` 或 `max_tokens` | |
| 发往 Anthropic 时缺少的输出上限 | | `max_completion_tokens` | `max_output_tokens` |
| 停止序列 | `stop_sequences` | `stop` | |
| 输出格式 | `output_config.format` | `response_format` | `text.format` |
| 推理强度 | `output_config.effort` | `reasoning_effort` | `reasoning.effort` |
| 其他推理设置 | `thinking` | `reasoning_effort` | `reasoning` |

在代码中构建、没有来源的请求使用目标的字段名。内容按其在 KPP 中的位置命名（`messages[i].blocks[j]`、`system[i]`、`tools[i]`）。

OpenAI Chat 已弃用的函数调用在 Chat 内部可以往返，但不会转换到其他任何方言。旧式调用没有调用 ID，因此不是 KPP 工具调用：

- 顶层的 `functions` 和 `function_call` 是请求的 extras。拒绝消息例如：`` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``；
- 助手消息的 `function_call` 是该消息的 extra，以 `messages[i].function_call` 被拒绝；
- `function` 消息成为一条包含一个未知块的用户消息，以 `` `function` message `` 被拒绝。

## 回复

回复从一个服务商流向一个应用，拒绝一个已经完成、已经付费的回复，比少显示其中一部分更糟。因此，其他方言的数据按以下优先顺序处理（`cross::reply_policy`、`cross::reasoning_reply_policy`）：

1. 应用的方言能够表示时，**映射**过去。其他方言的推理成为 OpenAI Responses 的推理 item 或 Anthropic 的 `thinking` 块。原始块被包装在一个[推理数据块](reasoning.md#推理数据块)中，由应用原样发回。
2. 无法表示时，**省略**。在流中以[保活消息](#保活消息)帧代替，使连接从不陷入静默。这包括面向 OpenAI Chat 应用的推理（网关在两轮之间[保存它](reasoning.md#由网关保存的推理)）、来自其他方言的未知块和流 extras、外来的引用和 extras。

网关会记录完整的 KPP 回复，因此 Kaeso 的记录中不会丢失任何内容。请求一侧会拒绝外来的服务器工具、引用和类似功能。因此，未知块、引用和 extras 只有在服务商发送了请求没有要求的内容时才会到达另一方言。

应用的格式中没有的停止原因，会映射为该格式中最接近的一个；各方言的映射页面都列出了这些映射。

## Extras

从方言 A 捕获的 Extras 只写回方言 A。其他方言的 `check()` 会以 `Unsupported` 拒绝它们并注明字段。下面允许列表中的字段则会被丢弃。允许列表只收录已知不会改变模型输出的字段，例如 beta 请求头和分辨率提示。

只有已知对模型输出没有任何影响的字段才能加入列表。每个条目都需要一行理由，以及一个展示真实客户端发送该字段的 fixture。

条目带有作用范围（`cross::EXTRAS_ALLOWLIST`）：一个键只在它所在的位置被允许。作用范围包括 `request`、`message`、`tool`、`output`、`reasoning`，以及块类型 `text`、`image`、`document`、`tool_call`、`tool_result` 和 `refusal`。请求头是 `request` 范围内的键 `header:<name>`。

下表中的 fixture 路径都相对于 `crates/kaeso-codecs/fixtures/`。

### 允许列表

| 键 | 范围和来源 | 理由 | Fixture / 客户端 |
| --- | --- | --- | --- |
| `header:anthropic-beta` | request，来自 `anthropic_messages` | Beta 请求头只用于开启服务商的功能。请求实际用到的功能会以请求体字段或块的形式出现，并单独检查。 | Claude Code 在每个请求中都发送 `claude-code-20250219,interleaved-thinking-…` 及其他 beta；`anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`、`anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `header:anthropic-version` | request，来自 `anthropic_messages` | Anthropic 线路格式的 API 版本控制，在其他地方没有意义。等于固定版本的值根本不会被保存（[Anthropic 版本](#anthropic-版本)）。 | Anthropic SDK |
| `header:openai-beta` | request，来自 `openai_chat`、`openai_responses`、`openai_embeddings` | 同 `anthropic-beta`。 | Codex 发送 `OpenAI-Beta: responses=experimental` |
| `cache_control` | request，来自 `anthropic_messages` | 顶层的自动提示缓存。缓存提示属于提示字段（[缓存提示](#缓存提示)）。 | `anthropic_messages/requests/system-string-multi-turn.json`、`explicit-nulls.json` |
| `display`（`thinking.display`） | reasoning，来自 `anthropic_messages` | 只控制是否返回思考文本（`summarized` 或 `omitted`）。发往 OpenAI Responses 时改为 `reasoning.summary: "auto"`，除非其值为 `omitted`（[可读推理与摘要](reasoning.md#可读推理与摘要)）。 | `anthropic_messages/requests/thinking-display-omitted.json`、`thinking-adaptive-effort.json` |
| `image_url.detail` | image，来自 `openai_chat` | 图像分辨率提示（`low`、`high`、`auto`）：它改变的是图像的 Token 成本，而不是提问内容；没有该字段的目标会自行选择分辨率。 | `openai_chat/requests/image-base64-detail.json` |
| `file.filename` | document，来自 `openai_chat` | OpenAI 要求随内联 `file_data` 一起提供的文件名。属于元数据，不是指令。 | `openai_chat/requests/file-pdf-data.json` |
| `prompt_cache_key` | request，来自 `openai_chat` | 把请求路由到某个提示缓存。会传给同样按它路由的 OpenAI Responses，发往其他目标时丢弃（[缓存提示](#缓存提示)）。 | `openai_chat/requests/all-params-sdk.json` |
| `prompt_cache_key` | request，来自 `openai_responses` | 把请求路由到某个提示缓存。会传给 Chat Completions，发往其他目标时丢弃（[缓存提示](#缓存提示)）。 | `openai_responses/requests/sampling-and-ids.json` |
| `model_id` | 仅助手消息，来自 `openai_chat` | 写出某条重放的助手消息的模型，由 Grok CLI 记在它自己的历史里。属于模型从不读取的簿记信息；记录下来的请求正文会保留它。它是 `cross::CHAT_ASSISTANT_BOOKKEEPING` 中唯一的键，Chat 提供商只有在 `with_message_bookkeeping()` 下才会收到（[Chat 提供商](openai-chat-providers.md)）；出现在其他消息上时，发往其他格式的请求会被拒绝；Chat 提供商按原样收到它。 | Grok CLI 在每条重放的助手消息上都会发送；`openai_chat/requests/grok-cli-model-id.json` |
| `client_metadata` | request，来自 `openai_responses` | 请求元数据：轮次和线程 ID、W3C 追踪上下文以及 `x-codex-turn-metadata`。它从不到达模型，记录下来的请求正文会保留它。 | Codex 每一轮都会发送；`openai_responses/requests/codex-namespace-tools.json` |
| `detail`（位于 `input_image` 上；`auto` 已经是形式标记） | image，来自 `openai_responses` | 图像分辨率提示，同 Chat 的 `image_url.detail`。`input_file.detail`（PDF 渲染）**不**在允许列表中。 | `openai_responses/requests/image-base64-file-id.json` |
| `phase`（位于助手消息上：`commentary`、`final_answer`） | message，来自 `openai_responses` | 把助手消息标记为前言或最终回答，便于 OpenAI 在重放该轮时区分它们。两种情况下文本相同，其他格式没有这种标记。 | Codex 把 GPT-6 的消息连同它一起发回；切换到其他模型的会话会带上它们；`openai_responses/requests/codex-model-switch.json` |

### 条件条目

这些字段只有取下列值时才能跨格式（`cross::is_droppable_value`，以及 `cross::check_foreign` 中针对单条消息的规则）；其他任何值仍然返回 `Unsupported`。

| 键 | 范围和来源 | 条件与理由 | Fixture / 客户端 |
| --- | --- | --- | --- |
| `context_management` | request，来自 `anthropic_messages` | 只包含 `edits`，其中至少有一个 edit，且每个 edit 都是 `clear_thinking_*`。清除思考块只决定模型能看到哪些较早的 Anthropic `thinking` 块，而这些块从不会到达其他方言（[推理](reasoning.md)）。 | Claude Code 2.1.296 在每个请求中都发送 `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}`；`anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `text.verbosity` | request，来自 `openai_responses` | `low` 或 `high`（`medium` 是形式标记）。这是输出长度提示（[字段类别](index.md#字段类别)）：配置接受 `verbosity` 的 Chat Completions 目标会在该字段收到它（[服务商配置](openai-chat-providers.md#服务商配置)）；Anthropic 和其他 Chat Completions 目标没有对应字段，因此会省略它，并在 `kaeso-left-out` 中注明。 | Codex 对 GPT 模型发送 `low`，线程切换模型时仍保留它；`openai_responses/requests/multi-turn-roles.json` |
| `output_config` | message（`role: "system"`），来自 `anthropic_messages` | `{}`，或只有与请求的 `output_config.effort` 相同的 `effort`。重复请求的推理强度不会改变任何内容。 | Claude Code 2.1.296 会在对话中间的系统消息里重复请求的推理强度；同一个 fixture |
| `prompt_cache_retention`、`prompt_cache_options` | request，来自 `openai_chat` 和 `openai_responses` | 仅限发往 Anthropic，用于设置缓存保留多久（[缓存提示](#缓存提示)）；其他所有目标仍会拒绝它们。 | `openai_chat/requests/prompt-cache-breakpoint.json`、`openai_responses/requests/item-reference-breakpoint.json` |

## 工具命名空间

OpenAI Responses 的 `namespace` 工具把多个函数工具归在一个名称下。Codex 用这种方式归组自己的工具和每个 MCP 服务器的工具，并把对其中某个工具的调用作为带 `namespace` 的 `function_call` 发送。OpenAI 原样收到命名空间。其他所有目标都收到普通的函数工具：Anthropic、Chat Completions，以及 SpaceX 的 Responses，它会以 422 拒绝命名空间。

- **工具。** 命名空间中的每个工具都变为名为 `<namespace>__<name>` 的函数工具，例如 `mcp__docs__search`。命名空间的描述只放在其第一个工具自身描述之前，中间隔一个空行；其他工具保留自己的描述。若在每个工具上重复，对于拥有数百个工具的 MCP 服务器，提示词会成倍增长。
- **对话中的调用。** 带 `namespace` 的 `function_call` 得到同样的名称，其 `namespace` 被移除。
- **回复。** Responses 回复或流会把名称与展开后的工具相同的调用映射回 `name` 和 `namespace`。这些名称取自应用的请求（`EncodeCtx::request`），因此应用未放进命名空间的调用保留其名称。
- **名称限制。** 展开后的名称如果不符合目标的工具名称规则，会像其他工具名称一样再被改写，并映射回来。
- **拒绝。** 含有 `type`、`name`、`description` 和 `tools` 以外键的命名空间、其中不是函数的工具、这类工具上只有 Responses 认识的字段（例如 `defer_loading`），以及已被其他工具占用的展开名称。

`ResponsesProfile::tool_namespaces` 表示 Responses 目标是否接受命名空间；只有 `ResponsesProfile::OPENAI` 接受。`EgressCtx::with_tool_namespaces` 可为单个请求覆盖该设置。

## 联合类型的工具 schema

Anthropic 和 SpaceX 只接受 schema 根部是对象的函数工具参数。有些应用会在根部放置 `oneOf`、`anyOf` 或 `allOf`：Codex Desktop 的 `automation_update` 工具（位于 `mcp__codex_app` 命名空间中）、由 Zod 的 `z.union` 或 `z.discriminatedUnion` 生成的 MCP 工具，以及根部为 `Union` 的 Pydantic 模型。对于这些目标，Kaeso 会把这样的根部合并为一个对象 schema。工具保留其名称，模型的调用按模型写出的样子返回给应用；应用会根据自己的 schema 校验参数。

| 目标 | 接受的根部 | 来源 |
| --- | --- | --- |
| Anthropic | 根部没有 `oneOf`、`anyOf` 或 `allOf` 的对象 | [Messages API 参考](https://platform.claude.com/docs/en/api/messages/create)把 `input_schema` 的 `type` 定为 `"object"`；根部出现这三者中的任何一个时，Anthropic 会以 400 回应 `input_schema does not support oneOf, allOf, or anyOf at the top level`（2026-10-26 在 Codex Desktop 上实际看到）。 |
| SpaceX，Responses 和 Chat Completions（`ToolSchemaRoot::ObjectUnions`） | 对象，或每个分支都是内联对象的 `oneOf` 或 `anyOf`；`allOf` 只能出现在对象根部 | xAI 的[函数调用指南](https://docs.x.ai/docs/guides/function-calling)中的 Tool Schema Reference，2026-10-26 阅读。2026-10-26 通过 Kaeso 网关实际验证：`$ref` 分支、`string` 分支以及既没有 `type: "object"` 也没有 `properties` 的 `allOf` 根部会被以 `invalid_client_tool_schema` 拒绝；由内联对象组成的联合会被接受，带有 `properties` 但没有 `type` 的 schema 在根部和分支中都会被 SpaceX 视为对象。 |
| OpenAI 和其他 Chat Completions 服务商 | 原样 | OpenAI 和 Mistral 会原样响应 Codex Desktop 的 `automation_update`（2026-10-26 实际看到），因此它们的配置保持 `ToolSchemaRoot::AsSent`。 |

合并方式：

- **变体。** 根部 `oneOf` 或 `anyOf` 的每个分支是一个变体。指向 schema 的 `$defs` 或 `definitions` 的 `$ref` 会被解析，本身是联合的分支会贡献它自己的分支，根部自身的关键字和每个 `allOf` 部分适用于每个变体。
- **精确组合。** 根部、它的 `allOf` 部分和一个分支必须对同一组参数同时成立，因此它们会被精确组合：其中两者都定义的属性会变为这两个定义的 `allOf`，它们的 `required` 列表会合并，带有 `additionalProperties: false` 的部分会排除它没有列出的属性。它们的 `description`、`title` 和 `$comment` 会被拼接，`examples` 会合并列出。`unevaluatedProperties: false` 也以同样方式封闭它所在的对象，范围是该对象内定义的所有属性；但当 `$schema` 指定 draft 7 或更早的版本时除外，这些版本会忽略该关键字。`$ref` 旁边的关键字遵循 `$schema` 所指定的草案：draft 7 及更早的版本会忽略它们，2019-09 及更新的版本会应用它们。
- **属性。** 合并后的对象包含所有变体的属性。在不同变体之间有差异的属性会变为其各种形式的 `anyOf`；但同一属性的 `enum` 和 `const` 值在其注解（例如 `description`，包括 `$ref` 旁边的注解）一致时会合并为一个 `enum`，例如 `mode`。只有部分变体定义的属性会保留这些定义，即使另一个变体只是因为不限制额外属性才会接受它；因此遵循合并后 schema 的每次调用仍然符合某个变体。
- **`required`。** 每个变体都要求的属性；`allOf` 部分会加入它们自己的。
- **`additionalProperties`** 和其他关键字在每个变体的值都相同时保留，例如 `additionalProperties: false`。根部的 `$defs`、`definitions`、`$schema`、`title` 和 `description` 保持不变，因此属性中的引用仍能解析。
- **描述。** 有多个变体时，工具描述的末尾会列出这些变体，让模型仍然知道哪些字段要一起使用：`1. mode "view": requires mode, id.` 只有部分变体才有、因而合并后的对象无法携带的关键字，会在其变体那一行中注明，例如 `; with minProperties 1`；分支自己的 `description` 位于该行末尾。合并后的对象接受所有组合；工具接受哪些参数仍由应用自己的 schema 决定。
- **不变。** 属性内部的联合、目标接受的根部联合，以及工具调用的参数。
- **拒绝。** 没有分支、因而没有任何参数符合的根部联合，根部联合中有不是对象的分支（例如 `{"type": "string"}`）、`$ref` 没有指向 schema 的 `$defs` 或 `definitions`、按所在位置解析的 `$dynamicRef` 和 `$recursiveRef`、根部以下设定相对引用基准的 `$id`、合并会移动或改变其目标的本地 `$ref`（例如指向某个分支自己的 `$defs` 或指向 `#`）、无法精确组合的部分（例如两个不同的 `minProperties`，或 `additionalProperties: false` 与另一部分要求的属性或另一部分的 `patternProperties` 相冲突）、与 `patternProperties` 或条件关键字并存的 `unevaluatedProperties`、`$schema` 未指定草案时 `$ref` 旁边除注解以外的关键字，以及根部组合出超过 64 个变体时，都是 `Unsupported`。错误消息会注明该工具，`param` 是它在请求中的路径。

每个被合并的工具都会以它在请求中的路径出现在回复的 `kaeso-adapted` 响应头中，例如第一个命名空间的第四个工具为 `tools[0].tools[3]`（[响应头](../../api/model-api.md#响应头)），网关日志中也会记录。记录下来的请求正文保留应用的 schema。`ChatProfile` 和 `ResponsesProfile` 上的 `ToolSchemaRoot` 说明服务商接受什么；Anthropic Messages 总是收到一个对象。Fixture：`openai_responses/requests/codex-desktop-root-union.json`。

## 工具搜索

Anthropic 的工具搜索按需加载工具。带 `defer_loading: true` 的工具不进入模型的上下文，直到某次搜索返回指名它的 `tool_reference` 块；此后模型在每一轮都能看到它的定义（[工具搜索工具](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool)）。Claude Code 用自己的 `ToolSearch` 工具搜索：每个会话都以一个延迟加载的 `DeferredToolPlaceholder` 开始；`ToolSearch` 找到某个工具后，之后的每个请求都会以延迟加载方式发送该工具，并附上一个带其 `tool_reference` 的 `tool_result`。其他格式没有延迟加载的工具，因此 Kaeso 替它们做 Anthropic 所做的事：

- **工具。** 带 `defer_loading: true` 的工具会被省略，直到对话中的某个 `tool_reference` 指名它，然后作为普通函数工具发送。被 `tool_choice` 强制使用的延迟工具也会发送：Claude 接受在搜索之前强制使用它，而其他格式只能强制使用已收到的工具。默认值 `defer_loading: false` 会被省略。
- **引用。** `tool_result` 中的 `tool_reference` 块变为包含该工具名称的文本块，让模型读到搜索加载了哪个工具。
- **像 Anthropic 一样拒绝**，返回 `InvalidRequest` 和 Anthropic 的消息：引用了请求中未发送的工具，以及所有工具都延迟加载的请求。
- **拒绝**并返回 `Unsupported`：不是布尔值的 `defer_loading`，以及位于 `tool_result` 之外或带有 Kaeso 不认识字段的 `tool_reference`。

Anthropic 模型按原样收到延迟加载的工具和引用。开启工具搜索的 beta `advanced-tool-use-2025-11-20` 位于 `anthropic-beta` 请求头中，其他格式不会收到该请求头（[允许列表](#允许列表)）。

## 托管网页搜索

OpenAI Responses 把它的托管搜索工具命名为 `web_search`，更早的名称是 `web_search_2025_08_26`、`web_search_preview` 和 `web_search_preview_2025_03_11`；Kaeso 对这四个名称一视同仁。发往 Anthropic 时，该工具变为 Anthropic 的服务端工具 `web_search_20250305`，名称为 `web_search`：

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`、`filters.excluded_domains`（二选一；空列表视为没有） | `allowed_domains`、`blocked_domains` |
| `enable_image_understanding`、`enable_image_search`（xAI 的字段） | 省略，为 `true` 时在 `kaeso-left-out` 中注明 |
| `user_location`（`approximate`，含 `city`、`region`、`country`、`timezone`） | `user_location` |
| `external_web_access: true`、`search_context_size: "medium"`（默认值）、`search_content_types: ["text"]` | 不写入；它们描述的正是 Anthropic 搜索的行为 |
| 含 `image` 的 `search_content_types` | 省略，并在 `kaeso-left-out` 中注明；Anthropic 的搜索只返回文本结果 |
| `external_web_access: false` | [省略](#省略的工具)该工具 |
| 其他 `search_context_size`、任何其他字段 | `Unsupported` |

发往 SpaceX 的 Responses API（`WebSearchTool::Xai`）时，该工具保留 xAI 为它记载的字段（[网页搜索](https://docs.x.ai/developers/tools/web-search)，2026-10-25 阅读）：

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`、`filters.excluded_domains`（各最多五个，不能同时使用） | 原样发送 |
| `enable_image_understanding`、`enable_image_search` | 原样发送 |
| 含 `image` 的 `search_content_types` | `enable_image_search: true` |
| `external_web_access: true`、`search_context_size: "medium"`、`search_content_types: ["text"]` | 不写入；SpaceX 拒绝 `external_web_access`，并且总是实时搜索 |
| `external_web_access: false` | [省略](#省略的工具)该工具 |
| 超过五个域名、同时使用两个列表、`user_location`、其他 `search_context_size`、任何其他字段 | `Unsupported` |

`external_web_access: false` 要求 OpenAI 只读取其页面缓存，这是 Codex 默认设置 `web_search = "cached"` 发送的内容。Claude 和 SpaceX 总是实时搜索。实时搜索会读取应用要求 Kaeso 不去获取的页面，因此改为省略这次搜索。Chat Completions 没有托管搜索，因此在那里也会省略该工具。OpenAI 原样收到该工具。

Anthropic 的网页搜索工具走相反的方向：Claude Code 的 `WebSearch` 发送的 `web_search_20250305`，以及 `web_search_20260209` 和 `web_search_20260318`。Responses 目标收到托管的 `web_search` 工具，搜索次数上限变为请求中的一个字段（[网页搜索工具](https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool)）：

| Anthropic | OpenAI | SpaceX |
| --- | --- | --- |
| `allowed_domains` | `filters.allowed_domains` | `filters.allowed_domains` |
| `blocked_domains` | `filters.blocked_domains` | `filters.excluded_domains` |
| `user_location`（`approximate`，含 `city`、`region`、`country`、`timezone`） | `user_location` | `Unsupported` |
| `max_uses` | `max_tool_calls` | `max_turns` |
| 值为 `["direct"]` 或该版本默认值的 `allowed_callers` | 不写入；托管搜索就是直接搜索 | 不写入 |
| `response_inclusion`（`web_search_20260318`） | 不写入；它只影响从代码执行中发起的搜索 | 不写入 |
| `cache_control` | 不写入；Responses 会自行缓存（[缓存提示](#缓存提示)） | 不写入 |
| 不是 `approximate` 的位置、任何其他字段 | `Unsupported` | `Unsupported` |

- **两个域名列表**同时出现在一个工具中时为 `InvalidRequest`，使用 Claude 自己的消息（`anthropic_messages/errors/web-search-both-domain-lists.json`）。
- **动态过滤。** 从 `web_search_20260209` 起，除非 `allowed_callers` 为 `["direct"]`，Claude 会通过运行代码来过滤结果。Responses 模型在两种情况下都直接读取结果。
- **来源。** 请求会带上 `include: ["web_search_call.action.sources"]`，因为 OpenAI 只有在被要求时才会列出一次搜索找到的页面。
- **上限。** OpenAI 的 `max_tool_calls` 也计入打开页面和在页面内搜索，因此比 `max_uses` 更严格。ChatGPT 套餐的路由会拒绝 `max_tool_calls`，因此在那里该上限会被省略（[ChatGPT 套餐用量](openai-responses.md#chatgpt-套餐用量)）。
- **SpaceX** 会忽略 `max_tool_calls`，并且会不报错地丢弃 `blocked_domains`，因此它收到自己的字段名。它的 `max_turns` 限制的是工具调用的轮数，一轮可能搜索不止一次。它的其他限制同样适用：每个列表最多五个域名。
- **强制搜索。** 强制使用 `web_search` 的 `tool_choice` 为 `Unsupported`；`auto` 和 `any` 让模型自行决定。
- **Chat Completions** 没有托管搜索，因此该工具在那里会被省略。

## 省略的工具

这些工具会从请求中省略，而不是被拒绝（`cross::TOOLS_LEFT_OUT`）。每个工具都只是向模型提供一项可用可不用的能力，而 Codex 这样的应用会把它加到每个请求中，且无法按模型关闭：如果拒绝它，所有不具备该工具的模型都无法在该应用中使用。模型的回答与没有该工具时相同。每个被省略的工具都会在回复的 `kaeso-left-out` 响应头（[响应头](../../api/model-api.md#响应头)）和网关日志中注明，记录的请求正文保留应用的工具。

| 工具 | 来自 | 发往 | 条件 |
| --- | --- | --- | --- |
| `web_search` | `openai_responses` | `openai_chat` | 始终省略：Chat Completions 没有托管搜索 |
| `web_search` | `openai_responses` | `anthropic_messages` | `external_web_access: false`：Claude 总是实时搜索 |
| `web_search` | `openai_responses` | `openai_responses` | `external_web_access: false`，且服务商的配置含 `WebSearchTool::Xai`：SpaceX 总是实时搜索 |
| `web_search_20250305` | `anthropic_messages` | `openai_chat` | 总是：Chat Completions 没有托管搜索 |
| `web_search_20260209` | `anthropic_messages` | `openai_chat` | 总是：Chat Completions 没有托管搜索 |
| `web_search_20260318` | `anthropic_messages` | `openai_chat` | 总是：Chat Completions 没有托管搜索 |

要求使用被省略工具的 `tool_choice`（直接指定或在 `allowed_tools` 中）为 `Unsupported`。所有工具都被省略时，`tool_choice: "auto"` 或 `"none"` 以及 `parallel_tool_calls` 也随之省略，并在 `kaeso-left-out` 中注明，因为服务商在没有工具时会拒绝它们。`tool_choice: "required"` 或指定某个工具为 `Unsupported`，因为已没有可调用的工具。只有 OpenAI 的 Responses API 接受应用在没有工具的请求中发送的 `tool_choice`，例如 Codex 的辅助调用；对其他所有提供商，`auto` 和 `none` 同样省略，强制使用某个工具则为 `invalid_request`。

Claude 的搜索以 `web_search_call` 项到达 Responses 应用。名为 `web_search` 的 `server_tool_use` 块变为 ID 为 `ws_kaeso_…`、带 `action: {type: search, query}` 的项。匹配的 `web_search_tool_result` 到达后，其状态为 `completed`；该结果是错误时为 `failed`；回复在它之前结束时为 `in_progress`，例如 `pause_turn`。流会发送 `response.output_item.added`、`response.web_search_call.in_progress`、`.searching`、`.completed` 和 `response.output_item.done`。搜索结果本身和回答中的引用会被省略，与应用格式无法显示的其他数据一样。

应用发回的 `web_search_call` 项在发往 Anthropic 和 Chat Completions 时会被丢弃。Anthropic 只有在带上加密搜索结果时才会重放一次搜索，而这些项不携带结果；Chat Completions 没有这种项。使用了这些结果的回答仍留在对话中。带有 Kaeso `ws_kaeso_` ID 的项在发往任何 Responses 目标时也会被丢弃，因为没有哪个 Responses 服务商签发过它们。每个被丢弃的项都会在 `kaeso-left-out` 中按类型和 ID 注明，例如 `web_search_call ws_67c9…`。

Responses 模型的搜索会像 Claude 的搜索一样到达 Anthropic 应用。每个进行了搜索且已完成或失败的 `web_search_call` 项都变为名为 `web_search` 的 `server_tool_use` 块，ID 为 `srvtoolu_kaeso_…`，带 `input: {query}`，其后是一个 `web_search_tool_result`。已完成的搜索把它的来源列为 `web_search_result` 条目：`title` 重复 `url`，`encrypted_content` 为空，因为 Responses 只报告地址，`page_age` 为 `null`。失败的搜索变为错误 `unavailable`。

流会在某次搜索的项完成后发送它：`server_tool_use` 的开始、一个带查询的 `input_json_delta`，然后是结果。回复结束时仍在进行的项、打开页面或在页面内搜索的项（`open_page`、`find_in_page`），以及回答中的 `url_citation` 注释会被省略，与应用格式无法显示的其他数据一样。`usage.server_tool_use.web_search_requests` 统计已完成的搜索，因为失败的搜索不计费。

应用把名为 `web_search` 的 `server_tool_use` 块及其 `web_search_tool_result` 块发往其他格式时，这些块会被丢弃，指向它们的 `web_search_result_location` 引用也一并丢弃，就像发往 Anthropic 时的 `web_search_call` 项一样。使用了它们的回答仍会保留。Claude 原样收回自己的块；带有 Kaeso `srvtoolu_kaeso_` ID 的块对 Claude 也会被丢弃，因为 Claude 无法在没有加密内容的情况下重放结果。每个被丢弃的块都会在 `kaeso-left-out` 中按类型和 ID 注明，例如 `server_tool_use srvtoolu_01…`；被丢弃的引用按所在块注明，例如 `messages[1].blocks[2].citations`。

## Codex 工具历史

在 GPT 模型上开始的 Codex 线程包含只有 OpenAI Responses 才有的项。线程在 Anthropic 或 Chat Completions 上继续时，这些项变为普通的工具调用和结果，因此模型保留历史：

| Responses 项 | KPP |
| --- | --- |
| `custom_tool_call`（自由格式工具，例如 Codex 的 `apply_patch`） | 以该工具命名的 `Block::ToolCall`（带 `namespace` 时为 `<namespace>__<name>`），其 `input` 文本作为参数 `input` |
| `custom_tool_call_output` | `Block::ToolResult`，按 `function_call_output` 的方式读取 |
| `local_shell_call` | 名为 `local_shell` 的 `Block::ToolCall`，其 `action` 对象作为参数 |
| `local_shell_call_output` | 对应其 `id` 所指调用的 `Block::ToolResult`；Codex 改为发送 `function_call_output` |

OpenAI 原样收到这些项，SpaceX 原样收到其中的 `custom_tool_call` 和 `custom_tool_call_output`：SpaceX 能根据这样的历史作答（`openai_responses/providers/xai/responses/recorded-codex-gpt-history.json`）。SpaceX 接受的是 `shell_call` 项，因此 `local_shell_call` 和 `local_shell_call_output` 按上文作为工具调用和结果发给它。发给 SpaceX 时会省略 GPT 的推理项，因为 SpaceX 无法解密 OpenAI 的 `encrypted_content`，会拒绝该请求（参见 [SpaceX](openai-responses.md#spacex)）。不符合上述形状的项仍是未知块，并像未知块一样被拒绝。

## Codex 智能体消息

Codex 运行子智能体时，智能体之间用 `agent_message` 输入 item 通信：根智能体交给子智能体的任务（`NEW_TASK`），以及各智能体的回复，带有 `author` 和 `recipient` 路径，例如 `/root/request_path`。只有 OpenAI 能读取它们。其他所有提供商在每种格式下都把它们作为消息收到：

- **角色。** 发起请求的智能体自己写的消息成为 `assistant` 消息，收到的消息成为 `user` 消息。若第一条智能体消息之前没有来自模型的内容（子智能体的第一次请求），发起请求的智能体就是它的接收方，否则是它的作者（根智能体）。
- **文本。** 消息的 `input_text` 和 `output_text` 部分，以及 Codex 写入任务明文的 `encrypted_content` 部分。Codex 自己的头部（`Message Type: …`、`Sender: …`）保留在开头；没有头部时，文本以 `Message from <author> to <recipient>:` 开头。
- **省略。** item 的 `id`，只有 OpenAI 能解析。每条消息都在回复的 `kaeso-adapted` 头部中列出，例如 `agent message amsg_…`。

若 `encrypted_content` 是密文（一段连续的 base64），只有 OpenAI 能读取，因此在其他地方都是 `Unsupported`，其他类型的部分同样如此。Fixtures：`openai_responses/providers/xai/requests/codex-multi-agent.json`（子智能体的第一次请求）和 `codex-multi-agent-root.json`（根智能体之后的一轮）。

## 工具调用参数

在每种格式中，工具调用的参数都是一个 JSON 对象。Chat Completions 和 Responses 以字符串传递参数，因此调用到达时携带的文本可能不是对象。Kaeso 在每条路径上都以相同方式处理这类文本：

- **空白参数。** 调用没有参数的工具（例如 Codex 的 `get_goal`）时，参数为 `{}`。Claude 流式传输这类调用时不带参数文本，因此 Chat Completions 和 Responses 应用会在该块结束时收到 `"{}"`，因为 `arguments` 以 JSON 形式保存调用的参数（`ChatCompletionMessageToolCall`、`FunctionToolCall`）。应用发回的空白 `arguments` 读取为 `{}`；与应用格式相同的提供商按原样收到它们。Fixture：`anthropic_messages/streams/tools-no-input.sse`、`openai_responses/requests/codex-desktop-blank-arguments.json`。
- **对话中的其他值（发往 Anthropic 时）。** `tool_use.input` 必须是对象，只要历史中有一个不是，Anthropic 就会拒绝整个请求。因此，来自其他格式、参数为其他值的调用会变为对象：不是 JSON 的文本（例如被输出上限截断的参数）变为 `{"input": "<该文本>"}`，与[自由格式工具的输入](#codex-工具历史)相同；数组、数字或布尔值变为 `{"input": <该值>}`；`null` 变为 `{}`。模型能看到它写下的内容以及随后的工具结果。每个这样的调用都会在 `kaeso-adapted` 中以 `tool call` 加其 ID 注明，例如 `tool call call_abc`。Chat Completions 和 Responses 的提供商按原样收到参数。Anthropic 应用自己的 `tool_use` 若 `input` 不是对象，仍为 `Unsupported`。
- **回复。** 发给 Anthropic 应用的回复中，若工具调用的参数不是对象，会被拒绝，因为应用会用模型从未写过的参数运行工具（[Anthropic 回复](anthropic-messages.md#回复)）。

## 用量

`Usage::input_tokens` 不含缓存读取和缓存写入，它们是单独的字段。这与 KPMR 目录中的价格一致，因此费用就是每项计数乘以其价格之和。服务商的原始用量对象总会保留。

## 系统消息

对话开始之后的系统消息保存为 `Role::System` 消息。

- **发往 Anthropic，来自 Anthropic Messages：** 原样发送，由 API 判断它们的位置是否合适、哪些模型支持它们。Claude Code 在每个请求中都会发送这类消息。
- **发往 Anthropic，来自其他方言：** 文本并入下一个用户轮次，排在其工具结果之后；后面没有用户轮次时，并入最后一个用户轮次。线程切换模型（`<model_switch>`）或模式（`<collaboration_mode>`）时，Codex 会发送这类消息。Anthropic 只接受紧跟在用户轮次之后、紧接在助手轮次之前的系统消息，而且不是每个模型都支持（[对话中间的系统消息](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)）。OpenAI 应用则可以把它们放在任何位置，通常放在它们引出的轮次之前。消息的字段与任何消息的字段经过同样的检查。开头的消息（位于任何用户或助手消息之前）并入系统提示词；其中大多数已由解码器作为系统提示词。若其后是最后一条助手消息（Anthropic 会把它当作预填充继续），文本则并入最后一个用户轮次。
- **发往 OpenAI：** 成为位置靠后的 `system` 或 `developer` 消息。发往 Chat Completions 时，位于助手消息的工具调用与其结果之间的此类消息会移到结果之后，因为 Mistral 拒绝那里的系统消息（`Unexpected role 'tool' after role 'system'`）。

## 保活消息

服务商可能持续流式输出好几分钟，却不产生任何应用格式能够显示的内容：面向 OpenAI Chat 应用的 Claude 思考、Anthropic 的 `ping` 事件、另一方言的未知块，或者推理期间什么都不发送的 OpenAI 推理模型。带空闲超时的代理（nginx 和 AWS ALB 为 60 秒，Cloudflare 为 100 秒）会切断连接，而首字节时间会等于整个思考时间。因此实时流从不会因为格式转换而陷入静默：每个流编码器都有一个在其方言中有效、并被官方 SDK 忽略的保活帧。

| 方言 | 保活帧 | SDK 为何忽略它 |
| --- | --- | --- |
| OpenAI Chat Completions 和 Responses | SSE 注释 `: keepalive` 加一个空行 | 在 SSE 规范中，以冒号开头的行是注释，OpenAI SDK 的解析器会跳过它们 |
| Anthropic Messages | `event: ping` 加 `data: {"type": "ping"}`，与 Anthropic 发送的完全一致 | Anthropic SDK 会跳过 ping 事件 |

保活帧以两种方式到达线路：

- **带内。** 当来自另一方言的事件没有产生任何字节时，编码器把保活帧加入 `StreamEncoder::encode` 的返回值。一个不依赖时钟的节拍器对此加以限制。另一方言的额外事件（例如 `ping`）总能换来一帧，因为服务商已经控制了它的节奏。隐藏文本（例如应用看不到的思考或缓冲中的工具参数）每 `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` 字节（即 1,024 字节）换来一帧。编码器写出任何内容都会重置计数。应用自身方言的事件从不换来保活帧，因此同一方言内的回放保持精确。
- **由网关驱动。** `StreamEncoder::keepalive` 按需返回保活帧，用于编码器无法察觉的上游静默。网关在 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（默认 15 秒）内没有向应用写出任何内容时，就会写出它。

OpenAI Chat 编码器还会在响应一开始就发送它的角色 chunk，而不是等到第一段文本。这样在模型思考期间应用也能收到流的消息。

## 结构化输出

发往 Anthropic 的结构化输出使用 Anthropic 原生的结构化输出，从不通过强制工具来模拟。

- 输出格式 `json_object` 在 Anthropic 中没有对应项，为 `Unsupported`。
- Anthropic 的输出格式只有一个 schema：schema 的 `name` 会被丢弃，而 `description` 或 `strict: false` 为 `Unsupported`。
- 从 Anthropic 发往 OpenAI 的 schema 会得到名称 `response`，因为 OpenAI 要求必须有名称。

## 采样参数

目标无法遵守的采样设置为 `Unsupported`，从不丢弃或重新缩放：

| 目标 | 拒绝 |
| --- | --- |
| Anthropic | `seed`、频率惩罚和存在惩罚，以及超出 Anthropic 取值范围 0 到 1 的 `temperature` |
| OpenAI Chat Completions | `top_k`，以及超过四个的停止序列 |
| OpenAI Responses | `top_k`、`seed`、各项惩罚和停止序列，Responses 没有这些参数；以及低于 16 个 Token 的输出上限，即 `CreateResponse` 的最小值 |

数字在同一方言内保持原来的写法：只要值未改变，`temperature: 1` 回来时仍是 `1`，`1.0` 仍是 `1.0`。跨方言时使用目标自己的写法，因此来自 Chat 的 `temperature: 1` 到达 Anthropic 时是 `1.0`。

KPP 没有 `n`：`n` 大于 1 的 Chat 请求在解码时就会被拒绝，与目标无关。Chat 的 `logprobs`、`logit_bias` 和 `top_logprobs` 是 extras，其他每个方言都会拒绝它们（[Extras](#extras)）。

## 有状态的 Responses 功能

`previous_response_id`、`conversation`、`store: true` 和 `background: true` 需要在 OpenAI 端保存状态，编解码器以 `Unsupported` 拒绝它们。只有网关能在进入 KPP 之前解析它们。

## 回复 ID

无论应用使用哪种格式，回复都保留服务商的 ID 不变。Claude 的回复以其 Anthropic ID（`msg_…`）到达 OpenAI Chat Completions 或 Responses 应用。OpenAI 的回复以 OpenAI 的 ID（`chatcmpl-…` 或 `resp_…`）到达 Anthropic Messages 应用。流中携带同一个 ID。

编解码器从不按应用自身格式的样式编造 ID。服务商的 ID 在服务商处标识该回复，而应用把回复 ID 视为不透明的字符串。Responses 输出项是一个例外，因为其他格式没有它们（[Responses 的存储与 ID](#responses-的存储与-id)）。没有开始事件的 Responses 流也是例外，它会得到 ID `resp_kaeso_<created_at>`（[OpenAI Responses](openai-responses.md)）。

## Responses 的存储与 ID

- **`store: false`。** 来自其他方言、编码给 OpenAI Responses 的请求带有 `"store": false`：Kaeso 是无状态的，没有这一项，OpenAI 会存储每一段经过转换的对话。从 Responses 解码的请求保留应用自己的写法，因此缺少 `store` 仍意味着 OpenAI 会存储该响应。
- **item ID。** Responses 的输出 item 需要 ID，而来自其他方言的回复没有 ID。Kaeso 为它们分配以 `msg_kaeso_`、`fc_kaeso_` 和 `rs_kaeso_` 开头的 ID。应用把它们发回时，请求解码器会移除这些 ID，因此 OpenAI 永远不会收到不是它签发的 ID。

## 缓存提示

缓存提示属于提示字段，但对于只在被要求时才缓存的目标，Kaeso 仍会提出要求。

- **OpenAI 以及其他 Chat Completions 和 Responses 服务商**会自行缓存，因此来自其他方言的缓存提示发往它们时会被丢弃。OpenAI 自己的 `prompt_cache_breakpoint` 解码为 KPP 的 `CacheHint`，并且只写回它的来源方言。`prompt_cache_key` 会随请求在 Chat Completions 和 Responses 之间传递，使服务商继续把同一会话发往同一个缓存。
- **Anthropic** 只缓存到 `cache_control` 断点为止（[提示词缓存](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)）。来自其他方言、且不带任何缓存提示的生成请求会得到三个断点：
  - 一个在最后一个函数工具上，这样即使系统提示词变化，工具列表也能从缓存读取；
  - 一个在最后一个系统块上，这样即使某一轮新增的块超过 Anthropic 从断点向前查找的 20 个块，工具和系统提示词也能从缓存读取；
  - 一个在对话的最后一个内容块上，它会随着每一轮向前移动。

  这三个断点都放在块上，而不是使用顶层自动缓存，因此只认识块级断点（即 Claude Code 发送的形式）的 Anthropic 兼容服务商也能接受。
- **应用自己的提示优先。**带有缓存提示的请求只保留这些提示。Anthropic 最多接受 4 个断点，应用发送更多时会省略最早的那些。OpenAI 的 `prompt_cache_options` 设为 `mode: explicit` 且没有断点时不缓存任何内容，与 OpenAI 上的行为一致。Token 计数不会得到断点。
- **有效期。**Anthropic 会把缓存保留 5 分钟或 1 小时。当应用要求 OpenAI 保留超过 5 分钟时（`prompt_cache_retention: "24h"`，或 `30m` 这样的 `prompt_cache_options.ttl`），发往 Anthropic 的每个断点都会请求 `1h`。
- **每一轮都重复上一轮。**对话的第 *n+1* 轮编码后的工具、系统提示词、思考设置和消息与第 *n* 轮相同，后面再接新的消息；只有对话断点会移动，而 Anthropic 不把断点计入前缀，因此每一轮都能读取上一轮写入的缓存（`crates/kaeso-codecs/tests/prompt_cache.rs`）。

## 工具错误

对于没有错误标志的目标，即 OpenAI Chat 和 Responses，`ToolResult.is_error` 是提示字段：它会被丢弃，由结果的内容承载错误文本。如果拒绝它，Claude Code 这类使用 Anthropic 格式的客户端的每一次失败的工具调用，都将无法路由到 OpenAI。

## 文件

服务商文件 ID（`file-…`）只能在该服务商的文件存储中使用。两种 OpenAI 生成方言共用同一个文件存储，因此 `cross::check_foreign` 接受目标的 `Origin`，即其方言和服务商。跨到另一服务商的文件 ID 为 `Unsupported`。

## Anthropic 版本

Anthropic 编解码器固定使用一个 API 版本 `2023-06-01`，导出为 `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`。它是 Anthropic 的[版本说明页面](https://platform.claude.com/docs/en/api/versioning)列出的唯一版本，是官方 SDK 发送的版本，也是每个 Anthropic fixture 所假定的版本。

- **出站。** Anthropic 编解码器编码的每个请求都带有 `anthropic-version: 2023-06-01`，除非应用自己的 Anthropic 请求转发了一个指向更新版本的 `anthropic-version`，即格式正确、排序在固定版本之后的 `YYYY-MM-DD`。格式错误或更旧的值会被忽略。来自其他方言的请求从不携带该请求头，因此总是得到固定版本。
- **入站。** 等于固定版本的转发 `anthropic-version` 不会存入请求的 extras。编解码器无论如何都会发送它，而保存它会把每个 Anthropic SDK 请求都绑定到其方言。其他任何值都保存为 extra `header:anthropic-version`，并在出站时按上述方式比较。
- **为什么较新的版本优先。** Anthropic 对一个版本只做增量变更，增加新的可选输入和新的输出值，而请求更新版本的应用依赖于该版本的行为。解码器能容忍未知的字段、块和事件，因此更新的版本会退化为 extras 和未知项，而不会导致失败。

要修改固定版本，需要修改该常量，用 `cargo xtask record` 重新录制 fixture（[录制](fixtures.md#录制真实响应)），并更新本节。
