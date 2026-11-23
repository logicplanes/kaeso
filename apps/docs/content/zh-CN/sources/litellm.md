---
title: LiteLLM
description: 根据 LiteLLM 代理的通用 API 日志记录器，统计它处理的请求及其 Token 和费用。
order: 5
---

LiteLLM 代理可以把它处理的每个请求的日志发送到任意 HTTP 端点。发送到 Kaeso 来源后，每条日志都会成为项目中的已观测请求，带有模型、服务商、Token 和费用。

[添加一个](index.md#添加来源)类型为 **LiteLLM** 的来源，然后在代理的 `config.yaml` 中添加 `generic_api` 回调，并填入您的安装地址和来源密钥：

```yaml
litellm_settings:
  callbacks: ["kaeso"]
callback_settings:
  kaeso:
    callback_type: generic_api
    endpoint: https://kaeso.example.com/ingest/v1/litellm
    headers:
      Authorization: Bearer sk-kso-src-…
```

重启代理并向它发送一个请求。LiteLLM 每隔几秒批量发送日志；之后来源会显示**已连接**，请求也会出现在项目的请求日志中。

## 保持批次较小

LiteLLM 每批最多发送 512 条日志，每条都包含请求的消息和回复。Kaeso 每批最多接受 8 MiB，因此如果代理的请求较长，请在代理的环境中设置 `DEFAULT_BATCH_SIZE=50`。被拒绝的批次 LiteLLM 不会重新发送。

## 请求属于谁

每条日志都会指明调用方使用的 LiteLLM 密钥，以及 LiteLLM 已知时该密钥用户的邮箱地址。来源按邮箱匹配成员时，请求计入拥有该地址的成员；否则计入来源的成员或服务账号。请求自身的 LiteLLM 标签会成为它在 Kaeso 中的标签。

## 保留提示词

LiteLLM 的日志总是包含消息和回复。只有当来源开启了**保留提示词和回复**且项目保留内容时，Kaeso 才会保留它们。

## Kaeso 读取的内容

| LiteLLM 字段 | 在 Kaeso 中 |
| --- | --- |
| `id` | 请求的 ID，因此重复发送的日志只计一次 |
| `model`、`custom_llm_provider` | 模型和服务商 |
| `prompt_tokens`、`completion_tokens` 和服务商的用量 | Token，包括缓存读取、缓存写入和推理 |
| `status`、`error_str` | 是否失败以及原因 |
| `startTime`、`endTime`、`completionStartTime` | 运行时间和首 Token 时间 |
| `response_cost` | Kaeso 没有该模型价格时使用的费用 |

LiteLLM 的 MCP 工具调用会成为项目中的工具调用。LiteLLM 也可以改为发送 OpenTelemetry：类型为 [OpenTelemetry](opentelemetry.md) 的来源会读取它的 GenAI Span。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [OpenRouter](openrouter.md)：通过 Broadcast 统计您的 OpenRouter 请求。
