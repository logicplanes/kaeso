# Fixtures: `openai_chat` (OpenAI Chat Completions)

Endpoint: `POST /v1/chat/completions`, including streaming.

## Sources (pinned)

| Source | Version |
|---|---|
| OpenAI OpenAPI spec (`openapi.yaml`, https://github.com/openai/openai-openapi) | commit **`e95c0fe615f45a19e8878af923a729948445a6bd`** (2026-10-09) |
| openai-python SDK (types in `src/openai/types/chat/`, tests in `tests/`) | commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` (2026-10-11) |
| Docs (fetched 2026-10-11) | API reference for chat create and streaming events, plus the error-codes, rate-limits, predicted-outputs, function-calling, prompt-caching, reasoning, file-inputs, images-vision, tools-web-search and moderation guides under https://developers.openai.com/api/docs/guides/ |

Every file is listed in its folder's `README.md` with exactly one provenance:

- `doc:` copied from an official doc example. For the reference page this is the `x-oaiMeta.examples` of `createChatCompletion` in the pinned spec.
- `sdk:` taken verbatim from an openai-python test or recorded snapshot. Names ending in `-sdk-2024` are live recordings from 2024. They use an older field set: no `annotations`, `service_tier` or `obfuscation`, a shorter `usage`, and no `usage: null` on non-final chunks. They are kept to test omitted fields.
- `schema:` built by hand from the named spec schemas, because no verbatim example exists. Values such as ids, token counts and texts are made up but internally consistent.
- `unknown-*`: these deliberately carry made-up fields or parts (`x_future_field`, `x_future_delta`, `x_future_choice_field`, content part `future_part`) to test Extras and Unknown handling. These are not real API fields.

## Layout and conventions

```
requests/<name>.json         exact request body (no forwarded headers are needed for Chat; no *.headers.json)
responses/<name>.json        exact non-streaming 200 body
streams/<name>.sse           exact SSE bytes: `data: <json>\n\n` frames, terminated by `data: [DONE]\n\n`
errors/<name>.json           exact error body {"error": {"message", "type", "param", "code"}}
errors/<name>.meta.json      {"status": <http status>, "headers": {...}} (lower-case header names)
```

- Pairing: when a request, a response and a stream share a base name, they belong together. The stream is the same completion as the response, with the same id, text split into deltas, tool-call arguments, `finish_reason` and final `usage`. It is what the request returns with `"stream": true, "stream_options": {"include_usage": true}` added. The exceptions are noted in the folder READMEs: `*-sdk-2024` and `content-filter-partial`.
- JSON files are pretty-printed with 2-space indentation. SSE `data:` lines are single-line JSON, as sent on the wire. Non-ASCII is written raw as UTF-8 (`°`, `—`).
- Constructed streams follow the current wire shape:
  - Chunk field order is `id, object, created, model, service_tier, system_fingerprint, choices, usage, obfuscation`.
  - The first chunk carries `delta.role` together with `content: ""` (text), or with `content: null` plus the first tool-call header (tool calls), or with `content: null, refusal: ""` (refusals).
  - A finish chunk with `delta: {}` follows.
  - With `include_usage`, every earlier chunk has `"usage": null` and a last chunk with `"choices": []` carries `usage`. This follows the `usage` description in `CreateChatCompletionStreamResponse`.
  - Every chunk carries `obfuscation`, which the spec says is "included by default". Its values are random padding.
- Usage semantics: `prompt_tokens` includes `prompt_tokens_details.cached_tokens` (and `cache_write_tokens`), and `completion_tokens` includes `completion_tokens_details.reasoning_tokens`. In every file, `total_tokens = prompt_tokens + completion_tokens`.
- Binary payloads are tiny but valid:
  - PNG: 1x1.
  - PDF: one page, with a correct xref table.
  - WAV: a 44-byte header plus 8 silent samples.

## Fixtures the codec must reject (`Unsupported`)

- `requests/input-audio-unsupported.json`: an `input_audio` content part (audio input is not part of KPP).
- `requests/n-2.json` and `responses/n-2.json`: `n > 1`, refused as `Unsupported`.

## Fixtures that only round-trip within this dialect

These have no KPP equivalent. They are kept within the dialect as Extras and `Unknown` items, round-trip, and are `Unsupported` across dialects.

- `requests/tools-custom-grammar.json` and `requests/audio-output.json`.
- `requests/legacy-functions.json`: deprecated function calling over several turns (top-level `functions` and `function_call`, an assistant `function_call`, a `function` role message). Legacy calls have no call ids, so they are no KPP tool calls: the assistant `function_call` is an extra of its message and the `function` message an `Unknown` block.

## Known gaps and judgement calls

- **Mid-stream error** (`streams/error-mid-stream.sse`): the spec says only that, after streaming starts, "a data frame may contain a JSON object with an `error` field". The inner shape is not documented. We used the `ErrorResponse` shape, with no `event:` line and no `[DONE]` after it. The SDK (`src/openai/_streaming.py`) detects such frames by `data.get("error")`.
- **Error bodies**: the docs give `type`/`code` only for some cases: `slow_down`, `server_is_overloaded`, the billing codes, and `invalid service_tier`. For the others we used the bodies the API is commonly seen to return, and flagged this in `errors/README.md`. That covers `invalid_api_key`, `model_not_found`, `context_length_exceeded`, `unsupported_country_region_territory`/`request_forbidden`, `insufficient_quota`, and the TPM `rate_limit_exceeded` with `type: tokens`. Note that, per the spec, 401/403 may also arrive as `text/plain` JSON-encoded text. No fixture covers that.
- **Rate limits**: the current docs moved "slow down" from 503 to **429** (`rate_limit_error`/`slow_down`). 503 is now `service_unavailable_error`/`server_is_overloaded`. Both are covered.
- **No 413 / 529**: Chat Completions documents neither.
- **No keep-alive or event-name frames**: Chat streams use bare `data:` frames. The only comment line (`: ...`) is in the verbatim doc stream `streams/stream-doc.sse`. That stream also has non-canonical spacing in its JSON, so round-trip tests must compare parsed events, not bytes.
- **Annotations in streams**: `ChatCompletionStreamResponseDelta` has no `annotations` field, so no web-search stream is included.
- **Forced tool choice**: `tools-single` uses `tool_choice: "required"` and reports `finish_reason: "tool_calls"`. Real responses to a *named* function choice may report `"stop"`. That is undocumented, so it is not encoded in a fixture.
