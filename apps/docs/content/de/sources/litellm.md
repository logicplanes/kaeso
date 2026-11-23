---
title: LiteLLM
description: Zählen Sie die Anfragen, die ein LiteLLM-Proxy bedient, mit Tokens und Kosten, aus seinem generischen API-Logger.
order: 5
---

Ein LiteLLM-Proxy kann ein Log jeder Anfrage, die er bedient, an jeden HTTP-Endpunkt senden. An eine Kaeso-Quelle gesendet, wird jedes zu einer beobachteten Anfrage im Projekt, mit Modell, Anbieter, Tokens und Kosten.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **LiteLLM** hinzu und ergänzen Sie in der `config.yaml` des Proxys einen Callback `generic_api`, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle:

```yaml
litellm_settings:
  callbacks: ["kaeso"]
callback_settings:
  kaeso:
    callback_type: generic_api
    endpoint: https://kaeso.example.com/ingest/v1/litellm
    headers:
      Authorization: Bearer sk-kso-src-…
```

Starten Sie den Proxy neu und senden Sie ihm eine Anfrage. LiteLLM sendet seine Logs gesammelt alle paar Sekunden; dann zeigt die Quelle **Verbunden**, und die Anfrage erscheint in den Anfrageprotokollen des Projekts.

## Stapel klein halten

LiteLLM sendet bis zu 512 Logs pro Stapel, jedes mit den Nachrichten und der Antwort der Anfrage. Kaeso nimmt höchstens 8 MiB pro Stapel, deshalb setzen Sie `DEFAULT_BATCH_SIZE=50` in der Umgebung des Proxys, wenn seine Anfragen lang sind. Einen abgelehnten Stapel sendet LiteLLM nicht erneut.

## Wessen Anfragen es sind

Jedes Log nennt den LiteLLM-Schlüssel seines Aufrufers und die E-Mail-Adresse des Benutzers dieses Schlüssels, wenn LiteLLM sie kennt. Ordnet die Quelle Mitglieder per E-Mail zu, zählt die Anfrage für das Mitglied mit dieser Adresse. Sonst zählt sie für das Mitglied oder Dienstkonto der Quelle. Die eigenen LiteLLM-Tags der Anfrage werden ihre Tags in Kaeso.

## Prompts behalten

Die Logs von LiteLLM enthalten immer die Nachrichten und die Antwort. Kaeso behält sie nur, wenn **Prompts und Antworten behalten** für die Quelle an ist und das Projekt Inhalte behält.

## Was Kaeso liest

| Feld von LiteLLM | In Kaeso |
| --- | --- |
| `id` | Die ID der Anfrage, sodass ein doppelt gesendetes Log einmal zählt |
| `model`, `custom_llm_provider` | Das Modell und der Anbieter |
| `prompt_tokens`, `completion_tokens` und die Nutzung des Anbieters | Tokens, mit Cache-Lese- und -Schreibvorgängen und Reasoning |
| `status`, `error_str` | Ob sie fehlschlug, und warum |
| `startTime`, `endTime`, `completionStartTime` | Wann sie lief, und die Zeit bis zum ersten Token |
| `response_cost` | Die Kosten, wenn Kaeso keinen Preis für das Modell hat |

Die MCP-Tool-Aufrufe von LiteLLM werden Tool-Aufrufe im Projekt. LiteLLM kann stattdessen auch OpenTelemetry senden: Eine Quelle der Art [OpenTelemetry](opentelemetry.md) liest seine GenAI-Spans.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [OpenRouter](openrouter.md): Zählen Sie Ihre OpenRouter-Anfragen mit Broadcast.
