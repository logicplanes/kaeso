---
title: OpenAI Chat Completions
description: How every field of the OpenAI Chat Completions format maps to KPP and back.
order: 5
nav: OpenAI Chat
---

Look up any field of the OpenAI Chat Completions format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::openai_chat::OpenAiChat` (`crates/kaeso-codecs/src/openai_chat/`)
- **Endpoints:** `POST /v1/chat/completions` (JSON and SSE), and the [models](#models) endpoints shared by every OpenAI codec
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_chat/`

**Sources.** The OpenAI OpenAPI spec at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (schema names below), the [create reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create), the [streaming events reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events) and the [error codes guide](https://developers.openai.com/api/docs/guides/error-codes).

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields` of the named item, bound to `openai_chat`. They stay within this dialect; another dialect refuses them with `Unsupported` ([extras](cross-format.md#extras)). Nested keys use key paths (`image_url.detail`).
- **form**: a codec-private `Extras::form` marker, presentation only, which other dialects ignore. `verbatim` markers keep `null` and `[]` spellings of declared keys, and of undeclared keys whose value is `null`.
- **Unsupported**: decoding or `check()` fails with `ErrorKind::Unsupported { feature }`; the feature is named in brackets.

**Numbers.** Typed numeric fields go through `u32`, `u64`, `i64` (`seed`) or `f64`. A value outside their range fails to decode, as `InvalidRequest` in a request and `ProviderError` in a reply, rather than being changed. Unknown fields are kept byte for byte, except keys whose names Kaeso can't carry ([below](#keys-kaeso-cant-carry)).

## Request: `CreateChatCompletionRequest`

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `GenerateRequest.model` typed | |
| `messages` | `system` + `messages` typed | See [messages](#messages-chatcompletionrequestmessage). |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typed | Written as integers (`1`): form `integers`, the list of keys so written. The encoder writes such a key as an integer while its value is still a whole number, so `temperature: 1` comes back as `1`, not `1.0`; a changed value is written as the new value. From other dialects: the `f64` spelling (`1.0`). |
| `max_completion_tokens` | `sampling.max_output_tokens` typed | The spelling written for requests from every source dialect, unless the provider's profile asks for `max_tokens` ([provider profiles](#provider-profiles)). `CreateChatCompletionRequest` sets no minimum for it or for `max_tokens`, so every non-negative integer is kept, `0` included; a negative one is `InvalidRequest`. |
| `max_tokens` (deprecated) | `sampling.max_output_tokens` typed | Form `max_tokens: "max_tokens"`. Both present with the same value: form `"both"`. With different values `max_completion_tokens` wins and `max_tokens` becomes the extra `max_tokens (differs from max_completion_tokens)`, sent back as `max_tokens`; the key names the conflict when another dialect refuses it. |
| `stop` | `sampling.stop` typed | String spelling: form `stop: "string"`; `[]`: form `"array"`. More than four sequences: `Unsupported` (`stop`). |
| `seed` | `sampling.seed` typed | Sent as `random_seed` when the provider's profile says so ([provider profiles](#provider-profiles)). |
| `frequency_penalty`, `presence_penalty` | `sampling.*` typed | Integer spelling as for `temperature` (form `integers`). |
| `n` | form `n` when `1` | `n > 1`: `Unsupported` (`n`, see [sampling](cross-format.md#sampling)); `0`: `InvalidRequest`. |
| `reasoning_effort` | `reasoning.effort` typed | From other dialects ([effort and budget](reasoning.md#effort-and-budget)): an explicit effort as is; Anthropic `thinking.disabled` becomes `none`; a budget becomes `low` (below 4,096), `medium` (4,096 to 16,383) or `high` (16,384 and more). Values `none` to `max`; unknown values are kept as `ReasoningEffort::Other` and passed through for Chat apps, while for requests from other dialects `check()` refuses values outside the spec enum (`reasoning.effort`). |
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
| `user` (deprecated) | `user` typed when `safety_identifier` is absent (form `user`); otherwise the extra `user (sent alongside safety_identifier)`, sent back as `user` | KPP `user` from other dialects is sent as `safety_identifier`. A provider whose profile has no end-user field gets neither ([provider profiles](#provider-profiles)). |
| `stream` | `stream` typed | `false`: form `stream_false`. |
| `stream_options.include_usage` | `StreamMode::On { include_usage }` typed | `stream_options` absent: form `stream_options: "absent"`; without `include_usage`: form `"no_include_usage"`; other keys (`include_obfuscation`) are extras `stream_options.<key>`; with `stream` off: the whole object as form `stream_options_off`. Requests from other dialects always ask for `include_usage: true`. The gateway needs usage, so it sets `include_usage: true` on every streaming request and keeps the app's own request in `EncodeCtx`; the app's stream encoder then hides the usage chunk from apps that did not ask for it. |
| `audio`, `functions` and `function_call` (deprecated, top level), `logit_bias`, `logprobs`, `metadata`, `modalities`, `moderation`, `prediction`, `prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `service_tier`, `store`, `top_logprobs`, `verbosity`, `web_search_options` | extras | No KPP equivalent; refused across dialects ([extras](cross-format.md#extras), [sampling](cross-format.md#sampling)), except `prompt_cache_key`, which is on the [extras allowlist](cross-format.md#extras) and dropped. `null` values are form. Towards Anthropic, for example, `functions` fails with `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. `` |
| unknown keys | extras | A key that starts with `header:` or contains `.` is refused ([keys Kaeso can't carry](#keys-kaeso-cant-carry)). |
| header `openai-beta` | extras `header:openai-beta` | Forwarded only when the request goes to Chat; on the [extras allowlist](cross-format.md#extras), so other dialects drop it. |

### Messages (`ChatCompletionRequestMessage`)

| Wire | KPP | Notes |
|---|---|---|
| leading `system` and `developer` messages | `GenerateRequest.system` blocks | The first block of each message carries form `message: {role, content}`, which keeps message boundaries, the role and the string or array spelling; the message's `name` and unknown keys are extras `message.<key>` on that block. A leading message whose first part is unknown stays a `Role::System` message and ends the leading run. From other dialects: one `system` message. |
| later `system` and `developer` messages | `Message { role: System }` ([system messages](cross-format.md#system-messages)) | `developer`: form `role: "developer"`. |
| `user` | `Message { role: User }` | `name` and unknown keys: message extras. |
| `assistant` | `Message { role: Assistant }`: content parts, then `refusal`, then `tool_calls` | `content` absent: form `content: "absent"`; `null`: verbatim; `name`, `audio`, `function_call`: extras. `annotations` is not in the request schema, but the SDKs send it back with every reply: `[]` is verbatim (form), so the conversation can move to another provider; a non-empty value becomes `TextBlock.citations` of the first text block, bound to Chat (without text: extras `annotations`), which other dialects refuse (`citations`) and Chat sends back as `annotations`. From other dialects, text blocks go to `content`, the first refusal to `refusal` and tool calls to `tool_calls`, because Chat cannot interleave them; reasoning is dropped ([reasoning](reasoning.md)); a turn left with nothing Chat can show, such as reasoning only, is sent as `content: ""`, so the conversation keeps its turns and Chat's rule that an assistant message needs content unless it has `tool_calls` is met. |
| assistant `refusal` | `Block::Refusal` | |
| assistant `function_call` (deprecated) | extras `function_call` of the message | `null` is kept verbatim. Written back as it came; other dialects refuse it (`messages[i].function_call`). |
| `tool` messages | `Block::ToolResult` in a `Role::User` message; consecutive tool messages share one KPP message | `tool_call_id` becomes `call_id`; content parts become nested text blocks; unknown keys are extras of the tool result. `is_error` from other dialects is dropped, because Chat reports errors in the content ([tool errors](cross-format.md#tool-errors)). From other dialects, the tool results of a user message become `tool` messages, followed by a `user` message with the remaining blocks. |
| `function` messages (deprecated) | `Message { role: User }` holding one `Block::Unknown` with the whole message, plus message form `role: "function"` | Written back as it came; other dialects refuse it (`` `function` message ``). |
| `content` as string or array | form `content: "array"` (message or tool result) | A string is the default for one plain text block. Empty arrays are invalid (`minItems: 1`). |
| a message without `role`, or with another role | `InvalidRequest` (`param` `messages[i]`) | |

Legacy function calling has no tool call ids, so its turns do not become KPP tool calls and results; within this dialect they round-trip instead, with the top-level `functions` and `function_call` parameters as extras. The fixture is `fixtures/openai_chat/requests/legacy-functions.json`. Other dialects refuse all of it with `Unsupported`: the top-level parameters as `functions` and `function_call`, an assistant's `function_call` as `messages[i].function_call`, and a `function` message as `` `function` message ``, with the hint `Remove the message, or send the request to a model served through its original format.`

### Content parts

| Wire | KPP | Notes |
|---|---|---|
| `text` | `Block::Text` | Unknown keys: block extras. |
| `prompt_cache_breakpoint` (text, image and file parts) | `cache: CacheHint { ttl: None }` bound to Chat | Both mark the end of a reusable prefix; the TTL comes from `prompt_cache_options`. Only hints from Chat are sent back; hints from other dialects are dropped ([cache hints](cross-format.md#cache-hints)). `mode` other than `explicit`: form `mode`. |
| `image_url.url` (`https:`) | `Block::Image { source: Url }` | |
| `image_url.url` (`data:<type>[;params];base64,<data>`, RFC 2397, scheme and `base64` in any case) | `Block::Image { source: Base64 }` | A header other than `data:<type>;base64`, such as parameters (`;name=a.png`) or upper case, is kept exactly in form `data_url`, also for `file.file_data`. Data URLs that are not base64 or have no media type stay `Url`. |
| `image_url.detail` | extras `image_url.detail` | On the [extras allowlist](cross-format.md#extras): dropped silently for other dialects. |
| `file.file_data` (`data:` URL) | `Block::Document { source: Base64 }` | Bare base64 is taken as media type `application/pdf`, the only type in OpenAI's file inputs guide, with form `bare_file_data`, and is written back bare. |
| `file.file_id` | `Block::Document { source: FileId }` bound to the OpenAI file store ([files](cross-format.md#files)) | With `file_data` too: extras `file.file_id`. |
| `file.filename` | extras `file.filename` | The file name OpenAI requires next to inline `file_data`, not a title shown to the model, so it never becomes `DocumentBlock::title`. On the [extras allowlist](cross-format.md#extras). Inline documents from other dialects get `document.<subtype>`, such as `document.pdf`, as every example of OpenAI's [file inputs guide](https://developers.openai.com/api/docs/guides/file-inputs) sends one with `file_data`. |
| `input_audio` | `Unsupported` (`input_audio`) | Audio input is not part of KPP. |
| `refusal` (assistant) | `Block::Refusal` + form `part` | |
| other types, and `image_url` and `file` parts outside user messages, `refusal` parts outside assistant messages | `Block::Unknown` | Same dialect only. |

For KPP from other dialects, `check()` refuses text documents, document URLs, document `title`, `context` and `citations`, image file ids, images and documents outside user messages, refusals outside assistant messages, citations outside assistant messages (and, through `check_foreign`, other dialects' citations), `top_k` ([sampling](cross-format.md#sampling)), unknown reasoning modes and reasoning efforts outside the spec enum. Budgets and `disabled` are converted ([effort and budget](reasoning.md#effort-and-budget)). These hints are dropped instead: reasoning blocks ([reasoning](reasoning.md)), other dialects' cache hints ([cache hints](cross-format.md#cache-hints)), `ToolResult::is_error`, whose information the error content carries ([tool errors](cross-format.md#tool-errors)), and other dialects' `reasoning.summary` and `reasoning.include_encrypted`, because Chat has no field that asks for them ([readable reasoning and summaries](reasoning.md#readable-reasoning-and-summaries)).

A body the codec can't read is `InvalidRequest` naming the field and what it must be, with `param` set, never a parser position: `` The request body is not a valid OpenAI Chat Completions request: `max_completion_tokens` must be an integer from 0 to 4294967295. ``

### Keys Kaeso can't carry

An unknown key whose name starts with `header:` or contains `.` can't be carried, because KPP spells headers and key paths that way. It is refused wherever the codec reads an object key by key:

- the body, `stream_options`, a `response_format` of type `text`, `json_object` or `json_schema` and its `json_schema`;
- function tools and their `function`, and a named `tool_choice` and its `function`;
- `system`, `developer`, `user`, `assistant` and `tool` messages;
- `text`, `refusal`, `image_url` and `file` parts with their `image_url`, `file` and `prompt_cache_breakpoint` objects;
- function tool calls and their `function`.

The request is then `InvalidRequest`, such as `` The request body is not a valid OpenAI Chat Completions request: the field `header:authorization` has a name Kaeso cannot carry; remove it. `` or, for a message, `` The request contains an invalid OpenAI Chat Completions user message: the field `a.b` has a name Kaeso cannot carry; remove it. `` A name longer than 64 bytes is cut and ends in `…`. Objects kept whole, such as unknown parts, tools and tool choices, custom tool calls, `function` messages and other `response_format` types, are not looked into. In replies and stream chunks the same rule gives a `ProviderError` (in a stream, an `Error` event).

### Tool calls (`ChatCompletionMessageToolCall`)

| Wire | KPP | Notes |
|---|---|---|
| `id`, `function.name` | `ToolCallBlock.id`, `.name` | Names Kaeso rewrote are mapped back when a reply is decoded. A call in a reply without an `id`, or with `""`, gets `call_kaeso_<response id>_<n>` (the id's letters and digits, `n` its position), so apps in other formats can answer it; form `id` keeps the received spelling (`null` for absent) for Chat apps. Streams do the same with `n` the KPP block index and form `tc`. |
| `function.arguments` | `ToolCallBlock.arguments` raw | The JSON text inside the string, byte for byte. Text that is not valid JSON, or has surrounding whitespace, keeps its literal in form `arguments`; KPP then holds `{}` for blank text and otherwise the text as a JSON string. KPP's convention is that a JSON string in `ToolCallBlock.arguments` means the provider sent text that is not JSON, so for calls from other dialects the string's content is sent as `arguments`; objects, arrays and other JSON values are sent as their JSON text. |
| unknown keys | extras (`function.<key>` nested) | |
| `type: custom` | `Block::Unknown` | Sent back in `tool_calls`. |
| `type` absent, with a `function` object | `Block::ToolCall`, form `no_type` | `ChatCompletionMessageToolCall` requires `type`, but some OpenAI-compatible providers leave it out; only function calls have a `function` object, so the call reaches every app. Written back without `type`. |

## Provider profiles

Other providers speak this format with differences of their own. Each provider in `PROVIDERS` (`crates/kaeso-core/src/provider.rs`) names a `kaeso_codecs::openai_chat::ChatProfile` in its `chat` field: the provider's name and the settings for the requests Kaeso sends it (`ChatProfile::request`, a `RequestProfile`), set with the builders below. The gateway hands it to the encoder with `EgressCtx::with_chat_profile`, which applies it to every request, whatever format the app sent. `ChatProfile::OPENAI`, the default, changes nothing, and neither does `ChatProfile::compatible(name)` until a setting is added.

| Setting | What the encoder does | Why it is allowed |
|---|---|---|
| `max_tokens()` | Sends the output limit as `max_tokens`, and a `max_completion_tokens: null` as `max_tokens: null`. A Chat request with both at different values is `Unsupported` (`max_tokens`). | Only the spelling changes; the value is the same. |
| `system_role()` | Sends `developer` messages as `system` messages. | The role spelling is form; both carry the instructions. |
| `random_seed()` | Sends `seed` as `random_seed`, a `seed: null` too. A negative seed, or a request with both `seed` and `random_seed` (either may be `null`), is `Unsupported` (`seed`). The rule follows Mistral's `random_seed`, which its OpenAPI document gives `minimum: 0`. | `seed` stays semantic: it is renamed or refused, never dropped. |
| `without_end_user()` | Leaves KPP `user` out instead of sending it as `user` or `safety_identifier`, together with a second `user` a Chat app sent alongside `safety_identifier`, and their `null` spellings. | `user` is a hint ([field classes](index.md#field-classes)). |
| `without_prefill()` | Refuses a conversation that ends with an assistant message, unless it calls tools or is a Chat app's message with `prefix: true`: `Unsupported` (`assistant prefill`, param `messages[i]`). | A provider that only continues such a message with `prefix: true` repeats the prefill in its reply, so the app would not get a continuation. |
| `refusing(fields)` | Refuses a request whose body would contain one of these top-level keys, such as `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | Extras are semantic, so they are refused, not dropped. |
| `refusing_in_messages(fields)` | The same for keys of system, user and assistant messages. Tool messages are not checked. | As above. |

Settings for the replies Kaeso reads (`ChatProfile::reply`, a `ReplyProfile`):

| Setting | What the decoder does | Why it is allowed |
|---|---|---|
| `thinking_parts()` | Reads a `thinking` part of a `content` array (`{type: "thinking", thinking: [text parts], closed}`, nothing else) as a `Block::Reasoning` whose `text` holds each text part, and streamed ones as `BlockStart(Reasoning)` and `BlockDelta(ReasoningText)`. The reasoning block ends when the answer, a refusal or a tool call starts, and a stream's empty text pieces before the first real one open no text block (form `et`). A `thinking` part with other keys or other inner parts stays as without the setting. | Nothing is dropped: Chat apps get the same parts back, and other formats get the thinking in their own reasoning shape with the original block in a [reasoning blob](reasoning.md#reasoning-blobs). |
| `reasoning_fields(fields)` | Reads the first of these message keys that holds a non-empty string, such as DeepSeek's `reasoning_content` or Groq's `reasoning`, as a `Block::Reasoning` before the reply's other blocks, with form `reasoning_field` naming the key. In a stream, that key of each delta becomes `BlockDelta(ReasoningText)` of one reasoning block, with form `rf` on the chunk. The reasoning block ends when the answer, a refusal or a tool call starts, and empty text pieces before the first real one open no text block (form `et`). An empty string, `null` or another type stays an extra. | Nothing is dropped: Chat apps get the key back as sent, the delta's other keys (Groq's `channel`) stay extras of the chunk, and other formats get the reasoning in their own reasoning shape with the original block in a [reasoning blob](reasoning.md#reasoning-blobs). Without the setting, every streamed piece of reasoning would be a chunk without a KPP event, logged as an unknown block of its own. |
| `error_detail(path)` | Appends the string at this path of an error body, such as OpenRouter's `error.metadata.raw`, to the error's message: `<message>: <detail>`. A missing, empty or non-string value, or one the message already contains, changes nothing. Stream errors get the same. | The body stays as it was: Chat apps get the provider's error unchanged, and other formats get the reason in the message, where a generic one (`Provider returned error`) would tell them nothing. The gateway takes key-like tokens out of every provider error message. |

`refusing` and `refusing_in_messages` each take the whole list; a second call replaces the first. A refusal names the field as the app wrote it: a Chat app's key by its own name and path (`logit_bias`, `messages[1].name`), and a refusal block from another format by its KPP path (`messages[1].blocks[0]`). Refusals happen while the request is encoded, on the request the encoder built and before it is written out, so nothing is sent, and they reach the app in its own format.

Each built-in provider whose profile differs from OpenAI's is listed below with the source of every setting: the provider's documentation, its OpenAPI document or a live recording in `fixtures/openai_chat/providers/<id>/`.

| Provider | Settings | Sources |
|---|---|---|
| DeepSeek (`deepseek`) | `max_tokens()`, `system_role()`, `reasoning_fields(["reasoning_content"])` | DeepSeek's API reference and thinking mode guide (`reasoning_content`), `responses/recorded-thinking-reasoning-content` and `streams/recorded-thinking-stream`; `fixtures/openai_chat/providers/deepseek/responses/recorded-max-completion-tokens-limit`: DeepSeek ignores `max_completion_tokens` (20 asked, 133 completion tokens), while its API reference documents `max_tokens`; `fixtures/openai_chat/providers/deepseek/errors/recorded-plain-developer-string`: DeepSeek answers the `developer` role with 422 `unknown variant` |
| Groq (`groq`) | `system_role()`, `reasoning_fields(["reasoning"])`; refuses `logit_bias`, `logprobs` and `top_logprobs`, and `name` and `refusal` on messages | Groq's OpenAI compatibility page (https://console.groq.com/docs/openai, which lists `logprobs`, `logit_bias`, `top_logprobs` and `messages[].name` as unsupported); `fixtures/openai_chat/providers/groq/errors/`: `recorded-logit-bias`, `recorded-logprobs-top-logprobs-doc`, `recorded-tools-results-parallel-error` (`refusal` on an assistant message) and `recorded-tools-single` (Qwen's chat template refuses the `developer` role); Groq's reasoning guide (`reasoning`), `responses/recorded-reasoning-format-parsed` and `streams/recorded-stream-doc` |
| Mistral (`mistral`) | `max_tokens()`, `system_role()`, `random_seed()`, `without_end_user()`, `without_prefill()`, `thinking_parts()`; refuses `audio`, `function_call`, `functions`, `logit_bias`, `moderation`, `prompt_cache_options`, `prompt_cache_retention`, `store`, `verbosity` and `web_search_options`, and `name` and `refusal` on messages | Mistral's OpenAPI document (https://docs.mistral.ai/openapi.yaml, read on 2026-10-19), where `ChatCompletionRequest` forbids undeclared fields and gives `random_seed` `minimum: 0`; the `422 extra_forbidden` recordings in `fixtures/openai_chat/providers/mistral/errors/`, one per refused or renamed field; `responses/recorded-audio-field`, where Mistral takes `audio` and answers with text only; and `recorded-assistant-prefill` (400 `invalid_request_message_order`) with `responses/recorded-assistant-prefix`, whose reply repeats the prefill; `responses/recorded-reasoning-effort-high` and `streams/recorded-reasoning-effort-high-stream` for `thinking_parts()` |
| OpenRouter (`openrouter`) | `reasoning_fields(["reasoning"])`, `error_detail(["error", "metadata", "raw"])` | OpenRouter's reasoning tokens guide (`reasoning`, with `reasoning_details` beside it); `responses/recorded-free-reasoning` and `streams/recorded-free-reasoning-stream`; OpenRouter's errors and debugging page (`metadata.raw`, the upstream provider's error), `errors/recorded-upstream-rate-limited` and `errors/recorded-routing-fields` |
| SpaceX (`xai`) | `reasoning_fields(["reasoning_content"])` | xAI's Chat Completions reference (`reasoning_content`); `responses/recorded-reasoning-content` and `streams/recorded-reasoning-content-stream` |
| Together AI (`togetherai`) | `reasoning_fields(["reasoning_content", "reasoning"])` | Together AI's chat completions reference; the key depends on the model: `responses/recorded-sampling-extras` (`reasoning_content`), `responses/recorded-reasoning-reply` and `streams/recorded-reasoning-stream` (`reasoning`) |

## Response: `CreateChatCompletionResponse`

| Wire field | KPP | Notes |
|---|---|---|
| `id`, `model` | `Response.id`, `.model` | From another format, the provider's id as it is ([reply ids](cross-format.md#reply-ids)). |
| `object` | none | Always `chat.completion`; another value, or none: form `object`. |
| `created`, `service_tier`, `system_fingerprint`, `metadata`, `moderation` | extras | A reply without `created` in its extras, such as one from another dialect, gets `EncodeCtx::created_at`, or `0`. |
| `choices` | `choices[0]` | More than one choice: `Unsupported` (`n`); none: `ProviderError`; an `index` other than 0: extras `choice.index`. A 200 body with an `error` object and no `choices`, which OpenRouter sends when the provider fails after the 200 ([errors and debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging)), is decoded as that error, like a stream error. |
| `choices[0].logprobs` | extras `choice.logprobs` | `null`: verbatim. |
| `choices[0].finish_reason` | `Response.stop` | `stop` ⇄ `EndTurn`, `length` ⇄ `MaxTokens`, `tool_calls` ⇄ `ToolUse`, `content_filter` ⇄ `ContentFilter`, other values (such as `function_call`) ⇄ `Other`. From other dialects: `StopSequence` and `PauseTurn` become `stop`; `Refusal` becomes `content_filter`, because the answer was withheld. Other dialects' own values become the closest Chat one: `max_tokens`, `max_output_tokens` and `model_context_window_exceeded` become `length`, `tool_use` becomes `tool_calls`, `refusal` becomes `content_filter`, and anything else outside the spec enum becomes `stop`. A non-streaming response without `finish_reason`, which the spec requires: `ProviderError`. |
| `message.content` | `Block::Text` | From other dialects all text blocks are joined without a separator, because Chat has no content parts in responses. There is no separator because Anthropic splits one answer into several text blocks around citations, and the streamed deltas, which carry no separator either, must add up to the same `content`. |
| `message.content` as an array of parts (Mistral, not in the OpenAI spec) | `Block::Text` for each part that is exactly `{type: "text", text}`, `Block::Reasoning` for a `thinking` part under a profile with `thinking_parts()`, `Block::Unknown` for every other part; form `content_parts`, and form `thinking` keeps a thinking part's other keys | Written back as the same array for Chat apps. Other dialects get the text and the reasoning; unknown parts reach only Chat apps ([replies](cross-format.md#replies)). Source: Mistral's `AssistantMessage.content` (`ContentChunk[]`, https://docs.mistral.ai/openapi.yaml) and the recording `fixtures/openai_chat/providers/mistral/responses/recorded-reasoning-effort-high.json`. |
| `message.reasoning_content`, `message.reasoning` and other keys a profile names in `reasoning_fields` | `Block::Reasoning` first, form `reasoning_field` | Only under a profile that names the key ([provider profiles](#provider-profiles)); otherwise extras `choice.message.<key>`. Written back to the same key for Chat apps. |
| `message.annotations` | `TextBlock.citations` (raw, bound to Chat) | Without content: extras `choice.message.annotations`. Citations of other dialects are left out ([replies](cross-format.md#replies)). |
| `message.refusal` | `Block::Refusal` | |
| `message.tool_calls` | `Block::ToolCall` or `Block::Unknown` | As in requests. |
| `message.audio` (`id`, `data`, `expires_at`, `transcript`), `message.function_call`, unknown keys | extras `choice.message.<key>` | The audio object, with its `transcript` and `expires_at`, is kept whole. |
| reasoning, unknown blocks of other dialects | none | Left out of a Chat reply ([replies](cross-format.md#replies)). The gateway keeps replayable reasoning for the next turn, keyed by the tool calls it came before ([reasoning kept by the gateway](reasoning.md#reasoning-kept-by-the-gateway), `kaeso_codecs::continuity`). |

A reply from another dialect spells empty values as OpenAI does: `content: null`, `refusal: null`, `annotations: []` and `logprobs: null`.

### Usage (`CompletionUsage`)

The KPP fields follow the [usage](cross-format.md#usage) convention.

| Wire | KPP |
|---|---|
| `prompt_tokens − cached_tokens − cache_write_tokens` | `input_tokens` |
| `prompt_tokens_details.cached_tokens` | `cache_read_tokens`, at most `prompt_tokens`; `cache_write_tokens` at most what is left |
| `prompt_tokens_details.cache_write_tokens` | `cache_write_tokens` |
| `completion_tokens` | `output_tokens` (includes reasoning; when `total_tokens` is `prompt_tokens + completion_tokens + reasoning_tokens`, as SpaceX counts, the reasoning tokens are added to it); `null`, as for `prompt_tokens` and `total_tokens`, counts as absent (0) and stays in `raw` |
| `completion_tokens_details.reasoning_tokens` | `reasoning_tokens` |
| `total_tokens` | derived (`prompt_tokens + completion_tokens`); not stored separately, kept in `raw` |
| `prompt_tokens_details.audio_tokens`, `text_tokens`, `image_tokens`, `completion_tokens_details.audio_tokens`, `text_tokens`, `accepted_prediction_tokens`, `rejected_prediction_tokens` | `raw` only (breakdowns of the typed totals) |
| the whole object | `raw`, sent back unchanged while the typed fields still match it; otherwise the typed fields win and the raw object adds the other keys |

From other dialects: `prompt_tokens = input + cache_read + cache_write`, `prompt_tokens_details.cached_tokens = cache_read`, plus `cache_write_tokens` when it is not zero, and `completion_tokens_details.reasoning_tokens` when KPP has a reasoning count.

## Streams: `CreateChatCompletionStreamResponse`

Bare `data:` frames, one JSON chunk each, ending with `data: [DONE]`.

| Wire | KPP event | Notes |
|---|---|---|
| first chunk | `ResponseStart { id, model }` | `created`, `service_tier`, `system_fingerprint`: extras; `object` other than `chat.completion.chunk`, or none: form `object`. When the first chunk has no content and an empty or missing `id` or `model`, the start and the chunks without content after it are held back until both values are known or a chunk carries content (a block or response delta); the start then goes out with the values known by then, each from the first chunk that had it non-empty. |
| `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint` on every chunk | none | The envelope repeats. Where a chunk's envelope differs from the one the encoder rebuilds from `ResponseStart`, form `chunk.env` keeps the chunk's values and `chunk.env_absent` the keys it lacks, so every chunk, the first included, is written back as it came. Chunks that match follow `ResponseStart`, so a gateway that rewrites `id` or `model` there rewrites them. |
| `choices[0].delta.role` | form `chunk.role` | From other dialects the stream opens with a role chunk as soon as the response starts, `delta: {"role": "assistant", "content": "", "refusal": null}`, OpenAI's own first chunk of a text reply, so the app hears from the stream while the model thinks ([keepalives](cross-format.md#keepalives)). |
| `choices[0].delta.content` | `BlockStart(Text)` once, then `BlockDelta(Text)` | Every piece, including `""`. |
| `choices[0].delta.content` as an array of parts (Mistral) | `BlockDelta(Text)` for each part that is exactly `{type: "text", text}`, `BlockDelta(ReasoningText)` for each text of a `thinking` part under a profile with `thinking_parts()`; the other parts are chunk extras `delta.content`, and form `cs` records where each part sat | Chat apps get the same array back; other dialects get the text and the reasoning and leave the rest out. Mistral streams its thinking this way, and the first answer token can share an array with it (`fixtures/openai_chat/providers/mistral/streams/recorded-reasoning-effort-high-stream.sse`). |
| `choices[0].delta.reasoning_content`, `delta.reasoning` and other keys a profile names in `reasoning_fields` | `BlockStart(Reasoning)` once, then `BlockDelta(ReasoningText)`; form `chunk.rf` | Only under a profile that names the key; otherwise chunk extras `delta.<key>`. The reasoning block stops when text, a refusal or a tool call starts. Chat apps get the key back in the same chunk. |
| `choices[0].delta.refusal` | `BlockStart(Refusal)` once, then `BlockDelta(Refusal)` | |
| `choices[0].delta.tool_calls[]` with a new `index` | `BlockStart(ToolCall { id, name })` + `BlockDelta(ToolArguments)` for `arguments` | KPP block indexes follow the order blocks start; the Chat `index` maps to the KPP index. Rewritten names are mapped back. Shape differences (a missing `id`, `type`, `function`, `name` or `arguments`, an unexpected `index`): form `tc`. Unknown keys: extras `tool_call.<key>` and `tool_call.function.<key>`. A `type` other than `function`: `BlockStart(Unknown)`, and its later entries `BlockDelta(Unknown)`. An entry without a non-empty `id` or `name`, which some OpenAI-compatible providers send in a later chunk: the `BlockStart` and every event after it are held until both arrived, the block ends, or the stream ends or breaks; the header then carries the late `id` and `name`, names mapped back. Form `tc` keeps an `id` or `name` the first entry had as `""`. |
| `choices[0].delta.tool_calls[]` with a known `index` | `BlockDelta(ToolArguments)` | A repeated or late `id`, `type` or `name`: form `tc`. A different non-empty `id` where the call already has one is a new call that reuses the index: the earlier block stops and a new `BlockStart` begins, with form `tc.index`. |
| `choices[0].finish_reason` | `BlockStop` for every open block, then `ResponseDelta { stop }` | Mapped as for responses. `""`, which is no finish reason, counts like `null`: form `chunk.cv`. |
| `usage` (chunk with `choices: []`) | `ResponseDelta { usage }` | Usage as in [usage](#usage-completionusage), raw kept. `choices` empty or absent: form `chunk.ch`. |
| `usage` on a chunk before `finish_reason` | held, form `chunk.u` | Running usage, which vLLM-based providers send on every chunk with `stream_options.continuous_usage_stats`. KPP reports usage only after the last block stops, so the decoder keeps the latest and sends it with the finish chunk's `ResponseDelta` when that chunk has none of its own, or as a `ResponseDelta { usage }` before `ResponseStop` when no usage follows the finish. Form `chunk.u` keeps each chunk's object, so a Chat app that asked for usage gets every chunk back as it came; an app that did not ask gets no usage, and a chunk that held only usage is left out for it. The fixture is `fixtures/openai_chat/streams/usage-every-chunk.sse`. |
| `usage: null`, `logprobs: null`, `finish_reason: null`, `content: null`, `refusal: null` | form `chunk.kv`, `cv`, `dv` | |
| `choices[0].logprobs`, unknown choice keys | extras `choice.<key>` on the chunk's first event | |
| `delta.audio`, `delta.function_call`, unknown delta keys | extras `delta.<key>` | A chunk after the first with no typed content at all is an `Extra` (`{"event": null, "data": <chunk>}`). |
| `obfuscation` | form `chunk.obf` | Not sent to apps when the events came from another dialect. |
| `moderation`, unknown chunk keys | extras on the chunk's first event | |
| `data: [DONE]` | `ResponseStop` (after the `BlockStop`s of blocks still open) | A stream that ends after `finish_reason` without `[DONE]`: `ResponseStop` with form `no_done`; one that ends earlier, with or without `[DONE]`: `Error`, because the reply was cut off. A `[DONE]` frame with SSE fields is an `Extra`, and the stream then ends as one without `[DONE]`. |
| `data: {"error": {…}}` | `Error`, decoded like an error body without a status | The decoder stops. Encoded as the same frame; no `[DONE]` follows. Also when the chunk carries `choices` as well, as OpenRouter sends mid-stream errors ([errors and debugging](https://openrouter.ai/docs/api/reference/errors-and-debugging)); fixture `fixtures/openai_chat/streams/error-in-chunk.sse`. |
| `choices[0].finish_reason: "error"` without an `error` object | `Error` (`ProviderError`) | Not a finished reply. The chunk is kept in `KppError.raw` and sent back to Chat apps as it came. |
| `event: error` frames with data, which some OpenAI-compatible providers send | `Error` | Decoded like an error body without a status; encoded as a `data:` frame. Without data, such a frame is an `Extra`. |
| a `data:` frame that is not JSON, a chunk Kaeso cannot read | `Error` (`ProviderError`) | The decoder stops. |
| SSE comments, frames without data or with empty data, other named events, frames after `[DONE]` | `Extra` (`{"comment"}` or `{"event", "data", "text", "id", "retry", "other"}`, without `data` for a frame that had none and with `text: true` when the data is not JSON) | Same dialect only; written back as they came. |
| several choices, a choice `index` other than 0 | `Error` with `Unsupported` (`n`) | |

Chunk grouping: the first event of each chunk carries the form marker `chunk` (`n` is the number of events from that chunk, plus the shape markers above) and the others carry `in_chunk`, so the encoder writes them back as one frame. An open chunk is written as soon as an event arrives that is not part of it (another dialect's event, `[DONE]`, an error), so a dropped event never puts a chunk after `[DONE]`; no chunk is written after `[DONE]` or an error. Tool arguments for a block Chat cannot show are left out.

SSE framing follows the spec ([server-sent events](codec-design.md#server-sent-events)): an `id` that contains NUL, or a `retry` that is not ASCII digits fitting a `u64`, counts as an unknown field. A chunk's SSE `id`, `retry` and unknown fields, which OpenAI does not send, are kept in form `chunk.sse` as `{"id", "retry", "other": [[name, value], …]}`; the chunk is decoded like any other, and the fields are written back to Chat apps. A frame without a `data:` line, such as `retry: 3000` or `event: ping` alone, is not an event under the spec: it becomes an `Extra` whose raw JSON has no `data` key and is written back without a data line. Named events and frames after `[DONE]` are kept whole and replayed only to Chat apps.

OpenAI sends a tool call's `id`, `type` and `function.name` in its first chunk, but the schema allows them in a later chunk, and some OpenAI-compatible providers send `id` and `name` there, after a first chunk with `choices: []` and an empty `id` and `model`. Anthropic and Responses announce both when a block starts, so the decoder holds the start and the call until it knows them, as above; the order of events never changes. Streams that stay in this format replay every chunk exactly. The fixture is `fixtures/openai_chat/streams/late-envelope-and-tool-header.sse`.

Events from other dialects become one chunk each, after the opening role chunk, with `logprobs: null` and `finish_reason`. Tool indexes are assigned in the order tool calls start, and reasoning, citations, opaque data, unknown blocks and extras of other dialects are left out ([replies](cross-format.md#replies)). While they arrive, the encoder writes paced SSE comments (`: keepalive`), which the SSE spec and the SDK parsers ignore: one for each extra event of another dialect, such as Anthropic `ping`, and one per 1,024 bytes of hidden text ([keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gives the gateway the same frame for silence upstream. Usage is written only when the app asked for it (`EncodeCtx::stream` with `include_usage`): every chunk then has `usage: null`, and a final `choices: []` chunk carries the usage. From other dialects this is the latest usage, with the input counts from `ResponseStart` when the later report has none.

## Errors (`ErrorResponse`)

The error body is `{error: {message, type, param, code}}`.

| Signal | `ErrorKind` | Encoded for KPP errors from elsewhere |
|---|---|---|
| `code: context_length_exceeded` | `ContextTooLong` | 400 `invalid_request_error`, code `context_length_exceeded` |
| `type` or `code` `insufficient_quota`, billing codes | `Billing` | 429 `insufficient_quota`, code `insufficient_quota` |
| `code` `server_is_overloaded` or `service_unavailable_error` | `Overloaded` | see 503 |
| `code` `content_policy_violation`, `content_filter`, `bio_policy`, `cyber_policy`, `misalignment_policy_violation` or `image_content_policy_violation` | `ContentFiltered` | 400 `invalid_request_error`, code `content_policy_violation` |
| 400, 409, 422 | `InvalidRequest` (and `Unsupported` when encoding) | 400 `invalid_request_error` |
| 401 | `Authentication` | 401 `invalid_request_error` |
| 402 | `Billing` | 429 `insufficient_quota`, code `insufficient_quota` |
| 403 | `PermissionDenied` | 403 `request_forbidden` |
| 404 | `NotFound` | 404 `invalid_request_error` |
| 413 | `RequestTooLarge` | 413 `invalid_request_error` |
| 429 | `RateLimited` | 429 `rate_limit_error`, code `rate_limit_exceeded` |
| 503 | `Overloaded` | 503 `service_unavailable_error`, code `server_is_overloaded` |
| 408, 504, `code` `vector_store_timeout` | `Timeout` | 504 `server_error`, code `timeout` |
| other | `ProviderError` | its 5xx status (not 503) or 500, `server_error` |
| `error.misalignment` (`MisalignmentErrorDetailsResource` in the `Error` schema), unknown `error` keys | kept in `KppError.raw` (the whole body) | Sent again only when the error came from Chat. |

A code named in the table, in `code` or `type`, wins over the status. Codes of the status-based kinds (`rate_limit_exceeded`, `rate_limit_error`, `slow_down`, `invalid_request_error`, `invalid_prompt`, the image codes, `server_error`) decide only for an error without an HTTP status: a stream error, or an error in the body of a 200 reply. For such an error, a numeric `code` between 400 and 599, which OpenRouter sends as the HTTP status, decides the kind as a status would; the error still has no status.

HTTP 402 is how OpenAI-compatible providers such as DeepSeek report an exhausted balance ([DeepSeek error codes](https://api-docs.deepseek.com/quick_start/error_codes)), so it is `Billing` whatever its `code` says.

The Responses and Embeddings codecs use the same code table (`openai_responses::error`), so an error in the OpenAI shape has the same kind, `type` and `code` in every OpenAI API. The OpenAI SDKs choose their exception class by status, so `type` only adds information.

`retry-after-ms` or `retry-after` becomes `retry_after`, which is written as `retry-after` in whole seconds, rounded up, plus `retry-after-ms`, rounded up, when the delay has a fraction of a second. `param` becomes `KppError.param` when it is a string. An error decoded from Chat is encoded again with its original status, and with its original body when that body has an `error` object. Bodies Kaeso cannot read as an `ErrorResponse`, such as an HTML 429 from a proxy, a plain-text 503, `{"error": "rate limited"}` or a body with a key Kaeso cannot carry, take their kind from the HTTP status alone (table above; without a status, as in a stream, `ProviderError`) and keep the raw body. A string `error` becomes the message. The codec reads no meaning from the text: a provider whose answer to a wrong key has no authentication status names that message in `refused_key_messages` in `crates/kaeso-core/src/provider.rs`, and the gateway reports such an error as `Authentication` (SpaceX's `Incorrect API key provided` with HTTP 400). Otherwise, the message is `` The provider returned an error (HTTP 503) that is not in the OpenAI error format: <body> ``, with the body cut after 500 bytes and without the HTTP part in a stream. A readable body without a message gets `The provider returned an error without a message.`

## Models

`GET /v1/models` and `GET /v1/models/{model}` are implemented in `crates/kaeso-codecs/src/openai_models.rs`, shared by the Chat, Responses and Embeddings codecs, because all OpenAI APIs list models in one shape. The fixtures are in `fixtures/openai_chat/models/`, and all three codecs round-trip them. Sources: the spec's `Model` and `ListModelsResponse` schemas and the `listModels` and `retrieveModel` operations, whose `x-oaiMeta` examples serve as fixtures. An app's request is OpenAI when it carries no `anthropic-version` header (`dispatch::models_dialect`, see [model listings](codec-design.md#model-listings)); the gateway then encodes with `openai_chat`.

### Query parameters

`listModels` documents none. Every parameter, percent-decoded, is kept as an extra `query.<name>` of the decoding dialect: a string, or an array when it is repeated. Malformed escapes are `InvalidRequest`. OpenAI does not page, so the gateway calls `ModelList::page` without a default limit, and an OpenAI app always gets the whole list. The `{model}` path segment arrives percent-encoded, since fine-tuned ids contain `:`, and the gateway's router decodes it.

### `Model` ⇄ `kaeso_kpp::ModelInfo`

| Wire field | KPP | Encoded when KPP has no value |
|---|---|---|
| `id` | `id` | (required) |
| `object` (`"model"`, optional) | form `object`, only when it differs | `"model"` |
| `created` (Unix seconds) | `created_at` | `0` |
| `owned_by` | `owned_by` | `"kaeso"`: the owner is unknown, as for a model listed by Anthropic, and the model is served by the Kaeso gateway. |
| `shutdown_date` (`YYYY-MM-DD` or `null`) | `retires_at` (that date at 00:00 UTC) | `null` |
| unknown keys | extras fields | |

The gateway lists models from its catalogue: it sets `owned_by` to the provider id and `created_at` to the model's release date when the catalogue has one.

A field received absent, or `null` where the default is a value, is recorded in form `none` and written the same way; `shutdown_date` is optional in the spec. `object` is required in the spec, but OpenRouter's listing ([`GET /api/v1/models`](https://openrouter.ai/docs/guides/overview/models)) leaves it out of the list and of every model, so it is read the same way (fixture `providers/openrouter/models/list-recorded.json`). A `shutdown_date` that is not a `full-date` is `ProviderError`. Extras from any of the three OpenAI dialects are honoured by all three encoders.

### `ListModelsResponse` ⇄ `ModelList`

| Wire field | KPP |
|---|---|
| `object` (`"list"`, optional) | form `object`, only when it differs; absent or `null` in form `none` |
| `data` | `models` |
| unknown keys | extras fields |
| a bare array of models instead of the object, as Together AI sends it | `models`, form `bare_list` |

Towards OpenAI apps some data is lost: `display_name`, `lifecycle`, `deprecated_at`, `context_window`, `max_output_tokens`, `has_more` and other dialects' extras (Anthropic `capabilities`, `line`) have no field and are left out ([replies](cross-format.md#replies)). A `retires_at` that is not midnight UTC is cut to its UTC date, and one after the year 9999 is written as `shutdown_date: null`. `created` is an integer and is written as stored, so a millisecond value stays one. `encode_model_request` builds `GET /v1/models/{model}` without headers or body (the spec's `retrieveModel`); the gateway adds the key. `encode_models_request` builds `GET /v1/models` (the spec's `listModels`, which takes no query parameters, so a query with `limit`, `after_id`, `before_id`, `lifecycle` or other parameters is `Unsupported`). In `decode_models` and `decode_model`, a status other than 2xx goes to `decode_error`. An id that is `.` or `..`, or has either between slashes, is `InvalidRequest`, because URL parsers drop such segments even when they are percent-encoded.

## Providers that use this format

Providers other than OpenAI that serve Chat Completions record into `fixtures/openai_chat/providers/<id>/` ([fixtures](fixtures.md)). Their extra fields follow the rules above; this section names what each one adds.

### DeepSeek

Sources: DeepSeek's [API reference](https://api-docs.deepseek.com/api/create-chat-completion), [thinking mode guide](https://api-docs.deepseek.com/guides/thinking_mode/) and [error codes](https://api-docs.deepseek.com/quick_start/error_codes), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/deepseek/`.

| DeepSeek field | KPP | Notes |
|---|---|---|
| `thinking` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged, and an assistant turn of a Chat app that carries one sends it back to DeepSeek. Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item; when they send it back, it is left out of the request to DeepSeek ([reasoning](reasoning.md)), which answers without it. |
| `usage.prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` | `raw` usage | The hits equal `prompt_tokens_details.cached_tokens`, which is typed as a cache read. |
| `: keep-alive` lines and empty lines while the server is busy | `Extra` | Same dialect only, like other SSE comments. |
| HTTP 402 | `Billing` | An empty balance, whatever the `code` says. |
| HTTP 422 for a `developer` message | `InvalidRequest` | DeepSeek doesn't know the role; the app gets DeepSeek's error. |
| extra model fields (`name`, `context_window`, `max_output_tokens`, `input_modalities`, `output_modalities`, `effort`, `api_capabilities`) | model extras | DeepSeek's listing has no `created`. |

### SpaceX

Sources: xAI's [Chat Completions reference](https://docs.x.ai/developers/rest-api-reference/inference/chat-completions) and [models reference](https://docs.x.ai/developers/rest-api-reference/inference/models), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/xai/`. SpaceX's own format is [OpenAI Responses](openai-responses.md#providers-that-use-this-format); a model reaches Chat Completions through its `egress_dialect`.

| SpaceX field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). |
| `citations`, `system_fingerprint`, `service_tier` | extras | Left out towards other dialects. |
| `max_completion_tokens` (request) | `sampling.max_output_tokens` | SpaceX applies it only to visible output, not to reasoning tokens (Chat Completions reference; `responses/recorded-max-completion-tokens-reasoning`: 20 asked, 18 visible and 271 reasoning tokens). Usage counts both, so the output can exceed the limit; the gateway reserves the model's own output limit for a SpaceX reasoning model (`reasoning_beyond_output_limit` in `PROVIDERS`) under hard budgets and token rates ([limits and budgets](../../console/limits-and-budgets.md)). |
| `usage.num_sources_used`, `cost_in_usd_ticks`, `prompt_tokens_details.text_tokens` and `image_tokens` | `raw` usage | The token counts are typed as above. SpaceX leaves reasoning out of `completion_tokens` and counts it in `total_tokens`, so the output is `completion_tokens` plus `reasoning_tokens`. |
| `{"error": "<message>"}`, with `"code": "invalid-argument"` on a 400 | `KppError` | The string is the message, and apps get it in the OpenAI error shape with SpaceX's status. A wrong key is a 400 whose message starts with `Incorrect API key provided`; SpaceX's `refused_key_messages` names it, so the gateway reports a refused key. |
| `presence_penalty` for a reasoning model, `reasoning_effort` for a model without reasoning | | SpaceX answers 400, which reaches the app as its error. |
| `search_parameters` (request) | extras | SpaceX answers 410 because live search is deprecated; the app gets the 410. |
| extra model fields (`aliases`, prices in USD cents per 100 million tokens, `capabilities.reasoning_effort`, image `pricing`) | model extras | |

### Together AI

Sources: Together AI's [chat completions reference](https://docs.together.ai/reference/chat-completions-1) and [OpenAI compatibility](https://docs.together.ai/docs/inference/openai-compatibility), read 2026-10-19, the SDK types in `togethercomputer/together-py`, and the live recordings in `fixtures/openai_chat/providers/togetherai/`.

| Together AI field | KPP | Notes |
|---|---|---|
| `top_k`, `min_p`, `repetition_penalty`, `safety_model`, `context_length_exceeded_behavior` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning_content` or `message.reasoning`, and the same in deltas, by model | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). A streamed piece of reasoning comes with `content: ""`, which opens no text block. |
| `matched_stop` and `seed` on a choice, `text` on a stream choice, `token_id` in a delta, top-level `prompt`, `metadata` and `warnings` | extras | Left out towards other dialects. |
| `usage.reasoning_tokens` beside `completion_tokens_details.reasoning_tokens` | `raw` usage | The token counts are typed as above. |
| errors in OpenAI's envelope with Together AI's own `type` and `code` (`start_generation_failed`, `model_not_available`), sometimes with a top-level `id` | `KppError` | The kind follows the status; a wrong key is a 401 `invalid_api_key`, so `Authentication`. |
| `GET /v1/models`: a bare array of models with `type`, `display_name`, `organization`, `context_length`, `config` and `pricing` | `ModelList`, model extras | `GET /v1/models/{model}` answers 404 for ids with `/`, so a model check relies on the listing. |

### Groq

Sources: Groq's [API reference](https://console.groq.com/docs/api-reference) and [reasoning guide](https://console.groq.com/docs/reasoning), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/groq/`.

| Groq field | KPP | Notes |
|---|---|---|
| `reasoning_format`, `include_reasoning` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). |
| `delta.channel` (`analysis` on GPT-OSS reasoning) | extras of the chunk | Chat apps get it back with the reasoning; left out towards other dialects. |
| `x_groq`, `usage_breakdown`, `service_tier` (`on_demand`) | extras | Left out towards other dialects. |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time` | `raw` usage | The token counts, `reasoning_tokens` included, are typed as above. |
| `usage` on the finish chunk, also without `stream_options.include_usage` | `Usage` | Every dialect gets the usage, whether or not Groq also sends the usage chunk that `include_usage` asks for. |
| extra model fields (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features` and others) | model extras | |

### OpenRouter

Sources: OpenRouter's [API reference](https://openrouter.ai/docs/api_reference/overview), [streaming](https://openrouter.ai/docs/api_reference/streaming), [errors and debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging) and [reasoning tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/openrouter/`, all with `:free` models.

| OpenRouter field | KPP | Notes |
|---|---|---|
| `provider`, `models`, `transforms`, `usage`, `reasoning` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). |
| `message.reasoning_details`, `delta.reasoning_details` | extras | The same reasoning in OpenRouter's structured form; Chat apps get it back unchanged, other dialects get the text from `reasoning`. |
| `provider`, `native_finish_reason` on a reply | extras | Left out towards other dialects. |
| `usage.cost`, `is_byok`, `cost_details`, the extra token details | `raw` usage | Kaeso records costs at the catalogue's prices, not at `usage.cost`. |
| the last stream chunk, which repeats the finish choice beside `usage` | `Usage` | Read like a usage chunk with `choices: []`. |
| `: OPENROUTER PROCESSING` comments | `Extra` | Same dialect only, like other SSE comments. |
| `error.metadata.raw`, the upstream provider's own error, beside the generic `message` `Provider returned error` | `KppError.message` | Through the profile's `error_detail`, Anthropic Messages and Responses apps and the request log get `Provider returned error: <reason>`, such as `google/gemma-4-31b-it:free is temporarily rate-limited upstream…`. Chat apps get the body as OpenRouter sent it. |
| an `error` object after the 200, in the body or in a chunk beside `choices` | `KppError` | See the [rows above](#streams-createchatcompletionstreamresponse); a numeric `code` decides the kind as a status would. |
| `GET /api/v1/models`: no `object`, models without `object` and `owned_by`, with `total_count` and `links` | `ModelList`, extras | `GET /api/v1/models/{model}` answers 404 for every model, so a model check relies on the listing, which OpenRouter serves without looking at the key. |
