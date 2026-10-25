---
title: 跨格式转换
description: 请求或回复从一种 API 格式转到另一种格式时编解码器遵循的规则，以及这些规则的理由。
order: 2
---

应用使用的 API 格式可能与服务商不同。下面是这种情况下编解码器遵循的规则，以及每条规则的理由。保持在自身格式内的请求会被精确重现，这些规则都不适用。

| 来自其他格式的数据 | 在请求中 | 在回复中 |
| --- | --- | --- |
| extras 字段 | `Unsupported`，除非在[允许列表](#extras)中 | 省略 |
| 未知块、工具和工具选择 | `Unsupported`，[工具命名空间](#工具命名空间)、[工具搜索](#工具搜索)和[托管网页搜索](#托管网页搜索)除外 | 省略未知块，网页搜索除外 |
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
| `prompt_cache_key` | request，来自 `openai_chat` | 把请求路由到某个提示缓存。属于缓存提示（[缓存提示](#缓存提示)）。 | `openai_chat/requests/all-params-sdk.json` |
| `prompt_cache_key` | request，来自 `openai_responses` | 同上。 | `openai_responses/requests/sampling-and-ids.json` |
| `client_metadata` | request，来自 `openai_responses` | 请求元数据：轮次和线程 ID、W3C 追踪上下文以及 `x-codex-turn-metadata`。它从不到达模型，记录下来的请求正文会保留它。 | Codex 每一轮都会发送；`openai_responses/requests/codex-namespace-tools.json` |
| `detail`（位于 `input_image` 上；`auto` 已经是形式标记） | image，来自 `openai_responses` | 图像分辨率提示，同 Chat 的 `image_url.detail`。`input_file.detail`（PDF 渲染）**不**在允许列表中。 | `openai_responses/requests/image-base64-file-id.json` |

### 条件条目

这些字段只有取下列值时才能跨格式（`cross::is_droppable_value`，以及 `cross::check_foreign` 中针对单条消息的规则）；其他任何值仍然返回 `Unsupported`。

| 键 | 范围和来源 | 条件与理由 | Fixture / 客户端 |
| --- | --- | --- | --- |
| `context_management` | request，来自 `anthropic_messages` | 只包含 `edits`，其中至少有一个 edit，且每个 edit 都是 `clear_thinking_*`。清除思考块只决定模型能看到哪些较早的 Anthropic `thinking` 块，而这些块从不会到达其他方言（[推理](reasoning.md)）。 | Claude Code 2.1.296 在每个请求中都发送 `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}`；`anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `output_config` | message（`role: "system"`），来自 `anthropic_messages` | `{}`，或只有与请求的 `output_config.effort` 相同的 `effort`。重复请求的推理强度不会改变任何内容。 | Claude Code 2.1.296 会在对话中间的系统消息里重复请求的推理强度；同一个 fixture |

## 工具命名空间

OpenAI Responses 的 `namespace` 工具把多个函数工具归在一个名称下。Codex 用这种方式归组自己的工具和每个 MCP 服务器的工具，并把对其中某个工具的调用作为带 `namespace` 的 `function_call` 发送。OpenAI 原样收到命名空间。其他所有目标都收到普通的函数工具：Anthropic、Chat Completions，以及 SpaceX 的 Responses，它会以 422 拒绝命名空间。

- **工具。** 命名空间中的每个工具都变为名为 `<namespace>__<name>` 的函数工具，例如 `mcp__docs__search`。命名空间的描述放在工具自身描述之前，中间隔一个空行。
- **对话中的调用。** 带 `namespace` 的 `function_call` 得到同样的名称，其 `namespace` 被移除。
- **回复。** Responses 回复或流会把名称与展开后的工具相同的调用映射回 `name` 和 `namespace`。这些名称取自应用的请求（`EncodeCtx::request`），因此应用未放进命名空间的调用保留其名称。
- **名称限制。** 展开后的名称如果不符合目标的工具名称规则，会像其他工具名称一样再被改写，并映射回来。
- **拒绝。** 含有 `type`、`name`、`description` 和 `tools` 以外键的命名空间、其中不是函数的工具、这类工具上只有 Responses 认识的字段（例如 `defer_loading`），以及已被其他工具占用的展开名称。

`EgressCtx::tool_namespaces` 表示 Responses 目标是否接受命名空间。网关根据 `Provider::tool_namespaces` 设置它，只有 OpenAI 为真。

## 工具搜索

Anthropic 的工具搜索按需加载工具。带 `defer_loading: true` 的工具不进入模型的上下文，直到某次搜索返回指名它的 `tool_reference` 块；此后模型在每一轮都能看到它的定义（[工具搜索工具](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool)）。Claude Code 用自己的 `ToolSearch` 工具搜索：每个会话都以一个延迟加载的 `DeferredToolPlaceholder` 开始；`ToolSearch` 找到某个工具后，之后的每个请求都会以延迟加载方式发送该工具，并附上一个带其 `tool_reference` 的 `tool_result`。其他格式没有延迟加载的工具，因此 Kaeso 替它们做 Anthropic 所做的事：

- **工具。** 带 `defer_loading: true` 的工具会被省略，直到对话中的某个 `tool_reference` 指名它，然后作为普通函数工具发送。默认值 `defer_loading: false` 会被省略。
- **引用。** `tool_result` 中的 `tool_reference` 块变为包含该工具名称的文本块，让模型读到搜索加载了哪个工具。
- **像 Anthropic 一样拒绝**，返回 `InvalidRequest` 和 Anthropic 的消息：引用了请求中未发送的工具，以及所有工具都延迟加载的请求。
- **拒绝**并返回 `Unsupported`：强制使用尚未被任何引用加载的延迟工具的 `tool_choice`、不是布尔值的 `defer_loading`，以及位于 `tool_result` 之外或带有 Kaeso 不认识字段的 `tool_reference`。

Anthropic 模型按原样收到延迟加载的工具和引用。开启工具搜索的 beta `advanced-tool-use-2025-11-20` 位于 `anthropic-beta` 请求头中，其他格式不会收到该请求头（[允许列表](#允许列表)）。

## 托管网页搜索

发往 Anthropic 的 OpenAI Responses `web_search` 工具会变为 Anthropic 的服务端工具 `web_search_20250305`，名称为 `web_search`：

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains` | `allowed_domains` |
| `user_location`（`approximate`，含 `city`、`region`、`country`、`timezone`） | `user_location` |
| `external_web_access: true`、`search_context_size: "medium"`（默认值） | 不写入；两者描述的正是 Anthropic 搜索的行为 |
| `external_web_access: false`、其他 `search_context_size`、任何其他字段 | `Unsupported` |

`external_web_access: false` 要求 OpenAI 只读取其页面缓存，这是 Codex 默认设置 `web_search = "cached"` 发送的内容。Claude 总是实时搜索，因此拒绝信息会建议设置 `web_search = "live"` 或关闭网页搜索。其他目标会像拒绝其他格式的任何工具一样拒绝 `web_search`；对于 Chat Completions 模型，拒绝信息会说明如何在 Codex 中关闭网页搜索。

Claude 的搜索以 `web_search_call` 项到达 Responses 应用。名为 `web_search` 的 `server_tool_use` 块变为 ID 为 `ws_kaeso_…`、带 `action: {type: search, query}` 的项。匹配的 `web_search_tool_result` 到达后，其状态为 `completed`；该结果是错误时为 `failed`；回复在它之前结束时为 `in_progress`，例如 `pause_turn`。流会发送 `response.output_item.added`、`response.web_search_call.in_progress`、`.searching`、`.completed` 和 `response.output_item.done`。搜索结果本身和回答中的引用会被省略，与应用格式无法显示的其他数据一样。

应用发回的 `web_search_call` 项在发往 Anthropic 时会被丢弃。Anthropic 只有在带上加密搜索结果时才会重放一次搜索，而这些项不携带结果；使用了这些结果的回答仍留在对话中。带有 Kaeso `ws_kaeso_` ID 的项在发往 OpenAI 时也会被丢弃，因为 OpenAI 从未签发过它们。

## 用量

`Usage::input_tokens` 不含缓存读取和缓存写入，它们是单独的字段。这与 models.dev 的价格列一致，因此费用就是每项计数乘以其价格之和。服务商的原始用量对象总会保留。

## 系统消息

对话开始之后的系统消息保存为 `Role::System` 消息。

- **发往 Anthropic，来自 Anthropic Messages：** 原样发送，由 API 判断它们的位置是否合适、哪些模型支持它们。Claude Code 在每个请求中都会发送这类消息。
- **发往 Anthropic，来自其他方言：** `Unsupported`。Anthropic 只允许部分模型使用它们，并且只能放在用户轮次之后，而 OpenAI 的 system 和 developer 消息可以出现在任何位置。
- **发往 OpenAI：** 成为位置靠后的 `system` 或 `developer` 消息。

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

缓存提示属于提示字段。OpenAI 会自行缓存，因此来自其他方言的缓存提示发往 OpenAI 时会被丢弃。OpenAI 自己的 `prompt_cache_breakpoint` 解码为 KPP 的 `CacheHint`，并且只写回它的来源方言。

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
