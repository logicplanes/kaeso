---
title: Gemini CLI
description: Zählen Sie die Anfragen, die Gemini CLI an Google sendet, und die MCP-Tools, die es aufruft, aus seinen OpenTelemetry-Ereignissen.
order: 3
---

Gemini CLI meldet jede Modellantwort und jeden Tool-Aufruf als OpenTelemetry-Ereignis. Über HTTP an eine Kaeso-Quelle gesendet, werden daraus beobachtete Anfragen und MCP-Tool-Aufrufe im Projekt, während Gemini CLI weiter direkt Google aufruft.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **Gemini CLI** hinzu und ergänzen Sie in `~/.gemini/settings.json` einen Block `telemetry`, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

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

Führen Sie Gemini CLI einmal aus, etwa `gemini -p "Say hello"`. Nach wenigen Sekunden zeigt die Quelle **Verbunden**, und die Anfrage erscheint in den Anfrageprotokollen des Projekts.

> [!NOTE]
> In einem Ordner, dem Sie nicht vertraut haben, ignoriert Gemini CLI die `.gemini/settings.json` des Ordners, und `gemini -p` verweigert die Ausführung. Vertrauen Sie dem Ordner zuerst, oder übergeben Sie `--skip-trust`.

## In der Shell setzen

Umgebungsvariablen überschreiben die Einstellungsdatei:

```bash
export GEMINI_TELEMETRY_ENABLED=true
export GEMINI_TELEMETRY_TARGET=local
export GEMINI_TELEMETRY_OTLP_PROTOCOL=http
export GEMINI_TELEMETRY_OTLP_ENDPOINT=https://kaeso.example.com/ingest/otlp
export GEMINI_TELEMETRY_OTLP_HEADERS="Authorization=Bearer sk-kso-src-…"
export GEMINI_TELEMETRY_LOG_PROMPTS=false
```

Belassen Sie das Protokoll bei `http`: Gemini CLI nutzt standardmäßig gRPC, das Kaeso nicht annimmt.

## Prompts behalten

Gemini CLI protokolliert Ihre Prompts, solange Sie es nicht abschalten, deshalb setzen die Einstellungen oben `logPrompts` auf `false`. Um Prompts zu behalten, schalten Sie **Prompts und Antworten behalten** für die Quelle ein und setzen Sie `logPrompts` auf `true`. Gemini CLI sendet dann die ganze Unterhaltung jeder Anfrage und ihre Antwort, und Kaeso behält sie mit ihren Tool-Aufrufen und Thoughts. Kaeso verwirft Inhalte, die eine Quelle sendet, solange sie keine Inhalte behält.

## Was Kaeso liest

| Ereignis von Gemini CLI | In Kaeso |
| --- | --- |
| `gemini_cli.api_response` | Eine Anfrage: Modell, Tokens, gecachte und Thinking-Tokens, der Prompt der Antwort eines Tools |
| `gemini_cli.api_error` | Eine fehlgeschlagene Anfrage mit Status und Meldung |
| `gemini_cli.tool_call` eines MCP-Tools | Ein Tool-Aufruf mit Server, Tool, Dauer und Ergebnis |
| `gemini_cli.user_prompt`, `gemini_cli.api_request` | Der Prompt und die Unterhaltung der Anfrage, wenn Gemini CLI Prompts protokolliert |

Gemini CLI zählt gecachte Eingabe in seiner Eingabe mit und seine Thoughts getrennt von seiner Ausgabe; Kaeso zählt die Thoughts als Ausgabe und den Cache getrennt. Sein `auth_type` nennt den Anbieter: Ein API-Schlüssel steht für die Gemini API von Google, Vertex AI für Gemini Enterprise Agent Platform. Ein Modell, das dieser Anbieter nicht verkauft, etwa wenn Gemini CLI mit einem Gateway spricht, gehört seinem Hersteller. Ein MCP-Tool behält seinen eigenen Namen, ohne das `mcp_<server>_`, das Gemini CLI davorschreibt. Gemini CLI sendet dieselben Aufrufe zusätzlich als GenAI-Ereignisse, die Kaeso auslässt, damit nichts doppelt zählt.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [Antigravity](antigravity.md): Zählen Sie die Modellaufrufe von Antigravity mit einem Hook-Skript.
