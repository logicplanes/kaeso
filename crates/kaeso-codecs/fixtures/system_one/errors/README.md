# `fixtures/system_one/errors`

| file | what it covers | provenance |
|---|---|---|
| `validation-422.json`, `.meta.json` | 422 `detail` list, a missing field | doc: OpenAPI `HTTPValidationError` example (openapi.json 0.2.0, read 2026-11-12) |
| `validation-criteria-422.json`, `.meta.json` | 422 with `input` and `ctx` | schema: `ValidationError`, its `loc`, `input` and `ctx` examples |
| `images-not-enabled-400.json`, `.meta.json` | 400 `detail` string: an image without the images header | doc: https://docs.typesafe.ai/images ("Errors"); wording built |
| `asset-unreferenced-400.json`, `.meta.json` | 400 `detail` object with `message` | schema: `HTTPBadRequestError` (string or object); doc: https://docs.typesafe.ai/images ("Every asset must be referenced") |
| `max-tokens-exceeded-400.json`, `.meta.json` | 400 with `max_tokens_exceeded` (KPP ContextTooLong) | doc: https://docs.typesafe.ai/api/errors; the shape is a guess (see the folder's Doubts) |
| `unauthorized-401.json`, `.meta.json` | 401 missing or invalid key | doc: https://docs.typesafe.ai/api/errors; wording built |
| `too-large-413.json`, `.meta.json` | 413 body over 32 MiB | doc: https://docs.typesafe.ai/api/errors; wording built |
| `unsupported-encoding-415.json`, `.meta.json` | 415 unsupported `Content-Encoding` | doc: https://docs.typesafe.ai/api/errors; wording built |
| `rate-limit-429.json`, `.meta.json` | 429 with `retry-after` and `retry-after-ms` | doc: https://docs.typesafe.ai/api/errors and https://docs.typesafe.ai/sdk/python/api/exceptions (`retry_after_ms`); wording built |
| `overloaded-529.json`, `.meta.json` | 529 overloaded | doc: https://docs.typesafe.ai/api/errors; wording built |
