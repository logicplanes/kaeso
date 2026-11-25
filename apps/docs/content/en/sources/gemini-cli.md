---
title: Gemini CLI
description: Count the requests Gemini CLI sends to Google and the MCP tools it calls, from its OpenTelemetry events.
order: 3
---

Gemini CLI reports each model response and tool call as an OpenTelemetry event. Sent to a Kaeso source over HTTP, they become observed requests and MCP tool calls in the project, while Gemini CLI keeps calling Google directly.

[Add a source](index.md#add-a-source) of the kind **Gemini CLI**, then add a `telemetry` block to `~/.gemini/settings.json`, with your install's address and the source's key:

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

Run Gemini CLI once, such as `gemini -p "Say hello"`. The source shows **Connected** within a few seconds, and the request appears in the project's request logs.

> [!NOTE]
> In a folder you haven't trusted, Gemini CLI ignores the folder's `.gemini/settings.json`, and `gemini -p` refuses to run. Trust the folder first, or pass `--skip-trust`.

## Set it in the shell instead

Environment variables override the settings file:

```bash
export GEMINI_TELEMETRY_ENABLED=true
export GEMINI_TELEMETRY_TARGET=local
export GEMINI_TELEMETRY_OTLP_PROTOCOL=http
export GEMINI_TELEMETRY_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export GEMINI_TELEMETRY_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export GEMINI_TELEMETRY_LOG_PROMPTS=false
```

Keep the protocol at `http`: Gemini CLI uses gRPC by default, which Kaeso doesn't take.

## Keep the prompts

Gemini CLI logs your prompts unless you turn it off, so the settings above set `logPrompts` to `false`. To keep prompts, turn on **Keep prompts and replies** for the source and set `logPrompts` to `true`. Gemini CLI then sends each request's whole conversation and its reply, and Kaeso keeps them with their tool calls and thoughts. Kaeso drops content a source sends while the source doesn't keep content.

## What Kaeso reads

| Gemini CLI event | In Kaeso |
| --- | --- |
| `gemini_cli.api_response` | A request: model, tokens, cached and thinking tokens, the prompt of a tool's answer |
| `gemini_cli.api_error` | A failed request with its status and message |
| `gemini_cli.tool_call` of an MCP tool | A tool call with its server, tool, duration and outcome |
| `gemini_cli.user_prompt`, `gemini_cli.api_request` | The prompt and the request's conversation, when Gemini CLI logs prompts |

Gemini CLI counts cached input inside its input and its thoughts apart from its output; Kaeso counts the thoughts as output and the cache apart. Its `auth_type` names the provider: an API key is Google's Gemini API, Vertex AI is Gemini Enterprise Agent Platform. A model that provider doesn't sell, as when Gemini CLI talks to a gateway, is its maker's. An MCP tool keeps its own name, without the `mcp_<server>_` that Gemini CLI writes before it. Gemini CLI also sends the same calls as GenAI events, which Kaeso leaves out so nothing counts twice.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [Antigravity](antigravity.md): Count Antigravity's model calls with a hook script.
