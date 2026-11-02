# Fixtures: Cerebras (`cerebras`) on `openai_chat`

Cerebras Inference's OpenAI-compatible Chat Completions, `POST https://api.cerebras.ai/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.cerebras.ai`. Chat Completions is Cerebras's only dialect: `/v1/responses`, `/v1/messages` and `/v1/embeddings` answer 404.

## Sources (read 2026-11-02)

| Source | Address |
|---|---|
| Chat Completions reference (request fields, `reasoning`, `time_info`, usage) | https://inference-docs.cerebras.ai/api-reference/chat-completions |
| OpenAI compatibility (`n`, images, `tools` with `response_format`, `reasoning_format`) | https://inference-docs.cerebras.ai/resources/openai |
| API keys | https://inference-docs.cerebras.ai/console/api-keys |

Every reply, stream and error named `recorded-…` below is a live recording made with `cargo xtask record` on 2026-11-02, with `gpt-oss-120b` and `qwen-3.8-27b`, the two models the test account lists; each is listed with its model in the `RECORDED.md` of its folder. The account allowed `gpt-oss-120b` 5 requests and 30,000 tokens a minute, so the recordings were made one at a time and most requests set `max_completion_tokens`. The model listing, the retrieved model, `errors/retrieve-not-found.json` and `errors/list-invalid-api-key.json` are plain `GET` requests made the same day. Nothing is redacted: the replies hold no key, account or person data.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/basic.json`, `responses/recorded-basic.json` | a `developer` message to `gpt-oss-120b`; `message.reasoning`, `system_fingerprint`, `time_info`, the prediction token counts in `completion_tokens_details` | live |
| `requests/structured-output.json`, `responses/recorded-structured-output.json` | `response_format` with a strict JSON schema | live |
| `requests/tools.json`, `responses/recorded-tools.json` | one function tool; `reasoning` and a tool call with an id such as `4e80d65a6`, without `content` | live |
| `requests/reasoning.json`, `responses/recorded-reasoning.json` | `qwen-3.8-27b`, which thinks by default; `message.reasoning`, `prompt_tokens_details.image_tokens` | live |
| `requests/reasoning-effort-none.json`, `responses/recorded-reasoning-effort-none.json` | `qwen-3.8-27b` at `reasoning_effort: "none"`: no reasoning, 0 reasoning tokens | live |
| `requests/reasoning-format-raw.json`, `responses/recorded-reasoning-format-raw.json` | `reasoning_format: "raw"`: the thinking in `content`, ended by `</think>` | live |
| `requests/reasoning-format-hidden.json`, `responses/recorded-reasoning-format-hidden.json` | `reasoning_format: "hidden"`: no reasoning text, reasoning tokens still counted | live |
| `requests/logprobs.json`, `responses/recorded-logprobs.json` | `logprobs` with `top_logprobs`, and `choices[].reasoning_logprobs` beside them | live |
| `requests/max-completion-tokens-reasoning.json`, `responses/recorded-max-completion-tokens-reasoning.json` | `max_completion_tokens: 20` spent on reasoning: `finish_reason: "length"`, no `content` | live |
| `requests/sampling.json`, `responses/recorded-sampling.json` | `temperature`, `top_p`, `seed`, a `stop` list and both penalties; the stop sequence ended the reasoning before any `content` | live |
| `requests/user-hints.json`, `responses/recorded-user-hints.json` | `user`, `prompt_cache_key` and `service_tier: "auto"`, all taken | live |
| `requests/logit-bias-honoured.json`, `responses/recorded-logit-bias-honoured.json` | `logit_bias` of +100 on the token ids 1000 to 1049; the biased tokens `...` and ` </` appear in the reply | live |
| `requests/reasoning-sent-back.json`, `responses/recorded-reasoning-sent-back.json` | `reasoning` on an earlier assistant message, which Cerebras takes | live |
| `requests/prefill.json`, `responses/recorded-prefill.json` | a final assistant message `1. Red\n2.`: the reply starts over and repeats it | live |
| `requests/image.json`, `responses/recorded-image.json` | a 32×32 base64 PNG to `qwen-3.8-27b`; 64 `image_tokens` | live |
| `requests/system-mid-conversation.json`, `responses/recorded-system-mid-conversation.json` | a `system` message after the first turn, which `gpt-oss-120b` takes | live |
| `requests/stream.json`, `streams/recorded-stream.sse` | `delta.reasoning`, then the answer; `usage` and `time_info` on the finish chunk without `stream_options` | live |
| `requests/stream-usage.json`, `streams/recorded-stream-usage.sse` | `include_usage` with thinking off: the usage still comes on the finish chunk, and no chunk with `choices: []` follows | live, `qwen-3.8-27b` |
| `requests/stream-reasoning.json`, `streams/recorded-stream-reasoning.sse` | `qwen-3.8-27b`'s streamed reasoning | live |
| `requests/stream-tools.json`, `streams/recorded-stream-tools.sse` | reasoning, then a tool call in three deltas | live |
| `requests/stream-json-object.json`, `streams/recorded-stream-json-object.sse` | JSON mode streamed, which the compatibility page calls unsupported; one delta carries the end of the reasoning and the start of the answer | live |
| `requests/developer-role-qwen.json`, `errors/recorded-developer-role-qwen.json` | 400 `Unexpected message role` for a `developer` message to `qwen-3.8-27b` | live |
| `requests/system-mid-conversation-qwen.json`, `errors/recorded-system-mid-conversation-qwen.json` | 400 `System message must be at the beginning.` from `qwen-3.8-27b` | live |
| `requests/reasoning-effort-none-gpt-oss.json`, `errors/recorded-reasoning-effort-none-gpt-oss.json` | 400 for `reasoning_effort: "none"` to `gpt-oss-120b` | live |
| `requests/reasoning-effort-minimal.json`, `errors/recorded-reasoning-effort-minimal.json` | 400 for `reasoning_effort: "minimal"` | live |
| `requests/reasoning-content-sent-back.json`, `errors/recorded-reasoning-content-sent-back.json` | 400 for `reasoning_content` on an assistant message | live |
| `requests/assistant-refusal.json`, `errors/recorded-assistant-refusal.json` | 400 for `refusal: null` on an assistant message | live |
| `requests/tool-without-parameters.json`, `errors/recorded-tool-without-parameters.json` | 400 `Tool "get_time" is missing function parameters.` | live |
| `requests/tools-with-response-format.json`, `errors/recorded-tools-with-response-format.json` | 400 `"tools" is incompatible with "response_format"` | live |
| `requests/safety-identifier-field.json`, `requests/metadata-field.json`, `requests/store-field.json`, `requests/verbosity-field.json`, and their `errors/recorded-…` | 400 `property '…' is unsupported` for a field Cerebras doesn't declare, with `param: "validation_error"` | live |
| `requests/n-above-one.json`, `errors/recorded-n-above-one.json` | `n: 2`; KPP refuses it before sending (`Unsupported` `n`), Cerebras answers 400 | live |
| `requests/image-url.json`, `errors/recorded-image-url.json` | 400 `Remote image URLs are not supported` for an `https` image | live, `qwen-3.8-27b` |
| `requests/image-gpt-oss.json`, `errors/recorded-image-gpt-oss.json` | 400 for an image to the text-only `gpt-oss-120b` | live |
| `requests/unknown-model.json`, `errors/recorded-unknown-model.json` | 404 `model_not_found` | live |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `Wrong API Key` | live, with a made-up key |
| `requests/rate-limited.json`, `errors/recorded-rate-limited.json` | 429 `too_many_tokens_error` with `retry-after: 60` | live |
| `errors/retrieve-not-found.json`, `errors/list-invalid-api-key.json` | 404 from `GET /v1/models/{model}`, and 401 from `GET /v1/models` with a made-up key | live |
| `models/list-recorded.json` | `GET /v1/models`: both models, `created: 0`, `owned_by: "Cerebras"` | live |
| `models/retrieve-recorded.json` | `GET /v1/models/gpt-oss-120b` | live |

## Where Cerebras differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| `message.reasoning` and `delta.reasoning` carry the thinking, and Cerebras takes `reasoning` back on assistant messages | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])`; thinking from apps in other formats goes back as `reasoning` | reference; live: `recorded-reasoning-sent-back` |
| a `reasoning_content` on an assistant message gets 400, and Grok CLI replays its reasoning there | the `chat` profile sends a Chat app's replayed `reasoning_content` as `reasoning` (`replay_reasoning_as_read()`) | live: `errors/recorded-reasoning-content-sent-back` |
| `time_info` (`queue_time`, `prompt_time`, `completion_time`, `total_time`, `created`) on replies and on the finish chunk; `choices[].reasoning_logprobs`; `system_fingerprint` | extras, round-trip | reference; live |
| `completion_tokens_details.accepted_prediction_tokens` and `rejected_prediction_tokens`, `prompt_tokens_details.image_tokens` | `raw` usage; the cached and reasoning tokens are typed | live |
| streams send `usage` and `time_info` on the finish chunk, whether or not `include_usage` is set, and never a chunk with `choices: []` | the typed usage comes from the finish chunk | live: `streams/recorded-stream`, `streams/recorded-stream-usage` |
| any field Cerebras doesn't declare gets 400 `property '…' is unsupported`, among them `safety_identifier`, `metadata`, `store`, `verbosity`, `modalities` and `web_search_options` | sent as written, so the app gets Cerebras's 400; the profile sends the end user's id as `user` (`end_user_as_user()`), and `compatible()` already leaves out a Responses app's `verbosity` | live: `errors/recorded-*-field`, a probe of `modalities` and `web_search_options` |
| `frequency_penalty`, `presence_penalty`, `logit_bias`, `logprobs`, `top_logprobs`, `seed`, `stop`, `user`, `prompt_cache_key`, `parallel_tool_calls`, `service_tier`, `prediction` are taken | sent as written; a probe on 2026-11-02 saw `frequency_penalty: 2` shorten a repeated word from 30 to 6 times and `parallel_tool_calls: false` keep one call | reference; live |
| the `developer` role gets 400 from Qwen's chat template | the `chat` profile sends `system` (`system_role()`) | live: `errors/recorded-developer-role-qwen` |
| Qwen's chat template refuses a `system` message after the first turn, which Claude Code and Codex send | for apps in other formats, the `chat` profile moves the text of such messages into the next user message (`leading_system_only()`), as it does for Anthropic Messages | live: `errors/recorded-system-mid-conversation-qwen` |
| a final assistant message is answered anew and repeated, not continued | refused before sending (`without_prefill()`) | live: `responses/recorded-prefill` |
| a `refusal` on an assistant message gets 400, also `refusal: null` | refused before sending (`refusing_in_messages(["refusal"])`) | live: `errors/recorded-assistant-refusal` |
| a function without `parameters` gets 400 from `gpt-oss-120b`'s chat template | the `chat` profile sends `{"type": "object", "properties": {}}` (`with_tool_parameters()`) | live: `errors/recorded-tool-without-parameters` |
| `reasoning_effort`: `low`, `medium`, `high` for `gpt-oss-120b`; `none`, `low`, `medium`, `high` for `qwen-3.8-27b`; other values get 400 | the catalogue's effort levels from KPMR | reference; live: `errors/recorded-reasoning-effort-*` |
| `max_completion_tokens` and `max_tokens` both count reasoning; both at once get 400 | sent as the app wrote them | reference; live: `recorded-max-completion-tokens-reasoning` |
| `reasoning_format` (`parsed`, `raw`, `hidden`, `none`) and `clear_thinking` (request) | extras; refused when the request goes to another dialect, like other Chat-only fields | reference |
| `n` above 1 | refused by KPP before sending | reference; live |
| images only as data URIs, and only to models that take images | sent as written; Cerebras answers 400 otherwise | compatibility page; live: `errors/recorded-image-url`, `errors/recorded-image-gpt-oss` |
| errors carry OpenAI's `message`, `type`, `param` and `code` without the `error` object, sometimes with `id: ""` | `KppError` with Cerebras's message and `param`; the kind follows the status, so 401 `wrong_api_key` is `Authentication` and 429 `too_many_tokens_error` is `RateLimited`; apps in Chat format get the error in OpenAI's shape | live |
| `GET /v1/models` lists the models the key may call | `ModelList`; a wrong key gets 401, so a model check spots it | live |
| no token-count and no embeddings endpoint | `count_tokens` and `embeddings` are `None` | live probe |
| rate limits as `x-ratelimit-{limit,remaining}-{requests,tokens}-{minute,hour,day}` headers | not read | live |
