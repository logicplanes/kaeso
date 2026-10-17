---
title: 调用模型
description: 通过 Kaeso，使用 OpenAI 和 Anthropic SDK、curl 以及流式方式调用模型。
order: 2
---

应用使用已有的官方 SDK 调用 Kaeso，只需更改基础 URL 和密钥。任何 SDK 都能调用项目目录中的任意聊天模型：OpenAI SDK 可以调用 Claude，Anthropic SDK 也可以调用 OpenAI 模型。Kaeso 会转换请求和回复。

| SDK | 基础 URL | 密钥的发送方式 |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

示例从 `KAESO_API_KEY` 读取 API 密钥（按[快速开始](quick-start.md#创建-api-密钥)中的步骤创建），并调用 `claude-haiku-4-5`。请使用项目目录中的模型。

## OpenAI SDK

在 Python 中调用 Responses API：

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

同一个客户端也能调用 Chat Completions：

```python
completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

在 TypeScript 中：

```ts
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const response = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways." });
console.log(response.output_text);
```

## Anthropic SDK

在 Python 中：

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

在 TypeScript 中：

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

## 流式传输

每种格式都支持流式传输，事件与其 SDK 预期的一致。使用上面的 Anthropic 客户端：

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

流处于静默状态时，例如模型正在思考，Kaeso 会发送 SDK 会忽略的保活消息，让沿途的代理保持连接（[长时间的流](../self-hosting/domains-and-tls.md#长时间的流)）。

## curl

OpenAI 格式：

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

Anthropic 格式：

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

每个端点都接受放在任一请求头中的密钥。[模型 API](../api/model-api.md) 列出了所有端点。

## 从浏览器调用

只有当网页的源列在 `KAESO_CORS_ALLOWED_ORIGINS` 中时，Kaeso 才会响应它的请求；而且使用该网页的每个人都能读到其中的密钥。[从浏览器调用](../api/model-api.md#从浏览器调用)对这两点都有说明。

## 再次找到请求

在项目中打开**请求日志**：每个请求都列在那里，附有状态、模型、Token、费用和耗时。每个响应都带有 `kaeso-request-id` 请求头，其 ID 与日志中显示的一致，因此可以从应用自己的日志在 Kaeso 中找到对应的请求。

失败的请求会以其 SDK 预期的格式返回错误并说明原因。[错误](../api/errors.md)列出了所有错误。
