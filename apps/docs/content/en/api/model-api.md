---
title: Model API
description: The endpoints apps call, how they authenticate and how they name models.
order: 1
---

Apps call the model API with the official OpenAI and Anthropic SDKs, or with Google's Gen AI SDK: point the SDK at your install and pass a Kaeso API key. Kaeso translates each request into the format of the model's provider and the reply back, streams included.

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

With Google's Gen AI SDK (`google-genai`), the same request in the Gemini format:

```bash tab="curl"
curl https://kaeso.example.com/v1beta/models/claude-haiku-4-5:generateContent \
  -H "x-goog-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "contents": [
      {"role": "user", "parts": [{"text": "Write a haiku about gateways."}]}
    ]
  }'
```

```python tab="Python"
import os
from google import genai

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com"},
)

response = client.models.generate_content(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
print(response.text)
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

const response = await ai.models.generateContent({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
console.log(response.text);
```

| SDK | Base URL | Sends the key as |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `Authorization: Bearer` |
| Google Gen AI (`google-genai`) | `https://kaeso.example.com` | `x-goog-api-key` |

Any format reaches any generation model in the project's catalogue, so a Gemini app can call Claude and an OpenAI app a Gemini model. [Calling models](../get-started/calling-models.md) shows streaming and more of each SDK.

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
| `POST /v1beta/models/{model}:generateContent` | Gemini |
| `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` | Gemini, streamed |
| `POST /v1beta/models/{model}:countTokens` | Gemini, counting tokens |
| `POST /v1beta/models/{model}:embedContent` | Gemini embeddings |
| `POST /v1beta/models/{model}:batchEmbedContents` | Gemini embeddings, several inputs |
| `GET /v1beta/models` | The models the key may call, in the Gemini format |
| `GET /v1beta/models/{model}` | One of them, in the Gemini format |

