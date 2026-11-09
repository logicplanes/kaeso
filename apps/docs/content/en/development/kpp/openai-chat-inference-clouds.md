---
title: Chat Completions inference clouds
description: What the inference clouds that host open models add to Chat Completions, and how the codec meets them.
order: 6
nav: Inference clouds
---

Inference clouds that host open models speak [OpenAI Chat Completions](openai-chat.md) with differences of their own. Look up here what each one adds before you change its profile; their profiles are in the table of [provider profiles](openai-chat-providers.md#provider-profiles), and the other providers are on [Chat Completions providers](openai-chat-providers.md).

Their fixtures are in `fixtures/openai_chat/providers/<id>/` ([fixtures](fixtures.md)).

## Fireworks AI

Sources: Fireworks' [Create Chat Completion](https://docs.fireworks.ai/api-reference/post-chatcompletions) reference (its OpenAPI document), [OpenAI compatibility](https://docs.fireworks.ai/tools-sdks/openai-compatibility) and [reasoning](https://docs.fireworks.ai/guides/reasoning) pages, read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/fireworks-ai/` are built from them; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.fireworks.ai/inference`, which takes `Authorization: Bearer` with a `fw_` key.

| Fireworks field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Thinking that an app in another format sends back reaches Fireworks as `reasoning_content`, which its reasoning guide asks for during tool calls. The documentation describes no `<think>` tags in `content`. |
| `reasoning_effort` with `xhigh`, `max`, booleans or a token count; `thinking` in Anthropic's form; `reasoning_history` | `reasoning.effort`, extras | The catalogue's effort levels decide what other formats send; a Chat app's own values go as written. |
| `top_k`, `min_p`, `typical_p`, `repetition_penalty`, `mirostat_*`, `context_length_exceeded_behavior`, `prompt_cache_isolation_key`, `perf_metrics_in_response`, `raw_output` and Fireworks' other request fields | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `max_tokens` beyond what the context window leaves | sent | Fireworks lowers it instead of refusing it, unless the request sets `context_length_exceeded_behavior: "error"`. |
| `user` | `user` | Fireworks' schema has no `safety_identifier`, so the profile sends the end user's id as `user` (`end_user_as_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()`; Fireworks' video models take them (`requests/video-url`). |
| `perf_metrics` on a reply or the finish chunk | extras | Left out towards other dialects. |
| usage by default in a trailing chunk with `choices: []` | `Usage` | Also without `stream_options.include_usage`. |
| errors in OpenAI's `error` object with an upper-case `code` such as `UNAUTHORIZED`, `type: "error"` and a top-level `request_id` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | A wrong key gets 401 (`errors/list-invalid-api-key`), so a model check spots it. Fireworks documents no listing on this path; the console takes its models from KPMR. |

Fireworks documents no image, speech, transcription, video or token-count endpoint, so those stay empty and `count_tokens` is `None`. Its Responses API reports usage as `prompt_tokens` and `completion_tokens` instead of OpenAI's names, so Kaeso doesn't send Responses to it; Responses apps reach Fireworks through Chat Completions.

## Nebius Token Factory

Sources: Nebius' [OpenAPI document](https://api.tokenfactory.nebius.com/openapi.json) (version `20261105-4c58718c5`), its [text generation](https://docs.tokenfactory.nebius.com/api-reference/examples/text-generation) example and its [overview](https://docs.tokenfactory.nebius.com/ai-models-inference/overview), read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/nebius/` are built from them, and one is the documentation's own example; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.tokenfactory.nebius.com`, which takes `Authorization: Bearer`.

| Nebius field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content`, `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`; the field depends on the model, as for Together AI. |
| the `developer` role | `Role::System` | Nebius lists only `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| `user` | `user` | Nebius' schema has no `safety_identifier`, so the end user's id goes as `user` (`end_user_as_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()` (`requests/video-url`). |
| vLLM sampling fields such as `top_k` and `repetition_penalty`, `extra_body`, `partial` and Kimi's `tools` on messages | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `service_tier` on every reply, `stop_reason`, `function_call: null`, `tool_calls: []`, `system_fingerprint: null` | extras | Left out towards other dialects (`responses/text-generation-doc`). |
| `response_format` with a JSON schema | sent | Nebius' description allows only `json_object` and `text`; Nebius answers a schema itself. |
| errors with OpenAI's `error` object and a top-level `detail` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | A wrong key gets 401 with only `{"detail": "…"}` (`errors/list-invalid-api-key`), which apps get in OpenAI's shape with Nebius' message, so a model check spots it. |

Nebius serves no images, audio, Messages or token counts: its OpenAPI document has no such path, and `/v1/images/generations`, `/v1/audio/transcriptions` and `/v1/messages` answer 404. Its Responses API isn't used: Responses apps reach Nebius through Chat Completions.

## SambaNova

Sources: SambaNova's [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) (SambaNova cloud API 1.2.0), its [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility), [text generation](https://docs.sambanova.ai/docs/en/features/text-generation) and [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes) pages, read 2026-11-09. No key was available: `fixtures/openai_chat/providers/sambanova/` holds the document's own example reply and stream and replies built from it, the model listing as SambaCloud serves it without a key, and errors answered live without a valid key. Kaeso's base URL is `https://api.sambanova.ai`, which takes `Authorization: Bearer`.

| SambaNova field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `delta.reasoning`, `channel` | `Block::Reasoning`, extras | Through the profile's `reasoning_fields`. Assistant messages take `reasoning` back. |
| `chat_template_kwargs` (`enable_thinking`), `top_k`, `repetition_penalty` | extras | DeepSeek-V3.2 and Gemma 4 think only with `enable_thinking`, which only a Chat app can send. Refused when the request goes to another dialect. |
| `frequency_penalty`, `presence_penalty` | refused | SambaNova accepts and ignores them, so the profile refuses them before sending. |
| an end user's id, a Responses app's `prompt_cache_key` | left out | The request schema has neither and refuses undeclared properties (`without_end_user()`, `without_cache_key()`). Other undeclared fields go as written, for SambaNova's own 400. |
| `video_url` parts | `Block::Video` | `with_video_url()`; Gemma 4 takes video. |
| a float `created`, `system_fingerprint` | extras | `responses/palindrome-poem-doc`. |
| `usage` with SambaNova's timing fields (`time_to_first_token`, `total_latency`, `acceptance_rate`, `is_last_response` and others) | `Usage`, `raw` usage | In replies and in the last chunk of a stream, which has `choices: []` (`streams/stream-length-doc`). |
| errors with OpenAI's `error` object and a top-level `request_id`; 410 `model_deprecated` | `KppError` | The kind follows the status: 401 is a refused key, 410 not found. |
| `GET /v1/models`, `GET /v1/models/{id}` | `ModelList`, `ModelInfo` | Answered without a key, with `context_length`, `max_completion_tokens` and `pricing` (`models/list-recorded`), so a model check can't spot a wrong key. |

SambaCloud serves no embeddings and no Whisper; SambaNova documents both for SambaStack only, and SambaCloud answers `model_not_found` for their models (`errors/recorded-embeddings-model-not-found`). Its Responses API isn't used: Responses apps reach SambaNova through Chat Completions.

## Scaleway

Sources: Scaleway's [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/), its [reasoning guide](https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/), [audio guide](https://www.scaleway.com/en/docs/generative-apis/how-to/query-audio-models/) and [errors page](https://www.scaleway.com/en/docs/generative-apis/api-cli/understanding-errors/), read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/scaleway/` are built from them, the stream's first chunks from the reasoning guide's own example; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.scaleway.ai`, which takes `Authorization: Bearer` with an IAM secret key and reaches the key's default Project.

| Scaleway field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Only legacy models put thinking in `<think>` tags in `content`, and those have reached end of life. |
| the `developer` role | `Role::System` | Scaleway lists only `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| an end user's id | left out | The request schema has no `user` or `safety_identifier` (`without_end_user()`). |
| `input_audio` parts in `wav` or `mp3` | `Block::Audio` | As OpenAI's (`requests/input-audio`). |
| `token_ids` on stream choices | extras | Left out towards other dialects. |
| errors with the HTTP status as a number in `error.code`, `type` such as `unauthorized` or `forbidden`, and `param: ""` | `KppError` | The kind follows the status. A key Scaleway doesn't know gets 403 `forbidden` (`errors/recorded-invalid-api-key`), like a key without a Generative APIs permission, so apps get it as a key that isn't allowed the request. |
| the legacy error shape, `{"object": "error", "message", "type", "param", "code"}` | `KppError` | Older models still send it (`errors/legacy-model-error-doc`); apps get an OpenAI error with its message. |

Scaleway serves no Messages, images, speech or token counts: `/v1/messages`, `/v1/images/generations` and `/v1/audio/speech` answer 404 `route_not_found`. Its Responses API, stateless and with function tools only, isn't used: Responses apps reach Scaleway through Chat Completions.
