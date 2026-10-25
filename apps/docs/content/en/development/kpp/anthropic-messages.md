---
title: Anthropic Messages
description: How every field of the Anthropic Messages format maps to KPP and back.
order: 4
---

Look up any field of the Anthropic Messages format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::anthropic_messages::AnthropicMessages` (`crates/kaeso-codecs/src/anthropic_messages/`)
- **Endpoints:** `POST /v1/messages` (JSON and SSE), `POST /v1/messages/count_tokens`, and the [models](#models) endpoints
- **Fixtures:** `crates/kaeso-codecs/fixtures/anthropic_messages/`

**Sources.** The mappings follow these sources:

- The Anthropic OpenAPI spec `anthropic-465bff21a179090915396565d1ae8f705cf8596e2ec920eb121072f25b8a7d68.yml` (the last `openapi_spec_url` in `anthropic-sdk-python/.stats.yml`, SDK commit `41aa7674`)
- The SDK types at `anthropic-sdk-python@b4b7916d` (1.13.0)
- The [Claude documentation](https://platform.claude.com/docs/en/) (`api/messages/create`, `api/messages/count_tokens`, `api/errors`, `api/versioning`, `build-with-claude/streaming`, `…/thinking`, `…/extended-thinking`, `…/effort`, `…/prompt-caching`, `…/structured-outputs`, `…/citations`, `…/pdf-support`, `…/vision`, `agents-and-tools/tool-use/*`, `…/handling-stop-reasons`)

Where the SDK types are newer than the spec, they win, and a field found only in the SDK types is cited as such. Schema names below refer to the spec.

## How to read the tables

- **KPP**: the typed field the wire value maps to.
- **fields**: kept raw in `Extras::fields`. These are semantic: written back only to Anthropic, refused with `Unsupported` by other dialects ([extras](cross-format.md#extras)) unless the [allowlist](cross-format.md#extras) lets them drop, and left out of replies to other dialects ([replies](cross-format.md#replies)). Nested objects use dotted key paths (`metadata.x`).
- **form**: a codec-private presentation marker in `Extras::form`, ignored by other dialects. `verbatim` means the generic `wire::Ext` mechanism (`null`, `false` and `[]` spellings).
- **raw**: kept whole in a `RawItem`, `Citation` or `RawUsage`.
- **Own** data has extras or an origin of `anthropic_messages`; **foreign** data comes from another dialect.

**Numbers.** Typed numeric fields are read as `u32` or `u64`, and `temperature` and `top_p` as JSON numbers that KPP holds as `f64`. A value outside their range is never changed: it fails to decode, as `InvalidRequest` in a request (`` The request body is not a valid Anthropic Messages request: `top_k` must be a non-negative integer. ``) and `ProviderError` in a provider reply.

**Keys Kaeso can't carry.** Every object Kaeso reads field by field keeps its unknown keys as fields. The exception is a key that starts with `header:` or contains `.`: it would be mistaken for a forwarded header or a nested key path. The object is then refused ([wire structs](codec-design.md#wire-structs)).

In a request the refusal is `InvalidRequest` (`` The request body is not a valid Anthropic Messages request: the field `a.b` has a name Kaeso cannot carry; remove it. ``); in a reply, a stream or a model listing it is `ProviderError`. An error body with such a key at its top level counts as a body that is not an `ErrorResponse` ([errors](#errors)).

- **Read field by field:** the body, messages, `metadata`, `output_config`, `thinking`, `tool_choice`, function tools, replies, token counts, stream events and their deltas, model info and model lists. The same holds for the known block types of the [content blocks](#content-blocks) table, with their `source`, `cache_control` and `citations` objects.
- **Kept whole, not checked:** unknown blocks and tools, `input_schema`, tool input, citations, `usage` and `output_config.format`.

## Requests

Schemas `CreateMessageParams` and `CountMessageTokensParams`.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `model` | |
| `messages[]` (`InputMessage`) | `messages[]` | Unknown keys → message `extras.fields`. |
| `messages[].output_config`, `.clear_at` (system messages, beta; SDK `BetaMessageParam`) | message `extras.fields` | Other dialects: an `output_config` that is `{}` or repeats the request's `output_config.effort` is dropped, because it changes nothing (Claude Code sends it on its system message); any other value, and `clear_at`, → `Unsupported` ([extras](cross-format.md#extras)). |
| `messages[].role` `user` / `assistant` | `Role::User` / `Role::Assistant` | |
| `messages[].role` `system` | `Role::System` | A system message in the middle of the conversation (SDK `MessageParam.role`; [mid-conversation system messages](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)). Sent back to Anthropic unchanged, and to OpenAI as a later `system` or `developer` message ([system messages](cross-format.md#system-messages)). Other roles → `InvalidRequest`. |
| `messages[].content` string | one `Block::Text` | form `content: "string"`; foreign messages are always sent as arrays. |
| `messages[].content` array | `blocks` | `[]` → form `content: "array"`. |
| `system` string | `system: [Text]` | form `system: "string"`. |
| `system` array (`RequestTextBlock[]`) | `system` | `[]` → form `system: "array"`. |
| `max_tokens` | `sampling.max_output_tokens` | Required for generation (`CreateMessageParams`): a request without it is `InvalidRequest` when it is decoded (`` The request body is not a valid Anthropic Messages request: `max_tokens` is required. ``, param `max_tokens`), so the gateway never fills it in for an Anthropic app; token counting has no `max_tokens`. `0` is valid, the spec's minimum: it fills the prompt cache without an answer. A request from another dialect without `sampling.max_output_tokens` is `InvalidRequest` in `check()`, naming the app's own field (`max_completion_tokens` or `max_output_tokens`, [crossing formats](cross-format.md#requests)). |
| `temperature`, `top_p`, `top_k` | `sampling.temperature`, `.top_p`, `.top_k` | Deprecated in the spec and absent from SDK 1.13.0, but still mapped. `temperature` and `top_p` keep their spelling: the keys written as integers (`1`) are listed in form `integers` and written as integers again while the value is still a whole number, so `temperature: 1` comes back as `1`, not `1.0`; a changed value is written as the new value. Requests from other dialects get Anthropic's own spelling (`1.0`) ([sampling](cross-format.md#sampling)). |
| `stop_sequences` | `sampling.stop` | `[]` → verbatim. |
| `stream: true` | `stream = On { include_usage: true }` | Anthropic always reports usage. `stream: false` → verbatim. |
| `metadata.user_id` | `user` | `null` → verbatim (`verbatim:metadata`); other keys → fields `metadata.*`; a present `metadata` object → form `objects: ["metadata"]`. |
| `thinking.type` `enabled` / `disabled` / `adaptive` | `reasoning.mode` `Enabled` / `Disabled` / `Adaptive` | `between_tools` (SDK only) and future types → `ReasoningMode::Other`. |
| `thinking.budget_tokens` | `reasoning.budget_tokens` | |
| `thinking.display` and other `thinking` keys | `reasoning.extras.fields` | No KPP field. `display` is on the [allowlist](cross-format.md#extras), so it is dropped for other dialects; OpenAI Responses gets `reasoning.summary: "auto"` unless it is `omitted` ([readable reasoning and summaries](reasoning.md#readable-reasoning-and-summaries)). Other keys → `Unsupported` there. |
| `output_config.effort` | `reasoning.effort` | `low`, `medium`, `high`, `xhigh`, `max`; unknown values → `ReasoningEffort::Other`. |
| `output_config.format` `{type: json_schema, schema}` | `output = JsonSchema { schema }` | `name`, `description` and `strict` are `None`. Unknown keys of `format` → the spec's `extras`. Other format types, and a `json_schema` format Kaeso cannot read field by field (without `schema`, or with a key it cannot carry), → fields `output_config.format`. |
| other `output_config` keys (such as `task_budget`, beta) | fields `output_config.*` | A present `output_config` → form `objects: ["output_config"]`; `null` members → `verbatim:output_config`. `task_budget` puts a countdown in front of the model, so it is semantic: other dialects → `Unsupported`, with a hint to set `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` in Claude Code. |
| `context_management` (beta, `BetaContextManagementConfigParam`) | fields `context_management` | Other dialects: dropped when it holds only `edits` and every edit is `clear_thinking_*`, because it only governs Anthropic thinking blocks, which never cross ([reasoning](reasoning.md)); Claude Code sends `clear_thinking_20251015` with `keep: "all"` on every request. Other edits (`clear_tool_uses_*`, `compact_*`) change what the model sees → `Unsupported`, with the `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` hint. |
| `tool_choice` `auto` / `any` / `none` / `tool{name}` | `tool_choice` `Auto` / `Required` / `None` / `Tool(name)` | Other types, `tool` without a `name`, and `auto`, `any` or `none` with one → `ToolChoice::Unknown` (raw, own only). Unknown keys → fields `tool_choice.*`. |
| `tool_choice.disable_parallel_tool_use` | `parallel_tool_calls = Some(!value)` | |
| `tools[]` with `type` absent, `null` or `custom` (`Tool`) | `ToolDef::Function` | The `type` spelling → `verbatim`. |
| `tools[].name`, `.description`, `.input_schema`, `.strict`, `.cache_control` | `name`, `description`, `parameters` (raw), `strict`, `cache` | |
| `tools[].defer_loading` | tool `extras.fields` | Other dialects: a deferred tool is left out until a `tool_reference` loads it ([tool search](cross-format.md#tool-search)). |
| `tools[].eager_input_streaming`, `.allowed_callers`, `.input_examples` | tool `extras.fields` | Other dialects → `Unsupported`; the message tells Claude Code users to set `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`, which stops these beta tool fields. |
| `tools[]` of any other type (server tools such as `web_search_20250305` or `advisor_20260301`) | `ToolDef::Unknown` (raw) | Same dialect only. For other dialects the `Unsupported` message names the type after `Input tag` (`Input tag 'advisor_20260301' found using 'type' is not a function tool, …`), the API's own wording, so Claude Code retries without the advisor tool ([gateway protocol](https://code.claude.com/docs/en/llm-gateway-protocol), "Automatic retry and error forwarding"). |
| `cache_control` (top level, automatic caching) | fields `cache_control` | Only a hint; KPP has no request-level cache hint. On the [allowlist](cross-format.md#extras), so it is dropped for other dialects. |
| `container`, `inference_geo`, `service_tier`, SDK `diagnostics`, `user_profile_id`, `workspace_id`, unknown keys | `extras.fields` | |
| header `anthropic-beta` | `extras` header `header:anthropic-beta` | Forwarded on egress to Anthropic only. |
| header `anthropic-version` | `extras` header when it is not `2023-06-01` | The pinned version is always sent unless a newer one was forwarded ([the Anthropic version](cross-format.md#the-anthropic-version)). |

### Content blocks

These apply to requests and replies alike.

| Wire block | KPP | Notes |
|---|---|---|
| `text` (`text`, `citations`, `cache_control`) | `Block::Text` | `citations[]` → `Citation { origin, raw }`; `null` and `[]` → verbatim. |
| `image` `source.type: base64` (`media_type`, `data`) | `Image { source: Base64 }` | |
| `image` `source.type: url` | `Image { source: Url }` | |
| `image` `source.type: file` (`file_id`) | `Image { source: FileId { origin } }` | Usable only with Anthropic (`cross::file_id_usable`, [files](cross-format.md#files)). |
| `image.transformations`, unknown keys | block `extras.fields` | Unknown source keys → `source.*`. An image whose `source.type` is none of these → `Block::Unknown`. |
| `document` `source.type: base64` (PDF), `url` or `file` | `Document { source: Base64 / Url / FileId }` | |
| `document` `source.type: text` (`text/plain`) | `Document { source: Text }` | |
| `document` `source.type: content` (custom content) | `Block::Unknown` | Same dialect only. |
| `document.title`, `.context` | `title`, `context` | `null` → verbatim. |
| `document.citations.enabled` | `citations` | `{enabled: false}` → form `citations: "disabled"`; `{}` → `"empty"`; `null` → verbatim. |
| `cache_control` on any block (`CacheControlEphemeral`) | `cache = CacheHint { ttl }` | `ttl` `5m` → `FiveMinutes`, `1h` → `OneHour`, other values → `Other`; absent → `None`. A `type` other than `ephemeral` → hint `extras.fields.type`. |
| `tool_use` (`id`, `name`, `input`, `cache_control`) | `Block::ToolCall` (`arguments` raw) | `caller` and `toolset_name` → block `extras.fields`, except `caller: {type: "direct"}`, a plain call, which is kept as verbatim so the call can go to other dialects. |
| `tool_result` (`tool_use_id`, `content`, `is_error`, `cache_control`) | `Block::ToolResult` | `content` string → form `content: "string"`; `[]` → `"array"`; `is_error: false` → verbatim. `toolset_name` → fields. A `tool_result` nested more than 4 levels deep in another one is refused: `InvalidRequest` in a request, `ProviderError` in a reply or stream. |
| `thinking` (`thinking`, `signature`) | `Block::Reasoning { text: [thinking], opaque: [AnthropicThinkingSignature] }` | `thinking: ""` (display omitted) stays `[""]`. `signature: ""` is not opaque data: it becomes form `empty_signature`, so the block never counts as signed thinking, and is written back as `""`; an absent `signature` stays absent. A `signature` that is a reasoning blob (`kpp:reasoning:v1:…`, another dialect's reasoning Kaeso gave the app, see [reasoning blobs](reasoning.md#reasoning-blobs)) decodes to the original block inside it, origin included; the `thinking` text is then ignored. |
| `redacted_thinking` (`data`) | `Block::Reasoning { opaque: [AnthropicRedactedThinking] }` | |
| `tool_reference` (`tool_name`, `cache_control`) | `Block::Unknown` (raw) | In a `tool_result`, other dialects get a text block with the tool's name and the referenced tool ([tool search](cross-format.md#tool-search)). |
| any other type (`server_tool_use`, `web_search_tool_result`, `web_fetch_tool_result`, `code_execution_tool_result`, `search_result`, `container_upload`, future types) | `Block::Unknown` (raw) | Same dialect only. |

### Requests from other dialects

What `encode_request` writes for a KPP request that did not come from Anthropic Messages.

| KPP | Wire | Notes |
|---|---|---|
| `system` | `system` as an array of text blocks | Other blocks in the system prompt → `Unsupported`. |
| `messages[].blocks` | `content` as an array | Empty text blocks are dropped, because Anthropic rejects them. Messages left empty after dropping reasoning or empty text are left out. |
| `Block::Reasoning` not from Anthropic, or from another Anthropic provider | dropped | [Reasoning](reasoning.md). This includes another dialect's reasoning that an app sent back in a reasoning blob: it goes to its own provider only. |
| `thinking` without a non-empty signature | own blocks (read from Anthropic Messages): written as received, an absent `signature` stays absent and `""` (form `empty_signature`) stays `""`; others dropped | An assistant turn that holds only such an own block is kept. Unsigned Anthropic reasoning that was not read from Anthropic Messages (built by code, without Anthropic extras) is dropped, because Anthropic rejects unsigned thinking blocks. `redacted_thinking` is replayed. |
| `Source::Url` with a base64 `data:` URL (from another dialect) | `source: {type: base64, media_type, data}` | A `data:` URL that is not base64 → `Unsupported`; media types are checked as for base64. |
| `ToolResult` content other than text, image or document (and own unknown blocks) | `Unsupported` | `RequestToolResultBlock.content`. |
| `Block::Refusal` | `Unsupported` (`refusal block`) | |
| `Text.cache`, any `CacheHint` | `cache_control {type: ephemeral, ttl?}` | Foreign hint extras are dropped. |
| `ToolCall.id` or `ToolResult.call_id` not matching `^[a-zA-Z0-9_-]+$` | rewritten (other characters become `_`, then `_` and a 16-digit hexadecimal FNV-1a hash are appended) and recorded in `EgressCtx::tool_ids` | Mapped back in replies and streams. |
| tool names not matching `^[a-zA-Z0-9_-]{1,128}$` | rewritten the same way, at most 128 characters including the hash, and recorded in `EgressCtx::tool_names` | Applied to `tools[].name`, `tool_choice.name` and `tool_use.name`. |
| `FunctionTool.parameters: None` | `input_schema: {"type":"object","properties":{}}` | For OpenAI, no parameters means an empty parameter list. |
| `parallel_tool_calls: Some(false)` without `tool_choice` | `tool_choice: {type: auto, disable_parallel_tool_use: true}` | Not added to `none`, whose schema has no such key. |
| `reasoning.effort` `low` to `max` without mode or budget | `thinking: {type: adaptive}` plus `output_config.effort`; on models without adaptive thinking (`EgressCtx::adaptive_thinking = Some(false)`), `thinking: {type: enabled, budget_tokens}` from the [effort and budget](reasoning.md#effort-and-budget) table (`low` 2,048, `medium` 8,192, `high`, `xhigh` and `max` 24,576, fitted to at least 1,024 and less than `max_tokens`; when `max_tokens` is 1,024 or less it is raised by 1,024 and the budget is 1,024) | [Reasoning](reasoning.md). |
| `reasoning.summary` without effort, mode or budget, with `EgressCtx::reasoning = Some(true)` | `thinking: {type: adaptive}` without an effort (the model's default depth); on models without adaptive thinking, the `medium` budget | OpenAI reasoning models reason by default, so an app asking for summaries expects reasoning ([readable reasoning and summaries](reasoning.md#readable-reasoning-and-summaries)). Unknown or non-reasoning models: nothing is sent. |
| `reasoning.summary` (any value) with thinking on | `thinking.display: "summarized"` | Claude Opus 4.7 and later default to `omitted` (empty thinking text). Not for token counting. On models that write short progress notes between tool calls, these come back mixed with the summaries; `display: "updates"` is never sent. |
| `sampling.temperature`, `.top_p`, `.top_k` when thinking from another dialect is on | left out | Anthropic refuses them, or limits `top_p`, with thinking ([thinking and sampling](reasoning.md#thinking-and-sampling)). Without reasoning settings they are sent as received, even to models that think by default; the gateway refuses a non-default `temperature` for models whose catalogue entry says `temperature: false`. |
| `tool_choice` `Required` or `Tool` from another dialect, with `enabled` thinking | sent without `thinking` (and without its raised `max_tokens`) | Anthropic refuses the combination, and a forced call holds no thinking anyway. Adaptive thinking keeps the forced tool. |
| `reasoning.effort` `none` | no `thinking`, no effort | Anthropic's default. Models that think when `thinking` is absent (Claude Opus 5 and 5.5, Sonnet 5 and 5.5, Haiku 5.5 and Fable) therefore still think at their default effort. `none` with thinking on, `minimal` and unknown efforts → `Unsupported`. |
| `reasoning.budget_tokens` (no mode) | `thinking: {type: enabled, budget_tokens}` | |
| `reasoning.mode = Enabled` without a budget; a budget with `Adaptive` or `Disabled` | `Unsupported` | No effort and budget conversion applies here. |
| `reasoning.include_encrypted` from another dialect | dropped | It only says which reasoning to return (Codex always sends it); Anthropic always returns signed thinking. |
| `thinking` `enabled` when the last message (system messages after it aside) returns tool results and the last assistant turn made tool calls without starting with a replayable thinking block (signed `thinking` or `redacted_thinking`; for example because that turn came from another provider) | the request is sent without `thinking` and `output_config.effort` | Anthropic rejects such a request ("a final assistant message must start with a thinking block"); see [tool turns without thinking](reasoning.md#tool-turns-without-thinking). `adaptive` thinking is kept: the adaptive-thinking documentation says it doesn't need the leading block. Nothing else is recorded. This is rare, because thinking Claude produced comes back from Responses and Anthropic apps in [reasoning blobs](reasoning.md#reasoning-blobs) and from Chat apps through the [gateway's store](reasoning.md#reasoning-kept-by-the-gateway), so the turn starts with replayable thinking. The rule applies to requests from Anthropic apps too. |
| `output = JsonObject` | `Unsupported` (`json_object output`) | [Structured output](cross-format.md#structured-output). |
| `output = JsonSchema` | `output_config.format` | `name` is dropped; `description` and `strict: false` → `Unsupported`. |
| `sampling.seed`, `.frequency_penalty`, `.presence_penalty` | `Unsupported` | [Sampling](cross-format.md#sampling); not for token counting, where sampling is left out. |
| `sampling.temperature` outside 0 to 1 | `Unsupported` | Anthropic's range is 0 to 1 (OpenAI allows up to 2); the value is not rescaled. Not for token counting. |
| `user` | `metadata.user_id` | |
| `Role::System` | a `role: "system"` message for requests decoded from Anthropic Messages; from other dialects, text of the next user turn, after its tool results | [System messages](cross-format.md#system-messages). |
| base64 images other than JPEG, PNG, GIF or WebP; text image sources | `Unsupported` | `Base64ImageSource`. |
| base64 documents other than `application/pdf`; text documents other than `text/plain` | `Unsupported` | `Base64PDFSource`, `PlainTextSource`. |
| tool call arguments that are not an object; tool parameters that are not an object | `Unsupported` | `input` and `input_schema` are objects. |
| `Request::CountTokens` from another dialect | `/v1/messages/count_tokens` without `max_tokens`, sampling, `metadata` and `stream` | They don't change the input count, and the endpoint rejects them. An Anthropic app's own token-counting request is sent as received. |
| `Request::CountTokens` output and reasoning settings that `check()` would refuse for generation (JSON mode, a schema description, `strict: false`, effort `minimal`, …) | left out, not refused | Only what changes the counted input is refused. |
| `Request::Embed` | `Unsupported` | |
| OpenAI Responses `web_search` tool | `{type: web_search_20250305, name: web_search}` | With `allowed_domains` and `user_location` where set; left out with `external_web_access: false`; Responses `web_search_call` items in the conversation are dropped ([hosted web search](cross-format.md#hosted-web-search)). |
| OpenAI Responses `custom_tool_call`, `local_shell_call` and their outputs | `tool_use` and `tool_result` blocks | [Codex tool history](cross-format.md#codex-tool-history). |
| OpenAI Responses `text.verbosity` | left out | No equivalent; named in `kaeso-left-out` ([conditional entries](cross-format.md#conditional-entries)). |
| OpenAI Responses `namespace` tools | one function tool per tool in the namespace, named `<namespace>__<name>` | [Tool namespaces](cross-format.md#tool-namespaces). |
| foreign extras, unknown blocks, tools and tool choices, citations, foreign file ids | `Unsupported` | `cross::check_foreign` ([requests](cross-format.md#requests)). |

## Replies

Schema `Message`.

| Wire field | KPP | Notes |
|---|---|---|
| `id`, `model` | `id`, `model` | From another format, the provider's id as it is ([reply ids](cross-format.md#reply-ids)). |
| `content[]` | `blocks` | As for [content blocks](#content-blocks); tool ids and names are mapped back through `EgressCtx`. |
| `stop_reason` `end_turn` / `max_tokens` / `stop_sequence` / `tool_use` / `pause_turn` / `refusal` | `stop` `EndTurn` / `MaxTokens` / `StopSequence(s)` / `ToolUse` / `PauseTurn` / `Refusal` | |
| `stop_reason` `model_context_window_exceeded` | `MaxTokens` plus form `spelling:stop_reason` | The output was cut by the context window; Anthropic apps get the exact value back. |
| unknown `stop_reason` values | `StopReason::Other(value)` | |
| `stop_reason: null` or absent | `EndTurn` plus form `spelling:stop_reason` | |
| `stop_sequence` | inside `StopSequence(s)` | Any other spelling (absent, or a value with another stop reason) → form `spelling:stop_sequence`. |
| `stop_details` (`RefusalStopDetails`), `container`, `diagnostics`, `type`, `role`, unknown keys | `extras.fields` | |
| `usage.input_tokens` | `usage.input_tokens` | Already excludes cache reads and writes ([usage](cross-format.md#usage)). |
| `usage.cache_read_input_tokens` | `usage.cache_read_tokens` | `null` → 0. |
| `usage.cache_creation_input_tokens` | `usage.cache_write_tokens` | `null` → 0. |
| `usage.cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` | `usage.cache_write_breakdown` | |
| `usage.output_tokens` | `usage.output_tokens` | Includes thinking. |
| `usage.output_tokens_details.thinking_tokens` | `usage.reasoning_tokens` | |
| the whole `usage` (with `server_tool_use`, `service_tier`, `inference_geo`, `iterations` and unknown keys) | `usage.raw` (bound to its origin) | Written again with the same keys and values for Anthropic replies. |

Replies from other dialects (`encode_response`) get `type: "message"`, `role: "assistant"`, `stop_sequence: null` (or the sequence) and `stop_details: null`. Usage is built from the typed fields: `cache_creation` from the breakdown or `null`, and `output_tokens_details` when `reasoning_tokens` is known.

- **Stop reasons.** `ContentFilter` becomes `refusal`, because Anthropic has no content-filter stop reason. `Other(s)` becomes `s` when it is a documented Anthropic value; otherwise `length`, `max_output_tokens` and `max_completion_tokens` become `max_tokens`, `function_call` and `tool_calls` become `tool_use`, `content_filter` becomes `refusal`, and anything else becomes `end_turn`. Own `Other` values are written as received.
- **Refusals.** `Refusal` blocks become `text` blocks.
- **Tool arguments that are not an object.** KPP keeps tool call arguments that are not valid JSON, for example cut off by an output limit, as a JSON string. Other JSON values stay as they are. `tool_use.input` must be an object, and an empty `input` would call the tool with arguments the model never wrote. So such a reply from another dialect is refused with `ProviderError` (`` The provider returned arguments for the tool call `…` that are not a JSON object, … ``). The gateway still logs the reply and its usage. Requests with such arguments are refused by `check()`.
- **Reasoning** from other dialects becomes a `thinking` block ([reasoning blobs](reasoning.md#reasoning-blobs)). Its `thinking` holds the summaries, then the reasoning text, with a blank line between paragraphs. Its `signature` is a reasoning blob of the original block. Anthropic never sees that block: the request decoder restores the original, which only goes back to its own provider.
- **Left out** ([replies](cross-format.md#replies)): image, document and tool result blocks, which are not part of the `Message` content union, and foreign opaque data, citations, extras and unknown blocks. Anthropic reasoning without a signature is left out too, unless it was read from Anthropic Messages: own unsigned thinking is written as received.

## Token counts

Schema `CountMessageTokensResponse`.

| Wire field | KPP | Notes |
|---|---|---|
| `input_tokens` | `CountTokensResponse::input_tokens` | All input tokens; the endpoint reports one total. |
| unknown keys | `extras.fields` | |

## Streams

Schema `MessageStreamEvent`.

| SSE event | KPP | Notes |
|---|---|---|
| `message_start` | `ResponseStart { id, model, usage }` | Other `message` keys (`type`, `role`, `content: []`, `stop_reason`, `stop_sequence`, `stop_details`, …) → fields `message.*`. |
| `content_block_start` `text` | `BlockStart(Text)` | Initial text and citations → `BlockDelta`s with form `in_start`. |
| `content_block_start` `tool_use` | `BlockStart(ToolCall { id, name })` | Ids and names are mapped back; a non-empty initial `input` → a `ToolArguments` delta with `in_start`. |
| `content_block_start` `thinking` | `BlockStart(Reasoning { origin })` | Initial thinking and signature → deltas with `in_start`. |
| `content_block_start` `redacted_thinking` | `BlockStart(Reasoning { opaque: [AnthropicRedactedThinking] })` | |
| `content_block_start` of other types | `BlockStart(Unknown)` (raw) | |
| a start block that differs from the default (`{type:text,text:""}`, `{type:tool_use,id,name,input:{}}`, `{type:thinking,thinking:"",signature:""}`, `{type:redacted_thinking,data}`) | form `content_block` (the exact block) | |
| `content_block_start.index` that differs from the KPP index | form `index` | Anthropic numbers blocks 0, 1, …, so this only happens in unusual streams. |
| `text_delta` | `Delta::Text` | |
| `input_json_delta` (`partial_json`) | `Delta::ToolArguments(partial_json)` | The `partial_json` fragments of a block concatenate to the arguments. On `Unknown` blocks (such as `server_tool_use`): `Delta::Unknown` (raw). |
| `thinking_delta` | `Delta::ReasoningText` | |
| `signature_delta` | `Delta::Opaque(AnthropicThinkingSignature)` | |
| `citations_delta` | `Delta::Citation` | |
| unknown delta types | `Delta::Unknown` (raw) | |
| `content_block_stop` | `BlockStop` | |
| `message_delta.delta.stop_reason` / `.stop_sequence` | `ResponseDelta.stop` | Unusual spellings → form `delta:spelling:*`. |
| `message_delta.delta.stop_details`, `.container`, unknown keys | fields `delta.*` | |
| `message_delta.usage` (`MessageDeltaUsage`) | `ResponseDelta.usage` (cumulative) | Counts it omits and the cache-write breakdown are completed from `message_start`; `raw` is the delta's own object. |
| `message_stop` | `ResponseStop` | |
| `error` | `StreamEvent::Error` (kind from the [error table](#errors), no status) | The decoder stops. |
| `ping`, unknown events | `StreamEvent::Extra { event, data }` | Same dialect only. Data that is not JSON is kept as text. The frame's `id`, `retry` and other SSE fields stay in the extra. |
| a frame without a `data:` line (such as `retry: 3000` or `event: ping` alone), whatever its event name | `StreamEvent::Extra` without a `data` key | Not an event under the SSE specification, so it is never an error or a lifecycle event. Written back without a `data:` line. |
| SSE comments | `StreamEvent::Extra { comment }` | |
| an `event:` name that differs from the data's `type`; `id:`, `retry:` and other SSE fields | form `event` / `sse` | `sse` is `{"id", "retry", "other": [[name, value], …]}` and is written back on the same frame. An `id` that contains NUL and a `retry` that is not ASCII digits fitting a `u64` are not an `id` or a `retry` under the specification; they are kept with the other fields ([server-sent events](codec-design.md#server-sent-events)). |
| order violations: block or message events before `message_start`, a second `message_start`, lifecycle events after `message_stop`, `message_stop` with an open block, a block index started twice, a delta or stop for a block that never started or already stopped, a known delta type on the wrong kind of block (such as `text_delta` on `tool_use`; any delta on an unknown block is fine), a known event whose data has no `type` | a final `StreamEvent::Error` (`ProviderError`); earlier events are kept | `ping`, comments and unknown events may follow `message_stop`. |
| a stream that ends without `message_stop` or inside a block; a known event whose data is not JSON | a final `StreamEvent::Error` (`ProviderError`) | Gaps in Anthropic indexes are not errors: blocks get KPP indexes in start order (form `index`). |

Encoding events from other dialects:

- **One block at a time.** Interleaved blocks are buffered in start order, and a `ResponseDelta` with a stop reason, or `ResponseStop`, closes open blocks. A foreign `ResponseDelta` without a stop reason, such as the running usage some OpenAI-compatible providers send on every chunk, only updates the usage of the final `message_delta`. An OpenAI stream that keeps its text block open until the end therefore delivers its tool calls only at the end. Several foreign `ResponseDelta`s merge into one `message_delta`.
- **A missing `ResponseStart`** becomes a `message_start` with an empty id and the request's model.
- **Refusals** become text blocks.
- **Tool call arguments** from other dialects are collected and written as one `input_json_delta` when the block stops, whenever they are not blank. Arguments that don't form a JSON object, such as ones cut off by an output limit, are sent as they came. Anthropic itself streams a call cut off by `max_tokens` the same way, so the app sees what the model wrote. Anthropic's own streams are replayed as received.
- **Reasoning blocks** from other dialects become `thinking` blocks ([reasoning blobs](reasoning.md#reasoning-blobs)). Summaries and reasoning text become `thinking_delta`s, with a blank line between parts. Opaque deltas are collected, and when the block stops, one `signature_delta` carries the reasoning blob of the whole block.
- **Left out** ([replies](cross-format.md#replies)): foreign citation and unknown deltas, unknown blocks and `Extra` events, as in complete replies. While they arrive, the encoder writes paced `event: ping` frames (`{"type": "ping"}`, as Anthropic sends them): one per foreign extra event and one per 1,024 bytes of hidden text ([keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gives the gateway the same frame.
- **Stop reasons:** `StopReason::Other` is mapped as for replies.
- **Errors:** a KPP `Error` becomes `event: error` with its body as for [errors](#errors), without a `request_id`: errors decoded from Anthropic as received, others as `{"type":"error","error":{type,message}}`.

## Errors

Schema `ErrorResponse`. Decode errors, for requests and provider bodies alike, name the field and what is wrong, never a parser position.

| HTTP | `error.type` | `ErrorKind` (decode) | Encode from `ErrorKind` |
|---|---|---|---|
| 400 | `invalid_request_error` | `InvalidRequest`; `ContextTooLong` when the message starts with `prompt is too long` | `InvalidRequest`, `ContextTooLong`, `ContentFiltered`, `Unsupported` |
| 401 | `authentication_error` | `Authentication` | `Authentication` |
| 402 | `billing_error` | `Billing` | `Billing` |
| 403 | `permission_error` | `PermissionDenied` | `PermissionDenied` |
| 404 | `not_found_error` | `NotFound` | `NotFound` |
| 409 | `conflict_error` | `InvalidRequest` (status 409 kept) | none |
| 413 | `request_too_large` | `RequestTooLarge` | `RequestTooLarge` |
| 429 | `rate_limit_error` | `RateLimited` | `RateLimited` |
| 500 | `api_error` | `ProviderError` | `ProviderError` (a 5xx status is kept), unknown kinds |
| 504 | `timeout_error` | `Timeout` | `Timeout` |
| 529 | `overloaded_error` | `Overloaded` | `Overloaded` |

- `error.message` → `message`; the whole body (with `request_id` and `error.details`) → `raw`; the HTTP status → `status`; `retry-after-ms` or `retry-after` → `retry_after`, which is written back as `retry-after`, rounded up to whole seconds.
- A too-long prompt is a 400 `invalid_request_error` like any other, so only its message tells it apart. If Anthropic changes the wording, the error decodes as `InvalidRequest`.
- An unknown `error.type`, or a body that is not an `ErrorResponse`, gets its kind from the status. Examples of such bodies are Cloudflare's HTML 413, JSON whose `type` is not `error`, and JSON with a top-level key Kaeso cannot carry. The status and the raw body (as a string when it is not JSON) are kept, and such bodies are encoded again in Anthropic's error shape.
- Statuses 400, 409 and 422 give `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied` and 404 `NotFound`. Statuses 408 and 504 give `Timeout`, 413 `RequestTooLarge`, 429 `RateLimited`, and 503 and 529 `Overloaded`. Any other status gives `ProviderError`, and so do stream errors, which have none. A documented `error.type` always wins over the status.
- Errors decoded from Anthropic are encoded again from `raw` with their status, so Anthropic apps see them unchanged; others are built as `{"type":"error","error":{"type","message"},"request_id":null}`.
- `request_id` is required in `ErrorResponse`, a string or `null`. `encode_error_for_request` (`dispatch::encode_error_for_request`) writes the given id there, including into a body encoded again from `raw`, where it replaces Anthropic's id. The gateway passes the id it sends in `kaeso-request-id` and keeps Anthropic's `request-id` header in its request log. `encode_error` writes `null` into the bodies it builds.
- Other response headers (such as `anthropic-ratelimit-*` and `request-id`) are not part of `KppError`.

## Models

`GET /v1/models` and `GET /v1/models/{model_id}`. An app's request is Anthropic when it carries `anthropic-version` (`dispatch::models_dialect`, see [model listings](codec-design.md#model-listings)).

- **Code:** `anthropic_messages/models.rs`
- **Fixtures:** `crates/kaeso-codecs/fixtures/anthropic_messages/models/`
- **Sources:** the [list models](https://platform.claude.com/docs/en/api/models/list.md) and [get a model](https://platform.claude.com/docs/en/api/models/retrieve.md) pages, the SDK's `types/model_info.py`, `model_list_params.py`, `model_capabilities.py` and `model_line.py`, and the spec's `ModelInfo` and `ListResponse_ModelInfo_`

The spec's shapes have no `lifecycle`, `deprecated_at`, `retires_at` or `line`, and no `lifecycle` query parameter. The documentation and the SDK win. Every field the encoder would otherwise fill with a default is decoded as optional, with its absence kept (form `none`), so both shapes round-trip.

### Query parameters

`decode_models_query` turns the query string into a `ModelsQuery`.

| Parameter | KPP | Notes |
|---|---|---|
| `limit` | `limit` | Digits only (no sign), 1 to 1000, once; anything else → `InvalidRequest` (param `limit`). When it is absent, the gateway passes the documented default of 20 to `ModelList::page`. |
| `after_id`, `before_id` | `after_id`, `before_id` | Non-empty, once each; both together → `InvalidRequest` (param `before_id`). An id that is not in the filtered, de-duplicated listing → `InvalidRequest` from `ModelList::page` (param `after_id` or `before_id`), rather than an empty page that would end an app's paging silently; a cursor filtered out by `lifecycle` counts as unknown. |
| `lifecycle[]` (what the SDK sends: `Querystring(array_format="brackets")`), `lifecycle` (repeated) | `lifecycle` | `active`, `deprecated`, `retired`; up to 3 values (`maxItems: 3`). Any other value, including a comma-separated `active,deprecated`, → `InvalidRequest` (param `lifecycle`). An empty list → the default of `ModelList::page` (`active` and `deprecated`). |
| any other parameter (such as `beta=true` from `client.beta.models`) | extras `query.<name>` | A string, or an array of strings when repeated. |

Names and values are percent-decoded (`+` is a space); a malformed escape or a result that is not UTF-8 → `InvalidRequest`. The `{model_id}` path segment also arrives percent-encoded; the gateway's router decodes it, since the codecs never see paths. Headers (`anthropic-beta`, `anthropic-workspace-id`) are not part of the query.

### Model info

`ModelInfo` on the wire and `kaeso_kpp::ModelInfo` in KPP.

| Wire field | KPP | Encoded when KPP has no value |
|---|---|---|
| `id` | `id` | (required) |
| `type` (`"model"`) | form `type`, only when it differs | `"model"` |
| `display_name` | `display_name` | the `id` |
| `created_at` (RFC 3339) | `created_at` (Unix seconds) | `"1970-01-01T00:00:00Z"`: the documentation says `created_at` "may be set to an epoch value if the release date is unknown" |
| `lifecycle` | `lifecycle` (`Other(s)` for undocumented values) | `"active"` (the documented default) |
| `deprecated_at`, `retires_at` (RFC 3339 or `null`) | `deprecated_at`, `retires_at` | `null` |
| `max_input_tokens`, `max_tokens` (or `null`) | `context_window`, `max_output_tokens` | `null` |
| `capabilities` (`ModelCapabilities` or `null`) | extras field `capabilities` | `null` |
| `line` (`ModelLine` or `null`) | extras field `line` | `null` |
| unknown keys | extras fields | |

`capabilities` and `line` are extras, not typed KPP fields. `ModelCapabilities` is a large, fast-growing tree of Anthropic feature names (batches, citations, effort levels, thinking types, server tools, context-management strategies) with no counterpart in any other dialect. `line` is an Anthropic product family (`opus`, `sonnet`, …), which the documentation says not to infer from the `id`. KPP would only carry them through for Anthropic apps, and extras already do that byte for byte.

**Dates.** An RFC 3339 `date-time` becomes Unix seconds (offsets applied, sub-second digits cut, `T` and `Z` in either case) and is written back as `YYYY-MM-DDTHH:MM:SSZ`. A received spelling this would not reproduce (`.117Z`, `-07:00`, `t`) is kept in form `spelling` and written again as long as it still means the same second. Dates that can't be parsed and dates before 1970 → `ProviderError`. The second `:60` is accepted in any minute; it counts as the first second of the next minute and its spelling is kept. A KPP time after `9999-12-31T23:59:59Z` can't be written in RFC 3339, which has four-digit years; this happens with an OpenAI-compatible `created` in milliseconds. `created_at` then gets the epoch, the documented unknown value, and `deprecated_at` and `retires_at` get `null`.

**Absent or `null`.** A field received absent, or `null` where the default above is a value, is recorded in form `none` and written the same way. This lets the older spec shape without `lifecycle` round-trip. `capabilities` and `line` absent on an Anthropic model stay absent.

### Model lists

`ListResponse[ModelInfo]` on the wire and `ModelList` in KPP.

| Wire field | KPP | Encoded |
|---|---|---|
| `data` | `models` | |
| `has_more` | `has_more` | `false` when not set (absent or `null` is kept in form `none`) |
| `first_id`, `last_id` | derived from `models` | The first and last model id, `null` for an empty page. A received value that differs from the derived one (the documentation's example sends the placeholders `"first_id"` and `"last_id"`) is kept in form `first_id` or `last_id` together with the id it was received for, and written again only while the page still starts or ends with that model. |
| unknown keys | extras fields | |

Lossy towards Anthropic apps: `owned_by` (OpenAI) has no field here, and extras from other dialects are left out ([replies](cross-format.md#replies)). A model from another dialect gets the defaults above (`display_name` is the id, `lifecycle: "active"`, `capabilities: null`, `line`, `max_*` and dates `null`).

On the egress side, these functions build requests and read replies:

- `encode_model_request` builds `GET /v1/models/{model_id}` with `anthropic-version: 2023-06-01` and no body. This request also resolves an alias such as `claude-sonnet-4-5` (the [get a model](https://platform.claude.com/docs/en/api/models/retrieve.md) page).
- `encode_models_request` builds `GET /v1/models` with the same header and the query's `limit`, `after_id`, `before_id` and `lifecycle[]`, each value percent-encoded. Any other parameter is `Unsupported`.
- `decode_models` and `decode_model` take a provider's reply. A status other than 2xx goes to `decode_error`.

Decoded extras carry the origin `anthropic_messages` without a provider, because these functions take no `EgressCtx`. The id `.` or `..` is `InvalidRequest`, because URL parsers drop such a segment even when it is percent-encoded.

## Round trips

Every fixture round-trips, with no exceptions: requests, token-counting requests, replies, token-count replies, model listings, streams and errors (`tests/`, `models_tests.rs`). Every stream fixture also accumulates to its paired reply after decode and encode.
