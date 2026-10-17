---
title: 模型 API
description: 应用调用的端点、身份验证方式以及模型的指定方式。
order: 1
---

模型 API 使用 OpenAI 和 Anthropic 的 API 格式，应用可以继续使用官方 SDK，只需更改 Base URL 和密钥。Kaeso 会把每个请求转换为模型所属服务商的格式，再把回复转换回来，流式回复也不例外。

| SDK | Base URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

任何格式都能调用项目目录中的任何生成模型。下面这个 Chat Completions 请求由 Claude 回答：

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H 'Authorization: Bearer sk-kso-…' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

[调用模型](../get-started/calling-models.md)展示了两种 SDK 的用法。

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

`/v1/embeddings` 只调用嵌入模型，这类模型只有 OpenAI 提供；嵌入模型也不服务其他端点。混用的请求会以 400 被拒绝。

`/v1` 下的其他路径返回 404，已知路径使用了错误的方法则返回 405。请求带有 `anthropic-version` 请求头时，这两种错误采用 Anthropic 格式，否则采用 OpenAI 格式。

### 列出模型

`GET /v1/models` 同样根据 `anthropic-version` 请求头选择格式。列表先列出该密钥可调用的目录模型，再列出项目中指向这些模型的别名。服务商不向项目的服务商密钥提供的模型不会列出（[可用性](../console/models-and-aliases.md#可用性)）。

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

两者都发送时以 `x-api-key` 为准。密钥决定所属项目，并以其成员或服务账号的权限和模型访问范围行事，该成员或服务账号需要 `api.request` 权限。仅具有平台访问权限的密钥和组织平台密钥（`sk-kso-org-…`）会以 403 被拒绝：它们从不调用模型。如何创建和限制密钥，见 [API 密钥](../console/api-keys.md)。

## 模型名称

请求可以用三种形式指定模型。Kaeso 按以下顺序在项目目录中查找：

| 形式 | 示例 |
| --- | --- |
| 项目的别名 | `fast` |
| 服务商加模型 | `anthropic/claude-haiku-4-5` |
| 模型自身的 ID | `claude-haiku-4-5` |

目录之外的模型以 404 被拒绝，密钥或其成员无权使用的模型以 403 被拒绝。如果项目中有多个服务商提供同一模型 ID，单独的 ID 会以 400 被拒绝，消息会列出这些服务商；此时请改用 `provider/model`。目录的说明见[模型和别名](../console/models-and-aliases.md)。

## 流式传输

按格式的约定请求流式回复，即 `"stream": true`；回复是采用应用所用格式的 server-sent events 流。

- **保活。** 15 秒内没有输出时，Kaeso 会写入一条 SDK 会忽略的保活消息：OpenAI 格式为 SSE 注释，Anthropic 格式为 `ping` 事件。`KAESO_STREAMS_KEEPALIVE_INTERVAL` 可更改间隔或关闭保活；参见[发往应用的流](../self-hosting/configuration.md#发往应用的流)。
- **失败。** 流一旦开始就无法再更改状态码。服务商失败，或在回复完成前结束流时，流会以采用应用所用格式的错误事件结束。
- **断开。** 应用断开连接时，Kaeso 会立即停止向服务商发出的请求，并按已用量计费。

## 跨工具调用的推理

Chat Completions 没有存放模型推理内容的字段。当 Chat Completions 应用调用通过其他格式提供服务的模型（例如 Claude，或经 Responses 调用的 OpenAI 模型）时，Kaeso 会保存工具调用之前的推理内容，并在应用用同一密钥发来下一轮时将其加回。推理内容默认保存一小时，经过加密，且从不记录到日志；参见[推理连续性](../self-hosting/configuration.md#推理连续性)。

## 响应头

| 响应头 | 值 |
| --- | --- |
| `kaeso-request-id` | 请求的 ID，`req_…`，每个响应都有；请求日志使用同一 ID |
| `kaeso-provider` | 处理该请求的服务商，例如 `anthropic`；确定模型后即带有，错误响应也包括在内 |
| `kaeso-model` | 服务商的模型 ID；确定模型后即带有，错误响应也包括在内 |

## 从浏览器调用

只有当另一个源以 CORS 响应头应答时，浏览器才允许网页调用它。在 `KAESO_CORS_ALLOWED_ORIGINS` 列出你的 Web 应用的源（例如 `https://app.example.com,http://localhost:5173`）之前，Kaeso 不发送任何 CORS 响应头；参见[浏览器应用](../self-hosting/configuration.md#浏览器应用)。列出之后，以下规则仅适用于 `/v1`：

- **精确匹配源。** 协议、主机和端口都必须一致：`https://app.example.com` 既不放行 `http://app.example.com`，也不放行 `https://www.app.example.com`。`*` 会在启动时被拒绝，因为它会让任何网站都能用访问者持有的密钥发送请求。
- **预检请求。** 来自已列出源、带有 `Origin` 和 `Access-Control-Request-Method` 的 `OPTIONS` 请求无需密钥即返回 204：`Access-Control-Allow-Origin` 为该源，`Access-Control-Allow-Methods: GET, POST`，`Access-Control-Max-Age: 7200`，`Access-Control-Allow-Headers` 中则是所请求的请求头里官方 SDK 会发送的那些：`authorization`、`x-api-key`、`content-type`、`accept`、`user-agent`、`anthropic-version`、`anthropic-beta`、`anthropic-dangerous-direct-browser-access`、`openai-organization`、`openai-project`、`openai-beta`、`idempotency-key` 和 `x-stainless-…`。预检请求不是模型请求：不记录日志、不计费，也不计入限额。
- **响应。** 发往已列出源的每个响应（包括错误响应）都带有值为该源的 `Access-Control-Allow-Origin`，以及 `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, retry-after, retry-after-ms`，以便应用读取这些响应头。
- **其他源**得不到 CORS 响应头，因此浏览器会拦截调用；它们的预检请求得到空的 204。`/v1` 上的每个响应都带有 `Vary: Origin`，以便缓存区分这些响应。
- **不使用 Cookie。** 由于密钥通过请求头传递，Kaeso 从不发送 `Access-Control-Allow-Credentials`。Platform API、健康检查和指标从不响应跨源请求。

两种 SDK 都只有在应用传入 `dangerouslyAllowBrowser: true` 时才会在浏览器中运行。

> [!WARNING]
> 使用页面的每个人都能读取页面中的密钥。请为浏览器应用单独创建密钥：仅具有模型调用权限、有效期短、只开放所需模型、限额从严；或者由你自己的后端发放短期密钥。参见 [API 密钥](../console/api-keys.md)。

## Kaeso 不做的事

- **重试。** Kaeso 每个请求只发送一次。SDK 会在错误允许时自行重试。
- **存储状态。** 要求服务商存储或续接响应的 Responses 请求（`store: true`、`previous_response_id`、`conversation`、`background: true`）会被拒绝；每次请发送完整对话。
- **悄然更改。** 无法传递给模型服务商的字段会被拒绝，错误会指出该字段，绝不会被悄悄丢弃。Chat Completions、Messages 和 Embeddings 请求中，名称以 `header:` 开头或包含 `.` 的字段会以 400 被拒绝，因为 Kaeso 无法传递这样的名称；Responses 则会把这类字段保留在请求正文中。格式之间能传递哪些内容，见[格式转换](../concepts/format-translation.md)。
