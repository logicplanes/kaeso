---
title: Crossing formats
description: The rules the codecs follow when a request or reply moves from one API format to another, and the reasons for them.
order: 2
---

An app may speak one API format while the provider speaks another. These are the rules the codecs follow then, with the reason for each. A request that stays in its own format is reproduced exactly, and none of these rules apply.

| Data from another format | In a request | In a reply |
| --- | --- | --- |
| Extras fields | `Unsupported`, unless [allowlisted](#extras) | Left out |
| Unknown blocks, tools and tool choices | `Unsupported`, except [tool namespaces](#tool-namespaces) and [hosted web search](#hosted-web-search) | Unknown blocks left out, except web searches |
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

- the top-level `functions` and `function_call` are extras of the request, refused as, for example, `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``;
- an assistant message's `function_call` is an extra of the message, refused as `messages[i].function_call`;
- a `function` message becomes a user message holding one unknown block, refused as `` `function` message ``.

## Replies

Replies flow from one provider to one app, and refusing a finished, paid-for reply would be worse than showing less of it. Data of another dialect is therefore, in this order of preference (`cross::reply_policy`, `cross::reasoning_reply_policy`):

1. **Mapped** where the app's dialect can represent it: reasoning of another dialect becomes an OpenAI Responses reasoning item or an Anthropic `thinking` block, with the original block wrapped in a [reasoning blob](reasoning.md#reasoning-blobs) that the app sends back unchanged.
2. **Left out** where it can't, and replaced by [keepalive](#keepalives) frames in streams so the connection never goes silent: reasoning for OpenAI Chat apps (the gateway [keeps it](reasoning.md#reasoning-kept-by-the-gateway) between turns), unknown blocks and stream extras from another dialect, foreign citations and extras.

The gateway logs the KPP reply in full, so nothing is lost from Kaeso's records. Unknown blocks, citations and extras reach another dialect only when the provider sends something the request didn't ask for, because the request side refuses foreign server tools, citations and similar features.

Stop reasons the app's format doesn't have are mapped to the closest one it has; each dialect's mapping page lists them.

## Extras

Extras captured from dialect A are written back only to dialect A. Another dialect's `check()` refuses them with `Unsupported`, naming the field, unless the field is on the allowlist below and is dropped instead: it holds only fields known not to change what the model writes, such as beta headers and resolution hints.

An entry is allowed only when the field is known to have no effect on the model's output. Every entry needs a one-line reason and the fixture that shows a real client sending it.

Entries are scoped (`cross::EXTRAS_ALLOWLIST`): a key is only allowlisted where it sits. The scopes are `request`, `message`, `tool`, `output`, `reasoning`, and the block kinds `text`, `image`, `document`, `tool_call`, `tool_result` and `refusal`. Headers are request-scoped keys `header:<name>`.

Fixture paths in these tables are relative to `crates/kaeso-codecs/fixtures/`.

### Allowlist

| Origin dialect | Scope | Key | Reason | Fixture / client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `header:anthropic-beta` | Beta headers only switch on provider features. A feature a request actually uses appears as a body field or block, which is checked on its own. | Claude Code sends `claude-code-20250219,interleaved-thinking-…` and further betas on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`, `anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `anthropic_messages` | request | `header:anthropic-version` | API versioning of the Anthropic wire format; meaningless elsewhere. A value equal to the pinned version is not stored at all ([the Anthropic version](#the-anthropic-version)). | Anthropic SDKs |
| `openai_chat`, `openai_responses`, `openai_embeddings` | request | `header:openai-beta` | As `anthropic-beta`. | Codex sends `OpenAI-Beta: responses=experimental` |
| `anthropic_messages` | request | `cache_control` | Top-level automatic prompt caching. Cache hints are hints ([cache hints](#cache-hints)). | `anthropic_messages/requests/system-string-multi-turn.json`, `explicit-nulls.json` |
| `anthropic_messages` | reasoning | `display` (`thinking.display`) | Only controls whether thinking text is returned (`summarized` or `omitted`). OpenAI Responses gets `reasoning.summary: "auto"` instead unless it is `omitted` ([readable reasoning and summaries](reasoning.md#readable-reasoning-and-summaries)). | `anthropic_messages/requests/thinking-display-omitted.json`, `thinking-adaptive-effort.json` |
| `openai_chat` | image | `image_url.detail` | Image resolution hint (`low`, `high`, `auto`): it changes image token cost, not what is asked; targets without it choose their own resolution. | `openai_chat/requests/image-base64-detail.json` |
| `openai_chat` | document | `file.filename` | The file name OpenAI requires next to inline `file_data`. Metadata, no instruction. | `openai_chat/requests/file-pdf-data.json` |
| `openai_chat` | request | `prompt_cache_key` | Routes requests to a prompt cache. A cache hint ([cache hints](#cache-hints)). | `openai_chat/requests/all-params-sdk.json` |
| `openai_responses` | request | `prompt_cache_key` | As above. | `openai_responses/requests/sampling-and-ids.json` |
| `openai_responses` | request | `client_metadata` | Request metadata: turn and thread ids, W3C trace context and `x-codex-turn-metadata`. It never reaches the model, and a logged request body keeps it. | Codex sends it on every turn; `openai_responses/requests/codex-namespace-tools.json` |
| `openai_responses` | image | `detail` (on `input_image`; `auto` is already a form marker) | Image resolution hint, as Chat's `image_url.detail`. `input_file.detail` (PDF rendering) is **not** allowlisted. | `openai_responses/requests/image-base64-file-id.json` |

### Conditional entries

These fields cross only with the values below (`cross::is_droppable_value`, and the per-message rule in `cross::check_foreign`); any other value stays `Unsupported`.

| Origin dialect | Scope | Key | Condition and reason | Fixture / client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `context_management` | Holds only `edits`, with at least one edit, and every edit is `clear_thinking_*`. Thinking-block clearing only decides which earlier Anthropic `thinking` blocks the model sees, and those never reach another dialect ([reasoning](reasoning.md)). | Claude Code 2.1.296 sends `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}` on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `anthropic_messages` | message (`role: "system"`) | `output_config` | `{}`, or only `effort` equal to the request's `output_config.effort`. Repeating the request's effort changes nothing. | Claude Code 2.1.296 repeats the request's effort on its mid-conversation system message; same fixture |

## Tool namespaces

An OpenAI Responses `namespace` tool groups function tools under a name. Codex groups its own tools and each MCP server's tools this way, and sends a call to one of them as a `function_call` with `namespace`. OpenAI gets namespaces as they are. Every other target gets plain function tools: Anthropic, Chat Completions, and SpaceX's Responses, which refuses namespaces with a 422.

- **Tools.** Each tool of a namespace becomes a function tool named `<namespace>__<name>`, such as `mcp__docs__search`. The namespace's description leads the tool's own description, separated by a blank line.
- **Calls in the conversation.** A `function_call` with `namespace` gets the same name, and its `namespace` is removed.
- **Replies.** A Responses reply or stream maps a call named like a flattened tool back to `name` and `namespace`. The names come from the app's request (`EncodeCtx::request`), so a call the app didn't namespace keeps its name.
- **Name limits.** A flattened name that breaks the target's tool-name rules is then rewritten like any other tool name and mapped back.
- **Refused.** A namespace with keys other than `type`, `name`, `description` and `tools`, a tool in it that is not a function, a field on such a tool that only Responses knows, such as `defer_loading`, and a flattened name that another tool already has.

`EgressCtx::tool_namespaces` says whether a Responses target takes namespaces. The gateway sets it from `Provider::tool_namespaces`, which is true only for OpenAI.

## Hosted web search

An OpenAI Responses `web_search` tool going to Anthropic becomes Anthropic's `web_search_20250305` server tool, named `web_search`:

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains` | `allowed_domains` |
| `user_location` (`approximate`, with `city`, `region`, `country`, `timezone`) | `user_location` |
| `external_web_access: true`, `search_context_size: "medium"` (the default) | nothing; both describe what Anthropic's search does |
| `external_web_access: false`, another `search_context_size`, any other field | `Unsupported` |

`external_web_access: false` asks OpenAI to read only its cache of pages, which is what Codex sends for its default `web_search = "cached"`. Claude always searches live, so the refusal says to set `web_search = "live"` or turn web search off. Other targets refuse `web_search` like any tool of another format; for a Chat Completions model, the refusal says how to turn web search off in Codex.

Claude's searches reach a Responses app as `web_search_call` items. A `server_tool_use` block named `web_search` becomes an item with the id `ws_kaeso_…` and `action: {type: search, query}`. Its status is `completed` once the matching `web_search_tool_result` arrives, `failed` when that result is an error, and `in_progress` when the reply ends before it, as with `pause_turn`. Streams send `response.output_item.added`, `response.web_search_call.in_progress`, `.searching`, `.completed` and `response.output_item.done`. The results themselves and the citations of the answer are left out, like other data the app's format can't show.

`web_search_call` items an app sends back are dropped for Anthropic. Anthropic replays a search only with its encrypted results, which these items don't carry, and the answer that used them stays in the conversation. Items with Kaeso's `ws_kaeso_` ids are dropped for OpenAI as well, which never issued them.

## Usage

`Usage::input_tokens` excludes cache reads and cache writes, which are fields of their own. This matches the price columns of models.dev, so a cost is the sum of each count times its price. The provider's raw usage object is always kept.

## System messages

A system message after the start of a conversation is kept as a `Role::System` message.

- **To Anthropic, from Anthropic Messages:** sent unchanged; the API judges their placement and which models support them. Claude Code sends such messages on every request.
- **To Anthropic, from other dialects:** `Unsupported`. Anthropic allows them only for some models and only after a user turn, while OpenAI system and developer messages may sit anywhere.
- **To OpenAI:** later `system` or `developer` messages.

## Keepalives

A provider can stream for minutes without producing anything the app's format can show: Claude's thinking for an OpenAI Chat app, Anthropic `ping` events, another dialect's unknown blocks, or an OpenAI reasoning model that sends nothing while it reasons. Proxies with idle timeouts (nginx and AWS ALB after 60 seconds, Cloudflare after 100) would cut the connection, and the time to first byte would equal the whole thinking time. So a live stream never goes silent because of translation: every stream encoder has a keepalive frame that is valid in its dialect and ignored by the official SDKs.

| Dialect | Keepalive frame | Why the SDKs ignore it |
| --- | --- | --- |
| OpenAI Chat Completions and Responses | The SSE comment `: keepalive` and a blank line | Lines starting with a colon are comments in the SSE specification, and the OpenAI SDK parsers skip them |
| Anthropic Messages | `event: ping` with `data: {"type": "ping"}`, exactly as Anthropic sends it | The Anthropic SDKs skip ping events |

Frames reach the wire in two ways:

- **In band.** An encoder adds the frame to what `StreamEncoder::encode` returns when an event from another dialect produced no bytes. A pacer limits this without a clock. Another dialect's extra event, such as `ping`, always earns a frame, because the provider already paces it. Hidden text, such as thinking the app can't see or buffered tool arguments, earns one frame per `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` bytes, 1,024. Anything the encoder writes resets the count. Events of the app's own dialect never earn frames, so a same-dialect replay stays exact.
- **Driven by the gateway.** `StreamEncoder::keepalive` returns the frame on demand, for upstream silence the encoder can't see. The gateway writes it when it has written nothing to the app for `KAESO_STREAMS_KEEPALIVE_INTERVAL`, 15 seconds by default.

The OpenAI Chat encoder also sends its role chunk as soon as the response starts, rather than with the first text, so the app hears from the stream while the model thinks.

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

A reply keeps the provider's id as it is, whatever the app's format. A Claude reply reaches an OpenAI Chat Completions or Responses app with its Anthropic id (`msg_…`). An OpenAI reply reaches an Anthropic Messages app with OpenAI's id (`chatcmpl-…` or `resp_…`). Streams carry the same id. The codecs never make up an id in the app's own pattern: the provider's id names the reply at the provider, and apps treat reply ids as opaque strings. The exceptions are Responses output items, which other formats don't have ([Responses storage and ids](#responses-storage-and-ids)), and a Responses stream without a start event, which gets the id `resp_kaeso_<created_at>` ([OpenAI Responses](openai-responses.md)).

## Responses storage and ids

- **`store: false`.** Requests from other dialects encoded for OpenAI Responses carry `"store": false`: Kaeso is stateless, and without it OpenAI would store every translated conversation. Requests decoded from Responses keep the app's spelling, so an absent `store` still means OpenAI stores the response.
- **Item ids.** Responses output items need ids, which replies from other dialects don't have. Kaeso gives them ids starting with `msg_kaeso_`, `fc_kaeso_` and `rs_kaeso_`; when an app sends them back, the request decoder removes them, so OpenAI never receives an id it did not issue.

## Cache hints

Cache hints are hints. OpenAI caches by itself, so hints from other dialects are dropped for OpenAI. OpenAI's own `prompt_cache_breakpoint` decodes to a KPP `CacheHint` and is written back only to the dialect it came from.

## Tool errors

`ToolResult.is_error` is a hint for targets without an error flag, OpenAI Chat and Responses: it is dropped, and the result's content carries the error text. Refusing it would make every failed tool call of an Anthropic-format client such as Claude Code unroutable to OpenAI.

## Files

A provider file id (`file-…`) is usable only within that provider's file store, and the two OpenAI generation dialects share one, so `cross::check_foreign` takes the target's `Origin`: its dialect and provider. File ids crossing to another provider are `Unsupported`.

## The Anthropic version

The Anthropic codec pins one API version, `2023-06-01`, exported as `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`. It is the only version Anthropic's [versioning page](https://platform.claude.com/docs/en/api/versioning) lists, the one the official SDKs send, and the one every Anthropic fixture assumes.

- **Egress.** Every request the Anthropic codec encodes carries `anthropic-version: 2023-06-01`, unless the app's own Anthropic request forwarded an `anthropic-version` naming a newer version: a well-formed `YYYY-MM-DD` that sorts after the pinned one. A malformed or older value is ignored. A request from another dialect never carries the header, so it always gets the pinned version.
- **Ingress.** A forwarded `anthropic-version` equal to the pinned version is not stored in the request's extras: the codec sends it anyway, and keeping it would bind every Anthropic SDK request to its dialect. Any other value is kept as the extra `header:anthropic-version` and compared on egress as above.
- **Why newer wins.** Anthropic only adds to a version, with new optional inputs and new output values, and an app that asks for a newer version relies on its behaviour. The decoders tolerate unknown fields, blocks and events, so a newer version degrades to extras and unknown items rather than failing.

To change the pin, change the constant, record the fixtures again with `cargo xtask record` ([recording](fixtures.md#recording-live-responses)), and update this section.
