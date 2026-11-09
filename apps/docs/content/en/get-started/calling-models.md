---
title: Calling models
description: Call any model in your project with the OpenAI, Anthropic or Google Gen AI SDK, or with curl.
order: 2
---

Point the official OpenAI or Anthropic SDK, or Google's Gen AI SDK, at your install and give it a Kaeso API key. Any SDK can call any chat model in your project, whichever [provider](../concepts/providers.md) serves it, so the OpenAI SDK can call Claude.

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

Set the base URL to `https://kaeso.example.com`, without `/v1`. The SDK sends the key as `x-api-key`, and `Authorization: Bearer` works too:

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

Every endpoint accepts the key in `Authorization: Bearer`, `x-api-key` or `x-goog-api-key`, so a tool that only sends `Authorization: Bearer` works here too.

## Use the Google Gen AI SDK

Set the base URL to `https://kaeso.example.com`, without a version: `google-genai` adds `/v1beta` itself. It sends the key as `x-goog-api-key`:

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

Keep the SDK's default API version, `v1beta`, because Kaeso serves the Gemini API only under `/v1beta`. To stream, call `generate_content_stream`. The SDK asks for server-sent events with `alt=sse`, the only kind of stream Kaeso sends to Gemini apps:

```bash tab="curl"
curl -N "https://kaeso.example.com/v1beta/models/claude-haiku-4-5:streamGenerateContent?alt=sse" \
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

stream = client.models.generate_content_stream(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
for chunk in stream:
    print(chunk.text or "", end="")
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

const stream = await ai.models.generateContentStream({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
for await (const chunk of stream) {
  process.stdout.write(chunk.text ?? "");
}
```

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
