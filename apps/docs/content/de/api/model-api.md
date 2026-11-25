---
title: Modell-API
description: Die Endpunkte, die Apps aufrufen, wie sie sich authentifizieren und wie sie Modelle benennen.
order: 1
---

Apps rufen die Modell-API mit den offiziellen SDKs von OpenAI und Anthropic oder mit Googles Gen AI SDK auf: Richten Sie das SDK auf Ihre Installation aus, und übergeben Sie einen Kaeso-API-Schlüssel. Kaeso übersetzt jede Anfrage in das Format des Anbieters des Modells und die Antwort zurück, Streams eingeschlossen.

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

Mit Googles Gen AI SDK (`google-genai`) dieselbe Anfrage im Gemini-Format:

```bash tab="curl"
curl https://kaeso.example.com/v1beta/models/claude-haiku-4-5:generateContent \
  -H "x-goog-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "contents": [
      {"role": "user", "parts": [{"text": "Write a haiku about gateways."}]}
    ]
  }'
```

```python tab="Python"
import os
from google import genai

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com"},
)

response = client.models.generate_content(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
print(response.text)
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

const response = await ai.models.generateContent({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
console.log(response.text);
```

| SDK | Basis-URL | Sendet den Schlüssel als |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |
| Google Gen AI (`google-genai`) | `https://kaeso.example.com` | `x-goog-api-key` |

Jedes Format erreicht jedes Generierungsmodell im Katalog des Projekts, eine Gemini-App also Claude und eine OpenAI-App ein Gemini-Modell. [Modelle aufrufen](../get-started/calling-models.md) zeigt Streaming und mehr von jedem SDK.

## Endpunkte

