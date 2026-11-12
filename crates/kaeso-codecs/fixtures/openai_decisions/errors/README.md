# `fixtures/openai_decisions/errors`

| file | what it covers | provenance |
|---|---|---|
| `too-many-questions-400.json`, `.meta.json` | 400 for 401 questions, with `param` and `code` | schema: `questions` (`maxItems: 400`); message and code in OpenAI's usual wording for an array over its maximum (see the folder's Doubts) |
| `invalid-api-key-401.json`, `.meta.json` | 401 authentication | doc: https://developers.openai.com/api/docs/guides/error-codes ("Incorrect API key provided") |
| `slow-down-429.json`, `.meta.json` | 429 `slow_down` with `retry-after` | doc: https://developers.openai.com/api/docs/guides/error-codes (type `rate_limit_error`, code `slow_down`); message as in `openai_responses/errors/slow-down-429.json` |
| `credit-balance-exhausted-429.json`, `.meta.json` | 429 `credit_balance_exhausted` (maps to KPP Billing) | doc: https://developers.openai.com/api/docs/guides/error-codes (code `credit_balance_exhausted`); type `insufficient_quota` as the guide says |
| `server-error-500.json`, `.meta.json` | 500 internal error | doc: https://developers.openai.com/api/docs/guides/error-codes ("The server had an error while processing your request") |
| `overloaded-503.json`, `.meta.json` | 503 `server_is_overloaded` with `retry-after` | doc: https://developers.openai.com/api/docs/guides/error-codes (type `service_unavailable_error`, code `server_is_overloaded`); message as in `openai_responses/errors/overloaded-503.json` |
