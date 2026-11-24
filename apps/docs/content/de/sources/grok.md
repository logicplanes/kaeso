---
title: Grok Build
description: Zählen Sie die Anfragen von Grok Build an xAI mit Tokens und Kosten und seine MCP-Tool-Aufrufe aus seinen OpenTelemetry-Ereignissen.
order: 5
---

Grok Build, der Coding-Agent von xAI, kann OpenTelemetry-Ereignisse zu jeder Modellanfrage und jedem Tool-Aufruf an einen eigenen Collector senden. An eine Kaeso-Quelle gesendet, werden daraus beobachtete Anfragen und MCP-Tool-Aufrufe im Projekt, während Grok Build weiter direkt xAI aufruft.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **Grok Build** hinzu und setzen Sie dann diese Variablen in der Shell, aus der Sie `grok` starten, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

```sh
export GROK_EXTERNAL_OTEL=1
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export OTEL_LOG_TOOL_DETAILS=1
```

Starten Sie `grok` und senden Sie einen Prompt. Nach wenigen Sekunden, oder bis zu 30 Sekunden, wenn Grok Build xAI beim Start nicht erreicht, zeigt die Quelle **Verbunden**, und die Anfrage erscheint in den Anfrageprotokollen des Projekts.

## Wohin die Einstellungen gehören

Grok Build sendet nichts, solange nicht sowohl `GROK_EXTERNAL_OTEL=1` als auch ein Exporter gesetzt sind, hier `OTEL_LOGS_EXPORTER=otlp`. Die Header des Collectors liest es nur aus der Umgebung, nie aus seiner Konfigurationsdatei, setzen Sie also mindestens `OTEL_EXPORTER_OTLP_HEADERS` in der Shell. Belassen Sie das Protokoll bei `http/protobuf`: Kaeso nimmt kein gRPC an. Metriken bringen nichts, lassen Sie `OTEL_METRICS_EXPORTER` also ungesetzt.

Mit `OTEL_LOG_TOOL_DETAILS=1` nennt Grok Build die MCP-Server und Tools, die es aufruft; ohne die Variable ist jeder MCP-Aufruf `mcp_tool` ohne benannten Server.

## Prompts behalten

Grok Build lässt Ihre Prompts, seine Antworten und die Inhalte der Tool-Aufrufe aus seinen Ereignissen heraus. Um sie zu behalten, schalten Sie **Prompts und Antworten behalten** für die Quelle ein und ergänzen Sie zwei Variablen:

```sh
export OTEL_LOG_USER_PROMPTS=1
export OTEL_LOG_TOOL_CONTENT=1
```

Die erste sendet Ihre Prompts und die Antworten von Grok Build, die zweite Eingabe und Ausgabe jedes Tool-Aufrufs. Die Antworten bleiben aus, wenn `OTEL_LOG_ASSISTANT_RESPONSES=0` gesetzt ist oder eine verwaltete Konfiguration sie abschaltet. Kaeso verwirft Inhalte, die eine Quelle sendet, solange sie keine Inhalte behält.

## Was Kaeso liest

| Ereignis von Grok Build | In Kaeso |
| --- | --- |
| `grok_code.api_request` | Eine Anfrage: Modell, Tokens mit Cache-Lese- und -Schreibvorgängen, Reasoning und die von xAI gemeldeten Kosten |
| `grok_code.api_error` | Eine fehlgeschlagene Anfrage; ein Ratenlimit ist eine 429 |
| `grok_code.user_prompt`, `grok_code.assistant_response` | Der Prompt des Turns, mit seiner ersten Anfrage, und seine Antwort, mit seiner letzten |
| `grok_code.tool_result` eines MCP-Tools | Ein Tool-Aufruf mit Server, Tool, Dauer und Ergebnis |

Grok Build zählt den ganzen Prompt als Eingabe, gecachte Tokens eingeschlossen, und das Reasoning getrennt von der Ausgabe; Kaeso hält seine eigenen Zählungen getrennt. Seine Anfragen gehören zu xAI, mit dem Preis aus dem Katalog des Projekts oder aus KPMR und sonst mit den von xAI gemeldeten Kosten. Sind Sie mit einem Konto angemeldet, das eine E-Mail-Adresse hat, nennt jedes Ereignis sie, sodass bei einer Quelle, die Mitglieder per E-Mail zuordnet, die Anfrage für dieses Mitglied zählt.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [Claude Code](claude-code.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe von Claude Code.
