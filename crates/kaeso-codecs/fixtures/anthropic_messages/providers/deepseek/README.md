# anthropic_messages / providers / deepseek (DeepSeek)

DeepSeek's Anthropic-compatible Messages API, `POST https://api.deepseek.com/anthropic/v1/messages` (also `/anthropic/messages`). The key works as `x-api-key` or `Authorization: Bearer`, which Kaeso sends; when both arrive, `Authorization` wins. `anthropic-version` is ignored. A request that arrives in this dialect is sent here as it came.

No key was available. Everything is built from the [Anthropic API guide](https://api-docs.deepseek.com/guides/anthropic_api) and the [Messages API reference](https://api-docs.deepseek.com/api/create-message.md), read on 2026-11-25. Message ids are replaced with zeros, keeping DeepSeek's UUID shape.

| file | what it covers | provenance |
|---|---|---|
| `requests/say-hello.json`, `responses/say-hello.json` | the reference's curl request and 200 example: a message id that is a UUID, not `msg_…`; `usage.service_tier: "standard"`; `cache_creation_input_tokens` always 0 | doc |
| `requests/say-hello-stream.json`, `streams/say-hello-stream.sse` | the reference's streaming example: a `ping` event between the block events, the full usage again on `message_delta`, no `data: [DONE]` | doc (the request is the curl example with `stream: true`) |
| `requests/thinking-cache-hit.json`, `responses/thinking-cache-hit.json` | `thinking: {type: "adaptive"}` (DeepSeek treats it as `enabled`) with `output_config.effort`; a `thinking` block with `signature`; `input_tokens` without the cache hits, which are in `cache_read_input_tokens`; `output_tokens_details.thinking_tokens` | schema (`AnthropicMessage`, `AnthropicMessageUsage`); texts and counts constructed |
| `errors/empty-messages.json` | 400 `invalid_request_error` for an empty `messages` array | doc |
| `streams/error-timeout-before-start.sse`, `streams/error-timeout-after-start.sse` | the unnamed `{"error": {"message": …}}` frame DeepSeek sends when generation doesn't start in time, alone and after `message_start` and `ping` | doc (the Streaming section of the Anthropic API guide; message text made up) |

Documented differences from Anthropic:

- `input_tokens` excludes cache hits; the total input is `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`, and `cache_creation_input_tokens` is always 0 (caching is automatic). The final `message_delta.usage` is the request's usage; it is not added to `message_start`'s.
- `thinking.budget_tokens` is ignored, `adaptive` means `enabled`. Kaeso's Anthropic check refuses `enabled` without `budget_tokens`, although DeepSeek takes it; the thinking fixture therefore uses `adaptive`.
- Ignored: `container`, `mcp_servers`, `service_tier`, `top_k`, every `cache_control`, `citations`, `tool_result.is_error`, `disable_parallel_tool_use`. `output_config` takes only `effort`. Not supported: `redacted_thinking`, `search_result`, code-execution and MCP blocks.
- In thinking mode `tool_choice: {type: "tool"}` answers 400, and `any` answers 400 when only client tools are declared.
- A client tool takes `name`, `input_schema` and `description` and no `type`: `type` selects a server tool, and `type: "custom"` answers 422 "Failed to deserialize the JSON body into the target type: tools[0]: unknown variant `custom`, …". The profile's `without_custom_tool_type()` sends an app's tools without it, such as LiteLLM's in `requests/litellm-custom-tool.json` of the dialect's own folder.
- If waiting for generation to start times out, DeepSeek may send an unnamed `data: {"error":{"message":"..."}}` frame, without an SSE event name and without a top-level `type`, and close the connection; errors after the stream started may also just close it. `streams/error-timeout-before-start.sse` holds that frame alone and `streams/error-timeout-after-start.sse` after `message_start` and a `ping`; the decoder reads it as DeepSeek's error, with its message, and an Anthropic app gets it as an `error` event.
