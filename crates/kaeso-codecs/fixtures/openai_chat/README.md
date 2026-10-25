# Fixtures: `openai_chat` (OpenAI Chat Completions)

Endpoint: `POST /v1/chat/completions`, including streaming.

## Sources (pinned)

| Source | Version |
|---|---|
| OpenAI OpenAPI spec (`openapi.yaml`, https://github.com/openai/openai-openapi) | commit **`e95c0fe615f45a19e8878af923a729948445a6bd`** (2026-10-09) |
| openai-python SDK (types in `src/openai/types/chat/`, tests in `tests/`) | commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` (2026-10-11) |
| Docs (fetched 2026-10-11) | API reference for chat create and streaming events, plus the error-codes, rate-limits, predicted-outputs, function-calling, prompt-caching, reasoning, file-inputs, images-vision, tools-web-search and moderation guides under https://developers.openai.com/api/docs/guides/ |

The layout, the provenance marks (`doc:`, `sdk:`, `schema:`) and the conventions every constructed fixture follows are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each folder's `README.md` lists every file, what it covers and its provenance. Names ending in `-sdk-2024` are live recordings from 2024 with an older field set (no `annotations`, `service_tier` or `obfuscation`, a shorter `usage`, no `usage: null` on non-final chunks), kept to test omitted fields. They and `content-filter-partial` have no stream that matches their response exactly; the folder READMEs say why.

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
