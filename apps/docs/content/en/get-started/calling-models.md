---
title: Calling models
description: Call any model in your project with the OpenAI or Anthropic SDK, or with curl.
order: 2
---

Point the official OpenAI or Anthropic SDK at your install and give it a Kaeso API key. Any SDK can call any chat model in your project, whichever [provider](../concepts/providers.md) serves it, so the OpenAI SDK can call Claude.

```bash tab="curl"
curl https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

response = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
)
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const response = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
});
console.log(response.output_text);
```

The examples read your key from `KAESO_API_KEY` and call `claude-haiku-4-5`. Create a key as in the [quick start](quick-start.md), and use a model from your project's catalogue.

The OpenAI SDK takes `https://kaeso.example.com/v1` as its base URL and sends the key as `Authorization: Bearer`. The same client calls Chat Completions too. The [Model API](../api/model-api.md) shows that request and lists every endpoint.

## Use the Anthropic SDK

Set the base URL to `https://kaeso.example.com`, without `/v1`. The SDK sends the key as `Authorization: Bearer`, and `x-api-key` works too:

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

Every endpoint accepts the key in either header, so a tool that only sends `Authorization: Bearer` works here too.

## Stream replies

Ask for a stream the way your SDK does. Kaeso sends the events that SDK expects, whichever provider serves the model:

```bash tab="curl"
curl -N https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "input": "Write a haiku about gateways.",
    "stream": true
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

stream = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
    stream=True,
)
for event in stream:
    if event.type == "response.output_text.delta":
        print(event.delta, end="")
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const stream = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
  stream: true,
});
for await (const event of stream) {
  if (event.type === "response.output_text.delta") {
    process.stdout.write(event.delta);
  }
}
```

While a stream is silent, for example while the model thinks, Kaeso sends keepalives that the SDKs ignore. Proxies along the way then keep the connection open.

## Call from a browser

Kaeso answers a web page's requests only when the page's origin is listed in `KAESO_CORS_ALLOWED_ORIGINS`. [Model API](../api/model-api.md) shows how to set it.

> [!WARNING]
> Everyone who uses the page can read the key it holds. Give a browser app its own key, limited to the models and rates it needs.

## Find a request in the logs

Every reply carries a `kaeso-request-id` header. Log it in your app, so you can find the same request in Kaeso:

1. Open **Request logs** in the project.
2. Choose **Open a request by id** and paste the id.

The entry shows the status, model, tokens, cost and timing. A failed request also shows its error, which your app received in the format its SDK expects.

## Next steps

- [Coding tools](coding-tools.md): Point Claude Code, Codex and other tools at your install.
- [Model API](../api/model-api.md): Look up every endpoint, header and model name.
- [Errors](../api/errors.md): Find out what an error means and how to handle it in your app.
- [Format translation](../concepts/format-translation.md): See what carries across when an SDK calls another provider's model.
