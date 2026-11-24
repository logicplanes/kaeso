---
title: Grok Build
description: Count the requests Grok Build sends to xAI, with their tokens and cost, and the MCP tools it calls, from its OpenTelemetry events.
order: 5
---

Grok Build, xAI's coding agent, can send OpenTelemetry events about each model request and tool call to a collector of your own. Sent to a Kaeso source, they become observed requests and MCP tool calls in the project, while Grok Build keeps calling xAI directly.

[Add a source](index.md#add-a-source) of the kind **Grok Build**, then set these variables in the shell you start `grok` from, with your install's address and the source's key:

```sh
export GROK_EXTERNAL_OTEL=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

Run `grok` and send it a prompt. The source shows **Connected** within a few seconds, or up to 30 seconds when Grok Build can't reach xAI as it starts, and the request appears in the project's request logs.

## Where the settings go

Grok Build sends nothing until both `GROK_EXTERNAL_OTEL=1` and an exporter, here `OTEL_LOGS_EXPORTER=otlp`, are set. It takes the collector's headers only from the environment, never from its config file, so set at least `OTEL_EXPORTER_OTLP_HEADERS` in the shell. Keep the protocol at `http/protobuf`: Kaeso doesn't take gRPC. Metrics add nothing, so leave `OTEL_METRICS_EXPORTER` unset.

`OTEL_LOG_TOOL_DETAILS=1` lets Grok Build name the MCP servers and tools it calls; without it, every MCP call is `mcp_tool` on no named server.

## Keep the prompts

Grok Build leaves your prompts, its replies and tool bodies out of its events. To keep them, turn on **Keep prompts and replies** for the source and add two variables:

```sh
export OTEL_LOG_USER_PROMPTS=1
export OTEL_LOG_TOOL_CONTENT=1
```

The first sends your prompts and Grok Build's replies, the second each tool call's input and output. Replies stay out when `OTEL_LOG_ASSISTANT_RESPONSES=0` is set, or a managed configuration turns them off. Kaeso drops content a source sends while the source doesn't keep content.

## What Kaeso reads

| Grok Build event | In Kaeso |
| --- | --- |
| `grok_code.api_request` | A request: model, tokens with cache reads and writes, reasoning, and the cost xAI reported |
| `grok_code.api_error` | A failed request; a rate limit is a 429 |
| `grok_code.user_prompt`, `grok_code.assistant_response` | The turn's prompt, with its first request, and its reply, with its last |
| `grok_code.tool_result` of an MCP tool | A tool call with its server, tool, duration and outcome |

Grok Build counts the whole prompt as input, cached tokens included, and the reasoning apart from the output; Kaeso keeps its own counts apart. Its requests are xAI's, priced from the project's catalogue or KPMR, and otherwise at the cost xAI reported. When you're signed in with an account that has an email address, each event names it, so a source that matches members by email counts the request for that member.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [Claude Code](claude-code.md): Count Claude Code's requests and MCP tool calls.
