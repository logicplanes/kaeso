---
title: Calling models
description: Call models through Kaeso with the OpenAI and Anthropic SDKs, curl and streaming.
order: 2
---

Apps call Kaeso with the official SDKs they already use; only the base URL and the key change. Any SDK can call any chat model in the project's catalogue, so an OpenAI SDK can call Claude and an Anthropic SDK an OpenAI model. Kaeso translates the request and the reply.

| SDK | Base URL | The key is sent as |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

The examples read an API key from `KAESO_API_KEY`, created as in the [quick start](quick-start.md#create-an-api-key), and call `claude-haiku-4-5`. Use a model in your project's catalogue.

## OpenAI SDK

The Responses API in Python:

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

The same client calls Chat Completions:

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

In Python:

```python
import os
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])

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

Every format streams, with the events its SDK expects. With the Anthropic client from above:

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

While a stream is silent, for example while the model thinks, Kaeso sends keepalives that the SDKs ignore, so proxies along the way keep the connection open ([long streams](../self-hosting/domains-and-tls.md#long-streams)).

## curl

The OpenAI formats:

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

The Anthropic format:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

Every endpoint accepts the key in either header. [Model API](../api/model-api.md) lists the endpoints.

## From a browser

Kaeso answers a web page's requests only when its origin is listed in `KAESO_CORS_ALLOWED_ORIGINS`, and everyone who uses the page can read the key it holds. [Calling from a browser](../api/model-api.md#calling-from-a-browser) covers both.

## Find a request again

Open **Request logs** in the project: every request is there with its status, model, tokens, cost and timing. Each answer carries a `kaeso-request-id` header with the id the log shows, so a request in your app's own logs can be found in Kaeso's.

A failed request gets an error in the format its SDK expects, saying why. [Errors](../api/errors.md) lists them.
