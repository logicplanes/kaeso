---
title: Modell-API
description: Die Endpunkte, die Apps aufrufen, wie sie sich authentifizieren und wie sie Modelle benennen.
order: 1
---

Die Modell-API spricht die API-Formate von OpenAI und Anthropic. Apps behalten ihre offiziellen SDKs und ändern nur die Basis-URL und den Schlüssel. Kaeso übersetzt jede Anfrage in das Format des Anbieters des Modells und die Antwort zurück, Streams eingeschlossen.

| SDK | Basis-URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

Jedes Format erreicht jedes Generierungsmodell im Katalog des Projekts. Diese Chat-Completions-Anfrage beantwortet Claude:

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H 'Authorization: Bearer sk-kso-…' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

[Modelle aufrufen](../get-started/calling-models.md) zeigt beide SDKs.

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

`/v1/embeddings` ruft nur Embedding-Modelle auf, die nur OpenAI anbietet, und ein Embedding-Modell bedient keinen anderen Endpunkt; eine Anfrage, die beides mischt, wird mit 400 abgelehnt.

Jeder andere Pfad unter `/v1` erhält 404, ein bekannter Pfad mit der falschen Methode 405. Beide Fehler kommen im Anthropic-Format, wenn die Anfrage einen `anthropic-version`-Header hat, sonst im OpenAI-Format.

### Modelle auflisten

`GET /v1/models` wählt sein Format ebenso über den Header `anthropic-version`. Die Liste enthält die Katalogmodelle, die der Schlüssel aufrufen darf, danach die Aliasse des Projekts, die auf sie zeigen. Ein Modell, das der Anbieter dem Anbieter-Schlüssel des Projekts nicht bereitstellt, fehlt ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)).

| | OpenAI-Format | Anthropic-Format |
| --- | --- | --- |
| Seiten | Keine; eine Antwort enthält alle Modelle | `limit` (1 bis 1000, Standard 20), `after_id` oder `before_id` |
| Veraltete Modelle | Nicht gekennzeichnet | `lifecycle: "deprecated"`, wie models.dev sie kennzeichnet |

