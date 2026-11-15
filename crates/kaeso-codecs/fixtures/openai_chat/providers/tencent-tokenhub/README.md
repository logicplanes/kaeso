# Fixtures: Tencent TokenHub (`tencent-tokenhub`) on `openai_chat`

Tencent TokenHub's OpenAI-compatible Chat Completions, `POST https://tokenhub.tencentmaas.com/v1/chat/completions`, with `Authorization: Bearer`. TokenHub's SDK base URL is `https://tokenhub.tencentmaas.com/v1` (Guangzhou); every path sits under `/v1` on the host, which is Kaeso's base URL `https://tokenhub.tencentmaas.com`. Singapore and Tencent Cloud's international site are other hosts, each with keys of its own. Model ids are lowercase and some name their maker, such as `deepseek/deepseek-v4-pro`.

No key was available. The requests are the cURL examples of TokenHub's Hunyuan guide and the replies and the stream are the replies the guide prints for them, unchanged: their ids are the guide's own `REPLACED_ID`. Another project's issue publishes the first two chunks of a real `hy-mt2-pro` stream, which match the guide's stream (the role alone in the first chunk, without a `finish_reason` key); they aren't a fixture, as a stream cut off after two chunks can't round-trip. The key errors were answered live without a valid key; the other errors are the error code reference's examples. The model listing needs a key, so `models/list-doc.json` is the API usage page's example.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| API usage (hosts, regions, `Authorization: Bearer`, `GET /v1/models` and its reply) | https://cloud.tencent.com/document/product/1823/130078 |
| Chat Completions field reference (roles, both output limits, `safety_identifier`, `video_url`, `reasoning_content`, undeclared fields ignored) | https://cloud.tencent.com/document/product/1823/135872 |
| Hunyuan guide (the requests, replies and stream below) | https://cloud.tencent.com/document/product/1823/132252 |
| Thinking (`thinking`, `reasoning_effort` per model) | https://cloud.tencent.com/document/product/1823/131208 |
| Error codes (the error shape, the codes, an integer `code` on 429) | https://cloud.tencent.com/document/product/1823/131595 |
| Two chunks of a `hy-mt2-pro` stream, which match `streams/stream-doc.sse` | https://github.com/higress-group/higress/issues/3953 |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/introduce-doc.json`, `responses/introduce-doc.json` | `hy4` with `temperature`; `reasoning_content` beside the answer, usage with `cached_tokens` and `reasoning_tokens` | doc |
| `requests/thinking-doc.json`, `responses/thinking-doc.json` | TokenHub's `thinking: {"type": "enabled"}`, which stays in extras | doc |
| `requests/reasoning-effort-doc.json`, `responses/reasoning-effort-doc.json` | `reasoning_effort: "high"`; usage with only the three totals | doc |
| `requests/tools-doc.json`, `responses/tools-doc.json` | one function tool and `tool_choice: "auto"`; content and reasoning beside the tool call, `finish_reason: "tool_calls"` | doc |
| `requests/agent-tools-doc.json`, `responses/agent-tools-doc.json` | a system message, `reasoning_effort` and a tool; the tool call id `chatcmpl-tool-b39c6375f812783a` | doc |
| `requests/reasoning-sent-back-doc.json` | the assistant turn sent back with its `reasoning_content` and tool call, then the tool result | doc |
| `requests/json-schema-doc.json`, `responses/json-schema-doc.json` | `response_format` `json_schema` | doc |
| `requests/translation-doc.json`, `responses/translation-doc.json` | `hy-mt2-plus`, a translation model without reasoning (`reasoning_tokens: 0`) | doc |
| `requests/stream-doc.json`, `streams/stream-doc.sse` | `stream_options.include_usage`; the role in the first chunk only, the usage in a last chunk with `choices: []` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `401002` with `type: "authentication_error"` and `source: "gateway"` | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 `401002` without an `Authorization` header | live |
| `errors/recorded-models-missing-api-key.json` | 401 `401001` for `GET /v1/models` without a key | live |
| `errors/invalid-api-key-doc.json` | 401 `401002` with `type: "gateway_error"` and no `source`, as the reference writes it | doc |
| `errors/missing-parameter-doc.json` | 400 `400002` | doc |
| `errors/model-not-found-doc.json` | 400 `400004`, an unknown model | doc |
| `errors/rate-limit-doc.json` | 429 with `code` as the number `429001` and no `type` | doc |
| `errors/upstream-doc.json` | 502 `502001` with `source: "upstream"` and `upstream_status` | doc |
| `models/list-doc.json` | `GET /v1/models` with TokenHub's `name` and `status` | doc |

## Where Tencent TokenHub differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | field reference |
| reasoning comes as `reasoning_content` on the message and the delta, and Hy4 and Hy3 want it back in the next request | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning_content"])`, sent back on assistant messages | field reference; Hunyuan guide |
| `video_url` content parts | sent as written (`with_video_url()`) | field reference |
| undeclared fields, such as `modalities` and `audio`, are ignored without a word; the reference has no `logit_bias`, `metadata`, `store`, `service_tier`, `prediction`, `prompt_cache_retention`, `functions` or `function_call` | refused before sending (`refusing([…])`) | field reference ("会被静默忽略") |
| `thinking`, `enable_thinking`, `thinking_budget`, `interleaved_thinking`, `reasoning_split`, `web_search_options`, `extra_body` | request `Extras`; refused when the request goes to another dialect | field reference; thinking page |
| `reasoning_effort` `no_think` turns Hy4 and Hy3 thinking off | not a KPP level; the catalogue lists `high` (Hy4) and `low`, `high` (Hy3), so the gateway sends the lowest level for `none` | thinking page |
| `reasoning_details`, `search_info`, `usage.cache_read_tokens`, `usage.cache_write_tokens` | extras and raw usage | field reference |
| errors carry `message_zh`, a six-digit `code`, `source` and `request_id`; `type` differs between the reference and live answers, and a 429 may carry `code` as a number without `type` | `KppError` of the kind of the status; the codes `400003` (`ContextTooLong`), `400004` (`NotFound`) and `451001` (`ContentFiltered`) give the kind where the status says less | error codes; live |
| `GET /v1/models` needs the key and adds `name` and `status`; there is no `GET /v1/models/{model}` (404 `404 page not found`) | `ModelList`, TokenHub's fields in extras | API usage; live |
