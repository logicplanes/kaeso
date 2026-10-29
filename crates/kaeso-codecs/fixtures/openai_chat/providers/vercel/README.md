# Fixtures: Vercel (`vercel`) on `openai_chat`

Vercel AI Gateway's Chat Completions API, `POST https://ai-gateway.vercel.sh/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://ai-gateway.vercel.sh`. Chat Completions is Vercel's `generation` dialect.

## Sources (read 2026-10-29)

| Source | Address |
|---|---|
| Chat Completions API, models and errors | https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions |
| Reasoning (`reasoning`, `reasoning_details`) | https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/reasoning |
| Provider options, model fallbacks, BYOK, caching | https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/advanced |
| API keys | https://vercel.com/docs/ai-gateway/authentication-and-byok/api-keys |

Every live recording was made on 2026-10-29 with a free-tier Vercel account, with `nvidia/nemotron-3.5-lightning-free` (free) or models that cost under $0.0001 a request, and is listed with its model in the `RECORDED.md` of its folder. The free tier refuses paid models such as Claude with 403, and allows 5 requests a minute to the free model. Vercel's `provider_metadata` (or `providerMetadata`) carries `gateway.routing.clientSessionId`, a fingerprint of the client; it is replaced with zeros and nothing else is changed. Generation ids (`gen_…`) and the upstream provider's request ids are kept as recorded.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/basic.json` | A system and a user message with `max_tokens` | written for recording |
| `requests/stream.json` | A stream with `include_usage` | written for recording |
| `requests/reasoning.json`, `requests/reasoning-stream.json` | The `reasoning` request object (`effort: "low"`), as a reply and as a stream | written for recording |
| `requests/tools.json`, `requests/tools-stream.json` | One function tool, `tool_choice: "auto"`, as a reply and as a stream | written for recording |
| `requests/gateway-options.json` | Vercel's fields with no KPP field: `providerOptions.gateway` (`order`, `sort`, `tags`, `user`), the `models` fallback list, `reasoning_effort` | written for recording |
| `requests/unknown-model.json`, `requests/invalid-messages.json`, `requests/restricted-model.json`, `requests/invalid-api-key.json` | Requests Vercel refuses: an unknown model, an empty `messages` array, a paid model on the free tier, and a wrong key | written for recording |
| `responses/recorded-basic.json` | A reply with `message.provider_metadata` (`gateway.routing`, `cost`, `marketCost`, `gatewayCost`, `generationId`), top-level `generationId`, and `usage.cost`, `market_cost`, `is_byok`, `cost_details`, `cache_creation_input_tokens`, `prompt_tokens_details.audio_tokens`, `video_tokens`, `completion_tokens_details.image_tokens` | live, `mistral/mistral-nemo` |
| `responses/recorded-gateway-options.json` | The same with `routing.sort`, and the tags' surcharge in `surcharge_cost`, `gateway_cost` and `reporting_tag_write_cost` | live, `mistral/mistral-nemo` |
| `responses/recorded-reasoning.json` | `message.reasoning` and `reasoning_details` (`reasoning.text`, `format: "unknown"`) | live, `nvidia/nemotron-3.5-lightning-free` |
| `responses/recorded-tools.json` | A tool call with reasoning beside it, `content: null` | live, `nvidia/nemotron-3.5-lightning-free` |
| `streams/recorded-stream.sse` | Text deltas; the last chunk carries the finish reason, `delta.provider_metadata` and `usage` together | live, `mistral/mistral-nemo` |
| `streams/recorded-reasoning-stream.sse` | `delta.reasoning` with `reasoning_details`, then content | live, `nvidia/nemotron-3.5-lightning-free` |
| `streams/recorded-tools-stream.sse` | Reasoning deltas, then a streamed tool call | live, `nvidia/nemotron-3.5-lightning-free` |
| `errors/recorded-unknown-model.json` | 404 `model_not_found` with `param: {modelId}` | live |
| `errors/recorded-invalid-messages.json` | 400 `invalid_request_error` for an empty `messages` array | live |
| `errors/recorded-restricted-model.json` | 403 `no_providers_available` for a paid model on the free tier: `param` is the whole upstream error object, with `providerMetadata` beside `error` | live, `anthropic/claude-haiku-4.5` |
| `errors/recorded-invalid-api-key.json` | 401 `authentication_error` for a wrong key | live, recorded with `VERCEL_API_KEY=vck_000…` |
| `errors/retrieve-not-found.json` | 404 `model_not_found` from `GET /v1/models/{model}` for an unknown model | live, `GET /v1/models/kaeso-example/no-such-model`, 2026-10-29 |
| `errors/list-invalid-api-key.json` | 401 from `GET /v1/models` with a wrong key; without any key the listing is served | live, 2026-10-29 |
| `models/list-recorded.json` | `GET /v1/models`: OpenAI's shape with Vercel's own model fields (`name`, `description`, `type`, `context_window`, `max_tokens`, `pricing` with tiers and regional prices, `tags`, `supported_parameters`, `zdr`). Three of 499 models are kept, unchanged: a Claude model, an embedding model and the free model | live, 2026-10-29 |
| `models/retrieve-recorded.json` | `GET /v1/models/nvidia/nemotron-3.5-lightning-free`, which Vercel answers without checking the key | live, 2026-10-29 |

## What KPP does with Vercel's own fields

- Request fields with no KPP field (`providerOptions`, `models`, `provider`, `reasoning`) stay in `openai_chat` Extras: they reach Vercel from a Chat app and are refused across formats as the [extras allowlist](https://docs.kaeso.app/development/kpp/cross-format#extras) says.
- `reasoning` on a reply message and in deltas becomes a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])`.
- `provider_metadata`, `generationId`, `reasoning_details` and the extra `usage` fields stay in Extras and round-trip unchanged to Chat apps. Kaeso records costs at the catalogue's prices.
- Errors are OpenAI-shaped and decode by status.
