# anthropic_messages / providers / fireworks-ai (Fireworks AI)

Fireworks AI's Anthropic-compatible Messages API, `POST https://api.fireworks.ai/inference/v1/messages`, with `Authorization: Bearer` (the same key; `x-api-key` reaches the same key check). A model sends in this format when its **Sends as** value is Messages; Chat Completions stays the default.

No key was available. The reply and the stream are built from the [Create a Message](https://docs.fireworks.ai/api-reference/anthropic-messages) reference and the [Anthropic compatibility](https://docs.fireworks.ai/tools-sdks/anthropic-compatibility) page, read on 2026-11-09; only the error was answered live, with a made-up key. Its request id is replaced with zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/thinking-effort.json`, `responses/thinking-effort.json` | `output_config.effort`, which Fireworks maps to `reasoning_effort`; a `thinking` block before the text; `cache_creation_input_tokens` always 0 | doc |
| `requests/text-stream.json`, `streams/text-stream.sse` | `thinking: {type: "disabled"}`; `message_start` with zero usage, the counts on the one `message_delta` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `authentication_error` in Anthropic's envelope with Fireworks' `request_id` | live, with a made-up key |

What the compatibility page says Fireworks doesn't do, left to Fireworks' own 400 rather than refused, because it answers them with an error instead of ignoring them: `document` blocks, `image` blocks with `source.type: "file"`, server tools such as `web_search_20250305`, a forced `tool_choice` with thinking on, a prefill with thinking on. `POST /v1/messages/count_tokens` answers 404 (probed live), so token counts are refused before sending.
