---
title: Codex
description: 根据 Codex CLI 的 OpenTelemetry 日志，统计它发往 OpenAI 的请求和它调用的 MCP 工具。
order: 2
---

Codex CLI 会把每个响应和工具调用记录为 OpenTelemetry 事件。发送到 Kaeso 来源后，它们会成为项目中的已观测请求和 MCP 工具调用，而 Codex 仍然直接调用 OpenAI。

[添加一个](index.md#添加来源)类型为 **Codex** 的来源，然后在 `~/.codex/config.toml` 中添加 `[otel]` 表，并填入您的安装地址和来源密钥：

```toml
[otel]
log_user_prompt = false
exporter = { otlp-http = { endpoint = "https://kaeso.example.com/ingest/otlp/v1/logs", protocol = "binary", headers = { authorization = "Bearer sk-kso-src-…" } } }
```

运行一次 Codex，例如 `codex exec "Say hello"`。几秒钟内，来源会显示**已连接**，请求也会出现在项目的请求日志中。

## 设置放在哪里

Codex 只从您自己的 `~/.codex/config.toml` 读取 `[otel]`，不读取项目的 `.codex/config.toml`。如果文件中已有 `[otel]` 表，请把这两个键加到其中，而不是再添加一个表。端点是日志信号的完整路径 `/ingest/otlp/v1/logs`，`protocol = "json"` 也可以使用。

## 保留提示词

Codex 的事件默认不含您的提示词。要保留它们，请为来源开启**保留提示词和回复**，并设置 `log_user_prompt = true`。Codex 总会发送每个工具调用的参数和输出的简短预览，只有来源保留内容时 Kaeso 才会保留它们。

## Kaeso 读取的内容

| Codex 事件 | 在 Kaeso 中 |
| --- | --- |
| 已完成响应的 `codex.sse_event` | 一个请求：模型、Token、缓存和推理 Token、响应开始前的时间 |
| 失败的 `codex.api_request` 或失败的响应 | 带有状态和消息的失败请求 |
| MCP 工具的 `codex.tool_result` | 带有服务器、工具、耗时和结果的工具调用 |

Codex 的输入数包含缓存的输入，输出数包含推理；Kaeso 把它们分开计数。Codex 自带工具（例如它的 shell）的调用不予保留。Codex 的响应不带 OpenAI 的请求 ID，因此 Kaeso 按会话和事件时间区分它们。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [Claude Code](claude-code.md)：统计 Claude Code 的请求和 MCP 工具调用。
