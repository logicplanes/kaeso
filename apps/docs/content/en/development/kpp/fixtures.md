---
title: Fixtures and recordings
nav: Fixtures
description: The real payloads the codecs are tested against, and how to record new ones from the providers.
order: 9
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

A dialect folder's `README.md` also names the pinned specifications and SDK versions its fixtures follow. Its fixtures follow the [conventions](#conventions) below. Add a row there with every new fixture.

A provider folder has one `README.md` of its own. It holds:

- the provider's name and id;
- its API reference, with the date you read it;
- the base URL and models you recorded with;
- a row for every file in its subfolders.

Each subfolder's `RECORDED.md` names the provider in its first lines.

## Conventions

Fixtures that aren't live recordings follow these rules, so that the tests can compare them exactly:

- **Pairs.** A request, a reply and a stream with the same base name belong together. The stream is the same reply as the response and accumulates to exactly that response; the dialect folder's `README.md` names any exception.
- **Numbers that agree.** Totals are the sum of their parts, and a stream reports the same usage as its reply.
- **Made-up values in the live shape.** Ids follow the provider's live format. Signatures and encrypted reasoning are random but well formed; a provider would refuse them on replay.
- **Tiny but valid files.** An image is a 1×1 PNG, a PDF one page with a correct cross-reference table, and audio a WAV header with eight silent samples.
- **Formatting.** JSON files are indented by two spaces and write non-ASCII as UTF-8. SSE `data:` lines hold single-line JSON, as on the wire.
- **Invented content only in `unknown-*`.** No other fixture carries a field, block or event the provider doesn't document.

### Anthropic Messages

- Every request assumes `anthropic-version: 2023-06-01`, which isn't stored. A `.headers.json` exists only where a request needs `anthropic-beta`.
- Ids look like `msg_01…`, `toolu_01…`, `srvtoolu_01…`, `file_011C…` and `req_011C…`.
- A `Message` carries `model`, `id`, `type`, `role`, `content`, `stop_reason`, `stop_sequence`, `stop_details` and `usage`, as the SDK's recorded streams and the documentation show. `container` and `diagnostics` are left out, because they are only set when used.
- `usage` carries `input_tokens`, the two cache counts, the `cache_creation` breakdown, `output_tokens`, `service_tier` and `inference_geo`. `output_tokens_details` appears only after thinking, and `server_tool_use` only after a server tool ran.
- `cache_creation_input_tokens` equals the sum of its breakdown, and `thinking_tokens` is at most `output_tokens`.
- `message_start` carries the reply's usage with a small `output_tokens`, and the last `message_delta` the cumulative counts.

### OpenAI Chat Completions

- No request needs a forwarded header, so there are no `.headers.json` files.
- A stream is what its request returns with `"stream": true` and `stream_options.include_usage` added. Its chunks keep the field order `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint`, `choices`, `usage`, `obfuscation`.
- The first chunk carries `delta.role` with `content: ""`, with `content: null` and the first tool call's header, or with `content: null, refusal: ""`. A chunk with `delta: {}` follows the last delta.
- Every chunk but the last has `"usage": null`, and the last, with `"choices": []`, carries `usage`. Every chunk carries `obfuscation`, random padding that the spec includes by default.
- `prompt_tokens` includes the cached and cache-write tokens, and `completion_tokens` the reasoning tokens. `total_tokens` is their sum.

### OpenAI Responses

- Doc examples use the spec's model, `gpt-6-astra`. Built fixtures use `gpt-5.5` with reasoning and `gpt-4.1` without, and their replies name the snapshots `gpt-5.5-2026-04-23` and `gpt-4.1-2025-04-14`.
- Ids follow `<prefix>_<16 hex>00<8 hex unix time><24 hex>`, with the prefixes `resp`, `msg`, `fc`, `rs` and `ws`, and the items of a reply share its 16-hex prefix. Call ids are `call_` and 24 letters and digits.
- Built `Response` objects carry every top-level field of the current SDK, `access_programs` included. Doc examples keep their smaller field sets, and the codec round-trips both.
- `input_tokens` includes `cached_tokens`, and `output_tokens` includes `reasoning_tokens`. `input_tokens_details.cache_write_tokens` is always present.
- In a stream, `event:` equals `data.type` and `sequence_number` counts up from 0. Deltas add up to their `.done` text and to the final reply, and no `[DONE]` follows.
- Every `*.delta` event carries `obfuscation`, except in a stream whose request sets `include_obfuscation: false` and in the documentation's own stream.

### OpenAI Embeddings

- `input` covers its four forms: a string, an array of strings, an array of token ids and an array of token-id arrays.
- Built requests set `dimensions: 8`, so vectors stay short. They are random unit vectors, each value the shortest decimal that survives float32.
- A `base64` reply holds little-endian float32 values, and `usage.total_tokens` equals `usage.prompt_tokens`.

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
cargo xtask record openai_chat --provider <id> --model <model> --dry-run \
  tools-single stream-include-usage
cargo xtask record openai_chat --provider <id> --model <model> \
  tools-single stream-include-usage
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
