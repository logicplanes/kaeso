---
title: 模型 API
description: 应用调用的端点、身份验证方式以及模型的指定方式。
order: 1
---

应用用 OpenAI 和 Anthropic 的官方 SDK 调用模型 API：把 SDK 指向您的实例，并传入一个 Kaeso API 密钥。Kaeso 会把每个请求转换为模型所属服务商的格式，再把回复转换回来，流式回复也不例外。

使用 OpenAI SDK 时，下面这个 Chat Completions 请求由 Claude 回答：

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

使用 Anthropic SDK 时，以 Messages 格式发送同一个请求：

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

| SDK | 基础 URL | 密钥的发送方式 |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

任何格式都能调用项目目录中的任何生成模型。[调用模型](../get-started/calling-models.md)展示了流式传输以及两种 SDK 的更多用法。

## 端点

| 方法和路径 | 格式 |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses，统计输入 Token |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages，统计 Token |
| `GET /v1/models` | 该密钥可调用的模型 |
| `GET /v1/models/{model}` | 其中某一个模型 |

`/v1/embeddings` 只服务嵌入模型，这类模型只有部分[服务商](../concepts/supported-providers.md)提供；嵌入模型也不服务其他端点。混用两者的请求，Kaeso 返回 400。

`/v1` 下的其他路径返回 404，已知路径使用了错误的方法则返回 405。请求带有 `anthropic-version` 请求头时，这两种错误采用 Anthropic 格式，否则采用 OpenAI 格式。

### 列出模型

