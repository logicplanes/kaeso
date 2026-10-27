---
title: Model API
description: The endpoints apps call, how they authenticate and how they name models.
order: 1
---

Apps call the model API with the official OpenAI and Anthropic SDKs: point the SDK at your install and pass a Kaeso API key. Kaeso translates each request into the format of the model's provider and the reply back, streams included.

With the OpenAI SDK, this Chat Completions request is served by Claude:

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const completion = await client.chat.completions.create({
  model: "claude-haiku-4-5",
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(completion.choices[0].message.content);
```

With the Anthropic SDK, the same request in the Messages format:

```bash tab="curl"
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 200,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
for (const block of message.content) {
  if (block.type === "text") console.log(block.text);
}
```

| SDK | Base URL | Sends the key as |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

Any format reaches any generation model in the project's catalogue. [Calling models](../get-started/calling-models.md) shows streaming and more of both SDKs.

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

`/v1/embeddings` serves only embeddings models, which only some [providers](../concepts/supported-providers.md) offer, and embeddings models serve no other endpoint. Kaeso answers 400 to a request that mixes them.

Any other path under `/v1` gets 404, and a known path called with the wrong method gets 405. Both errors are in the Anthropic format when the request has an `anthropic-version` header, and in the OpenAI format otherwise.

### Listing models

`GET /v1/models` chooses its format the same way, by the `anthropic-version` header. The list holds the catalogue's models that the key may call, followed by the project's aliases that point at them. Each model is listed by its own id, or as `provider/model` when its own id doesn't lead to it, for example when another provider serves the same id ([model names](#model-names)). The list leaves out a model that the provider doesn't serve to the project's provider key ([availability](../console/models-and-aliases.md#availability)).

| | OpenAI format | Anthropic format |
| --- | --- | --- |
| Paging | None; one answer holds every model | `limit` (1 to 1000, default 20), `after_id` or `before_id` |
| Deprecated models | Not marked | `lifecycle: "deprecated"`, as models.dev marks them |

`GET /v1/models/{model}` takes any [model name](#model-names). It answers 404 when the model isn't in the catalogue, the key may not use it, or the provider doesn't serve it.

## Keys

Send an API key with inference access in either header, on any endpoint:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

When a request sends both, `x-api-key` wins. The key decides the project. It acts with the rights and model access of its member or service account, which needs the right `api.request`.

Kaeso refuses keys with platform access only, and organization platform keys (`sk-kso-org-…`), with 403: they never call models. [API keys](../console/api-keys.md) explains how to create and limit keys.

## Model names

A request names a model in one of three forms. Kaeso looks in the project's catalogue in this order:

| Form | Example |
| --- | --- |
| An alias of the project | `fast` |
| The provider and the model | `anthropic/claude-haiku-4-5` |
| The model's own id | `claude-haiku-4-5` |

Kaeso answers 404 for a model outside the catalogue and for one the key or its member may not use. Both get the same message, so a key can't learn which other models the project has. When several providers in the project serve the same model id, the plain id gets 400 with a message that names the providers; send `provider/model` instead. [Models and aliases](../console/models-and-aliases.md) explains the catalogue.

## Streaming

Ask for a stream as the format does, with `"stream": true`. The reply is a server-sent event stream in the app's format.

- **Keepalives.** After 15 seconds without output, Kaeso writes a keepalive that the SDKs ignore: an SSE comment in the OpenAI formats, a `ping` event in the Anthropic format. `KAESO_STREAMS_KEEPALIVE_INTERVAL` changes the interval or turns keepalives off ([streams to apps](../self-hosting/configuration.md#streams-to-apps)).
- **Failures.** A stream can't change its status once it has started. When the provider fails, or ends the stream before the reply is complete, the stream ends with an error event in the app's format.
- **Disconnects.** When the app disconnects before the stream's last event, Kaeso stops the provider's request at once and charges what was used. A stream whose last event reached the app counts as finished, even when the app disconnects right after it.

## Reasoning across tool calls

Chat Completions has no field for a model's reasoning. When a Chat Completions app calls a model served through another format, Kaeso keeps the model's reasoning before each tool call. It adds the reasoning back on the app's next turn ([format translation](../concepts/format-translation.md#chat-completions)).

## Response headers

| Header | Value |
| --- | --- |
| `kaeso-request-id` | The request's id, `req_…`, on every answer. The request log shows the same id |
| `kaeso-provider` | The provider that served the request, such as `anthropic`, once the model is known, errors included |
| `kaeso-model` | The provider's model id, once the model is known, errors included |
| `kaeso-left-out` | What Kaeso left out because the model has no equivalent, as the request names it, such as `tools[1], text.verbosity`; an item of the conversation by its type and id, such as `web_search_call ws_67c9…`. Only on answers that left something out ([tools left out](../development/kpp/cross-format.md#tools-left-out)) |
| `kaeso-adapted` | The tools whose parameter schema Kaeso merged into one object because the model takes no union at its root, by their path in the request, such as `tools[0].tools[3]`. The model sees the variants in the tool's description, and its calls reach the app unchanged. Also the tool calls in the conversation whose arguments Kaeso turned into an object for a model that takes only objects, as `tool call` and the call's id, such as `tool call call_abc`. Only on answers that adapted a tool or a call ([union tool schemas](../development/kpp/cross-format.md#union-tool-schemas), [tool call arguments](../development/kpp/cross-format.md#tool-call-arguments)) |
| `server-timing` | `upload;dur=…`, the time in milliseconds the app took to send the request body. Then `kaeso;dur=…`, Kaeso's own time in milliseconds before it called the provider, or until it answered when the request didn't reach one, without the upload. Then `upstream;dur=…`, the provider's time to its first byte, once known. A stream sends it with its headers, so the time Kaeso spends translating the stream is only in the [request log](../console/request-logs.md#read-a-request) |

## Calling from a browser

A browser lets a web page call another origin only when that origin answers with CORS headers. Kaeso sends none until `KAESO_CORS_ALLOWED_ORIGINS` lists the origins of your web apps, such as `https://app.example.com,http://localhost:5173` ([browser apps](../self-hosting/configuration.md#browser-apps)). The rules then apply to `/v1` only:

- **Exact origins.** Scheme, host and port must match: `https://app.example.com` admits neither `http://app.example.com` nor `https://www.app.example.com`. Kaeso refuses `*` at startup, because it would let every website send requests with the keys its visitors hold.
- **Preflights.** An `OPTIONS` request with `Origin` and `Access-Control-Request-Method` from a listed origin gets 204 without a key. The answer carries `Access-Control-Allow-Origin` with that origin, `Access-Control-Allow-Methods: GET, POST` and `Access-Control-Max-Age: 7200`. `Access-Control-Allow-Headers` lists those of the requested headers that the official SDKs send: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key` and `x-stainless-…`. A preflight isn't a model request: it isn't logged, charged or counted against limits.
- **Answers.** Every answer to a listed origin, errors included, carries `Access-Control-Allow-Origin` with that origin and `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, retry-after, retry-after-ms`, so the app can read those headers.
- **Other origins** get no CORS headers, so the browser blocks the call, and their preflights get an empty 204. Every answer on `/v1` carries `Vary: Origin`, so caches keep the answers apart.
- **No cookies.** Kaeso never sends `Access-Control-Allow-Credentials`, since keys travel in headers. The Platform API, the health checks and the metrics never answer cross-origin requests.

Both SDKs refuse to run in a browser unless the app passes `dangerouslyAllowBrowser: true`.

> [!WARNING]
> Everyone who uses a page can read the key it holds. Give a browser app its own key with inference access only, a short expiry, only the models it needs and tight limits. Or have your own backend hand out short-lived keys ([API keys](../console/api-keys.md)).

## What Kaeso doesn't do

- **Retries.** Kaeso sends each request once. The SDKs retry on their own where the error allows it.
- **Stored state.** Kaeso refuses Responses requests that ask the provider to store or continue a response (`store: true`, `previous_response_id`, `conversation`, `background: true`). Send the whole conversation each time.
- **Silent changes.** Kaeso refuses a field it can't carry to the model's provider, with an error that names it, and never drops it. In Chat Completions, Messages and Embeddings, a field whose name starts with `header:` or contains a `.` gets 400, because Kaeso can't carry that name; Responses keeps such fields in the body. [Format translation](../concepts/format-translation.md) lists what crosses between formats.
