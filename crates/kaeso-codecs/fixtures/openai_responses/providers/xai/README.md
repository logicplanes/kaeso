# `fixtures/openai_responses/providers/xai`

SpaceX, provider id `xai`, over the xAI Responses API at `https://api.x.ai/v1/responses`, with `authorization: Bearer`. It is the provider's `generation` dialect: xAI calls Responses its primary interface and keeps reasoning and tool state there.

Sources, read 2026-10-19:

- Responses reference: https://docs.x.ai/developers/rest-api-reference/inference/responses
- Migrating from Chat Completions: https://docs.x.ai/developers/model-capabilities/text/comparison
- Models and pricing: https://docs.x.ai/developers/models

Every reply, stream and error below is a live recording made with `cargo xtask record` on 2026-10-19 (see `RECORDED.md` in each folder), with `grok-4.3`. `streams/recorded-claude-code-web-search-stream.sse` is gateway-relayed instead, with `grok-4.7`, because it answers the request Kaeso writes for an Anthropic app. `errors/recorded-invalid-api-key.json` was recorded the same day with a plain `POST` and a placeholder key. The model listing is in `openai_chat/providers/xai/models/`.

The `codex-*` recordings of 2026-10-25 are a Codex 0.164 thread with `grok-4.7`, gateway-relayed, not raw xAI recordings: `cargo xtask record` sent them to Kaeso 0.4.2 at `ai.logicplanes.com` (`XAI_BASE_URL`) on 2026-10-25, which passed them to xAI. That release predates the handling of Codex's developer messages, reasoning replay and web search options, so it sent these requests to xAI unchanged. It re-encodes replies in the same format and errors in the OpenAI shape, so these two errors carry `{"error": {"message", "type"}}` instead of xAI's own `{"error": "<message>"}`.

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
| `streams/recorded-claude-code-web-search-stream.sse` | `grok-4.7` answering the request Kaeso writes for Claude Code's `WebSearch` (`filters.allowed_domains`, `max_turns: 8`, `include: ["web_search_call.action.sources"]`): `web_search_call` items added with `action: {type: search, query: "", sources: []}` and done with the query and its sources, reasoning items between them | gateway-relayed (Kaeso at `ai.logicplanes.com`), 2026-10-25 |
| `errors/recorded-reasoning-encrypted.json` | 400 `invalid-argument` for an `encrypted_content` from another provider | live |
| `errors/recorded-invalid-api-key.json` | 400 `invalid-argument` for a wrong key | live, plain `POST` |
| `requests/codex-turn-one.json` | Codex's first turn as Kaeso sends it to SpaceX: two leading `developer` items with `id`s, `exec_command`, `web_search` without `external_web_access`, `include: ["reasoning.encrypted_content"]` | `requests/codex-developer-resume.json` |
| `requests/codex-turn-two.json` | the second turn as Kaeso sends it: the first reply's reasoning item without `content: null`, its message, a new user turn | built from `recorded-codex-turn-one` the way Codex replays it |
| `requests/codex-turn-two-null-content.json` | the same turn as Codex writes it, with `content: null` on the reasoning item | as above |
| `requests/codex-web-search-access.json` | the first turn with `external_web_access: true`, as Codex sends it | Codex 0.164 with `web_search = "live"` |
| `responses/recorded-codex-turn-one.json` | a reasoning item with SpaceX's `encrypted_content` and the answer `ok` | gateway-relayed (Kaeso 0.4.2 at `ai.logicplanes.com`) |
| `responses/recorded-codex-turn-two.json` | the second turn recalls the first: `4817` | gateway-relayed (Kaeso 0.4.2 at `ai.logicplanes.com`) |
| `errors/recorded-codex-turn-two-null-content.json` | 400 `Could not decode the compaction blob. Ensure it is unmodified from the compact response.` for `content: null` | gateway-relayed (Kaeso 0.4.2 at `ai.logicplanes.com`) |
| `errors/recorded-codex-web-search-access.json` | 400 `Argument not supported: external_web_access` | gateway-relayed (Kaeso 0.4.2 at `ai.logicplanes.com`) |
| `requests/codex-multi-agent.json` | a Codex 0.165 subagent's first request: its task arrives as an `agent_message` from `/root`, with an `input_text` header and the task in `encrypted_content` as plain text | cut down from production request `req_01m64abc8neh89hzb6q6v4qq4d` (2026-10-29), with the task's text shortened |
| `requests/codex-multi-agent-root.json` | the root agent's later turn: its own `agent_message` to the subagent, then the subagent's answer | the shape of production request `req_01m64ad47regztt49ahenq9ts7` (2026-10-29), cut down |
| `errors/recorded-codex-agent-message.json` | 422 `unknown item type "agent_message"`, listing the item types SpaceX takes (it takes `custom_tool_call_output` too, which the list leaves out: `responses/recorded-codex-gpt-history.json`) | gateway-relayed (production Kaeso at `ai.logicplanes.com`, 2026-10-29) |
| `requests/codex-gpt-history.json` | `requests/codex-model-switch.json` resumed on `grok-4.7`: GPT-6 history with `phase`, OpenAI's `web_search_call` and a `custom_tool_call`, as Kaeso sends it | `requests/codex-model-switch.json` |
| `responses/recorded-codex-gpt-history.json` | SpaceX takes that history as sent and answers `OK` | gateway-relayed (Kaeso 0.4.2 at `ai.logicplanes.com`) |
