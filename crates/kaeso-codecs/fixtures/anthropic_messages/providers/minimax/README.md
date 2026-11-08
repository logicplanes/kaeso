# Fixtures: MiniMax (`minimax`) on `anthropic_messages`

MiniMax's Anthropic-compatible API, `POST https://api.minimax.io/anthropic/v1/messages`, with `x-api-key` (MiniMax also takes `Authorization: Bearer`, which wins when both are sent) and `anthropic-version: 2023-06-01`. It serves `MiniMax-M3` and `MiniMax-M3.1-Flash` only.

## Sources (read 2026-11-08)

| Source | Address |
|---|---|
| Anthropic API compatibility: the supported and ignored parameters and message types | https://platform.minimax.io/docs/api-reference/text-anthropic-api |
| The OpenAPI document of that API, with the request, reply, stream, error, token-count and model examples | https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json |
| Model listing | https://platform.minimax.io/docs/api-reference/models/anthropic/list-models |
| Error codes | https://platform.minimax.io/docs/api-reference/errorcode |
| Automatic prompt caching | https://platform.minimax.io/docs/api-reference/text-prompt-caching |
| The China endpoint, `https://api.minimax.cn/anthropic` | https://platform.minimaxi.com/docs/api-reference/text-anthropic-api |

No working key was available. Only the error below is a live recording; every other file is an example from MiniMax's OpenAPI document, copied with its values and key order and indented, and is marked `doc:`. Nothing here holds a key or personal data.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/invalid-api-key.json` | A plain request to `MiniMax-M3` | written for recording |
| `errors/recorded-invalid-api-key.json` | 401 `authentication_error` in Anthropic's shape, with MiniMax's `request_id`, for a made-up key | live, `cargo xtask record`, `MiniMax-M3` |
| `requests/deep-thinking.json`, `responses/deep-thinking.json` | `thinking: {type: "adaptive"}` and `output_config.effort: "max"`; a reply with a signed `thinking` block, a 32-hex id without `msg_` and no `stop_sequence` | doc: the "Deep Thinking" example |
| `requests/tool-use-auto.json`, `responses/tool-use-auto.json` | One tool with `tool_choice: {type: "auto"}`; a reply with thinking, text and `tool_use` (`call_function_…` ids) | doc: the "Tool Use" example |
| `requests/image-url-adaptive.json`, `responses/image-url-adaptive.json` | An image by URL | doc: the "Image Understanding" example |
| `requests/video-url.json`, `responses/video-url.json` | MiniMax's own `video` block by URL, which KPP keeps as an unknown block | doc: the "Video Understanding" example |
| `requests/image-url-stream.json`, `streams/image-url-stream.sse` | A stream: `message_start` with `service_tier`, `ping`, thinking with a `signature_delta`, text, and the usage on `message_delta` | doc: the "Stream" example, which lists the events as JSON; the `event:` and `data:` lines around each are added |
| `errors/unsupported-content-type.json` | 400 `invalid_request_error` for a content type MiniMax doesn't take | doc: the 400 example |
| `errors/rate-limited.json`, `errors/overloaded.json` | 429 `rate_limit_error` and 529 `overloaded_error` | doc: the 429 and 529 examples |
| `models/list-documented.json`, `models/retrieve-documented.json` | `GET /v1/models` and `GET /v1/models/MiniMax-M3` in Anthropic's shape | doc: the model listing examples |

## Probes (2026-11-08, not stored)

- Without a key, with a made-up key in `x-api-key` and with one as `Authorization: Bearer`, `/v1/messages`, a streamed `/v1/messages`, `/v1/messages/count_tokens`, `/v1/models` and `/v1/models/MiniMax-M3` all answer the same 401 as the recorded error, while a made-up path answers 404 `404 page not found` in plain text.
- The China endpoint answers the same 401.

## What KPP does with MiniMax's own fields

- Replies, streams, errors and model listings round-trip unchanged: the id without `msg_`, the missing `stop_sequence`, `service_tier` on `message_start` and the `video` block need nothing of their own.
- MiniMax's Messages profile refuses what its compatibility table lists as ignored: `top_k`, `stop_sequences`, `mcp_servers`, `container`, `context_management` (left out when it only clears thinking), a `temperature` other than 1 and a `top_p` other than 0.95, and a `tool_choice` other than `auto`.
- Token counts are refused: MiniMax's example answer puts `input_tokens` under `usage`, not at the top level as Anthropic does.
