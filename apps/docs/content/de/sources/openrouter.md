---
title: OpenRouter
description: Zählen Sie die Anfragen, die Sie an OpenRouter senden, mit Tokens und Kosten, über seinen Broadcast an einen OpenTelemetry-Collector.
order: 6
---

Der Broadcast von OpenRouter sendet einen Trace jeder Anfrage, die Sie an OpenRouter stellen, an ein Ziel Ihrer Wahl. Mit einer Kaeso-Quelle als Ziel vom Typ OpenTelemetry-Collector wird jede Anfrage eine beobachtete Anfrage im Projekt, mit Modell, Tokens und Kosten.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **OpenRouter** hinzu. Ergänzen Sie dann in den Einstellungen von OpenRouter unter **Broadcast** ein Ziel **OpenTelemetry Collector** mit diesen Werten:

```text
https://kaeso.example.com/ingest/otlp/v1/traces
```

```json
{ "Authorization": "Bearer sk-kso-src-…" }
```

Der erste ist der Endpunkt, der volle Pfad des Trace-Signals; der zweite sind die Header. Senden Sie eine Anfrage an OpenRouter, und die Quelle zeigt **Verbunden**, sobald ihr Trace ankommt.

## Was Kaeso liest

Der GenAI-Span jedes Traces wird eine Anfrage: das Modell, der Anbieter, an den OpenRouter weitergeleitet hat, die Tokens, die Kosten, die OpenRouter berechnet hat, und die Dauer. OpenRouter benennt Modelle als `provider/model`, etwa `anthropic/claude-sonnet-4.5`. Kaeso bepreist sie aus dem Katalog des Projekts oder aus KPMR, wenn es das Modell kennt, und behält sonst die Kosten von OpenRouter.

Die Traces von OpenRouter nennen kein Mitglied, deshalb zählt jede Anfrage für das Mitglied oder Dienstkonto der Quelle. Geben Sie jedem Team einen eigenen OpenRouter-Schlüssel und eine eigene Quelle, wenn seine Anfragen getrennt zählen sollen.

## Prompts behalten

OpenRouter schreibt den Prompt und die Antwort in seine Traces. Kaeso behält sie nur, wenn **Prompts und Antworten behalten** für die Quelle an ist und das Projekt Inhalte behält.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [OpenTelemetry](opentelemetry.md): Was Kaeso aus GenAI-Spans und -Log-Ereignissen liest.
