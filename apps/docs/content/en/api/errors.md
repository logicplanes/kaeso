---
title: Errors
description: What a failed model request returns in each API format, and what each status means.
order: 6
---

A failed model request returns an HTTP status and an error in the format your app called, so the SDK raises its usual error. Every answer carries the header `kaeso-request-id`, and the [request logs](../console/request-logs.md) keep an entry under that id. Platform API errors are on [their own page](platform-api.md#errors).

This request names a model that isn't in the project's catalogue, so Kaeso answers 404:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-9", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import APIStatusError, OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.responses.create(
        model="gpt-9",
        input="Write a haiku about gateways.",
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.responses.create({
    model: "gpt-9",
    input: "Write a haiku about gateways.",
  });
} catch (error) {
  if (!(error instanceof OpenAI.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

## OpenAI formats

Chat Completions, Responses and Embeddings share one error body:

```json
{
  "error": {
    "message": "The model `gpt-9` does not exist or this key may not use it.",
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
| 500, 502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

The body has no field for the request's id, so read it from the `kaeso-request-id` header. `Retry-After` gives the wait in whole seconds, rounded up. When the wait has a fraction of a second, `retry-after-ms` gives it in milliseconds.

## Anthropic format

The same request with the Anthropic SDK:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "gpt-9",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic, APIStatusError

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.messages.create(
        model="gpt-9",
        max_tokens=1024,
        messages=[
            {"role": "user", "content": "Write a haiku about gateways."}
        ],
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.messages.create({
    model: "gpt-9",
    max_tokens: 1024,
    messages: [{ role: "user", content: "Write a haiku about gateways." }],
  });
} catch (error) {
  if (!(error instanceof Anthropic.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

The error body carries the request's id in `request_id`, also when the error comes from the provider:

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "The model `gpt-9` does not exist or this key may not use it."
  },
  "request_id": "req_…"
}
```

| Status | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| Any other | `api_error` |

`Retry-After` gives the wait in whole seconds, rounded up.

## Gemini format

The same request with the Google Gen AI SDK:

```bash tab="curl"
curl -i https://kaeso.example.com/v1beta/models/gpt-9:generateContent \
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
from google.genai import errors

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com"},
)

try:
    client.models.generate_content(
        model="gpt-9",
        contents="Write a haiku about gateways.",
    )
except errors.APIError as error:
    print(error.code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import { ApiError, GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

try {
  await ai.models.generateContent({
    model: "gpt-9",
    contents: "Write a haiku about gateways.",
  });
} catch (error) {
  if (!(error instanceof ApiError)) throw error;
  console.log(error.status, error.message);
}
```

The error body follows Google's shape. It has no field for the request's id, so read it from the `kaeso-request-id` header:

```json
{
  "error": {
    "code": 404,
    "message": "The model `gpt-9` does not exist or this key may not use it.",
    "status": "NOT_FOUND"
  }
}
```

| Status | `error.status` |
| --- | --- |
| 400 | `INVALID_ARGUMENT` |
| 401 | `UNAUTHENTICATED` |
| 403 | `PERMISSION_DENIED` |
| 404 | `NOT_FOUND` |
| 405, 413 | `INVALID_ARGUMENT` |
| 429 | `RESOURCE_EXHAUSTED` |
| 500, 502 | `INTERNAL` |
| 503 | `UNAVAILABLE`, or `INTERNAL` when Kaeso can't load its own data or is restarting |
| 504 | `DEADLINE_EXCEEDED` |

`Retry-After` gives the wait in whole seconds, rounded up. A 429 or 503 with a known wait also names it in `details`, as a `RetryInfo` with `retryDelay`, such as `"23s"`.

## ElevenLabs format

The same mistake with the official ElevenLabs SDK, which sends the key in `xi-api-key`:

```bash tab="curl"
curl -i "https://kaeso.example.com/v1/text-to-speech/21m00Tcm4TlvDq8ikWAM" \
  -H "xi-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"text": "Write a haiku about gateways.", "model_id": "gpt-9"}'
```

```python tab="Python"
import os
from elevenlabs import ElevenLabs
from elevenlabs.core.api_error import ApiError

client = ElevenLabs(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    audio = client.text_to_speech.convert(
        voice_id="21m00Tcm4TlvDq8ikWAM",
        text="Write a haiku about gateways.",
        model_id="gpt-9",
    )
    b"".join(audio)
except ApiError as error:
    print(error.status_code, error.body)
```

```ts tab="TypeScript"
import { ElevenLabsClient, ElevenLabsError } from "@elevenlabs/elevenlabs-js";

const client = new ElevenLabsClient({
  baseUrl: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.textToSpeech.convert("21m00Tcm4TlvDq8ikWAM", {
    text: "Write a haiku about gateways.",
    modelId: "gpt-9",
  });
} catch (error) {
  if (!(error instanceof ElevenLabsError)) throw error;
  console.log(error.statusCode, error.body);
}
```

The error body follows ElevenLabs' documented shape, with the same value in `code` and `status`. Read the request's id from the `kaeso-request-id` header:

```json
{
  "detail": {
    "type": "not_found",
    "code": "not_found",
    "message": "The model `gpt-9` does not exist or this key may not use it.",
    "status": "not_found",
    "param": "model"
  }
}
```

| Status | `detail.type` | `detail.code` |
| --- | --- | --- |
| 400 | `invalid_request` | `invalid_parameters`, or `unsupported_parameter` for a field the model's provider doesn't take |
| 401 | `authentication_error` | `invalid_api_key` |
| 402 | `payment_required` | `insufficient_credits` |
| 403 | `authorization_error` | `permission_denied` |
| 404 | `not_found` | `not_found` |
| 413 | `invalid_request` | `request_too_large` |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 503 | `service_unavailable` | `system_busy` |
| 500, 502, 504 | `internal_error` | `internal_error` |

An error ElevenLabs itself sent reaches the app as ElevenLabs sent it, with its status. `Retry-After` gives the wait in whole seconds, rounded up. A path Kaeso doesn't serve, such as `/v1/dubbing`, answers 404 in this shape ([ElevenLabs API](elevenlabs.md#what-kaeso-refuses)).

## Causes

The message names the cause and what to do. These are the causes behind each status:

| Status | Cause |
| --- | --- |
| 400 | Kaeso can't read the request, or the model doesn't take a setting or field the request uses. |
| 400 | The request holds more messages, content blocks in one message, tools or embedding inputs than Kaeso takes ([the request](../concepts/request-flow.md#the-request)). |
| 400 | The request counts tokens, and the model's provider has no endpoint for counting them. |
| 400 | The context is too long for the model. |
| 400 | Several providers in the project serve the model id. Send `provider/model` instead. |
| 400 | The project has no provider key for the model's provider. |
| 401 | The key is missing, unknown, disabled, archived or expired. |
| 401 | The key's member or service account is no longer active, or its project or organization can't be used. |
| 403 | The key has platform access only, or it is a platform key. Neither calls models. |
| 403 | The key's member or service account lacks the right `api.request`. |
| 403 | A spend budget applies, and the request uses a feature the provider bills beyond tokens, such as a provider-run tool or `service_tier: priority`, or a model without a price ([limits and budgets](../console/limits-and-budgets.md#features-a-spend-budget-cant-price)). |
| 404 | The model isn't in the project's catalogue, the key or its member may not use it, or the provider doesn't serve it to the project's provider key. A model the key may not use answers like one that doesn't exist, so a key can't learn which other models the project has. |
| 404 | The path doesn't exist. |
| 405 | The path doesn't take this method. |
| 413 | The body is larger than `KAESO_BODY_LIMITS_MODEL_API`, 32 MiB by default. |
| 429 | A rate limit or budget is reached, or the request could cost more than what's left of one, and the message names it. For a used-up budget, the message says when it resets. |
| 429 | Too many requests with unknown keys came from your address. |
| 500 | Kaeso can't decrypt the provider key, or the model's provider isn't available in this install. |
| 500 | The model's provider is a custom provider, and the install turned custom providers off. |
| 502 | Kaeso can't reach the provider, or the transfer failed. |
| 502 | A custom provider is at an address the install doesn't allow, or answered in a form its format can't read. |
| 502 | The provider refused the project's provider key, didn't allow the request with it, or refused it for billing or quota reasons. |
| 503 | Kaeso can't load the catalogue, the keys, the provider keys or the limits, or it is restarting. The OpenAI formats answer these with 500. |
| 503 | The provider is overloaded, in the OpenAI and Gemini formats. |
| 504 | The provider didn't answer in time. |
| 529 | The provider is overloaded, in the Anthropic format. |

Once a stream has started, its status can't change. If the provider fails or ends the stream early, the stream ends with an error event in your app's format.

## Provider errors

When your app and the provider use the same format, Kaeso passes the provider's error on with its status, unless the provider refused the project's provider key. An Anthropic error also gets Kaeso's id in `request_id`. In every error, Kaeso replaces anything that looks like a key or an account, user, project or organization id with `[redacted]`. This includes streamed errors and errors rewritten for another format.

Otherwise Kaeso writes the error in your app's format and keeps its meaning:

| The provider | Your app gets |
| --- | --- |
| Refuses the provider key | 502, in every format |
| Doesn't allow the request with the key, for example for a model the key's plan excludes | 502, in every format |
| Rate-limits the request | 429 |
| Refuses for billing or quota reasons | 502, in every format |
| Is overloaded | 529 in the Anthropic format, 503 in the OpenAI and Gemini formats |
| Fails with another server error | Its own 5xx status. A Gemini app gets 500, or 504 for a timeout |

A refused provider key is the install's problem, not your app's. The provider's 401, 402 or 403 would make an SDK report a wrong Kaeso key. The provider's message can also hold a fragment of the provider key or the provider's organization id.

Instead, your app gets a provider error with the message "The provider refused this project's provider key. Ask an organization admin to check it under Providers.", or the same with "for billing or quota reasons". For the provider's 403, which comes with a working key that may not make this request, the message is "The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers." An organization admin checks the key under **Providers**.

The request log keeps the provider's own message, with anything that looks like a key, a project or an organization id replaced by `[redacted]`. Kaeso sends every request once and never retries it, on this model or another. The SDKs retry on their own where the error allows it.

### Custom providers

A [custom provider](../console/custom-providers.md) gets messages that name it by its id, so you can tell your own server's problems from a provider's:

| What happened | Status | Message |
| --- | --- | --- |
| Kaeso can't connect to it | 502 | "Kaeso cannot reach the custom provider `…`; check its base URL." |
| Its address isn't allowed | 502 | "The custom provider `…` is at an address this install does not allow; ask its operator." |
| It refused the project's key | 502 | "The custom provider `…` refused this project's key; ask an organization admin to check it under Providers." |
| It wants a key, but takes none in Kaeso | 502 | "The custom provider `…` asks for a key; an organization admin sets how it signs in under Providers." |
| Its reply isn't in its format | 502 | "The custom provider `…` answered in a form `…` cannot read; check its API format." |

When the provider answers with an error in its format, your app gets that error as for any provider. The request log keeps what the provider said.

### Models the provider doesn't serve

When the provider answers 404 for a model in the catalogue, Kaeso asks the provider whether it still serves that model to the project's provider key. If it doesn't, your app gets 404 with a message that names the provider and the model. A 410, which providers send for a model they retired, counts like a 404. In another format than the provider's, your app gets it as 404.

The catalogue then shows the model as **Unavailable** ([availability](../console/models-and-aliases.md#availability)), unless the project's provider key changed during the request. Kaeso then answers later requests for the model with the same 404 without calling the provider, apart from one request every five minutes. The first answer and every later one carry the same message, and so does the request log.

If the provider still lists the model, your app gets the provider's 404 as it was sent: it was about something else, such as a file id. If the provider can't say, because it has no model listing or the listing fails, your app also gets the provider's 404 as it was sent. The catalogue doesn't change in either case.
