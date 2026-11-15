# Fixtures: Venice (`venice`) on `openai_chat`

Venice's OpenAI-compatible Chat Completions, `POST https://api.venice.ai/api/v1/chat/completions`, with `Authorization: Bearer`. Venice's base URL is `https://api.venice.ai/api/v1`, so Kaeso's base URL is `https://api.venice.ai/api` and its `/v1/...` paths land on the documented ones.

No key was available. `responses/greeting-doc.json` is the example reply of Venice's OpenAPI document. The other replies, the streams and the unknown-model error were recorded against Venice by the cassette tests of [Rig](https://github.com/0xPlaygrounds/rig) and are published in its repository ([Venice cassettes](https://github.com/0xPlaygrounds/rig/tree/46b86a65b2b23a963f36deed384e0869da30931d/crates/rig-cassette/fixtures/cassettes/venice), commit `46b86a65b2b23a963f36deed384e0869da30931d`); each file is that recording's request and reply, with the request ids kept and the `created` times as recorded. The key error was answered live, with a made-up key.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| OpenAPI document (`ChatCompletionRequest`, the reply example, `StandardError`, `DetailedError`), version `20261113.174621` | https://docs.venice.ai/swagger.yaml |
| API spec (base URL, `venice_parameters`, `include_venice_system_prompt` defaulting to `true`) | https://docs.venice.ai/api-reference/api-spec |
| OpenAI migration guide (strict top-level fields, the usage chunk on every stream) | https://docs.venice.ai/guides/getting-started/openai-migration |
| Error codes (the error shapes, 402 without a key, 401 with a wrong one) | https://docs.venice.ai/api-reference/error-codes |
| Reasoning models (`reasoning_content`) | https://docs.venice.ai/guides/features/reasoning-models |
| Rig's recorded cassettes | https://github.com/0xPlaygrounds/rig/tree/46b86a65b2b23a963f36deed384e0869da30931d/crates/rig-cassette/fixtures/cassettes/venice |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json`, `responses/greeting-doc.json` | `zai-org-glm-5-1` with `venice_parameters`; the reply's `cost`, `venice_parameters`, `choices[].stop_reason`, `prompt_logprobs` and `null` usage details | doc |
| `requests/tool-call-reasoning-published.json`, `responses/tool-call-reasoning-published.json` | `qwen3-235b-a22b-thinking-2507` with one function tool; `reasoning_content` beside the tool call, `finish_reason: "tool_calls"` | published (Rig, `reasoning_matrix/tool_unary.yaml`) |
| `requests/cached-published.json`, `responses/cached-published.json` | `qwen3-5-9b` with `prompt_cache_key`; `cached_tokens` and the undocumented `usage.cache_read_input_tokens`, reasoning tokens, vLLM's extra reply fields | published (Rig, `prompt_caching/cache_key_stable.yaml`) |
| `requests/disable-thinking-published.json`, `responses/disable-thinking-published.json` | `venice_parameters.disable_thinking` and `strip_thinking_response` | published (Rig, `venice_parameters/disable_thinking.yaml`) |
| `requests/disable-thinking-stream-published.json`, `streams/disable-thinking-stream-published.sse` | a whole stream ending in a separate `choices: []` chunk with `usage` and `cost` | published (Rig, `raw_stream_capture_matrix/stream_raw_exposes_terminal_cost.yaml`) |
| `requests/reasoning-stream-published.json`, `streams/reasoning-stream-published.sse` | `delta.reasoning_content` and then `delta.content`; chunks 1–4, 818–822 and the last two of 882, cut by hand | published (Rig, `reasoning_matrix/text_streamed.yaml`) |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `{"error": "Authentication failed"}` | live, with a made-up key |
| `errors/unknown-model-published.json` | 404 `{"error": "Specified model not found: …"}` | published (Rig, `error_envelope/nonexistent_model_streaming_error_preserves_status_and_body.yaml`) |
| `errors/validation-doc.json` | 400 `{"error": "Invalid request parameters", "details", "issues"}` | doc |
| `errors/context-length-doc.json` | 400 in OpenAI's shape with `code: "context_length_exceeded"` | doc |
| `errors/insufficient-balance-doc.json` | 402 `{"error": "Insufficient USD or Diem balance …"}` | doc |
| `errors/payload-too-large-doc.json` | 413 `{"code": "PAYLOAD_TOO_LARGE", "error"}` | doc |
| `models/list-recorded.json` | `GET /v1/models` without a key, cut to three of its 141 text models, with Venice's `type`, `context_length` and `model_spec` | live, without a key |

## Where Venice differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| Venice prepends a system prompt of its own unless `venice_parameters.include_venice_system_prompt` is `false` | the profile writes that key when the request doesn't set it (`with_option_defaults`), also into a Chat app's own `venice_parameters`, whose other keys go as written | API spec |
| unknown top-level fields answer 400 | sent; Venice refuses them itself | OpenAI migration guide |
| `user` is discarded, there is no `safety_identifier`; `anon_user_id` is Venice's own | `without_end_user()`; `anon_user_id` in extras | OpenAPI document |
| `metadata` and `store` are taken without acting on them | refused before sending | OpenAPI document |
| reasoning in `message.reasoning_content` and `delta.reasoning_content` | a reasoning block through `reasoning_fields(["reasoning_content"])` | reasoning models page |
| `venice_parameters`, `cost`, `choices[].stop_reason`, `prompt_logprobs` and vLLM's fields in replies | extras | OpenAPI document; Rig |
| every stream ends with a `choices: []` chunk with `usage` and `cost`, also without `stream_options.include_usage` | usage | OpenAI migration guide; Rig |
| most errors are `{"error": "<text>"}`, validation errors add `details` and `issues` | `KppError` of the kind of the status, the text as its message | error codes |
| `GET /v1/models` lists text models only; `?type=embedding` lists the embeddings models | `ModelListing::Partial` | OpenAPI document; live |
