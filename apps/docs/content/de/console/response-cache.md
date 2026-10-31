---
title: Antwort-Cache
description: Beantworten Sie wiederholte, gleiche Anfragen ohne Anbieterkosten aus einer gespeicherten Antwort und wählen Sie, wie lange sie bleibt.
order: 9
---

Der Antwort-Cache beantwortet eine Anfrage, die das Projekt Byte für Byte schon einmal gesendet hat, mit der Antwort, die der Anbieter beim ersten Mal gab. Die App erhält sie in Millisekunden, und der Anbieter berechnet nichts. Das passt zu Evaluierungen, die dieselben Fälle erneut ausführen, zu Support-Bots mit wiederkehrenden Fragen und zu Wiederholungen nach einem Timeout.

Der Cache ist aus, bis Sie ihn für ein Projekt einschalten. Danach senden Sie dieselbe Anfrage zweimal, und die zweite Antwort trägt `kaeso-cache: hit`:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 256,
    "messages": [{"role": "user", "content": "What are your opening hours?"}]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

for _ in range(2):
    raw = client.messages.with_raw_response.create(
        model="claude-haiku-4-5",
        max_tokens=256,
        messages=[{"role": "user", "content": "What are your opening hours?"}],
    )
    print(raw.headers.get("kaeso-cache"))
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

for (let i = 0; i < 2; i++) {
  const { response } = await client.messages
    .create({
      model: "claude-haiku-4-5",
      max_tokens: 256,
      messages: [{ role: "user", content: "What are your opening hours?" }],
    })
    .withResponse();
  console.log(response.headers.get("kaeso-cache"));
}
```

## Cache einschalten

1. Öffnen Sie die **Einstellungen** des Projekts.
2. Schalten Sie unter **Antwort-Cache** die Option **Antworten cachen** ein.
3. Wählen Sie unter **Antworten behalten für**, wie lange eine Antwort im Cache bleibt: 5 Minuten bis 7 Tage. Der Standard ist 1 Stunde.

Die Einstellungen des Projekts zu ändern erfordert das Recht **Projekt bearbeiten**. Eine neue Dauer gilt für Antworten, die ab dann gespeichert werden.

## Was als dieselbe Anfrage gilt

Kaeso vergleicht die Anfrage, die es nach dem Übersetzen an den Anbieter senden würde. So können sich dieselbe Anfrage aus einer App im OpenAI-Format und aus einer App im Anthropic-Format einen Eintrag teilen. Jeder Unterschied ist ein Fehltreffer: der Prompt, das Modell, `max_tokens`, `temperature`, die Tools oder ein Header wie `anthropic-beta`.

- **Im Projekt geteilt.** Alle Schlüssel und Mitglieder des Projekts teilen seine Einträge. Kein anderes Projekt und keine andere Organisation liest sie je.
- **Eine Antwort je Anfrage.** Ein Treffer liefert die Antwort, die das Modell beim ersten Mal gab, auch wenn ein neuer Aufruf wegen `temperature` anders antworten würde.
- **Gecacht.** Erfolgreiche Anfragen ohne Stream an `/v1/chat/completions`, `/v1/responses`, `/v1/messages` und `/v1/embeddings`.
- **Nicht gecacht.** Gestreamte Anfragen, Token-Zählungen, Fehler und Antworten, die größer sind, als die Installation erlaubt (1 MiB, sofern der Betreiber es nicht geändert hat).

## Einzelne Anfrage steuern

Apps entscheiden je Anfrage mit dem Standard-Header `Cache-Control` der Anfrage:

| `Cache-Control` | Was Kaeso tut |
| --- | --- |
| Keiner | Antwortet aus dem Cache, wenn möglich, und speichert neue Antworten |
| `no-cache` | Fragt den Anbieter erneut und speichert die neue Antwort anstelle der alten |
| `no-store` | Umgeht den Cache: kein Nachschlagen, nichts gespeichert |

Jede Antwort eines Projekts mit eingeschaltetem Cache trägt `kaeso-cache`: `hit`, `miss` oder `bypass` (nicht cachebar oder mit `no-store` übersprungen). Ein Treffer trägt außerdem `age`, die Sekunden seit dem Speichern der Antwort.

## Kosten, Limits und Protokolle

Ein Treffer kostet nichts: Seine Kosten sind 0 in der Nutzung, im Anfrageprotokoll und gegenüber Ausgabenbudgets. Seine Token zählen trotzdem so, wie die gespeicherte Antwort sie meldet, damit die Nutzung zeigt, was die App erhalten hat. Anfrage- und Token-Limits zählen Treffer wie andere Anfragen ([Limits und Budgets](limits-and-budgets.md)).

Das Anfrageprotokoll zeigt das Ergebnis jeder Anfrage unter **Antwort-Cache** ([Anfrageprotokolle](request-logs.md)), und die Protokollliste der Platform API filtert mit `response_cache` danach. Ein Treffer hat keine Zeit bis zum ersten Byte und keine Anfrage-ID des Anbieters.

## Cache leeren

Wählen Sie unter **Antwort-Cache** die Option **Cache leeren**. Gespeicherte Antworten werden sofort nicht mehr geliefert, auf jeder Instanz des Gateways. Auch das Ausschalten von **Antworten cachen** leert den Cache.

## Für die Organisation erlauben oder verbieten

Ein Organisations-Admin entscheidet, ob Projekte den Cache überhaupt nutzen dürfen. Öffnen Sie **Organisationseinstellungen**, dann **Protokollierung**, und schalten Sie **Antwort-Cache erlauben** aus oder ein. Aus stoppt den Cache sofort in jedem Projekt und leert ihre Einträge. Wieder eingeschaltet, bleibt der eigene Schalter jedes Projekts, wie er war. Dafür brauchen Sie das Recht **Organisation bearbeiten**.

Betreiber können den Cache für die ganze Installation ausschalten oder ihn abstimmen, mit [den Einstellungen des Antwort-Caches](../self-hosting/configuration.md#antwort-cache).

> [!NOTE]
> Kaeso bewahrt gecachte Antworten in Valkey auf, verschlüsselt mit dem Hauptschlüssel der Installation. Die Schlüssel der Einträge werden mit einem Hash mit Schlüssel aus den Anfragen abgeleitet, daher hält Valkey nie einen Prompt in lesbarer Form.

## Nächste Schritte

- [Anfrageprotokolle](request-logs.md): Sehen Sie, welche Anfragen der Cache beantwortet hat.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie, was ein Projekt ausgeben darf.
- [Modell-API](../api/model-api.md#antwort-header): Lesen Sie jeden Header, den Kaeso seinen Antworten hinzufügt.
