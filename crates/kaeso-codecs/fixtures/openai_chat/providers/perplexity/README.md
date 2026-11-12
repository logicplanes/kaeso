# Fixtures: Perplexity (`perplexity`) on `openai_chat`

Perplexity's Sonar chat completions at `POST https://api.perplexity.ai/v1/sonar`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.perplexity.ai`. The body is a subset of OpenAI's Chat Completions plus Perplexity's search fields; replies add `citations`, `search_results`, `images`, `related_questions` and a `usage.cost` object.

Perplexity ended support for Sonar chat completions on 2026-09-27: synchronous and streaming requests keep working and are answered by its Agent API behind the same endpoint. KPMR lists the four Sonar models as deprecated.

No key was available, so **every file is spec-built** from the sources below, read on 2026-11-12; none is a live recording. Ids, timestamps, token counts and prices are made up; field names, nesting and value types follow the schema.

## Sources (read 2026-11-12)

| Source | Address |
|---|---|
| OpenAPI document: `POST /v1/sonar`, `ApiChatCompletionsRequest`, `CompletionResponse`, `Choice`, `UsageInfo`, `Cost`, `ApiPublicSearchResult`, `HTTPValidationError`; `GET /v1/models` with its example | https://docs.perplexity.ai/openapi.json |
| Migrating from Sonar: the end of support and the `/v1/sonar` endpoint | https://docs.perplexity.ai/docs/agent-api/migrate-from-sonar/overview.md, https://docs.perplexity.ai/docs/agent-api/migrate-from-sonar/how-to.md |
| Official Python SDK types: `StreamChunk` (`status`, `type`, `usage` on every chunk), `Choice` (`delta` and `message` both required), `UsageInfo` (`request_cost` only on the last chunk) | https://github.com/perplexityai/perplexity-py at commit `1a79c5d3a7b1ef8c2dce17c0f0aa7e23f9bbdced`, `src/perplexity/types/` |
| Rate limits: 429 bodies, `Retry-After`, `x-ratelimit-*` headers | https://docs.perplexity.ai/docs/admin/rate-limits-usage-tiers.md |
| Errors (status codes) | https://docs.perplexity.ai/docs/sdk/error-handling.md |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/search-filters.json` | `sonar-pro` with a system message, `max_tokens`, `temperature`, `top_p` and every search field: `search_mode`, `search_domain_filter` (with a `-` exclusion), `search_language_filter`, `search_recency_filter`, the date filters, `return_related_questions`, `return_images`, `enable_search_classifier`, `language_preference` and `web_search_options` with `search_type` and `user_location` | schema: `ApiChatCompletionsRequest`, `WebSearchOptions`, `UserLocation` |
| `requests/json-schema.json` | `response_format` of type `json_schema` without a `name`, and `disable_search` | schema: `ResponseFormatJSONSchema`, `JSONSchema` |
| `requests/reasoning-stream.json` | `sonar-reasoning-pro`, streamed, with `reasoning_effort` and no `stream_options` | schema: `ApiChatCompletionsRequest` |
| `responses/citations-doc.json` | a reply with `citations`, `search_results`, `related_questions`, a `delta` beside the `message`, and usage with `search_context_size`, `citation_tokens`, `num_search_queries` and `cost` | schema: `CompletionResponse`, `Choice`, `UsageInfo`, `Cost` |
| `responses/json-schema-doc.json` | the JSON answer to `requests/json-schema.json`, with empty `citations` and `search_results` | schema: `CompletionResponse` |
| `streams/full-mode-doc.sse` | `stream_mode: "full"` (the default): every chunk has the `delta`, the `message` so far, the citations, the search results and a running usage whose `cost` is zero until the last chunk, which has `status: "COMPLETED"`, `type: "end_of_stream"` and `finish_reason: "stop"`; no `[DONE]` | sdk: `StreamChunk`, `Choice`, `UsageInfo` |
| `errors/request-rate-limit-doc.json` | 429 `request_rate_limit_exceeded` with a numeric `code` | doc: rate limits page, verbatim |
| `errors/token-rate-limit-doc.json` | 429 `token_rate_limit_exceeded` (TPM) | doc: rate limits page, verbatim |
| `errors/validation-error-doc.json` | 422 in FastAPI's `detail` list for a model the endpoint doesn't take | schema: `HTTPValidationError`, `ValidationError` |
| `models/list-doc.json` | `GET /v1/models`: the Agent API's models (`anthropic/claude-opus-5`, `perplexity/sonar`, …), not the Sonar or Decider model ids | doc: the endpoint's example, verbatim |

## Where Perplexity differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the endpoint is `/v1/sonar`; `/v1/chat/completions` belongs to no Perplexity API | the `chat` profile's path, `at_path("/v1/sonar")` | migration guide; OpenAPI document |
| search fields (`search_mode`, `search_domain_filter`, `web_search_options`, …) | extras of the request, sent as written to Perplexity from Chat Completions apps; refused when a request comes from another format | OpenAPI document |
| `citations`, `search_results`, `images`, `related_questions`, `usage.cost`, `usage.citation_tokens`, `usage.num_search_queries`, `usage.search_context_size` | extras of the reply, returned as sent to Chat Completions apps; apps in other formats get the text with its `[1]` markers | OpenAPI document |
| every stream chunk repeats the message so far, the citations and a running usage, and the stream ends without `[DONE]` | the text from `delta`; the last usage counts; the rest are extras of the chunk | SDK types |
| no `stream_options`; usage comes with every stream | not sent (`without_stream_options()`); the app's `include_usage` decides whether it sees usage, and other options are refused | OpenAPI document; SDK types |
| `max_tokens`, not `max_completion_tokens`; roles `system`, `user`, `assistant` | `max_tokens()` and `system_role()` | OpenAPI document |
| no `tools`, `tool_choice`, `parallel_tool_calls`, `logprobs`, `top_logprobs`, `logit_bias`, penalties, `seed`, `n` above 1, `audio`, `modalities`, `prediction`, `store`, `metadata`, `service_tier` or prompt cache keys in the API reference; the SDK lists some of them, so the sources disagree | refused before sending | OpenAPI document; SDK types |
| `stream_mode: "concise"` sends reasoning as events whose shape isn't documented | `stream_mode` is refused; the default, `full`, is what Kaeso reads | OpenAPI document |
| end users go in a `Perplexity-Safety-Identifier` header, not a body field | the end user is left out (`without_end_user()`) | https://docs.perplexity.ai/docs/admin/safety-identifiers.md |
| a request Perplexity can't validate gets 422 with FastAPI's `detail` list | `InvalidRequest`, with the message and the field from the first entry | OpenAPI document |
| `GET /v1/models` lists the Agent API's models, not `sonar` or `sonar-pro` | the provider's listing is partial: a model check calls a model it leaves out inconclusive, never unavailable | OpenAPI document |
