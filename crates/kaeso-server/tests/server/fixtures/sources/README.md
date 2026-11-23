# Source exports

Real OpenTelemetry exports of coding agents, as they reached a collector, for the readers in `crates/kaeso-server/src/ingest`. Each was recorded with the client's own exporter. Names, email addresses, account, organization, session and installation ids, provider request and tool call ids, host names and the host's system, process arguments, plans, plugins and connector names were replaced or left out; timestamps, token counts, costs and the structure are as the client sent them.

| File | Client | How it was made |
| --- | --- | --- |
| `claude-code-logs-1.json` | Claude Code 2.1.322 | `claude -p` reading a file with `Read`, `OTEL_LOGS_EXPORTER=otlp`, `OTEL_EXPORTER_OTLP_PROTOCOL=http/json`; first logs export (user prompt, tool decision, `api_request`, `tool_result`) |
| `claude-code-logs-2.json` | Claude Code 2.1.322 | The same run's second logs export (`api_request`, `assistant_response`) |
| `claude-code-metrics.json` | Claude Code 2.1.322 | The same run's metrics export |
| `claude-code-traces.json` | Claude Code 2.1.322 | The same run with `CLAUDE_CODE_ENHANCED_TELEMETRY_BETA=1`, `OTEL_TRACES_EXPORTER=otlp`: its spans |
| `claude-code-logs-1.pb` … `-3.pb` | Claude Code 2.1.322 | `claude -p` running `echo` with `Bash`, `OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf`; its three logs exports |
| `claude-code-mcp-unnamed.json` | Claude Code 2.1.322 | `claude -p` calling a tool of an HTTP MCP server, without `OTEL_LOG_TOOL_DETAILS`: the tool is `mcp_tool` |
| `claude-code-mcp-named.json` | Claude Code 2.1.322 | The same with `OTEL_LOG_TOOL_DETAILS=1`: `tool_parameters` names the server and the tool |
| `codex-logs.json` | Codex CLI 0.171.0 | `codex exec` against a mock Responses provider, `[otel] exporter = { otlp-http = { protocol = "json" } }` |
| `gemini-cli-logs.json` | Gemini CLI 0.70.0 | `gemini -p` against a mock Gemini API, `GEMINI_TELEMETRY_OTLP_PROTOCOL=http` |
