---
title: 模型 API
description: 应用调用的端点、身份验证方式以及模型的指定方式。
order: 1
---

模型 API 支持 OpenAI 和 Anthropic 的 API 格式，因此应用可以继续使用官方 SDK。Kaeso 会将每个请求转换为模型服务商的格式，再将回复转换回来，流式传输也包括在内。

## Base URL

| SDK | Base URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

## 端点

| 方法和路径 | 格式 |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses，计算输入 Token 数 |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages，计算 Token 数 |
| `GET /v1/models` | 密钥可调用的模型 |
| `GET /v1/models/{model}` | 其中的某一个模型 |

生成类端点可以调用目录中的所有生成模型，无论其服务商是谁。`/v1/embeddings` 只调用嵌入模型，而嵌入模型只有 OpenAI 提供，且嵌入模型不服务于其他任何端点；两种混用都会以 400 拒绝。

请求带有 `anthropic-version` 头时，`GET /v1/models` 以 Anthropic 格式响应，否则以 OpenAI 格式响应。它先列出目录中密钥可调用的模型，再列出指向这些模型的项目别名。在 Anthropic 格式下，列表通过 `limit`（默认 20）、`after_id` 和 `before_id` 分页。`GET /v1/models/{model}` 接受任何[模型名称](#模型名称)，对不在目录中或密钥无权使用的模型返回 404。

`/v1` 下的其他任何路径都返回 404，以错误方法调用已知路径则返回 405。请求带有 `anthropic-version` 头时，这两种错误都采用 Anthropic 格式，否则采用 OpenAI 格式。

## 密钥

在任意端点上，通过以下任一请求头发送具有模型调用权限的 API 密钥：

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

两者都发送时，使用 `x-api-key`。密钥决定所属项目，并以其成员或服务账号的权限和模型访问设置行事，该成员或服务账号需要 `api.request` 权限。仅具有平台访问权限的密钥会以 403 被拒绝，组织平台密钥（`sk-kso-org-…`）同样如此，因为它们从不调用模型。

## 模型名称

请求通过以下三种方式之一指定模型，Kaeso 按以下顺序在项目目录中查找：

| 形式 | 示例 |
| --- | --- |
| 项目的别名 | `fast` |
| 服务商加模型 | `anthropic/claude-haiku-4-5` |
| 模型自身的 ID | `claude-haiku-4-5` |

目录之外的模型会以 404 被拒绝，密钥或其成员无权使用的模型会以 403 被拒绝。当项目中有多个服务商提供相同的模型 ID 时，单独的 ID 会以 400 被拒绝，消息中会列出这些服务商；请改为发送 `provider/model`。[模型和别名](../console/models-and-aliases.md)介绍了目录。

## 流式传输

按照所用格式的方式请求流式传输（`"stream": true`）；回复是采用应用所用格式的服务器发送事件（SSE）流。

- 如果 15 秒内没有向应用写入任何内容，Kaeso 会写入一条 SDK 会忽略的保活消息：OpenAI 格式为 SSE 注释，Anthropic 格式为 `ping` 事件。`KAESO_STREAMS_KEEPALIVE_INTERVAL` 可更改间隔或关闭保活；请参阅[发往应用的流](../self-hosting/configuration.md#发往应用的流)。
- 已开始后失败的流，或服务商在回复完成前结束的流，会以采用应用所用格式的错误事件结束。
- 应用断开连接时，Kaeso 会立即停止对服务商的请求，并按已用量计费。

## 跨工具调用的推理

Chat Completions 没有用于模型推理内容的字段。当 Chat Completions 应用调用的模型通过其他格式提供服务时（Claude，或通过 Responses 调用的 OpenAI 模型），Kaeso 会保存工具调用之前的推理内容，并在应用使用同一密钥的下一轮中将其加回。推理内容默认保存一小时，经过加密，且从不记录到日志；请参阅[推理连续性](../self-hosting/configuration.md#推理连续性)。

## 响应头

| 响应头 | 值 |
| --- | --- |
| `kaeso-request-id` | 请求的 ID，`req_…`，每个响应都带有；请求日志中的 ID 与之相同 |
| `kaeso-provider` | 处理请求的服务商，在模型确定后提供，包括错误响应 |
| `kaeso-model` | 服务商的模型 ID，在模型确定后提供，包括错误响应 |

## 从浏览器调用

只有当另一个源的 API 以 CORS 响应头应答时，浏览器才允许网页调用它。在 `KAESO_CORS_ALLOWED_ORIGINS` 列出你的 Web 应用的源（例如 `https://app.example.com,http://localhost:5173`）之前，Kaeso 不发送任何 CORS 响应头；请参阅[浏览器应用](../self-hosting/configuration.md#浏览器应用)。列出之后，仅在 `/v1` 上：

- **预检请求。** 来自已列出源、带有 `Origin` 和 `Access-Control-Request-Method` 的 `OPTIONS` 请求无需密钥，以 204 应答且不带响应体：`Access-Control-Allow-Origin` 为该源，`Access-Control-Allow-Methods: GET, POST`，`Access-Control-Max-Age: 7200`，`Access-Control-Allow-Headers` 中是所请求的、属于官方 SDK 会发送的请求头：`authorization`、`x-api-key`、`content-type`、`accept`、`user-agent`、`anthropic-version`、`anthropic-beta`、`anthropic-dangerous-direct-browser-access`、`openai-organization`、`openai-project`、`openai-beta`、`idempotency-key` 和 `x-stainless-…`。预检请求不是模型请求：不会被记录到日志、不计费，也不计入限额。
- **应答。** 对已列出源的每个应答（包括错误）都带有值为该源的 `Access-Control-Allow-Origin`，以及 `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, retry-after, retry-after-ms`，让应用可以读取这些响应头。
- **其他源**不会收到 CORS 响应头，因此浏览器会拦截调用；它们的预检请求会得到空的 204。`/v1` 上的每个应答都带有 `Vary: Origin`，让缓存区分这些应答。
- **精确匹配源。** 协议、主机和端口必须一致：`https://app.example.com` 既不允许 `http://app.example.com`，也不允许 `https://www.app.example.com`。`*` 会被拒绝，因为它会让任何网站都能用访问者持有的密钥发送请求。
- **不使用 Cookie。** Kaeso 从不发送 `Access-Control-Allow-Credentials`，因为密钥放在请求头中。Platform API、健康检查和指标从不应答跨源请求。

两个 SDK 都只有在应用传入 `dangerouslyAllowBrowser: true` 时才会在浏览器中运行。

> 使用某个页面的任何人都能读取该页面持有的密钥。请为浏览器应用单独创建一个密钥：只有推理访问权限、有效期短、只能调用所需的模型并设有严格的限额；或者由你自己的后端发放短期密钥。请参阅 [API 密钥](../console/api-keys.md)。

## Kaeso 不做的事

- **重试。** Kaeso 对每个请求只发送一次。在错误允许的情况下，SDK 会自行重试。
- **存储状态。** 要求服务商存储或继续某个响应的 Responses 请求（`store: true`、`previous_response_id`、`conversation`、`background: true`）会被拒绝；每次都请发送完整对话。
- **静默更改。** 无法传递给模型服务商的字段会被拒绝，错误中会注明该字段，而不会被丢弃。Chat Completions、Messages 和 Embeddings 请求中如果有字段名以 `header:` 开头或含有 `.`，会以 400 被拒绝，因为 Kaeso 无法携带这样的名称；Responses 会把这类字段保留在请求体中。[格式转换](../concepts/format-translation.md)列出了格式之间可以互通的内容。
