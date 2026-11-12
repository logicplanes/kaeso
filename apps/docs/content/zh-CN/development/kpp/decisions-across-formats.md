---
title: 跨格式的决策
nav: 决策
description: 决策请求和答案如何在 OpenAI 的 Decisions API、TypeSafe 的 System One API 与 Gemini 的 Decisions API 之间转换。
order: 3
---

决策请求通过同一个 `DecideRequest` 在 OpenAI 的 Decisions API（[OpenAI Decisions](openai-decisions.md)）、TypeSafe 的 System One API（[System One](system-one.md)）和 Gemini 的 Decisions API（[Gemini Decisions](gemini-decisions.md)）之间跨格式传递。目标在发送任何内容之前拒绝其格式无法表达的内容。少数有名称的调整列在回复的 `kaeso-adapted` 请求头中，目标没有位置的提示列在 `kaeso-left-out` 中。括号中的编号是决策格式的设计决定。

## OpenAI 与 System One 之间的请求

| 内容 | OpenAI 应用到 System One 服务商 | System One 应用到 OpenAI |
| --- | --- | --- |
| 问题名称（D19） | 当每个问题都有名称、名称不重复且服务商可接受时保留；否则全部以 `q0`…`qN-1` 发送，`adapted: names` | 每个键成为问题的 `name` |
| 布尔值（D18） | 以 `"true"` 和 `"false"` 发送，`adapted: booleans`；同时含有 `true` 和 `"true"` 的问题为 `Unsupported` | — |
| 评分级别 | 级别同时有两者时写成 `"label: description"`，`adapted: levels`；否则只写标签 | 每个级别的文本成为它的 `label` |
| 结构化状态（D13） | — | 在 OpenAI 的限制内为 `input_json`；否则为带原样 JSON 的 `input_text`，`adapted: state` |
| 是非题的 criteria（D11） | — | 用一个固定模板并入说明，`adapted: criteria` |
| JSON 形式的说明和描述 | — | 原样的 JSON 作为文本，`adapted: instructions` 或 `criteria` |
| 对话（D12） | `Unsupported`（`conversation input`）：请发送一条用户消息 | — |
| 图片 | data URL 成为资产；URL 为 `Unsupported`（`image URL`）：请以 data URL 发送图片（D6） | 输入、状态列表或说明的字符串中的每个引用，在原位置成为一个 `input_image` 部分；其他位置为 `Unsupported` |
| box 问题 | 只发往能回答它们的配置文件 | `Unsupported` |
| 限制（D5） | 配置文件对问题、选项、级别和图片的限制 | 400 个问题，2 到 255 个选项，2 到 10 个级别 |
| `prompt_cache_key`、`prompt_cache_options`、断点 | 省略，`left_out: cache` | — |
| `safety_identifier` | 省略，`left_out: user` | — |
| `detail` | 省略 | — |

criteria 模板在说明后加一个空行，再加 `It is true when: <true>` 和 `It is false when: <false>`，各占一行。由部分组成的说明把这些行作为另一个文本部分。JSON 文本从不重新格式化。

## 进出 Gemini 的请求

