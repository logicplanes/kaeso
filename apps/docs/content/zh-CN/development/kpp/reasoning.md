---
title: 跨格式的推理
description: 编解码器如何在各 API 格式之间传递推理内容、推理设置及其签名。
order: 3
---

Anthropic 对其思考块签名，OpenAI 对其推理 item 加密。不透明的推理（签名、加密内容、item ID）只会回放给签发它的方言和服务商。

## 哪些推理会被回放

- **在请求中**，发往另一方言的可读推理文本和摘要作为提示字段被丢弃；旁边的[推理数据块](#推理数据块)会在对话回到其服务商时恢复原始块。
- **没有签名时**，从 Anthropic Messages 读到的 Anthropic `thinking` 块会按收到时的样子发回 Anthropic，请求和回复都是如此：缺失的 `signature` 保持缺失，`signature: ""` 保持为 `""`。不是从 Anthropic Messages 读到的无签名 Anthropic 推理（例如由代码构建的块）没有可回放的内容，会被省略。
- **在回复中**，其他方言的推理会以应用自己的推理形态展示（如果应用有这种形态），原始块放在推理数据块中，因此它会在下一轮重新回到其来源。OpenAI Chat 应用没有这种形态，因此由网关在两轮之间保存它们的推理。

## 推理强度与预算

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

## 可读推理与摘要

`reasoning.summary` 和 `reasoning.include_encrypted` 在跨方言时属于提示字段：它们只控制返回哪些推理，因此在目标没有对应项时会被丢弃。Codex 在每个请求中都会发送这两项。

OpenAI 只有在请求设置了 `reasoning.summary` 时才返回可读推理，而 Claude Opus 4.7 及更新版本返回的思考块文本为空，除非请求设置了 `thinking.display: "summarized"`。因此，除 Token 计数请求外：

- 来自其他方言、带有推理摘要（`auto`、`concise`、`detailed`）并开启了思考的请求，发往 Anthropic 时带上 `display: "summarized"`；
- 开启了思考（`enabled`、`adaptive`，或 `none` 以外的推理强度）且 `display` 不是 `omitted` 的 Anthropic Messages 请求，发往 OpenAI Responses 时带上 `summary: "auto"`。

网关还会告诉编码器目标模型是否推理（`EgressCtx::reasoning`，来自模型目录的 `reasoning`）。为 `Some(true)` 时：

- 没有推理强度、模式或预算的摘要（OpenAI 的 `reasoning: {summary: "auto"}`）会以模型的默认深度开启 Anthropic 思考：不带推理强度的自适应思考，或者 `medium` 对应的预算，即 OpenAI 的默认推理强度；
- 没有推理强度的 Anthropic `adaptive` 思考对 OpenAI Responses 而言算作推理：它会请求 `include: ["reasoning.encrypted_content"]`，并像上面那样请求摘要。

没有推理设置的请求、推理强度 `none`，以及不推理或未知的模型，都不会被开启推理。OpenAI Chat 应用的格式不显示推理，因此不会请求摘要。

## 思考与采样

当 Kaeso 为来自其他方言的请求开启思考时，Anthropic 禁止与思考同时使用的设置会被调整而不是被拒绝，因为采样设置和强制工具在 OpenAI 应用中很常见：

- 省略 `temperature`、`top_p` 和 `top_k`。开启思考时，Anthropic 拒绝 `temperature` 和 `top_k`，并把 `top_p` 限制在 0.95 到 1 之间，而 Claude Opus 4.7 及更新版本拒绝任何非默认值。
- `enabled` 思考不会与强制工具（`tool_choice` 为 `required` 或指定名称的工具）一起发送，因为 Anthropic 会拒绝这种组合。在 Anthropic 接受强制工具的情况下，即自适应思考，回复以工具调用开头，本来就不含思考，因此不会损失什么。带自适应思考的强制工具原样发送。

Anthropic 应用自己的设置按收到时的样子发送。

## 没有思考的工具轮次

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

来自其他方言的请求，如果设置了 `none` 以外的推理强度或思考预算，或者为网关已知会推理的模型开启了思考（[可读推理与摘要](#可读推理与摘要)），会向 OpenAI Responses 请求 `include: ["reasoning.encrypted_content"]`。否则 Kaeso 无法判断模型是否推理，而 OpenAI 会对不推理的模型拒绝加密推理。由于这些请求带有 `store: false`（[Responses 的存储与 ID](cross-format.md#responses-的存储与-id)），没有加密内容的推理不会发给 OpenAI，因为 OpenAI 无法解析它的 item ID。

### 由网关保存的推理

Chat Completions 没有推理字段，KPP 也不会凭空创造一个。回复中省略推理，由网关保存它，并以推理之后的工具调用作为键。工具调用 ID 是唯一的，由服务商签发，每个 Chat 客户端都会将其发回，因此可以在不改变应用格式的情况下作为推理的键。

1. 回复之后，`continuity::reasoning_for_tool_calls`（完整回复）或 `continuity::ReasoningCollector`（流）为每个前面有可回放推理的工具调用返回一个 `CarriedReasoning`：Anthropic 的签名思考或 redacted 思考，或 OpenAI 的加密推理。最后一个工具调用之后的推理不会保存。
2. 网关把这些条目存入 Valkey，用主密钥加密，只有发出该请求的 API 密钥才能读取，保存时长为 `KAESO_CONTINUITY_REASONING_TTL`（默认一小时）。
3. 在应用的下一个请求中，解码之后、`check` 和编码之前，`continuity::tool_calls_missing_reasoning` 列出需要查找的工具调用 ID，`continuity::restore_reasoning` 把找到的内容放回助手轮次：轮次中第一个工具调用的推理放在轮次开头，后续调用的推理紧跟在前一个调用之后。已经含有推理的轮次保持不变。

编码器只把恢复的块回放给它们自己的服务商。
