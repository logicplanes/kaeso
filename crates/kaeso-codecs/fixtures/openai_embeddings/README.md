# Fixtures: `openai_embeddings`

Wire bodies for `POST /v1/embeddings`.

## Sources

- OpenAI OpenAPI spec `openapi.yaml` at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (2026-10-09): `CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`, `components.responses.InferenceRateLimited` / `InferenceServiceUnavailable`.
- openai-python at commit `16d3d3b3c33b1b4f01114a16e4c3eba2293799ba` for type checks.
- `string-input` request and response are the spec/doc example verbatim. Everything else is constructed (`schema:`). Each folder README gives per-file provenance.

The layout, the provenance marks (`doc:`, `sdk:`, `schema:`) and the conventions every constructed fixture follows are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each folder's `README.md` lists every file, what it covers and its provenance.

## Doubts

- Error messages for embeddings (`invalid-input-400`, `context-length-exceeded-400`, `dimensions-unsupported-400`) are worded the way the live API words them. They are not in the spec, and `code`/`param` are `null` as in those live bodies. Confirm them by recording.
