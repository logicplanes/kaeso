# Fixtures: Cloudflare AI Gateway (`cloudflare-ai-gateway`) on `openai_chat`

Cloudflare's REST API at `POST https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1/chat/completions`, with a Cloudflare API token as `Authorization: Bearer` and the gateway, when the key names one, in `cf-aig-gateway-id`. It serves third-party models (`openai/…`, `anthropic/…`), paid with Unified Billing credits or keys stored on the gateway, and Workers AI models (`@cf/…`). The same API serves Responses (`…/ai/v1/responses`, fixtures in `openai_responses/providers/cloudflare-ai-gateway/`) and Messages (`…/ai/v1/messages`, fixtures in `anthropic_messages/providers/cloudflare-ai-gateway/`).

Recorded live on 2026-11-12 with `cargo xtask record` and a token with Workers AI Read, the base URL set to the account's `…/ai`. The account had no Unified Billing credits, so third-party models answered 402 and the replies come from Workers AI models: `@cf/zai-org/glm-4.7-flash` (reasoning) and `@cf/deepseek-ai/deepseek-v4.1-flash-1017` (tools, streams). `account/search-recorded.json` is Cloudflare's Workers AI model search for `glm-4.7-flash`, recorded with curl. No file holds the account id.

## Sources (read 2026-11-12)

| Source | Address |
|---|---|
| REST API (endpoints, token permission, `cf-aig-gateway-id`, `cf-aig-*` headers) | https://developers.cloudflare.com/ai-gateway/usage/rest-api/ |
| Unified API (`/compat`, deprecated for single-model calls) | https://developers.cloudflare.com/ai-gateway/usage/chat-completion/ |
| Unified Billing (credits, no markup, stored keys under `default`) | https://developers.cloudflare.com/ai-gateway/features/unified-billing/ |
| Model catalogue (request formats, prices per model) | https://developers.cloudflare.com/ai/models/ |
| Model search API | https://developers.cloudflare.com/api/resources/ai/subresources/models/methods/list/ |
| Token formats (`cfut_`, `cfat_`, unprefixed) | https://developers.cloudflare.com/fundamentals/api/get-started/token-formats/ |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `responses/recorded-workers-ai-reply.json`, `recorded-plain-developer-string.json`, `recorded-json-schema.json`, `recorded-stop-array.json` | GLM-4.7-Flash: thinking in `reasoning` and again in `reasoning_content`, `refusal`, `annotations`, `audio`, `function_call` set to `null` and an empty `tool_calls`, `usage.neurons` | live |
| `responses/recorded-tools-single.json`, `recorded-tools-parallel.json`, `recorded-tools-results-parallel-error.json`, `recorded-system-name-multiturn.json` | DeepSeek V4.1 Flash: one and two tool calls, the answer after tool results, named messages | live |
| `streams/recorded-stream-include-usage.sse`, `recorded-reasoning-stream.sse` | `usage` on every chunk, counting only that chunk's tokens, then a second `finish_reason` chunk with the whole usage; `token_ids`; `[DONE]` | live |
| `requests/*.json` | the requests below that only this provider's fixtures send | — |
| `errors/recorded-invalid-api-key.json` | 401, Cloudflare's envelope, code 10000 `Authentication error` | live |
| `errors/recorded-insufficient-balance.json` | 402, code 2021: a third-party model without credits or a stored key | live |
| `errors/recorded-unknown-model.json` | 404, code 7003 `Model not found` | live |
| `errors/recorded-retired-model.json` | 410, code 7010, naming the model to use instead | live |
| `errors/recorded-reasoning-effort-none.json` | 400, code 8001 `Invalid input`: GLM-4.7-Flash takes `low`, `medium` and `high` only | live |
| `errors/recorded-image-base64-detail.json` | 400, code 6004: Workers AI takes images as base64 data, not links | live |
| `account/search-recorded.json` | `GET …/ai/models/search?search=glm-4.7-flash`: Workers AI's catalogue, in Cloudflare's envelope | live |

## Where Cloudflare differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the API path names the account: `/client/v4/accounts/{account_id}/ai/v1` | the key's account becomes `EgressCtx::base_path`, its gateway the `cf-aig-gateway-id` header | REST API page |
| errors come in Cloudflare's envelope, `{"errors": [{"code", "message"}], "success": false}`, not OpenAI's `error` object | `KppError` with Cloudflare's message; the kind follows the status | live |
| Workers AI models send thinking twice, as `reasoning` and `reasoning_content` | the profile reads `reasoning`; Chat apps get `reasoning_content` back as it came | live |
| every stream chunk carries `usage` for its own tokens, and the totals come in a last chunk | the last usage wins | live |
| `usage.neurons`, Workers AI's billing unit | raw usage; Kaeso prices tokens from the catalogue | live |
| no model listing in a model format (`GET …/ai/v1/models` answers 405) | a model check asks the Workers AI model search for `@cf/` models, and calls other models inconclusive after checking the key | live |
| models Cloudflare serves only through Messages or Responses | KPMR's `gateway.egress_dialect` per model, from the model's catalogue page | catalogue |
| no token counting | refused before sending | REST API page |
| Workers AI Responses streams fail (`$error: Error: Network connection lost.`) or answer in Workers AI's own event format | Workers AI models stream through Chat Completions, the provider's first format | live |