| Methode und Pfad | Format |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses, Eingabe-Token zählen |
| `GET /v1/responses/{response_id}`, `DELETE /v1/responses/{response_id}` | OpenAI Responses, eine Antwort, die der Anbieter für das Projekt aufbewahrt ([Gespeicherte Antworten](../console/organizations-and-projects.md#gespeicherte-antworten)) |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/rerank`, `POST /v2/rerank` | Cohere Rerank ([Rerank-API](rerank.md)) |
| `POST /v1/images/generations`, `POST /v1/images/edits`, `POST /v1/images/variations` | OpenAI Images ([Bilder-API](images.md)) |
| `POST /v1/audio/speech` | OpenAI Speech ([Audio-API](audio.md)) |
| `POST /v1/audio/transcriptions`, `POST /v1/audio/translations` | OpenAI Transcriptions ([Audio-API](audio.md)) |
| `POST /v1/videos`, `GET /v1/videos`, `GET /v1/videos/{video_id}`, `DELETE /v1/videos/{video_id}`, `GET /v1/videos/{video_id}/content`, `POST /v1/videos/{video_id}/remix` | OpenAI Videos ([Videos-API](videos.md)) |
| `POST /v1/files`, `GET /v1/files/{file_id}`, `DELETE /v1/files/{file_id}`, `GET /v1/files/{file_id}/content`, `POST /v1/batches`, `GET /v1/batches`, `GET /v1/batches/{batch_id}`, `POST /v1/batches/{batch_id}/cancel` | OpenAI Batch und Files ([Batches-API](batches.md)) |
| `POST /v1/decisions` | OpenAI Decisions, oder Gemini Decisions mit dem Schlüssel von Gemini ([Decisions-API](decisions.md)) |
| `POST /v1/systemone` | System One ([Decisions-API](decisions.md)) |
| `POST /v1/text-to-speech/{voice_id}`, `/v1/text-to-dialogue`, `/v1/sound-generation`, `/v1/music`, `/v1/voice-changer/{voice_id}`, `/v1/speech-to-text`, `/v1/flows/image`, `GET /v1/voices`, `/v2/voices` | ElevenLabs ([ElevenLabs-API](elevenlabs.md)) |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages, Token zählen |
| `GET /v1/models` | Die Modelle, die der Schlüssel aufrufen darf ([Modelle auflisten](#modelle-auflisten)) |
| `GET /v1/models/{model}` | Eines davon |
| `POST /v1beta/models/{model}:generateContent` | Gemini |
| `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` | Gemini, gestreamt |
| `POST /v1beta/models/{model}:countTokens` | Gemini, Token zählen |
| `POST /v1beta/models/{model}:embedContent` | Gemini-Embeddings |
| `POST /v1beta/models/{model}:batchEmbedContents` | Gemini-Embeddings, mehrere Eingaben |
| `GET /v1beta/models` | Die Modelle, die der Schlüssel aufrufen darf, im Gemini-Format |
| `GET /v1beta/models/{model}` | Eines davon, im Gemini-Format |
| `POST /v1beta/decisions` | Gemini Decisions ([Decisions-API](decisions.md)) |

Kaeso bedient die Pfade von Gemini auch unter `/v1`, wie Google: `google-genai` ruft ab Version 3 standardmäßig `/v1` auf, frühere Versionen `/v1beta`. `POST /v1/models/{model}:{method}` ist immer Gemini. `GET /v1/models`, `GET /v1/models/{model}` und `POST /v1/decisions` antworten im Gemini-Format, wenn die Anfrage den Schlüssel von Gemini trägt, `x-goog-api-key` oder `?key=`, und keinen Header `Authorization`. In einem Gemini-Pfad ist `{model}` ein beliebiger [Modellname](#modellnamen), etwa `claude-haiku-4-5`.

`/v1/embeddings`, `:embedContent` und `:batchEmbedContents` bedienen nur Embeddings-Modelle, die nur manche [Anbieter](../concepts/provider-apis.md) anbieten, und Embeddings-Modelle bedienen keinen anderen Endpunkt. Kaeso antwortet mit 400 auf eine Anfrage, die beides mischt. Ebenso bedienen `/v1/decisions`, `/v1/systemone` und `/v1beta/decisions` nur Entscheidungsmodelle, und ein reines Entscheidungsmodell bedient keinen anderen Endpunkt ([Decisions-API](decisions.md)). `/v1/rerank` bedient nur Rerank-Modelle, und diese bedienen keinen anderen Endpunkt ([Rerank-API](rerank.md)).

Jeder andere Pfad unter `/v1` erhält 404, ein bekannter Pfad mit der falschen Methode 405. Beide Fehler kommen im Anthropic-Format, wenn die Anfrage einen `anthropic-version`-Header hat, im Format von ElevenLabs, wenn sie einen `xi-api-key`-Header hat, im Gemini-Format, wenn sie den Schlüssel von Gemini trägt, und sonst im OpenAI-Format.

Unter `/v1beta` und für Gemini-Aufrufe unter `/v1` kommt jeder Fehler im Gemini-Format. Andere Methoden wie `batchGenerateContent` und andere Pfade wie Dateien oder Cached Contents erhalten 404. Ein bekannter Pfad mit der falschen Methode erhält 405. Ein Aufruf nimmt nur die Query-Parameter `key` und `alt` an, jeder andere erhält 400.

### Modelle auflisten

`GET /v1/models` wählt sein Format ebenso: das von Anthropic mit einem Header `anthropic-version`, das von ElevenLabs mit einem Header `xi-api-key` ([ElevenLabs API](elevenlabs.md#endpunkte)), das von Gemini mit dem Schlüssel von Gemini und ohne Header `Authorization`. `GET /v1beta/models` antwortet immer im Gemini-Format und nennt jedes Modell `models/<name>`. Die Liste enthält die Katalogmodelle, die der Schlüssel aufrufen darf, danach die Aliasse des Projekts, die auf sie zeigen. Jedes Modell steht mit seiner eigenen ID in der Liste, oder als `provider/model`, wenn seine eigene ID nicht zu ihm führt, etwa weil ein anderer Anbieter dieselbe ID anbietet ([Modellnamen](#modellnamen)). Ein Modell, das der Anbieter dem Anbieter-Schlüssel des Projekts nicht bereitstellt, fehlt ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)).

| | OpenAI-Format | Anthropic-Format | Gemini-Format |
| --- | --- | --- | --- |
| Paginierung | Keine; eine Antwort enthält alle Modelle | `limit` (1 bis 1000, Standard 20), `after_id` oder `before_id` | `pageSize` (Standard 50) und `pageToken` mit dem `nextPageToken` der vorigen Seite |
| Veraltete Modelle | Nicht gekennzeichnet | `lifecycle: "deprecated"`, wie KPMR sie kennzeichnet | Nicht gekennzeichnet |

`GET /v1/models/{model}` und `GET /v1beta/models/{model}` nehmen jeden [Modellnamen](#modellnamen). Die Antwort ist 404, wenn das Modell nicht im Katalog ist, der Schlüssel es nicht verwenden darf oder der Anbieter es nicht bereitstellt.

## Schlüssel

Senden Sie einen API-Schlüssel mit Inferenzzugriff in einem dieser Header, an jedem Endpunkt:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
x-goog-api-key: sk-kso-…
xi-api-key: sk-kso-…
```

Sendet eine Anfrage mehrere, gilt `x-api-key`, dann `x-goog-api-key`, dann `xi-api-key`, dann `Authorization`. Auf den Pfaden von Gemini, unter `/v1beta` und `/v1`, kann der Schlüssel auch in der Query stehen, als `?key=sk-kso-…`, wie in Googles REST-Beispielen. Kaeso nutzt ihn nur, wenn kein Header einen Schlüssel trägt, und der Schlüssel aus der Query erreicht nie die Protokolle oder den Anbieter. Der Schlüssel bestimmt das Projekt. Er handelt mit den Rechten und dem Modellzugriff seines Mitglieds oder Dienstkontos, das dafür das Recht `api.request` braucht.

Schlüssel nur mit Plattformzugriff und Plattform-Schlüssel der Organisation (`sk-kso-org-…`) lehnt Kaeso mit 403 ab: Sie rufen nie Modelle auf. [API-Schlüssel](../console/api-keys.md) erklärt, wie Sie Schlüssel erstellen und begrenzen.

## Modellnamen

Eine Anfrage benennt ein Modell in einer von drei Formen. Kaeso sucht im Katalog des Projekts in dieser Reihenfolge:

| Form | Beispiel |
| --- | --- |
| Ein Alias des Projekts | `fast` |
| Anbieter und Modell | `anthropic/claude-haiku-4-5` |
| Die eigene ID des Modells | `claude-haiku-4-5` |

Kaeso antwortet mit 404 für ein Modell außerhalb des Katalogs und für eines, das der Schlüssel oder sein Mitglied nicht verwenden darf. Beide erhalten dieselbe Meldung, damit ein Schlüssel nicht erfährt, welche anderen Modelle das Projekt hat. Bieten mehrere Anbieter im Projekt dieselbe Modell-ID an, erhält die bloße ID 400 mit einer Meldung, die die Anbieter nennt; senden Sie dann `provider/model`. [Modelle und Aliasse](../console/models-and-aliases.md) erklärt den Katalog.

## Anfragekörper

Kaeso liest einen Anfragekörper, der mit `Content-Encoding: gzip`, `x-gzip` oder `zstd` gesendet wird, wie ihn manche SDKs für größere Bodys senden. Kaeso entpackt den Body, bevor es ihn prüft und übersetzt. Der Anbieter erhält die Anfrage also unkomprimiert, und ein Anfrageprotokoll, das Inhalte aufbewahrt, zeigt den entpackten Body. Die Body-Grenze zählt die entpackte Größe; ein Body, der zu mehr entpackt wird, erhält 413 ([Body-Grenzen](../self-hosting/configuration.md#anfrage--und-antwortkörper)).

Ein Body trägt eine Kodierung. Jede andere, etwa `br`, und zwei auf einmal, etwa `gzip, zstd`, erhalten 415 im Format der App. Ein Body, der sich nicht entpacken lässt, erhält 400.

Ein Body über der Grenze erhält 413 im Format der App. Bevor Kaeso antwortet, liest es den Rest eines abgelehnten Bodys, etwa bei einem ungültigen Schlüssel oder einer Methode, die der Pfad nicht annimmt, höchstens 1 GiB und innerhalb der Lesefrist für Bodys ([`server.body_read_deadline`](../self-hosting/configuration.md#adressen)). So gibt ein Proxy vor Kaeso, etwa Traefik, die Antwort weiter, statt selbst mit 502 zu antworten. Ein Body, der mehr als 1 GiB angibt, wird nicht gelesen, und ein Client, der `Expect: 100-continue` sendet und wartet, erhält eine Ablehnung, bevor er den Body sendet.

## Streaming

Fordern Sie einen Stream so an, wie es das Format vorsieht, mit `"stream": true`. Die Antwort ist ein Server-Sent-Events-Stream im Format der App.

Eine Gemini-App ruft stattdessen `:streamGenerateContent?alt=sse` auf, wie es `google-genai` tut. Kaeso streamt Gemini-Antworten nur als Server-Sent Events, daher erhält ein Stream ohne `alt=sse` 400.

- **Keepalives.** Nach 15 Sekunden ohne Ausgabe schreibt Kaeso ein Keepalive, das die SDKs ignorieren: einen SSE-Kommentar in den OpenAI-Formaten, ein `ping`-Event im Anthropic-Format und eine Leerzeile im Gemini-Format, weil `google-genai` jede andere Zeile als JSON liest. `KAESO_STREAMS_KEEPALIVE_INTERVAL` ändert das Intervall oder schaltet Keepalives ab ([Streams an Apps](../self-hosting/configuration.md#streams-an-apps)).
- **Fehler.** Kaeso beginnt den Stream mit der ersten Ausgabe des Anbieters oder wenn das erste Keepalive fällig ist. Scheitert der Stream des Anbieters vorher, erhält die App den Fehler mit seinem Status, etwa 429, wie bei einer nicht gestreamten Anfrage. Ein begonnener Stream kann seinen Status nicht mehr ändern: Scheitert der Anbieter später oder beendet er den Stream vor dem Ende der Antwort, endet der Stream mit einem Fehler-Event im Format der App.
- **Abbrüche.** Trennt die App die Verbindung vor dem letzten Event des Streams, stoppt Kaeso die Anfrage beim Anbieter sofort und berechnet, was verbraucht wurde. Ein Stream, dessen letztes Event die App erreicht hat, gilt als abgeschlossen, auch wenn die App die Verbindung direkt danach trennt.

## Inhalt aus dem Protokoll halten

Ein Projekt, das Inhalte protokolliert, kann den Inhalt einer einzelnen Anfrage mit dem Header `kaeso-log-content: off` aus dem [Anfrageprotokoll](../console/request-logs.md) halten, etwa einen Prompt mit den Zahlungsdaten eines Kunden. Das Protokoll enthält weiter die Metadaten der Anfrage, etwa Modell, Tokens, Kosten, Status, Zeiten und Schlüssel, aber nicht, was die App gesendet oder erhalten hat, und auch keine Medien. Der Header schaltet Inhalte nur aus, nie ein, und jeder andere Wert ergibt 400. Er hält die Antwort nicht aus dem [Antwort-Cache](#antwort-cache); senden Sie dafür zusätzlich `Cache-Control: no-store`.

## Zeitlimits

Eine Anfrage kann mit dem Header `kaeso-timeout` verkürzen, wie lange Kaeso auf den Anbieter wartet, in Sekunden von 1 bis 3600, etwa `kaeso-timeout: 20` für eine App, die höchstens 20 Sekunden wartet. Er senkt die Wartezeit auf das erste Byte und bei einer nicht gestreamten Antwort die auf die ganze Antwort unter die der Installation und des Modells, hebt sie aber nie an; das Leerlauf-Zeitlimit eines Streams bleibt. Hinter einer [Alias-Route](../concepts/alias-routes.md) gilt er für jeden Schritt, den die Anfrage erreicht: Ein Schritt, dessen Zeit abläuft, scheitert als `unavailable` mit 504, sodass eine Route, die bei `unavailable` zurückfällt, weitergeht und rechtzeitig antwortet. Ein anderer Wert ergibt 400. Das eigene `timeout` der SDKs schließt stattdessen die Verbindung, sodass die App die Antwort des Fallbacks nie erhält.

Eine App kann eine Anfrage mit dem Header `kaeso-tags` markieren, einer kommagetrennten Liste wie `kaeso-tags: checkout,eval-42`, um ihren Verkehr unterhalb des Schlüssels zu unterscheiden: eine Funktion von einer anderen oder einen Evaluationslauf vom Produktivbetrieb. Eine Anfrage trägt bis zu 10 Tags aus je 1 bis 64 Kleinbuchstaben, Ziffern, `.`, `_`, `:` und `-`; alles andere wird mit Status 400 im Format der App abgelehnt, mit dem Namen des Headers, damit kein Tag stillschweigend verloren geht. Das [Anfrageprotokoll](../console/request-logs.md) speichert die Tags mit den Metadaten der Anfrage, sein Filter `tag` findet die Anfragen mit einem davon, und der Export enthält sie. Wer die Metadaten des Protokolls sehen darf, sieht die Tags, also schreiben Sie keine personenbezogenen Daten hinein.

## Reasoning über Tool-Aufrufe hinweg

Chat Completions hat kein Feld für das Reasoning eines Modells, und das Gemini-Format keines für das Reasoning eines anderen Anbieters. Ruft eine Chat-Completions- oder Gemini-App ein Modell auf, das über ein anderes Format bedient wird, behält Kaeso das Reasoning des Modells vor jedem Tool-Aufruf. In der nächsten Runde der App fügt Kaeso das Reasoning wieder ein ([Formatübersetzung](../concepts/format-translation.md#chat-completions)). Ruft eine Gemini-App ein Gemini-Modell auf, gehen Geminis eigene Thought Signatures unverändert hin und zurück.

## Antwort-Cache

Ein Projekt kann eine wiederholte, gleiche Anfrage aus einer gespeicherten Antwort beantworten ([Antwort-Cache](../console/response-cache.md)). Senden Sie `Cache-Control: no-cache`, um den Anbieter erneut zu fragen und die neue Antwort zu speichern, `Cache-Control: no-store`, um den Cache für diese Anfrage zu umgehen, `Cache-Control: max-age=60`, um nur eine in der letzten Minute gespeicherte Antwort zu nehmen, oder `Cache-Control: only-if-cached`, um aus dem Cache oder mit 504 zu antworten, ohne den Anbieter aufzurufen.

## Antwort-Header

| Header | Wert |
| --- | --- |
| `kaeso-request-id` | Die ID der Anfrage, `req_…`, in jeder Antwort. Das Anfrageprotokoll zeigt dieselbe ID |
| `kaeso-upstream-request-id` | Die ID, die der Anbieter seiner Antwort gab (sein `request-id`, `x-request-id` oder das `cf-aig-log-id` von Cloudflare AI Gateway), für seinen Support. In Antworten und in Fehlern, die der Anbieter beantwortet hat, vom Routenschritt, der antwortete; fehlt bei einem Treffer im Antwort-Cache, bevor ein Anbieter antwortete und wenn der Anbieter keine sendete. Das Anfrageprotokoll zeigt sie als **Upstream-Anfrage** |
| `kaeso-cost` | Was eine nicht gestreamte Antwort gekostet hat, wie ihr Eintrag im Anfrageprotokoll sagen wird: der Betrag in USD, `0` bei einem Treffer im Antwort-Cache oder `unknown`, wenn das Modell keinen Preis für das hat, was es gemacht hat. Nur wenn der Inhaber des Schlüssels im Projekt `costs.view` hat; nicht beim Start eines Videojobs oder einer Bildgenerierung, die abgerechnet werden, wenn sie enden |
| `kaeso-provider` | Der Anbieter, der die Anfrage bedient hat, etwa `anthropic`, sobald das Modell feststeht, auch bei Fehlern |
| `kaeso-model` | Die Modell-ID des Anbieters, sobald das Modell feststeht, auch bei Fehlern |
| `kaeso-left-out` | Was Kaeso weggelassen hat, weil das Modell keine Entsprechung hat, so benannt wie in der Anfrage, etwa `tools[1], text.verbosity`, `temperature` für ein Modell, das nur seine Standardtemperatur annimmt, und auch `top_p` und `top_k`, wenn dieses Modell Claude ist, etwa Claude Opus 4.7 und neuer, oder `messages[4].output_config.effort` für die Aufwandsstufe einer Systemnachricht; ein Item der Unterhaltung mit Typ und ID, etwa `web_search_call ws_67c9…`. Nur an Antworten, die etwas weggelassen haben ([weggelassene Tools](../development/kpp/cross-format.md#weggelassene-tools), [Modelle ohne Aufwandsstufen](../development/kpp/reasoning.md#modelle-ohne-aufwandsstufen)) |
| `kaeso-adapted` | Die Tools, deren Parameter-Schema Kaeso zu einem Objekt zusammengeführt hat, weil das Modell keine Union an seiner Wurzel annimmt, mit ihrem Pfad in der Anfrage, etwa `tools[0].tools[3]`. Das Modell sieht die Varianten in der Beschreibung des Tools, und seine Aufrufe erreichen die App unverändert. Außerdem die Tool-Aufrufe im Gespräch, deren Argumente Kaeso für ein Modell, das nur Objekte annimmt, zu einem Objekt gemacht hat, als `tool call` mit der ID des Aufrufs, etwa `tool call call_abc`. Außerdem die Tool-Ergebnisse, deren Bilder und Dateien Kaeso für ein Chat-Completions-Modell in die nächste Benutzernachricht verschoben hat, als `tool result` mit der ID des Aufrufs, etwa `tool result call_abc`. Außerdem die `metadata.user_id` einer Anthropic-App, die Kaeso als ihren SHA-256-Hash gesendet hat, weil der Anbieter eine kürzere Endnutzer-ID annimmt, etwa die 128 Zeichen von OpenAI. Außerdem ein Ausgabelimit über dem des Modells in einer Anfrage aus einem anderen Format, das Kaeso auf das Maximum des Modells gesenkt hat, mit dem Feld der App, etwa `max_tokens`. Außerdem das Ausgabelimit einer Anfrage an ein ChatGPT-Abo, dessen Route es ablehnt: Kaeso beendet die Antwort selbst, sobald ihre geschätzte Ausgabe das Limit erreicht, mit dem Stoppgrund des Formats dafür, etwa `max_tokens`, mit dem Feld der App. Außerdem abgeschaltetes Thinking für ein Modell, das immer Reasoning nutzt und stattdessen seine niedrigste Aufwandsstufe bekommt, mit dem Feld der App, etwa `thinking`. Außerdem eine Aufwandsstufe, die das Modell nicht nennt und die zur nächstgelegenen Stufe wird, die es nennt, mit dem Feld der App, etwa `output_config.effort`, und adaptives Denken für ein Claude-Modell ohne adaptives Denken, das zu `enabled`-Denken mit Budget wird, als `thinking`. Ebenso ein Denk-Budget für Claude Opus 4.7 und neuer, die Budgets ablehnen, das zu adaptivem Denken mit der Aufwandsstufe des Budgets wird, als `thinking`, bei einer Gemini-App als `generationConfig.thinkingConfig`. Außerdem die `tool_addition`- und `tool_removal`-Blöcke der Systemnachrichten einer Anthropic-App, deren Tool-Änderungen Kaeso auf die Tool-Liste angewandt hat, die ein Modell in einem anderen Format bekam, mit dem Pfad des Blocks, etwa `messages[4].blocks[1]`. Außerdem ein Tool, in dessen Parameter-Schema Kaeso ein `pattern` für einen Anbieter umgeschrieben hat, der Patterns kompiliert, mit seinem Pfad, etwa `tools[2]`. Nur an Antworten, die ein Tool, einen Aufruf, ein Ergebnis, eine Tool-Änderung, die ID des Endnutzers, das Ausgabelimit oder die Aufwandsstufe angepasst haben ([Modelle, die immer Reasoning nutzen](../development/kpp/reasoning.md#modelle-die-immer-reasoning-nutzen), [Tool-Schemas mit Union-Wurzel](../development/kpp/cross-format.md#tool-schemas-mit-union-wurzel), [Argumente von Tool-Aufrufen](../development/kpp/cross-format.md#argumente-von-tool-aufrufen), [Medien in Tool-Ergebnissen](../development/kpp/cross-format.md#medien-in-tool-ergebnissen), [Inline-Tools](../development/kpp/cross-format.md#inline-tools), [Anbieterprofile](../development/kpp/openai-chat-providers.md#anbieterprofile)) |
| `kaeso-route` | Jeder Schritt der Route eines Alias, den die Anfrage erreichte, der Reihe nach als Structured-Field-Liste ([RFC 9651](https://www.rfc-editor.org/rfc/rfc9651)): der Pfad des Schritts, sein `model` als `provider/model`, sein `status` (der des Anbieters oder der, den Kaeso einem eigenen Fehler gibt: 502, wenn der Anbieter nicht erreichbar ist oder Kaesos Schlüssel ablehnt, 504, wenn er nicht rechtzeitig antwortet) und `dur`, seine Dauer in Millisekunden, wenn er scheiterte. Die Antwort selbst kann im Format der App einen anderen Status tragen, etwa 503 für Anthropics 529 an eine OpenAI-App. `tries` zählt, wie oft der Schritt die Anfrage gesendet hat, seine Wiederholungen und die Schlüssel des Projekts für den Anbieter, die er versuchte, eingeschlossen, wenn es mehr als einmal war. Ein Schritt, den der Schlüssel nicht nutzen darf, trägt `skipped` und kein Modell; ein Schritt, der übersprungen wurde, weil er gerade scheiterte, trägt `skipped` und sein Modell. Etwa `"route";model="anthropic/claude-sonnet-5-5";status=529;dur=1204, "route.fallback";model="openai/gpt-5";status=200`. Nur bei Antworten an einen Alias ([Alias-Routen](../concepts/alias-routes.md#was-apps-und-protokolle-sehen)) |
| `kaeso-limits` | Wie viel von jeder geltenden Rate und jedem harten Budget übrig ist, als Structured-Field-Liste: Ebene, Art und Zeitraum des Limits, etwa `"key.requests.minute"`, mit `model` für das eigene Limit eines Modells, `limit`, `remaining` und `reset` in Sekunden (keins für ein Gesamtbudget). Ausgaben sind ein USD-String. `remaining` gilt vor dieser Anfrage, nur eine Anfragerate zählt sie schon mit. Nur wenn der Inhaber des Schlüssels die Limits des Projekts sehen darf (`limits.view`), Ausgaben nur mit `costs.view`; eine 429 von Kaeso trägt das Limit, das sie abgelehnt hat, mit `remaining=0` ([Limits und Budgets](../console/limits-and-budgets.md#was-eine-app-sieht)) |
| `kaeso-cache` | `hit`, wenn der [Antwort-Cache](../console/response-cache.md) geantwortet hat, `miss`, wenn die Anfrage zum Anbieter ging, `bypass`, wenn sie nicht cachebar war oder die App `Cache-Control: no-store` gesendet hat. Nur in Projekten mit eingeschaltetem Cache |
| `age` | Die Sekunden seit dem Speichern der gecachten Antwort. Nur bei `kaeso-cache: hit` |
| `kaeso-guardrails` | Ein Eintrag je [Guardrail](../console/guardrails.md#was-apps-sehen), die die Anfrage oder die Antwort geprüft hat: ihr Name, `phase`, `action`, `category`, `failure` und `dur` in Millisekunden. Nur in Projekten, deren Anfragen Guardrails durchlaufen |
| `Deprecation`, `Sunset`, `Link` | Der Tag der Abkündigung, der Tag der Abschaltung und Links zum Nachfolger und zur Ankündigung des Anbieters. Nur für ein Modell, das sein Anbieter auslaufen lässt ([Modell-Lebenszyklus](../concepts/model-lifecycle.md#antworten-tragen-die-daten)) |
| `server-timing` | `upload;dur=…`, die Zeit in Millisekunden, die die App zum Senden des Bodys brauchte. Danach `kaeso;dur=…`, die eigene Zeit von Kaeso in Millisekunden, bevor es den Anbieter aufrief, oder bis zur Antwort, wenn die Anfrage keinen erreichte, ohne den Upload und ohne die Zeit, die gescheiterte Schritte eines Alias auf ihre Anbieter gewartet haben. Zuletzt `upstream;dur=…`, die Zeit des Anbieters bis zu seinem ersten Byte, sobald sie bekannt ist. Ein Stream sendet den Header mit seinen übrigen Headern, daher steht die Zeit, die Kaeso für das Übersetzen des Streams braucht, nur im [Anfrageprotokoll](../console/request-logs.md#anfrage-lesen) |

## Aufrufe aus dem Browser

Ein Browser erlaubt einer Webseite den Aufruf eines anderen Origins nur, wenn dieser mit CORS-Headern antwortet. Kaeso sendet keine, bis `KAESO_CORS_ALLOWED_ORIGINS` die Origins Ihrer Web-Apps auflistet, etwa `https://app.example.com,http://localhost:5173` ([Browser-Apps](../self-hosting/configuration.md#browser-apps)). Dann gelten diese Regeln, nur auf `/v1`, `/v1beta` und `/v2`:

- **Exakte Origins.** Schema, Host und Port müssen übereinstimmen: `https://app.example.com` lässt weder `http://app.example.com` noch `https://www.app.example.com` zu. Kaeso lehnt `*` beim Start ab, weil damit jede Website Anfragen mit den Schlüsseln ihrer Besucher senden könnte.
- **Preflights.** Eine `OPTIONS`-Anfrage mit `Origin` und `Access-Control-Request-Method` von einem gelisteten Origin erhält ohne Schlüssel 204. Die Antwort trägt `Access-Control-Allow-Origin` mit diesem Origin, `Access-Control-Allow-Methods: GET, POST, DELETE` und `Access-Control-Max-Age: 7200`. `Access-Control-Allow-Headers` nennt diejenigen angefragten Header, die die offiziellen SDKs senden: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key`, `x-goog-api-key`, `x-goog-api-client`, `xi-api-key` und `x-stainless-…`, dazu `cache-control`, das der [Antwort-Cache](#antwort-cache) liest, `kaeso-timeout` ([Zeitlimits](#zeitlimits)), `kaeso-log-content` ([Inhalt aus dem Protokoll halten](#inhalt-aus-dem-protokoll-halten)), `kaeso-tags`, `traceparent` und `tracestate` ([Traces](../self-hosting/monitoring.md#traces)), und die Namen in `KAESO_CORS_ALLOWED_HEADERS` ([Konfiguration](../self-hosting/configuration.md#browser-apps)). Ein Preflight ist keine Modellanfrage: Er wird weder protokolliert noch berechnet noch auf Limits angerechnet.
- **Antworten.** Jede Antwort an einen gelisteten Origin, auch eine Fehlerantwort, trägt `Access-Control-Allow-Origin` mit diesem Origin und `Access-Control-Expose-Headers: kaeso-request-id, kaeso-upstream-request-id, kaeso-cost, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, kaeso-route, kaeso-limits, kaeso-cache, kaeso-guardrails, server-timing, age, retry-after, retry-after-ms`, damit die App diese Header lesen kann.
- **Andere Origins** erhalten keine CORS-Header, also blockiert der Browser den Aufruf, und ihre Preflights erhalten ein leeres 204. Jede Antwort auf `/v1`, `/v1beta` und `/v2` trägt `Vary: Origin`, damit Caches die Antworten auseinanderhalten.
- **Keine Cookies.** Kaeso sendet nie `Access-Control-Allow-Credentials`, da Schlüssel in Headern übertragen werden. Die Platform API, die Health-Checks und die Metriken beantworten nie Cross-Origin-Anfragen.

Die SDKs von OpenAI und Anthropic laufen im Browser nur, wenn die App `dangerouslyAllowBrowser: true` übergibt.

> [!WARNING]
> Wer eine Seite nutzt, kann den Schlüssel darin lesen. Geben Sie einer Browser-App einen eigenen Schlüssel nur mit Inferenzzugriff, kurzer Laufzeit, nur den nötigen Modellen und engen Limits. Oder lassen Sie Ihr eigenes Backend kurzlebige Schlüssel ausgeben ([API-Schlüssel](../console/api-keys.md)).

## Was Kaeso nicht tut

- **Wiederholungen.** Kaeso sendet eine Anfrage nur dann erneut an ihr Modell, wenn das Projekt oder ein Alias-Schritt es verlangt, nach einer abgebrochenen Verbindung, einem 5xx oder einem überlasteten Anbieter, und nur, bevor eine Antwort die App erreicht hat ([Alias-Routen](../concepts/alias-routes.md#fallbacks)). Sonst sendet es jede Anfrage einmal; die SDKs wiederholen selbst, wo der Fehler es zulässt.
- **Gespeicherter Zustand.** Kaeso selbst speichert keine Unterhaltungen. `store: true` und `previous_response_id` funktionieren nur, wo ein Projekt beim Anbieter gespeicherte Antworten erlaubt, für Modelle, deren Antworten der Anbieter aufbewahrt ([Gespeicherte Antworten](../console/organizations-and-projects.md#gespeicherte-antworten)); anderswo lehnt Kaeso sie ab, und `conversation` und `background: true` lehnt es überall ab. Senden Sie sonst jedes Mal die ganze Unterhaltung.
- **Stille Änderungen.** Kaeso lehnt ein Feld, das es nicht zum Anbieter des Modells übertragen kann, mit einem Fehler ab, der es nennt, und verwirft es nie. In Chat Completions, Messages und Embeddings erhält ein Feld, dessen Name mit `header:` beginnt oder einen `.` enthält, 400, weil Kaeso diesen Namen nicht übertragen kann; Responses behält solche Felder im Body. [Formatübersetzung](../concepts/format-translation.md) listet, was zwischen den Formaten übertragen wird.
