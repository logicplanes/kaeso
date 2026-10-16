---
title: OpenAI Responses
description: How every field of the OpenAI Responses format maps to KPP and back.
order: 6
---

OpenAI Responses, `POST /v1/responses` (streaming included) and `POST /v1/responses/input_tokens`. Codec: `kaeso_codecs::openai_responses::OpenAiResponses`.

Sources: the OpenAI OpenAPI spec at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (schema names below), the [create reference](https://developers.openai.com/api/reference/resources/responses/methods/create), the [streaming events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events), the [input token count reference](https://developers.openai.com/api/reference/resources/responses/subresources/input_tokens/methods/count) and the [error codes guide](https://developers.openai.com/api/docs/guides/error-codes).

The tables use these terms:

- **typed**: a typed KPP field.
- **extras**: `Extras::fields` of the named item, bound to `openai_responses`. They are same-dialect only: another dialect refuses them with `Unsupported` ([extras](cross-format.md#extras)). Nested keys use key paths (`text.verbosity`, `summary.0.x`).
- **form**: a codec-private `Extras::form` marker. It is presentation only, and other dialects ignore it.
- **Unsupported**: decoding or `check()` fails with `ErrorKind::Unsupported { feature }`.

## How exact spellings survive

Every wire object has a *canonical* encoding built only from typed KPP fields and the structural form markers listed under [form markers](#form-markers). Requests and replies from other dialects get exactly this encoding. When decoding, the codec builds the canonical encoding of what it decoded and records every difference from the received JSON as an override in the extras of the KPP item that owns the object:

- `fields["<path>"]`: a key the canonical encoding does not write, or writes with another value. These are unknown keys and known keys KPP does not model (`metadata`, `text.verbosity`, `detail: "low"`, …), kept byte for byte. They are **extras** in the tables below.
- `form["set"]["<path>"]`: the same for presentation. These are `null` values, explicitly spelled defaults (`store: false`, `background: false`, `stream: false`, `tools: []`, `include: []`, `metadata: {}`, `text: {}`, `detail: "auto"`, `truncation: "disabled"`, `service_tier: "auto"`, `text.format: {type: text}`, `text.verbosity: "medium"`, `stream_options: {}`, `stream_options.include_obfuscation`, empty `annotations`/`logprobs`/`summary`/`content`), and the keys `type`, `id`, `item_id`, `status`, `role`, `object`, `filename`, `obfuscation`, `sequence_number`. They are **form** in the tables below.
- `form["unset"]`: paths the canonical encoding writes that the original lacked.

Encoding applies the overrides only when the extras came from `openai_responses`. Typed fields always win. An override is never recorded for a value the canonical encoding reproduces, and a test checks that no typed key is ever stored as an extras field. Each override also remembers what the canonical encoding had at its path when decoded (`form["canon"]`). If a typed field changed after decoding (the gateway set `max_output_tokens`, added messages to a prompt-only request, set a reasoning effort, …), the canonical value at that path differs and the stale override is skipped.

A `type` is presentation only where the canonical encoding has a `type` at that path (part and item type spellings), and for the optional `type: "message"` of an input message. An object the canonical encoding lacks that carries any other `type` (for example `text.format: {type: "grammar", …}`) is kept whole as one extras field, so another dialect refuses it.

## Request: `CreateResponse` / `TokenCountsBody`

| Wire field | KPP | Notes |
| --- | --- | --- |
| `model` | `model` typed | Required for `/v1/responses`; optional for `/v1/responses/input_tokens` (missing: empty KPP `model`, left out again). |
| `input` (string) | one `Role::User` message with one text block, typed | Form `input: "string"`. Other dialects always get an item list. |
| `input` (items) | `system` + `messages` typed | See [input items](#input-items-inputitem). Absent `input` (allowed with `prompt`): no messages; form `unset input`. |
| `instructions` | `system[0]` text block typed | From other dialects, a single text block becomes `instructions`; several blocks become one leading `developer` message item with `input_text` parts, which keeps the block boundaries but not the difference between `instructions` and a system message (the API treats both as developer instructions). `null`: form. |
| `tools[]` `{type: function, name, description, parameters, strict}` (`FunctionTool`) | `ToolDef::Function` typed (`parameters` raw) | Other keys (`defer_loading`, `allowed_callers`, `output_schema`, …) are extras of the tool. Cache hints on tools from other dialects are dropped ([cache hints](cross-format.md#cache-hints)). Names from other dialects outside `^[a-zA-Z0-9_-]{1,64}$` are rewritten deterministically (other characters become `_`, cut to 55 characters, then `_` and 8 hexadecimal digits of an FNV-1a hash), recorded in `EgressCtx::tool_names` and mapped back on decode. |
| `tools[]` other types (`web_search`, `file_search`, `mcp`, `custom`, …) | `ToolDef::Unknown` | Same-dialect only. |
| `tool_choice` `"auto"` / `"none"` / `"required"` | `ToolChoice::Auto` / `None` / `Required` typed | |
| `tool_choice` `{type: function, name}` | `ToolChoice::Tool(name)` typed | With other keys: `ToolChoice::Unknown`. |
| `tool_choice` `allowed_tools`, hosted-tool, `mcp`, `custom`, other strings | `ToolChoice::Unknown` | Same-dialect only. |
| `parallel_tool_calls` | `parallel_tool_calls` typed | |
| `text.format` `{type: text}` | `output = Text` | Absent is the canonical form; an explicit `{type: text}` is form. |
| `text.format` `{type: json_object}` | `output = JsonObject` typed | |
| `text.format` `{type: json_schema, name, description, schema, strict}` | `output = JsonSchema` typed (`schema` raw) | Other keys: extras of the `JsonSchemaSpec`. A schema without a name (Anthropic) is sent as `name: "response"`. |
| `text.format` other types | extras `text.format` | |
| `text.verbosity` | `"medium"`, the default that GPT-5 and Codex clients send on every request: form, so such requests can go to other dialects. `low`/`high` change the output: extras `text.verbosity`, refused by other dialects ([extras](cross-format.md#extras)). | `null`: form. |
| `reasoning.effort` | `reasoning.effort` typed | From other dialects ([reasoning](cross-format.md#reasoning)): an explicit effort as is; Anthropic `thinking.disabled` → `none`; a budget → `low` (below 4,096), `medium` (4,096 to 16,383), `high` (16,384 and more), as in [effort and budget](cross-format.md#effort-and-budget). Values `none` to `max`; unknown values are `ReasoningEffort::Other`. |
| `reasoning.summary` | `reasoning.summary` typed | `auto`/`concise`/`detailed`/other. Anthropic Messages requests with thinking on and a `display` other than `omitted` get `summary: "auto"`, because OpenAI returns readable reasoning only when asked ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)). Not for `/v1/responses/input_tokens`. OpenAI requires some organisations to be verified before they get reasoning summaries; for such a provider key, OpenAI's error reaches the app unchanged. |
| `reasoning.mode` (`standard`/`pro`), `reasoning.context`, `reasoning.generate_summary` (deprecated) | extras of the `ReasoningConfig` | Not the same concept as KPP `ReasoningMode` (Anthropic thinking on or off). |
| `reasoning` `{}` | `reasoning` = empty config | Form `object`. |
| `include[]` `"reasoning.encrypted_content"` | `reasoning.include_encrypted` typed | Without a `reasoning` object: form `no_object` on the config. Requests from other dialects always ask for it when they send a reasoning effort other than `none` (after the effort mapping), and so does Anthropic `adaptive` or `enabled` thinking without an effort when `EgressCtx::reasoning = Some(true)` ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)). With `store: false` it is the only way to give the reasoning back on the next turn ([reasoning blobs](cross-format.md#reasoning-blobs)). |
| `include[]` other values | extras `include` (the whole list) | `message.output_text.logprobs`, `web_search_call.action.sources`, … |
| `max_output_tokens` | `sampling.max_output_tokens` typed | |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typed | |
| `safety_identifier` | `user` typed | KPP `user` from other dialects is sent as `safety_identifier`. |
| `user` (deprecated) | `user` typed when `safety_identifier` is absent (form `user_key`); otherwise extras `user` | |
| `stream: true` | `StreamMode::On { include_usage: true }` | Responses streams always report usage. `false`/`null`: form. |
| `stream_options` | `include_obfuscation`: form; other keys: extras | Obfuscation is padding, not content. |
| `store` | `false`/`null`: form; `true`: **Unsupported** (`store`, [stateful Responses features](cross-format.md#stateful-responses-features)) | Requests from other dialects are sent with `store: false` ([Responses storage and ids](cross-format.md#responses-storage-and-ids)). Requests of this dialect keep their spelling, so an absent `store` still means OpenAI stores the response. Form `native` marks a request decoded from this dialect. |
| `previous_response_id`, `conversation` | `null`: form; any value: **Unsupported** ([stateful Responses features](cross-format.md#stateful-responses-features)) | |
| `background` | `false`/`null`: form; `true`: **Unsupported** ([stateful Responses features](cross-format.md#stateful-responses-features)) | |
| `metadata`, `truncation`, `service_tier`, `top_logprobs`, `max_tool_calls`, `prompt_cache_key`, `prompt_cache_retention`, `prompt_cache_options`, `prompt`, `context_management`, `moderation`, `access_programs`, `personality` (count only), unknown keys | extras | Spelled-out defaults and `null` are form ([how exact spellings survive](#how-exact-spellings-survive)). |
| header `openai-beta` | extras `header:openai-beta` | Forwarded to Responses only. |

`check()` also refuses, for requests from any dialect: `sampling.top_k`, `seed`, `frequency_penalty`, `presence_penalty` and stop sequences, which the Responses API doesn't have ([sampling](cross-format.md#sampling)); unknown reasoning modes, named by their wire value; text documents, document `title`/`context`/`citations`, and text image sources; and everything `cross::check_foreign` refuses (foreign extras, unknown blocks, tools and choices, foreign file ids, foreign citations; see [requests](cross-format.md#requests)). Reasoning blocks that can't be replayed are dropped ([reasoning](cross-format.md#reasoning)), and so are cache hints from other dialects ([cache hints](cross-format.md#cache-hints)). `ToolResult::is_error` is dropped too: it is a hint, and the content describes the failure ([tool errors](cross-format.md#tool-errors)). A reasoning config from another dialect is sent as its effort: an explicit effort as is, a budget by the [effort table](cross-format.md#effort-and-budget), `disabled` as `none`; `adaptive` or `enabled` without either is sent without an effort, so the model uses its default effort. Embeddings requests are `Unsupported`.

### Input items (`InputItem`)

| Wire | KPP | Notes |
| --- | --- | --- |
| leading `system` / `developer` message items holding only text and no item data besides `type`/`role` | `GenerateRequest.system` blocks | Form `system_items` (`[{n, role, type, parts}]`) and `instructions` on the request record the split. A leading item with an `id`, a `status`, an image or an unknown key stays a `Role::System` message. |
| `message` item, role `user` / `assistant` | `Message { role: User / Assistant }` content blocks | `EasyInputMessage`, `InputMessage` and `OutputMessage` shapes. Item `type`, `id`, `status` and the `role` spelling: form on the message; `phase` and unknown keys: extras of the message. A KPP message holds at most one message item. |
| `message` item, role `system` / `developer` (not leading) | `Message { role: System }` ([system messages](cross-format.md#system-messages)) | Canonical role `developer`; `system`: form. |
| `content` string vs parts | form `parts` on the message | The string is canonical for one plain text block. Empty parts list: form `empty_at`. |
| `function_call` (`FunctionToolCall`) | `Block::ToolCall` in the current assistant message | `call_id` → `id`, `name`, `arguments` (a JSON string) → raw JSON. Arguments that aren't JSON are kept as a JSON string with form `args_string`, the KPP convention; a JSON-string `arguments` value from another dialect is sent as its text. `id`, `status`: form; `namespace`, `caller`, unknown keys: extras of the block. Call ids from other dialects longer than 64 characters are shortened like tool names (`EgressCtx::tool_ids`) and mapped back on decode. |
| `function_call_output` (`FunctionCallOutputItemParam`) | `Block::ToolResult` in the current user message | `call_id`; an `output` string → one text block, parts → text, image and document blocks (form `parts` when a string would do). `id`, `status`: form; `name`, `namespace`, unknown keys: extras. `is_error` from other dialects is dropped. Empty content from other dialects: `output: ""`. |
| `reasoning` (`ReasoningItem`) | `Block::Reasoning` (origin `openai_responses`) in the current assistant message | A Kaeso-invented id (`rs_kaeso_…`) is not kept (form `kaeso_id`); such an item without `encrypted_content` is not sent to the provider at all, since it would have neither an `id` nor content OpenAI can use. `id` → opaque `OpenAiReasoningItemId`, `encrypted_content` → opaque `OpenAiEncryptedReasoning`, `summary[].text` → `summary`, `content[].text` → `text`; `status` and part `type`s: form; unknown keys: extras. Replayed only to `openai_responses` with the same provider ([reasoning](cross-format.md#reasoning)). An item whose `encrypted_content` is a reasoning blob (`kpp:reasoning:v1:…`, another dialect's reasoning Kaeso gave the app, see [reasoning blobs](cross-format.md#reasoning-blobs)) decodes to the original block inside it, origin included (for example Claude's signed thinking), whatever the rest of the item says. In requests Kaeso sends with `store: false` (from other dialects, see [Responses storage and ids](cross-format.md#responses-storage-and-ids)), reasoning without encrypted content is not sent, because OpenAI can't resolve the item id. |
| `item_reference` and every other item type (server-tool calls and outputs, `compaction`, future types) | a message with one `Block::Unknown`, form `raw_item` | Role `Assistant` for types ending in `_call`, `mcp_list_tools`, `mcp_approval_request`; otherwise `User`. The role only matters to KPP validation, because these items never go to another dialect. Same-dialect only. |
| items that don't parse as their type (for example a `function_call_output` with `call_id: null`) | `Block::Unknown` (as above) | Lossless; the provider judges them. |

Grouping: consecutive `function_call` and `reasoning` items join the current assistant message, consecutive `function_call_output` items join the current user message, and a message item joins a message of its role that holds no message item yet; system and developer items never join. Encoding splits each KPP message back: every run of content blocks is one message item, and every other block is one item. A message from another dialect such as Anthropic's `[thinking, text, tool_use, tool_use]` becomes a `message` item and two `function_call` items. The thinking is dropped ([reasoning](cross-format.md#reasoning)), unless it is OpenAI reasoning an Anthropic app carried back in a [reasoning blob](cross-format.md#reasoning-blobs).

The `id` and `status` of message and function call items (`msg_…`, `fc_…`) are presentation, so a conversation with echoed output items can still go to another provider. Reasoning item ids are semantic (opaque `OpenAiReasoningItemId`), because OpenAI resolves them.

### Content parts

| Wire part | KPP | Notes |
| --- | --- | --- |
| `input_text` (user, system, developer, tool output) | `Block::Text` | |
| `output_text` (assistant) | `Block::Text`, `annotations[]` → `citations` (raw, origin-bound) | Canonical in requests: `{type, text, annotations}`; `logprobs` in input history: extras. The text type that isn't the role's default: form. |
| `refusal` | `Block::Refusal` | |
| `input_image` `image_url` (data URL `data:<type>;base64,…`) | `Block::Image`, `Source::Base64` | Data URLs with other parameters stay `Source::Url`. |
| `input_image` `image_url` (URL) / `file_id` | `Source::Url` / `Source::FileId` (origin-bound) | Both or neither set: `Block::Unknown`. An image file id works only with OpenAI Responses of the same provider, since OpenAI Chat Completions takes no image file ids. |
| `input_image.detail`, `input_file.detail` | `"auto"`: form; other values: extras `detail` | On images, `detail` is a resolution hint on the [extras allowlist](extras-allowlist.md), so it is dropped for other dialects. On files it controls how PDFs are rendered and other dialects refuse it. |
| `input_file` `file_data` / `file_url` / `file_id` | `Block::Document`, `Source::Base64` / `Url` / `FileId` | Exactly one source, and `file_data` must be a data URL; otherwise `Block::Unknown`. |
| `input_file.filename` | form | KPP has no field for a file name (`DocumentBlock::title` is a title), so PDFs can cross dialects. OpenAI requires a file name with `file_data`: documents from other dialects sent as `file_data` get `filename: "document.<ext>"`, with the extension `pdf`, `txt`, `md`, `csv` or `json` by media type and `bin` otherwise. |
| `prompt_cache_breakpoint` (`PromptCacheBreakpointConfig`) on a text, image or file part | `cache: CacheHint` of the block (form `breakpoint`) | Re-emitted only to this dialect; hints from other dialects are dropped ([cache hints](cross-format.md#cache-hints)). |
| other part types | `Block::Unknown` | |

## Response: `Response`

| Wire field | KPP | Notes |
| --- | --- | --- |
| `id`, `model` | `Response.id`, `.model` typed | |
| `status` `completed` | `stop = EndTurn`, or `ToolUse` when `output` holds a `function_call` | |
| `status` `incomplete` + `incomplete_details.reason` `max_output_tokens` / `content_filter` / other | `stop = MaxTokens` / `ContentFilter` / `Other(reason)` | Other statuses (`in_progress`, `cancelled`, `queued`): `Other(status)`; the exact `status` and `incomplete_details` come back from overrides. |
| `status` `failed` + `error {code, message}` | `decode_response` returns the error | Kind from `error.code` (`ResponseErrorCode`): `server_error` → `ProviderError`, `rate_limit_exceeded` → `RateLimited`, `vector_store_timeout` → `Timeout`, image and `invalid_prompt` codes → `InvalidRequest`, policy codes → `ContentFiltered`, unlisted codes → `ProviderError`. The whole object is kept in `raw` and replayed exactly to Responses apps. |
| `output[]` `message` (role `assistant`) | content blocks (`output_text` → `Text` with citations, `refusal` → `Refusal`, other parts → `Unknown`) | Item boundaries: form `messages` = `[[first block, block count], …]` and `blocks` (block count) on the response, plus form `item` (output index) on every decoded block. The ranges are used only while the block list is unchanged. If the gateway removed or added blocks, blocks are grouped by their `item` markers, so ids and item overrides (scoped `@out<i>`) stay with their item and no spurious item appears. `logprobs` of a part: extras of the block. |
| `output[]` `function_call` | `Block::ToolCall` | `id` (`fc_…`): form on the block; names and ids rewritten on the way out are mapped back. |
| `output[]` `reasoning` | `Block::Reasoning` | As for input items. |
| `output[]` other types (`web_search_call`, …) | `Block::Unknown` | |
| `usage` (`ResponseUsage`) | `usage` typed | As described under [usage](cross-format.md#usage): `input_tokens` = wire `input_tokens` − `cached_tokens` − `cache_write_tokens`, since wire `input_tokens` counts cache reads and cache writes; `cache_read_tokens` = `cached_tokens`; `cache_write_tokens`; `output_tokens` (includes reasoning); `reasoning_tokens`; the object in `raw`. Encoding writes the typed fields and overlays `raw` from this dialect. `null`: form. |
| `object`, `created_at`, `completed_at`, `error: null`, echoed request parameters (`instructions`, `tools`, `tool_choice`, `text`, `reasoning`, `temperature`, `top_p`, `store`, `truncation`, `metadata`, `service_tier`, …), unknown keys | extras of the response (overrides) | Form `native` marks a response decoded from this dialect. |

A reply from another dialect is written as a complete `Response` object:

- `object: "response"`, `created_at` (`EncodeCtx::created_at`), `background: false`, `completed_at` (the same time when completed, otherwise `null`) and `error: null`.
- The echoed request parameters come from the app's request (`EncodeCtx::request`, encoded the way this codec sends it), with OpenAI's defaults where unset: `instructions`, `max_output_tokens`, `max_tool_calls`, `prompt_cache_key`, `safety_identifier`, `user` and `previous_response_id` are `null`, `parallel_tool_calls: true`, `reasoning: {effort, summary}`, `service_tier: "default"`, `store: false` (Kaeso stores nothing), `temperature: 1`, `top_p: 1`, `top_logprobs: 0`, `text: {format: {type: text}}`, `tool_choice: "auto"`, `tools: []`, `truncation: "disabled"`, `metadata: {}`.
- Stop reasons without an equivalent (`StopSequence`, `Refusal`, `PauseTurn`, `Other`) are reported as `completed`, which loses information: an Anthropic `pause_turn` asks the app to continue, and a Responses app can't tell. Message items are `incomplete` when the response is.
- Reasoning of another dialect becomes a reasoning item ([reasoning blobs](cross-format.md#reasoning-blobs)): id `rs_kaeso_…`, its reasoning text as one `summary_text` followed by its summaries, and a reasoning blob of the original block as `encrypted_content`. The blob is sent whether or not the app asked for it in `include`, because Kaeso stores nothing and the SDK agent loops send output items back as received.
- Reply data this dialect can't show is left out ([replies](cross-format.md#replies)): foreign `Unknown` blocks, foreign citations and extras, images, documents and tool results.
- Message and function call items get the ids `msg_kaeso_…` and `fc_kaeso_…`. Requests recognise these Kaeso-invented ids and never send them to a provider.

## Input token count: `TokenCountsResource`

| Wire | KPP | Notes |
| --- | --- | --- |
| `{object: "response.input_tokens", input_tokens}` | `CountTokensResponse.input_tokens` typed | Other keys: extras. |

The request is `TokenCountsBody`, decoded like `CreateResponse` into `Request::CountTokens`; the path is `/v1/responses/input_tokens`. `include`, `max_output_tokens`, `temperature`, `top_p`, `user`, `safety_identifier`, `stream` and `store` are not part of `TokenCountsBody`. Sent by an app, they stay extras of the request (same-dialect only). KPP count requests from other dialects are encoded without them, since they don't change the input token count.

## Streams: `ResponseStreamEvent`

There is one KPP block per message content part, function call, reasoning item and other item. `sequence_number` and `obfuscation` deviations are form. Everything else that differs from the canonical events is an extras override scoped by the attached event (`@open`, `@close`, `@done`, `@in_progress`, `@sdone`, `@spdone`, `@rdone`, `@rpdone`, `@text`, `@e<index>`, `@a<index>-…`).

| SSE event | KPP |
| --- | --- |
| `response.created` | `ResponseStart` (`id`, `model`, `usage` when not null; form `native`) |
| `response.in_progress` | attached to `ResponseStart` |
| `response.output_item.added` (`message`) | attached to the first part's `BlockStart` (`@open`) |
| `response.content_part.added` (message: `output_text` / `refusal` / other) | `BlockStart` `Text` / `Refusal` / `Unknown` (form `part`; form `new_item` when it opens a message item while another is open) |
| `response.output_text.delta` | `BlockDelta` `Text`; `logprobs` extras |
| `response.output_text.annotation.added` | `BlockDelta` `Citation` |
| `response.refusal.delta` | `BlockDelta` `Refusal` |
| `response.output_text.done`, `response.refusal.done` | attached to the part's `BlockStop` (`@done`) |
| `response.content_part.done` (message) | `BlockStop` |
| `response.output_item.done` (`message`) | attached to the next lifecycle event (`@close`) |
| `response.output_item.added` (`function_call`) | `BlockStart` `ToolCall` (`call_id`, `name`) |
| `response.function_call_arguments.delta` | `BlockDelta` `ToolArguments` |
| `response.function_call_arguments.done` | attached to the `BlockStop` (`@done`) |
| `response.output_item.done` (`function_call`) | `BlockStop` |
| `response.output_item.added` (`reasoning`) | `BlockStart` `Reasoning` (item id as opaque `OpenAiReasoningItemId`) |
| `response.reasoning_summary_part.added` | `BlockDelta` `ReasoningSummary` with empty text (opens summary `summary_index`) |
| `response.reasoning_summary_text.delta` | `BlockDelta` `ReasoningSummary` |
| `response.reasoning_summary_text.done`, `response.reasoning_summary_part.done` | attached to the next summary part or the `BlockStop` (`@sdone`, `@spdone`) |
| `response.content_part.added` (reasoning, `reasoning_text`) | `BlockDelta` `ReasoningText` with empty text, form `new_part` |
| `response.reasoning_text.delta` | `BlockDelta` `ReasoningText` |
| `response.reasoning_text.done`, `response.content_part.done` (reasoning) | attached to the next part or the `BlockStop` (`@rdone`, `@rpdone`) |
| `response.output_item.done` (`reasoning`) | `BlockDelta` `Opaque` (`encrypted_content`, if any) + `BlockStop` |
| `response.output_item.added` / `.done` (other item types) | `BlockStart` `Unknown` / `BlockDelta` `Unknown` (the final item) + `BlockStop` |
| `response.completed`, `response.incomplete` | `ResponseDelta` (`stop`, `usage`) + `ResponseStop` (overrides of the final response object, for example `completed_at`) |
| `response.failed` | `StreamEvent::Error` (kind from `error.code`; the event in `raw`, replayed as is) |
| `error` (`ResponseErrorEvent`, flat `code`/`message`/`param`) | `StreamEvent::Error` (kind from `code`; the event in `raw`, replayed as is). An event without a flat `code`, such as one with a nested `error` object, becomes a `ProviderError`, one without a flat `message` gets a generic message, and both are replayed from `raw`. |
| no `response.created` before the content | a synthesized `ResponseStart` (empty id and model, form `skip` so it is never written back; the final response object still carries the real id) |
| a known event for an output item or part the stream never announced, or without its required fields | `StreamEvent::Error` (`ProviderError`); nothing more is decoded |
| `error` immediately followed by `response.failed` | one `StreamEvent::Error` (both events in `raw`, both replayed) |
| anything after `response.completed`/`.incomplete`, `response.failed` or `error` | not decoded |
| everything else (`response.queued`, `response.web_search_call.*`, `response.file_search_call.*`, `response.mcp_*`, `response.image_generation_call.*`, `response.audio.*`, future events), SSE comments, frames with `id:`/`retry:` fields, empty data or JSON data without a `type` | `StreamEvent::Extra` (`{event, data}` / `{comment}`), same-dialect only |
| data that is not JSON | `StreamEvent::Error` (`ProviderError`); nothing more is decoded |
| redundant events in an unusual place | `StreamEvent::Extra`, and the lifecycle event that would have produced them gets form `skip` |

Encoding KPP events from another dialect produces the full canonical sequence:

- `response.created` and `response.in_progress`, each with the response object with echo fields and `usage: null`;
- `output_item.added` and `content_part.added` before the first delta of a message item;
- the `.done` events and `content_part.done` at each `BlockStop`;
- `output_item.done` for a message when the next non-text block starts or the response ends;
- `reasoning_summary_*` part events around summaries, and `content_part.*` and `reasoning_text.*` around reasoning text;
- then `response.completed`, or `response.incomplete` for `MaxTokens` and `ContentFilter`, with every output item and the usage.

`sequence_number` counts from 0. Item ids are generated (`msg_kaeso_…`, `fc_kaeso_…`, `rs_kaeso_…` from the response id and output index). `obfuscation` is never generated. Blocks still open at `ResponseStop` are closed first. A stream without `ResponseStart` gets one, with the id `resp_kaeso_<created_at>` from `EncodeCtx::created_at`. Reasoning blocks of other dialects become reasoning items with an `rs_kaeso_…` id ([reasoning blobs](cross-format.md#reasoning-blobs)): the reasoning text streams as `reasoning_summary_text` of summary 0 (summaries keep their index), and `output_item.done` carries the reasoning blob of the whole block in `encrypted_content`. After `ResponseStop` or `Error` the encoder refuses further lifecycle events. Foreign errors become an `error` event, with the `code` from the error kind (see [errors](#errors-errorresponse)).

What another dialect sends that this dialect can't show (extra events such as Anthropic `ping`, foreign unknown blocks) earns paced SSE comments, `: keepalive` ([keepalives](cross-format.md#keepalives)): one per extra event, and one per 1,024 bytes of hidden text. `StreamEncoder::keepalive` gives the gateway the same frame. There is no end-of-stream sentinel.

## Spec properties without a row above

Every remaining property of the covered schemas, by name:

| Property | KPP target |
| --- | --- |
| `FunctionTool.async` | extras of the `FunctionTool` (`async`) |
| `FunctionTool.defer_loading`, `.allowed_callers`, `.output_schema` | extras of the `FunctionTool` |
| `FunctionToolCall.async`, `.namespace`, `.caller` (input and output items) | extras of the `ToolCallBlock` (`async`, …) |
| `FunctionToolCallOutputResource.created_by` (and `id`, `status`) | `created_by`: extras of the `ToolResultBlock`; `id`, `status`: form |
| `Response.prompt_cache_diagnostics` | extras of the `Response` (`prompt_cache_diagnostics`, the whole object) |
| `Response.prompt_cache_options`, `.moderation`, `.conversation`, `.output_text` | extras of the `Response` |
| `ResponseUsage.input_tokens` | `usage.input_tokens` + `usage.cache_read_tokens` + `usage.cache_write_tokens` ([usage](cross-format.md#usage)) |
| `ResponseUsage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens` |
| `ResponseUsage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens` |
| `ResponseUsage.output_tokens` | `usage.output_tokens` |
| `ResponseUsage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens` |
| `ResponseUsage.total_tokens` | derived (input + output); the original object is in `usage.raw` and wins for this dialect |
| stream events' `output_index` | the KPP block of the item (decoder state; message parts by `output_index` + `content_index`); written from the encoder's own counter, deviations are `fields` overrides of the event |
| stream events' `content_index` | the KPP block of the content part (message parts) or the reasoning text part (`new_part` deltas); written from the encoder's counter |
| stream events' `item_id` | the item id (form overrides for ids of this dialect, generated ids otherwise) |
| stream events' `sequence_number` | form overrides when they deviate from 0, 1, 2, … |
| stream events' `obfuscation` | form override on the delta |
| stream events' `logprobs` (`output_text.delta`/`.done`) | `fields` override on the delta / stop event |
| `prompt.id`, `prompt.version`, `prompt.variables` (`Prompt`) | extras `prompt` (the whole object; reusable prompts are stored server-side) |
| `prompt_cache_options.ttl`, `prompt_cache_options.mode` | extras `prompt_cache_options` (the whole object) |
| `context_management[].type`, `context_management[].compact_threshold` | extras `context_management` (the whole list) |
| `Error.misalignment` (`MisalignmentErrorDetailsResource`: `review_target`, `error_type`, `detailed_explanation`, `steer`) | `KppError.raw` (the whole body), replayed to apps of an OpenAI dialect |
| `ResponseError.misalignment` (failed responses, `response.failed`) | `KppError.raw` (the whole response or event), replayed to Responses apps |
| `ResponseError.code` / `.message` | `KppError.kind` (via `ResponseErrorCode`) / `.message` |
| `Error.type`, `.code`, `.param`, `.message` | `KppError.kind` (with the status), `.param`, `.message`; the body in `raw` |

## Errors: `ErrorResponse`

`{"error": {"message", "type", "param", "code"}}` ⇄ `KppError` (`message`, `param`, `status`, `retry_after` from `retry-after-ms` / `retry-after`, the body in `raw`).

| Decode: status / `type` / `code` | `ErrorKind` |
| --- | --- |
| `code` `context_length_exceeded` | `ContextTooLong` |
| `type`/`code` `insufficient_quota`, `credit_balance_exhausted`, `organization_spend_limit_exceeded`, `project_spend_limit_exceeded`, `organization_usage_limit_exceeded`, `billing_hard_limit_reached`, `billing_not_active` | `Billing` |
| 503, `service_unavailable_error`, `server_is_overloaded` | `Overloaded` |
| `content_policy_violation`, `content_filter`, `image_content_policy_violation`, `bio_policy`, `misalignment_policy_violation` | `ContentFiltered` |
| 400, 409, 422 | `InvalidRequest` |
| 401 | `Authentication` |
| 403 | `PermissionDenied` |
| 404 | `NotFound` |
| 413 | `RequestTooLarge` |
| 429 | `RateLimited` |
| 408, 504, `vector_store_timeout` | `Timeout` |
| other statuses | `ProviderError` |
| a body that is not an OpenAI error, at any status (such as a proxy's HTML 429) | `ProviderError` (status and raw body kept) |

The `type` and `code` rows win over the status. Other codes (`rate_limit_exceeded`, `server_error`, `invalid_prompt`, the image codes) don't decide here; the status does. They decide only where there is no status: in a stream `error` event and in the `error.code` of a failed response.

Encoding replays the body and status of errors that came from an OpenAI dialect (Chat, Responses, Embeddings) while they are unchanged, that is while the kind decoded again from the body equals the error's kind and the message is the same. Other errors get:

| `ErrorKind` | Status | `type` | `code` |
| --- | --- | --- | --- |
| `InvalidRequest`, `Unsupported` | 400 | `invalid_request_error` | `null` |
| `ContextTooLong` | 400 | `invalid_request_error` | `context_length_exceeded` |
| `ContentFiltered` | 400 | `invalid_request_error` | `content_policy_violation` |
| `Authentication` | 401 | `invalid_request_error` | `null` |
| `PermissionDenied` | 403 | `request_forbidden` | `null` |
| `NotFound` | 404 | `invalid_request_error` | `null` |
| `RequestTooLarge` | 413 | `invalid_request_error` | `null` |
| `RateLimited` | 429 | `rate_limit_error` | `rate_limit_exceeded` |
| `Billing` | 429 | `insufficient_quota` | `insufficient_quota` |
| `Overloaded` | 503 | `service_unavailable_error` | `server_is_overloaded` |
| `Timeout` | 504 | `server_error` | `timeout` |
| `ProviderError` | its 5xx status other than 503, otherwise 500 | `server_error` | `null` |

`retry_after` is written as `retry-after` (whole seconds, rounded up), plus `retry-after-ms` when it is not a whole number of seconds. In streams, a foreign error is `{"type": "error", "code": <code, or type when null>, "message", "param"}`.

## Form markers

| Marker | On | Meaning |
| --- | --- | --- |
| `set`, `unset` | any item | Presentation overrides ([how exact spellings survive](#how-exact-spellings-survive)). |
| `input` | request | `input` was a string. |
| `system_items`, `instructions` | request | How `system` splits into `instructions` and leading system and developer items. |
| `user_key` | request | The end-user id came from `user`. |
| `object`, `no_object` | reasoning config | The `reasoning` object was empty / absent. |
| `parts` | message, tool result | Content was a part list although a string would do. |
| `empty_at` | message | Block index of an empty message item. |
| `raw_item` | message | The message is one unknown item. |
| `args_string` | tool call | `arguments` was not JSON; the raw value is a JSON string. |
| `breakpoint` | cache hint | It was an explicit `prompt_cache_breakpoint`. |
| `native` | request | Decoded from this dialect (no `store: false` is added). |
| `canon` | any item | Canonical values at override paths when decoded, for the stale-override check. |
| `native`, `messages`, `blocks` | response | Decoded from this dialect; message item block ranges; block count. |
| `item` | response block | Index of the output item the block came from. |
| `kaeso_id` | reasoning block | Its Kaeso-invented item id was dropped. |
| `new_item`, `part`, `new_part`, `skip`, `~event` | stream events | See [streams](#streams-responsestreamevent). |

## Models

`GET /v1/models` and `GET /v1/models/{model}` use the same shape in every OpenAI API. This codec encodes and decodes it with the shared implementation described on the [OpenAI Chat Completions](openai-chat.md) page (fixtures `fixtures/openai_chat/models/`). Decoded extras carry the origin `openai_responses`.
