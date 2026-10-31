# Fixtures: Moonshot AI (`moonshotai`) on `openai_chat`

The Kimi API's OpenAI-compatible Chat Completions, `POST https://api.moonshot.ai/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.moonshot.ai`. Chat Completions is Moonshot AI's `generation` dialect.

## Sources (read 2026-10-30)

| Source | Address |
|---|---|
| API overview (base URLs, compatible formats) | https://platform.kimi.ai/docs/api/overview |
| Chat Completions reference (`reasoning_content`, Partial Mode, usage) | https://platform.kimi.ai/docs/api/chat |
| Model parameter reference (fixed sampling values, `thinking`, `reasoning_effort`) | https://platform.kimi.ai/docs/api/models-overview |
| Thinking models | https://platform.kimi.ai/docs/guide/use-thinking-models |
| Partial Mode | https://platform.kimi.ai/docs/guide/use-partial-mode-feature-of-kimi-api |
| Error codes | https://platform.kimi.ai/docs/api/errors |

Every reply, stream and error named `recorded-…` below is a live recording made with `cargo xtask record` on 2026-10-30, with `kimi-k2.6` and `kimi-k3`; each is listed with its model in the `RECORDED.md` of its folder. The account was limited to 3 requests a minute, so the recordings were made one at a time. The model listing, the retrieved model, `errors/retrieve-not-found.json` and `errors/list-invalid-api-key.json` are plain `GET` requests made the same day. `errors/exceeded-current-quota.json` and `errors/engine-overloaded.json` are constructed from the error page's types and messages, because neither can be caused on purpose.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/thinking-disabled.json`, `requests/invalid-api-key.json`, `requests/rate-limited.json` | a short question with `thinking: {type: "disabled"}` | written for recording |
| `requests/reasoning.json`, `requests/reasoning-stream.json` | `kimi-k2.6`, which thinks by default, as a reply and as a stream with `include_usage` | written for recording |
| `requests/reasoning-effort-low.json`, `requests/reasoning-effort-none.json` | `kimi-k3` with `reasoning_effort` `low` and `none` | written for recording |
| `requests/stream-thinking-off.json` | a stream with thinking off and `include_usage` | written for recording |
| `requests/reasoning-tools.json`, `requests/reasoning-tools-stream.json`, `requests/tool-result.json` | one function tool with thinking, as a reply and a stream, then the recorded call, its `reasoning_content` and a tool result sent back | written for recording |
| `requests/max-tokens-honoured.json`, `requests/max-completion-tokens-honoured.json` | the same request with `max_tokens: 10` and `max_completion_tokens: 10` | written for recording |
| `requests/partial.json`, `requests/prefill-without-partial.json` | a prefilled assistant message with and without `partial: true` | written for recording; Partial Mode page |
| `requests/logprobs.json`, `requests/json-object-system.json`, `requests/system-name-stop.json`, `requests/image.json` | `logprobs` with `top_logprobs`; JSON mode; `system`, `name`, a `stop` list, `user` and `seed`; a base64 PNG | written for recording |
| `requests/parallel-tool-calls-false.json` | two cities with `parallel_tool_calls: false` | written for recording |
| `requests/developer-role.json`, `requests/temperature.json`, `requests/top-p.json`, `requests/logit-bias-refused.json`, `requests/tool-choice-required-thinking.json`, `requests/unknown-model.json` | requests Moonshot refuses | written for recording |
| `responses/recorded-thinking-disabled.json`, `responses/recorded-image.json` | `reasoning_content: ""` beside the answer when thinking is off | live, `kimi-k2.6` |
| `responses/recorded-reasoning.json`, `responses/recorded-reasoning-effort-low.json` | `message.reasoning_content`; `completion_tokens_details.reasoning_tokens`; `prompt_tokens_details.cache_write_tokens` from `kimi-k3` | live |
| `responses/recorded-reasoning-effort-none.json` | `kimi-k3` answers `none` and thinks not at all, without a `reasoning_content` key | live, `kimi-k3` |
| `responses/recorded-reasoning-tools.json`, `responses/recorded-tool-result.json` | thinking, `content: ""` and a tool call with `index` and an id such as `get_weather_0_2fdaa899`; the answer to the tool result | live, `kimi-k2.6` |
| `responses/recorded-max-tokens-honoured.json`, `responses/recorded-max-completion-tokens-honoured.json` | 10 completion tokens and `finish_reason: "length"` for both spellings; the second also has `usage.cached_tokens` beside `prompt_tokens_details.cached_tokens` | live, `kimi-k2.6` |
| `responses/recorded-partial.json` | the continuation only, without the prefill and without `reasoning_content` | live, `kimi-k2.6` |
| `responses/recorded-prefill-without-partial.json` | the prefill repeated at the start of the reply | live, `kimi-k2.6` |
| `responses/recorded-logprobs.json`, `responses/recorded-json-object-system.json`, `responses/recorded-system-name-stop.json` | `logprobs.content` with `top_logprobs` and `bytes`; a JSON object; the reply stopped before `5` | live, `kimi-k2.6` |
| `responses/recorded-parallel-tool-calls-false.json` | two tool calls despite `parallel_tool_calls: false` | live, `kimi-k2.6` |
| `streams/recorded-reasoning-stream.sse`, `streams/recorded-reasoning-tools-stream.sse` | `delta.reasoning_content` pieces, then an empty `reasoning_content` piece, then the answer or the whole tool call in one delta; a finish chunk with `usage`, then a usage chunk with `choices: []` | live, `kimi-k2.6` |
| `streams/recorded-stream-thinking-off.sse` | an empty `reasoning_content` piece before the first text, with thinking off | live, `kimi-k2.6` |
| `errors/recorded-developer-role.json` | 400 `role 'developer' is not allowed` | live |
| `errors/recorded-temperature.json`, `errors/recorded-top-p.json` | 400 `invalid temperature: only 1 is allowed for this model`, and the same for `top_p` (0.95 only) | live |
| `errors/recorded-logit-bias-refused.json` | 400 `logit_bias is not supported yet` | live |
| `errors/recorded-tool-choice-required-thinking.json` | 400 `tool_choice 'required' is incompatible with thinking enabled` | live |
| `errors/recorded-unknown-model.json` | 404 `resource_not_found_error` | live |
| `errors/recorded-invalid-api-key.json` | 401 `invalid_authentication_error` | live, recorded with a made-up key |
| `errors/recorded-rate-limited.json` | 429 `rate_limit_reached_error` with `retry-after: 1`; its message names the organization and key id, replaced here with zeros | live |
| `errors/exceeded-current-quota.json`, `errors/engine-overloaded.json` | 429 `exceeded_current_quota_error` (balance used up) and 429 `engine_overloaded_error` | constructed from the error page |
| `errors/retrieve-not-found.json`, `errors/list-invalid-api-key.json` | 404 from `GET /v1/models/{model}`, and 401 from `GET /v1/models` with a made-up key | live |
| `models/list-recorded.json` | `GET /v1/models`: all 5 models, unchanged, with `owned_by: "moonshot"` and Moonshot's fields per model (`context_length`, `supports_reasoning`, `reasoning`, `modalities`, `limit` and others) | live |
| `models/retrieve-recorded.json` | `GET /v1/models/kimi-k2.6` | live |