| 内容 | OpenAI 或 System One 应用到 Gemini | Gemini 应用到 OpenAI 或 System One |
| --- | --- | --- |
| 问题名称（D19） | 当每个问题都有名称、名称不重复且符合 `[A-Za-z0-9_-]{1,64}` 时保留；否则全部以 `q0`…`qN-1` 发送，`adapted: names` | 每个键成为问题的名称 |
| 选项键 | 每个值，布尔值写成 `"true"` 和 `"false"`（`adapted: booleans`）。Google 拒绝的键为 `Unsupported`（`option key`） | 每个键成为选项的值 |
| 选项描述 | 缺少的描述为 `""` | 保留，包括 `""` |
| 评分级别 | 与 System One 相同：`"label: description"`，`adapted: levels`；没有文本的级别为 `Unsupported` | 每个级别的文本，作为 System One 的级别 |
| 结构化状态（D13） | 按原样写出的 JSON，作为输入字符串；按 Google 指南的要求，不记录任何调整 | — |
| JSON 部分、说明和描述 | 按原样写出的 JSON，作为文本；`adapted: instructions` 或 `criteria` | — |
| 是非题的 criteria | `true_description` 和 `false_description` | 并入 OpenAI 的说明（D11）；System One 的 `true` 和 `false` |
| 对话（D12） | `Unsupported`（`conversation input`） | — |
| 图片 | 字节和 data URL 写成 `data`；`https` URL 写成 `uri`，带有取自扩展名的 MIME 类型；`http`、`gs://` 和 YouTube 的 URL 为 `Unsupported` | 内嵌图片可以跨格式；Files API URI 为 `Unsupported`（`file id`） |
| 资产引用 | 在输入字符串或状态列表的字符串中，于原位置成为 `image` 块；列表项的文本以换行连接；其他位置为 `Unsupported` | — |
| 说明或描述中的图片 | `Unsupported`：Gemini 在那里只接收文本 | — |
| 音频、视频和 PDF | — | `Unsupported`，指明该部分 |
| `detail` 和 `resolution` | OpenAI 的 `low` 和 `high` 成为 `resolution`；其他值被省略 | `low` 和 `high` 成为 OpenAI 的 `detail`；其他值被省略 |
| box 问题 | `Unsupported` | — |
| 限制（D5） | 1,000 个问题，2 到 255 个选项，2 到 255 个级别 | 同上表 |
| `prompt_cache_key`、`prompt_cache_options`、`safety_identifier`、断点 | 省略，`left_out: cache` 或 `user` | — |

Google 拒绝超过 128 个字符或 512 字节、两端有空格、含控制字符或非 ASCII 空格的选项键。Gemini 把相邻的文本块直接连接、不加分隔符，因此它的块到达 System One 时是字符串组成的状态列表，到达 OpenAI 时是文本部分。

## 回复

回复按位置跨格式：应用格式的编码器从应用的请求中取得每个名称、选项和级别。

- **置信度（D8）。** 保持原格式的回复逐字节保留其 `confidence`。来自其他格式的回复根据概率重新计算：System One 应用使用 TypeSafe 的公式，OpenAI 应用也一样，因为其文档中的两个示例与这些公式一致。Gemini 应用使用 Google 的规则：取最高概率，是非题取 `max(p, 1 − p)`。对 OpenAI 和 System One 应用，是非题的答案没有置信度。
- **拒答（D9）。** OpenAI 的拒答以 `{"type": "refusal"}` 写在问题的键下，到达 System One 应用。TypeSafe 的 SDK 会跳过它们不认识的答案类型，其他答案照常有效。Gemini 的格式没有拒答这种答案，因此含拒答的回复以 `ContentFiltered` 到达 Gemini 应用：一个代码为 `safety` 的 400，消息中列出被拒答的问题。
- **安全拦截（D10）。** Gemini 以带安全代码的 400，或以带 `prompt_feedback.block_reason` 且没有答案的回复报告被拦截的输入。两者都以各应用自己格式的 `ContentFiltered` 到达。
- **标签和图例。** OpenAI 应用从其请求中得到每个级别的标签；System One 和 Gemini 应用从其请求的级别得到 `legend`。
- **模型。** Gemini 的回复不指明模型，因此解码器取请求中的模型；Gemini 应用不会收到模型。
- **用量。** KPP 统计的输入 token 不含缓存。OpenAI 的 `input_tokens` 包含缓存读取和写入，因此解码器减去它们，编码器再加回；`total_tokens` 为输入加输出。Gemini 按模态统计输入：KPP 把图片 token（含视频和 PDF 页面）和音频 token 分开记录。Gemini 应用得到 `input_tokens_by_modality`，其余部分记为文本。
