---
title: 调用模型
description: 使用 OpenAI、Anthropic 或 Google Gen AI SDK，或者用 curl，调用项目中的任意模型。
order: 2
---

把官方 OpenAI 或 Anthropic SDK，或者 Google 的 Gen AI SDK，指向您的实例，并提供一个 Kaeso API 密钥。任何 SDK 都能调用项目中的任意聊天模型，无论由哪个[服务商](../concepts/providers.md)提供，因此 OpenAI SDK 也能调用 Claude。

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

示例从 `KAESO_API_KEY` 读取密钥，并调用 `claude-haiku-4-5`。按[快速开始](quick-start.md)中的步骤创建密钥，并使用项目目录中的模型。

OpenAI SDK 以 `https://kaeso.example.com/v1` 为基础 URL，并以 `Authorization: Bearer` 发送密钥。同一个客户端也能调用 Chat Completions。[模型 API](../api/model-api.md) 展示了这个请求，并列出了所有端点。

## 使用 Anthropic SDK

把基础 URL 设为 `https://kaeso.example.com`，不带 `/v1`。SDK 以 `x-api-key` 发送密钥，`Authorization: Bearer` 同样可用：

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

每个端点都接受放在 `Authorization: Bearer`、`x-api-key` 或 `x-goog-api-key` 中的密钥，所以只发送 `Authorization: Bearer` 的工具也能使用。

## 使用 Google Gen AI SDK

把基础 URL 设为 `https://kaeso.example.com`，不带版本：`google-genai` 会自行加上 `/v1beta`。它以 `x-goog-api-key` 发送密钥：

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

保留 SDK 的默认 API 版本 `v1beta`，因为 Kaeso 只在 `/v1beta` 下提供 Gemini API。要流式接收回复，调用 `generate_content_stream`。SDK 会用 `alt=sse` 请求服务器发送事件，这是 Kaeso 发给 Gemini 应用的唯一一种流：

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

## 流式接收回复

按 SDK 自己的方式请求流式回复。无论哪个服务商提供该模型，Kaeso 都会发送该 SDK 预期的事件：

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

流暂时没有输出时，例如模型正在思考，Kaeso 会发送 SDK 会忽略的保活消息，让沿途的代理保持连接。

## 从浏览器调用

只有当网页的源列在 `KAESO_CORS_ALLOWED_ORIGINS` 中时，Kaeso 才会响应它的请求。[模型 API](../api/model-api.md) 说明了如何设置。

> [!WARNING]
> 使用该网页的每个人都能读到其中的密钥。请为浏览器应用单独创建密钥，并限制在它所需的模型和速率内。

## 在日志中查找请求

每个回复都带有 `kaeso-request-id` 响应头。在应用中记录它，就能在 Kaeso 中找到同一个请求：

1. 在项目中打开**请求日志**。
2. 选择**按 ID 打开请求**，然后粘贴该 ID。

条目会显示状态、模型、Token、费用和耗时。失败的请求还会显示错误，也就是应用以其 SDK 的格式收到的那个错误。

## 下一步

- [编程工具](coding-tools.md)：让 Claude Code、Codex 和其他工具使用您的实例。
- [模型 API](../api/model-api.md)：查阅每个端点、请求头和模型名称。
- [错误](../api/errors.md)：了解错误的含义，以及应用该如何处理。
- [格式转换](../concepts/format-translation.md)：了解 SDK 调用其他服务商的模型时，哪些内容能够保留。
