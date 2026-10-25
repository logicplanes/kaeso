---
title: Modell-API
description: Die Endpunkte, die Apps aufrufen, wie sie sich authentifizieren und wie sie Modelle benennen.
order: 1
---

Apps rufen die Modell-API mit den offiziellen SDKs von OpenAI und Anthropic auf: Richten Sie das SDK auf Ihre Installation aus, und übergeben Sie einen Kaeso-API-Schlüssel. Kaeso übersetzt jede Anfrage in das Format des Anbieters des Modells und die Antwort zurück, Streams eingeschlossen.

Mit dem OpenAI SDK beantwortet Claude diese Chat-Completions-Anfrage:

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const completion = await client.chat.completions.create({
  model: "claude-haiku-4-5",
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(completion.choices[0].message.content);
```

Mit dem Anthropic SDK dieselbe Anfrage im Messages-Format:

```bash tab="curl"
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 200,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
for (const block of message.content) {
  if (block.type === "text") console.log(block.text);
}
```

| SDK | Basis-URL | Sendet den Schlüssel als |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

Jedes Format erreicht jedes Generierungsmodell im Katalog des Projekts. [Modelle aufrufen](../get-started/calling-models.md) zeigt Streaming und mehr von beiden SDKs.

## Endpunkte

| Methode und Pfad | Format |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses, Eingabe-Token zählen |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages, Token zählen |
| `GET /v1/models` | Die Modelle, die der Schlüssel aufrufen darf |
| `GET /v1/models/{model}` | Eines davon |

`/v1/embeddings` bedient nur Embeddings-Modelle, die nur manche [Anbieter](../concepts/supported-providers.md) anbieten, und Embeddings-Modelle bedienen keinen anderen Endpunkt. Kaeso antwortet mit 400 auf eine Anfrage, die beides mischt.

Jeder andere Pfad unter `/v1` erhält 404, ein bekannter Pfad mit der falschen Methode 405. Beide Fehler kommen im Anthropic-Format, wenn die Anfrage einen `anthropic-version`-Header hat, sonst im OpenAI-Format.

### Modelle auflisten

`GET /v1/models` wählt sein Format ebenso über den Header `anthropic-version`. Die Liste enthält die Katalogmodelle, die der Schlüssel aufrufen darf, danach die Aliasse des Projekts, die auf sie zeigen. Jedes Modell steht mit seiner eigenen ID in der Liste, oder als `provider/model`, wenn seine eigene ID nicht zu ihm führt, etwa weil ein anderer Anbieter dieselbe ID anbietet ([Modellnamen](#modellnamen)). Ein Modell, das der Anbieter dem Anbieter-Schlüssel des Projekts nicht bereitstellt, fehlt ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)).

| | OpenAI-Format | Anthropic-Format |
| --- | --- | --- |
| Paginierung | Keine; eine Antwort enthält alle Modelle | `limit` (1 bis 1000, Standard 20), `after_id` oder `before_id` |
| Veraltete Modelle | Nicht gekennzeichnet | `lifecycle: "deprecated"`, wie models.dev sie kennzeichnet |

`GET /v1/models/{model}` nimmt jeden [Modellnamen](#modellnamen). Die Antwort ist 404, wenn das Modell nicht im Katalog ist, der Schlüssel es nicht verwenden darf oder der Anbieter es nicht bereitstellt.

## Schlüssel

Senden Sie einen API-Schlüssel mit Inferenzzugriff in einem der beiden Header, an jedem Endpunkt:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

Sendet eine Anfrage beide, gilt `x-api-key`. Der Schlüssel bestimmt das Projekt. Er handelt mit den Rechten und dem Modellzugriff seines Mitglieds oder Dienstkontos, das dafür das Recht `api.request` braucht.

Schlüssel nur mit Plattformzugriff und Plattform-Schlüssel der Organisation (`sk-kso-org-…`) lehnt Kaeso mit 403 ab: Sie rufen nie Modelle auf. [API-Schlüssel](../console/api-keys.md) erklärt, wie Sie Schlüssel erstellen und begrenzen.

## Modellnamen

Eine Anfrage benennt ein Modell in einer von drei Formen. Kaeso sucht im Katalog des Projekts in dieser Reihenfolge:

| Form | Beispiel |
| --- | --- |
| Ein Alias des Projekts | `fast` |
| Anbieter und Modell | `anthropic/claude-haiku-4-5` |
| Die eigene ID des Modells | `claude-haiku-4-5` |

Kaeso antwortet mit 404 für ein Modell außerhalb des Katalogs und für eines, das der Schlüssel oder sein Mitglied nicht verwenden darf. Beide erhalten dieselbe Meldung, damit ein Schlüssel nicht erfährt, welche anderen Modelle das Projekt hat. Bieten mehrere Anbieter im Projekt dieselbe Modell-ID an, erhält die bloße ID 400 mit einer Meldung, die die Anbieter nennt; senden Sie dann `provider/model`. [Modelle und Aliasse](../console/models-and-aliases.md) erklärt den Katalog.

## Streaming

Fordern Sie einen Stream so an, wie es das Format vorsieht, mit `"stream": true`. Die Antwort ist ein Server-Sent-Events-Stream im Format der App.

- **Keepalives.** Nach 15 Sekunden ohne Ausgabe schreibt Kaeso ein Keepalive, das die SDKs ignorieren: einen SSE-Kommentar in den OpenAI-Formaten, ein `ping`-Event im Anthropic-Format. `KAESO_STREAMS_KEEPALIVE_INTERVAL` ändert das Intervall oder schaltet Keepalives ab ([Streams an Apps](../self-hosting/configuration.md#streams-an-apps)).
- **Fehler.** Ein begonnener Stream kann seinen Status nicht mehr ändern. Scheitert der Anbieter oder beendet er den Stream vor dem Ende der Antwort, endet der Stream mit einem Fehler-Event im Format der App.
- **Abbrüche.** Trennt die App die Verbindung vor dem letzten Event des Streams, stoppt Kaeso die Anfrage beim Anbieter sofort und berechnet, was verbraucht wurde. Ein Stream, dessen letztes Event die App erreicht hat, gilt als abgeschlossen, auch wenn die App die Verbindung direkt danach trennt.

## Reasoning über Tool-Aufrufe hinweg

Chat Completions hat kein Feld für das Reasoning eines Modells. Ruft eine Chat-Completions-App ein Modell auf, das über ein anderes Format bedient wird, behält Kaeso das Reasoning des Modells vor jedem Tool-Aufruf. In der nächsten Runde der App fügt Kaeso das Reasoning wieder ein ([Formatübersetzung](../concepts/format-translation.md#chat-completions)).

## Antwort-Header

| Header | Wert |
| --- | --- |
| `kaeso-request-id` | Die ID der Anfrage, `req_…`, in jeder Antwort. Das Anfrageprotokoll zeigt dieselbe ID |
| `kaeso-provider` | Der Anbieter, der die Anfrage bedient hat, etwa `anthropic`, sobald das Modell feststeht, auch bei Fehlern |
| `kaeso-model` | Die Modell-ID des Anbieters, sobald das Modell feststeht, auch bei Fehlern |
| `kaeso-left-out` | Was Kaeso weggelassen hat, weil das Modell keine Entsprechung hat, so benannt wie in der Anfrage, etwa `tools[1], text.verbosity`; ein Item der Unterhaltung mit Typ und ID, etwa `web_search_call ws_67c9…`. Nur an Antworten, die etwas weggelassen haben ([weggelassene Tools](../development/kpp/cross-format.md#weggelassene-tools)) |
| `server-timing` | `kaeso;dur=…`, die eigene Zeit von Kaeso in Millisekunden, bevor es den Anbieter aufrief, oder bis zur Antwort, wenn die Anfrage keinen erreichte. Danach `upstream;dur=…`, die Zeit des Anbieters bis zu seinem ersten Byte, sobald sie bekannt ist. Ein Stream sendet den Header mit seinen übrigen Headern, daher steht die Zeit, die Kaeso für das Übersetzen des Streams braucht, nur im [Anfrageprotokoll](../console/request-logs.md#anfrage-lesen) |

## Aufrufe aus dem Browser

Ein Browser erlaubt einer Webseite den Aufruf eines anderen Origins nur, wenn dieser mit CORS-Headern antwortet. Kaeso sendet keine, bis `KAESO_CORS_ALLOWED_ORIGINS` die Origins Ihrer Web-Apps auflistet, etwa `https://app.example.com,http://localhost:5173` ([Browser-Apps](../self-hosting/configuration.md#browser-apps)). Dann gelten diese Regeln, nur auf `/v1`:

- **Exakte Origins.** Schema, Host und Port müssen übereinstimmen: `https://app.example.com` lässt weder `http://app.example.com` noch `https://www.app.example.com` zu. Kaeso lehnt `*` beim Start ab, weil damit jede Website Anfragen mit den Schlüsseln ihrer Besucher senden könnte.
- **Preflights.** Eine `OPTIONS`-Anfrage mit `Origin` und `Access-Control-Request-Method` von einem gelisteten Origin erhält ohne Schlüssel 204. Die Antwort trägt `Access-Control-Allow-Origin` mit diesem Origin, `Access-Control-Allow-Methods: GET, POST` und `Access-Control-Max-Age: 7200`. `Access-Control-Allow-Headers` nennt diejenigen angefragten Header, die die offiziellen SDKs senden: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key` und `x-stainless-…`. Ein Preflight ist keine Modellanfrage: Er wird weder protokolliert noch berechnet noch auf Limits angerechnet.
- **Antworten.** Jede Antwort an einen gelisteten Origin, auch eine Fehlerantwort, trägt `Access-Control-Allow-Origin` mit diesem Origin und `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, kaeso-left-out, retry-after, retry-after-ms`, damit die App diese Header lesen kann.
- **Andere Origins** erhalten keine CORS-Header, also blockiert der Browser den Aufruf, und ihre Preflights erhalten ein leeres 204. Jede Antwort auf `/v1` trägt `Vary: Origin`, damit Caches die Antworten auseinanderhalten.
- **Keine Cookies.** Kaeso sendet nie `Access-Control-Allow-Credentials`, da Schlüssel in Headern übertragen werden. Die Platform API, die Health-Checks und die Metriken beantworten nie Cross-Origin-Anfragen.

Beide SDKs laufen im Browser nur, wenn die App `dangerouslyAllowBrowser: true` übergibt.

> [!WARNING]
> Wer eine Seite nutzt, kann den Schlüssel darin lesen. Geben Sie einer Browser-App einen eigenen Schlüssel nur mit Inferenzzugriff, kurzer Laufzeit, nur den nötigen Modellen und engen Limits. Oder lassen Sie Ihr eigenes Backend kurzlebige Schlüssel ausgeben ([API-Schlüssel](../console/api-keys.md)).

## Was Kaeso nicht tut

- **Wiederholungen.** Kaeso sendet jede Anfrage einmal. Die SDKs wiederholen selbst, wo der Fehler es zulässt.
- **Gespeicherter Zustand.** Kaeso lehnt Responses-Anfragen ab, die den Anbieter eine Antwort speichern oder fortsetzen lassen (`store: true`, `previous_response_id`, `conversation`, `background: true`). Senden Sie jedes Mal die ganze Unterhaltung.
- **Stille Änderungen.** Kaeso lehnt ein Feld, das es nicht zum Anbieter des Modells übertragen kann, mit einem Fehler ab, der es nennt, und verwirft es nie. In Chat Completions, Messages und Embeddings erhält ein Feld, dessen Name mit `header:` beginnt oder einen `.` enthält, 400, weil Kaeso diesen Namen nicht übertragen kann; Responses behält solche Felder im Body. [Formatübersetzung](../concepts/format-translation.md) listet, was zwischen den Formaten übertragen wird.
