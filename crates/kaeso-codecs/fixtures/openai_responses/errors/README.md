# `fixtures/openai_responses/errors`

| file | what it covers | provenance |
|---|---|---|
| `context-length-exceeded-400.json` | context window exceeded (maps to KPP ContextTooLong) | schema: ErrorResponse / Error; code/message as returned by the API (not in the spec) |
| `context-length-exceeded-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; code/message as returned by the API (not in the spec) |
| `insufficient-quota-429.json` | 429 without `retry-after` (user action required) | doc: https://developers.openai.com/api/docs/guides/error-codes (type insufficient_quota; message as returned by the API) |
| `insufficient-quota-429.meta.json` | HTTP status 429 | doc: https://developers.openai.com/api/docs/guides/error-codes (type insufficient_quota; message as returned by the API) |
| `integer-below-min-400.json` | 400 numeric validation error | schema: ErrorResponse / Error; minimum 16 from CreateResponse.max_output_tokens |
| `integer-below-min-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; minimum 16 from CreateResponse.max_output_tokens |
| `invalid-api-key-401.json` | 401 authentication | doc: https://developers.openai.com/api/docs/guides/error-codes ("Incorrect API key provided"; body fields per ErrorResponse) |
| `invalid-api-key-401.meta.json` | HTTP status 401 | doc: https://developers.openai.com/api/docs/guides/error-codes ("Incorrect API key provided"; body fields per ErrorResponse) |
| `invalid-service-tier-400.json` | 400 with `code: null` | doc: https://developers.openai.com/api/docs/guides/error-codes (message, type and param from the guide; code not given there, so null) |
| `invalid-service-tier-400.meta.json` | HTTP status 400 | doc: https://developers.openai.com/api/docs/guides/error-codes (message, type and param from the guide; code not given there, so null) |
| `model-not-found-404.json` | 404 unknown model | schema: ErrorResponse / Error; 404 documented on createResponse |
| `model-not-found-404.meta.json` | HTTP status 404 | schema: ErrorResponse / Error; 404 documented on createResponse |
| `overloaded-503.json` | 503 overloaded with `retry-after` (maps to KPP Overloaded) | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (openapi.yaml components.responses.InferenceServiceUnavailable example serverOverloaded, verbatim) |
| `overloaded-503.meta.json` | HTTP status 503, headers ['retry-after'] | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (openapi.yaml components.responses.InferenceServiceUnavailable example serverOverloaded, verbatim) |
| `previous-response-not-found-400.json` | stateful error the gateway may need to pass through | schema: ErrorResponse / Error; code from the error-codes guide (WebSocket section) |
| `previous-response-not-found-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; code from the error-codes guide (WebSocket section) |
| `rate-limit-tokens-429.json` | 429 TPM rate limit with `retry-after` | schema: ErrorResponse / Error; InferenceRateLimited response (Retry-After header) |
| `rate-limit-tokens-429.meta.json` | HTTP status 429, headers ['retry-after'] | schema: ErrorResponse / Error; InferenceRateLimited response (Retry-After header) |
| `server-error-500.json` | 500 internal error | doc: https://developers.openai.com/api/docs/guides/error-codes ("The server had an error while processing your request") |
| `server-error-500.meta.json` | HTTP status 500 | doc: https://developers.openai.com/api/docs/guides/error-codes ("The server had an error while processing your request") |
| `slow-down-429.json` | 429 slow_down with `retry-after` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (openapi.yaml components.responses.InferenceRateLimited example slowDown, verbatim) |
| `slow-down-429.meta.json` | HTTP status 429, headers ['retry-after'] | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (openapi.yaml components.responses.InferenceRateLimited example slowDown, verbatim) |
| `unsupported-parameter-400.json` | 400 invalid_request_error with `param` and `code` | schema: ErrorResponse / Error; message text as commonly returned by the API (not in the spec) |
| `unsupported-parameter-400.meta.json` | HTTP status 400 | schema: ErrorResponse / Error; message text as commonly returned by the API (not in the spec) |
| `unsupported-region-403.json` | 403 permission denied | doc: https://developers.openai.com/api/docs/guides/error-codes (message from the guide; type/code as returned by the API) |
| `unsupported-region-403.meta.json` | HTTP status 403 | doc: https://developers.openai.com/api/docs/guides/error-codes (message from the guide; type/code as returned by the API) |