## Where Moonshot AI differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the `developer` role is refused with 400 | the `chat` profile sends `system` (`system_role()`) | live: `errors/recorded-developer-role` |
| a final assistant message is answered anew, and the reply repeats it, unless it has `partial: true` | the `chat` profile adds `partial: true` to the final assistant message of apps in other formats (`partial_prefill()`) | live: `recorded-prefill-without-partial`, `recorded-partial`; Partial Mode page |
| `message.reasoning_content` and `delta.reasoning_content` carry the thinking, and Moonshot asks for it back on assistant turns | a reasoning block through `reasoning_fields(["reasoning_content"])`; thinking from apps in other formats goes back as `reasoning_content` | live; Chat Completions reference |
| a stream ends its thinking with an empty `reasoning_content` piece | an empty piece of the open reasoning block, so the reasoning stays one block | live: `streams/recorded-reasoning-stream` |
| with thinking off, a stream sends one empty `reasoning_content` piece before the text | an extra chunk, kept as it came | live: `streams/recorded-stream-thinking-off` |
| `audio`, `metadata`, `modalities`, `parallel_tool_calls`, `service_tier`, `store`, `verbosity` and `web_search_options` are accepted and ignored | refused before sending by the `chat` profile (`refusing`) | live: `recorded-parallel-tool-calls-false` and a probe of the others, 2026-10-30 |
| `temperature`, `top_p`, `n` and the penalties take one value per model | sent as written; Moonshot refuses other values with 400, and Kaeso refuses a `temperature` other than 1 before sending, as KPMR marks the models as taking none | live: `errors/recorded-temperature`, `errors/recorded-top-p`; model parameter page |
| `reasoning_effort`: K3 models document `low`, `high` and `max` and answer `none` without thinking; `kimi-k2.6` thinks unless it gets `none`; `kimi-k2.7-code` refuses `none` | the catalogue's effort levels from KPMR | live: `recorded-reasoning-effort-low`, `recorded-reasoning-effort-none`, and a probe of `kimi-k2.6` and `kimi-k2.7-code` |
| `thinking: {type}`, Moonshot's switch for K2 models | a request extra; refused when the target speaks another format | model parameter page |
| `max_tokens` and `max_completion_tokens` are both honoured | sent as the app wrote them | live: `recorded-max-tokens-honoured`, `recorded-max-completion-tokens-honoured` |
| `usage.cached_tokens` next to `usage.prompt_tokens_details.cached_tokens`, and `prompt_tokens_details.cache_write_tokens` | the cache counts come from `prompt_tokens_details`; the top-level copy stays in the raw usage | live |
| `system_fingerprint` such as `fpv0_45f9e909` on stream chunks | an extra of the chunk, round-trips | live |
| errors in OpenAI's envelope without `code`, with Moonshot's own `type` | the kind comes from the status; `exceeded_current_quota_error` is a billing refusal and `engine_overloaded_error` an overload, although both are 429 | live; error page |
| `/v1/embeddings` answers 403 `The API you are accessing is not open`, and token counting has no endpoint KPP knows | `embeddings` and `count_tokens` are `None` | live probe |
| `/v1/responses` and `/anthropic/v1/messages` answer too | not in `dialects`: Kaeso translates every app format to Chat Completions | live probe |
