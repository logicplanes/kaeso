# `fixtures/gemini_decisions/errors`

| file | what it covers | provenance |
|---|---|---|
| `safety-400.json`, `.meta.json` | 400 with the safety code `safety` (KPP: `ContentFiltered`) | doc: https://aistudio.google.com/docs/decisions-api ("Request and response reference", verbatim) |
| `image-safety-400.json`, `.meta.json` | 400 with the safety code `image_safety` | doc: https://aistudio.google.com/docs/decisions-api (code list); message constructed in the wording of the `safety` example |
| `too-many-questions-400.json`, `.meta.json` | 400 `INVALID_ARGUMENT` for 1,001 questions | doc: https://aistudio.google.com/docs/decisions-api ("Limits": "HTTP 400 with an error message that names the rule"); message constructed |
| `invalid-option-key-400.json`, `.meta.json` | 400 `INVALID_ARGUMENT` with a `BadRequest` field violation for an option key | doc: https://aistudio.google.com/docs/decisions-api ("Limits", option key); message constructed |
| `invalid-api-key-400.json`, `.meta.json` | 400 `INVALID_ARGUMENT` with the reason `API_KEY_INVALID` (KPP: `Authentication`) | as `fixtures/gemini/errors/recorded-error-invalid-key.json` |
| `rate-limit-429.json`, `.meta.json` | 429 `RESOURCE_EXHAUSTED` with `RetryInfo` | as `fixtures/gemini/errors/recorded-image-output.json`, shortened |
| `overloaded-503.json`, `.meta.json` | 503 `UNAVAILABLE` | as `fixtures/gemini/errors/recorded-stream-thinking-high.json` |
