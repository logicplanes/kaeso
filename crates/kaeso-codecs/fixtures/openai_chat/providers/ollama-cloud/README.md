# Fixtures: Ollama Cloud (`ollama-cloud`) on `openai_chat`

Ollama Cloud's OpenAI-compatible Chat Completions API, `POST https://ollama.com/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://ollama.com`. Chat Completions is Ollama Cloud's `generation` dialect; model ids keep Ollama's colon tag (`gpt-oss:20b`, `deepseek-v4.1-flash:1017`).

## Sources (read 2026-10-30)

| Source | Address |
|---|---|
| Cloud models and API keys | https://docs.ollama.com/cloud |
| Authentication | https://docs.ollama.com/api/authentication |
| OpenAI compatibility (supported fields, `reasoning_effort`, `reasoning`) | https://docs.ollama.com/api/openai-compatibility |
| Errors | https://docs.ollama.com/api/errors |

Every reply, stream and error below is a live recording made with `cargo xtask record` on 2026-10-30 with a free-plan account, with `gpt-oss:20b` and `gemma4:31b`, two of the models the free plan includes; each is listed with its model in the `RECORDED.md` of its folder. The model listing, the retrieved model and `errors/retrieve-not-found.json` are plain `GET` requests made the same day; Ollama answers them without a key. Request ids in error messages (`ref: …`) are kept as recorded.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning-effort.json`, `requests/reasoning-object.json` | `reasoning_effort: "low"`, and Ollama's `reasoning: {effort}` object | written for recording; OpenAI compatibility page |
| `requests/reasoning-stream.json` | reasoning, streamed with `include_usage` | written for recording |
| `requests/reasoning-sent-back.json` | `reasoning` on an earlier assistant turn | written for recording |
| `requests/reasoning-tools.json`, `requests/reasoning-tools-stream.json` | one function tool with reasoning, as a reply and as a stream | written for recording |
| `requests/tool-result.json` | a tool call and its result sent back | written for recording |
| `requests/image.json` | a base64 PNG in a user message | written for recording |
| `requests/max-completion-tokens-ignored.json`, `requests/max-tokens-honoured.json` | the same request with `max_completion_tokens: 20` and with `max_tokens: 20` | written for recording |
| `requests/unknown-model.json`, `requests/model-outside-plan.json` | a model that doesn't exist, and a paid model on the free plan | written for recording |
| `responses/recorded-reasoning-effort.json`, `responses/recorded-reasoning-object.json`, `responses/recorded-reasoning-sent-back.json` | `message.reasoning` beside `content`; `system_fingerprint: "fp_ollama"`; `usage.prompt_tokens_details.cached_tokens` | live, `gpt-oss:20b` |
| `responses/recorded-reasoning-tools.json` | reasoning, `content: ""` and a tool call with `index` | live, `gpt-oss:20b` |
| `responses/recorded-max-completion-tokens-ignored.json` | 150 completion tokens and `finish_reason: "stop"`: `max_completion_tokens` is ignored | live, `gemma4:31b` |
| `responses/recorded-max-tokens-honoured.json` | 20 completion tokens and `finish_reason: "length"` | live, `gemma4:31b` |
| `responses/recorded-plain-developer-string.json`, `responses/recorded-system-name-multiturn.json`, `responses/recorded-stop-string-seed-penalties.json`, `responses/recorded-max-tokens-length.json` | the dialect's requests of the same name: `developer` messages, `name`, `stop`, `seed` and penalties are accepted | live, `gemma4:31b` |
| `responses/recorded-tools-single.json`, `responses/recorded-tools-parallel.json`, `responses/recorded-tool-result.json`, `responses/recorded-image.json` | tool calls, two parallel tool calls, an answer to a tool result, an image | live, `gemma4:31b` |
| `streams/recorded-reasoning-stream.sse` | `delta.reasoning` beside `delta.content: ""`, then text; a finish chunk with an empty `delta`; a usage chunk with `choices: []` | live, `gpt-oss:20b` |
| `streams/recorded-reasoning-tools-stream.sse` | reasoning deltas, then the whole tool call in one delta | live, `gpt-oss:20b` |
| `streams/recorded-stream-doc.sse`, `streams/recorded-stream-include-usage.sse` | text deltas without and with the usage chunk | live, `gemma4:31b` |
| `errors/recorded-invalid-api-key.json` | 401 `api_error` `Unauthorized` | live, recorded with a made-up key |
| `errors/recorded-unknown-model.json` | 404 `not_found_error` that suggests a free model | live |
| `errors/recorded-model-outside-plan.json` | 402 `api_error` for a model the plan doesn't include | live, `kimi-k2.6` |
| `errors/recorded-content-array-all-roles.json` | 400 `invalid_request_error` `invalid message format`: Ollama refuses a `refusal` content part on an assistant message; text parts in every role are accepted | live, `gemma4:31b` |
| `errors/retrieve-not-found.json` | 404 from `GET /v1/models/{model}` for an unknown model | live, `GET /v1/models/no-such-model:1b` |
| `models/list-recorded.json` | `GET /v1/models`: OpenAI's list with `owned_by: "ollama"`, and a top-level `models` array in Codex's model-catalogue shape. Three of 23 models are kept in both arrays, unchanged | live |
| `models/retrieve-recorded.json` | `GET /v1/models/gpt-oss:20b`, a tagged id in the path | live |

## Where Ollama Cloud differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| `max_completion_tokens` is ignored; `max_tokens` is honoured | the `chat` profile sends every app's output limit as `max_tokens` (`max_tokens()`) | live: `recorded-max-completion-tokens-ignored`, `recorded-max-tokens-honoured` |
| `message.reasoning` and `delta.reasoning` carry the model's thinking; Ollama takes `reasoning` back on assistant turns | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])`; Anthropic Messages and Responses apps get a thinking block or reasoning item, and their thinking returns to Ollama as `reasoning` | live; OpenAI compatibility page |
| `reasoning: {effort}` next to `reasoning_effort` | a request extra for Ollama; refused when the target speaks another format | OpenAI compatibility page; live |
| `logprobs`, `top_logprobs` and `logit_bias` are accepted and ignored | refused before sending by the `chat` profile (`refusing`) | live probe, 2026-10-30 |
| `tool_choice` other than `auto`, and `response_format` with `json_object` or `json_schema`, are accepted and ignored by the cloud models tried | sent as written; Ollama decides | live probe with `gemma4:31b`, `gpt-oss:20b` and `nemotron-3-nano:30b`, 2026-10-30 |
| `n` above 1 is ignored | refused by KPP before sending | live probe |
| a `refusal` content part on an assistant message: 400 | Ollama's error reaches the app in its own format | live |
| `system_fingerprint: "fp_ollama"` | an extra of the reply, round-trips | live |
| errors use OpenAI's shape with `type` `api_error`, `not_found_error` or `invalid_request_error`; a paid model on the free plan is 402 | the kind comes from the status; 401 and 402 reach apps as Kaeso's key-refused 502 | live |
| `/v1/embeddings` answers 404, and there is no token-count endpoint | `embeddings` and `count_tokens` are `None` | live probe |
| `/v1/responses` and `/v1/messages` answer too | not in `dialects`: Kaeso translates every app format to Chat Completions | live probe |
