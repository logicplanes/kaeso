# anthropic_messages / errors

`<name>.json` is the exact error body; `<name>.meta.json` is `{"status": N, "headers": {...}}`. Shape per https://platform.claude.com/docs/en/api/errors#error-shapes: `{"type": "error", "error": {"type", "message"}, "request_id"}`. The mid-stream error is in `streams/error-overloaded-mid-stream.sse`. Status table (errors page): 400 invalid_request_error, 401 authentication_error, 402 billing_error, 403 permission_error, 404 not_found_error, 409 conflict_error (not used by Messages, no fixture), 413 request_too_large, 429 rate_limit_error, 500 api_error, 504 timeout_error, 529 overloaded_error.

| file | what it covers | provenance |
|---|---|---|
| `invalid-request-beta-header.json` | 400 `invalid_request_error` (bad `anthropic-beta`), with `request_id`. | doc: https://platform.claude.com/docs/en/api/beta-headers#error-handling |
| `invalid-request-beta-header.meta.json` | HTTP status (and headers) for the body above. | doc: https://platform.claude.com/docs/en/api/beta-headers#error-handling |
| `invalid-request-prefill.json` | 400 `invalid_request_error` without `request_id` (as shown in the doc). | doc: https://platform.claude.com/docs/en/api/errors#prefill-not-supported |
| `invalid-request-prefill.meta.json` | HTTP status (and headers) for the body above. | doc: https://platform.claude.com/docs/en/api/errors#prefill-not-supported |
| `authentication.json` | 401 `authentication_error`. | schema: ErrorResponse, AuthenticationError |
| `authentication.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, AuthenticationError |
| `billing.json` | 402 `billing_error`. | schema: ErrorResponse, BillingError |
| `billing.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, BillingError |
| `permission.json` | 403 `permission_error`. | schema: ErrorResponse, PermissionError |
| `permission.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, PermissionError |
| `tool-search-all-deferred.json` | 400 `invalid_request_error` for a request whose tools all have `defer_loading: true`. Kaeso refuses such a request with the same message for other formats. | live: `claude-sonnet-5-5` through Kaeso (same format) on 2026-10-25; same message as https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool#http-errors-400-status |
| `tool-search-all-deferred.meta.json` | HTTP status for the body above. | live, as above |
| `tool-search-unknown-reference.json` | 400 `invalid_request_error` for a `tool_reference` that names a tool the request doesn't send. Kaeso refuses such a request with the same message for other formats. | live: `claude-sonnet-5-5` through Kaeso (same format) on 2026-10-25; same message as https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool#http-errors-400-status |
| `tool-search-unknown-reference.meta.json` | HTTP status for the body above. | live, as above |
| `not-found.json` | 404 `not_found_error`. | doc: https://platform.claude.com/docs/en/api/errors#error-shapes |
| `not-found.meta.json` | HTTP status (and headers) for the body above. | doc: https://platform.claude.com/docs/en/api/errors#error-shapes |
| `not-found-file.json` | 404 `not_found_error` for a missing `file_id`. | doc: https://platform.claude.com/docs/en/build-with-claude/files (error handling) |
| `not-found-file.meta.json` | HTTP status (and headers) for the body above. | doc: https://platform.claude.com/docs/en/build-with-claude/files (error handling) |
| `request-too-large.json` | 413 `request_too_large`. Not in the OpenAPI `ErrorResponse` union; the errors page documents it and says Cloudflare returns it before the API, so no `request_id` is included. Message text is constructed. | schema: ErrorResponse shape + error type from https://platform.claude.com/docs/en/api/errors#http-errors |
| `request-too-large.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse shape + error type from https://platform.claude.com/docs/en/api/errors#http-errors |
| `rate-limit.json` | 429 `rate_limit_error` with `retry-after: 20` and `anthropic-ratelimit-input-tokens-*` headers in meta. Message text is constructed. | schema: ErrorResponse, RateLimitError (headers from https://platform.claude.com/docs/en/api/rate-limits#response-headers) |
| `rate-limit.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, RateLimitError (headers from https://platform.claude.com/docs/en/api/rate-limits#response-headers) |
| `rate-limit-spend-cap.json` | 429 `rate_limit_error` **without** `retry-after`; carries `error.details.error_code: enforced_spend_limit_reached` (field not in the OpenAPI spec). | doc: https://platform.claude.com/docs/en/api/rate-limits#reaching-your-spend-cap |
| `rate-limit-spend-cap.meta.json` | HTTP status (and headers) for the body above. | doc: https://platform.claude.com/docs/en/api/rate-limits#reaching-your-spend-cap |
| `api-error.json` | 500 `api_error`. | schema: ErrorResponse, APIError |
| `api-error.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, APIError |
| `timeout.json` | 504 `timeout_error`. | schema: ErrorResponse, GatewayTimeoutError |
| `timeout.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, GatewayTimeoutError |
| `overloaded.json` | 529 `overloaded_error` (message 'Overloaded' as in the streaming doc). | schema: ErrorResponse, OverloadedError |
| `overloaded.meta.json` | HTTP status (and headers) for the body above. | schema: ErrorResponse, OverloadedError |
