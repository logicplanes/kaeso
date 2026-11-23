---
title: Claude Code
description: Zählen Sie jede Anfrage, die Claude Code an Anthropic sendet, und jedes MCP-Tool, das es aufruft, ohne zu ändern, wohin es sie sendet.
order: 1
---

Claude Code meldet jede Modellanfrage und jeden Tool-Aufruf als OpenTelemetry-Ereignis. Zeigen diese auf eine Kaeso-Quelle, werden daraus beobachtete Anfragen und MCP-Tool-Aufrufe im Projekt, mit ihren Tokens und Kosten, während Claude Code weiter direkt Anthropic aufruft.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **Claude Code** hinzu und tragen Sie ihre Umgebungsvariablen in `~/.claude/settings.json` ein, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

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

Starten Sie Claude Code und senden Sie einen Prompt. Nach wenigen Sekunden zeigt die Quelle **Verbunden**, und die Anfrage erscheint in den Anfrageprotokollen des Projekts. Claude Code sendet seine Ereignisse alle paar Sekunden und beim Beenden.

## In der Shell setzen

Dieselben Variablen funktionieren als Umgebungsvariablen, für eine Shell oder einen CI-Job:

```bash
export CLAUDE_CODE_ENABLE_TELEMETRY=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_METRICS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

Behalten Sie `OTEL_EXPORTER_OTLP_PROTOCOL`: Manche Versionen von Claude Code exportieren ohne sie nichts. Ein Team kann die Variablen in den verwalteten Einstellungen von Claude Code setzen, sodass das Claude Code jedes Mitglieds an dieselbe Quelle meldet und jede Anfrage für das Mitglied zählt, dessen E-Mail-Adresse sie nennt.

## MCP-Tools benennen

Mit `OTEL_LOG_TOOL_DETAILS=1` nennt Claude Code den MCP-Server und das Tool jedes Aufrufs, etwa `linear` und `list_issues`. Ohne die Variable protokolliert Kaeso jeden MCP-Aufruf trotzdem, aber mit unbekanntem Server. Aufrufe der eigenen Tools von Claude Code, etwa Dateien lesen oder Befehle ausführen, behält Kaeso nicht.

## Prompts behalten

Claude Code lässt Ihre Prompts aus seinen Ereignissen heraus. Um sie zu behalten, schalten Sie **Prompts und Antworten behalten** für die Quelle ein und ergänzen Sie `OTEL_LOG_USER_PROMPTS=1`. Mit `OTEL_LOG_TOOL_DETAILS=1` tragen auch die Tool-Aufrufe ihre Argumente.

## Was Kaeso liest

| Ereignis von Claude Code | In Kaeso |
| --- | --- |
| `api_request` | Eine Anfrage: Modell, Tokens, Cache-Lese- und -Schreibvorgänge, Zeit bis zum ersten Token, Anthropics Anfrage-ID, die von Claude Code berechneten Kosten |
| `api_error` | Eine fehlgeschlagene Anfrage mit Status und Meldung |
| `tool_result` eines MCP-Tools | Ein Tool-Aufruf mit Server, Tool, Dauer und Ergebnis |

Kaeso berechnet den Preis jeder Anfrage aus dem Katalog des Projekts oder aus KPMR und nutzt die Kosten von Claude Code nur für ein Modell, das keiner kennt. Die Metriken von Claude Code halten nur den Status der Quelle aktuell, und die Spans seiner erweiterten Telemetrie bleiben außen vor, weil seine Ereignisse dieselben Anfragen schon melden.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [Anfrageprotokolle](../console/request-logs.md): Finden Sie eine Anfrage und sehen Sie, was gesendet und zurückgegeben wurde.
