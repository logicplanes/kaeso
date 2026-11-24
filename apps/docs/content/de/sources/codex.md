---
title: Codex
description: Zählen Sie die Anfragen, die die Codex-CLI an OpenAI sendet, und die MCP-Tools, die sie aufruft, aus ihren OpenTelemetry-Logs.
order: 2
---

Die Codex-CLI protokolliert jede Antwort und jeden Tool-Aufruf als OpenTelemetry-Ereignis. An eine Kaeso-Quelle gesendet, werden daraus beobachtete Anfragen und MCP-Tool-Aufrufe im Projekt, während Codex weiter direkt OpenAI aufruft.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **Codex** hinzu und ergänzen Sie in `~/.codex/config.toml` eine Tabelle `[otel]`, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

```toml
[otel]
log_user_prompt = false
exporter = { otlp-http = { endpoint = "https://kaeso.example.com/ingest/otlp/v1/logs", protocol = "binary", headers = { authorization = "Bearer sk-kso-src-…" } } }
```

Führen Sie Codex einmal aus, etwa `codex exec "Say hello"`. Nach wenigen Sekunden zeigt die Quelle **Verbunden**, und die Anfrage erscheint in den Anfrageprotokollen des Projekts.

## Wohin die Einstellungen gehören

Codex liest `[otel]` nur aus Ihrer eigenen `~/.codex/config.toml`, nicht aus der `.codex/config.toml` eines Projekts. Hat die Datei schon eine Tabelle `[otel]`, ergänzen Sie dort die beiden Schlüssel statt einer zweiten Tabelle. Der Endpunkt ist der volle Pfad des Log-Signals, `/ingest/otlp/v1/logs`, und `protocol = "json"` funktioniert auch.

## Prompts behalten

Codex lässt Ihre Prompts aus seinen Ereignissen heraus. Um sie zu behalten, schalten Sie **Prompts und Antworten behalten** für die Quelle ein und setzen Sie `log_user_prompt = true`. Codex sendet immer eine kurze Vorschau der Argumente und Ausgabe jedes Tool-Aufrufs, die Kaeso nur behält, wenn die Quelle Inhalte behält.

## Was Kaeso liest

| Ereignis von Codex | In Kaeso |
| --- | --- |
| `codex.sse_event` einer abgeschlossenen Antwort | Eine Anfrage: Modell, Tokens, gecachte und Reasoning-Tokens, die Zeit bis zum Beginn der Antwort |
| Ein fehlgeschlagenes `codex.api_request` oder eine fehlgeschlagene Antwort | Eine fehlgeschlagene Anfrage mit Status und Meldung |
| `codex.tool_result` eines MCP-Tools | Ein Tool-Aufruf mit Server, Tool, Dauer und Ergebnis |
| `codex.conversation_starts` | Der Anbieter, den die Unterhaltung aufruft, aus dem Namen, den Codex ihm gibt |
| `codex.user_prompt` | Der Prompt, mit der nächsten Anfrage der Unterhaltung |

Codex zählt gecachte Eingabe in seiner Eingabe mit und Reasoning in seiner Ausgabe; Kaeso hält seine eigenen Zählungen getrennt. Aufrufe der eigenen Tools von Codex, etwa seiner Shell, behält Kaeso nicht. Die Antworten von Codex tragen keine Anfrage-ID von OpenAI, deshalb unterscheidet Kaeso sie nach Unterhaltung und Zeit des Ereignisses. Beginn, Prompt und Anbieter einer Anfrage können in einem früheren Export kommen als ihre Nutzung; Kaeso hebt sie für die Anfrage auf.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [Claude Code](claude-code.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe von Claude Code.
