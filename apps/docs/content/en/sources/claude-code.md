---
title: Claude Code
description: Count every request Claude Code sends to Anthropic and every MCP tool it calls, without changing where it sends them.
order: 1
---

Claude Code reports each model request and tool call as an OpenTelemetry event. Pointed at a Kaeso source, those events become observed requests and MCP tool calls in the project, with their tokens and cost, while Claude Code keeps calling Anthropic directly.

[Add a source](index.md#add-a-source) of the kind **Claude Code**, then add its environment variables to `~/.claude/settings.json`, with your install's address and the source's key:

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

Start Claude Code and send it a prompt. Within a few seconds the source shows **Connected**, and the request appears in the project's request logs. Claude Code sends its events every few seconds and when it exits.

## Set it in the shell instead

The same variables work as environment variables, for one shell or a CI job:

```bash
export CLAUDE_CODE_ENABLE_TELEMETRY=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_METRICS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

Keep `OTEL_EXPORTER_OTLP_PROTOCOL`: some Claude Code versions export nothing without it. A team can set the variables in Claude Code's managed settings. Every member's Claude Code then reports to the same source, and each request counts for the member it names.

## Name the MCP tools

`OTEL_LOG_TOOL_DETAILS=1` makes Claude Code name the MCP server and tool of each call, such as `linear` and `list_issues`. Without it, Kaeso still logs each MCP call, but as `mcp_tool`, with no server or tool name. Calls to Claude Code's own tools, such as reading files or running commands, aren't kept.

## Keep the prompts

Claude Code leaves your prompts out of its events. To keep them, turn on **Keep prompts and replies** for the source and add `OTEL_LOG_USER_PROMPTS=1`. With `OTEL_LOG_TOOL_DETAILS=1`, the tool calls also carry their arguments.

## What Kaeso reads

| Claude Code event | In Kaeso |
| --- | --- |
| `api_request` | A request: model, tokens, cache reads and writes, time to the first token, Anthropic's request id, the cost Claude Code computed |
| `api_error` | A failed request with its status and message |
| `tool_result` of an MCP tool | A tool call with its server, tool, duration and outcome |

Kaeso prices each request from the project's catalogue or KPMR, and uses Claude Code's own cost only for a model neither knows. Claude Code's metrics only keep the source's status current, and the spans of its enhanced telemetry are left out, because its events already report the same requests.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [Request logs](../console/request-logs.md): Find a request and see what was sent and returned.
