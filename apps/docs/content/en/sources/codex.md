---
title: Codex
description: Count the requests the Codex CLI sends to OpenAI and the MCP tools it calls, from its OpenTelemetry logs.
order: 2
---

The Codex CLI logs each response and tool call as an OpenTelemetry event. Sent to a Kaeso source, they become observed requests and MCP tool calls in the project, while Codex keeps calling OpenAI directly.

[Add a source](index.md#add-a-source) of the kind **Codex**, then add an `[otel]` table to `~/.codex/config.toml`, with your install's address and the source's key:

```toml
[otel]
log_user_prompt = false
exporter = { otlp-http = { endpoint = "https://kaeso.example.com/ingest/otlp/v1/logs", protocol = "binary", headers = { authorization = "Bearer sk-kso-src-…" } } }
```

Run Codex once, such as `codex exec "Say hello"`. The source shows **Connected** within a few seconds, and the request appears in the project's request logs.

## Where the settings go

Codex reads `[otel]` only from your own `~/.codex/config.toml`, not from a project's `.codex/config.toml`. If the file has an `[otel]` table already, add the two keys to it instead of a second table. The endpoint is the full path of the logs signal, `/ingest/otlp/v1/logs`, and `protocol = "json"` works too.

## Keep the prompts

Codex leaves your prompts out of its events. To keep them, turn on **Keep prompts and replies** for the source and set `log_user_prompt = true`. Codex always sends a short preview of each tool call's arguments and output, which Kaeso keeps only when the source keeps content.

## What Kaeso reads

| Codex event | In Kaeso |
| --- | --- |
| `codex.sse_event` of a completed response | A request: model, tokens, cached and reasoning tokens, the time until the response began |
| `codex.api_request` that failed, or a failed response | A failed request with its status and message |
| `codex.tool_result` of an MCP tool | A tool call with its server, tool, duration and outcome |

Codex counts cached input inside its input, and reasoning inside its output; Kaeso keeps its own counts apart. Calls to Codex's own tools, such as its shell, aren't kept. Codex's responses carry no request id of OpenAI's, so Kaeso tells them apart by the conversation and the event's time.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [Claude Code](claude-code.md): Count Claude Code's requests and MCP tool calls.
