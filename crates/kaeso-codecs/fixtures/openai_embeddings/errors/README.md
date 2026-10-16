# `fixtures/openai_embeddings/errors`

| file | what it covers | provenance |
|---|---|---|
| `context-length-exceeded-400.json` | 400 input over 8192 tokens (KPP ContextTooLong) | schema: ErrorResponse / Error; 8192 limit from CreateEmbeddingRequest.input; message as returned by the API |
| `context-length-exceeded-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; 8192 limit from CreateEmbeddingRequest.input; message as returned by the API |
| `dimensions-unsupported-400.json` | 400 `dimensions` on text-embedding-ada-002 | schema: ErrorResponse / Error; CreateEmbeddingRequest.dimensions (text-embedding-3 and later only) |
| `dimensions-unsupported-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; CreateEmbeddingRequest.dimensions (text-embedding-3 and later only) |
| `invalid-api-key-401.json` | 401 authentication | doc: https://developers.openai.com/api/docs/guides/error-codes ("Incorrect API key provided") |
| `invalid-api-key-401.meta.json` | HTTP status 401 | doc: https://developers.openai.com/api/docs/guides/error-codes ("Incorrect API key provided") |
| `invalid-input-400.json` | 400 invalid input (e.g. empty string), `param`/`code` null | schema: ErrorResponse / Error; message as returned by the API |
| `invalid-input-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; message as returned by the API |
| `overloaded-503.json` | 503 overloaded with `retry-after` | doc: https://developers.openai.com/api/reference/resources/embeddings/methods/create (openapi.yaml components.responses.InferenceServiceUnavailable example, verbatim) |
| `overloaded-503.meta.json` | HTTP status 503, headers ['retry-after'] | doc: https://developers.openai.com/api/reference/resources/embeddings/methods/create (openapi.yaml components.responses.InferenceServiceUnavailable example, verbatim) |
| `rate-limit-429.json` | 429 with `retry-after` | schema: ErrorResponse / Error; InferenceRateLimited (Retry-After) |
| `rate-limit-429.meta.json` | HTTP status 429, headers ['retry-after'] | schema: ErrorResponse / Error; InferenceRateLimited (Retry-After) |
