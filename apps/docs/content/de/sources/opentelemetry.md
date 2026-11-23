---
title: OpenTelemetry
description: Zählen Sie die Anfragen und MCP-Tool-Aufrufe jeder App und jedes Collectors, die GenAI-Spans oder -Ereignisse von OpenTelemetry exportieren.
order: 7
---

Viele KI-SDKs und Agent-Frameworks beschreiben jeden Modellaufruf als OpenTelemetry-Span nach den semantischen GenAI-Konventionen. An eine Kaeso-Quelle exportiert, wird jeder Aufruf eine beobachtete Anfrage im Projekt und jeder MCP-Tool-Aufruf ein Tool-Aufruf.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **OpenTelemetry** hinzu und richten Sie den OTLP-Exporter Ihrer App auf sie, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer%20sk-kso-src-…"
export OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental
```

Das Leerzeichen im Header steht als `%20`, wie die OpenTelemetry-SDKs es lesen. Starten Sie Ihre App, und ihr nächster Modellaufruf erscheint in den Anfrageprotokollen des Projekts.

## Über einen Collector senden

Ein OpenTelemetry Collector leitet mit einem Exporter `otlphttp` weiter, was Ihre Apps ihm senden:

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

In einem Collector, der schon Pipelines hat, ergänzen Sie `otlphttp/kaeso` bei deren Exportern. Kaeso nimmt OTLP über HTTP an, als Protobuf oder JSON, aber kein gRPC.

## Was Kaeso liest

Ein Span oder ein Log-Ereignis ist eine Anfrage, wenn sein `gen_ai.operation.name` `chat`, `text_completion`, `generate_content` oder `embeddings` ist. Kaeso liest Modell, Anbieter, Tokens, Kosten, Status und Zeiten:

| Attribut | In Kaeso |
| --- | --- |
| `gen_ai.request.model`, `gen_ai.response.model` | Das Modell |
| `gen_ai.provider.name` oder `gen_ai.system` | Der Anbieter |
| `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens` | Tokens, mit dem Cache getrennt gezählt |
| `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_write.input_tokens`, `gen_ai.usage.reasoning.output_tokens` | Cache-Lese- und -Schreibvorgänge und Reasoning |
| `gen_ai.response.id` | Die ID der Anfrage, sodass ein doppelt gesendeter Aufruf einmal zählt |
| `error.type`, `http.response.status_code` | Ob sie fehlschlug |
| `user.email` oder `enduser.id` mit einer E-Mail-Adresse | Das Mitglied, für das sie zählt |

Ein Span mit `gen_ai.operation.name` `execute_tool` und `mcp.method.name` `tools/call` ist ein MCP-Tool-Aufruf, benannt nach `gen_ai.tool.name` und `server.address`. Ein Span, der andere Aufrufe zusammenfasst, etwa `invoke_agent`, zählt nicht, sodass die Aufrufe eines Agents einmal zählen. Hat ein Aufruf einen Span und ein Log-Ereignis, nennen beide denselben Span und zählen einmal.

## Prompts behalten

Die GenAI-Konventionen legen Prompts und Antworten in `gen_ai.input.messages` und `gen_ai.output.messages`, die die meisten SDKs nur senden, wenn Sie es einschalten. Kaeso behält sie nur, wenn **Prompts und Antworten behalten** für die Quelle an ist und das Projekt Inhalte behält.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [HTTP](http.md): Senden Sie Anfragen und Tool-Aufrufe stattdessen als JSON.
