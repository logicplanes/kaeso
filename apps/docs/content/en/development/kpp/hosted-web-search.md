---
title: Hosted web search across formats
nav: Web search
description: How the codecs carry hosted web search tools between OpenAI Responses, SpaceX and Anthropic Messages.
order: 3
---

OpenAI Responses names its hosted search tool `web_search`, and earlier `web_search_2025_08_26`, `web_search_preview` and `web_search_preview_2025_03_11`; Kaeso treats all four alike. Going to Anthropic, the tool becomes Anthropic's `web_search_20250305` server tool, named `web_search`:

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (one of them; an empty list counts as none) | `allowed_domains`, `blocked_domains` |
| `enable_image_understanding`, `enable_image_search` (xAI's) | left out, named in `kaeso-left-out` when `true` |
| `user_location` (`approximate`, with `city`, `region`, `country`, `timezone`) | `user_location` |
| `external_web_access: true`, `search_context_size: "medium"` (the default), `search_content_types: ["text"]` | nothing; they describe what Anthropic's search does |
| `search_content_types` with `image` | left out, named in `kaeso-left-out`; Anthropic's search returns text results only |
| `external_web_access: false` | the tool is [left out](cross-format.md#tools-left-out) |
| another `search_context_size`, any other field | `Unsupported` |

Going to SpaceX's Responses API (`WebSearchTool::Xai`), the tool keeps the fields xAI documents for it ([web search](https://docs.x.ai/developers/tools/web-search), read 2026-10-25):

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (at most five each, not both) | as sent |
| `enable_image_understanding`, `enable_image_search` | as sent |
| `search_content_types` with `image` | `enable_image_search: true` |
| `external_web_access: true`, `search_context_size: "medium"`, `search_content_types: ["text"]` | nothing; SpaceX refuses `external_web_access` and always searches live |
| `external_web_access: false` | the tool is [left out](cross-format.md#tools-left-out) |
| more than five domains, both lists, `user_location`, another `search_context_size`, any other field | `Unsupported` |

`external_web_access: false` asks OpenAI to read only its cache of pages, which is what Codex sends for its default `web_search = "cached"`. Claude and SpaceX always search live. Searching live would read pages the app asked Kaeso not to fetch, so the search is left out instead. Chat Completions has no hosted search, so the tool is left out there too. OpenAI gets the tool as sent.

Anthropic's web search tool goes the other way: `web_search_20250305`, which Claude Code's `WebSearch` sends, `web_search_20260209` and `web_search_20260318`. A Responses target gets the hosted `web_search` tool, and the limit on searches becomes a field of the request ([web search tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool)):

| Anthropic | OpenAI | SpaceX |
| --- | --- | --- |
| `allowed_domains` | `filters.allowed_domains` | `filters.allowed_domains` |
| `blocked_domains` | `filters.blocked_domains` | `filters.excluded_domains` |
| `user_location` (`approximate`, with `city`, `region`, `country`, `timezone`) | `user_location` | `Unsupported` |
| `max_uses` | `max_tool_calls` | `max_turns` |
| `allowed_callers` that is `["direct"]` or the version's default | nothing; the hosted search is a direct search | nothing |
| `response_inclusion` (`web_search_20260318`) | nothing; it only shapes searches run from code execution | nothing |
| `cache_control` | nothing; Responses caches by itself ([cache hints](cross-format.md#cache-hints)) | nothing |
| a location that isn't `approximate`, any other field | `Unsupported` | `Unsupported` |

- **Both domain lists** in one tool are `InvalidRequest`, with Claude's own message (`anthropic_messages/errors/web-search-both-domain-lists.json`).
- **Dynamic filtering.** From `web_search_20260209` on, Claude filters results by running code unless `allowed_callers` is `["direct"]`. A Responses model reads the results directly either way.
- **Sources.** The request asks for `include: ["web_search_call.action.sources"]`, because OpenAI lists the pages a search found only when asked.
- **The limit.** OpenAI's `max_tool_calls` also counts opening pages and searching within them, so it is tighter than `max_uses`. The ChatGPT plan route refuses `max_tool_calls`, so the limit is left out there ([ChatGPT plan usage](openai-responses.md#chatgpt-plan-usage)).
- **SpaceX** ignores `max_tool_calls` and drops `blocked_domains` without an error, so it gets its own names. Its `max_turns` limits rounds of tool calls, and one round may search more than once. Its other limits apply too: at most five domains in a list.
- **Forcing the search.** A `tool_choice` that forces `web_search` is `Unsupported`; `auto` and `any` let the model decide.
- **Chat Completions** has no hosted search, so the tool is left out there.
