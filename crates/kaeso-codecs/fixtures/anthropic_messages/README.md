# Fixtures: `anthropic_messages`

Wire fixtures for the Anthropic Messages API: `POST /v1/messages` (non-streaming and SSE streaming) and `POST /v1/messages/count_tokens`.

The layout, the provenance marks (`doc:`, `sdk:`, `schema:`) and the conventions every constructed fixture follows are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each folder's `README.md` lists every file, what it covers and its provenance.

## Sources used

- Anthropic OpenAPI spec: `anthropic-465bff21a179090915396565d1ae8f705cf8596e2ec920eb121072f25b8a7d68.yml` (the last spec URL recorded in the SDK's `.stats.yml`, SDK commit `41aa767464881bb51937f42fe9c426ae3081fd9f`, 2026-09-02). Schema names in `schema:` rows refer to this file.
- `anthropics/anthropic-sdk-python` at `b4b7916deaf4e5570cd627b1ae7ee4394a4de39f` (release 1.13.0, 2026-10-09). Its types are newer than the spec; it adds e.g. `thinking.type: "between_tools"` and `Message.diagnostics`, and drops `temperature` / `top_k` / `top_p`.
- Docs at https://platform.claude.com/docs/en/ (plain-text `.md` versions), fetched 2026-10-11: messages/create, count_tokens, errors, streaming, thinking, extended-thinking, adaptive-thinking, prompt-caching, structured-outputs, citations, pdf-support, vision, files, tool-use (overview / define / handle), web-search-tool, handling-stop-reasons, refusals-and-fallback, rate-limits, beta-headers, versioning, service-tiers.

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
