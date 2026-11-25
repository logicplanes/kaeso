# anthropic_messages / providers / alibaba (QwenCloud)

QwenCloud's Anthropic-compatible Messages API, `POST https://ap-southeast-1.maas.qwencloudapi.com/apps/anthropic/v1/messages` (the other regions' hosts likewise). It sits at the host root, beside the `/compatible-mode/v1` Chat Completions root, not under it. The key works as `x-api-key` or `Authorization: Bearer`, which Kaeso sends. There is no `/v1/models` and no token count there. A request for a Qwen model that arrives in this dialect is sent here as it came.

No key was available. The fixtures are built from the [Anthropic Messages API](https://docs.qwencloud.com/api-reference/chat/anthropic.md) reference, read on 2026-11-25. The message id is replaced with zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/who-are-you.json`, `responses/who-are-you.json` | the reference's request example and 200 example: a `thinking` block with an empty `signature`, then the text | doc |
| `requests/thinking-stream.json`, `streams/thinking-stream.sse` | the reference's thinking curl example (`budget_tokens: 1024`, streamed); `message_start` usage with only `input_tokens` and `output_tokens`, the cache counts first on `message_delta`; the `thinking` block ends with an empty `signature_delta` | doc (request, event rules); the stream itself is constructed from the `AnthropicStreamEvent` and `AnthropicUsage` descriptions |

Documented differences from Anthropic:

- `signature` is empty unless the summarized thinking mode is on; then a single `signature_delta` carries the complete ciphertext before the block closes.
- In a stream, `message_start.usage` has only `input_tokens` and `output_tokens`; `cache_creation_input_tokens`, `cache_read_input_tokens` and `server_tool_use` come on `message_delta`. `stop_sequence` is always `null`, and `stop_reason` is one of `end_turn`, `max_tokens`, `tool_use`.
- `output_config.format` (JSON schema) is a platform extension; `temperature` is in `[0, 2)`. The model list is explicit (Qwen Max, Plus, Flash, Coder, VL, open models, and some third-party models).
