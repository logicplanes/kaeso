# `fixtures/openai_chat/providers/xai`

SpaceX, provider id `xai`, over the xAI OpenAI-compatible Chat Completions API at `https://api.x.ai/v1/chat/completions`, with `authorization: Bearer`. xAI calls this API the predecessor of Responses, which is the provider's `generation` dialect; Chat is reached through a model's `egress_dialect`.

Sources, read 2026-10-19:

- Chat Completions reference: https://docs.x.ai/developers/rest-api-reference/inference/chat-completions
- Models reference: https://docs.x.ai/developers/rest-api-reference/inference/models
- Legacy and deprecated endpoints: https://docs.x.ai/developers/rest-api-reference/inference/legacy
- Models and pricing: https://docs.x.ai/developers/models

Every reply, stream and error below is a live recording made with `cargo xtask record` on 2026-10-19 (see `RECORDED.md` in each folder), with `grok-4.3` and `grok-4.20-0309-non-reasoning`. The model listing, the retrieved model and `errors/recorded-invalid-api-key.json` were recorded the same day with a plain `GET`/`POST`; the invalid key was a placeholder. xAI has no embeddings endpoint and no token-count endpoint in either dialect's shape (`/v1/tokenize-text` takes `{model, text}`; `/v1/responses/input_tokens` answers 405), so there are no embedding or count fixtures.

xAI sends errors as `{"error": "<message>"}`, with `"code": "invalid-argument"` on a 400. The OpenAI codecs read the string as the message and answer apps in the OpenAI error shape, so each error's `.meta.json` has an `encoded` body that the round trip compares against. A wrong key is a 400 `invalid-argument`, not a 401. Its message starts with `Incorrect API key provided`, which SpaceX's `refused_key_messages` in `crates/kaeso-core/src/provider.rs` names, so the gateway reports a refused key; apps of the OpenAI formats still get the 400.

| file | what it covers | provenance |
|---|---|---|
| `requests/reasoning-content.json` | `grok-4.3` with `reasoning_effort: "low"` | chat-completions reference |
| `requests/reasoning-content-stream.json` | the same, streamed with `stream_options.include_usage` | as above |
| `requests/presence-penalty-reasoning-model.json` | `presence_penalty` for a reasoning model, which xAI refuses | chat-completions reference, `presence_penalty` |
| `requests/live-search-deprecated.json` | `search_parameters`, which xAI now answers with 410 | chat-completions reference, `search_parameters` |
| `responses/recorded-plain-developer-string.json` | text reply with `system_fingerprint`, `service_tier`, `usage.num_sources_used`, `usage.cost_in_usd_ticks`, `prompt_tokens_details.text_tokens` and `image_tokens` | live, `grok-4.20-0309-non-reasoning` |
| `responses/recorded-reasoning-content.json` | `message.reasoning_content`, `reasoning_tokens` and `cached_tokens` in `usage` | live |
| `responses/recorded-json-schema.json` | Structured Outputs reply | live |
| `responses/recorded-tools-single.json` | tool call with `content: ""` | live |
| `responses/recorded-tools-parallel.json` | parallel tool calls | live |
| `streams/recorded-stream-include-usage.sse` | usage chunk with empty `choices`, then `[DONE]` | live |
| `streams/recorded-reasoning-content-stream.sse` | `delta.reasoning_content` before `delta.content` | live |
| `errors/recorded-presence-penalty-reasoning-model.json` | 400 `{"code": "invalid-argument", "error": …}` | live |
| `errors/recorded-live-search-deprecated.json` | 410 `{"error": …}` without `code` | live, `grok-4.20-0309-non-reasoning` |
| `errors/recorded-invalid-api-key.json` | 400 `invalid-argument` for a wrong key | live, plain `POST` |
| `models/list-recorded.json` | `GET /v1/models`: `aliases`, `context_length`, prices in USD cents per 100 million tokens, `capabilities.reasoning_effort`, image `pricing` tiers | live, plain `GET` |
| `models/retrieve-recorded.json` | `GET /v1/models/grok-4.3` | live, plain `GET` |
