# Recorded fixtures

Written by `cargo xtask record`. Provenance: live API of Moonshot AI (`moonshotai`).

| File | Request fixture | Model | Recorded (UTC) |
|---|---|---|---|
| `recorded-unknown-model.json` | `openai_chat/providers/moonshotai/requests/unknown-model.json` | `kimi-no-such-model` | 2026-10-30 17:00 |
| `recorded-temperature.json` | `openai_chat/providers/moonshotai/requests/temperature.json` | `kimi-k2.6` | 2026-10-30 17:01 |
| `recorded-top-p.json` | `openai_chat/providers/moonshotai/requests/top-p.json` | `kimi-k2.6` | 2026-10-30 17:01 |
| `recorded-logit-bias-refused.json` | `openai_chat/providers/moonshotai/requests/logit-bias-refused.json` | `kimi-k2.6` | 2026-10-30 17:01 |
| `recorded-developer-role.json` | `openai_chat/providers/moonshotai/requests/developer-role.json` | `kimi-k2.6` | 2026-10-30 17:02 |
| `recorded-tool-choice-required-thinking.json` | `openai_chat/providers/moonshotai/requests/tool-choice-required-thinking.json` | `kimi-k2.6` | 2026-10-30 17:02 |
| `recorded-invalid-api-key.json` | `openai_chat/providers/moonshotai/requests/invalid-api-key.json`, sent with a made-up key | `kimi-k2.6` | 2026-10-30 17:03 |
| `recorded-rate-limited.json` | `openai_chat/providers/moonshotai/requests/rate-limited.json`, sent until the account's limit of 3 requests a minute was reached; organization and key ids replaced with zeros | `kimi-k2.6` | 2026-10-30 17:03 |
