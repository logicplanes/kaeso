# Fixtures: Volcengine Ark (`volcengine`) on `openai_chat`

Volcengine Ark's (火山方舟) OpenAI-compatible Chat Completions, `POST https://ark.cn-beijing.volces.com/api/v3/chat/completions`, with `Authorization: Bearer`. Ark's API sits at `/api/v3`, not `/v1`: Kaeso's base URL is `https://ark.cn-beijing.volces.com/api/v3` and the provider's API root is empty (`api_root: Some("")`), so every path loses its `/v1` (`/chat/completions`, `/models`). Model ids are lower case with a date, such as `doubao-seed-2-1-pro-260915`; a request may name an inference endpoint (`ep-…`) instead.

No key was available. The replies, the stream and the model listing are the samples of Ark's Chat API reference and deep thinking guide; `streams/reasoning-stream-built.sse` is a stream Ark sent, published in another project's issue, completed with the finishing chunk the issue leaves out. The key errors were answered live without a valid key, with the request ids zeroed.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Base URL and authentication | https://docs.volcengine.com/docs/zh/ark/base-url-and-authentication.md |
| Chat API reference (request fields, replies, the samples of `responses/` and `streams/greeting-stream-doc.sse`; updated 2026-11-12) | https://docs.volcengine.com/docs/zh/ark/chat-api.md |
| Schema upgrade notice (`developer` role accepted, `prompt_cache_key`) | https://docs.volcengine.com/docs/zh/ark/api-schema-upgrade-notice.md |
| Deep thinking (`thinking`, `reasoning_effort` per model, `encrypted_content` sent back in tool calls) | https://docs.volcengine.com/docs/zh/ark/deep-thinking.md |
| Model listing (`GET /models`, `list-doc.json`) | https://docs.volcengine.com/docs/zh/ark/models-api.md |
| Error codes | https://docs.volcengine.com/docs/zh/ark/error-codes.md |
| Multimodal embeddings (`/embeddings/multimodal`) | https://docs.volcengine.com/docs/zh/ark/multimodal-vectorization-api.md |
| A stream of `doubao-seed-1-6-thinking-250715` (`streams/reasoning-stream-built.sse`) | https://github.com/spring-projects/spring-ai/issues/4296 |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json`, `responses/greeting-doc.json` | a system and a user message; the reply's id without `chatcmpl-`, `service_tier: "default"`, `logprobs: null`, `cached_tokens` and `reasoning_tokens` | doc (默认) |
| `requests/reasoning-doc.json`, `responses/reasoning-doc.json` | thinking on by default: `reasoning_content` beside the answer, 419 reasoning tokens | doc (深度推理; the doc elides parts of both texts with `***`) |
| `responses/audio-encrypted-content-doc.json` | `reasoning_content` and `encrypted_content` on the message, `prompt_tokens_details.audio_tokens` | doc (音频理解) |
| `requests/web-search-doc.json`, `responses/web-search-doc.json` | `web_search_options: {}` and `max_completion_tokens`; `url_citation` annotations without `start_index`/`end_index`, `usage.tool_usage.web_search` | doc (联网搜索) |
| `requests/prefill-doc.json`, `responses/prefill-doc.json` | a trailing assistant message that the reply continues | doc (续写模式) |
| `requests/thinking-disabled-doc.json` | `thinking: {"type": "disabled"}` from a Chat app | doc (deep thinking guide) |
| `requests/tools-encrypted-content-doc.json`, `responses/tools-encrypted-content-doc.json` | a tool call with `reasoning_content` and `encrypted_content`, and arguments that start with a space | doc (deep thinking guide; the guide elides the reply after `choices`, so `id`, `created`, `model`, `service_tier`, `object` and `usage` are built from the schema) |
| `requests/tools-encrypted-content-replay-doc.json` | the next turn: the assistant message with its `reasoning_content` and `encrypted_content` sent back unchanged, then the tool result | doc (deep thinking guide) |
| `requests/greeting-stream-doc.json`, `streams/greeting-stream-doc.sse` | a stream whose chunks repeat `role`, carry `usage: null` and no `finish_reason` until the last, which ends with `content: ""` | doc (流式输出; the doc prints the chunks without `data: `, added here) |
| `requests/reasoning-stream-built.json`, `streams/reasoning-stream-built.sse` | `thinking: {"type": "enabled"}` and `include_usage`; `reasoning_content` deltas beside `content: ""`, then the answer, then a usage chunk with `choices: []` and no `service_tier` | published, completed: the request and every chunk but one are the issue's, with `data: ` added; the issue elides chunks with `...`, among them the one that sets `finish_reason`, which is built after the reference's stream sample (`content: ""`, `finish_reason: "stop"`) |
| `requests/recorded-*-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `AuthenticationError`, `type: "Unauthorized"`: "The API key format is incorrect." | live, with a made-up key |
| `errors/recorded-unknown-api-key.json` | 401: "The API key doesn't exist." | live, with a made-up key shaped like a UUID |
| `errors/recorded-missing-api-key.json` | 401: "the API key or AK/SK in the request is missing or invalid." | live, without a key |
| `models/list-doc.json` | `GET /models`: no `owned_by`, and Ark's `name`, `version`, `domain`, `task_type`, `token_limits`, `modalities`, `features` and `max_output_tokens` | doc |

## Where Volcengine Ark differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the API sits at `/api/v3` (`…/chat/completions`, `…/models`), not `/v1` | the provider's API root, `api_root: Some("")`, takes the place of `/v1`; the base URL ends at `/api/v3` | base URL and authentication |
| `developer` messages are accepted since the 2026-09 schema upgrade | sent as `developer` | schema upgrade notice |
| `max_tokens` limits only the answer on most models (thinking and answer on `deepseek-v4-1-flash-*`); `max_completion_tokens` limits both, at most 65536; the two can't go together | sent as the app wrote it; Kaeso's own limit goes as `max_completion_tokens`, which keeps an Anthropic or Responses app's limit meaning thinking and answer. The provider has `reasoning_beyond_output_limit`, so a Chat app's `max_tokens` doesn't bound the cost estimate | Chat API reference |
| no `user` or `safety_identifier` | the end user is not sent (`without_end_user()`) | Chat API reference |
| thinking is switched with `thinking: {"type": "enabled" \| "disabled"}`, on by default; `reasoning_effort` takes `none` to `max`, mapped per model (`minimal` turns Seed's thinking off but is `low` on DeepSeek V4.1 Flash) | `thinking_type()`: thinking turned off, or an effort of `none` or `minimal`, goes as `{"type": "disabled"}` without an effort; thinking turned on without an effort as `{"type": "enabled"}` beside the effort a budget maps to; an effort alone as `reasoning_effort`. A Chat app's own `thinking` goes as written | deep thinking guide |
| reasoning comes as `reasoning_content` on the message and the delta | a reasoning block (`reasoning_fields(["reasoning_content"])`) | Chat API reference; published stream |
| newer Seed models add `encrypted_content`, which must go back unchanged in multi-turn tool calls; in a stream it comes in a chunk of its own after the thinking | `encrypted_reasoning_field("encrypted_content")`: a Chat app gets and sends it as written; for the other formats the reasoning block carries it (an `openai_encrypted_reasoning` opaque of Chat Completions origin, inside Anthropic's signature or Responses' `encrypted_content`) and Kaeso writes it back on the assistant message | deep thinking guide |
| `service_tier` takes `auto`, `default`, `fast` and `flex`; replies name `scale`, `default`, `fast` or `flex` | sent as written; OpenAI's `priority` and `scale` are refused (`refusing_values`) | Chat API reference |
| `web_search_options` works with Doubao Search only; its `search_context_size` and `user_location` have no effect | `{}` and the other keys go as written; those two are refused | Chat API reference |
| `video_url` parts, with `fps` | sent as written (`with_video_url()`) | Chat API reference |
| the schema has no `seed`, `n`, `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_retention`, `functions` or `function_call`, and doesn't say whether undeclared fields are refused | refused before sending | Chat API reference; compatibility page |
| a trailing assistant message is continued (续写模式) | sent as written | Chat API reference |
| usage adds `audio_tokens`, `audio_cached_tokens` and `tool_usage.web_search`; replies add `service_status.model_fallback` and `choices[].moderation_hit_type` | `raw` usage and extras, round-trip | Chat API reference |
| errors carry OpenAI's `error` object with Ark's `code` and an HTTP-like `type` (`Unauthorized`, `BadRequest`), and the message ends with the request id | `KppError` from the status; the message as sent | live |
| `GET /models` needs a key, lists the base model versions the account sees but not its endpoints (`ep-…`), and has no `owned_by`; no `GET /models/{model}` is documented | the listing is partial (`ModelListing::Partial`); Ark's fields stay as extras | model listing reference |
| embeddings only at `/embeddings/multimodal`, one vector for the whole input | not used: the provider has no embeddings dialect | multimodal embeddings reference |
