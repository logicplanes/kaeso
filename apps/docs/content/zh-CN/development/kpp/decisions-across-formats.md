---
title: 跨格式的决策
nav: 决策
description: 决策请求和答案如何在 OpenAI 的 Decisions API 与 TypeSafe 的 System One API 之间转换。
order: 3
---

决策请求通过同一个 `DecideRequest` 在 OpenAI 的 Decisions API（[OpenAI Decisions](openai-decisions.md)）和 TypeSafe 的 System One API（[System One](system-one.md)）之间跨格式传递。目标在发送任何内容之前拒绝其格式无法表达的内容。少数有名称的调整列在回复的 `kaeso-adapted` 请求头中，目标没有位置的提示列在 `kaeso-left-out` 中。括号中的编号是决策格式的设计决定。

## 请求

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

## 回复

回复按位置跨格式：应用格式的编码器从应用的请求中取得每个名称、选项和级别。

- **置信度（D8）。** 保持原格式的回复逐字节保留其 `confidence`。来自其他格式的回复根据概率重新计算：System One 应用使用 TypeSafe 的公式，OpenAI 应用也一样，因为其文档中的两个示例与这些公式一致。是非题的答案没有置信度。
- **拒答（D9）。** OpenAI 的拒答以 `{"type": "refusal"}` 写在问题的键下，到达 System One 应用。TypeSafe 的 SDK 会跳过它们不认识的答案类型，其他答案照常有效。
- **标签和图例。** OpenAI 应用从其请求中得到每个级别的标签；System One 应用从其请求的级别得到 `legend`。
- **用量。** KPP 统计的输入 token 不含缓存。OpenAI 的 `input_tokens` 包含缓存读取和写入，因此解码器减去它们，编码器再加回；`total_tokens` 为输入加输出。
