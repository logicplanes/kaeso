# Fixtures: Alibaba Cloud (`alibaba`) on `openai_chat`

Alibaba Cloud Model Studio's OpenAI-compatible API in the international (Singapore) region, `POST https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions` and `GET /v1/models`, with `Authorization: Bearer`. Kaeso's base URL is `https://dashscope-intl.aliyuncs.com/compatible-mode`. Chat Completions is Model Studio's `generation` dialect. The China (Beijing) region, KPMR's `alibaba-cn`, has its own base URL and keys and is not recorded here.

## Sources (read 2026-10-30)

| Source | Address |
|---|---|
| OpenAI compatibility: parameters, `stream_options`, region-bound keys | https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope |
| Deep thinking: `enable_thinking`, `thinking_budget`, `reasoning_content`, models that think only, models that only stream | https://www.alibabacloud.com/help/en/model-studio/deep-thinking |
| API keys: keys start with `sk-`, keys made after the workspace upgrade with `sk-ws` | https://www.alibabacloud.com/help/en/model-studio/get-api-key |

Every file under `responses/`, `streams/` and `errors/` was recorded live with `cargo xtask record` on 2026-10-30 (`RECORDED.md` in each folder names the request and model), except the four model-listing files named below, recorded the same day with `curl`. `errors/recorded-invalid-api-key.json` and `errors/list-invalid-api-key.json` were recorded with a placeholder key of zeros. The key on this account is on the free tier, which is why `qwen-plus` answered 403. Nothing in a recording was changed.

## Chat profile

| setting | recorded source |
|---|---|
| `system_role()` | `errors/recorded-developer-role`: 400 `developer is not one of ['system', 'assistant', 'user', 'tool', 'function']` |
| `thinking_switch()` | Thinking is switched with `enable_thinking` and capped with `thinking_budget` (`responses/recorded-thinking`: 200 asked, 200 `reasoning_tokens`). `reasoning_effort` alone doesn't turn thinking on: `qwen-flash`, which doesn't think by default, answers `reasoning_effort: "high"` without any reasoning (`responses/recorded-reasoning-effort-alone`, 3 completion tokens) and thinks with `enable_thinking` (`responses/recorded-enable-thinking`, 100 of a 100-token budget). So the switch sends an app's thinking as `enable_thinking` and its budget as `thinking_budget`, an effort as `reasoning_effort` beside `enable_thinking: true` (`responses/recorded-reasoning-effort`), and `none` or thinking turned off as `enable_thinking: false` |
| `reasoning_fields(["reasoning_content"])` | `responses/recorded-thinking`, `responses/recorded-thinking-tools`, `streams/recorded-thinking-stream`; thinking sent back on an assistant turn is accepted (`responses/recorded-reasoning-sent-back`) |
| `refusing([...])` | Model Studio answers fields it doesn't declare with 200 and ignores them (a `bogus_field` too). `responses/recorded-logit-bias-field` (a bias of +100 on one token changed nothing), `recorded-store-metadata-fields`, `recorded-prediction-field`, `recorded-audio-fields` (text only) and `recorded-web-search-options-field` (no search) answer as if the field weren't there, so `audio`, `logit_bias`, `metadata`, `modalities`, `prediction`, `service_tier`, `store`, `verbosity` and `web_search_options` are refused |
| `reasoning_beyond_output_limit: true` (in `PROVIDERS`) | `responses/recorded-max-tokens-beyond-reasoning`: `max_tokens: 50` with thinking on gave 453 completion tokens, because `max_tokens` caps only the answer. `max_completion_tokens` caps both (`responses/recorded-max-completion-tokens-thinking`: 300 asked, 300 reasoning tokens, `finish_reason: "length"`), and Kaeso sends that for Anthropic and Responses apps, but a Chat app's `max_tokens` is sent as written |

Model Studio takes `max_completion_tokens` (`responses/recorded-output-limit`), `temperature`, `top_p`, `seed`, both penalties and a `stop` string (`responses/recorded-sampling`), `logprobs` with `top_logprobs` (`responses/recorded-logprobs`), `response_format` with a strict JSON schema (`responses/recorded-structured-output`), `tool_choice` `required` and a named function, `parallel_tool_calls: false` (`responses/recorded-tools-required`, `recorded-tools-named`), legacy `functions` (`responses/recorded-functions-field`), an assistant prefill, which it continues (`responses/recorded-assistant-prefill`), and the hints `user`, `safety_identifier` and `prompt_cache_key` (`responses/recorded-user-hints`), so the profile keeps OpenAI's spellings for them.

## Files