`GET /v1/models/{model}` nimmt jeden [Modellnamen](#modellnamen). Die Antwort ist 404, wenn das Modell nicht im Katalog ist, der Schlüssel es nicht verwenden darf oder der Anbieter es nicht bereitstellt.

## Schlüssel

Einen API-Schlüssel mit Inferenzzugriff in einem der beiden Header senden, an jedem Endpunkt:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

Kommen beide, gilt `x-api-key`. Der Schlüssel bestimmt das Projekt und handelt mit den Rechten und dem Modellzugriff seines Mitglieds oder Dienstkontos, das dafür das Recht `api.request` braucht. Schlüssel nur mit Plattformzugriff und Plattform-Schlüssel der Organisation (`sk-kso-org-…`) werden mit 403 abgelehnt: Sie rufen nie Modelle auf. [API-Schlüssel](../console/api-keys.md) erklärt, wie Schlüssel erstellt und begrenzt werden.

## Modellnamen

Eine Anfrage benennt ein Modell in einer von drei Formen. Kaeso sucht im Katalog des Projekts in dieser Reihenfolge:

| Form | Beispiel |
| --- | --- |
| Ein Alias des Projekts | `fast` |
| Anbieter und Modell | `anthropic/claude-haiku-4-5` |
| Die eigene ID des Modells | `claude-haiku-4-5` |

Ein Modell außerhalb des Katalogs wird mit 404 abgelehnt, eines, das der Schlüssel oder sein Mitglied nicht verwenden darf, mit 403. Bieten mehrere Anbieter im Projekt dieselbe Modell-ID an, wird die bloße ID mit 400 abgelehnt, und die Meldung nennt die Anbieter; dann `provider/model` senden. [Modelle und Aliasse](../console/models-and-aliases.md) erklärt den Katalog.

## Streaming

Einen Stream so anfordern, wie es das Format vorsieht, mit `"stream": true`; die Antwort ist ein Server-Sent-Events-Stream im Format der App.

- **Keepalives.** Nach 15 Sekunden ohne Ausgabe schreibt Kaeso ein Keepalive, das die SDKs ignorieren: einen SSE-Kommentar in den OpenAI-Formaten, ein `ping`-Event im Anthropic-Format. `KAESO_STREAMS_KEEPALIVE_INTERVAL` ändert das Intervall oder schaltet Keepalives ab; siehe [Streams an Apps](../self-hosting/configuration.md#streams-an-apps).
- **Fehler.** Ein begonnener Stream kann seinen Status nicht mehr ändern. Scheitert der Anbieter oder beendet er den Stream vor dem Ende der Antwort, endet der Stream mit einem Fehler-Event im Format der App.
- **Abbrüche.** Trennt die App die Verbindung, stoppt Kaeso die Anfrage beim Anbieter sofort und berechnet, was verbraucht wurde.

## Reasoning über Tool-Aufrufe hinweg

Chat Completions hat kein Feld für das Reasoning eines Modells. Ruft eine Chat-Completions-App ein Modell auf, das über ein anderes Format bedient wird, etwa Claude oder ein OpenAI-Modell hinter Responses, behält Kaeso das Reasoning vor einem Tool-Aufruf und fügt es wieder ein, wenn der nächste Zug der App mit demselben Schlüssel kommt. Es wird standardmäßig eine Stunde lang verschlüsselt aufbewahrt und nie protokolliert; siehe [Kontinuität des Reasonings](../self-hosting/configuration.md#kontinuität-des-reasonings).

## Antwort-Header

| Header | Wert |
| --- | --- |
| `kaeso-request-id` | Die ID der Anfrage, `req_…`, in jeder Antwort; das Anfrageprotokoll hat dieselbe ID |
| `kaeso-provider` | Der Anbieter, der die Anfrage bedient hat, etwa `anthropic`, sobald das Modell feststeht, auch bei Fehlern |
| `kaeso-model` | Die Modell-ID des Anbieters, sobald das Modell feststeht, auch bei Fehlern |

## Aufrufe aus dem Browser

Ein Browser erlaubt einer Webseite den Aufruf eines anderen Origins nur, wenn dieser mit CORS-Headern antwortet. Kaeso sendet keine, bis `KAESO_CORS_ALLOWED_ORIGINS` die Origins der eigenen Web-Apps auflistet, etwa `https://app.example.com,http://localhost:5173`; siehe [Browser-Apps](../self-hosting/configuration.md#browser-apps). Dann gelten diese Regeln, nur auf `/v1`:

- **Exakte Origins.** Schema, Host und Port müssen übereinstimmen: `https://app.example.com` lässt weder `http://app.example.com` noch `https://www.app.example.com` zu. `*` wird beim Start abgelehnt, weil damit jede Website Anfragen mit den Schlüsseln ihrer Besucher senden könnte.
- **Preflights.** Eine `OPTIONS`-Anfrage mit `Origin` und `Access-Control-Request-Method` von einem gelisteten Origin erhält ohne Schlüssel 204: `Access-Control-Allow-Origin` mit diesem Origin, `Access-Control-Allow-Methods: GET, POST`, `Access-Control-Max-Age: 7200` und in `Access-Control-Allow-Headers` diejenigen angefragten Header, die die offiziellen SDKs senden: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key` und `x-stainless-…`. Ein Preflight ist keine Modellanfrage: Er wird weder protokolliert noch berechnet noch auf Limits angerechnet.
- **Antworten.** Jede Antwort an einen gelisteten Origin, auch eine Fehlerantwort, trägt `Access-Control-Allow-Origin` mit diesem Origin und `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, retry-after, retry-after-ms`, damit die App diese Header lesen kann.
- **Andere Origins** erhalten keine CORS-Header, also blockiert der Browser den Aufruf; ihre Preflights erhalten ein leeres 204. Jede Antwort auf `/v1` trägt `Vary: Origin`, damit Caches die Antworten auseinanderhalten.
- **Keine Cookies.** Kaeso sendet nie `Access-Control-Allow-Credentials`, da Schlüssel in Headern reisen. Die Platform API, die Health-Checks und die Metriken beantworten nie Cross-Origin-Anfragen.

Beide SDKs laufen im Browser nur, wenn die App `dangerouslyAllowBrowser: true` übergibt.

> [!WARNING]
> Wer eine Seite nutzt, kann den Schlüssel darin lesen. Eine Browser-App bekommt daher einen eigenen Schlüssel nur mit Inferenzzugriff, mit kurzer Laufzeit, nur den nötigen Modellen und engen Limits, oder das eigene Backend gibt kurzlebige Schlüssel aus; siehe [API-Schlüssel](../console/api-keys.md).

## Was Kaeso nicht tut

- **Wiederholungen.** Kaeso sendet jede Anfrage einmal. Die SDKs wiederholen selbst, wo der Fehler es zulässt.
- **Gespeicherter Zustand.** Responses-Anfragen, die den Anbieter eine Antwort speichern oder fortsetzen lassen (`store: true`, `previous_response_id`, `conversation`, `background: true`), werden abgelehnt; jedes Mal die ganze Unterhaltung senden.
- **Stille Änderungen.** Ein Feld, das nicht zum Anbieter des Modells übertragen werden kann, wird mit einem Fehler abgelehnt, der es nennt, und nie verworfen. Chat-Completions-, Messages- und Embeddings-Anfragen mit einem Feld, dessen Name mit `header:` beginnt oder einen `.` enthält, werden mit 400 abgelehnt, weil Kaeso diesen Namen nicht übertragen kann; Responses behält solche Felder im Body. [Formatübersetzung](../concepts/format-translation.md) listet, was zwischen den Formaten übertragen wird.
