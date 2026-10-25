---
title: Fixture conventions
nav: Fixture conventions
description: The rules every constructed fixture follows, so that the codec tests can compare it exactly.
order: 10
---

Fixtures that aren't live recordings follow these rules, so that the tests can compare them exactly:

- **Pairs.** A request, a reply and a stream with the same base name belong together. The stream is the same reply as the response and accumulates to exactly that response; the dialect folder's `README.md` names any exception.
- **Numbers that agree.** Totals are the sum of their parts, and a stream reports the same usage as its reply.
- **Made-up values in the live shape.** Ids follow the provider's live format. Signatures and encrypted reasoning are random but well formed; a provider would refuse them on replay.
- **Tiny but valid files.** An image is a 1×1 PNG, a PDF one page with a correct cross-reference table, and audio a WAV header with eight silent samples.
- **Formatting.** JSON files are indented by two spaces and write non-ASCII as UTF-8. SSE `data:` lines hold single-line JSON, as on the wire.
- **Invented content only in `unknown-*`.** No other fixture carries a field, block or event the provider doesn't document.

## Anthropic Messages

- Every request assumes `anthropic-version: 2023-06-01`, which isn't stored. A `.headers.json` exists only where a request needs `anthropic-beta`.
- Ids look like `msg_01…`, `toolu_01…`, `srvtoolu_01…`, `file_011C…` and `req_011C…`.
- A `Message` carries `model`, `id`, `type`, `role`, `content`, `stop_reason`, `stop_sequence`, `stop_details` and `usage`, as the SDK's recorded streams and the documentation show. `container` and `diagnostics` are left out, because they are only set when used.
- `usage` carries `input_tokens`, the two cache counts, the `cache_creation` breakdown, `output_tokens`, `service_tier` and `inference_geo`. `output_tokens_details` appears only after thinking, and `server_tool_use` only after a server tool ran.
- `cache_creation_input_tokens` equals the sum of its breakdown, and `thinking_tokens` is at most `output_tokens`.
- `message_start` carries the reply's usage with a small `output_tokens`, and the last `message_delta` the cumulative counts.

## OpenAI Chat Completions

- No request needs a forwarded header, so there are no `.headers.json` files.
- A stream is what its request returns with `"stream": true` and `stream_options.include_usage` added. Its chunks keep the field order `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint`, `choices`, `usage`, `obfuscation`.
- The first chunk carries `delta.role` with `content: ""`, with `content: null` and the first tool call's header, or with `content: null, refusal: ""`. A chunk with `delta: {}` follows the last delta.
- Every chunk but the last has `"usage": null`, and the last, with `"choices": []`, carries `usage`. Every chunk carries `obfuscation`, random padding that the spec includes by default.
- `prompt_tokens` includes the cached and cache-write tokens, and `completion_tokens` the reasoning tokens. `total_tokens` is their sum.

## OpenAI Responses

- Doc examples use the spec's model, `gpt-6-astra`. Built fixtures use `gpt-5.5` with reasoning and `gpt-4.1` without, and their replies name the snapshots `gpt-5.5-2026-04-23` and `gpt-4.1-2025-04-14`.
- Ids follow `<prefix>_<16 hex>00<8 hex unix time><24 hex>`, with the prefixes `resp`, `msg`, `fc`, `rs` and `ws`, and the items of a reply share its 16-hex prefix. Call ids are `call_` and 24 letters and digits.
- Built `Response` objects carry every top-level field of the current SDK, `access_programs` included. Doc examples keep their smaller field sets, and the codec round-trips both.
- `input_tokens` includes `cached_tokens`, and `output_tokens` includes `reasoning_tokens`. `input_tokens_details.cache_write_tokens` is always present.
- In a stream, `event:` equals `data.type` and `sequence_number` counts up from 0. Deltas add up to their `.done` text and to the final reply, and no `[DONE]` follows.
- Every `*.delta` event carries `obfuscation`, except in a stream whose request sets `include_obfuscation: false` and in the documentation's own stream.

## OpenAI Embeddings

- `input` covers its four forms: a string, an array of strings, an array of token ids and an array of token-id arrays.
- Built requests set `dimensions: 8`, so vectors stay short. They are random unit vectors, each value the shortest decimal that survives float32.
- A `base64` reply holds little-endian float32 values, and `usage.total_tokens` equals `usage.prompt_tokens`.
