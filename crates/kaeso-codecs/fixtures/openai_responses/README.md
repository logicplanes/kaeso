# Fixtures: `openai_responses`

Wire bodies for `POST /v1/responses` (incl. streaming) and `POST /v1/responses/input_tokens`.

## Sources

- OpenAI OpenAPI spec `openapi.yaml` at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (2026-10-09), schemas `CreateResponse`, `Response`, `ResponseStreamEvent`, `TokenCountsBody`, `TokenCountsResource`, `ErrorResponse`.
- openai-python at commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` (`src/openai/types/responses/`) to check required vs optional fields, and `tests/lib/responses/test_responses.py` for the top-level field set of a current Response object.
- Official doc pages (create, streaming events, input token count, error codes). Files marked `doc:` copy the `x-oaiMeta` examples of the spec, which are the same examples the reference pages show.

No fixture was recorded from the live API. Re-record with `cargo xtask record` and replace `schema:` files where possible.

The layout, the provenance marks (`doc:`, `sdk:`, `schema:`) and the conventions every constructed fixture follows are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each folder's `README.md` lists every file, what it covers and its provenance.

## Special fixture groups

- `stateful-*` (`previous_response_id`, `conversation` string and object form, `background`) must be rejected by the codec with `Unsupported`.
- `explicit-nulls.json` sets every nullable field to `null`, including `previous_response_id`, `conversation` and `background`. A `null` there must not count as a stateful request, and the nulls must survive a round trip.
- `unknown-*` carry made-up content on purpose: `x_future_field` (top-level request and response), input item type `future_item`, output item type `future_output_item`, stream event `response.future_event`, and the SSE comment line `: keepalive`.
- Built-in tools (`web_search` tool, `web_search_call` items and `response.web_search_call.*` events) are Unknown/Extras carriers: same-dialect only.

## Open doubts

1. Stream `error` event: the spec (`ResponseErrorEvent`) and the SDK use a flat `{type, code, message, param, sequence_number}`. WebSocket-mode frames in the SDK tests use a nested `error` object. The fixture follows the spec. The live SSE shape should be confirmed by recording.
2. `function_call_arguments.done`: the spec has no `name` field, so the fixtures have none. Recent live streams may include one.
3. `output_item.added` for `web_search_call` carries `action: {"type": "search"}` because the SDK type requires `action`. The live API may send it without `action` or with `query`.
4. `output_text.logprobs` in messages use `LogProb` (with `bytes`). Delta and `.done` events use `ResponseLogProb` (without `bytes`). This follows the spec.
5. The web_search tool echo in Responses (`filters`, `search_context_size`, `user_location` with `country: "US"`) is modelled on the doc's `web_search_preview` example.
6. `responses/failed.json` shows the Response object with `status: "failed"`. A synchronous create normally returns an HTTP error instead. The live API returns this object shape for background or retrieved responses.
7. Many error `message` strings are written the way the live API words them, but the spec does not contain them. Only `slow-down-429` and `overloaded-503` are verbatim spec examples.
