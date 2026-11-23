---
title: OpenRouter
description: 通过 OpenRouter 向 OpenTelemetry Collector 的 Broadcast，统计您发往 OpenRouter 的请求及其 Token 和费用。
order: 6
---

OpenRouter 的 Broadcast 会把您发往 OpenRouter 的每个请求的链路追踪发送到您选择的目标。把 Kaeso 来源设为 OpenTelemetry Collector 目标后，每个请求都会成为项目中的已观测请求，带有模型、Token 和费用。

[添加一个](index.md#添加来源)类型为 **OpenRouter** 的来源。然后在 OpenRouter 设置的 **Broadcast** 下添加一个 **OpenTelemetry Collector** 目标，填入以下值：

```text
https://kaeso.example.com/ingest/otlp/v1/traces
```

```json
{ "Authorization": "Bearer sk-kso-src-…" }
```

第一个是端点，即链路追踪信号的完整路径；第二个是请求头。向 OpenRouter 发送一个请求，链路追踪到达后，来源会显示**已连接**。

## Kaeso 读取的内容

每条链路追踪中的 GenAI Span 都会成为一个请求：模型、OpenRouter 路由到的服务商、Token、OpenRouter 收取的费用和耗时。OpenRouter 以 `provider/model` 命名模型，例如 `anthropic/claude-sonnet-4.5`。Kaeso 认识该模型时，按项目目录或 KPMR 定价；否则保留 OpenRouter 的费用。

OpenRouter 的链路追踪不指明成员，因此每个请求都计入来源的成员或服务账号。如果希望各团队的请求分开统计，请为每个团队分配单独的 OpenRouter 密钥和来源。

## 保留提示词

OpenRouter 会把提示词和回复写入链路追踪。只有当来源开启了**保留提示词和回复**且项目保留内容时，Kaeso 才会保留它们。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [OpenTelemetry](opentelemetry.md)：Kaeso 从 GenAI Span 和日志事件中读取的内容。
