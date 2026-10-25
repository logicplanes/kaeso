---
title: Crossing formats
description: The rules the codecs follow when a request or reply moves from one API format to another, and the reasons for them.
order: 2
---

An app may speak one API format while the provider speaks another. These are the rules the codecs follow then, with the reason for each. A request that stays in its own format is reproduced exactly, and none of these rules apply.

| Data from another format | In a request | In a reply |
| --- | --- | --- |
| Extras fields | `Unsupported`, unless [allowlisted](#extras) | Left out |
| Unknown blocks, tools and tool choices | `Unsupported`, except [tool namespaces](#tool-namespaces), [hosted web search](#hosted-web-search) and [Codex tool history](#codex-tool-history) | Unknown blocks left out, except web searches |
| Citations | `Unsupported` | Left out |
| Provider file ids | `Unsupported` unless the target provider issued them | — |
| Readable reasoning | Dropped; the [reasoning blob](reasoning.md#reasoning-blobs) restores it at its origin | Shown in the app's reasoning shape where it has one |
| Opaque reasoning | Replayed only to the dialect and provider that issued it | Wrapped in a reasoning blob |
| Cache hints | Dropped where the target caches by itself | — |
| Sampling settings the target lacks | `Unsupported` | — |
| Stop reasons the app's format lacks | — | The closest one it has |

## Requests

`cross::check_foreign` walks a request and refuses with `Unsupported` what the target can't use:

- **Extras fields from another dialect**, unless the field is on the [allowlist](#extras) or crosses by its value (`cross::is_droppable_value`). Extras that hold fields but no origin are refused with `InvalidRequest` instead, because nobody could write them back.
- **Unknown blocks, tools and tool choices from another dialect.** An unknown block without a `type` but with a `role` holds a whole message, such as an OpenAI Chat `function` message. The refusal names it `` `<role>` message `` and adds the hint "Remove the message, or send the request to a model served through its original format."
- **Provider file ids** the target provider can't resolve ([files](#files)).
- **Citations from another dialect:** no target documents what citations in earlier turns do, and the formats don't map.

Not refused, because the encoders drop them instead: reasoning blocks the target can't replay ([reasoning](reasoning.md)), and cache hints where the target caches by itself ([cache hints](#cache-hints)).

`check()` also refuses what a target can't express at all. Anthropic needs `max_tokens`, so a generation request without `sampling.max_output_tokens` is `InvalidRequest` for it, naming the app's own field for the limit, rather than getting an invented value.

A refusal names the field the app sent, never the target's. It names it in `param` and, for the output limit, in the message too. `cross::app_field` reads the request's origin and, for OpenAI Chat, which of `max_completion_tokens` and `max_tokens` it carried; it picks `max_completion_tokens` when the app sent both. The names follow the app's format:

| Refused setting | From Anthropic | From Chat | From Responses |
| --- | --- | --- | --- |
| An output limit below 16, for OpenAI Responses | `max_tokens` | `max_completion_tokens` or `max_tokens` | |
| A missing output limit, for Anthropic | | `max_completion_tokens` | `max_output_tokens` |
| Stop sequences | `stop_sequences` | `stop` | |
| The output format | `output_config.format` | `response_format` | `text.format` |
| The reasoning effort | `output_config.effort` | `reasoning_effort` | `reasoning.effort` |
| Other reasoning settings | `thinking` | `reasoning_effort` | `reasoning` |

A request built in code, without an origin, gets the target's names. Content is named by its place in KPP (`messages[i].blocks[j]`, `system[i]`, `tools[i]`).

OpenAI Chat's deprecated function calling round-trips within Chat and crosses to no other dialect. Legacy calls have no call ids, so they are no KPP tool calls:

- the top-level `functions` and `function_call` are extras of the request. A refusal reads, for example, `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``;
- an assistant message's `function_call` is an extra of the message, refused as `messages[i].function_call`;
- a `function` message becomes a user message holding one unknown block, refused as `` `function` message ``.

## Replies

Replies flow from one provider to one app, and refusing a finished, paid-for reply would be worse than showing less of it. Data of another dialect is therefore, in this order of preference (`cross::reply_policy`, `cross::reasoning_reply_policy`):

1. **Mapped** where the app's dialect can represent it. Reasoning of another dialect becomes an OpenAI Responses reasoning item or an Anthropic `thinking` block. The original block is wrapped in a [reasoning blob](reasoning.md#reasoning-blobs) that the app sends back unchanged.
2. **Left out** where it can't. In streams, [keepalive](#keepalives) frames replace it, so the connection never goes silent. This covers reasoning for OpenAI Chat apps (the gateway [keeps it](reasoning.md#reasoning-kept-by-the-gateway) between turns), unknown blocks and stream extras from another dialect, foreign citations and extras.

The gateway logs the KPP reply in full, so nothing is lost from Kaeso's records. The request side refuses foreign server tools, citations and similar features. So unknown blocks, citations and extras reach another dialect only when the provider sends something the request didn't ask for.

Stop reasons the app's format doesn't have are mapped to the closest one it has; each dialect's mapping page lists them.

## Extras

Extras captured from dialect A are written back only to dialect A. Another dialect's `check()` refuses them with `Unsupported`, naming the field. A field on the allowlist below is dropped instead. The allowlist holds only fields known not to change what the model writes, such as beta headers and resolution hints.

An entry is allowed only when the field is known to have no effect on the model's output. Every entry needs a one-line reason and the fixture that shows a real client sending it.

Entries are scoped (`cross::EXTRAS_ALLOWLIST`): a key is only allowlisted where it sits. The scopes are `request`, `message`, `tool`, `output`, `reasoning`, and the block kinds `text`, `image`, `document`, `tool_call`, `tool_result` and `refusal`. Headers are request-scoped keys `header:<name>`.

Fixture paths in these tables are relative to `crates/kaeso-codecs/fixtures/`.

### Allowlist

| Key | Scope and origin | Reason | Fixture / client |
| --- | --- | --- | --- |
| `header:anthropic-beta` | request, from `anthropic_messages` | Beta headers only switch on provider features. A feature a request actually uses appears as a body field or block, which is checked on its own. | Claude Code sends `claude-code-20250219,interleaved-thinking-…` and further betas on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`, `anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `header:anthropic-version` | request, from `anthropic_messages` | API versioning of the Anthropic wire format; meaningless elsewhere. A value equal to the pinned version is not stored at all ([the Anthropic version](#the-anthropic-version)). | Anthropic SDKs |
| `header:openai-beta` | request, from `openai_chat`, `openai_responses`, `openai_embeddings` | As `anthropic-beta`. | Codex sends `OpenAI-Beta: responses=experimental` |
| `cache_control` | request, from `anthropic_messages` | Top-level automatic prompt caching. Cache hints are hints ([cache hints](#cache-hints)). | `anthropic_messages/requests/system-string-multi-turn.json`, `explicit-nulls.json` |
| `display` (`thinking.display`) | reasoning, from `anthropic_messages` | Only controls whether thinking text is returned (`summarized` or `omitted`). OpenAI Responses gets `reasoning.summary: "auto"` instead unless it is `omitted` ([readable reasoning and summaries](reasoning.md#readable-reasoning-and-summaries)). | `anthropic_messages/requests/thinking-display-omitted.json`, `thinking-adaptive-effort.json` |
| `image_url.detail` | image, from `openai_chat` | Image resolution hint (`low`, `high`, `auto`): it changes image token cost, not what is asked; targets without it choose their own resolution. | `openai_chat/requests/image-base64-detail.json` |
| `file.filename` | document, from `openai_chat` | The file name OpenAI requires next to inline `file_data`. Metadata, no instruction. | `openai_chat/requests/file-pdf-data.json` |
| `prompt_cache_key` | request, from `openai_chat` | Routes requests to a prompt cache. A cache hint ([cache hints](#cache-hints)). | `openai_chat/requests/all-params-sdk.json` |
| `prompt_cache_key` | request, from `openai_responses` | As above. | `openai_responses/requests/sampling-and-ids.json` |
| `client_metadata` | request, from `openai_responses` | Request metadata: turn and thread ids, W3C trace context and `x-codex-turn-metadata`. It never reaches the model, and a logged request body keeps it. | Codex sends it on every turn; `openai_responses/requests/codex-namespace-tools.json` |
| `detail` (on `input_image`; `auto` is already a form marker) | image, from `openai_responses` | Image resolution hint, as Chat's `image_url.detail`. `input_file.detail` (PDF rendering) is **not** allowlisted. | `openai_responses/requests/image-base64-file-id.json` |
| `phase` (on assistant messages: `commentary`, `final_answer`) | message, from `openai_responses` | Marks an assistant message as a preamble or the final answer, so OpenAI can tell them apart when the turn is replayed. The text is the same either way, and other formats have no such mark. | Codex sends GPT-6's messages back with it; a thread that switches to another model carries them; `openai_responses/requests/codex-model-switch.json` |

### Conditional entries

These fields cross only with the values below (`cross::is_droppable_value`, and the per-message rule in `cross::check_foreign`); any other value stays `Unsupported`.

| Key | Scope and origin | Condition and reason | Fixture / client |
| --- | --- | --- | --- |
| `context_management` | request, from `anthropic_messages` | Holds only `edits`, with at least one edit, and every edit is `clear_thinking_*`. Thinking-block clearing only decides which earlier Anthropic `thinking` blocks the model sees, and those never reach another dialect ([reasoning](reasoning.md)). | Claude Code 2.1.296 sends `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}` on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `text.verbosity` | request, from `openai_responses` | `low` or `high` (`medium` is a form marker). An output length hint ([field classes](index.md#field-classes)): a Chat Completions target whose profile takes `verbosity` gets it there ([provider profiles](openai-chat-providers.md#provider-profiles)); Anthropic and other Chat Completions targets have no equivalent, so it is left out and named in `kaeso-left-out`. | Codex sends `low` for GPT models and keeps it when a thread switches models; `openai_responses/requests/multi-turn-roles.json` |
| `output_config` | message (`role: "system"`), from `anthropic_messages` | `{}`, or only `effort` equal to the request's `output_config.effort`. Repeating the request's effort changes nothing. | Claude Code 2.1.296 repeats the request's effort on its mid-conversation system message; same fixture |

## Tool namespaces

An OpenAI Responses `namespace` tool groups function tools under a name. Codex groups its own tools and each MCP server's tools this way, and sends a call to one of them as a `function_call` with `namespace`. OpenAI gets namespaces as they are. Every other target gets plain function tools: Anthropic, Chat Completions, and SpaceX's Responses, which refuses namespaces with a 422.

- **Tools.** Each tool of a namespace becomes a function tool named `<namespace>__<name>`, such as `mcp__docs__search`. The namespace's description leads the tool's own description, separated by a blank line.
- **Calls in the conversation.** A `function_call` with `namespace` gets the same name, and its `namespace` is removed.
- **Replies.** A Responses reply or stream maps a call named like a flattened tool back to `name` and `namespace`. The names come from the app's request (`EncodeCtx::request`), so a call the app didn't namespace keeps its name.
- **Name limits.** A flattened name that breaks the target's tool-name rules is then rewritten like any other tool name and mapped back.
- **Refused.** A namespace with keys other than `type`, `name`, `description` and `tools`, a tool in it that is not a function, a field on such a tool that only Responses knows, such as `defer_loading`, and a flattened name that another tool already has.

`ResponsesProfile::tool_namespaces` says whether a Responses target takes namespaces; only `ResponsesProfile::OPENAI` does. `EgressCtx::with_tool_namespaces` overrides it for one request.

## Hosted web search

OpenAI Responses names its hosted search tool `web_search`, and earlier `web_search_2025_08_26`, `web_search_preview` and `web_search_preview_2025_03_11`; Kaeso treats all four alike. Going to Anthropic, the tool becomes Anthropic's `web_search_20250305` server tool, named `web_search`:

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (one of them; an empty list counts as none) | `allowed_domains`, `blocked_domains` |
| `enable_image_understanding`, `enable_image_search` (xAI's) | left out, named in `kaeso-left-out` when `true` |
| `user_location` (`approximate`, with `city`, `region`, `country`, `timezone`) | `user_location` |
| `external_web_access: true`, `search_context_size: "medium"` (the default), `search_content_types: ["text"]` | nothing; they describe what Anthropic's search does |
| `search_content_types` with `image` | left out, named in `kaeso-left-out`; Anthropic's search returns text results only |
| `external_web_access: false` | the tool is [left out](#tools-left-out) |
| another `search_context_size`, any other field | `Unsupported` |

Going to SpaceX's Responses API (`WebSearchTool::Xai`), the tool keeps the fields xAI documents for it ([web search](https://docs.x.ai/developers/tools/web-search), read 2026-10-25):

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (at most five each, not both) | as sent |
| `enable_image_understanding`, `enable_image_search` | as sent |
| `search_content_types` with `image` | `enable_image_search: true` |
| `external_web_access: true`, `search_context_size: "medium"`, `search_content_types: ["text"]` | nothing; SpaceX refuses `external_web_access` and always searches live |
| `external_web_access: false` | the tool is [left out](#tools-left-out) |
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
| `cache_control` | nothing; Responses caches by itself ([cache hints](#cache-hints)) | nothing |
| a location that isn't `approximate`, any other field | `Unsupported` | `Unsupported` |

- **Both domain lists** in one tool are `InvalidRequest`, with Claude's own message (`anthropic_messages/errors/web-search-both-domain-lists.json`).
- **Dynamic filtering.** From `web_search_20260209` on, Claude filters results by running code unless `allowed_callers` is `["direct"]`. A Responses model reads the results directly either way.
- **Sources.** The request asks for `include: ["web_search_call.action.sources"]`, because OpenAI lists the pages a search found only when asked.
- **The limit.** OpenAI's `max_tool_calls` also counts opening pages and searching within them, so it is tighter than `max_uses`. The ChatGPT plan route refuses `max_tool_calls`, so the limit is left out there ([ChatGPT plan usage](openai-responses.md#chatgpt-plan-usage)).
- **SpaceX** ignores `max_tool_calls` and drops `blocked_domains` without an error, so it gets its own names. Its `max_turns` limits rounds of tool calls, and one round may search more than once. Its other limits apply too: at most five domains in a list.
- **Forcing the search.** A `tool_choice` that forces `web_search` is `Unsupported`; `auto` and `any` let the model decide.
- **Chat Completions** has no hosted search, so the tool is left out there.

## Tools left out

These tools are left out of a request instead of refused (`cross::TOOLS_LEFT_OUT`). Each only offers the model something it may use, and an app such as Codex adds it to every request with no switch per model: refusing it would make every model without it unusable from that app. The model answers as it would without the tool. Every tool left out is named in the reply's `kaeso-left-out` header ([response headers](../../api/model-api.md#response-headers)) and in the gateway's log, and a logged request body keeps the app's tools.

| Tool | From | To | When |
| --- | --- | --- | --- |
| `web_search` | `openai_responses` | `openai_chat` | always: Chat Completions has no hosted search |
| `web_search` | `openai_responses` | `anthropic_messages` | `external_web_access: false`: Claude always searches live |
| `web_search` | `openai_responses` | `openai_responses` | `external_web_access: false`, for a provider whose profile has `WebSearchTool::Xai`: SpaceX always searches live |
| `web_search_20250305` | `anthropic_messages` | `openai_chat` | always: Chat Completions has no hosted search |
| `web_search_20260209` | `anthropic_messages` | `openai_chat` | always: Chat Completions has no hosted search |
| `web_search_20260318` | `anthropic_messages` | `openai_chat` | always: Chat Completions has no hosted search |

A `tool_choice` that asks for a tool left out, itself or in `allowed_tools`, is `Unsupported`. When every tool is left out, `tool_choice: "auto"` or `"none"` and `parallel_tool_calls` go with them and are named in `kaeso-left-out`, because providers refuse them without tools; `tool_choice: "required"` or a named tool is `Unsupported`, since no tool would be left to call.

Claude's searches reach a Responses app as `web_search_call` items. A `server_tool_use` block named `web_search` becomes an item with the id `ws_kaeso_…` and `action: {type: search, query}`. Its status is `completed` once the matching `web_search_tool_result` arrives, `failed` when that result is an error, and `in_progress` when the reply ends before it, as with `pause_turn`. Streams send `response.output_item.added`, `response.web_search_call.in_progress`, `.searching`, `.completed` and `response.output_item.done`. The results themselves and the citations of the answer are left out, like other data the app's format can't show.

`web_search_call` items an app sends back are dropped for Anthropic and Chat Completions. Anthropic replays a search only with its encrypted results, which these items don't carry, and Chat Completions has no such item. The answer that used them stays in the conversation. Items with Kaeso's `ws_kaeso_` ids are dropped for every Responses target as well, because no Responses provider issued them. Each item dropped is named in `kaeso-left-out` by its type and id, such as `web_search_call ws_67c9…`.

A Responses model's searches reach an Anthropic app as Claude's do. Each `web_search_call` item that searched and completed or failed becomes a `server_tool_use` block named `web_search`, with the id `srvtoolu_kaeso_…` and `input: {query}`, followed by a `web_search_tool_result`. A completed search lists its sources as `web_search_result` entries: `title` repeats the `url` and `encrypted_content` is empty, because Responses reports only the address, and `page_age` is `null`. A failed search becomes the error `unavailable`.

Streams send a search once its item is done: the `server_tool_use` start, one `input_json_delta` with the query, and the result. Items still in progress when the reply ends, items that open or search a page (`open_page`, `find_in_page`) and the answer's `url_citation` annotations are left out, like other data the app's format can't show. `usage.server_tool_use.web_search_requests` counts the completed searches, since a failed search isn't billed.

`server_tool_use` blocks named `web_search` and their `web_search_tool_result` blocks are dropped when an app sends them to another format, with the `web_search_result_location` citations that point into them, as `web_search_call` items are for Anthropic. The answer that used them stays. Claude gets its own blocks back unchanged; blocks with Kaeso's `srvtoolu_kaeso_` ids are dropped for Claude too, because Claude can't replay a result without its encrypted content.

## Codex tool history

A Codex thread that started on a GPT model holds items only OpenAI Responses has. When the thread continues on Anthropic or Chat Completions, they become ordinary tool calls and results, so the model keeps the history:

| Responses item | KPP |
| --- | --- |
| `custom_tool_call` (a freeform tool, such as Codex's `apply_patch`) | `Block::ToolCall` named as the tool (`<namespace>__<name>` with a `namespace`), its `input` text as the argument `input` |
| `custom_tool_call_output` | `Block::ToolResult`, read like a `function_call_output` |
| `local_shell_call` | `Block::ToolCall` named `local_shell`, its `action` object as the arguments |
| `local_shell_call_output` | `Block::ToolResult` for the call its `id` names; Codex sends a `function_call_output` instead |

OpenAI and SpaceX get the items as sent: SpaceX answers from such a history (`openai_responses/providers/xai/responses/recorded-codex-gpt-history.json`). An item that doesn't have the shape above stays an unknown block and is refused like one.

## Usage

`Usage::input_tokens` excludes cache reads and cache writes, which are fields of their own. This matches the price columns of models.dev, so a cost is the sum of each count times its price. The provider's raw usage object is always kept.

## System messages

A system message after the start of a conversation is kept as a `Role::System` message.

- **To Anthropic, from Anthropic Messages:** sent unchanged; the API judges their placement and which models support them. Claude Code sends such messages on every request.
- **To Anthropic, from other dialects:** the text joins the next user turn, after its tool results, or the last user turn when none follows. Codex sends such messages when a thread switches models (`<model_switch>`) or modes (`<collaboration_mode>`). Anthropic takes a system message only right after a user turn and right before an assistant turn, and not on every model ([mid-conversation system messages](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)), while OpenAI apps place them anywhere, usually before the turn they introduce. The fields of the message go through the same check as any message's. Leading ones, before any user or assistant message, join the system prompt; the decoders already make most of them the system prompt. Before a final assistant message, which Anthropic continues as a prefill, the text joins the last user turn instead.
- **To OpenAI:** later `system` or `developer` messages. For Chat Completions, one between an assistant message's tool calls and their results moves after the results, because Mistral refuses a system message there (`Unexpected role 'tool' after role 'system'`).

## Keepalives

A provider can stream for minutes without producing anything the app's format can show: Claude's thinking for an OpenAI Chat app, Anthropic `ping` events, another dialect's unknown blocks, or an OpenAI reasoning model that sends nothing while it reasons. Proxies with idle timeouts (nginx and AWS ALB after 60 seconds, Cloudflare after 100) would cut the connection, and the time to first byte would equal the whole thinking time. So a live stream never goes silent because of translation: every stream encoder has a keepalive frame that is valid in its dialect and ignored by the official SDKs.

| Dialect | Keepalive frame | Why the SDKs ignore it |
| --- | --- | --- |
| OpenAI Chat Completions and Responses | The SSE comment `: keepalive` and a blank line | Lines starting with a colon are comments in the SSE specification, and the OpenAI SDK parsers skip them |
| Anthropic Messages | `event: ping` with `data: {"type": "ping"}`, exactly as Anthropic sends it | The Anthropic SDKs skip ping events |

Frames reach the wire in two ways:

- **In band.** An encoder adds the frame to what `StreamEncoder::encode` returns when an event from another dialect produced no bytes. A pacer limits this without a clock. Another dialect's extra event, such as `ping`, always earns a frame, because the provider already paces it. Hidden text, such as thinking the app can't see or buffered tool arguments, earns one frame per `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` bytes, 1,024. Anything the encoder writes resets the count. Events of the app's own dialect never earn frames, so a same-dialect replay stays exact.
- **Driven by the gateway.** `StreamEncoder::keepalive` returns the frame on demand, for upstream silence the encoder can't see. The gateway writes it when it has written nothing to the app for `KAESO_STREAMS_KEEPALIVE_INTERVAL`, 15 seconds by default.

The OpenAI Chat encoder also sends its role chunk as soon as the response starts, rather than with the first text. That way the app hears from the stream while the model thinks.

## Structured output

Structured output going to Anthropic uses Anthropic's native structured outputs, never an emulation through a forced tool.

- The output format `json_object` has no Anthropic equivalent and is `Unsupported`.
- Anthropic's output format is a schema alone: a schema's `name` is dropped, while a `description` or `strict: false` is `Unsupported`.
- Schemas going to OpenAI from Anthropic get the name `response`, because OpenAI requires one.

## Sampling

Sampling settings a target can't honour are `Unsupported`, never dropped or rescaled:

| Target | Refused |
| --- | --- |
| Anthropic | `seed`, frequency and presence penalties, and a `temperature` outside 0 to 1, Anthropic's range |
| OpenAI Chat Completions | `top_k`, and more than four stop sequences |
| OpenAI Responses | `top_k`, `seed`, the penalties and stop sequences, which Responses doesn't have, and an output limit below 16 tokens, the `CreateResponse` minimum |

Numbers keep their spelling within a dialect: `temperature: 1` comes back as `1` and `1.0` as `1.0` while the value is unchanged. Across dialects the target's own spelling is used, so `temperature: 1` from Chat reaches Anthropic as `1.0`.

KPP has no `n`: a Chat request with `n` above 1 is refused when it is decoded, whatever the target. Chat's `logprobs`, `logit_bias` and `top_logprobs` are extras, which every other dialect refuses ([extras](#extras)).

## Stateful Responses features

`previous_response_id`, `conversation`, `store: true` and `background: true` need state at OpenAI, and the codec refuses them with `Unsupported`. Only the gateway, before KPP, could resolve them.

## Reply ids

A reply keeps the provider's id as it is, whatever the app's format. A Claude reply reaches an OpenAI Chat Completions or Responses app with its Anthropic id (`msg_…`). An OpenAI reply reaches an Anthropic Messages app with OpenAI's id (`chatcmpl-…` or `resp_…`). Streams carry the same id.

The codecs never make up an id in the app's own pattern. The provider's id names the reply at the provider, and apps treat reply ids as opaque strings. Responses output items are an exception, because other formats don't have them ([Responses storage and ids](#responses-storage-and-ids)). So is a Responses stream without a start event, which gets the id `resp_kaeso_<created_at>` ([OpenAI Responses](openai-responses.md)).

## Responses storage and ids

- **`store: false`.** Requests from other dialects encoded for OpenAI Responses carry `"store": false`: Kaeso is stateless, and without it OpenAI would store every translated conversation. Requests decoded from Responses keep the app's spelling, so an absent `store` still means OpenAI stores the response.
- **Item ids.** Responses output items need ids, which replies from other dialects don't have. Kaeso gives them ids starting with `msg_kaeso_`, `fc_kaeso_` and `rs_kaeso_`. When an app sends them back, the request decoder removes them, so OpenAI never receives an id it did not issue.

## Cache hints

Cache hints are hints. OpenAI caches by itself, so hints from other dialects are dropped for OpenAI. OpenAI's own `prompt_cache_breakpoint` decodes to a KPP `CacheHint` and is written back only to the dialect it came from.

## Tool errors

`ToolResult.is_error` is a hint for targets without an error flag, OpenAI Chat and Responses: it is dropped, and the result's content carries the error text. Refusing it would make every failed tool call of an Anthropic-format client such as Claude Code unroutable to OpenAI.

## Files

A provider file id (`file-…`) is usable only within that provider's file store. The two OpenAI generation dialects share one store, so `cross::check_foreign` takes the target's `Origin`: its dialect and provider. File ids crossing to another provider are `Unsupported`.

## The Anthropic version

The Anthropic codec pins one API version, `2023-06-01`, exported as `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`. It is the only version Anthropic's [versioning page](https://platform.claude.com/docs/en/api/versioning) lists, the one the official SDKs send, and the one every Anthropic fixture assumes.

- **Egress.** Every request the Anthropic codec encodes carries `anthropic-version: 2023-06-01`, unless the app's own Anthropic request forwarded an `anthropic-version` naming a newer version: a well-formed `YYYY-MM-DD` that sorts after the pinned one. A malformed or older value is ignored. A request from another dialect never carries the header, so it always gets the pinned version.
- **Ingress.** A forwarded `anthropic-version` equal to the pinned version is not stored in the request's extras. The codec sends it anyway, and keeping it would bind every Anthropic SDK request to its dialect. Any other value is kept as the extra `header:anthropic-version` and compared on egress as above.
- **Why newer wins.** Anthropic only adds to a version, with new optional inputs and new output values, and an app that asks for a newer version relies on its behaviour. The decoders tolerate unknown fields, blocks and events, so a newer version degrades to extras and unknown items rather than failing.

To change the pin, change the constant, record the fixtures again with `cargo xtask record` ([recording](fixtures.md#recording-live-responses)), and update this section.
