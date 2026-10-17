---
title: Errors
description: What a failed model request returns, in each API format.
order: 2
---

A failed model request gets an HTTP status and an error body in the format the app called, so the SDK raises its usual error. The message says what went wrong and what to do.

## OpenAI formats

Chat Completions, Responses and Embeddings share one error shape:

```json
{
  "error": {
    "message": "The model `gpt-9` is not in this project's model catalogue. A project admin can add it under the project's models.",
    "type": "invalid_request_error",
    "param": "model",
    "code": null
  }
}
```

| Status | `type` | `code` |
| --- | --- | --- |
| 400 | `invalid_request_error` | `context_length_exceeded` or `content_policy_violation` where they apply |
| 401 | `invalid_request_error` | |
| 403 | `request_forbidden` | |
| 404 | `invalid_request_error` | |
| 405 | `invalid_request_error` | |
| 413 | `invalid_request_error` | |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 429 | `insufficient_quota` | `insufficient_quota`, when the provider refuses for billing reasons |
| 500, 502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

`Retry-After` is in whole seconds, rounded up; when the wait has a fraction of a second, `retry-after-ms` has it in milliseconds. The OpenAI shape has no field for the request's id; it is in the `kaeso-request-id` header.

## Anthropic format

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit reached: this key's request rate of 60 requests per minute; retry in 12 seconds."
  },
  "request_id": "req_…"
}
```

`request_id` is the request's id, the same as in `kaeso-request-id`, also on a provider's own error.

| Status | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 402 | `billing_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| other | `api_error` |

## Provider errors

When the app and the provider speak the same format, a provider's error comes back as the provider sent it, with its status; an Anthropic error carries Kaeso's request id in `request_id`. Otherwise it keeps its meaning in the app's format: a provider key the provider refuses comes back as 401 or 403, the provider's own rate limit as 429, its billing refusal as 402 or 429, and its other server errors with their 5xx status. The request log keeps the provider's own message.

## Why requests fail

| Status | Cause |
| --- | --- |
| 400 | The request can't be read, the model doesn't take a setting it uses, a field can't be carried to the provider, the context is too long, a model id is served by several providers in the project, or no provider key is set up for the model's provider |
| 401 | No key, or the key is unknown, disabled, archived or expired, its member or service account is no longer active, or its project or organization can't be used |
| 403 | The key has platform access only, is an organization platform key, or may not call models (`api.request`), or the key or its member may not use this model |
| 404 | The model isn't in the project's catalogue, or the path isn't served |
| 405 | The path doesn't take this method |
| 413 | The body is larger than `KAESO_BODY_LIMITS_MODEL_API`, 32 MiB by default |
| 429 | A rate limit or budget, or too many unknown keys from the same address; `Retry-After` says when to try again, except for a used-up budget, whose message says when it resets |
| 500 | A provider key can't be decrypted, or the model's provider isn't available in this install; for OpenAI-format apps, also the 503 causes below |
| 502 | The provider couldn't be reached, or the transfer failed |
| 503 | For Anthropic-format apps, the catalogue, the API keys or the provider keys can't be loaded, a hard budget can't be checked because Valkey is down, or the gateway is restarting; for OpenAI-format apps, the provider is overloaded |
| 504 | The provider didn't answer in time |
| 529 | The provider is overloaded, for Anthropic-format apps |

A stream that already started can't change its status: when the provider fails, or ends the stream before the reply is complete, it ends with an error event in the app's format instead.

Every answer carries `kaeso-request-id`. The request log has the same id, with the provider's own message where there was one.
