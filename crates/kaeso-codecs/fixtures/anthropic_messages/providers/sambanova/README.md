# anthropic_messages / providers / sambanova (SambaNova)

SambaNova's Anthropic-compatible Messages API, `POST https://api.sambanova.ai/v1/messages`, and its token count, `POST /v1/messages/count_tokens`. The key works as `Authorization: Bearer`, which Kaeso sends, or as `x-api-key`. A model sends in this format when its **Sends as** value is Messages; Chat Completions stays the default.

No key was available. The reply is built from the [Messages API](https://docs.sambanova.ai/docs/en/features/messages) page and the `Message` schema of SambaNova's OpenAPI document, read on 2026-11-09; only the error was answered live, with a made-up key, and its request id is replaced with zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/thinking.json`, `responses/thinking.json` | `gpt-oss-120b` without thinking settings: a `thinking` block without `signature`, then the text; usage without the cache counts, which SambaNova doesn't send yet | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `authentication_error` in Anthropic's envelope, with `request_id` | live, with a made-up key |

SambaNova answers server tools, `document` blocks and images by URL with 400, which reaches the app. `count_tokens` answers `{"input_tokens": n}`, Anthropic's shape.
