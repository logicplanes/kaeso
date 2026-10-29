# Fixtures: Vercel (`vercel`) on `openai_responses`

Vercel AI Gateway's Responses API, `POST https://ai-gateway.vercel.sh/v1/responses`, with `Authorization: Bearer`. A Vercel model uses it when its **Sends as** value is `openai_responses`.

## Sources (read 2026-10-29)

| Source | Address |
|---|---|
| OpenAI Responses API | https://vercel.com/docs/ai-gateway/sdks-and-apis/responses |
| Streaming | https://vercel.com/docs/ai-gateway/sdks-and-apis/responses/streaming |
| Reasoning | https://vercel.com/docs/ai-gateway/sdks-and-apis/responses/reasoning |

Every live recording was made on 2026-10-29 with a free-tier Vercel account, with `nvidia/nemotron-3.5-lightning-free` (free) or models that cost under $0.0001 a request, and is listed with its model in the `RECORDED.md` of its folder. The free tier refuses paid models such as Claude with 403, and allows 5 requests a minute to the free model. Vercel's `provider_metadata` (or `providerMetadata`) carries `gateway.routing.clientSessionId`, a fingerprint of the client; it is replaced with zeros and nothing else is changed. Generation ids (`gen_…`) and the upstream provider's request ids are kept as recorded.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/text.json`, `requests/text-stream.json` | Plain text, as a reply (with `instructions`) and as a stream | written for recording |
| `requests/reasoning-stream.json` | `reasoning: {effort: "low"}`, streamed | written for recording |
| `requests/tools.json` | One function tool | written for recording |
| `requests/gpt-reasoning-encrypted.json` | An OpenAI model with `reasoning.summary` and `include: ["reasoning.encrypted_content"]` | written for recording |
| `requests/unknown-model.json` | An unknown model | written for recording |
| `responses/recorded-text.json` | A reply with top-level `provider_metadata` | live, `mistral/mistral-nemo` |
| `responses/recorded-tools.json` | A `reasoning` item with `content` (`reasoning_text`), then a `function_call` | live, `nvidia/nemotron-3.5-lightning-free` |
| `responses/recorded-gpt-reasoning-encrypted.json` | A `reasoning` item with `encrypted_content`, and `usage.input_tokens_details.cache_write_tokens` | live, `openai/gpt-5-nano` |
| `streams/recorded-text-stream.sse` | Text deltas | live, `mistral/mistral-nemo` |
| `streams/recorded-reasoning-stream.sse` | `response.reasoning.delta` and `response.reasoning.done`, Vercel's names for `response.reasoning_text.*`, without `content_part` events for the reasoning | live, `nvidia/nemotron-3.5-lightning-free` |
| `errors/recorded-unknown-model.json` | 404 `model_not_found` | live |

## What KPP does with Vercel's own fields

- `response.reasoning.delta` and `response.reasoning.done` decode like `response.reasoning_text.delta` and `.done`; Responses apps get the events as Vercel sent them, other dialects get the reasoning.
- `provider_metadata` and `cache_write_tokens` stay in Extras and round-trip unchanged.
- `POST /v1/responses/input_tokens` answers 404; Kaeso refuses token counts for Vercel models before sending anything.
