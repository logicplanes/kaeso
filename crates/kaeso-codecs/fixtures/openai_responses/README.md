# Fixtures: `openai_responses`

Wire bodies for `POST /v1/responses` (incl. streaming) and `POST /v1/responses/input_tokens`.

## Sources

- OpenAI OpenAPI spec `openapi.yaml` at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (2026-10-09), schemas `CreateResponse`, `Response`, `ResponseStreamEvent`, `TokenCountsBody`, `TokenCountsResource`, `ErrorResponse`.
- openai-python at commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` (`src/openai/types/responses/`) to check required vs optional fields, and `tests/lib/responses/test_responses.py` for the top-level field set of a current Response object.
- Official doc pages (create, streaming events, input token count, error codes). Files marked `doc:` copy the `x-oaiMeta` examples of the spec, which are the same examples the reference pages show.

Every folder has a `README.md` with one row per file and its provenance (`doc:` / `sdk:` / `schema:`). No fixture was recorded from the live API. Re-record with `cargo xtask record` and replace `schema:` files where possible.

## Layout

```
requests/                 POST /v1/responses bodies
requests/input_tokens/    POST /v1/responses/input_tokens bodies
responses/                non-streaming 200 bodies (same base name as their request)
responses/input_tokens/   {"object": "response.input_tokens", "input_tokens": N}
streams/                  SSE bytes; `<name>.sse` pairs with `requests/<name>.json`
errors/                   error bodies + `<name>.meta.json` ({status, headers})
```

## Conventions

- Doc examples use the model name the spec uses (`gpt-6-astra`). Constructed fixtures use `gpt-5.5` (reasoning) and `gpt-4.1` (non-reasoning). Responses report the snapshots `gpt-5.5-2026-04-23` / `gpt-4.1-2025-04-14`.
- Ids follow the current layout `<prefix>_<16 hex>00<8 hex unix time><24 hex>` (prefixes `resp`, `msg`, `fc`, `rs`, `ws`). Call ids are `call_` plus 24 alphanumerics. Items in one response share the 16-hex prefix with the response id. Timestamps are on 2026-10-11 (UTC).
- `encrypted_content` values are random Fernet-shaped tokens (`gAAAAA...`), not real ciphertext.
- Constructed Response objects carry this top-level field set: `id`, `object`, `access_programs` (required by the spec), `created_at`, `status`, `background`, `completed_at`, `error`, `incomplete_details`, `instructions`, `max_output_tokens`, `max_tool_calls`, `model`, `output`, `parallel_tool_calls`, `previous_response_id`, `prompt_cache_key`, `prompt_cache_retention`, `reasoning`, `safety_identifier`, `service_tier`, `store`, `temperature`, `text`, `tool_choice`, `tools`, `top_logprobs`, `top_p`, `truncation`, `usage`, `user`, `metadata`. Doc-copied Response objects keep the smaller field sets of the doc. The codec must round-trip both.
- `usage`: `total_tokens = input_tokens + output_tokens`. `input_tokens` includes `cached_tokens`. `output_tokens` includes `reasoning_tokens`. `input_tokens_details.cache_write_tokens` is required by the current spec and SDK.
- Streams: `event:` always equals `data.type`. Data lines are compact single-line JSON. `sequence_number` starts at 0 and increases by 1. Delta texts concatenate to the matching `.done` text and to the final `response.completed`/`incomplete` output. The terminal response's `output` equals the list of `output_item.done` items. The Responses API sends no `[DONE]` sentinel.
- `obfuscation`: the spec documents it only in `stream_options.include_obfuscation` ("random characters to an `obfuscation` field on streaming delta events ... included by default"), not in the event schemas. Constructed streams therefore put `obfuscation` on every `*.delta` event (`output_text`, `function_call_arguments`, `reasoning_summary_text`, `refusal`). `logprobs-no-obfuscation-stream.sse` (request has `include_obfuscation: false`) and the doc-copied `text-basic-stream.sse` have none.

## Special fixture groups

- `stateful-*` (`previous_response_id`, `conversation` string and object form, `background`) must be rejected by the codec with `Unsupported`.
- `explicit-nulls.json` sets every nullable field to `null`, including `previous_response_id`, `conversation` and `background`. A `null` there must not count as a stateful request, and the nulls must survive a round trip.
- `unknown-*` carry made-up content on purpose: `x_future_field` (top-level request and response), input item type `future_item`, output item type `future_output_item`, stream event `response.future_event`, and the SSE comment line `: keepalive`.
- Built-in tools (`web_search` tool, `web_search_call` items and `response.web_search_call.*` events) are Unknown/Extras carriers: same-dialect only.

## Open doubts (see also the lead's open-questions list)

1. Stream `error` event: the spec (`ResponseErrorEvent`) and the SDK use a flat `{type, code, message, param, sequence_number}`. WebSocket-mode frames in the SDK tests use a nested `error` object. The fixture follows the spec. The live SSE shape should be confirmed by recording.
2. `function_call_arguments.done`: the spec has no `name` field, so the fixtures have none. Recent live streams may include one.
3. `output_item.added` for `web_search_call` carries `action: {"type": "search"}` because the SDK type requires `action`. The live API may send it without `action` or with `query`.
4. `output_text.logprobs` in messages use `LogProb` (with `bytes`). Delta and `.done` events use `ResponseLogProb` (without `bytes`). This follows the spec.
5. The web_search tool echo in Responses (`filters`, `search_context_size`, `user_location` with `country: "US"`) is modelled on the doc's `web_search_preview` example.
6. `responses/failed.json` shows the Response object with `status: "failed"`. A synchronous create normally returns an HTTP error instead. The live API returns this object shape for background or retrieved responses.
7. Many error `message` strings are written the way the live API words them, but the spec does not contain them. Only `slow-down-429` and `overloaded-503` are verbatim spec examples.
