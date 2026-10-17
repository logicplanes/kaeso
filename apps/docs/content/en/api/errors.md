---
title: Errors
description: What a failed model request returns, in each API format.
order: 2
---

A failed model request gets an HTTP status and an error body in the format the app called, so the SDK raises its usual error. The message says what went wrong and what to do. Every answer carries `kaeso-request-id`, and the request log has an entry under the same id, with the provider's own message where there was one. The Platform API has [its own errors](platform-api.md#errors).

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

`Retry-After` is in whole seconds, rounded up; when the wait has a fraction of a second, `retry-after-ms` gives it in milliseconds. The shape has no field for the request's id, so read it from the `kaeso-request-id` header.

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

`request_id` is the same id as in `kaeso-request-id`, also on an error that comes from the provider.

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
| Any other | `api_error` |

## Why requests fail

| Status | Cause |
| --- | --- |
| 400 | The request can't be read; the model doesn't take a setting the request uses; a field can't be carried to the provider; the context is too long; several providers in the project serve the model id; no provider key is set up for the model's provider |
| 401 | No key; or the key is unknown, disabled, archived or expired; its member or service account is no longer active; its project or organization can't be used |
| 403 | The key has platform access only, is an organization platform key, or may not call models (`api.request`); or the key or its member may not use this model |
| 404 | The model isn't in the project's catalogue, the provider doesn't serve it to the project's provider key, or the path doesn't exist |
| 405 | The path doesn't take this method |
| 413 | The body is larger than `KAESO_BODY_LIMITS_MODEL_API`, 32 MiB by default |
| 429 | A rate limit or budget, or too many unknown keys from the same address. `Retry-After` says when to try again, except for a used-up budget, whose message says when it resets |
| 500 | A provider key can't be decrypted, or the model's provider isn't available in this install; in the OpenAI formats, also every 503 cause below |
| 502 | The provider can't be reached, or the transfer failed |
| 503 | In the Anthropic format: the catalogue, the API keys or the provider keys can't be loaded, a hard budget can't be checked because Valkey is down, or the gateway is restarting. In the OpenAI formats: the provider is overloaded |
| 504 | The provider didn't answer in time |
| 529 | The provider is overloaded, in the Anthropic format |

A stream that has started can't change its status: when the provider fails, or ends the stream before the reply is complete, the stream ends with an error event in the app's format instead.

## Provider errors

When the app and the provider speak the same format, a provider's error comes back as the provider sent it, with its status; an Anthropic error gets Kaeso's request id in `request_id`. Otherwise the error keeps its meaning in the app's format: a provider key the provider refuses comes back as 401 or 403, the provider's rate limit as 429, its billing refusal as 402 or 429, and its other server errors with their 5xx status. The request log keeps the provider's own message.

### A model the provider doesn't serve

When the provider answers 404 for a model in the catalogue, Kaeso checks whether the provider still serves that model to the project's provider key. It asks the provider's `GET /v1/models/{model}`, and, since a 404 there can also mean the provider has no such endpoint, reads the provider's full listing, `GET /v1/models`.

| The listing | What the app gets | The catalogue |
| --- | --- | --- |
| Answers without the model, because it is retired or the key has no access to it | 404 with a message that names the provider and the model; `param` is `model` in the OpenAI formats | The model is marked **Unavailable** ([availability](../console/models-and-aliases.md#availability)), unless the project's provider key changed in the meantime |
| Lists the model | The provider's 404 as it was sent: it was about something else, such as a file id | Unchanged |
| Doesn't answer, as with some OpenAI-compatible services | The provider's 404 as it was sent | Unchanged |

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "Anthropic does not serve the model `claude-sonnet-4-5` to this project's provider key; check the key's access to it, or remove the model from the catalogue."
  },
  "request_id": "req_…"
}
```

Kaeso never sends the request again, to this model or another.
