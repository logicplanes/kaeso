---
title: HTTP
description: Senden Sie die Anfragen und Tool-Aufrufe Ihrer eigenen Skripte und Dienste als JSON über HTTP an eine Quelle.
order: 9
---

Eine Quelle der Art **HTTP** nimmt Stapel von Anfragen und Tool-Aufrufen in Kaesos eigenem JSON an. Nutzen Sie sie für ein Skript, einen Job oder einen Dienst, der Anbieter ohne Kaeso aufruft und kein OpenTelemetry exportieren kann.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **HTTP** hinzu und senden Sie einen Stapel mit dem Schlüssel der Quelle an `/ingest/v1/events`:

```bash
curl https://kaeso.example.com/ingest/v1/events \
  -H "Authorization: Bearer sk-kso-src-…" \
  -H "Content-Type: application/json" \
  -d '{"requests":[{"id":"req_0001","duration_ms":1200,"provider":"openai","model":"gpt-5.4-mini","status":200,"usage":{"input_tokens":1200,"output_tokens":300},"user_email":"ada@example.com"}],"tool_calls":[{"id":"call_0001","duration_ms":80,"server":"linear","tool":"list_issues","user_email":"ada@example.com"}]}'
```

Kaeso antwortet mit dem, was es behalten hat:

```json
{ "requests": 1, "tool_calls": 1, "duplicates": 0, "refused": 0, "refusal": null }
```

## Stapel senden

Ein Stapel ist ein JSON-Objekt mit `requests` und `tool_calls`; beide dürfen fehlen. Senden Sie höchstens 1.000 Datensätze und 8 MiB pro Stapel, auf Wunsch mit gzip und `Content-Encoding: gzip`. Einen Datensatz, den Kaeso nicht behalten kann, lehnt es allein ab: `refused` zählt sie, und `refusal` nennt den Grund des letzten, etwa `no_model`. Einen Body, den Kaeso nicht lesen kann, beantwortet es mit `400` und behält nichts davon.

Jeder Datensatz braucht eine `id`, die innerhalb der Quelle eindeutig ist. Senden Sie einen Datensatz erneut mit derselben `id`, behält Kaeso ihn einmal, sodass Sie einen fehlgeschlagenen Stapel wiederholen können.

## Anfragen

| Feld | Bedeutung |
| --- | --- |
| `id` | Pflicht. Ihre ID der Anfrage |
| `model` | Pflicht. Das Modell, etwa `gpt-5.4-mini` |
| `provider` | Die ID des Anbieters, etwa `openai`, die die Anfrage aus KPMR bepreist |
| `started_at` | Wann sie begann, in RFC 3339; standardmäßig jetzt. Höchstens sieben Tage zurück |
| `duration_ms`, `ttft_ms` | Wie lange sie dauerte, und bis zum ersten Token; ohne `duration_ms` hat sie keine Dauer |
| `reasoning_effort` | Der Reasoning-Aufwand, den sie verlangte, etwa `high` |
| `status` | Ihr HTTP-Status; 200, oder 502 mit einem `error` |
| `error` | Was schiefging, bei einer fehlgeschlagenen Anfrage |
| `usage` | Ihre Tokens, siehe unten |
| `cost_usd` | Ihre Kosten in US-Dollar, wenn Kaeso keinen Preis für das Modell hat |
| `stream` | Ob die Antwort gestreamt wurde |
| `client` | Die App, die sie gesendet hat, etwa `nightly-evals/1.4` |
| `user_email` | Das Mitglied, für das sie zählt, wenn die Quelle Mitglieder per E-Mail zuordnet |
| `tags` | Bis zu zehn Tags; Kaeso verwirft den Rest |
| `provider_request_id` | Die ID der Anfrage beim Anbieter |
| `prompt`, `reply` | Der letzte Prompt und die Antwort, nur behalten, wenn die Quelle Inhalte behält |

`usage` nimmt `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `cache_write_1h_tokens` und `reasoning_tokens`. Standardmäßig lässt `input_tokens` heraus, was aus dem Cache gelesen oder in ihn geschrieben wurde, wie Anthropic zählt. Setzen Sie `input_includes_cache` auf `true`, wenn es diese enthält, wie OpenAI und Gemini zählen.

## Tool-Aufrufe

| Feld | Bedeutung |
| --- | --- |
| `id` | Pflicht. Ihre ID des Aufrufs |
| `tool` | Pflicht. Der Name des Tools |
| `server` | Der Name des MCP-Servers, etwa `linear` |
| `started_at`, `duration_ms` | Wann er begann und wie lange er dauerte |
| `ok` | Ob er gelang; standardmäßig `true` |
| `error` | Was schiefging, bei einem fehlgeschlagenen Aufruf |
| `client`, `user_email` | Wie bei Anfragen |
| `arguments`, `result` | Das JSON des Aufrufs, nur behalten, wenn die Quelle Inhalte behält |

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [OpenTelemetry](opentelemetry.md): Senden Sie stattdessen GenAI-Telemetrie, wenn Ihre App sie exportiert.