| file | what it covers |
|---|---|
| `requests/basic.json`, `responses/recorded-basic.json` | a system and a user message with thinking off; usage with `prompt_tokens_details.text_tokens` |
| `requests/thinking.json`, `responses/recorded-thinking.json` | `enable_thinking` with `thinking_budget: 200`: `message.reasoning_content` and `completion_tokens_details.reasoning_tokens` |
| `requests/reasoning-effort-alone.json`, `responses/recorded-reasoning-effort-alone.json` | `reasoning_effort: "high"` without `enable_thinking` on `qwen-flash`: no reasoning |
| `requests/enable-thinking.json`, `responses/recorded-enable-thinking.json` | `enable_thinking` with `thinking_budget: 100` on `qwen-flash`: 100 reasoning tokens |
| `requests/reasoning-effort.json`, `responses/recorded-reasoning-effort.json` | `reasoning_effort: "low"` with `enable_thinking` on `qwen3.8-flash` |
| `requests/reasoning-sent-back.json`, `responses/recorded-reasoning-sent-back.json` | an assistant turn carrying `reasoning_content` |
| `requests/stream.json`, `streams/recorded-stream.sse` | a stream with `include_usage`: `usage: null` on every chunk, then a last chunk with `choices: []` and the usage |
| `requests/thinking-stream.json`, `streams/recorded-thinking-stream.sse` | `delta.reasoning_content` beside `content: ""`, then the answer with `reasoning_content: ""` |
| `requests/tools.json`, `responses/recorded-tools.json` | two function tools answered with two parallel calls |
| `requests/tools-required.json`, `responses/recorded-tools-required.json` | `tool_choice: "required"` with `parallel_tool_calls: false` |
| `requests/tools-named.json`, `responses/recorded-tools-named.json` | a named function in `tool_choice` |
| `requests/tools-stream.json`, `streams/recorded-tools-stream.sse` | a streamed tool call: the id and name first, then argument pieces whose `id` is `""` and, on the last one, `null` |
| `requests/thinking-tools.json`, `responses/recorded-thinking-tools.json` | reasoning before a tool call |
| `requests/tool-result.json`, `responses/recorded-tool-result.json` | a tool result turn |
| `requests/image.json`, `responses/recorded-image.json` | an `image_url` part with a 32×32 PNG data URL on `qwen3-vl-plus`; `prompt_tokens_details.image_tokens` and `system_fingerprint` |
| `requests/structured-output.json`, `responses/recorded-structured-output.json` | `response_format` with a strict JSON schema |
| `requests/sampling.json`, `responses/recorded-sampling.json` | sampling fields and a `stop` string, honoured |
| `requests/assistant-prefill.json`, `responses/recorded-assistant-prefill.json` | a conversation ending with an assistant message, which Model Studio continues |
| `requests/logprobs.json`, `responses/recorded-logprobs.json` | `logprobs` and `top_logprobs`, returned |
| `requests/output-limit.json`, `responses/recorded-output-limit.json` | `max_completion_tokens: 10`, `finish_reason: "length"` |
| `requests/max-completion-tokens-thinking.json`, `responses/recorded-max-completion-tokens-thinking.json` | `max_completion_tokens` caps thinking and answer together |
| `requests/max-tokens-beyond-reasoning.json`, `responses/recorded-max-tokens-beyond-reasoning.json` | `max_tokens` caps only the answer |
| `requests/user-hints.json`, `responses/recorded-user-hints.json` | `user`, `safety_identifier` and `prompt_cache_key` |
| `requests/functions-field.json`, `responses/recorded-functions-field.json` | legacy `functions`, answered with `function_call` |
| `requests/logit-bias-field.json`, `requests/store-metadata-fields.json`, `requests/prediction-field.json`, `requests/audio-fields.json`, `requests/web-search-options-field.json` and their `responses/recorded-*.json` | fields Model Studio ignores with 200 |
| `requests/developer-role.json`, `errors/recorded-developer-role.json` | 400 for the `developer` role |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key` for a wrong key |
| `requests/unknown-model.json`, `errors/recorded-unknown-model.json` | 404 `model_not_found` |
| `requests/embeddings-model.json`, `errors/recorded-embeddings-model.json` | 404 `model_not_supported` for an embeddings model |
| `requests/free-tier-exhausted.json`, `errors/recorded-free-tier-exhausted.json` | 403 `AllocationQuota.FreeTierOnly`: the free quota of `qwen-plus` is used up on a free-tier account |
| `requests/stream-only-model.json`, `errors/recorded-stream-only-model.json` | 400: `qwq-plus-2025-03-05` thinks only and answers only streamed |
| `requests/thinking-only-model-off.json`, `errors/recorded-thinking-only-model-off.json` | 400: `enable_thinking: false` for `qwen3.7-max-preview`, which thinks only |
| `requests/thinking-budget-over-limit.json`, `errors/recorded-thinking-budget-over-limit.json` | 400: `max_completion_tokens` must be greater than `thinking_budget` |
| `requests/thinking-budget-zero.json`, `errors/recorded-thinking-budget-zero.json` | 400: `thinking_budget` must be from 1 to 131072 |
| `requests/thinking-budget-kimi.json`, `errors/recorded-thinking-budget-kimi.json` | 400: `kimi-k3` refuses `thinking_budget` |
| `requests/reasoning-effort-max.json`, `errors/recorded-reasoning-effort-max.json` | 400: `qwen3.7-flash` takes `reasoning_effort` `none` to `xhigh`, not `max` |
| `requests/allowed-tools.json`, `errors/recorded-allowed-tools.json` | 400 for `tool_choice` `allowed_tools` |
| `requests/image-too-small.json`, `errors/recorded-image-too-small.json` | 400: an 8×8 image is below the vision model's 10-pixel minimum |
| `models/list-recorded.json`, `models/retrieve-recorded.json` | `GET /v1/models` (all 102 models, unchanged, `owned_by: "system"`, `first_id` `model-id-0`) and `GET /v1/models/qwen3.7-flash`, recorded with `curl` |
| `errors/list-invalid-api-key.json`, `errors/retrieve-not-found.json` | the listing's 401 for a wrong key and `GET /v1/models/kaeso-example-no-such-model`'s 404, recorded with `curl` |

Every error body is OpenAI's `error` object with `type`, `code` and `param`, beside a top-level `request_id` and sometimes `id`, so they take the usual error round trip. No 429 or 5xx body was recorded.