In a `/v1beta` path, `{model}` is any [model name](#model-names), such as `claude-haiku-4-5`.

`/v1/embeddings`, `:embedContent` and `:batchEmbedContents` serve only embeddings models, which only some [providers](../concepts/supported-providers.md) offer, and embeddings models serve no other endpoint. Kaeso answers 400 to a request that mixes them.

Any other path under `/v1` gets 404, and a known path called with the wrong method gets 405. Both errors are in the Anthropic format when the request has an `anthropic-version` header, and in the OpenAI format otherwise.

Under `/v1beta`, every error is in the Gemini format. Other methods, such as `batchGenerateContent`, and other paths, such as files or cached contents, get 404. A known path called with the wrong method gets 405. A call takes only the query parameters `key` and `alt`, and any other gets 400. A Gemini call under `/v1`, which `google-genai` makes with `api_version="v1"`, gets 404 with a message that points to `/v1beta`.

### Listing models

`GET /v1/models` chooses its format the same way, by the `anthropic-version` header. `GET /v1beta/models` always answers in the Gemini format and names each model `models/<name>`. The list holds the catalogue's models that the key may call, followed by the project's aliases that point at them. Each model is listed by its own id, or as `provider/model` when its own id doesn't lead to it, for example when another provider serves the same id ([model names](#model-names)). The list leaves out a model that the provider doesn't serve to the project's provider key ([availability](../console/models-and-aliases.md#availability)).

| | OpenAI format | Anthropic format | Gemini format |
| --- | --- | --- | --- |
| Paging | None; one answer holds every model | `limit` (1 to 1000, default 20), `after_id` or `before_id` | `pageSize` (default 50), and `pageToken` with the `nextPageToken` of the page before |
| Deprecated models | Not marked | `lifecycle: "deprecated"`, as KPMR marks them | Not marked |

`GET /v1/models/{model}` and `GET /v1beta/models/{model}` take any [model name](#model-names). They answer 404 when the model isn't in the catalogue, the key may not use it, or the provider doesn't serve it.

## Keys

Send an API key with inference access in one of these headers, on any endpoint:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
x-goog-api-key: sk-kso-…
```

When a request sends several, `x-api-key` wins, then `x-goog-api-key`, then `Authorization`. Under `/v1beta`, the key can also come in the query, as `?key=sk-kso-…`, as in Google's REST examples. Kaeso uses it only when no header carries a key, and the query key never reaches the logs or the provider. The key decides the project. It acts with the rights and model access of its member or service account, which needs the right `api.request`.

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

A Gemini app calls `:streamGenerateContent?alt=sse` instead, as `google-genai` does. Kaeso streams Gemini replies only as server-sent events, so a stream without `alt=sse` gets 400.

- **Keepalives.** After 15 seconds without output, Kaeso writes a keepalive that the SDKs ignore: an SSE comment in the OpenAI formats, a `ping` event in the Anthropic format, and a blank line in the Gemini format, because `google-genai` reads every other line as JSON. `KAESO_STREAMS_KEEPALIVE_INTERVAL` changes the interval or turns keepalives off ([streams to apps](../self-hosting/configuration.md#streams-to-apps)).
- **Failures.** A stream can't change its status once it has started. When the provider fails, or ends the stream before the reply is complete, the stream ends with an error event in the app's format.
- **Disconnects.** When the app disconnects before the stream's last event, Kaeso stops the provider's request at once and charges what was used. A stream whose last event reached the app counts as finished, even when the app disconnects right after it.

## Reasoning across tool calls

Chat Completions has no field for a model's reasoning, and the Gemini format has none for another provider's reasoning. When a Chat Completions or Gemini app calls a model served through another format, Kaeso keeps the model's reasoning before each tool call. It adds the reasoning back on the app's next turn ([format translation](../concepts/format-translation.md#chat-completions)). Gemini's own thought signatures go back and forth unchanged when a Gemini app calls a Gemini model.

## Response cache

A project can answer a repeated identical request from a stored reply ([response cache](../console/response-cache.md)). Send `Cache-Control: no-cache` to ask the provider again and store the new reply, or `Cache-Control: no-store` to skip the cache for that request.

## Response headers

| Header | Value |
| --- | --- |
| `kaeso-request-id` | The request's id, `req_…`, on every answer. The request log shows the same id |
| `kaeso-provider` | The provider that served the request, such as `anthropic`, once the model is known, errors included |
| `kaeso-model` | The provider's model id, once the model is known, errors included |
| `kaeso-left-out` | What Kaeso left out because the model has no equivalent, as the request names it, such as `tools[1], text.verbosity`, `temperature` for a model that takes only its default, or `messages[4].output_config.effort` for the effort of a system message; an item of the conversation by its type and id, such as `web_search_call ws_67c9…`. Only on answers that left something out ([tools left out](../development/kpp/cross-format.md#tools-left-out), [models without effort levels](../development/kpp/reasoning.md#models-without-effort-levels)) |
| `kaeso-adapted` | The tools whose parameter schema Kaeso merged into one object because the model takes no union at its root, by their path in the request, such as `tools[0].tools[3]`. The model sees the variants in the tool's description, and its calls reach the app unchanged. Also the tool calls in the conversation whose arguments Kaeso turned into an object for a model that takes only objects, as `tool call` and the call's id, such as `tool call call_abc`. Also the tool results whose images and files Kaeso moved into the next user message for a Chat Completions model, as `tool result` and the call's id, such as `tool result call_abc`. Also an Anthropic app's `metadata.user_id` that Kaeso sent as its SHA-256 hash because the provider takes a shorter end-user id, such as OpenAI's 128 characters. Also an output limit above the model's own in a request from another format, which Kaeso lowered to the model's maximum, by the app's field, such as `max_tokens`. Also thinking turned off for a model that always reasons, which gets its lowest effort level instead, by the app's field, such as `thinking`. Also an effort the model doesn't list, which becomes the nearest level it lists, by the app's field, such as `output_config.effort`, and adaptive thinking for a Claude model without it, which becomes `enabled` thinking with a budget, as `thinking`. Also the `tool_addition` and `tool_removal` blocks of an Anthropic app's system messages, whose tool changes Kaeso applied to the tool list a model in another format got, by the block's path, such as `messages[4].blocks[1]`. Also a tool whose parameter schema holds a `pattern` Kaeso rewrote for a provider that compiles patterns, by its path, such as `tools[2]`. Only on answers that adapted a tool, a call, a result, a tool change, the end user's id, the output limit or the effort ([models that always reason](../development/kpp/reasoning.md#models-that-always-reason), [union tool schemas](../development/kpp/cross-format.md#union-tool-schemas), [tool call arguments](../development/kpp/cross-format.md#tool-call-arguments), [media in tool results](../development/kpp/cross-format.md#media-in-tool-results), [inline tools](../development/kpp/cross-format.md#inline-tools), [provider profiles](../development/kpp/openai-chat-providers.md#provider-profiles)) |
| `kaeso-route` | Every step of an alias's route the request reached, in order, as a structured-field list ([RFC 9651](https://www.rfc-editor.org/rfc/rfc9651)): the step's path, its `model` as `provider/model`, its `status` (the provider's, or the one Kaeso gives a failure of its own: 502 when the provider can't be reached or refuses Kaeso's key, 504 when it doesn't answer in time), and `dur`, its time in milliseconds, when it failed. The answer itself can carry another status in the app's format, such as 503 for Anthropic's 529 to an OpenAI app. A step the key may not use carries `skipped` and no model; a step skipped because it failed moments ago carries `skipped` and its model. Such as `"route";model="anthropic/claude-sonnet-4-5";status=529;dur=1204, "route.fallback";model="openai/gpt-5";status=200`. Only on answers to an alias ([alias routes](../concepts/alias-routes.md#what-apps-and-logs-see)) |
| `kaeso-cache` | `hit` when the [response cache](../console/response-cache.md) answered, `miss` when the request went to the provider, `bypass` when it couldn't be cached or the app sent `Cache-Control: no-store`. Only in projects with the cache on |
| `age` | The seconds since the cached reply was stored. Only on `kaeso-cache: hit` |
| `server-timing` | `upload;dur=…`, the time in milliseconds the app took to send the request body. Then `kaeso;dur=…`, Kaeso's own time in milliseconds before it called the provider, or until it answered when the request didn't reach one, without the upload. Then `upstream;dur=…`, the provider's time to its first byte, once known. A stream sends it with its headers, so the time Kaeso spends translating the stream is only in the [request log](../console/request-logs.md#read-a-request) |

## Calling from a browser

A browser lets a web page call another origin only when that origin answers with CORS headers. Kaeso sends none until `KAESO_CORS_ALLOWED_ORIGINS` lists the origins of your web apps, such as `https://app.example.com,http://localhost:5173` ([browser apps](../self-hosting/configuration.md#browser-apps)). The rules then apply to `/v1` and `/v1beta` only:

- **Exact origins.** Scheme, host and port must match: `https://app.example.com` admits neither `http://app.example.com` nor `https://www.app.example.com`. Kaeso refuses `*` at startup, because it would let every website send requests with the keys its visitors hold.
- **Preflights.** An `OPTIONS` request with `Origin` and `Access-Control-Request-Method` from a listed origin gets 204 without a key. The answer carries `Access-Control-Allow-Origin` with that origin, `Access-Control-Allow-Methods: GET, POST` and `Access-Control-Max-Age: 7200`. `Access-Control-Allow-Headers` lists those of the requested headers that the official SDKs send: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key`, `x-goog-api-key`, `x-goog-api-client` and `x-stainless-…`. A preflight isn't a model request: it isn't logged, charged or counted against limits.
- **Answers.** Every answer to a listed origin, errors included, carries `Access-Control-Allow-Origin` with that origin and `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, kaeso-route, kaeso-cache, age, retry-after, retry-after-ms`, so the app can read those headers.
- **Other origins** get no CORS headers, so the browser blocks the call, and their preflights get an empty 204. Every answer on `/v1` and `/v1beta` carries `Vary: Origin`, so caches keep the answers apart.
- **No cookies.** Kaeso never sends `Access-Control-Allow-Credentials`, since keys travel in headers. The Platform API, the health checks and the metrics never answer cross-origin requests.

The OpenAI and Anthropic SDKs refuse to run in a browser unless the app passes `dangerouslyAllowBrowser: true`.

> [!WARNING]
> Everyone who uses a page can read the key it holds. Give a browser app its own key with inference access only, a short expiry, only the models it needs and tight limits. Or have your own backend hand out short-lived keys ([API keys](../console/api-keys.md)).

## What Kaeso doesn't do

- **Retries.** Kaeso sends each request once. The SDKs retry on their own where the error allows it.
- **Stored state.** Kaeso refuses Responses requests that ask the provider to store or continue a response (`store: true`, `previous_response_id`, `conversation`, `background: true`). Send the whole conversation each time.
- **Silent changes.** Kaeso refuses a field it can't carry to the model's provider, with an error that names it, and never drops it. In Chat Completions, Messages and Embeddings, a field whose name starts with `header:` or contains a `.` gets 400, because Kaeso can't carry that name; Responses keeps such fields in the body. [Format translation](../concepts/format-translation.md) lists what crosses between formats.
