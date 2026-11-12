---
title: Chat Completions gateways
description: What the gateways and routers that pass Chat Completions on to other providers add, and how the codec meets them.
order: 6
nav: Gateways
---

Gateways and routers that pass requests on to other providers speak [OpenAI Chat Completions](openai-chat.md) with differences of their own. Look up here what each one adds before you change its profile; their profiles are in the table of [provider profiles](openai-chat-providers.md#provider-profiles), and the other providers are on [Chat Completions providers](openai-chat-providers.md).

Their fixtures are in `fixtures/openai_chat/providers/<id>/` ([fixtures](fixtures.md)).

## OpenRouter

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

## Vercel

Sources: Vercel's [Chat Completions](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions), [reasoning](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/reasoning) and [advanced configuration](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/advanced) pages, read 2026-10-29, and the live recordings in `fixtures/openai_chat/providers/vercel/`. Chat Completions is Vercel's `generation` dialect, which every model it lists speaks.

| Vercel field | KPP | Notes |
|---|---|---|
| `providerOptions` (`gateway` routing such as `order`, `only`, `sort`, `models`, `byok`, `tags`, `user`), `models`, `provider`, `reasoning` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields (`requests/gateway-options`). |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`, as for OpenRouter. |
| `message.reasoning_details`, `delta.reasoning_details` | extras | The same reasoning in OpenRouter's structured form, with `format: "unknown"` for models that aren't OpenAI's or Anthropic's. |
| `provider_metadata` on the message and on the last chunk's delta (`gateway.routing`, `cost`, `marketCost`, `gatewayCost`, `generationId`), `generationId` at the top level | extras | Left out towards other dialects. The recordings' `clientSessionId`, a fingerprint of the client, is replaced with zeros. |
| `usage.cost`, `market_cost`, `gateway_cost`, `surcharge_cost`, `reporting_tag_write_cost`, `is_byok`, `cost_details`, `cache_creation_input_tokens`, the extra token details | `raw` usage | Kaeso records costs at the catalogue's prices. `usage.cost` is the inference alone: the tags in `requests/gateway-options` added a surcharge of $0.00015, which only `gateway_cost` holds. |
| the last stream chunk, with the finish reason, `provider_metadata` and `usage` together | `Usage` | Read like a usage chunk with `choices: []`. |
| errors in OpenAI's envelope with Vercel's own `type` (`model_not_found`, `no_providers_available`, `authentication_error`), a `param` that is an object, and `providerMetadata` beside `error` | `KppError` | The kind follows the status: 401 for a wrong key, 403 for a model the account may not call (`errors/recorded-restricted-model`, a free-tier account and Claude), 404 for an unknown model. Chat apps get the body as Vercel sent it. |
| `GET /v1/models`: OpenAI's shape, each model with Vercel's own fields (`name`, `type`, `context_window`, `max_tokens`, `pricing`, `tags`, `supported_parameters`) | `ModelList`, extras | The listing answers a wrong key with 401 (`errors/list-invalid-api-key`), but `GET /v1/models/{model}`, which a model check asks first, answers without checking the key. |

## Standard Compute

Sources: Standard Compute's [Chat Completions](https://docs.standardcompute.com/api/chat-completions), [models and routing](https://docs.standardcompute.com/models), [errors](https://docs.standardcompute.com/api/errors) and [models listing](https://docs.standardcompute.com/api/models) pages, read 2026-11-12, and the live recordings in `fixtures/openai_chat/providers/standardcompute/`, most through smart routing (`standardcompute`).

| Standard Compute field | KPP | Notes |
|---|---|---|
| `model: "StandardCompute"` on a smart routing reply | `Response.model` | The reply never names the model that answered; Standard Compute's request logs do. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`, as for OpenRouter. |
| `message.reasoning_details`, `delta.reasoning_details` | extras | The same reasoning in OpenRouter's structured form, including a Gemini model's thought signature as a `reasoning.encrypted` item; Chat apps get it back unchanged and send it back on later turns. |
| `native_finish_reason`, `refusal: null`, `system_fingerprint: null`, `service_tier` | extras | Left out towards other dialects. |
| `cache_write_tokens`, `audio_tokens`, `video_tokens`, `image_tokens` in `usage` | `raw` usage | |
| the last stream chunk, which repeats the finish choice beside `usage` | `Usage` | Read like a usage chunk with `choices: []`. |
| a 401 `error` without `code`; an error with a numeric `code` from the model's own provider | `KppError` | The kind follows the status; 402 `insufficient_quota` with `code: "budget_exhausted"` is `Billing`. |
| 400 `model_not_found` for an unknown model | `KppError` of kind `InvalidRequest` | Never 404, so it never marks a catalogue model unavailable. |
| `GET /v1/models`: `StandardCompute`, `context_length`, `architecture`, `supported_parameters`, `anthropic_family_tier`, `is_family_default` | `ModelList`, extras | Served without a key. A key's list can leave out models the key may still call, and names the router `StandardCompute`, writes `anthropic/claude-haiku-5-5` for the `anthropic/claude-haiku-5.5` its replies name, so a model check matches ids without regard to case or to `.` against `-`, and calls a model it can't find inconclusive. `GET /v1/models/{model}` answers 404 for every model. |

## Cloudflare AI Gateway

Sources: Cloudflare's [REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/), [Unified Billing](https://developers.cloudflare.com/ai-gateway/features/unified-billing/) and [model catalogue](https://developers.cloudflare.com/ai/models/) pages, read 2026-11-12, and the live recordings in `fixtures/openai_chat/providers/cloudflare-ai-gateway/`, from Workers AI models (third-party models answered 402 without credits).

| Cloudflare field | KPP | Notes |
|---|---|---|
| `/client/v4/accounts/{account_id}/ai/v1/…` | `EgressCtx::base_path` | The key's account takes the place of `/v1` for Chat Completions, Responses and Messages; a key's gateway goes in `cf-aig-gateway-id`. |
| `message.reasoning` and `reasoning_content`, `delta.reasoning` and `reasoning_content` | `Block::Reasoning` | The same text in both fields; the profile reads `reasoning` and Chat apps get `reasoning_content` back unchanged. |
| `usage` on every stream chunk, then a last chunk with the whole usage | `Usage` | Each chunk counts only its own tokens; the last one, after `finish_reason`, holds the totals. |
| `usage.neurons` | `raw` usage | Workers AI's own unit; Kaeso prices tokens. |
| `refusal`, `annotations`, `audio`, `function_call`, `tool_calls: []`, `token_ids` | extras | Round-trip for Chat apps. |
| `{"errors": [{"code", "message"}], "success": false}` | `KppError` | Cloudflare's own envelope for a refused token (401, code 10000), an empty balance (402, code 2021), an unknown (404, code 7003) or retired model (410, code 7010), an image link (400, code 6004) and a refused reasoning effort (400, code 8001). The kind follows the status; the message is Cloudflare's. |
| Messages errors, `{"type": "error", "error": {"type": "invalid_request_error"}}` with 402 | `KppError` of kind `Billing` | A 402 is a billing error whatever its type. |
| Responses streams from Workers AI models | not used | `$error: Error: Network connection lost.` or Workers AI's own `{"response"}` events instead of Responses events (live), so Kaeso streams Workers AI models through Chat Completions. |
