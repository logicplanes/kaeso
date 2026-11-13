# Fixtures: `system_one`

Wire bodies for TypeSafe's System One API, `POST /v1/systemone`: typed questions about a state, answered with probabilities.

## Sources

- TypeSafe's OpenAPI document `https://api.typesafe.ai/openapi.json`, version 0.2.0, read on 2026-11-12: `SystemOneRequest`, `Question` (`NoulQuestion`, `ChoiceQuestion`, `ScoreQuestion`, `BoxQuestion`), `NoulCriteria`, `Asset`, `SystemOneResponse`, `Answer`, `Usage`, `HTTPBadRequestError`, `HTTPValidationError`, `ValidationError`, and the `multipart/form-data` request body.
- TypeSafe's documentation, read on 2026-11-12: https://docs.typesafe.ai/api/system-one, https://docs.typesafe.ai/images, https://docs.typesafe.ai/api/errors, https://docs.typesafe.ai/confidence.
- The request and response examples of the OpenAPI document (`noul`, `choice`, `score`, `box`) are verbatim (`doc:`). Everything else is built from the schema (`schema:`). Every file is spec-built: no key was used, so nothing here is a live recording.
- TypeSafe's refused-key replies and its own model listing are in `providers/typesafe/`; its two refused-key replies there were recorded without a key. Cloudflare's Clef replies and refusals, recorded live, are in `providers/cloudflare-ai-gateway/`.

The layout and the provenance marks (`doc:`, `sdk:`, `schema:`) are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures), and the rules every constructed fixture follows in [Fixture conventions](https://docs.kaeso.app/development/kpp/fixture-conventions). Each folder's `README.md` lists every file, what it covers and its provenance.

A response is read against the questions its request sent, so every file in `responses/` has a request of the same name in `requests/`. A request that needs a header has a `.headers.json` beside it.

## Doubts

- The error bodies other than the 422 example are built from the error table of https://docs.typesafe.ai/api/errors and the Images guide: the documentation names each status and its cause, not the exact `detail` text. The 400 for a request over the context length is written with the code `max_tokens_exceeded` in its `detail` string; whether TypeSafe sends it as a string or inside an object is not documented. Confirm them by recording.
- The OpenAPI document's box example holds a 16 × 16 PNG, which TypeSafe's own limits (280 to 1568 pixels a side) refuse; the round trip checks it without TypeSafe's image limits.
