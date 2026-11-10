# Fixtures: SambaNova (`sambanova`) on `openai_chat`

SambaNova's SambaCloud, OpenAI-compatible Chat Completions at `POST https://api.sambanova.ai/v1/chat/completions`, with `Authorization: Bearer` (the same key also works as `x-api-key`). Kaeso's base URL is `https://api.sambanova.ai`.

No key was available. The files named `…-doc` are the examples of SambaNova's OpenAPI document and text generation guide, unchanged but for one `created`; `streams/reasoning-stream.sse` is built from the same schema. The model listing was answered live without a key, as SambaCloud lists its models publicly; the errors were answered live without a valid key, and their request ids are replaced with zeros.

## Sources (read 2026-11-09)

| Source | Address |
|---|---|
| OpenAPI document (`openapi.documented.json`, SambaNova cloud API 1.2.0) | https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json |
| Create chat-based completion | https://docs.sambanova.ai/docs/api-reference/chat-completions/create-chat-based-completion |
| OpenAI compatibility (`presence_penalty` and `frequency_penalty` ignored, `n`, `seed`, `logit_bias`) | https://docs.sambanova.ai/docs/en/features/openai-compatibility |
| Text generation (`reasoning`, `chat_template_kwargs.enable_thinking`) | https://docs.sambanova.ai/docs/en/features/text-generation |
| Video input (`video_url`) | https://docs.sambanova.ai/docs/en/features/video |
| API error codes | https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes |
| Embeddings and audio, SambaStack only | https://docs.sambanova.ai/docs/en/features/embeddings, https://docs.sambanova.ai/docs/en/features/audio |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/palindrome-poem-doc.json`, `responses/palindrome-poem-doc.json` | the reference's example: a float `created`, `system_fingerprint: "fastcoe"`, and SambaNova's timing fields in `usage` (`acceptance_rate`, `time_to_first_token`, `total_latency`, `is_last_response` and the rest) | doc |
| `requests/stream-length-doc.json`, `streams/stream-length-doc.sse` | the reference's example stream: `finish_reason: "length"`, then a last chunk with `choices: []` and the usage with its timing fields | doc |
| `requests/thinking-tool-call-doc.json`, `responses/thinking-tool-call-doc.json` | the guide's example for `gemma-4-31B-it` with `chat_template_kwargs.enable_thinking`: `reasoning` beside a tool call; the example leaves out `created`, which the schema requires and the encoder always writes, so the fixture adds it | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | `gpt-oss-120b` with `reasoning_effort`; `delta.reasoning`, then the answer and the timing usage | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `authentication_error`, `code: "invalid_api_key"`; SambaNova masks the key in the message itself | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 without a key | live |
| `errors/recorded-embeddings-model-not-found.json` | 404 `model_not_found` from `/v1/embeddings` for `E5-Mistral-7B-Instruct`, which SambaCloud doesn't serve; it answers before checking the key | live |
| `errors/top-p-above-max-doc.json` | 400 `decimal_above_max_value` with `param: "top_p"` | doc |
| `models/list-recorded.json`, `models/retrieve-recorded.json` | `GET /v1/models` and `GET /v1/models/gpt-oss-120b`, answered without a key: `context_length`, `max_completion_tokens`, `pricing` per token as strings, `sn_metadata` | live |

## Where SambaNova differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| reasoning comes as `message.reasoning` and `delta.reasoning` (with `channel` from `gpt-oss-120b`), and assistant messages take `reasoning` back | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])` | OpenAPI document; text generation guide |
| DeepSeek-V3.2 and Gemma 4 think only with `chat_template_kwargs.enable_thinking: true` | a Chat app's `chat_template_kwargs` is a request extra and goes as written; other formats can't set it | text generation guide |
| `presence_penalty` and `frequency_penalty` are accepted and ignored | refused before sending (`refusing(["frequency_penalty", "presence_penalty"])`) | OpenAI compatibility |
| the request schema has no `user`, `safety_identifier`, `metadata`, `store` or `prompt_cache_key`, and refuses other properties | the end user's id is left out (`without_end_user()`), and a Responses app's cache key too (`without_cache_key()`); any other undeclared field goes as written, for SambaNova's own 400 | OpenAPI document (`additionalProperties: false`) |
| `video_url` content parts | sent as written (`with_video_url()`) | video input |
| `top_k`, `repetition_penalty`, `chat_template_kwargs` | request `Extras`; refused when the request goes to another dialect | OpenAPI document |
| a float `created`, `system_fingerprint`, the timing fields in `usage` | extras and `raw` usage, round-trip | OpenAPI document |
| errors carry OpenAI's `error` object and a top-level `request_id`; 410 `model_deprecated` for a retired model | `KppError`; the kind follows the status, so 401 is `Authentication` and 410 `NotFound` | error codes; live |
| `GET /v1/models` answers without a key | a model check can't spot a wrong key | live |
| embeddings and Whisper transcription are SambaStack features; SambaCloud answers `model_not_found` for them | `embeddings` is `None` and `transcriptions` is empty | embeddings and audio pages; live |
| rate limits as `x-ratelimit-*-requests` and `-requests-day` headers | not read | rate limits |
