---
title: Fixtures and recordings
description: The real payloads the codecs are tested against, and how to record new ones from the providers.
order: 8
---

Every codec change starts with a fixture: a real request body, reply, stream or error, in `crates/kaeso-codecs/fixtures`. Every fixture round-trips through its own codec, and the cross-dialect matrix runs every generation fixture through every other dialect. The [SDK conformance suite](../testing.md#sdk-conformance) serves the fixtures to the official SDKs, the gateway's mock provider answers with them, and the fuzz targets start from them.

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
| `errors/<name>.json` and `errors/<name>.meta.json` | An error body, and `{"status": …, "headers": {…}}`. When the codec answers apps with another body, such as the OpenAI error shape for SpaceX's string `error`, `"encoded"` holds that body and the round trip compares against it |

Some endpoints have subfolders of their own: `count_tokens/` for Anthropic and `input_tokens/` for OpenAI Responses, inside `requests/` and `responses/`, and `models/` for model listings (`anthropic_messages/models/`, and `openai_chat/models/`, which all three OpenAI codecs use).

Recordings from other providers that speak a dialect, such as an OpenAI-compatible API, go into `<dialect>/providers/<id>/`, named by the provider's id in `crates/kaeso-core/src/provider.rs`. That folder has the same `requests/`, `responses/`, `streams/` and `errors/` subfolders, each only where the provider has files. `requests/` holds requests that only this provider accepts; every name in it must differ from the dialect's own request fixtures.

The round-trip tests, the decoder tests and the cross-dialect matrix read every provider folder along with the dialect's own, and name its files `providers/<id>/<name>`. A new provider recording therefore adds entries to the matrix snapshots; review them and accept them with `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix`. The SDK conformance suite serves only the dialect's own fixtures; the gateway's mock provider serves a provider's file when a test names it as `providers/<id>/<name>`.

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

A provider folder has one `README.md` of its own: the provider's name and id, its API reference with the date you read it, the base URL and models you recorded with, and a row for every file in its subfolders. Each subfolder's `RECORDED.md` names the provider in its first lines.

## Recording live responses

`cargo xtask record` sends request fixtures to the real provider and stores what comes back. It is opt-in, runs only on your machine and never in CI, and needs the provider's API key in the environment:

```bash
export ANTHROPIC_API_KEY=…
cargo xtask record anthropic_messages tools-single thinking-enabled-budget
cargo xtask record anthropic_messages --count --all
```

```bash
export OPENAI_API_KEY=…
cargo xtask record openai_chat --all --dry-run
cargo xtask record openai_responses --count --all
```

Any other provider in `crates/kaeso-core/src/provider.rs` records the same way with `--provider`, usually with `--model` because the request fixtures name OpenAI and Anthropic models:

```bash
export <ID>_API_KEY=…
cargo xtask record openai_chat --provider <id> --model <model> --dry-run tools-single stream-include-usage
cargo xtask record openai_chat --provider <id> --model <model> tools-single stream-include-usage
```

| Option | What it does |
| --- | --- |
| `<dialect>` | `anthropic_messages`, `openai_chat`, `openai_responses` or `openai_embeddings` |
| `<names>` | Request fixtures to send, without `.json` |
| `--all` | Every request fixture of the dialect except `stateful-*`, `unknown-*`, `n-2*` and `input-audio*`, which a provider would refuse or which carry made-up fields |
| `--count` | The token-count endpoint and its fixtures in `requests/count_tokens/` or `requests/input_tokens/`; `anthropic_messages` and `openai_responses` only, and only for a provider whose `count_tokens` is that dialect |
| `--dry-run` | Prints what would be sent and sends nothing |
| `--provider <id>` | The provider to record from, by its id; it must list the dialect in its `dialects`. `anthropic` for `anthropic_messages` and `openai` for the others by default |
| `--model <id>` | Sends this model instead of the one each request fixture names, and records it in `RECORDED.md` |

- It reads `requests/<name>.json`, and for another provider also `providers/<id>/requests/<name>.json`, and, when it exists, `<name>.headers.json`, whose allowlisted headers (`anthropic-beta`, `anthropic-version`, `openai-beta`) are sent along. Anthropic requests carry `anthropic-version: 2023-06-01` unless that file names another version.
- It writes a 2xx reply to `responses/recorded-<name>.json`, or to `responses/count_tokens/` or `responses/input_tokens/` with `--count`. A request with `"stream": true` gets `streams/recorded-<name>.sse`, the raw bytes as received. An error gets `errors/recorded-<name>.json` (`.txt` when the body isn't JSON) and `recorded-<name>.meta.json` with the status and any `retry-after`. Each recording adds a row with the model and the UTC time to `RECORDED.md` in that folder. `openai` and `anthropic` record into the dialect's folders, every other provider into `providers/<id>/`. JSON bodies are indented by two spaces and keep the provider's key order.
- The key comes from `<ID>_API_KEY`, the provider's id in capitals with underscores for hyphens: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `FIREWORKS_AI_API_KEY` for a provider `fireworks-ai`. `<ID>_BASE_URL` replaces the provider's default base URL, which is the one the gateway uses. The key goes into the provider's auth header: `authorization: Bearer` or `x-api-key`.
- API keys are read from the environment and never written, and request headers are never stored.
- Before anything is written, it removes the query string from every URL that carries a cloud signing credential: AWS Signature Version 4, Google Cloud signed URLs and Azure shared access signatures. A web search can return such presigned links. Everything else stays byte for byte.

The code is in `crates/xtask/src/record.rs`. Review every recorded file before committing it: it holds whatever the model said, and it must not contain personal data. The round-trip tests pick recorded fixtures up by themselves.
