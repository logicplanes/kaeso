---
title: Extras 允许列表
description: 请求转换格式时被丢弃而不是被拒绝的 Extras，以及每一项为何安全。
order: 3
---

从方言 A 捕获的 Extras 只会写回方言 A。带有 Extras 的请求发往其他方言时，编解码器的 `check()` 会返回 `Unsupported` 并注明该字段，除非该字段列在这里：此时它会被静默丢弃（[Extras](cross-format.md#extras)）。

只有已知对模型输出没有任何影响的字段才能加入列表。每个条目都需要一行理由，以及一个展示真实客户端发送该字段的测试样本（fixture）。

条目带有作用范围（`cross::EXTRAS_ALLOWLIST`）：一个键只在它所在的位置被允许。作用范围包括 `request`、`message`、`tool`、`output`、`reasoning`，以及块类型 `text`、`image`、`document`、`tool_call`、`tool_result` 和 `refusal`。请求头是 `request` 范围内的键 `header:<name>`。

本页中的 fixture 路径都相对于 `crates/kaeso-codecs/fixtures/`。

## 条目

| 来源方言 | 范围 | 键 | 理由 | Fixture / 客户端 |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `header:anthropic-beta` | Beta 请求头只用于开启服务商的功能。请求实际用到的功能会以请求体字段或块的形式出现，并单独检查。 | Claude Code 在每个请求中都发送 `claude-code-20250219,interleaved-thinking-…` 及其他 beta；`anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`、`anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `anthropic_messages` | request | `header:anthropic-version` | Anthropic wire 格式的 API 版本控制，在其他地方没有意义。等于固定版本的值根本不会被保存（[Anthropic 版本](cross-format.md#anthropic-版本)）。 | Anthropic SDK |
| `openai_chat`、`openai_responses`、`openai_embeddings` | request | `header:openai-beta` | 同 `anthropic-beta`。 | Codex 发送 `OpenAI-Beta: responses=experimental` |
| `anthropic_messages` | request | `cache_control` | 顶层的自动提示缓存。缓存提示属于提示字段（[缓存提示](cross-format.md#缓存提示)）。 | `anthropic_messages/requests/system-string-multi-turn.json`、`explicit-nulls.json` |
| `anthropic_messages` | reasoning | `display`（`thinking.display`） | 只控制是否返回思考文本（`summarized` 或 `omitted`）。发往 OpenAI Responses 时改为 `reasoning.summary: "auto"`，除非其值为 `omitted`（[可读推理与摘要](cross-format.md#可读推理与摘要)）。 | `anthropic_messages/requests/thinking-display-omitted.json`、`thinking-adaptive-effort.json` |
| `openai_chat` | image | `image_url.detail` | 图像分辨率提示（`low`、`high`、`auto`）：它改变的是图像的 Token 成本，而不是提问内容；没有该字段的目标会自行选择分辨率。 | `openai_chat/requests/image-base64-detail.json` |
| `openai_chat` | document | `file.filename` | OpenAI 要求随内联 `file_data` 一起提供的文件名。属于元数据，不是指令。 | `openai_chat/requests/file-pdf-data.json` |
| `openai_chat` | request | `prompt_cache_key` | 把请求路由到某个提示缓存。属于缓存提示（[缓存提示](cross-format.md#缓存提示)）。 | `openai_chat/requests/all-params-sdk.json` |
| `openai_responses` | request | `prompt_cache_key` | 同上。 | `openai_responses/requests/sampling-and-ids.json` |
| `openai_responses` | image | `detail`（位于 `input_image` 上；`auto` 已经是形式标记） | 图像分辨率提示，同 Chat 的 `image_url.detail`。`input_file.detail`（PDF 渲染）**不**在允许列表中。 | `openai_responses/requests/image-base64-file-id.json` |

## 条件条目

这些字段只有取下列值时才能跨格式（`cross::is_droppable_value`，以及 `cross::check_foreign` 中针对单条消息的规则）；其他任何值仍然返回 `Unsupported`。

| 来源方言 | 范围 | 键 | 条件与理由 | Fixture / 客户端 |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `context_management` | 只包含 `edits`，其中至少有一个 edit，且每个 edit 都是 `clear_thinking_*`。清除思考块只决定模型能看到哪些较早的 Anthropic `thinking` 块，而这些块从不会到达其他方言（[推理](cross-format.md#推理)）。 | Claude Code 2.1.296 在每个请求中都发送 `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}`；`anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `anthropic_messages` | message（`role: "system"`） | `output_config` | `{}`，或只有与请求的 `output_config.effort` 相同的 `effort`。重复请求的推理强度不会改变任何内容。 | Claude Code 2.1.296 会在对话中间的系统消息里重复请求的推理强度；同一个 fixture |
