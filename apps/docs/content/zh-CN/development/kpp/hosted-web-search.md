---
title: 跨格式的托管网页搜索
nav: 网页搜索
description: 编解码器如何在 OpenAI Responses、SpaceX 和 Anthropic Messages 之间传递托管网页搜索工具。
order: 3
---

OpenAI Responses 把它的托管搜索工具命名为 `web_search`，更早的名称是 `web_search_2025_08_26`、`web_search_preview` 和 `web_search_preview_2025_03_11`；Kaeso 对这四个名称一视同仁。发往 Anthropic 时，该工具变为 Anthropic 的服务端工具 `web_search_20250305`，名称为 `web_search`：

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`、`filters.excluded_domains`（二选一；空列表视为没有） | `allowed_domains`、`blocked_domains` |
| `enable_image_understanding`、`enable_image_search`（xAI 的字段） | 省略，为 `true` 时在 `kaeso-left-out` 中注明 |
| `user_location`（`approximate`，含 `city`、`region`、`country`、`timezone`） | `user_location` |
| `external_web_access: true`、`search_context_size: "medium"`（默认值）、`search_content_types: ["text"]` | 不写入；它们描述的正是 Anthropic 搜索的行为 |
| 含 `image` 的 `search_content_types` | 省略，并在 `kaeso-left-out` 中注明；Anthropic 的搜索只返回文本结果 |
| `external_web_access: false` | [省略](cross-format.md#省略的工具)该工具 |
| 其他 `search_context_size`、任何其他字段 | `Unsupported` |

发往 SpaceX 的 Responses API（`WebSearchTool::Xai`）时，该工具保留 xAI 为它记载的字段（[网页搜索](https://docs.x.ai/developers/tools/web-search)，2026-10-25 阅读）：

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`、`filters.excluded_domains`（各最多五个，不能同时使用） | 原样发送 |
| `enable_image_understanding`、`enable_image_search` | 原样发送 |
| 含 `image` 的 `search_content_types` | `enable_image_search: true` |
| `external_web_access: true`、`search_context_size: "medium"`、`search_content_types: ["text"]` | 不写入；SpaceX 拒绝 `external_web_access`，并且总是实时搜索 |
| `external_web_access: false` | [省略](cross-format.md#省略的工具)该工具 |
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
| `cache_control` | 不写入；Responses 会自行缓存（[缓存提示](cross-format.md#缓存提示)） | 不写入 |
| 不是 `approximate` 的位置、任何其他字段 | `Unsupported` | `Unsupported` |

- **两个域名列表**同时出现在一个工具中时为 `InvalidRequest`，使用 Claude 自己的消息（`anthropic_messages/errors/web-search-both-domain-lists.json`）。
- **动态过滤。** 从 `web_search_20260209` 起，除非 `allowed_callers` 为 `["direct"]`，Claude 会通过运行代码来过滤结果。Responses 模型在两种情况下都直接读取结果。
- **来源。** 请求会带上 `include: ["web_search_call.action.sources"]`，因为 OpenAI 只有在被要求时才会列出一次搜索找到的页面。
- **上限。** OpenAI 的 `max_tool_calls` 也计入打开页面和在页面内搜索，因此比 `max_uses` 更严格。ChatGPT 套餐的路由会拒绝 `max_tool_calls`，因此在那里该上限会被省略（[ChatGPT 套餐用量](openai-responses.md#chatgpt-套餐用量)）。
- **SpaceX** 会忽略 `max_tool_calls`，并且会不报错地丢弃 `blocked_domains`，因此它收到自己的字段名。它的 `max_turns` 限制的是工具调用的轮数，一轮可能搜索不止一次。它的其他限制同样适用：每个列表最多五个域名。
- **强制搜索。** 强制使用 `web_search` 的 `tool_choice` 为 `Unsupported`；`auto` 和 `any` 让模型自行决定。
- **Chat Completions** 没有托管搜索，因此该工具在那里会被省略。

搜索如何在回复和流中到达应用，以及应用发回的搜索会怎样处理，见[省略的工具](cross-format.md#省略的工具)。
