---
title: 调用模型
description: 通过 Kaeso 使用 OpenAI 和 Anthropic SDK、curl 以及流式传输调用模型。
order: 2
---

应用使用它们已有的官方 SDK 调用 Kaeso。只需更改基础 URL 和密钥，任何 SDK 都可以调用项目目录中的任何模型：OpenAI SDK 可以调用 Claude 模型，Anthropic SDK 也可以调用 OpenAI 模型。Kaeso 会转换请求和回复。

| SDK | 基础 URL | 密钥 |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `api_key`，以 `Authorization: Bearer` 发送 |
| Anthropic | `https://kaeso.example.com` | `api_key`，以 `x-api-key` 发送 |

示例使用在项目的 **API 密钥**下创建的密钥（如[快速开始](quick-start.md#创建-api-密钥)所示）和模型 `claude-haiku-4-5`。请使用项目目录中已有的模型。

## OpenAI SDK

Python 中的 Responses API：

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

同一个客户端也支持 Chat Completions：

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

流式传输适用于所有格式，并带有 SDK 所期望的事件。使用上面的 Anthropic 客户端：

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

当流暂时没有数据时，例如模型正在思考，Kaeso 会发送 SDK 会忽略的保活消息，使途经的代理保持连接打开。参见[长时间的流](../self-hosting/domains-and-tls.md#长时间的流)。

## curl

OpenAI 格式以 Bearer Token 的形式接收密钥：

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

Anthropic 格式在 `x-api-key` 中接收密钥：

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

模型 API 的每个端点都接受这两种请求头中的任意一种。[模型 API](../api/model-api.md) 列出了所有端点。

## 再次找到请求

在项目中打开**请求日志**。每个请求都列在那里，附有状态、模型、Token、费用和耗时。Kaeso 的每个响应都带有一个 `kaeso-request-id` 响应头，其中是相同的 ID，因此可以从应用自己的日志中找到对应的请求。

如果请求失败，错误会以 SDK 所期望的格式说明原因。[错误](../api/errors.md)列出了所有错误。
