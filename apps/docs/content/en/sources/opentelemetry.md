---
title: OpenTelemetry
description: Count the requests and MCP tool calls of any app or collector that exports OpenTelemetry GenAI spans or events.
order: 7
---

Many AI SDKs and agent frameworks describe each model call as an OpenTelemetry span that follows the GenAI semantic conventions. Exported to a Kaeso source, each call becomes an observed request in the project, and each MCP tool call a tool call.

[Add a source](index.md#add-a-source) of the kind **OpenTelemetry**, then point your app's OTLP exporter at it, with your install's address and the source's key:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer%20sk-kso-src-…"
export OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental
```

The space in the header is written `%20`, as the OpenTelemetry SDKs read it. Run your app, and its next model call appears in the project's request logs.

## Send it through a collector

An OpenTelemetry Collector forwards what your apps send it with an `otlphttp` exporter:

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

In a collector that already has pipelines, add `otlphttp/kaeso` to their exporters. Kaeso takes OTLP over HTTP, as protobuf or JSON; it doesn't take gRPC.

## What Kaeso reads

A span or a log event is a request when its `gen_ai.operation.name` is `chat`, `text_completion`, `generate_content` or `embeddings`. Kaeso reads its model, provider, tokens, cost, status and timing:

| Attribute | In Kaeso |
| --- | --- |
| `gen_ai.request.model`, `gen_ai.response.model` | The model |
| `gen_ai.provider.name`, or `gen_ai.system` | The provider |
| `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens` | Tokens, with the cache counted apart |
| `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_write.input_tokens`, `gen_ai.usage.reasoning.output_tokens` | Cache reads and writes, and reasoning |
| `gen_ai.response.id` | The request's id, so a call sent twice counts once |
| The span's status `ERROR`, `http.response.status_code` | Whether it failed; `error.type` names why |
| `gen_ai.usage.cost`, or `gen_ai.cost` | The cost, used when Kaeso has no price for the model |
| `user.email`, or `enduser.id` with an email address | The member it counts for |

A span with `gen_ai.operation.name` `execute_tool`, or with `mcp.method.name` `tools/call`, is an MCP tool call, named by `gen_ai.tool.name` and by `mcp.server.name` or `server.address`. A span that sums other calls, such as `invoke_agent`, isn't counted, so an agent's calls count once. When a call has both a span and a log event, both name the same span and count once.

## Keep the prompts

The GenAI conventions put prompts and replies into `gen_ai.input.messages` and `gen_ai.output.messages`, which most SDKs send only when you turn it on. Kaeso keeps them only when **Keep prompts and replies** is on for the source and the project keeps content.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [HTTP](http.md): Send requests and tool calls as JSON instead.
