---
title: Fixtures and recordings
description: The real payloads the codecs are tested against, and how to record new ones from the providers.
order: 7
---

Every codec change starts with a fixture: a real request body, reply, stream or error, in `crates/kaeso-codecs/fixtures`. Every fixture round-trips through its own codec, the cross-dialect matrix runs every generation fixture through every other dialect, the [SDK conformance suite](../testing.md#sdk-conformance) serves them to the official SDKs, the gateway's mock provider answers with them, and the fuzz targets start from them.

## Layout

| Folder | Dialect |
| --- | --- |
| `anthropic_messages/` | Anthropic Messages (`/v1/messages`, `/v1/messages/count_tokens`) |
| `openai_chat/` | OpenAI Chat Completions (`/v1/chat/completions`) |
| `openai_responses/` | OpenAI Responses (`/v1/responses`, `/v1/responses/input_tokens`) |
| `openai_embeddings/` | OpenAI Embeddings (`/v1/embeddings`) |

Each dialect folder has `requests/`, `responses/`, `streams/` where the endpoint streams, and `errors/`.

| File | Holds |
| --- | --- |
| `requests/<name>.json` | The exact request body |
| `requests/<name>.headers.json` | Allowlisted forwarded headers sent with that request |
| `responses/<name>.json` | The exact 200 body |
| `streams/<name>.sse` | The exact SSE bytes |
| `errors/<name>.json` and `errors/<name>.meta.json` | An error body, and `{"status": …, "headers": {…}}` |

Some endpoints have subfolders of their own: `count_tokens/` for Anthropic and `input_tokens/` for OpenAI Responses, inside `requests/` and `responses/`, and `models/` for model listings (`anthropic_messages/models/`, and `openai_chat/models/`, which all three OpenAI codecs use).

- `unknown-*` fixtures carry made-up fields, blocks or events on purpose, to test `Extras` and `Unknown` handling.
- Fixtures a codec must refuse (`stateful-*`, `n-2*`, `input-audio*`) are tested for their `Unsupported` error instead of a round trip.

## Provenance

Every dialect folder and its `requests/`, `responses/`, `streams/`, `errors/` and `models/` folders have a `README.md` that lists each file, what it covers and where it came from:

| Mark | Source |
| --- | --- |
| `doc:` | An example from the provider's documentation |
| `sdk:` | A test or recording from an official SDK |
| `schema:` | A body built from the pinned OpenAPI schemas |
| `RECORDED.md` | A live recording made with `cargo xtask record`, with the model and the time |

A dialect folder's `README.md` also names the pinned specifications and SDK versions its fixtures follow, and the conventions they share, such as ids and token counts that agree between a stream and its response. Add a row there with every new fixture.

## Recording live responses

`cargo xtask record` sends request fixtures to the real provider and stores what comes back. It is opt-in, runs only on your machine and never in CI, and needs the provider's API key in the environment:

```bash
export ANTHROPIC_API_KEY=…
cargo xtask record anthropic_messages tools-single thinking-signature
cargo xtask record anthropic_messages --count --all
```

```bash
export OPENAI_API_KEY=…
cargo xtask record openai_chat --all --dry-run
cargo xtask record openai_responses --count --all
```

| Option | What it does |
| --- | --- |
| `<dialect>` | `anthropic_messages`, `openai_chat`, `openai_responses` or `openai_embeddings` |
| `<names>` | Request fixtures to send, without `.json` |
| `--all` | Every request fixture of the dialect except `stateful-*`, `unknown-*`, `n-2*` and `input-audio*`, which a provider would refuse or which carry made-up fields |
| `--count` | The token-count endpoint and its fixtures in `requests/count_tokens/` or `requests/input_tokens/`; `anthropic_messages` and `openai_responses` only |
| `--dry-run` | Prints what would be sent and sends nothing |

- It reads `requests/<name>.json` and, when it exists, `<name>.headers.json`, whose allowlisted headers (`anthropic-beta`, `anthropic-version`, `openai-beta`) are sent along. Anthropic requests carry `anthropic-version: 2023-06-01` unless that file names another version.
- It writes `responses/recorded-<name>.json` for a 2xx reply (in `responses/count_tokens/` or `responses/input_tokens/` with `--count`), `streams/recorded-<name>.sse` when the request has `"stream": true` (the raw bytes as received), or `errors/recorded-<name>.json` (`.txt` when the body isn't JSON) with `recorded-<name>.meta.json` (the status and any `retry-after`) for an error, and adds a row with the model and the UTC time to `RECORDED.md` in that folder. JSON bodies are indented by two spaces and keep the provider's key order.
- `ANTHROPIC_BASE_URL` and `OPENAI_BASE_URL` change the API addresses.
- API keys are read from the environment and never written, and request headers are never stored.
- Before anything is written, it removes the query string from every URL that carries a cloud signing credential (AWS Signature Version 4, Google Cloud signed URLs, Azure shared access signatures), such as the presigned links a web search can return; everything else stays byte for byte.

The code is in `crates/xtask/src/record.rs`. Review every recorded file before committing it: it holds whatever the model said, and it must not contain personal data. The round-trip tests pick recorded fixtures up by themselves.
