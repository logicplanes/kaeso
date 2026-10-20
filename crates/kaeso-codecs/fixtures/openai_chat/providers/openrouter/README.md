# Fixtures: OpenRouter (`openrouter`) on `openai_chat`

OpenRouter's Chat Completions API, `POST https://openrouter.ai/api/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://openrouter.ai/api`.

## Sources (read 2026-10-19)

| Source | Address |
|---|---|
| API reference | https://openrouter.ai/docs/api_reference/overview |
| Streaming, keep-alive comments and mid-stream errors | https://openrouter.ai/docs/api_reference/streaming |
| Error bodies and typed error codes | https://openrouter.ai/docs/api_reference/errors-and-debugging |
| Reasoning tokens (`reasoning`, `reasoning_details`) | https://openrouter.ai/docs/guides/best-practices/reasoning-tokens |
| Usage accounting (`usage.cost`, `cost_details`) | https://openrouter.ai/docs/cookbook/administration/usage-accounting |
| Provider routing (`provider`) and model fallbacks (`models`) | https://openrouter.ai/docs/guides/routing/provider-selection, https://openrouter.ai/docs/guides/routing/model-fallbacks |
| Models API | https://openrouter.ai/docs/guides/overview/models |

Every live recording used a free model (an id ending in `:free`) and is listed with its model in the `RECORDED.md` of its folder. Error bodies carry the account's `user_id`; it is replaced by `user_000000000000000000000000000` and nothing else is changed.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/free-basic.json` | A system and a user message with `max_tokens` | written for recording |
| `requests/free-stream.json` | A stream with `include_usage` and `reasoning: {exclude: true}` | written for recording |
| `requests/keepalive-stream.json` | A stream from a large model, which keeps the connection with `: OPENROUTER PROCESSING` comments | written for recording |
| `requests/free-tools.json` | One function tool, `tool_choice: "auto"` | written for recording |
| `requests/free-reasoning.json`, `requests/free-reasoning-stream.json` | The `reasoning` request object (`effort: "low"`), as a reply and as a stream | written for recording |
| `requests/routing-fields.json` | The routing fields with no KPP field: `models` (fallback list), `provider` (routing preferences), `transforms`, `usage: {include: true}`; model ids with `/` and `:` | written for recording |
| `requests/free-stream-rate-limited.json` | The earlier `free-stream.json`, which `errors/recorded-upstream-rate-limited.json` was recorded with: `liquid/lfm-2.5-2.6b:free`, no `reasoning.exclude`, `max_tokens: 256` | written for recording |
| `requests/unknown-model.json`, `requests/invalid-messages.json` | Requests OpenRouter refuses with 400 | written for recording |
| `responses/recorded-free-basic.json` | A reply with `provider`, `native_finish_reason`, `message.reasoning`, `message.reasoning_details` (`reasoning.text`), `usage.cost`, `usage.is_byok`, `usage.cost_details`, `prompt_tokens_details.cache_write_tokens` and `video_tokens`, `completion_tokens_details.image_tokens` | live, `liquid/lfm-2.5-2.6b:free` |
| `responses/recorded-free-tools.json` | A tool call with reasoning beside it | live, `nvidia/nemotron-3.5-lightning:free` |
| `responses/recorded-free-reasoning.json` | A reasoning reply | live, `nvidia/nemotron-3.5-lightning:free` |
| `streams/recorded-free-stream.sse` | A stream with `reasoning.exclude`: text only, while `completion_tokens_details.reasoning_tokens` still counts the hidden reasoning; the last chunk repeats the finish choice beside `usage` instead of OpenAI's empty `choices` | live, `nvidia/nemotron-3.5-lightning:free` |
| `streams/recorded-keepalive-stream.sse` | `: OPENROUTER PROCESSING` comments before the first chunk, then reasoning deltas with `reasoning_details` | live, `nvidia/nemotron-3-ultra-550b-a55b:free` |
| `streams/recorded-free-reasoning-stream.sse` | Reasoning deltas, then content | live, `nvidia/nemotron-3-ultra-550b-a55b:free` |
| `streams/doc-error-mid-stream.sse` | An error after the 200: a chunk with a top-level `error` (`code` a number, `metadata.error_type`) and `finish_reason: "error"`, with no `[DONE]` after it | doc: errors-and-debugging, "Example SSE data", verbatim |
| `streams/doc-error-mid-stream-string-code.sse` | The same with `code` as a string (`"server_error"`), as the streaming page shows it | doc: streaming, "Errors after the response is committed", verbatim |
| `errors/recorded-routing-fields.json` | 429 from the upstream provider after the `models` fallback was tried too: `metadata.raw`, `provider_name`, `is_byok`, `provider_error_code`, `limit_source`, `remedy_hint`, `previous_errors`, top-level `user_id` | live, `google/gemma-4-31b-it:free` |
| `errors/recorded-upstream-rate-limited.json` | 429 with `Retry-After` and `metadata.retry_after_seconds`, `metadata.headers`; recorded with `requests/free-stream-rate-limited.json` | live, `liquid/lfm-2.5-2.6b:free` |
| `errors/recorded-unknown-model.json` | 400 for a model id OpenRouter doesn't know | live |
| `errors/recorded-invalid-messages.json` | 400 for an empty `messages` array | live |
| `errors/retrieve-not-found.json` | 404 from `GET /api/v1/models/{model}`, which OpenRouter answers for every model, so Kaeso's model check falls back to the listing | live, `curl` without a key, 2026-10-19 |
| `models/list-recorded.json` | `GET /api/v1/models`: `data`, `total_count` and `links`, no `object`; each model without `object` and `owned_by`, with OpenRouter's own fields. The first 2 of 471 models and one `:free` model are kept, unchanged | live, `curl` without a key, 2026-10-19 |

## What KPP does with OpenRouter's own fields

- Request fields with no KPP field (`provider`, `models`, `transforms`, `usage`, `reasoning`) stay in `openai_chat` Extras: they round-trip to OpenRouter and are refused or dropped across formats as the [extras allowlist](https://docs.kaeso.app/development/kpp/cross-format#extras) says.
- `reasoning` on a reply message and in deltas becomes a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])`, so Anthropic Messages and Responses apps get it as a thinking block or reasoning item and Chat apps get the field back as sent.
- Reply fields with no KPP field (`provider`, `native_finish_reason`, `reasoning_details`, `usage.cost`, `usage.is_byok`, `usage.cost_details` and the extra token details) stay in Extras.
- SSE comments round-trip as comments.
