# Fixtures: `openai_embeddings`

Wire bodies for `POST /v1/embeddings`.

## Sources

- OpenAI OpenAPI spec `openapi.yaml` at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (2026-10-09): `CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`, `components.responses.InferenceRateLimited` / `InferenceServiceUnavailable`.
- openai-python at commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` for type checks.
- `string-input` request and response are the spec/doc example verbatim. Everything else is constructed (`schema:`). Each folder README gives per-file provenance.

## Layout

`requests/`, `responses/` (same base name as the request), `errors/` (+ `<name>.meta.json` with status and headers).

## Conventions

- `input` covers all four forms: string, array of strings, array of token ids, array of token-id arrays.
- Every constructed request sets `dimensions: 8`, so the vectors stay short and the responses stay consistent with their requests.
- Float vectors are random unit vectors. Each value is printed as the shortest decimal that round-trips through float32, like the doc example (`0.26726124`).
- `encoding_format: "base64"` responses hold standard base64 of little-endian float32 values (8 values = 32 bytes).
- `usage.total_tokens == usage.prompt_tokens`.
- `unknown-top-level-field.json` carries the made-up field `x_future_field` on purpose.

## Doubts

- Error messages for embeddings (`invalid-input-400`, `context-length-exceeded-400`, `dimensions-unsupported-400`) are worded the way the live API words them. They are not in the spec, and `code`/`param` are `null` as in those live bodies. Confirm them by recording.
