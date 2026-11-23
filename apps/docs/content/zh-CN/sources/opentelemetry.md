---
title: OpenTelemetry
description: 统计任何导出 OpenTelemetry GenAI Span 或事件的应用或 Collector 的请求和 MCP 工具调用。
order: 7
---

许多 AI SDK 和代理框架会按照 GenAI 语义约定，把每次模型调用描述为一个 OpenTelemetry Span。导出到 Kaeso 来源后，每次调用都会成为项目中的已观测请求，每次 MCP 工具调用都会成为工具调用。

[添加一个](index.md#添加来源)类型为 **OpenTelemetry** 的来源，然后把应用的 OTLP 导出器指向它，并填入您的安装地址和来源密钥：

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer%20sk-kso-src-…"
export OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental
```

请求头中的空格写作 `%20`，这是 OpenTelemetry SDK 读取它的方式。运行您的应用，它的下一次模型调用就会出现在项目的请求日志中。

## 通过 Collector 发送

OpenTelemetry Collector 可以用 `otlphttp` 导出器转发应用发送给它的内容：

```yaml
receivers:
  otlp:
    protocols:
      http:
exporters:
  otlphttp/kaeso:
    endpoint: https://kaeso.example.com/ingest/otlp
    headers:
      Authorization: Bearer sk-kso-src-…
service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [otlphttp/kaeso]
    logs:
      receivers: [otlp]
      exporters: [otlphttp/kaeso]
```

如果 Collector 已有管道，请把 `otlphttp/kaeso` 加入这些管道的导出器。Kaeso 接受基于 HTTP 的 OTLP（protobuf 或 JSON），不接受 gRPC。

## Kaeso 读取的内容

当 Span 或日志事件的 `gen_ai.operation.name` 为 `chat`、`text_completion`、`generate_content` 或 `embeddings` 时，它就是一个请求。Kaeso 读取其模型、服务商、Token、费用、状态和时间：

| 属性 | 在 Kaeso 中 |
| --- | --- |
| `gen_ai.request.model`、`gen_ai.response.model` | 模型 |
| `gen_ai.provider.name` 或 `gen_ai.system` | 服务商 |
| `gen_ai.usage.input_tokens`、`gen_ai.usage.output_tokens` | Token，缓存分开计数 |
| `gen_ai.usage.cache_read.input_tokens`、`gen_ai.usage.cache_write.input_tokens`、`gen_ai.usage.reasoning.output_tokens` | 缓存读取和写入，以及推理 |
| `gen_ai.response.id` | 请求的 ID，因此重复发送的调用只计一次 |
| `error.type`、`http.response.status_code` | 是否失败 |
| `user.email`，或带有邮箱地址的 `enduser.id` | 请求计入的成员 |

`gen_ai.operation.name` 为 `execute_tool` 且 `mcp.method.name` 为 `tools/call` 的 Span 是一次 MCP 工具调用，以 `gen_ai.tool.name` 和 `server.address` 命名。汇总其他调用的 Span（例如 `invoke_agent`）不计数，因此代理的调用只计一次。同一次调用既有 Span 又有日志事件时，两者指向同一个 Span，只计一次。

## 保留提示词

GenAI 约定把提示词和回复放在 `gen_ai.input.messages` 和 `gen_ai.output.messages` 中，大多数 SDK 只有在您开启后才会发送它们。只有当来源开启了**保留提示词和回复**且项目保留内容时，Kaeso 才会保留它们。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [HTTP](http.md)：改为以 JSON 发送请求和工具调用。
