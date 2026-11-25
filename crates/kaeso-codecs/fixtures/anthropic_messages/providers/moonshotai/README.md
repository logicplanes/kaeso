# anthropic_messages / providers / moonshotai (Moonshot AI, Kimi)

Kimi's Anthropic-compatible Messages API, `POST https://api.moonshot.ai/anthropic/v1/messages`, for the Kimi K3 models only (`kimi-k3.1-pro`, `kimi-k3.1-pro-highspeed`, `kimi-k3`). The reference lists only `Authorization: Bearer`, which Kaeso sends. A request for a K3 model that arrives in this dialect is sent here as it came.

No key was available. The fixtures are built from the `MessagesRequest`, `MessagesResponse` and `MessagesStreamEvent` schemas of the [Messages API](https://platform.kimi.ai/docs/api/messages.md) reference, read on 2026-11-25; the page shows no filled-in reply, so texts, ids, signatures and counts are constructed.

| file | what it covers | provenance |
|---|---|---|
| `requests/thinking-effort-cache.json`, `responses/thinking-effort-cache.json` | top-level `cache_control` with `ttl: "1h"` (the only place Kimi reads it) and `output_config.effort`; a `thinking` block with `signature`; usage with the `cache_creation` TTL breakdown and `output_tokens_details.thinking_tokens` | schema (the request follows the page's cache example) |
| `requests/tools-stream.json`, `streams/tools-stream.sse` | a streamed `thinking` block (`thinking_delta`, then `signature_delta`) followed by a `tool_use` block built from `input_json_delta` fragments, ending with `stop_reason: "tool_use"` | schema |

Documented differences from Anthropic:

- `cache_control` works only at the top level; markers inside `messages` are ignored. Total input is `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`.
- `tool_choice` takes `auto`, `any` and `none`, not `tool`. Up to 5 `stop_sequences` of at most 32 bytes. `output_config.effort` takes `low`, `high` and `max` (default `max`). Images by URL must be `ms://<file_id>` references.
- `stop_reason` is `end_turn` also when a stop sequence matched; there is no `stop_sequence` value.
- Errors use Anthropic's envelope with a `request_id`, except 401, which answers `{"error": {"message", "type", "code"}}`.
