---
title: Calling models
description: Call models through Kaeso with the OpenAI and Anthropic SDKs, curl and streaming.
order: 2
---

Apps call Kaeso with the official SDKs they already use. Only the base URL and the key change, and any SDK can call any model in the project's catalogue: an OpenAI SDK can call a Claude model, and an Anthropic SDK an OpenAI model. Kaeso translates the request and the reply.

| SDK | Base URL | Key |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `api_key`, sent as `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `api_key`, sent as `x-api-key` |

The examples use a key created under **API keys** in a project, as in the [quick start](quick-start.md#create-an-api-key), and the model `claude-haiku-4-5`. Use a model that is in your project's catalogue.

## OpenAI SDK

The Responses API, in Python:

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

The same client also serves Chat Completions:

```python
completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

In TypeScript:

```ts
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const response = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways." });
console.log(response.output_text);
```

## Anthropic SDK

```python
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key="sk-kso-…")

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

In TypeScript:

```ts
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({ baseURL: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(message.content);
```

## Streaming

Streams work in every format, with the events the SDK expects. With the Anthropic client from above:

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

While a stream is silent, for example while a model thinks, Kaeso sends keepalives the SDKs ignore, so proxies on the way keep the connection open. See [long streams](../self-hosting/domains-and-tls.md#long-streams).

## curl

The OpenAI formats take the key as a bearer token:

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

The Anthropic format takes it in `x-api-key`:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

Kaeso accepts either header on every endpoint of the model API. [Model API](../api/model-api.md) lists every endpoint.

## Find a request again

Open **Request logs** in the project. Every request is there with its status, model, tokens, cost and timing. Every answer from Kaeso carries a `kaeso-request-id` header with the same id, so you can find a request from your app's own logs.

If a request fails, the error says why in the format your SDK expects. [Errors](../api/errors.md) lists them.
