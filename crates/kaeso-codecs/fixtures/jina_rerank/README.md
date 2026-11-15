# Fixtures: `jina_rerank`

Wire bodies for Jina AI's rerank API, `POST https://api.jina.ai/v1/rerank` with a `jina_` key: a query and documents as strings or `{"text"}` objects, `top_n`, `return_documents` (true when left out) and, for `jina-reranker-v3` and `jina-reranker-v3.5`, `max_doc_length`; answered with `results` sorted by `relevance_score` and `usage.total_tokens`.

## Sources (read 2026-11-14)

- Jina AI's OpenAPI document, `info.version` `2026.10.13.1624` (https://api.jina.ai/openapi.json): `RerankerV3Request`, `TextRerankerRequest`, `RerankingResponse`, `RerankingResult`, `BaseUsage`, `ErrorResponse`, `FieldError`.
- Jina AI's API reference, Errors (https://docs.jina.ai/api.md): the codes and their statuses.
- No key was available. Jina checks the key before the body, so only the refused-key errors could be recorded; every reply is built from the schema.

| file | what it covers | provenance |
|---|---|---|
| `requests/v3-doc.json` | `jina-reranker-v3` with `top_n`, `return_documents: false` and `return_embeddings: false` | the `RerankerV3Request` example, verbatim |
| `requests/v2-doc.json` | `jina-reranker-v2-base-multilingual` | the `TextRerankerRequest` example, verbatim |
| `requests/text-objects.json` | a `{"text"}` document beside a string, `return_documents: true` and `max_doc_length` | schema |
| `responses/v3-scores.json` | scores without documents, `usage.total_tokens` | schema: `RerankingResponse` for the `v3-doc` request |
| `responses/text-objects.json` | documents echoed in the form they were sent, a string and a `{"text"}` | schema: `RerankingResult.document` is a string, `TextDoc` or `ImageDoc` |
| `errors/recorded-no-key-401.json` | `AUTH_MISSING_API_KEY` | live, without a key; `request_id` zeroed |
| `errors/recorded-invalid-key-401.json` | `AUTH_INVALID_API_KEY` | live, with a made-up key; `request_id` zeroed |
| `errors/insufficient-balance-403.json` | `AUTHZ_INSUFFICIENT_BALANCE`, read as a billing error | schema and the code table; the `detail` text is built |
| `errors/validation-422.json` | a 422 with `errors[]` | schema (`FieldError`); the texts are built |
| `errors/rate-limit-429.json` | `RATE_REQUEST_LIMIT_EXCEEDED` with `retry-after` | schema and the code table; the text and the header are built |

## Doubts

- Scores' range is not documented, and the replies' scores are built.
- Whether Jina AI refuses or ignores fields it doesn't know could not be checked without a key.
