# Fixtures: Standard Compute (`standardcompute`) on `openai_chat`

Standard Compute's OpenAI-compatible Chat Completions at `POST https://api.stdcmpt.com/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.stdcmpt.com`. Standard Compute sells monthly plans with a compute budget and routes each request either to the model a router picks (`standardcompute`, smart routing) or to the exact model it names.

The replies, streams and errors were recorded live on 2026-11-12 with `cargo xtask record`, most through smart routing, which answers with `model: "StandardCompute"`. The model listing was answered without a key, as Standard Compute lists its models publicly. `errors/budget-exhausted-doc.json` is the example of the errors page.

## Sources (read 2026-11-12)

| Source | Address |
|---|---|
| Chat Completions | https://docs.standardcompute.com/api/chat-completions |
| Models and routing (`standardcompute`, pinned ids, `model_not_found`, `unsupported_model_input`) | https://docs.standardcompute.com/models |
| Reasoning (`reasoning_effort`, `reasoning`) | https://docs.standardcompute.com/reasoning |
| Errors (codes, `retry-after`, `x-sc-budget-renews-at`) | https://docs.standardcompute.com/api/errors |
| Budgets and limits (402 `budget_exhausted`, 429 `rate_limit_exceeded`, request limits) | https://docs.standardcompute.com/budgets-and-limits |
| Regions and privacy (`:eu` and `:us` models, `data_policy_restriction`) | https://docs.standardcompute.com/regions-and-privacy |
| Models listing | https://docs.standardcompute.com/api/models |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `responses/recorded-plain-developer-string.json`, `recorded-system-name-multiturn.json`, `recorded-stop-array.json` | smart routing with a `developer` message, a named system message and stop strings: `model: "StandardCompute"`, `native_finish_reason`, `refusal`, `reasoning` and `reasoning_details` with `reasoning.text` items, and usage with `cache_write_tokens`, `audio_tokens`, `video_tokens` and `image_tokens` | live |
| `responses/recorded-tools-single.json`, `recorded-tools-parallel.json`, `recorded-tools-results-parallel-error.json` | one tool call, two parallel calls with text beside them, and the answer after tool results, one of them an error | live |
| `responses/recorded-json-schema.json`, `recorded-file-pdf-data.json`, `recorded-logprobs-top-logprobs-doc.json` | structured output, a PDF `file` part, and `logprobs` with `top_logprobs` | live |
| `streams/recorded-stream-include-usage.sse`, `recorded-routed-reasoning-stream.sse` | smart routing streams: `delta.reasoning` with `reasoning_details`, a chunk with `finish_reason` and then a usage chunk that repeats it, `[DONE]` | live |
| `requests/gemini-tools-stream.json`, `streams/recorded-gemini-tools-stream.sse` | `google/gemini-3.5-flash-lite` pinned: a tool call whose thought signature comes as a `reasoning.encrypted` item of `reasoning_details` with the call's id | live |
| `requests/routed-reasoning-stream.json` | smart routing with `reasoning_effort` | live |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `invalid_request_error` without a code, for a made-up key; a request without a key gets the same | live |
| `requests/unknown-model.json`, `errors/recorded-unknown-model.json` | 400 `model_not_found` with `param: "model"` | live |
| `requests/image-to-text-model.json`, `errors/recorded-image-to-text-model.json` | 400 `unsupported_model_input`: an image to a text-only model | live |
| `errors/recorded-image-base64-detail.json` | 400 with a numeric `code` (`400`): the model's provider could not fetch an image link | live |
| `errors/recorded-retrieve-not-found.json` | 404 `{"detail": "Not Found"}` from `GET /v1/models/{id}`, which Standard Compute doesn't serve | live |
| `errors/budget-exhausted-doc.json` | 402 `insufficient_quota` with `code: "budget_exhausted"`: the plan's monthly budget is used up | doc |
| `models/list-recorded.json` | `GET /v1/models` without a key: `StandardCompute` for smart routing, `context_length`, `architecture.input_modalities`, `supported_parameters`, `anthropic_family_tier`, `is_family_default`, `display_name`, and the `:eu` and `:us` models | live |

## Where Standard Compute differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| smart routing answers with `model: "StandardCompute"`; the reply doesn't name the model that answered | the reply's model, as sent | live |
| thinking comes as `reasoning` with `reasoning_details` (`reasoning.text`, and `reasoning.encrypted` for Gemini's thought signatures), as OpenRouter sends it | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])`; `reasoning_details` is replayed with the assistant message | live |
| `native_finish_reason`, `refusal: null`, `system_fingerprint: null`, `service_tier` | extras, round-trip | live |
| a stream's last chunk carries the usage and repeats `finish_reason` | events, round-trip | live |
| errors use OpenAI's `error` object; a 401 has no `code`, and an error from the model's own provider can have a numeric `code` | `KppError`; the kind follows the status, and 402 `insufficient_quota` is `Billing` | live; errors page |
| unknown models get 400 `model_not_found`, not 404 | `InvalidRequest`, so an unknown id never marks a catalogue model unavailable | live |
| `GET /v1/models` answers without a key, names the router `StandardCompute`, and a key's listing can leave out models the key can still call | the provider's listing is partial: a model check matches ids without regard to case or to `.` against `-` (it lists `anthropic/claude-haiku-5-5` and answers `anthropic/claude-haiku-5.5`) and calls a model it leaves out inconclusive | live |
| `GET /v1/models/{id}` has no route (404 `{"detail": "Not Found"}`) | a model check falls back to the listing | live |
| `n` above 1 answers one choice | refused by the dialect, as for every provider | live |
| `video_url` parts fail upstream | video blocks are refused (no `with_video_url()`) | live |
