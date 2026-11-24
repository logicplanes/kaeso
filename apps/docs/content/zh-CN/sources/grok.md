---
title: Grok Build
description: 根据 Grok Build 的 OpenTelemetry 事件，统计它发往 xAI 的请求及其 Token 和费用，以及它调用的 MCP 工具。
order: 5
---

Grok Build 是 xAI 的编程代理，可以把每个模型请求和工具调用的 OpenTelemetry 事件发送到您自己的收集器。发送到 Kaeso 来源后，它们会成为项目中的已观测请求和 MCP 工具调用，而 Grok Build 仍然直接调用 xAI。

[添加一个](index.md#添加来源)类型为 **Grok Build** 的来源，然后在启动 `grok` 的 shell 中设置这些变量，并填入您的安装地址和来源密钥：

```sh
export GROK_EXTERNAL_OTEL=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

运行 `grok` 并发送一个提示词。几秒钟内（如果 Grok Build 启动时无法连接 xAI，则最多 30 秒），来源会显示**已连接**，请求也会出现在项目的请求日志中。

## 设置放在哪里

只有同时设置了 `GROK_EXTERNAL_OTEL=1` 和一个导出器（这里是 `OTEL_LOGS_EXPORTER=otlp`），Grok Build 才会发送内容。它只从环境变量读取收集器的请求头，从不读取配置文件，因此至少要在 shell 中设置 `OTEL_EXPORTER_OTLP_HEADERS`。请保持协议为 `http/protobuf`：Kaeso 不接受 gRPC。指标没有任何用处，因此不要设置 `OTEL_METRICS_EXPORTER`。

设置 `OTEL_LOG_TOOL_DETAILS=1` 后，Grok Build 会指明它调用的 MCP 服务器和工具；不设置时，每次 MCP 调用都记为 `mcp_tool`，没有服务器名称。

## 保留提示词

Grok Build 的事件默认不含您的提示词、它的回复和工具调用的内容。要保留它们，请为来源开启**保留提示词和回复**，并添加两个变量：

```sh
export OTEL_LOG_USER_PROMPTS=1
export OTEL_LOG_TOOL_CONTENT=1
```

第一个变量发送您的提示词和 Grok Build 的回复，第二个发送每个工具调用的输入和输出。如果设置了 `OTEL_LOG_ASSISTANT_RESPONSES=0`，或托管配置将其关闭，则不会发送回复。来源不保留内容时，Kaeso 会丢弃它发送的内容。

## Kaeso 读取的内容

| Grok Build 事件 | 在 Kaeso 中 |
| --- | --- |
| `grok_code.api_request` | 一个请求：模型、含缓存读取和写入的 Token、推理、xAI 报告的费用 |
| `grok_code.api_error` | 失败的请求；速率限制记为 429 |
| `grok_code.user_prompt`、`grok_code.assistant_response` | 该轮的提示词随它的第一个请求保存，回复随它的最后一个请求保存 |
| MCP 工具的 `grok_code.tool_result` | 带有服务器、工具、耗时和结果的工具调用 |

Grok Build 把整个提示词计为输入（包括缓存的 Token），推理与输出分开计数；Kaeso 把它们分开计数。它的请求属于 xAI，按项目目录或 KPMR 定价，两者都没有时使用 xAI 报告的费用。当您使用带有邮箱地址的账号登录时，每个事件都会带上该地址，因此按邮箱匹配成员的来源会把请求计入该成员。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [Claude Code](claude-code.md)：统计 Claude Code 的请求和 MCP 工具调用。
