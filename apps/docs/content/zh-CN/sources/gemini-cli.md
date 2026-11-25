---
title: Gemini CLI
description: 根据 Gemini CLI 的 OpenTelemetry 事件，统计它发往 Google 的请求和它调用的 MCP 工具。
order: 3
---

Gemini CLI 会把每个模型响应和工具调用作为 OpenTelemetry 事件上报。通过 HTTP 发送到 Kaeso 来源后，它们会成为项目中的已观测请求和 MCP 工具调用，而 Gemini CLI 仍然直接调用 Google。

[添加一个](index.md#添加来源)类型为 **Gemini CLI** 的来源，然后在 `~/.gemini/settings.json` 中添加 `telemetry` 块，并填入您的安装地址和来源密钥：

```json
{
  "telemetry": {
    "enabled": true,
    "target": "local",
    "otlpProtocol": "http",
    "otlpEndpoint": "https://kaeso.example.com/ingest/otlp",
    "otlpHeaders": { "Authorization": "Bearer sk-kso-src-…" },
    "logPrompts": false
  }
}
```

运行一次 Gemini CLI，例如 `gemini -p "Say hello"`。几秒钟内，来源会显示**已连接**，请求也会出现在项目的请求日志中。

> [!NOTE]
> 在您尚未信任的文件夹中，Gemini CLI 会忽略该文件夹的 `.gemini/settings.json`，`gemini -p` 也会拒绝运行。请先信任该文件夹，或传入 `--skip-trust`。

## 在 shell 中设置

环境变量会覆盖设置文件：

```bash
export GEMINI_TELEMETRY_ENABLED=true
export GEMINI_TELEMETRY_TARGET=local
export GEMINI_TELEMETRY_OTLP_PROTOCOL=http
export GEMINI_TELEMETRY_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export GEMINI_TELEMETRY_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export GEMINI_TELEMETRY_LOG_PROMPTS=false
```

请保持协议为 `http`：Gemini CLI 默认使用 gRPC，而 Kaeso 不接受 gRPC。

## 保留提示词

Gemini CLI 默认会记录您的提示词，因此上面的设置把 `logPrompts` 设为 `false`。要保留提示词，请为来源开启**保留提示词和回复**，并把 `logPrompts` 设为 `true`。之后 Gemini CLI 会发送每个请求的完整对话及其回复，Kaeso 会连同其中的工具调用和思考一起保留。来源不保留内容时，Kaeso 会丢弃它发送的内容。

## Kaeso 读取的内容

| Gemini CLI 事件 | 在 Kaeso 中 |
| --- | --- |
| `gemini_cli.api_response` | 一个请求：模型、Token、缓存和思考 Token、工具回答的提示词 |
| `gemini_cli.api_error` | 带有状态和消息的失败请求 |
| MCP 工具的 `gemini_cli.tool_call` | 带有服务器、工具、耗时和结果的工具调用 |
| `gemini_cli.user_prompt`、`gemini_cli.api_request` | Gemini CLI 记录提示词时，提示词和请求的对话 |

Gemini CLI 的输入数包含缓存的输入，思考部分与输出分开计数；Kaeso 把思考计入输出，并把缓存分开计数。它的 `auth_type` 指明服务商：API 密钥对应 Google 的 Gemini API，Vertex AI 对应 Gemini Enterprise Agent Platform。该服务商不销售的模型（例如 Gemini CLI 连接网关时）归属其开发方。MCP 工具保留自己的名称，不含 Gemini CLI 在前面加上的 `mcp_<server>_`。Gemini CLI 还会把相同的调用作为 GenAI 事件发送，Kaeso 会忽略它们，避免重复统计。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [Antigravity](antigravity.md)：用钩子脚本统计 Antigravity 的模型调用。