`GET /v1/models` 同样根据 `anthropic-version` 请求头选择格式。列表先列出该密钥可调用的目录模型，再列出项目中指向这些模型的别名。每个模型以其自身的 ID 列出；如果其自身的 ID 不能指向它（例如另一个服务商提供同一 ID），则以 `provider/model` 列出（[模型名称](#模型名称)）。服务商不向项目的服务商密钥提供的模型不会列出（[可用性](../console/models-and-aliases.md#可用性)）。

| | OpenAI 格式 | Anthropic 格式 |
| --- | --- | --- |
| 分页 | 无，一次响应包含所有模型 | `limit`（1 到 1000，默认 20）、`after_id` 或 `before_id` |
| 已弃用的模型 | 不标注 | 按 models.dev 的标注带有 `lifecycle: "deprecated"` |

`GET /v1/models/{model}` 接受任何[模型名称](#模型名称)。模型不在目录中、密钥无权使用或服务商不提供该模型时，返回 404。

## 密钥

在任一端点上，通过以下任一请求头发送具有模型调用权限的 API 密钥：

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

请求同时发送两者时，以 `x-api-key` 为准。密钥决定所属项目。它以其成员或服务账号的权限和模型访问范围行事，该成员或服务账号需要 `api.request` 权限。

仅具有平台访问权限的密钥和组织平台密钥（`sk-kso-org-…`），Kaeso 以 403 拒绝：它们从不调用模型。如何创建和限制密钥，见 [API 密钥](../console/api-keys.md)。

## 模型名称

请求可以用三种形式指定模型。Kaeso 按以下顺序在项目目录中查找：

| 形式 | 示例 |
| --- | --- |
| 项目的别名 | `fast` |
| 服务商加模型 | `anthropic/claude-haiku-4-5` |
| 模型自身的 ID | `claude-haiku-4-5` |

目录之外的模型，以及密钥或其成员无权使用的模型，Kaeso 都返回 404。两者的消息相同，这样密钥无法得知项目还有哪些其他模型。如果项目中有多个服务商提供同一模型 ID，单独的 ID 会得到 400，消息会列出这些服务商；此时请改用 `provider/model`。目录的说明见[模型和别名](../console/models-and-aliases.md)。

## 流式传输

按格式的约定请求流式回复，即 `"stream": true`。回复是采用应用所用格式的 SSE（服务器发送事件）流。

- **保活。** 15 秒内没有输出时，Kaeso 会写入一条 SDK 会忽略的保活消息：OpenAI 格式为 SSE 注释，Anthropic 格式为 `ping` 事件。`KAESO_STREAMS_KEEPALIVE_INTERVAL` 可更改间隔或关闭保活（[发往应用的流](../self-hosting/configuration.md#发往应用的流)）。
- **失败。** 流一旦开始就无法再更改状态码。服务商出错，或在回复完成前结束流时，流会以采用应用所用格式的错误事件结束。
- **断开。** 应用在流的最后一个事件之前断开连接时，Kaeso 会立即停止向服务商发出的请求，并按已用量计费。最后一个事件已到达应用的流视为已完成，即使应用随即断开连接也是如此。

## 跨工具调用的推理

Chat Completions 没有存放模型推理内容的字段。Chat Completions 应用调用通过其他格式提供服务的模型时，Kaeso 会保存模型在每次工具调用之前的推理内容。在应用的下一轮请求中，Kaeso 会将这些推理内容加回（[格式转换](../concepts/format-translation.md#chat-completions)）。

## 响应头

| 响应头 | 值 |
| --- | --- |
| `kaeso-request-id` | 请求的 ID，`req_…`，每个响应都有。请求日志显示同一 ID |
| `kaeso-provider` | 处理该请求的服务商，例如 `anthropic`；确定模型后即带有，错误响应也包括在内 |
| `kaeso-model` | 服务商的模型 ID；确定模型后即带有，错误响应也包括在内 |
| `kaeso-left-out` | Kaeso 因模型没有对应项而省略的内容，按请求中的名称列出，例如 `tools[1], text.verbosity`；对话中的项按类型和 ID 列出，例如 `web_search_call ws_67c9…`。只出现在省略了内容的响应上（[省略的工具](../development/kpp/cross-format.md#省略的工具)） |
| `server-timing` | `kaeso;dur=…`：Kaeso 调用服务商之前自身花费的毫秒数；请求未到达服务商时，则是到 Kaeso 作出响应为止的时间。随后是 `upstream;dur=…`：服务商返回第一个字节所用的时间，得知后即带有。流式回复随响应头一起发送它，因此 Kaeso 转换流所花的时间只记录在[请求日志](../console/request-logs.md#查看请求)中 |

## 从浏览器调用

只有当另一个源以 CORS 响应头应答时，浏览器才允许网页调用它。在 `KAESO_CORS_ALLOWED_ORIGINS` 列出您的 Web 应用的源（例如 `https://app.example.com,http://localhost:5173`）之前，Kaeso 不发送任何 CORS 响应头（[浏览器应用](../self-hosting/configuration.md#浏览器应用)）。列出之后，以下规则仅适用于 `/v1`：

- **精确匹配源。** 协议、主机和端口都必须一致：`https://app.example.com` 既不放行 `http://app.example.com`，也不放行 `https://www.app.example.com`。Kaeso 会在启动时拒绝 `*`，因为它会让任何网站都能用访问者持有的密钥发送请求。
- **预检请求。** 来自已列出源、带有 `Origin` 和 `Access-Control-Request-Method` 的 `OPTIONS` 请求无需密钥即返回 204。响应带有值为该源的 `Access-Control-Allow-Origin`、`Access-Control-Allow-Methods: GET, POST` 和 `Access-Control-Max-Age: 7200`。`Access-Control-Allow-Headers` 列出所请求的请求头里官方 SDK 会发送的那些：`authorization`、`x-api-key`、`content-type`、`accept`、`user-agent`、`anthropic-version`、`anthropic-beta`、`anthropic-dangerous-direct-browser-access`、`openai-organization`、`openai-project`、`openai-beta`、`idempotency-key` 和 `x-stainless-…`。预检请求不是模型请求：不记录日志、不计费，也不计入限额。
- **响应。** 发往已列出源的每个响应（包括错误响应）都带有值为该源的 `Access-Control-Allow-Origin`，以及 `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, kaeso-left-out, retry-after, retry-after-ms`，以便应用读取这些响应头。
- **其他源**得不到 CORS 响应头，因此浏览器会拦截调用，它们的预检请求得到空的 204。`/v1` 上的每个响应都带有 `Vary: Origin`，以便缓存区分这些响应。
- **不使用 Cookie。** 由于密钥通过请求头传递，Kaeso 从不发送 `Access-Control-Allow-Credentials`。Platform API、健康检查和指标从不响应跨源请求。

两种 SDK 都只有在应用传入 `dangerouslyAllowBrowser: true` 时才会在浏览器中运行。

> [!WARNING]
> 使用页面的每个人都能读取页面中的密钥。请为浏览器应用单独创建密钥：仅具有模型调用权限、有效期短、只开放所需模型、限额从严。或者由您自己的后端发放短期密钥（[API 密钥](../console/api-keys.md)）。

## Kaeso 不做的事

- **重试。** Kaeso 每个请求只发送一次。SDK 会在错误允许时自行重试。
- **存储状态。** 要求服务商存储或续接响应的 Responses 请求（`store: true`、`previous_response_id`、`conversation`、`background: true`），Kaeso 会拒绝。请每次发送完整对话。
- **悄然更改。** 无法传递给模型服务商的字段，Kaeso 会拒绝并在错误中指出该字段，绝不会悄悄丢弃。在 Chat Completions、Messages 和 Embeddings 中，名称以 `header:` 开头或包含 `.` 的字段会得到 400，因为 Kaeso 无法传递这样的名称；Responses 则会把这类字段保留在请求正文中。格式之间能传递哪些内容，见[格式转换](../concepts/format-translation.md)。
