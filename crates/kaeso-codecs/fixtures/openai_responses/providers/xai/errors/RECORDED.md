# Recorded fixtures

Written by `cargo xtask record`. Provenance: live API of xAI (`xai`).

| File | Request fixture | Model | Recorded (UTC) |
|---|---|---|---|
| `recorded-reasoning-encrypted.json` | `openai_responses/requests/reasoning-encrypted.json` | `grok-4.3` | 2026-10-19 00:39 |
| `recorded-codex-turn-two-null-content.json` | `openai_responses/providers/xai/requests/codex-turn-two-null-content.json` | `grok-4.7` | 2026-10-25 10:55 |
| `recorded-codex-web-search-access.json` | `openai_responses/providers/xai/requests/codex-web-search-access.json` | `grok-4.7` | 2026-10-25 10:55 |
| `recorded-codex-agent-message.json` | `openai_responses/providers/xai/requests/codex-multi-agent.json` | `grok-4.7` | 2026-10-29 17:12 |

The `recorded-codex-*` files are gateway-relayed, not raw xAI: recorded through Kaeso 0.4.2 at `ai.logicplanes.com` on 2026-10-25, and `recorded-codex-agent-message.json` from a production refusal there on 2026-10-29; see `../README.md`.
