---
title: Chat Completions providers
description: How the Chat codec adapts its requests to each provider that speaks the format, and what each one adds.
order: 6
nav: Chat providers
---

Other providers than OpenAI speak [OpenAI Chat Completions](openai-chat.md) with differences of their own. Look up here how the codec adapts each request to a provider, and what each provider adds, before you add a provider or change its profile.

## Provider profiles

Other providers speak this format with differences of their own. Each provider in `PROVIDERS` (`crates/kaeso-core/src/provider.rs`) names a `kaeso_codecs::openai_chat::ChatProfile` in its `chat` field. The profile holds the provider's name and the settings for the requests Kaeso sends it (`ChatProfile::request`, a `RequestProfile`), set with the builders below.

The gateway hands the profile to the encoder with `EgressCtx::with_chat_profile`, which applies it to every request, whatever format the app sent. `ChatProfile::OPENAI`, the default, changes nothing, and neither does `ChatProfile::compatible(name)` until a setting is added.

| Setting | What the encoder does | Why it is allowed |
|---|---|---|
| `max_tokens()` | Sends the output limit as `max_tokens`, and a `max_completion_tokens: null` as `max_tokens: null`. A Chat request with both at different values is `Unsupported` (`max_tokens`). | Only the spelling changes; the value is the same. |
| `system_role()` | Sends `developer` messages as `system` messages. | The role spelling is form; both carry the instructions. |
| `random_seed()` | Sends `seed` as `random_seed`, a `seed: null` too. A negative seed, or a request with both `seed` and `random_seed` (either may be `null`), is `Unsupported` (`seed`). The rule follows Mistral's `random_seed`, which its OpenAPI document gives `minimum: 0`. | `seed` stays semantic: it is renamed or refused, never dropped. |
| `without_end_user()` | Leaves KPP `user` out instead of sending it as `user` or `safety_identifier`, together with a second `user` a Chat app sent alongside `safety_identifier`, and their `null` spellings. | `user` is a hint ([field classes](index.md#field-classes)). |
| `without_prefill()` | Refuses a conversation that ends with an assistant message, unless it calls tools or is a Chat app's message with `prefix: true`: `Unsupported` (`assistant prefill`, param `messages[i]`). | A provider that only continues such a message with `prefix: true` repeats the prefill in its reply, so the app would not get a continuation. |
| `with_message_bookkeeping()` | Sends the bookkeeping keys of a Chat app's assistant messages, `cross::CHAT_ASSISTANT_BOOKKEEPING` (today Grok CLI's `model_id`, also as `null`). Without it, the encoder leaves them out. | The model never reads them, and most Chat providers refuse a message field they don't declare (Groq, Mistral, and OpenAI-compatible proxies), so only a provider known to take them gets them. |
| `greedy_top_p()` | Sends `top_p: 1` whenever the request has `temperature: 0`, whatever `top_p` it set. | At `temperature: 0` the model always takes the most likely token, so `top_p` changes nothing and the app's value is not lost; a provider whose default `top_p` is lower refuses greedy sampling without `top_p: 1`. |
| `refusing(fields)` | Refuses a request whose body would contain one of these top-level keys, such as `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | Extras are semantic, so they are refused, not dropped. |
| `refusing_in_messages(fields)` | The same for keys of system, user and assistant messages. Tool messages are not checked. | As above. |

Settings for the replies Kaeso reads (`ChatProfile::reply`, a `ReplyProfile`):

| Setting | What the decoder does | Why it is allowed |
|---|---|---|
| `thinking_parts()` | Reads each part of a `content` array on its own: a text part is text, a `thinking` part (`{type: "thinking", thinking: [chunks], ...}`) a `Block::Reasoning` whose `text` holds its text chunks, with its other keys (`closed`, `signature`) and, when it holds anything but text, its whole `thinking` array in form `thinking`. Other parts (`reference`) are `Block::Unknown`. Streamed thinking becomes `BlockStart(Reasoning)` and `BlockDelta(ReasoningText)`; the reasoning block ends when the answer, a refusal or a tool call starts, and a stream's empty text pieces before the first real one open no text block (form `et`). | Chat clients read `content` as a string, so Chat apps never get an array: the text is `content`, the thinking `reasoning_content` (several thinking parts joined by a blank line, none when empty), as from DeepSeek. Other parts, which Chat has no field for, stay in the logged reply only; a stream chunk that carries nothing readable, such as the empty thinking part that closes Mistral's thinking, reaches them without `content`. Text and thinking keep no order between them. Replayed reasoning of a Chat app (`reasoning_content`, `reasoning`, `reasoning_details`) goes back to the provider as `thinking` parts before the text, as do thinking blocks an app in another format sends back; reasoning without readable text is left out. A `thinking` part's `closed` and references don't survive a trip through another format, because a reasoning blob keeps no form. Other formats get the thinking in their own reasoning shape with the original block in a [reasoning blob](reasoning.md#reasoning-blobs). |
| `reasoning_fields(fields)` | Reads the first of these message keys that holds a non-empty string, such as DeepSeek's `reasoning_content` or Groq's `reasoning`, as a `Block::Reasoning` before the reply's other blocks, with form `reasoning_field` naming the key. In a stream, that key of each delta becomes `BlockDelta(ReasoningText)` of one reasoning block, with form `rf` on the chunk. The reasoning block ends when the answer, a refusal or a tool call starts, and empty text pieces before the first real one open no text block (form `et`). An empty string, `null` or another type stays an extra. | Nothing is dropped: Chat apps get the key back as sent, reasoning that an app in another format sends back reaches the provider in the first of these keys, the delta's other keys (Groq's `channel`) stay extras of the chunk, and other formats get the reasoning in their own reasoning shape with the original block in a [reasoning blob](reasoning.md#reasoning-blobs). Without the setting, every streamed piece of reasoning would be a chunk without a KPP event, logged as an unknown block of its own. |
| `error_detail(path)` | Appends the string at this path of an error body, such as OpenRouter's `error.metadata.raw`, to the error's message: `<message>: <detail>`. A missing, empty or non-string value, or one the message already contains, changes nothing. Stream errors get the same. | The body stays as it was: Chat apps get the provider's error unchanged, and other formats get the reason in the message, where a generic one (`Provider returned error`) would tell them nothing. The gateway takes key-like tokens out of every provider error message. |

`refusing` and `refusing_in_messages` each take the whole list; a second call replaces the first. A refusal names the field as the app wrote it. It names a Chat app's key by its own name and path (`logit_bias`, `messages[1].name`), and a refusal block from another format by its KPP path (`messages[1].blocks[0]`).

Refusals happen while the request is encoded, on the request the encoder built and before it is written out. Nothing is sent, and the refusal reaches the app in its own format.

Each built-in provider whose profile differs from OpenAI's is listed below with the source of every setting: the provider's documentation, its OpenAPI document or a live recording in `fixtures/openai_chat/providers/<id>/`.

| Provider | Settings | Sources |
|---|---|---|
| DeepSeek (`deepseek`) | `max_tokens()`, `system_role()`, `reasoning_fields(["reasoning_content"])` | DeepSeek's API reference and thinking mode guide (`reasoning_content`), `responses/recorded-thinking-reasoning-content` and `streams/recorded-thinking-stream`; `fixtures/openai_chat/providers/deepseek/responses/recorded-max-completion-tokens-limit`: DeepSeek ignores `max_completion_tokens` (20 asked, 133 completion tokens), while its API reference documents `max_tokens`; `fixtures/openai_chat/providers/deepseek/errors/recorded-plain-developer-string`: DeepSeek answers the `developer` role with 422 `unknown variant` |
| Groq (`groq`) | `system_role()`, `reasoning_fields(["reasoning"])`; refuses `logit_bias`, `logprobs` and `top_logprobs`, and `name` and `refusal` on messages | Groq's OpenAI compatibility page (https://console.groq.com/docs/openai, which lists `logprobs`, `logit_bias`, `top_logprobs` and `messages[].name` as unsupported); `fixtures/openai_chat/providers/groq/errors/`: `recorded-logit-bias`, `recorded-logprobs-top-logprobs-doc`, `recorded-tools-results-parallel-error` (`refusal` on an assistant message) and `recorded-tools-single` (Qwen's chat template refuses the `developer` role); Groq's reasoning guide (`reasoning`), `responses/recorded-reasoning-format-parsed` and `streams/recorded-stream-doc` |
| Mistral (`mistral`) | `max_tokens()`, `system_role()`, `random_seed()`, `without_end_user()`, `without_prefill()`, `greedy_top_p()`, `thinking_parts()`; refuses `audio`, `function_call`, `functions`, `logit_bias`, `moderation`, `prompt_cache_options`, `prompt_cache_retention`, `store`, `verbosity` and `web_search_options`, and `name` and `refusal` on messages | Mistral's OpenAPI document (https://docs.mistral.ai/openapi.yaml, read on 2026-10-19), where `ChatCompletionRequest` and its message types such as `AssistantMessage` forbid undeclared fields and gives `random_seed` `minimum: 0`; the `422 extra_forbidden` recordings in `fixtures/openai_chat/providers/mistral/errors/`, one per refused or renamed field; `responses/recorded-audio-field`, where Mistral takes `audio` and answers with text only; and `recorded-assistant-prefill` (400 `invalid_request_message_order`) with `responses/recorded-assistant-prefix`, whose reply repeats the prefill; `responses/recorded-reasoning-effort-high` and `streams/recorded-reasoning-effort-high-stream` for `thinking_parts()`; `errors/greedy-sampling` (400 `invalid_request_greedy_sampling`) for `greedy_top_p()` |
| OpenRouter (`openrouter`) | `reasoning_fields(["reasoning"])`, `error_detail(["error", "metadata", "raw"])` | OpenRouter's reasoning tokens guide (`reasoning`, with `reasoning_details` beside it); `responses/recorded-free-reasoning` and `streams/recorded-free-reasoning-stream`; OpenRouter's errors and debugging page (`metadata.raw`, the upstream provider's error), `errors/recorded-upstream-rate-limited` and `errors/recorded-routing-fields` |
| SpaceX (`xai`) | `reasoning_fields(["reasoning_content"])`, `with_message_bookkeeping()` | xAI's Chat Completions reference (`reasoning_content`); Grok CLI, xAI's own client, sends `model_id` to it on every assistant turn (`requests/grok-cli-model-id.json`); `responses/recorded-reasoning-content` and `streams/recorded-reasoning-content-stream` |
| Together AI (`togetherai`) | `reasoning_fields(["reasoning_content", "reasoning"])` | Together AI's chat completions reference; the key depends on the model: `responses/recorded-sampling-extras` (`reasoning_content`), `responses/recorded-reasoning-reply` and `streams/recorded-reasoning-stream` (`reasoning`) |

## Providers that use this format

Providers other than OpenAI that serve Chat Completions record into `fixtures/openai_chat/providers/<id>/` ([fixtures](fixtures.md)). Their extra fields follow the rules above; this section names what each one adds.

### DeepSeek

Sources: DeepSeek's [API reference](https://api-docs.deepseek.com/api/create-chat-completion), [thinking mode guide](https://api-docs.deepseek.com/guides/thinking_mode/) and [error codes](https://api-docs.deepseek.com/quick_start/error_codes), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/deepseek/`.

| DeepSeek field | KPP | Notes |
|---|---|---|
| `thinking` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged, and an assistant turn of a Chat app that carries one sends it back to DeepSeek. Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item; when they send it back, it returns to DeepSeek as `reasoning_content` ([reasoning](reasoning.md)), which its thinking mode expects on tool-call turns. |
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
| an `error` object after the 200, in the body or in a chunk beside `choices` | `KppError` | See the [rows above](openai-chat.md#streams-createchatcompletionstreamresponse); a numeric `code` decides the kind as a status would. |
| `GET /api/v1/models`: no `object`, models without `object` and `owned_by`, with `total_count` and `links` | `ModelList`, extras | `GET /api/v1/models/{model}` answers 404 for every model, so a model check relies on the listing, which OpenRouter serves without looking at the key. |
