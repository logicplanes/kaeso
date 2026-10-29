# Fixtures: Vercel (`vercel`) on `anthropic_messages`

Vercel AI Gateway's Anthropic Messages API, `POST https://ai-gateway.vercel.sh/v1/messages`, with `Authorization: Bearer` (Vercel takes `x-api-key` too) and `anthropic-version: 2023-06-01`. A Vercel model uses it when its **Sends as** value is `anthropic_messages`; it serves every model, not only Claude.

## Sources (read 2026-10-29)

| Source | Address |
|---|---|
| Anthropic Messages API, `count_tokens`, errors | https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api |
| Extended thinking | https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api/reasoning |

Every live recording was made on 2026-10-29 with a free-tier Vercel account, with `nvidia/nemotron-3.5-lightning-free` (free) or models that cost under $0.0001 a request, and is listed with its model in the `RECORDED.md` of its folder. The free tier refuses paid models such as Claude with 403, and allows 5 requests a minute to the free model. Vercel's `provider_metadata` (or `providerMetadata`) carries `gateway.routing.clientSessionId`, a fingerprint of the client; it is replaced with zeros and nothing else is changed. Generation ids (`gen_…`) and the upstream provider's request ids are kept as recorded.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/text.json`, `requests/text-stream.json` | Plain text with a `system` string, as a reply and as a stream | written for recording |
| `requests/thinking.json`, `requests/thinking-stream.json` | A question a reasoning model thinks about, as a reply and as a stream | written for recording |
| `requests/tools.json` | One tool, `tool_choice: {type: "auto"}` | written for recording |
| `requests/rate-limited.json` | The same request as `tools.json`, which met the free model's limit of 5 requests a minute | written for recording |
| `requests/unknown-model.json`, `requests/restricted-model.json` | An unknown model, and a paid model on the free tier | written for recording |
| `responses/recorded-text.json` | A reply with top-level `provider_metadata` | live, `mistral/mistral-nemo` |
| `responses/recorded-thinking.json` | A `thinking` block without `signature`, then text | live, `nvidia/nemotron-3.5-lightning-free` |
| `responses/recorded-tools.json` | A `thinking` block, then `tool_use` | live, `nvidia/nemotron-3.5-lightning-free` |
| `streams/recorded-text-stream.sse`, `streams/recorded-thinking-stream.sse` | Text, and thinking then text, streamed | live |
| `errors/recorded-unknown-model.json` | 404 in OpenAI's error shape, not Anthropic's | live |
| `errors/recorded-restricted-model.json` | 403 `no_providers_available` in OpenAI's error shape, with `providerMetadata` | live, `anthropic/claude-haiku-4.5` |
| `errors/recorded-rate-limited.json` | 429 `rate_limit_exceeded` in OpenAI's error shape, with `Retry-After: 48` | live, `nvidia/nemotron-3.5-lightning-free` |

Each error's `meta.json` holds under `encoded` the Anthropic error an Anthropic app gets: the message of Vercel's error with the type of its status.

## What KPP does with Vercel's own fields

- `provider_metadata` stays in Extras and round-trips unchanged.
- A `thinking` block without `signature` decodes and reaches Anthropic apps as Vercel sent it.
- Errors in OpenAI's shape keep their message: the decoder reads `error.message` and takes the kind from `error.type` when it is one of Anthropic's, else from the status.
- Vercel answers `POST /v1/messages/count_tokens` for every model, but counts exactly only for Claude models (`x-ai-gateway-token-count: estimated` and `provider_metadata.gateway.countTokens.estimated: true` otherwise), so Kaeso refuses token counts for Vercel models and records no fixture for them.
