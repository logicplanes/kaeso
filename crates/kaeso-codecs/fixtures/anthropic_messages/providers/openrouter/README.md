# anthropic_messages / providers / openrouter (OpenRouter)

OpenRouter's Messages API, `POST https://openrouter.ai/api/v1/messages`, for every model, with `Authorization: Bearer`, which Kaeso sends. A request that arrives in this dialect is sent here as it came.

No key was available. The fixtures are the examples of the [Create a message](https://openrouter.ai/docs/api/api-reference/anthropic-messages/create-a-message.md) reference, read on 2026-11-25. Their ids (`msg_abc123`, `msg_01XFDUDYJgAACzvnptvVoYEL`) are the reference's own placeholders.

| file | what it covers | provenance |
|---|---|---|
| `requests/hello.json`, `responses/hello.json` | the reference's request and 200 example: `container`, `stop_details` and most usage fields sent as `null` (`cache_creation_input_tokens`, `cache_read_input_tokens`, `inference_geo`, `output_tokens_details`, `server_tool_use`), `citations: []` on a text block | doc |
| `requests/hello-stream.json`, `streams/hello-stream.sse` | the per-event examples of `MessagesStreamEvents` in order: `message_start` with the null usage fields, a `ping`, `message_delta` with `input_tokens: null` and `container: null` | doc (the request is the example with `stream: true`; the events' `model` is the reference's `claude-sonnet-4-5-20250929`) |

Documented differences from Anthropic:

- Fields are sent as `null` where Anthropic omits them or sends a number; `message_delta.usage.input_tokens` can be `null`.
- Errors carry `request_id` (often `null`) and an OpenRouter `error.error_type` beside Anthropic's `error.type`; a stream `error` event carries the same. With the `X-OpenRouter-Metadata` header the reply gains `openrouter_metadata`, and a reply may name its upstream `provider`.
- The reference documents no `count_tokens` endpoint.
