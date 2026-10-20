# `fixtures/openai_responses/providers/xai`

SpaceX, provider id `xai`, over the xAI Responses API at `https://api.x.ai/v1/responses`, with `authorization: Bearer`. It is the provider's `generation` dialect: xAI calls Responses its primary interface and keeps reasoning and tool state there.

Sources, read 2026-10-19:

- Responses reference: https://docs.x.ai/developers/rest-api-reference/inference/responses
- Migrating from Chat Completions: https://docs.x.ai/developers/model-capabilities/text/comparison
- Models and pricing: https://docs.x.ai/developers/models

Every reply, stream and error below is a live recording made with `cargo xtask record` on 2026-10-19 (see `RECORDED.md` in each folder), with `grok-4.3`. `errors/recorded-invalid-api-key.json` was recorded the same day with a plain `POST` and a placeholder key. The model listing is in `openai_chat/providers/xai/models/`.

xAI's errors are `{"error": "<message>"}`, with `"code": "invalid-argument"` on a 400; each error's `.meta.json` has the `encoded` OpenAI-shaped body the codec sends apps instead.

| file | what it covers | provenance |
|---|---|---|
| `requests/reasoning-stream.json` | `grok-4.3` with `reasoning.effort: "low"`, streamed | responses reference |
| `requests/presence-penalty-ignored.json` | `presence_penalty`, which the Responses API doesn't support | responses reference, `presence_penalty` |
| `responses/recorded-text-input-string.json` | a `reasoning` item with a `summary` before the message; echoed request fields; `usage.num_sources_used`, `num_server_side_tools_used`, `cost_in_usd_ticks`, `context_details` | live |
| `responses/recorded-reasoning-effort-high.json` | `reasoning.effort: "high"`, `reasoning_tokens` in `usage` | live |
| `responses/recorded-tools-function.json` | function call after a reasoning item | live |
| `responses/recorded-tools-parallel.json` | parallel function calls | live |
| `responses/recorded-web-search.json` | `web_search_call` items, `url_citation` annotations, `server_side_tool_usage_details` | live |
| `responses/recorded-presence-penalty-ignored.json` | 200; xAI ignores `presence_penalty` and echoes `0.0` | live |
| `streams/recorded-text-basic-stream.sse` | `reasoning_summary_*` events before the text; no `[DONE]` | live |
| `streams/recorded-reasoning-stream.sse` | reasoning summary deltas | live |
| `streams/recorded-reasoning-summary-stream.sse` | `reasoning.summary: "auto"` with `include: ["reasoning.encrypted_content"]` | live |
| `streams/recorded-function-call-stream.sse` | function call arguments in one delta | live |
| `streams/recorded-web-search-stream.sse` | `web_search_call.*` and `output_text.annotation.added` events | live |
| `errors/recorded-reasoning-encrypted.json` | 400 `invalid-argument` for an `encrypted_content` from another provider | live |
| `errors/recorded-invalid-api-key.json` | 400 `invalid-argument` for a wrong key | live, plain `POST` |
