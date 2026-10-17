---
title: 错误
description: 模型请求失败时在各 API 格式下返回的内容。
order: 2
---

失败的模型请求会收到一个 HTTP 状态码和一个采用应用所调用格式的错误正文，因此 SDK 会抛出其惯常的错误。消息会说明出了什么问题以及应如何处理。

## OpenAI 格式

Chat Completions、Responses 和 Embeddings 使用同一种错误结构：

```json
{
  "error": {
    "message": "The model `gpt-9` is not in this project's model catalogue. A project admin can add it under the project's models.",
    "type": "invalid_request_error",
    "param": "model",
    "code": null
  }
}
```

| 状态码 | `type` | `code` |
| --- | --- | --- |
| 400 | `invalid_request_error` | 适用时为 `context_length_exceeded` 或 `content_policy_violation` |
| 401 | `invalid_request_error` | |
| 403 | `request_forbidden` | |
| 404 | `invalid_request_error` | |
| 405 | `invalid_request_error` | |
| 413 | `invalid_request_error` | |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 429 | `insufficient_quota` | `insufficient_quota`，服务商因计费原因拒绝时 |
| 500, 502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

`Retry-After` 以整秒为单位，向上取整；当等待时间含有不足一秒的部分时，`retry-after-ms` 以毫秒给出。OpenAI 的错误结构没有请求 ID 字段；该 ID 位于 `kaeso-request-id` 响应头中。

## Anthropic 格式

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit reached: this key's request rate of 60 requests per minute; retry in 12 seconds."
  },
  "request_id": "req_…"
}
```

`request_id` 是请求的 ID，与 `kaeso-request-id` 中的相同，服务商自身的错误也是如此。

| 状态码 | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 402 | `billing_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| 其他 | `api_error` |

## 服务商错误

当应用和服务商使用相同格式时，服务商的错误会按服务商发送的原样返回，状态码不变；Anthropic 错误在 `request_id` 中带有 Kaeso 的请求 ID。否则，错误会在应用所用格式中保持其含义：服务商拒绝的服务商密钥返回 401 或 403，服务商自身的速率限制返回 429，其计费拒绝返回 402 或 429，其他服务器错误返回各自的 5xx 状态码。请求日志会保存服务商自身的消息。

有一种服务商错误在所有格式中都使用 Kaeso 自己的消息：当服务商对目录中的某个模型返回 404 时，Kaeso 会用同一个服务商密钥询问服务商的模型端点（`GET /v1/models/{model}`）是否提供该模型。如果不提供（模型已停用，或密钥无权访问），应用会收到 404，带有 `param: model`，消息会指出服务商和模型并说明该怎么做：

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "Anthropic does not serve the model `claude-sonnet-4-5` to this project's provider key; check the key's access to it, or remove the model from the catalogue."
  }
}
```

随后 Kaeso 会在目录中将该模型标记为**不可用**（[可用性](../console/models-and-aliases.md#可用性)）。如果服务商确实提供该模型，说明这个 404 与其他内容有关（例如文件 ID），会按服务商发送的原样返回。Kaeso 从不会重发请求，也不会把请求发给其他模型。

## 请求失败的原因

| 状态码 | 原因 |
| --- | --- |
| 400 | 请求无法读取，模型不接受请求使用的某项设置，某个字段无法传递给服务商，上下文过长，项目中有多个服务商提供同一模型 ID，或尚未为模型的服务商设置服务商密钥 |
| 401 | 没有密钥，或密钥未知、已停用、已归档或已过期，其成员或服务账号不再有效，或其项目或组织无法使用 |
| 403 | 密钥仅具有平台访问权限、是组织平台密钥或无权调用模型（`api.request`），或密钥或其成员无权使用此模型 |
| 404 | 模型不在项目目录中，服务商不向项目的服务商密钥提供该模型，或该路径不提供服务 |
| 405 | 该路径不接受此方法 |
| 413 | 正文大于 `KAESO_BODY_LIMITS_MODEL_API`，默认为 32 MiB |
| 429 | 速率限制或预算，或来自同一地址的未知密钥过多；`Retry-After` 说明何时重试，但预算用尽时除外，此时由消息说明何时重置 |
| 500 | 服务商密钥无法解密，或模型的服务商在此部署中不可用；对于 OpenAI 格式的应用，还包括下方 503 的原因 |
| 502 | 无法连接服务商，或传输失败 |
| 503 | 对于 Anthropic 格式的应用：无法加载目录、API 密钥或服务商密钥，因 Valkey 不可用而无法检查硬性预算，或网关正在重启；对于 OpenAI 格式的应用：服务商过载 |
| 504 | 服务商未及时响应 |
| 529 | 服务商过载，适用于 Anthropic 格式的应用 |

已开始的流无法更改其状态码：当服务商失败，或在回复完成前结束流时，流会改为以采用应用所用格式的错误事件结束。

每个响应都带有 `kaeso-request-id`。请求日志中的 ID 与之相同，并在有服务商消息时包含该消息。
