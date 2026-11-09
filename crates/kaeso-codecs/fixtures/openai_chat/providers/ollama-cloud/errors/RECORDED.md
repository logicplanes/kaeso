# Recorded fixtures

Written by `cargo xtask record`. Provenance: live API of Ollama Cloud (`ollama-cloud`).

| File | Request fixture | Model | Recorded (UTC) |
|---|---|---|---|
| `recorded-model-outside-plan.json` | `openai_chat/providers/ollama-cloud/requests/model-outside-plan.json` | `kimi-k2.6` | 2026-10-30 15:21 |
| `recorded-unknown-model.json` | `openai_chat/providers/ollama-cloud/requests/unknown-model.json` | `no-such-model:1b` | 2026-10-30 15:21 |
| `recorded-content-array-all-roles.json` | `openai_chat/requests/content-array-all-roles.json` | `gemma4:31b` | 2026-10-30 15:21 |
| `recorded-invalid-api-key.json` | `openai_chat/requests/plain-developer-string.json`, sent with a made-up key | `gemma4:31b` | 2026-10-30 15:21 |
| `recorded-retired-model.json` | a plain chat completion, relayed through Kaeso's OpenAI-format passthrough instead of `cargo xtask record` | `nemotron-3-nano:30b` | 2026-11-09 13:55 |
