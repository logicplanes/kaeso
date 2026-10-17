---
title: Model API
description: The endpoints apps call, how they authenticate, and how they name models.
order: 1
---

The model API serves the OpenAI and Anthropic API formats, so apps keep using the official SDKs. Kaeso translates every request into the format of the model's provider and the reply back, streaming included.

## Base URLs

| SDK | Base URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

## Endpoints

| Method and path | Format |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses, counting input tokens |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages, counting tokens |
| `GET /v1/models` | The models the key may call |
| `GET /v1/models/{model}` | One of them |

The generation endpoints can call every generation model in the catalogue, whatever its provider. `/v1/embeddings` calls only embeddings models, which only OpenAI offers, and an embeddings model serves no other endpoint; either mix is refused with 400.

`GET /v1/models` answers in the Anthropic format when the request has an `anthropic-version` header, and in the OpenAI format otherwise. It lists the catalogue's models the key may call, then the project's aliases that point at them. A model the provider does not serve to the project's provider key ([availability](../console/models-and-aliases.md#availability)) is left out, and a model models.dev marks deprecated has `lifecycle: "deprecated"` in the Anthropic format. In the Anthropic format the list is paged with `limit` (default 20), `after_id` and `before_id`. `GET /v1/models/{model}` takes any [model name](#model-names) and answers 404 for a model that isn't in the catalogue, that the key may not use, or that the provider does not serve.

Any other path under `/v1` is answered with 404, and a known path called with the wrong method with 405. Both errors are in the Anthropic format when the request has an `anthropic-version` header, and in the OpenAI format otherwise.

## Keys

Send an API key with inference access in either header, on any endpoint:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

When both are sent, `x-api-key` is used. The key decides the project, and acts with the rights and model access of its member or service account, which needs the right `api.request`. A key with platform access only is refused with 403, and so are organization platform keys (`sk-kso-org-…`), which never call models.

## Model names

A request names a model in one of three ways, and Kaeso looks among the project's catalogue in this order:

| Form | Example |
| --- | --- |
| An alias of the project | `fast` |
| The provider and the model | `anthropic/claude-haiku-4-5` |
| The model's own id | `claude-haiku-4-5` |

A model outside the catalogue is refused with 404, and one the key or its member may not use with 403. When several providers in the project serve the same model id, the plain id is refused with 400 and the message names the providers; send `provider/model` instead. [Models and aliases](../console/models-and-aliases.md) explains the catalogue.

## Streaming

Ask for a stream as the format does (`"stream": true`); the reply is a server-sent event stream in the app's format.

- When nothing was written to the app for 15 seconds, Kaeso writes a keepalive that SDKs ignore: an SSE comment for the OpenAI formats, a `ping` event for the Anthropic format. `KAESO_STREAMS_KEEPALIVE_INTERVAL` changes the interval or turns keepalives off; see [streams to apps](../self-hosting/configuration.md#streams-to-apps).
- A stream that fails after it started, or that the provider ends before the reply is complete, ends with an error event in the app's format.
- When the app disconnects, Kaeso stops the provider's request at once and charges what was used.

## Reasoning across tool calls

Chat Completions has no field for a model's reasoning. When a Chat Completions app calls a model served through another format, Claude or an OpenAI model through Responses, Kaeso keeps the reasoning that came before a tool call and adds it back on the app's next turn with the same key. It is held for an hour by default, encrypted, and never logged; see [reasoning continuity](../self-hosting/configuration.md#reasoning-continuity).

## Response headers

| Header | Value |
| --- | --- |
| `kaeso-request-id` | The request's id, `req_…`, on every answer; the same id is in the request log |
| `kaeso-provider` | The provider that served the request, once the model is known, errors included |
| `kaeso-model` | The provider's model id, once the model is known, errors included |

## Calling from a browser

A browser lets a web page call another origin's API only when that API answers with CORS headers. Kaeso sends none until `KAESO_CORS_ALLOWED_ORIGINS` lists the origins of your web apps, such as `https://app.example.com,http://localhost:5173`; see [browser apps](../self-hosting/configuration.md#browser-apps). Then, on `/v1` only:

- **Preflights.** An `OPTIONS` request with `Origin` and `Access-Control-Request-Method` from a listed origin is answered with 204 and no body, without a key: `Access-Control-Allow-Origin` with that origin, `Access-Control-Allow-Methods: GET, POST`, `Access-Control-Max-Age: 7200`, and in `Access-Control-Allow-Headers` the requested headers among those the official SDKs send: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key` and `x-stainless-…`. A preflight is no model request: it isn't logged, charged or counted against limits.
- **Answers.** Every answer to a listed origin, errors included, carries `Access-Control-Allow-Origin` with that origin and `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, retry-after, retry-after-ms`, so the app can read those headers.
- **Other origins** get no CORS headers, so the browser blocks the call; their preflights get an empty 204. Every answer on `/v1` carries `Vary: Origin`, so caches keep the answers apart.
- **Exact origins.** Scheme, host and port must match: `https://app.example.com` admits neither `http://app.example.com` nor `https://www.app.example.com`. `*` is refused, because it would let every website send requests with the keys its visitors hold.
- **No cookies.** Kaeso never sends `Access-Control-Allow-Credentials`, since keys travel in headers. The Platform API, the health checks and the metrics never answer cross-origin requests.

Both SDKs refuse to run in a browser unless the app passes `dangerouslyAllowBrowser: true`.

> Everyone who uses a page can read the key it holds. Give a browser app its own key with inference access only, a short expiry, only the models it needs and tight limits, or have your own backend hand out short-lived keys; see [API keys](../console/api-keys.md).

## What Kaeso doesn't do

- **Retries.** Kaeso sends each request once. The SDKs retry on their own where the error allows it.
- **Stored state.** Responses requests that ask the provider to store or continue a response (`store: true`, `previous_response_id`, `conversation`, `background: true`) are refused; send the whole conversation each time.
- **Silent changes.** A field that can't be carried to the model's provider is refused with an error that names it, never dropped. Chat Completions, Messages and Embeddings requests with a field whose name starts with `header:` or contains a `.` are refused with 400, because Kaeso can't carry that name; Responses keeps such fields in the body. [Format translation](../concepts/format-translation.md) lists what crosses between formats.
