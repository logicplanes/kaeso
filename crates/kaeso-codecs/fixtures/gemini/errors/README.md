# `fixtures/gemini/errors`

| file | what it covers | provenance |
|---|---|---|
| `recorded-error-invalid-argument.json` | 400 `INVALID_ARGUMENT` with a `BadRequest` field violation | RECORDED.md |
| `recorded-error-invalid-key.json` | 400 `INVALID_ARGUMENT` with the reason `API_KEY_INVALID` | RECORDED.md |
| `recorded-error-unknown-model.json` | 404 `NOT_FOUND` | RECORDED.md |
| `recorded-image-output.json` | 429 `RESOURCE_EXHAUSTED` with `QuotaFailure` and `RetryInfo`: no quota for the image model | RECORDED.md |
| `recorded-stream-thinking-high.json` | 503 `UNAVAILABLE`, the model under high demand | RECORDED.md |
| `image-generation-region.json` | 400 `FAILED_PRECONDITION`, image generation not available in the key's country | recorded from `gemini-3.5-flash` with `responseModalities: [TEXT, IMAGE]` on 2026-11-08 03:00 UTC; written again by hand after a later recording replaced the file |
