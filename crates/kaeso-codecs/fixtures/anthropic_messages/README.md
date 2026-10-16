# Fixtures: `anthropic_messages`

Wire fixtures for the Anthropic Messages API: `POST /v1/messages` (non-streaming and SSE streaming) and `POST /v1/messages/count_tokens`.

| folder | contents |
|---|---|
| `requests/` | request bodies, a `.headers.json` next to those that need an allowlisted header; count_tokens bodies in `requests/count_tokens/` |
| `responses/` | non-streaming 200 bodies; count_tokens bodies in `responses/count_tokens/` |
| `streams/` | SSE streams |
| `errors/` | error bodies, each with a `.meta.json` (status, headers) |
| `models/` | model listing and model retrieval bodies |

Each folder's `README.md` lists every file, what it covers and its provenance (`doc:` / `sdk:` / `schema:`).

## Sources used

- Anthropic OpenAPI spec: `anthropic-465bff21a179090915396565d1ae8f705cf8596e2ec920eb121072f25b8a7d68.yml` (the last spec URL recorded in the SDK's `.stats.yml`, SDK commit `41aa767464881bb51937f42fe9c426ae3081fd9f`, 2026-09-02). Schema names in `schema:` rows refer to this file.
- `anthropics/anthropic-sdk-python` at `b4b7916deaf4e5570cd627b1ae7ee4394a4de39f` (release 1.13.0, 2026-10-09). Its types are newer than the spec; it adds e.g. `thinking.type: "between_tools"` and `Message.diagnostics`, and drops `temperature` / `top_k` / `top_p`.
- Docs at https://platform.claude.com/docs/en/ (plain-text `.md` versions), fetched 2026-10-11: messages/create, count_tokens, errors, streaming, thinking, extended-thinking, adaptive-thinking, prompt-caching, structured-outputs, citations, pdf-support, vision, files, tool-use (overview / define / handle), web-search-tool, handling-stop-reasons, refusals-and-fallback, rate-limits, beta-headers, versioning, service-tiers.

## Conventions

- `anthropic-version: 2023-06-01` is assumed for every request and is not stored per fixture. A `.headers.json` file exists only where a request needs an allowlisted header (`anthropic-beta`).
- Ids follow the live formats: `msg_01…`, `toolu_01…`, `srvtoolu_01…`, `file_011C…`, `req_011C…`. Signatures, `redacted_thinking.data`, `encrypted_content` and `encrypted_index` are random but valid base64. They are not real tokens, so the API would reject them on replay.
- Binary payloads are tiny but valid: a 1x1 PNG, and a 602-byte one-page PDF whose text is "Water is essential for life." (`pdftotext` extracts it).
- Response field set (`schema:` rows) follows the wire as recorded in the SDK's SSE fixtures (`tests/fixtures/fable-fallback/*.sse`) and the doc examples, and stays inside the spec:
  - `Message`: `model, id, type, role, content, stop_reason, stop_sequence, stop_details, usage`. `container` and `diagnostics` are left out because they are only non-null when used.
  - `usage`: `input_tokens, cache_creation_input_tokens, cache_read_input_tokens, cache_creation{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}, output_tokens, service_tier, inference_geo`. `output_tokens_details` appears only when thinking happened, and `server_tool_use` only when a server tool ran.
  - `message_start` carries the same usage with a small `output_tokens`. The final `message_delta.usage` carries the cumulative `input_tokens`, `cache_*_input_tokens` and `output_tokens`, plus `output_tokens_details` / `server_tool_use` where applicable.
- Token counts are internally consistent: `cache_creation_input_tokens` equals the sum of its breakdown, `thinking_tokens` ≤ `output_tokens`, and stream usage matches the paired response.
- Every stream that shares a base name with a file in `responses/` accumulates to exactly that response. This was checked mechanically.
- `unknown-*` files contain deliberately invented fields, blocks and events (`x_future_field`, `future_block`, `future_delta`, `x_future_event`). Nothing else contains invented fields.

## Validation performed

- All non-`unknown-*` requests, count_tokens requests, responses, error bodies and stream events were validated with `jsonschema` against the spec above. Properties that the spec marks required but gives a default (Stainless convention) were treated as omittable. The expected exceptions:
  - `requests/thinking-between-tools.json`: `between_tools` is newer than the spec; it is in SDK 1.13.0.
  - `errors/request-too-large.json`: `request_too_large` is documented on the errors page but missing from the spec's `ErrorResponse` union.
- Every constructed stream that has a paired response was parsed as SSE and accumulated, and the result equals that response.

## Known gaps and doubts (see also the folder READMEs)

- Not expressible in this dialect: non-leading system messages (there is no `system` role), JSON mode without a schema, `seed`, penalties, logprobs, `n`.
- Unverified wire details, chosen conservatively:
  - Whether `tool_use` / `server_tool_use` blocks carry `caller: {"type":"direct"}`. The spec marks it required with that default; docs omit it. Fixtures omit it on tool-use blocks and include it on `web_search_tool_result`, as the SDK recording does.
  - Whether citation-enabled `content_block_start` text blocks carry `"citations": []`. Fixtures omit it.
  - Whether non-thinking responses include `output_tokens_details: {"thinking_tokens": 0}`. Fixtures omit it.
  - Citation objects include `file_id: null` per the spec, although the citations doc examples omit it.
- `usage.iterations` (in `responses/cache-prewarm.json`, from the docs) and `error.details` (in `errors/rate-limit-spend-cap.json`, from the docs) are real documented fields that are missing from the OpenAPI schemas. Codecs must keep them as Extras.
- Error message texts for 401/402/403/413/429/500/504 are constructed; the doc pages show no examples for those statuses.
- Server-side fallback (`fallbacks`, `fallback` blocks, beta) and the other beta-only features are not covered.
