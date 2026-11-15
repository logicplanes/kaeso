# Fixtures: Novita AI (`novita-ai`) on `openai_chat`

Novita AI's OpenAI-compatible Chat Completions, `POST https://api.novita.ai/openai/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.novita.ai/openai`, the base URL Novita AI documents, so Kaeso's `/v1/chat/completions`, `/v1/embeddings` and `/v1/models` are the `/openai/v1/…` paths of its API reference. Model ids name their maker, such as `deepseek/deepseek-v4.1-flash`.

No key was available. The replies are examples from Novita AI's guides, completed with the reference's fields where a guide shows only part of a reply; the stream is built from the reference's schema, as Novita AI documents only one stream chunk. The model listing, one model and the key errors were answered live without a key, as Novita AI lists its models publicly.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Chat completion reference (request fields, `separate_reasoning`, `enable_thinking`, `stream_options`, the reply) | https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md |
| List models and retrieve model references | https://docs.novita.ai/api-reference/model-apis-llm-list-models.md, https://docs.novita.ai/api-reference/model-apis-llm-retrieve-model.md |
| Error codes (`INVALID_API_KEY` 403, `FAILED_TO_AUTH` 401, `MODEL_NOT_FOUND` 404, `RATE_LIMIT_EXCEEDED` 429, `SERVICE_NOT_AVAILABLE` 503) | https://docs.novita.ai/api-reference/basic-error-code.md |
| Reasoning models (`delta.reasoning_content`, `max_tokens` 4096) | https://docs.novita.ai/guides/llm-reasoning.md |
| Interleaved thinking (`reasoning_details`, the tool call and the tool result of `*-interleaved-*`) | https://docs.novita.ai/guides/llm-interleaved-thinking.md |
| Prompt cache (the usage of `responses/cached-doc.json`) | https://docs.novita.ai/guides/llm-prompt-cache.md |
| Batch API (the reply of `responses/greeting-doc.json`, inside a batch output line) | https://docs.novita.ai/guides/llm-batch-api.md |
| FAQ (`enable_thinking: false`, `separate_reasoning: true`) | https://docs.novita.ai/guides/LLM-FAQ.md |
| Exa answer reference, on the same gateway (the 429 and 503 bodies; the chat reference shows no error body) | https://docs.novita.ai/api-reference/model-apis-exa-answer.md |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json`, `responses/greeting-doc.json` | the first request of the batch guide's input file; the `chat.completion` body of the guide's batch output line, with a UUID as its id | doc |
| `requests/interleaved-tool-call-doc.json`, `responses/interleaved-tool-call-doc.json` | the guide's reply with `reasoning_content`, `reasoning_details`, `name` and a tool call, `finish_reason: "tool_calls"` and no `usage`; the request is built from the guide's later turn and the reference | doc; request built from the schema |
| `requests/interleaved-tool-result-doc.json` | the guide's next turn: `reasoning_details` sent back on the assistant turn, a `tool` message, MiniMax's `reasoning_split` | doc |
| `requests/cached-doc.json`, `responses/cached-doc.json` | the prompt cache guide's usage with 448 of 3295 input tokens cached and `cache_creation_Prompt_tokens`, `cache_read_Prompt_tokens`; the message around it is built from the reference | doc; reply built from the schema |
| `requests/reasoning-stream-doc.json`, `streams/reasoning-stream-doc.sse` | `separate_reasoning: true` and `include_usage`; `delta.reasoning_content`, then the answer, `usage: null` in every chunk and the usage in a last chunk with `choices: []` | request from the reasoning guide, with the FAQ's `separate_reasoning` and the reference's `stream_options` added; stream built from the schema |
| `requests/thinking-off-doc.json` | the FAQ's `enable_thinking: false` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `FAILED_TO_AUTH`, "failed to authenticate API key" | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 403 `INVALID_API_KEY`, "invalid API key", without OpenAI's `error` object | live |
| `errors/recorded-model-not-found.json` | 404 `MODEL_NOT_FOUND` from `GET /v1/models/{model}` | live |
| `errors/rate-limit-exceeded-doc.json`, `errors/service-unavailable-doc.json` | 429 `RATE_LIMIT_EXCEEDED` and 503 `SERVICE_UNAVAILABLE` in the same envelope, `metadata.trace_id` zeroed | doc |
| `models/list-recorded.json` | `GET /v1/models` answered without a key, cut to three models: prices in 1/10,000 USD per million tokens, `pricing`, `features`, `endpoints`, no `object: "list"` | live |
| `models/retrieve-embedding-recorded.json` | `GET /v1/models/baai/bge-m3` answered without a key: an embeddings model the default listing leaves out | live |

## Where Novita AI differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the roles are `system`, `user` and `assistant` (the guides also send `tool`); there is no `developer` | the profile sends `system` (`system_role()`) | chat completion reference |
| `max_tokens` and no `max_completion_tokens`; `max_tokens` counts reasoning tokens too | the output limit goes as `max_tokens` (`max_tokens()`) | chat completion reference, FAQ |
| no end-user field | the end user's id is left out (`without_end_user()`) | chat completion reference |
| reasoning goes into `content` unless the request sets `separate_reasoning: true`, then into `reasoning_content` | the profile sends `separate_reasoning: true` unless the app set it (`separate_reasoning()`); a reasoning block through `reasoning_fields(["reasoning_content"])` | chat completion reference, FAQ |
| `enable_thinking` turns thinking off for switchable models | an app's thinking goes as `enable_thinking` (`thinking_switch()`) | chat completion reference, FAQ |
| `video_url` content parts | sent as written (`with_video_url()`) | chat completion reference |
| no `prompt_cache_key`, `store`, `metadata`, `parallel_tool_calls`, `service_tier`, `audio`, `prediction`, `web_search_options`, `functions` or `function_call`, and nothing says whether undeclared fields are refused | refused before sending (`refusing([…])`); a Responses app's cache key is left out (`without_cache_key()`); `parallel_tool_calls: true` is left out and named in `kaeso-left-out` | chat completion reference |
| `top_k`, `min_p`, `repetition_penalty`, `separate_reasoning`, `enable_thinking`, MiniMax's `reasoning_split` | request `Extras`; refused when the request goes to another dialect | chat completion reference, interleaved thinking guide |
| `reasoning_details` and `name` on the reply message; `cache_creation_Prompt_tokens` and `cache_read_Prompt_tokens` in `prompt_tokens_details` | extras and `raw` usage, round-trip | interleaved thinking guide, prompt cache guide |
| errors as `{"code", "reason", "message", "metadata"}`, without OpenAI's `error` object; no key is 403, a wrong key 401 | `KppError` whose kind follows the status, with Novita AI's message | error codes, live |
| `GET /v1/models` answers without a key and lists chat models only; `GET /v1/models/{model}` answers for embeddings models too | the listing is partial (`ModelListing::Partial`); a model check reads `GET /v1/models/{model}` and can't spot a wrong key | live |
