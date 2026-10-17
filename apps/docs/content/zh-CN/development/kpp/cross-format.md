---
title: 跨格式转换
description: 请求或回复从一种 API 格式转到另一种格式时编解码器遵循的规则，以及这些规则的理由。
order: 1
---

应用使用的 API 格式可能与服务商不同。下面是这种情况下编解码器遵循的规则，以及每条规则的理由。保持在自身格式内的请求会被精确重现，这些规则都不适用。

| 来自其他格式的数据 | 在请求中 | 在回复中 |
| --- | --- | --- |
| Extras 字段 | `Unsupported`，除非在[允许列表](extras-allowlist.md)中 | 省略 |
| 未知块、工具和工具选择 | `Unsupported` | 省略未知块 |
| 引用 | `Unsupported` | 省略 |
| 服务商文件 ID | `Unsupported`，除非由目标服务商签发 | — |
| 可读推理 | 丢弃；[推理数据块](#推理数据块)会在其来源处恢复它 | 应用有推理形态时，以该形态展示 |
| 不透明推理 | 只回放给签发它的方言和服务商 | 包装在推理数据块中 |
| 缓存提示 | 目标会自行缓存时丢弃 | — |
| 目标没有的采样设置 | `Unsupported` | — |
| 应用格式中没有的停止原因 | — | 映射为该格式中最接近的一个 |

## 请求

`cross::check_foreign` 遍历请求，并以 `Unsupported` 拒绝目标无法使用的内容：

- **来自其他方言的 extras 字段**，除非该字段在[允许列表](extras-allowlist.md)中，或者凭其取值可以跨格式（`cross::is_droppable_value`）。含有字段却没有来源的 extras 则以 `InvalidRequest` 拒绝，因为没有谁能把它们写回。
- **来自其他方言的未知块、工具和工具选择。** 没有 `type` 但有 `role` 的未知块保存的是一整条消息，例如 OpenAI Chat 的 `function` 消息，它被称为 `` `<role>` message ``，提示为“Remove the message, or send the request to a model served through its original format.”。
- **目标服务商无法解析的服务商文件 ID**（[文件](#文件)）。
- **来自其他方言的引用**：没有哪个目标说明过早先轮次中的引用会起什么作用，而且各格式的引用之间无法映射。

以下内容不会被拒绝，因为编码器会把它们丢弃：目标无法回放的推理块（[推理](#推理)），以及目标会自行缓存时的缓存提示（[缓存提示](#缓存提示)）。

`check()` 也会拒绝目标根本无法表达的内容。Anthropic 需要 `max_tokens`，因此对它而言，没有 `sampling.max_output_tokens` 的生成请求属于 `InvalidRequest`，错误中注明应用自己表示该上限的字段，而不会得到一个编造的值。

拒绝时注明的是应用发送的字段，而不是目标的字段：写在 `param` 中，输出上限的拒绝还写在消息中。`cross::app_field` 读取请求的来源，对 OpenAI Chat 还读取请求带的是 `max_completion_tokens` 还是 `max_tokens`（两者都发送时为 `max_completion_tokens`）。对 OpenAI Responses 而言低于 16 的输出上限，来自 Chat 应用时为 `max_completion_tokens` 或 `max_tokens`，来自 Anthropic 应用时为 `max_tokens`；对 Anthropic 而言缺少的输出上限，来自 Chat 时为 `max_completion_tokens`，来自 Responses 时为 `max_output_tokens`。停止序列来自 Anthropic 时为 `stop_sequences`，来自 Chat 时为 `stop`；输出格式为 `response_format`、`text.format` 或 `output_config.format`；推理强度为 `reasoning_effort`、`reasoning.effort` 或 `output_config.effort`，其他推理设置为 `reasoning_effort`、`reasoning` 或 `thinking`。在代码中构建、没有来源的请求使用目标的字段名。内容按其在 KPP 中的位置命名（`messages[i].blocks[j]`、`system[i]`、`tools[i]`）。

OpenAI Chat 已弃用的函数调用在 Chat 内部可以往返，但不会转换到其他任何方言。旧式调用没有调用 ID，因此不是 KPP 工具调用：

- 顶层的 `functions` 和 `function_call` 是请求的 extras，被拒绝时的消息例如：`` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``；
- 助手消息的 `function_call` 是该消息的 extra，以 `messages[i].function_call` 被拒绝；
- `function` 消息成为一条包含一个未知块的用户消息，以 `` `function` message `` 被拒绝。

## 回复

回复从一个服务商流向一个应用，拒绝一个已经完成、已经付费的回复，比少显示其中一部分更糟。因此，其他方言的数据按以下优先顺序处理（`cross::reply_policy`、`cross::reasoning_reply_policy`）：

1. 应用的方言能够表示时，**映射**过去：其他方言的推理成为 OpenAI Responses 的推理 item 或 Anthropic 的 `thinking` 块，原始块被包装在一个[推理数据块](#推理数据块)中，由应用原样发回。
2. 无法表示时，**省略**，在流中以[保活消息](#保活消息)帧代替，使连接从不陷入静默：面向 OpenAI Chat 应用的推理（网关在两轮之间[保存它](#由网关保存的推理)）、来自其他方言的未知块和流 extras、外来的引用和 extras。

网关会记录完整的 KPP 回复，因此 Kaeso 的记录中不会丢失任何内容。未知块、引用和 extras 只有在服务商发送了请求没有要求的内容时才会到达另一方言，因为请求一侧会拒绝外来的服务器工具、引用和类似功能。

应用的格式中没有的停止原因，会映射为该格式中最接近的一个；各方言的映射页面都列出了这些映射。

## Extras

从方言 A 捕获的 Extras 只写回方言 A。其他方言的 `check()` 会以 `Unsupported` 拒绝它们并注明字段，除非 [Extras 允许列表](extras-allowlist.md)允许丢弃该字段：列表只收录已知不会改变模型输出的字段，例如 beta 请求头和分辨率提示。

## 用量

`Usage::input_tokens` 不含缓存读取和缓存写入，它们是单独的字段。这与 models.dev 的价格列一致，因此费用就是每项计数乘以其价格之和。服务商的原始用量对象总会保留。

## 系统消息

对话开始之后的系统消息保存为 `Role::System` 消息。

- **发往 Anthropic，来自 Anthropic Messages：** 原样发送，由 API 判断它们的位置是否合适、哪些模型支持它们。Claude Code 在每个请求中都会发送这类消息。
- **发往 Anthropic，来自其他方言：** `Unsupported`。Anthropic 只允许部分模型使用它们，并且只能放在用户轮次之后，而 OpenAI 的 system 和 developer 消息可以出现在任何位置。
- **发往 OpenAI：** 成为位置靠后的 `system` 或 `developer` 消息。

## 推理

Anthropic 对其思考块签名，OpenAI 对其推理 item 加密。不透明的推理（签名、加密内容、item ID）只会回放给签发它的方言和服务商。

- **在请求中**，发往另一方言的可读推理文本和摘要作为提示字段被丢弃；旁边的[推理数据块](#推理数据块)会在对话回到其服务商时恢复原始块。
- **没有签名时**，从 Anthropic Messages 读到的 Anthropic `thinking` 块会按收到时的样子发回 Anthropic，请求和回复都是如此：缺失的 `signature` 保持缺失，`signature: ""` 保持为 `""`。不是从 Anthropic Messages 读到的无签名 Anthropic 推理（例如由代码构建的块）没有可回放的内容，会被省略。
- **在回复中**，其他方言的推理会以应用自己的推理形态展示（如果应用有这种形态），原始块放在推理数据块中，因此它会在下一轮重新回到其来源。OpenAI Chat 应用没有这种形态，因此由网关在两轮之间保存它们的推理。

推理设置在推理强度和 Token 预算之间转换：

| 来源 | 目标 | 结果 |
| --- | --- | --- |
| 推理强度 `none` | Anthropic | 不思考，即 Anthropic 的默认行为 |
| 推理强度 `low` 到 `max` | 支持自适应思考的 Anthropic 模型 | 采用该推理强度的自适应思考 |
| 推理强度 `low` 到 `max` | 不支持自适应思考的 Anthropic 模型 | `enabled` 思考，预算取自下表 |
| 只有预算 | Anthropic | 采用该预算的 `enabled` 思考 |
| Anthropic 预算 | OpenAI | 取自下表的推理强度 |
| Anthropic `thinking: {type: "disabled"}` | OpenAI | 推理强度 `none` |

两种 OpenAI 方言都把 `none` 记为它们共用的 `ReasoningEffort` 枚举中的最低值，因此禁用的 Anthropic 思考总有对应的值。推理强度 `minimal` 在表中没有对应的行，对 Anthropic 仍为 `Unsupported`。

### 推理强度与预算

转换逻辑位于 `kaeso_codecs::cross::reasoning_map`。

| Anthropic `budget_tokens` | OpenAI 推理强度 |
| --- | --- |
| 低于 4,096 | `low` |
| 4,096 到 16,383 | `medium` |
| 16,384 及以上 | `high` |

同一请求中显式的推理强度优先于预算。

| OpenAI 推理强度 | Anthropic `budget_tokens` |
| --- | --- |
| `low` | 2,048 |
| `medium` | 8,192 |
| `high`、`xhigh`、`max` | 24,576 |

此表只适用于目标模型不支持自适应思考的情况（Claude 4.5 及更早版本），网关根据模型目录的 `adaptive_thinking` 字段，用 `EgressCtx::adaptive_thinking = Some(false)` 表明这一点；Claude 4.7 及更新版本拒绝 `enabled` 思考，改为得到采用该推理强度的自适应思考。

随后预算会按 Anthropic 的规则调整：至少为 1,024，且小于 `max_tokens`，因此当 `max_tokens − 1` 更小时，预算取 `max_tokens − 1`。当 `max_tokens` 不超过 1,024 时，不存在有效的预算，于是 `max_tokens` 增加 1,024，预算取 1,024：应用自己的 `max_tokens` 仍全部留给回答。`reasoning_map::budget_within` 负责调整和增加，`reasoning_map::fit_budget` 只负责调整。

依据的来源是 OpenAPI 规范中 OpenAI 的 `ReasoningEffort` 枚举（`none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`；并非每个模型都支持每个值），以及 Anthropic 的[扩展思考](https://platform.claude.com/docs/en/build-with-claude/extended-thinking)页面：`budget_tokens` 至少为 1,024 且小于 `max_tokens`，而 Claude Sonnet 4.5、Opus 4.5、Haiku 4.5 以及更早的 Claude 4 模型不支持自适应思考。

### 可读推理与摘要

`reasoning.summary` 和 `reasoning.include_encrypted` 在跨方言时属于提示字段：它们只控制返回哪些推理，因此在目标没有对应项时会被丢弃。Codex 在每个请求中都会发送这两项。

OpenAI 只有在请求设置了 `reasoning.summary` 时才返回可读推理，而 Claude Opus 4.7 及更新版本返回的思考块文本为空，除非请求设置了 `thinking.display: "summarized"`。因此，除 Token 计数请求外：

- 来自其他方言、带有推理摘要（`auto`、`concise`、`detailed`）并开启了思考的请求，发往 Anthropic 时带上 `display: "summarized"`；
- 开启了思考（`enabled`、`adaptive`，或 `none` 以外的推理强度）且 `display` 不是 `omitted` 的 Anthropic Messages 请求，发往 OpenAI Responses 时带上 `summary: "auto"`。

网关还会告诉编码器目标模型是否推理（`EgressCtx::reasoning`，来自模型目录的 `reasoning`）。为 `Some(true)` 时：

- 没有推理强度、模式或预算的摘要（OpenAI 的 `reasoning: {summary: "auto"}`）会以模型的默认深度开启 Anthropic 思考：不带推理强度的自适应思考，或者 `medium` 对应的预算，即 OpenAI 的默认推理强度；
- 没有推理强度的 Anthropic `adaptive` 思考对 OpenAI Responses 而言算作推理：它会请求 `include: ["reasoning.encrypted_content"]`，并像上面那样请求摘要。

没有推理设置的请求、推理强度 `none`，以及不推理或未知的模型，都不会被开启推理。OpenAI Chat 应用的格式不显示推理，因此不会请求摘要。

### 思考与采样

当 Kaeso 为来自其他方言的请求开启思考时，Anthropic 禁止与思考同时使用的设置会被调整而不是被拒绝，因为采样设置和强制工具在 OpenAI 应用中很常见：

- 省略 `temperature`、`top_p` 和 `top_k`。开启思考时，Anthropic 拒绝 `temperature` 和 `top_k`，并把 `top_p` 限制在 0.95 到 1 之间，而 Claude Opus 4.7 及更新版本拒绝任何非默认值。
- `enabled` 思考不会与强制工具（`tool_choice` 为 `required` 或指定名称的工具）一起发送，因为 Anthropic 会拒绝这种组合。在 Anthropic 接受强制工具的情况下，即自适应思考，回复以工具调用开头，本来就不含思考，因此不会损失什么。带自适应思考的强制工具原样发送。

Anthropic 应用自己的设置按收到时的样子发送。

### 没有思考的工具轮次

如果请求使用 `enabled` 思考，而其最后一个助手轮次进行了工具调用，前面却没有可回放的思考块（签名思考块或 redacted 思考块），Anthropic 会拒绝该请求（“a final assistant message must start with a thinking block”）。这样的请求在该轮次中不带 `thinking` 发往 Anthropic。自适应思考保持不变，因为自适应模型不需要开头的思考块。

由于推理会随对话传递，这种情况很少见。但在以下情况下仍会发生：该轮次确实来自另一个服务商（在工具循环中途切换了模型）；应用丢弃了推理 item、思考块或推理数据块；或者网关不再持有某个 Chat 应用的推理，因为它已过期，或该轮次由另一个密钥发送。

## 推理的延续

如果不加处理，应用在另一服务商的模型上运行时，每个工具循环轮次都会丢失该模型的推理：预算思考会在每个后续轮次被关闭，自适应思考则会丢失之前推理的内容。`kaeso_codecs::continuity` 让推理随对话传递：对 Responses 和 Anthropic 应用，借助推理数据块；对 Chat 应用，由网关保存。

### 推理数据块

对于 OpenAI Responses 和 Anthropic Messages 应用，KPP 会把另一方言的整个推理块（可读文本、摘要、签名、加密内容、redacted 数据、item ID）包装成一个推理数据块：`continuity::REASONING_BLOB_PREFIX`（`kpp:reasoning:v1:`），后接该块去掉形式标记后的 KPP JSON，以无填充的 base64url 编码。

| 应用 | 得到 | 推理数据块所在位置 |
| --- | --- | --- |
| OpenAI Responses | 一个 ID 为 `rs_kaeso_…` 的推理 item，思考内容作为一段摘要文本，流中以 `response.reasoning_summary_*` 事件发送 | `encrypted_content` |
| Anthropic Messages | 一个 `thinking` 块，其中摘要和推理文本作为思考内容，段落之间以空行分隔 | `signature`，在块结束时以一个 `signature_delta` 流式发送 |

两个请求解码器都能在任何位置识别推理数据块（`continuity::unwrap_reasoning`），并恢复原始块及其来源，因此编码器会像对待其他推理一样，把它回放给它自己的方言和服务商。

- **总是发送。** 无论应用是否请求了 `include: ["reasoning.encrypted_content"]`，也无论其 `store` 设置如何，推理数据块都会发送：Kaeso 不存储任何内容，而 SDK 的 agent 循环（openai-agents 的 `to_input_list()`、Codex、Claude Code、Anthropic tool runner）会把输出 item 和内容块原样发回。
- **以推理数据块为准。** 推理数据块中再次包含了可读文本，因为它必须精确恢复原始块（Anthropic 会根据签名校验思考文本），而应用可能会编辑或丢弃可读的副本。应用把两者都发回时，以推理数据块为准。
- **绝不会与服务商数据混淆。** 前缀包含 `:`，它既不在 Anthropic 签名所用的 base64 字母表中，也不在 OpenAI 加密内容所用的 base64url 字母表中。带有该前缀但无法解码的值会作为服务商自己的不透明数据保留，随后由服务商拒绝，因此不会有任何内容被静默丢弃。
- **Kaeso 不对它加密或签名**。其中只有应用已经看到的内容，以及只有签发它的服务商才能校验的数据。

来自其他方言的请求，如果设置了 `none` 以外的推理强度或思考预算，或者为网关已知会推理的模型开启了思考（[可读推理与摘要](#可读推理与摘要)），会向 OpenAI Responses 请求 `include: ["reasoning.encrypted_content"]`。否则 Kaeso 无法判断模型是否推理，而 OpenAI 会对不推理的模型拒绝加密推理。由于这些请求带有 `store: false`（[Responses 的存储与 ID](#responses-的存储与-id)），没有加密内容的推理不会发给 OpenAI，因为 OpenAI 无法解析它的 item ID。

### 由网关保存的推理

Chat Completions 没有推理字段，KPP 也不会凭空创造一个。回复中省略推理，由网关保存它，并以推理之后的工具调用作为键。工具调用 ID 是唯一的，由服务商签发，每个 Chat 客户端都会将其发回，因此可以在不改变应用格式的情况下作为推理的键。

1. 回复之后，`continuity::reasoning_for_tool_calls`（完整回复）或 `continuity::ReasoningCollector`（流）为每个前面有可回放推理的工具调用返回一个 `CarriedReasoning`：Anthropic 的签名思考或 redacted 思考，或 OpenAI 的加密推理。最后一个工具调用之后的推理不会保存。
2. 网关把这些条目存入 Valkey，用主密钥加密，只有发出该请求的 API 密钥才能读取，保存时长为 `KAESO_CONTINUITY_REASONING_TTL`（默认一小时）。
3. 在应用的下一个请求中，解码之后、`check` 和编码之前，`continuity::tool_calls_missing_reasoning` 列出需要查找的工具调用 ID，`continuity::restore_reasoning` 把找到的内容放回助手轮次：轮次中第一个工具调用的推理放在轮次开头，后续调用的推理紧跟在前一个调用之后。已经含有推理的轮次保持不变。

编码器只把恢复的块回放给它们自己的服务商。

## 保活消息

服务商可能持续流式输出好几分钟，却不产生任何应用格式能够显示的内容：面向 OpenAI Chat 应用的 Claude 思考、Anthropic 的 `ping` 事件、另一方言的未知块，或者推理期间什么都不发送的 OpenAI 推理模型。带空闲超时的代理（nginx 和 AWS ALB 为 60 秒，Cloudflare 为 100 秒）会切断连接，而首字节时间会等于整个思考时间。因此实时流从不会因为格式转换而陷入静默：每个流编码器都有一个在其方言中有效、并被官方 SDK 忽略的保活帧。

| 方言 | 保活帧 | SDK 为何忽略它 |
| --- | --- | --- |
| OpenAI Chat Completions 和 Responses | SSE 注释 `: keepalive` 加一个空行 | 在 SSE 规范中，以冒号开头的行是注释，OpenAI SDK 的解析器会跳过它们 |
| Anthropic Messages | `event: ping` 加 `data: {"type": "ping"}`，与 Anthropic 发送的完全一致 | Anthropic SDK 会跳过 ping 事件 |

保活帧以两种方式到达线路：

- **带内。** 当来自另一方言的事件没有产生任何字节时，编码器把保活帧加入 `StreamEncoder::encode` 的返回值。一个不依赖时钟的节拍器对此加以限制（`kaeso_codecs::keepalive`）：另一方言的额外事件（例如 `ping`，其节奏已由服务商控制）总能换来一帧；隐藏文本（应用看不到的思考、缓冲中的工具参数）每 `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` 字节，即 1,024 字节，换来一帧；编码器写出任何内容都会重置计数。应用自身方言的事件从不换来保活帧，因此同一方言内的回放保持精确。
- **由网关驱动。** `StreamEncoder::keepalive` 按需返回保活帧，用于编码器无法察觉的上游静默。网关在 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（默认 15 秒）内没有向应用写出任何内容时，就会写出它。

OpenAI Chat 编码器还会在响应一开始就发送它的角色 chunk，而不是等到第一段文本，这样在模型思考期间应用也能收到流的消息。

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

无论应用使用哪种格式，回复都保留服务商的 ID 不变：Claude 的回复以其 Anthropic ID（`msg_…`）到达 OpenAI Chat Completions 或 Responses 应用，OpenAI 的回复以 OpenAI 的 ID（`chatcmpl-…` 或 `resp_…`）到达 Anthropic Messages 应用。流中携带同一个 ID。编解码器从不按应用自身格式的样式编造 ID，因为服务商的 ID 才是在服务商处标识该回复的 ID；应用应把回复 ID 当作不透明的字符串。例外是其他格式没有的 Responses 输出项（[Responses 的存储与 ID](#responses-的存储与-id)）。

## Responses 的存储与 ID

- **`store: false`。** 来自其他方言、编码给 OpenAI Responses 的请求带有 `"store": false`：Kaeso 是无状态的，没有这一项，OpenAI 会存储每一段经过转换的对话。从 Responses 解码的请求保留应用自己的写法，因此缺少 `store` 仍意味着 OpenAI 会存储该响应。
- **item ID。** Responses 的输出 item 需要 ID，而来自其他方言的回复没有 ID。Kaeso 为它们分配以 `msg_kaeso_`、`fc_kaeso_` 和 `rs_kaeso_` 开头的 ID；应用把它们发回时，请求解码器会移除这些 ID，因此 OpenAI 永远不会收到不是它签发的 ID。

## 缓存提示

缓存提示属于提示字段。OpenAI 会自行缓存，因此来自其他方言的缓存提示发往 OpenAI 时会被丢弃。OpenAI 自己的 `prompt_cache_breakpoint` 解码为 KPP 的 `CacheHint`，并且只写回它的来源方言。

## 工具错误

对于没有错误标志的目标，即 OpenAI Chat 和 Responses，`ToolResult.is_error` 是提示字段：它会被丢弃，由结果的内容承载错误文本。如果拒绝它，Claude Code 这类使用 Anthropic 格式的客户端的每一次失败的工具调用，都将无法路由到 OpenAI。

## 文件

服务商文件 ID（`file-…`）只能在该服务商的文件存储中使用，而两种 OpenAI 生成方言共用同一个文件存储，因此 `cross::check_foreign` 接受目标的 `Origin`，即其方言和服务商。跨到另一服务商的文件 ID 为 `Unsupported`。

## Anthropic 版本

Anthropic 编解码器固定使用一个 API 版本 `2023-06-01`，导出为 `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`。它是 Anthropic 的[版本说明页面](https://platform.claude.com/docs/en/api/versioning)列出的唯一版本，是官方 SDK 发送的版本，也是每个 Anthropic fixture 所假定的版本。

- **出站。** Anthropic 编解码器编码的每个请求都带有 `anthropic-version: 2023-06-01`，除非应用自己的 Anthropic 请求转发了一个指向更新版本的 `anthropic-version`，即格式正确、排序在固定版本之后的 `YYYY-MM-DD`。格式错误或更旧的值会被忽略。来自其他方言的请求从不携带该请求头，因此总是得到固定版本。
- **入站。** 等于固定版本的转发 `anthropic-version` 不会存入请求的 extras：编解码器无论如何都会发送它，而保存它会把每个 Anthropic SDK 请求都绑定到其方言。其他任何值都保存为 extra `header:anthropic-version`，并在出站时按上述方式比较。
- **为什么较新的版本优先。** Anthropic 对一个版本只做增量变更，增加新的可选输入和新的输出值，而请求更新版本的应用依赖于该版本的行为。解码器能容忍未知的字段、块和事件，因此更新的版本会退化为 extras 和未知项，而不会导致失败。

要修改固定版本，需要修改该常量，用 `cargo xtask record` 重新录制 fixture（[录制](fixtures.md#录制真实响应)），并更新本节。
