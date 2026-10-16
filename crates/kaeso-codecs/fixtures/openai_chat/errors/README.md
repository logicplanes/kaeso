# openai_chat / errors

Error bodies and status/headers

| file | what it covers | provenance |
|---|---|---|
| `context-length-exceeded.json / context-length-exceeded.meta.json` | HTTP 400: 400 `context_length_exceeded` (maps to KPP ContextTooLong) | schema: ErrorResponse (code listed in spec enums, e.g. SessionTurnError; message wording as commonly returned) |
| `credit-balance-exhausted.json / credit-balance-exhausted.meta.json` | HTTP 429: 429 billing error with `code: credit_balance_exhausted`, `type: insufficient_quota` | doc: https://developers.openai.com/api/docs/guides/error-codes (code, type and cause text; exact message string not shown in docs) |
| `insufficient-quota.json / insufficient-quota.meta.json` | HTTP 429: 429 quota (must NOT be retried; map to RateLimited without retry-after or a quota kind) | schema: ErrorResponse (`error.type` `insufficient_quota` per https://developers.openai.com/api/docs/guides/error-codes; legacy message/code as commonly returned) |
| `invalid-api-key.json / invalid-api-key.meta.json` | HTTP 401: 401 incorrect API key | schema: ErrorResponse (doc heading "401 - Incorrect API key provided" at https://developers.openai.com/api/docs/guides/error-codes; body wording/code as commonly returned) |
| `invalid-service-tier.json / invalid-service-tier.meta.json` | HTTP 400: 400 with `param: service_tier`, `code: null` | doc: https://developers.openai.com/api/docs/guides/error-codes (message, type, param verbatim; `code` not given there, set to null) |
| `invalid-type-param.json / invalid-type-param.meta.json` | HTTP 400: 400 `invalid_request_error` with `param` pointing into the body and a `code` | schema: ErrorResponse, Error (message/code as commonly returned by the API; not quoted in the docs) |
| `model-not-found.json / model-not-found.meta.json` | HTTP 404: 404 unknown model | schema: ErrorResponse (404 description of createChatCompletion; wording/code as commonly returned) |
| `rate-limit-slow-down.json / rate-limit-slow-down.meta.json` | HTTP 429: 429 `rate_limit_error` / `slow_down` with `retry-after` | doc: https://developers.openai.com/api/docs/guides/error-codes (body = `components.responses.InferenceRateLimited` example `slowDown` in openai-openapi@e95c0fe615f45a19e8878af923a729948445a6bd) |
| `rate-limit-tokens.json / rate-limit-tokens.meta.json` | HTTP 429: 429 TPM limit with `retry-after` and `x-ratelimit-*` headers; `type: tokens` (not one of the documented generic types) | schema: InferenceRateLimited response (headers per https://developers.openai.com/api/docs/guides/rate-limits; message/type/code as commonly returned) |
| `server-error.json / server-error.meta.json` | HTTP 500: 500 `server_error` | schema: ErrorResponse (doc heading "500 - The server had an error while processing your request" at https://developers.openai.com/api/docs/guides/error-codes) |
| `server-overloaded.json / server-overloaded.meta.json` | HTTP 503: 503 `service_unavailable_error` / `server_is_overloaded` with `retry-after` (KPP Overloaded) | doc: https://developers.openai.com/api/docs/guides/error-codes (body = `components.responses.InferenceServiceUnavailable` example in openai-openapi@e95c0fe615f45a19e8878af923a729948445a6bd) |
| `unknown-error-field.json / unknown-error-field.meta.json` | HTTP 400: MADE-UP `x_future_field` inside `error` (unknown-* fixture; the real schema also allows an optional `misalignment` object) | schema: Error + invented field |
| `unsupported-country.json / unsupported-country.meta.json` | HTTP 403: 403 unsupported country/region | schema: ErrorResponse (doc heading "403 - Country, region, or territory not supported" at https://developers.openai.com/api/docs/guides/error-codes; type/code as commonly returned) |
| `unsupported-parameter.json / unsupported-parameter.meta.json` | HTTP 400: 400 for `max_tokens` on a reasoning model | schema: ErrorResponse (message/code as commonly returned by the API; not quoted in the docs) |
