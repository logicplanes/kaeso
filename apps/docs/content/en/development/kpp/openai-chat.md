---
title: OpenAI Chat Completions
description: How every field of the OpenAI Chat Completions format maps to KPP and back.
order: 5
---

OpenAI Chat Completions, `POST /v1/chat/completions`, streaming included. Codec: `kaeso_codecs::openai_chat::OpenAiChat`.

Sources: the OpenAI OpenAPI spec at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (schema names below), the [create reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create), the [streaming events reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events) and the [error codes guide](https://developers.openai.com/api/docs/guides/error-codes).

The tables use these terms:

- **typed**: a typed KPP field.
- **extras**: `Extras::fields` of the named item, bound to `openai_chat`. They stay within this dialect; another dialect refuses them with `Unsupported` ([extras](cross-format.md#extras)). Nested keys use key paths (`image_url.detail`).
- **form**: a codec-private `Extras::form` marker. It is presentation only, and other dialects ignore it. `verbatim` markers keep `null` and `[]` spellings of declared keys, and of undeclared keys whose value is `null`.
- **Unsupported**: decoding fails with `ErrorKind::Unsupported { feature }`.

Typed numeric fields go through `u32`, `u64`, `i64` (`seed`) or `f64`. A value outside their range fails to decode, as `InvalidRequest` in a request and `ProviderError` in a reply, rather than being changed. Unknown fields are kept byte for byte.

## Request: `CreateChatCompletionRequest`

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `GenerateRequest.model` typed | |
| `messages` | `system` + `messages` typed | See [messages](#messages-chatcompletionrequestmessage). |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typed | |
| `max_completion_tokens` | `sampling.max_output_tokens` typed | The spelling written for requests from every source dialect. |
| `max_tokens` (deprecated) | `sampling.max_output_tokens` typed | Form `max_tokens: "max_tokens"`. Both present with the same value: form `"both"`. With different values `max_completion_tokens` wins and `max_tokens` becomes the extra `max_tokens (differs from max_completion_tokens)`, sent back as `max_tokens`; the key names the conflict when another dialect refuses it. |
| `stop` | `sampling.stop` typed | String spelling: form `stop: "string"`; `[]`: form `"array"`. More than four sequences: `Unsupported` (`stop`). |
| `seed` | `sampling.seed` typed | |
| `frequency_penalty`, `presence_penalty` | `sampling.*` typed | |
| `n` | form `n` when `1` | `n > 1`: `Unsupported` (`n`, see [sampling](cross-format.md#sampling)); `0`: `InvalidRequest`. |
| `reasoning_effort` | `reasoning.effort` typed | From other dialects ([effort and budget](cross-format.md#effort-and-budget)): an explicit effort as is; Anthropic `thinking.disabled` becomes `none`; a budget becomes `low` (below 4,096), `medium` (4,096 to 16,383) or `high` (16,384 and more). Values `none` to `max`; unknown values are kept as `ReasoningEffort::Other` and passed through for Chat apps, while for requests from other dialects `check()` refuses values outside the spec enum (`reasoning.effort`). |
| `response_format` `{type: text}` | `output = Text` typed | Form `response_format_text`; unknown keys are extras `response_format.<key>`. |
| `response_format` `{type: json_object}` | `output = JsonObject` typed | Unknown keys are extras `response_format.<key>`. |
| `response_format` `{type: json_schema, json_schema: {name, description, schema, strict}}` | `output = JsonSchema` typed | Unknown keys are extras (`json_schema.<key>` for nested ones). Missing `schema`: `{}` + form `schema_absent`; missing `json_schema` object: `InvalidRequest`. A schema from another dialect without a name is sent as `name: "response"` ([structured output](cross-format.md#structured-output)). |
| `response_format` other types | extras `response_format` | Same dialect only. |
| `tools[]` `{type: function, function: FunctionObject}` | `ToolDef::Function` typed (`name`, `description`, `parameters` raw, `strict`) | Unknown keys are extras (`function.<key>` nested). Names outside `[a-zA-Z0-9_-]{1,64}` from other dialects are rewritten deterministically (`EgressCtx::tool_names`). |
| `tools[]` other types (`custom`) | `ToolDef::Unknown` | Same dialect only. |
| `tool_choice` `"auto"`, `"none"`, `"required"` | `ToolChoice::Auto`, `None`, `Required` typed | |
| `tool_choice` `{type: function, function: {name}}` | `ToolChoice::Tool(name)` typed | With unknown keys: `ToolChoice::Unknown`. |
| `tool_choice` `allowed_tools`, `custom`, other strings | `ToolChoice::Unknown` | Same dialect only. |
| `parallel_tool_calls` | `parallel_tool_calls` typed | |
| `safety_identifier` | `user` typed | KPP `user` is the end-user id for abuse monitoring, which is what `safety_identifier` is. |
| `user` (deprecated) | `user` typed when `safety_identifier` is absent (form `user`); otherwise the extra `user (sent alongside safety_identifier)`, sent back as `user` | KPP `user` from other dialects is sent as `safety_identifier`. |
| `stream` | `stream` typed | `false`: form `stream_false`. |
| `stream_options.include_usage` | `StreamMode::On { include_usage }` typed | `stream_options` absent: form `stream_options: "absent"`; without `include_usage`: form `"no_include_usage"`; other keys (`include_obfuscation`) are extras `stream_options.<key>`; with `stream` off: the whole object as form `stream_options_off`. Requests from other dialects always ask for `include_usage: true`. The gateway needs usage, so it sets `include_usage: true` on every streaming request and keeps the app's own request in `EncodeCtx`; the app's stream encoder then hides the usage chunk from apps that did not ask for it. |
| `audio`, `functions` and `function_call` (deprecated, top level), `logit_bias`, `logprobs`, `metadata`, `modalities`, `moderation`, `prediction`, `prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `service_tier`, `store`, `top_logprobs`, `verbosity`, `web_search_options` | extras | No KPP equivalent; refused across dialects ([extras](cross-format.md#extras), [sampling](cross-format.md#sampling)), except `prompt_cache_key`, which is on the [extras allowlist](extras-allowlist.md) and dropped. `null` values are form. |
| unknown keys | extras | |
| header `openai-beta` | extras `header:openai-beta` | Forwarded to Chat only. |

### Messages (`ChatCompletionRequestMessage`)

| Wire | KPP | Notes |
|---|---|---|
| leading `system` and `developer` messages | `GenerateRequest.system` blocks | The first block of each message carries form `message: {role, content}`, which keeps message boundaries, the role and the string or array spelling; the message's `name` and unknown keys are extras `message.<key>` on that block. A leading message whose first part is unknown stays a `Role::System` message and ends the leading run. From other dialects: one `system` message. |
| later `system` and `developer` messages | `Message { role: System }` ([system messages](cross-format.md#system-messages)) | `developer`: form `role: "developer"`. |
| `user` | `Message { role: User }` | `name` and unknown keys: message extras. |
| `assistant` | `Message { role: Assistant }`: content parts, then `refusal`, then `tool_calls` | `content` absent: form `content: "absent"`; `null`: verbatim; `name`, `audio`: extras. `annotations` is not in the request schema, but the SDKs send it back with every reply: `[]` is verbatim (form), so the conversation can move to another provider; a non-empty value becomes `TextBlock.citations` of the first text block, bound to Chat (without text: extras `annotations`), which other dialects refuse (`citations`) and Chat sends back as `annotations`. From other dialects, text blocks go to `content`, the first refusal to `refusal` and tool calls to `tool_calls`, because Chat cannot interleave them; reasoning is dropped ([reasoning](cross-format.md#reasoning)); a turn left with nothing Chat can show, such as reasoning only, is sent as `content: ""`, so the conversation keeps its turns and Chat's rule that an assistant message needs content unless it has `tool_calls` is met. |
| assistant `refusal` | `Block::Refusal` | |
| assistant `function_call` (deprecated) | `Unsupported` (`function_call`) | `null` is kept verbatim. |
| `tool` messages | `Block::ToolResult` in a `Role::User` message; consecutive tool messages share one KPP message | `tool_call_id` becomes `call_id`; content parts become nested text blocks; unknown keys are extras of the tool result. `is_error` from other dialects is dropped, because Chat reports errors in the content ([tool errors](cross-format.md#tool-errors)). From other dialects, the tool results of a user message become `tool` messages, followed by a `user` message with the remaining blocks. |
| `function` messages (deprecated) | `Unsupported` (`function role message`) | |
| `content` as string or array | form `content: "array"` (message or tool result) | A string is the default for one plain text block. Empty arrays are invalid (`minItems: 1`). |

Legacy function calling has no tool call ids, so `function` messages and a non-null assistant `function_call` cannot become KPP tool calls and results. The top-level `functions` and `function_call` parameters travel as extras within this dialect. The fixture is `fixtures/openai_chat/requests/legacy-functions.json`.

### Content parts

| Wire | KPP | Notes |
|---|---|---|
| `text` | `Block::Text` | Unknown keys: block extras. |
| `prompt_cache_breakpoint` (text, image and file parts) | `cache: CacheHint { ttl: None }` bound to Chat | Both mark the end of a reusable prefix; the TTL comes from `prompt_cache_options`. Only hints from Chat are sent back; hints from other dialects are dropped ([cache hints](cross-format.md#cache-hints)). `mode` other than `explicit`: form `mode`. |
| `image_url.url` (`https:`) | `Block::Image { source: Url }` | |
| `image_url.url` (`data:<type>[;params];base64,<data>`, RFC 2397, scheme and `base64` in any case) | `Block::Image { source: Base64 }` | A header other than `data:<type>;base64`, such as parameters (`;name=a.png`) or upper case, is kept exactly in form `data_url`, also for `file.file_data`. Data URLs that are not base64 or have no media type stay `Url`. |
| `image_url.detail` | extras `image_url.detail` | On the [extras allowlist](extras-allowlist.md): dropped silently for other dialects. |
| `file.file_data` (`data:` URL) | `Block::Document { source: Base64 }` | Bare base64 is taken as media type `application/pdf`, the only type in OpenAI's file inputs guide, with form `bare_file_data`, and is written back bare. |
| `file.file_id` | `Block::Document { source: FileId }` bound to the OpenAI file store ([files](cross-format.md#files)) | With `file_data` too: extras `file.file_id`. |
| `file.filename` | extras `file.filename` | The file name OpenAI requires next to inline `file_data`, not a title shown to the model, so it never becomes `DocumentBlock::title`. On the [extras allowlist](extras-allowlist.md). |
| `input_audio` | `Unsupported` (`input_audio`) | Audio input is not part of KPP. |
| `refusal` (assistant) | `Block::Refusal` + form `part` | |
| other types, and `image_url` and `file` parts outside user messages, `refusal` parts outside assistant messages | `Block::Unknown` | Same dialect only. |

For KPP from other dialects, `check()` refuses text documents, document URLs, document `title`, `context` and `citations`, image file ids, images and documents outside user messages, refusals outside assistant messages, citations outside assistant messages (and, through `check_foreign`, other dialects' citations), `top_k` ([sampling](cross-format.md#sampling)), unknown reasoning modes and reasoning efforts outside the spec enum. Budgets and `disabled` are converted ([effort and budget](cross-format.md#effort-and-budget)). These hints are dropped instead: reasoning blocks ([reasoning](cross-format.md#reasoning)), other dialects' cache hints ([cache hints](cross-format.md#cache-hints)), `ToolResult::is_error`, whose information the error content carries ([tool errors](cross-format.md#tool-errors)), and other dialects' `reasoning.summary` and `reasoning.include_encrypted`, because Chat returns no reasoning to show ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)).

A body the codec cannot read is `InvalidRequest` naming the field and what it must be (`` `max_completion_tokens` must be a non-negative integer ``, with `param` set), never a parser position.

### Tool calls (`ChatCompletionMessageToolCall`)

| Wire | KPP | Notes |
|---|---|---|
| `id`, `function.name` | `ToolCallBlock.id`, `.name` | Rewritten names are mapped back on decode. |
| `function.arguments` | `ToolCallBlock.arguments` raw | The JSON text inside the string, byte for byte. Text that is not valid JSON, or has surrounding whitespace, keeps its literal in form `arguments`; KPP then holds `{}` for blank text and otherwise the text as a JSON string. KPP's convention is that a JSON string in `ToolCallBlock.arguments` means the provider sent text that is not JSON, so for calls from other dialects the string's content is sent as `arguments`; objects, arrays and other JSON values are sent as their JSON text. |
| unknown keys | extras (`function.<key>` nested) | |
| `type: custom` | `Block::Unknown` | Sent back in `tool_calls`. |

## Response: `CreateChatCompletionResponse`

| Wire field | KPP | Notes |
|---|---|---|
| `id`, `model` | `Response.id`, `.model` | |
| `object` | none | Always `chat.completion`; other values: form `object`. |
| `created`, `service_tier`, `system_fingerprint`, `metadata`, `moderation` | extras | A reply from another dialect gets `created` from `EncodeCtx::created_at`. |
| `choices` | `choices[0]` | More than one choice: `Unsupported` (`n`); none: `ProviderError`; an `index` other than 0: extras `choice.index`. |
| `choices[0].logprobs` | extras `choice.logprobs` | `null`: verbatim. |
| `choices[0].finish_reason` | `Response.stop` | `stop` ⇄ `EndTurn`, `length` ⇄ `MaxTokens`, `tool_calls` ⇄ `ToolUse`, `content_filter` ⇄ `ContentFilter`, other values (such as `function_call`) ⇄ `Other`. From other dialects: `StopSequence` and `PauseTurn` become `stop`; `Refusal` becomes `content_filter`, because the answer was withheld. A non-streaming response without `finish_reason`, which the spec requires: `ProviderError`. |
| `message.content` | `Block::Text` | From other dialects all text blocks are joined without a separator, because Chat has no content parts in responses. There is no separator because Anthropic splits one answer into several text blocks around citations, and the streamed deltas, which carry no separator either, must add up to the same `content`. |
| `message.annotations` | `TextBlock.citations` (raw, bound to Chat) | Without content: extras `choice.message.annotations`. Citations of other dialects are left out ([replies](cross-format.md#replies)). |
| `message.refusal` | `Block::Refusal` | |
| `message.tool_calls` | `Block::ToolCall` or `Block::Unknown` | As in requests. |
| `message.audio` (`id`, `data`, `expires_at`, `transcript`), `message.function_call`, unknown keys | extras `choice.message.<key>` | The audio object, with its `transcript` and `expires_at`, is kept whole. |
| reasoning, unknown blocks of other dialects | none | Left out of a Chat reply ([replies](cross-format.md#replies)). The gateway keeps replayable reasoning for the next turn, keyed by the tool calls it came before ([reasoning kept by the gateway](cross-format.md#reasoning-kept-by-the-gateway), `kaeso_codecs::continuity`). |

A reply from another dialect spells empty values as OpenAI does: `content: null`, `refusal: null`, `annotations: []` and `logprobs: null`.

### Usage (`CompletionUsage`)

The KPP fields follow the [usage](cross-format.md#usage) convention.

| Wire | KPP |
|---|---|
| `prompt_tokens − cached_tokens − cache_write_tokens` | `input_tokens` |
| `prompt_tokens_details.cached_tokens` | `cache_read_tokens` |
| `prompt_tokens_details.cache_write_tokens` | `cache_write_tokens` |
| `completion_tokens` | `output_tokens` (includes reasoning) |
| `completion_tokens_details.reasoning_tokens` | `reasoning_tokens` |
| `total_tokens` | derived (`prompt_tokens + completion_tokens`); not stored separately, kept in `raw` |
| `prompt_tokens_details.audio_tokens`, `text_tokens`, `image_tokens`, `completion_tokens_details.audio_tokens`, `text_tokens`, `accepted_prediction_tokens`, `rejected_prediction_tokens` | `raw` only (breakdowns of the typed totals) |
| the whole object | `raw`, sent back unchanged while the typed fields still match it; otherwise the typed fields win and the raw object adds the other keys |

From other dialects: `prompt_tokens = input + cache_read + cache_write`, `prompt_tokens_details.cached_tokens = cache_read`, plus `cache_write_tokens` when it is not zero, and `completion_tokens_details.reasoning_tokens` when KPP has a reasoning count.

## Streams: `CreateChatCompletionStreamResponse`

Bare `data:` frames, one JSON chunk each, ending with `data: [DONE]`.

| Wire | KPP event | Notes |
|---|---|---|
| first chunk | `ResponseStart { id, model }` | `created`, `service_tier`, `system_fingerprint`: extras; `object` other than `chat.completion.chunk`: form `object`. |
| `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint` on later chunks | none | The envelope repeats; differences in `created`, `service_tier` and `system_fingerprint` are kept in `chunk.env` and `chunk.env_absent`. `id`, `object` and `model` always come from `ResponseStart`. |
| `choices[0].delta.role` | form `chunk.role` | From other dialects the stream opens with a role chunk as soon as the response starts, `delta: {"role": "assistant", "content": "", "refusal": null}`, OpenAI's own first chunk of a text reply, so the app hears from the stream while the model thinks ([keepalives](cross-format.md#keepalives)). |
| `choices[0].delta.content` | `BlockStart(Text)` once, then `BlockDelta(Text)` | Every piece, including `""`. |
| `choices[0].delta.refusal` | `BlockStart(Refusal)` once, then `BlockDelta(Refusal)` | |
| `choices[0].delta.tool_calls[]` with a new `index` | `BlockStart(ToolCall { id, name })` + `BlockDelta(ToolArguments)` for `arguments` | KPP block indexes follow the order blocks start; the Chat `index` maps to the KPP index. Rewritten names are mapped back. Shape differences (a missing `id`, `type`, `function`, `name` or `arguments`, an unexpected `index`): form `tc`. Unknown keys: extras `tool_call.<key>` and `tool_call.function.<key>`. A `type` other than `function`: `BlockStart(Unknown)`. |
| `choices[0].delta.tool_calls[]` with a known `index` | `BlockDelta(ToolArguments)` | A repeated `id`, `type` or `name`: form `tc`. |
| `choices[0].finish_reason` | `BlockStop` for every open block, then `ResponseDelta { stop }` | Mapped as for responses. |
| `usage` (chunk with `choices: []`) | `ResponseDelta { usage }` | Usage as in [usage](#usage-completionusage), raw kept. `choices` empty or absent: form `chunk.ch`. |
| `usage: null`, `logprobs: null`, `finish_reason: null`, `content: null`, `refusal: null` | form `chunk.kv`, `cv`, `dv` | |
| `choices[0].logprobs`, unknown choice keys | extras `choice.<key>` on the chunk's first event | |
| `delta.audio`, `delta.function_call`, unknown delta keys | extras `delta.<key>` | A chunk with no typed content at all is an `Extra` (`{"event": null, "data": <chunk>}`). |
| `obfuscation` | form `chunk.obf` | Not sent to apps when the events came from another dialect. |
| `moderation`, unknown chunk keys | extras on the chunk's first event | |
| `data: [DONE]` | `ResponseStop` (after the `BlockStop`s of blocks still open) | A stream that ends after `finish_reason` without `[DONE]`: `ResponseStop` with form `no_done`; one that ends earlier: `Error`. |
| `data: {"error": {…}}` | `Error`, decoded like an error body without a status | The decoder stops. Encoded as the same frame; no `[DONE]` follows. |
| `event: error` frames, which some OpenAI-compatible providers send | `Error` | Decoded like an error body. |
| SSE comments, other named events, frames after `[DONE]` | `Extra` (`{"comment"}` or `{"event", "data", "id", "retry", "fields"}`) | Same dialect only. |
| several choices | `Error` with `Unsupported` (`n`) | |

Chunk grouping: the first event of each chunk carries the form marker `chunk` (`n` is the number of events from that chunk, plus the shape markers above) and the others carry `in_chunk`, so the encoder writes them back as one frame. An open chunk is written as soon as an event arrives that is not part of it (another dialect's event, `[DONE]`, an error), so a dropped event never puts a chunk after `[DONE]`; no chunk is written after `[DONE]` or an error. A gateway that rewrites `id` or `model` on `ResponseStart` rewrites every chunk. Tool arguments for a block Chat cannot show are left out.

A data chunk that has SSE `id` or `retry` lines, which OpenAI does not send, is decoded like any other chunk, and the lines are written back to Chat apps. Named events, unknown SSE fields and frames after `[DONE]` are kept whole and replayed only to Chat apps.

OpenAI sends a tool call's `id`, `type` and `function.name` in its first chunk, but the schema allows them in a later chunk. Streams that stay in this format replay them exactly. A call that starts without them reaches Anthropic and Responses apps with an empty id and name, because those formats announce both when the block starts.

Events from other dialects become one chunk each, after the opening role chunk, with `logprobs: null` and `finish_reason`. Tool indexes are assigned in the order tool calls start, and reasoning, citations, opaque data, unknown blocks and extras of other dialects are left out ([replies](cross-format.md#replies)). While they arrive, the encoder writes paced SSE comments (`: keepalive`), which the SSE spec and the SDK parsers ignore: one for each extra event of another dialect, such as Anthropic `ping`, and one per 1,024 bytes of hidden text ([keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gives the gateway the same frame for silence upstream. Usage is written only when the app asked for it (`EncodeCtx::stream` with `include_usage`): every chunk then has `usage: null`, and a final `choices: []` chunk carries the usage. From other dialects this is the latest usage, with the input counts from `ResponseStart` when the later report has none.

## Errors (`ErrorResponse`)

The error body is `{error: {message, type, param, code}}`.

| Signal | `ErrorKind` | Encoded for KPP errors from elsewhere |
|---|---|---|
| `code: context_length_exceeded` | `ContextTooLong` | 400 `invalid_request_error`, code `context_length_exceeded` |
| `type` or `code` `insufficient_quota`, billing codes | `Billing` | 429 `insufficient_quota`, code `insufficient_quota` |
| `code` `server_is_overloaded` or `service_unavailable_error` | `Overloaded` | see 503 |
| `code` `content_policy_violation`, `content_filter`, `bio_policy`, `misalignment_policy_violation` or `image_content_policy_violation` | `ContentFiltered` | 400 `invalid_request_error`, code `content_policy_violation` |
| 400, 409, 422 | `InvalidRequest` (and `Unsupported` when encoding) | 400 `invalid_request_error` |
| 401 | `Authentication` | 401 `invalid_request_error` |
| 403 | `PermissionDenied` | 403 `request_forbidden` |
| 404 | `NotFound` | 404 `invalid_request_error` |
| 413 | `RequestTooLarge` | 413 `invalid_request_error` |
| 429 | `RateLimited` | 429 `rate_limit_error`, code `rate_limit_exceeded` |
| 503 | `Overloaded` | 503 `service_unavailable_error`, code `server_is_overloaded` |
| 408, 504, `code` `vector_store_timeout` | `Timeout` | 504 `server_error`, code `timeout` |
| other | `ProviderError` | its 5xx status (not 503) or 500, `server_error` |
| `error.misalignment` (`MisalignmentErrorDetailsResource` in the `Error` schema), unknown `error` keys | kept in `KppError.raw` (the whole body) | Sent again only when the error came from Chat. |

A code named in the table, in `code` or `type`, wins over the status. Codes of the status-based kinds (`rate_limit_exceeded`, `invalid_request_error`, `server_error`, the image codes) decide only for a stream error, which has no status.

The Responses and Embeddings codecs use the same code table (`openai_responses::error`), so an error in the OpenAI shape has the same kind, `type` and `code` in every OpenAI API. The OpenAI SDKs choose their exception class by status, so `type` only adds information.

`retry-after-ms` or `retry-after` becomes `retry_after`, which is written as `retry-after` in whole seconds, rounded up, plus `retry-after-ms`, rounded up, when the delay has a fraction of a second. `param` becomes `KppError.param`. An error decoded from Chat is encoded again with its original body and status. Bodies that are not an `ErrorResponse`, such as an HTML 429 from a proxy, a plain-text 503 or `{"error": "rate limited"}`, take their kind from the status (table above) and keep the raw body; a string `error` becomes the message.

## Models

`GET /v1/models` and `GET /v1/models/{model}` are implemented in `crates/kaeso-codecs/src/openai_models.rs`, shared by the Chat, Responses and Embeddings codecs, because all OpenAI APIs list models in one shape. The fixtures are in `fixtures/openai_chat/models/`, and all three codecs round-trip them. Sources: the spec's `Model` and `ListModelsResponse` schemas and the `listModels` and `retrieveModel` operations, whose `x-oaiMeta` examples serve as fixtures. An app's request is OpenAI when it carries no `anthropic-version` header (`dispatch::models_dialect`, see [model listings](index.md#model-listings)); the gateway then encodes with `openai_chat`.

### Query parameters

`listModels` documents none. Every parameter, percent-decoded, is kept as an extra `query.<name>` of the decoding dialect: a string, or an array when it is repeated. Malformed escapes are `InvalidRequest`. OpenAI does not page, so the gateway calls `ModelList::page` without a default limit, and an OpenAI app always gets the whole list. The `{model}` path segment arrives percent-encoded, since fine-tuned ids contain `:`, and the gateway's router decodes it.

### `Model` ⇄ `kaeso_kpp::ModelInfo`

| Wire field | KPP | Encoded when KPP has no value |
|---|---|---|
| `id` | `id` | (required) |
| `object` (`"model"`) | form `object`, only when it differs | `"model"` |
| `created` (Unix seconds) | `created_at` | `0` |
| `owned_by` | `owned_by` | `"kaeso"`: the owner is unknown, as for a model listed by Anthropic, and the model is served by the Kaeso gateway. |
| `shutdown_date` (`YYYY-MM-DD` or `null`) | `retires_at` (that date at 00:00 UTC) | `null` |
| unknown keys | extras fields | |

The gateway lists models from its catalogue: it sets `owned_by` to the provider id and `created_at` to the model's release date when the catalogue has one.

A field received absent, or `null` where the default is a value, is recorded in form `none` and written the same way; `shutdown_date` is optional in the spec. A `shutdown_date` that is not a `full-date` is `ProviderError`. Extras from any of the three OpenAI dialects are honoured by all three encoders.

### `ListModelsResponse` ⇄ `ModelList`

| Wire field | KPP |
|---|---|
| `object` (`"list"`) | form `object`, only when it differs |
| `data` | `models` |
| unknown keys | extras fields |

Towards OpenAI apps some data is lost: `display_name`, `lifecycle`, `deprecated_at`, `context_window`, `max_output_tokens`, `has_more` and other dialects' extras (Anthropic `capabilities`, `line`) have no field and are left out ([replies](cross-format.md#replies)). A `retires_at` that is not midnight UTC is cut to its UTC date, and one after the year 9999 is written as `shutdown_date: null`. `created` is an integer and is written as stored, so a millisecond value stays one. In `decode_models` and `decode_model`, a status other than 2xx goes to `decode_error`.
