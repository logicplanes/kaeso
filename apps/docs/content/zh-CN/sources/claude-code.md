---
title: Claude Code
description: 统计 Claude Code 发往 Anthropic 的每个请求和它调用的每个 MCP 工具，而不改变它的请求去向。
order: 1
---

Claude Code 会把每个模型请求和工具调用作为 OpenTelemetry 事件上报。把这些事件指向 Kaeso 来源后，它们会成为项目中带有 Token 和费用的已观测请求和 MCP 工具调用，而 Claude Code 仍然直接调用 Anthropic。

[添加一个](index.md#添加来源)类型为 **Claude Code** 的来源，然后把它的环境变量加入 `~/.claude/settings.json`，并填入您的安装地址和来源密钥：

```json
{
  "env": {
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_LOGS_EXPORTER": "otlp",
    "OTEL_METRICS_EXPORTER": "otlp",
    "OTEL_EXPORTER_OTLP_PROTOCOL": "http/protobuf",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "https://kaeso.example.com/ingest/otlp",
    "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer sk-kso-src-…",
    "OTEL_LOG_TOOL_DETAILS": "1"
  }
}
```

启动 Claude Code 并发送一个提示词。几秒钟内，来源会显示**已连接**，请求也会出现在项目的请求日志中。Claude Code 每隔几秒以及退出时发送一次事件。

## 在 shell 中设置

同样的变量也可以作为环境变量使用，适用于单个 shell 或 CI 任务：

```bash
export CLAUDE_CODE_ENABLE_TELEMETRY=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_METRICS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

请保留 `OTEL_EXPORTER_OTLP_PROTOCOL`：某些 Claude Code 版本没有它就什么也不导出。团队可以在 Claude Code 的托管设置中设置这些变量。这样每个成员的 Claude Code 都会上报到同一个来源，每个请求计入它所指明的成员。

## 指明 MCP 工具

设置 `OTEL_LOG_TOOL_DETAILS=1` 后，Claude Code 会指明每次调用的 MCP 服务器和工具，例如 `linear` 和 `list_issues`。不设置时，Kaeso 仍会记录每次 MCP 调用，但记为 `mcp_tool`，没有服务器和工具名称。Claude Code 自带工具的调用（例如读取文件或运行命令）不予保留。

## 保留提示词

Claude Code 的事件默认不含您的提示词。要保留它们，请为来源开启**保留提示词和回复**，并添加 `OTEL_LOG_USER_PROMPTS=1`。设置 `OTEL_LOG_TOOL_DETAILS=1` 后，工具调用也会附带参数。

## Kaeso 读取的内容

| Claude Code 事件 | 在 Kaeso 中 |
| --- | --- |
| `api_request` | 一个请求：模型、Token、缓存读取和写入、首 Token 时间、推理强度级别、Anthropic 的请求 ID、Claude Code 计算的费用 |
| `api_error` | 带有状态和消息的失败请求 |
| MCP 工具的 `tool_result` | 带有服务器、工具、耗时和结果的工具调用 |

Kaeso 根据项目目录或 KPMR 为每个请求定价，只有两者都不认识的模型才使用 Claude Code 自己计算的费用。Claude Code 的指标只用于保持来源状态最新，其增强遥测的 Span 不予采用，因为它的事件已经上报了相同的请求。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [请求日志](../console/request-logs.md)：查找请求，查看发送和返回的内容。
