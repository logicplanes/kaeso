---
title: Modell-API
description: Die Endpunkte, die Apps aufrufen, wie sie sich authentifizieren und wie sie Modelle benennen.
order: 1
---

Die Modell-API spricht die API-Formate von OpenAI und Anthropic, sodass Apps weiter die offiziellen SDKs nutzen. Kaeso übersetzt jede Anfrage in das Format des Anbieters des Modells und die Antwort zurück, Streaming eingeschlossen.

## Basis-URLs

| SDK | Basis-URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

## Endpunkte

| Methode und Pfad | Format |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses, zählt Eingabe-Token |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages, zählt Token |
| `GET /v1/models` | Die Modelle, die der Schlüssel aufrufen darf |
| `GET /v1/models/{model}` | Eines davon |

Die Generierungs-Endpunkte können jedes Generierungsmodell im Katalog aufrufen, unabhängig von seinem Anbieter. `/v1/embeddings` ruft nur Embedding-Modelle auf, die nur OpenAI anbietet, und ein Embedding-Modell bedient keinen anderen Endpunkt; beide Mischungen werden mit 400 abgelehnt.

`GET /v1/models` antwortet im Anthropic-Format, wenn die Anfrage einen `anthropic-version`-Header hat, sonst im OpenAI-Format. Es listet die Modelle des Katalogs, die der Schlüssel aufrufen darf, danach die Aliasse des Projekts, die auf sie zeigen. Im Anthropic-Format wird die Liste mit `limit` (Standard 20), `after_id` und `before_id` geblättert. `GET /v1/models/{model}` nimmt jeden [Modellnamen](#modellnamen) an und antwortet mit 404 für ein Modell, das nicht im Katalog steht oder das der Schlüssel nicht nutzen darf.

Jeder andere Pfad unter `/v1` wird mit 404 beantwortet, ein bekannter Pfad mit der falschen Methode mit 405. Beide Fehler kommen im Anthropic-Format, wenn die Anfrage einen `anthropic-version`-Header hat, sonst im OpenAI-Format.

## Schlüssel

Einen API-Schlüssel mit Inferenzzugriff in einem der beiden Header senden, auf jedem Endpunkt:

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
```

Werden beide gesendet, gilt `x-api-key`. Der Schlüssel bestimmt das Projekt und handelt mit den Rechten und dem Modellzugriff seines Mitglieds oder Dienstkontos, das das Recht `api.request` braucht. Ein Schlüssel nur mit Plattformzugriff wird mit 403 abgelehnt, ebenso Plattform-Schlüssel von Organisationen (`sk-kso-org-…`), die nie Modelle aufrufen.

## Modellnamen

Eine Anfrage nennt ein Modell auf eine von drei Arten, und Kaeso sucht im Katalog des Projekts in dieser Reihenfolge:

| Form | Beispiel |
| --- | --- |
| Ein Alias des Projekts | `fast` |
| Anbieter und Modell | `anthropic/claude-haiku-4-5` |
| Die eigene ID des Modells | `claude-haiku-4-5` |

Ein Modell außerhalb des Katalogs wird mit 404 abgelehnt, eines, das der Schlüssel oder sein Mitglied nicht nutzen darf, mit 403. Wenn mehrere Anbieter im Projekt dieselbe Modell-ID bedienen, wird die reine ID mit 400 abgelehnt, und die Meldung nennt die Anbieter; stattdessen `provider/model` senden. [Modelle und Aliasse](../console/models-and-aliases.md) erklärt den Katalog.

## Streaming

Einen Stream so anfordern, wie das Format es vorsieht (`"stream": true`); die Antwort ist ein Server-Sent-Events-Stream im Format der App.

- Wurde 15 Sekunden lang nichts an die App geschrieben, schreibt Kaeso ein Keepalive, das SDKs ignorieren: einen SSE-Kommentar für die OpenAI-Formate, ein `ping`-Event für das Anthropic-Format. `KAESO_STREAMS_KEEPALIVE_INTERVAL` ändert das Intervall oder schaltet Keepalives ab; siehe [Streams zu Apps](../self-hosting/configuration.md#streams-an-apps).
- Ein Stream, der nach dem Start fehlschlägt oder den der Anbieter beendet, bevor die Antwort vollständig ist, endet mit einem Fehler-Event im Format der App.
- Trennt die App die Verbindung, stoppt Kaeso die Anfrage beim Anbieter sofort und berechnet, was verbraucht wurde.

## Reasoning über Tool-Aufrufe hinweg

Chat Completions hat kein Feld für das Reasoning eines Modells. Ruft eine Chat-Completions-App ein Modell auf, das über ein anderes Format bedient wird, Claude oder ein OpenAI-Modell über Responses, behält Kaeso das Reasoning vor einem Tool-Aufruf und fügt es beim nächsten Zug der App mit demselben Schlüssel wieder ein. Es wird standardmäßig eine Stunde lang verschlüsselt aufbewahrt und nie protokolliert; siehe [Reasoning-Kontinuität](../self-hosting/configuration.md#kontinuität-des-reasonings).

## Antwort-Header

| Header | Wert |
| --- | --- |
| `kaeso-request-id` | Die ID der Anfrage, `req_…`, in jeder Antwort; dieselbe ID steht im Anfrageprotokoll |
| `kaeso-provider` | Der Anbieter, der die Anfrage bedient hat, sobald das Modell bekannt ist, auch bei Fehlern |
| `kaeso-model` | Die Modell-ID des Anbieters, sobald das Modell bekannt ist, auch bei Fehlern |

## Aufrufe aus dem Browser

Ein Browser lässt eine Webseite die API eines anderen Origins nur aufrufen, wenn diese API mit CORS-Headern antwortet. Kaeso sendet keine, bis `KAESO_CORS_ALLOWED_ORIGINS` die Origins Ihrer Web-Apps nennt, etwa `https://app.example.com,http://localhost:5173`; siehe [Browser-Apps](../self-hosting/configuration.md#browser-apps). Dann gilt, nur auf `/v1`:

- **Preflights.** Eine `OPTIONS`-Anfrage mit `Origin` und `Access-Control-Request-Method` von einem eingetragenen Origin wird ohne Schlüssel mit 204 und ohne Body beantwortet: `Access-Control-Allow-Origin` mit diesem Origin, `Access-Control-Allow-Methods: GET, POST`, `Access-Control-Max-Age: 7200` und in `Access-Control-Allow-Headers` die angefragten Header unter denen, die die offiziellen SDKs senden: `authorization`, `x-api-key`, `content-type`, `accept`, `user-agent`, `anthropic-version`, `anthropic-beta`, `anthropic-dangerous-direct-browser-access`, `openai-organization`, `openai-project`, `openai-beta`, `idempotency-key` und `x-stainless-…`. Ein Preflight ist keine Modellanfrage: Er wird weder protokolliert noch abgerechnet noch auf Limits angerechnet.
- **Antworten.** Jede Antwort an einen eingetragenen Origin, auch Fehler, trägt `Access-Control-Allow-Origin` mit diesem Origin und `Access-Control-Expose-Headers: kaeso-request-id, kaeso-provider, kaeso-model, retry-after, retry-after-ms`, damit die App diese Header lesen kann.
- **Andere Origins** bekommen keine CORS-Header, also blockiert der Browser den Aufruf; ihre Preflights bekommen ein leeres 204. Jede Antwort auf `/v1` trägt `Vary: Origin`, damit Caches die Antworten auseinanderhalten.
- **Exakte Origins.** Schema, Host und Port müssen übereinstimmen: `https://app.example.com` lässt weder `http://app.example.com` noch `https://www.app.example.com` zu. `*` wird abgelehnt, weil damit jede Website Anfragen mit den Schlüsseln ihrer Besucher senden könnte.
- **Keine Cookies.** Kaeso sendet nie `Access-Control-Allow-Credentials`, denn Schlüssel stehen in Headern. Die Platform API, die Health-Checks und die Metriken beantworten nie Anfragen von anderen Origins.

Beide SDKs laufen im Browser nur, wenn die App `dangerouslyAllowBrowser: true` übergibt.

> Jeder, der eine Seite benutzt, kann den Schlüssel lesen, den sie enthält. Geben Sie einer Browser-App einen eigenen Schlüssel nur mit Inferenz-Zugriff, kurzer Laufzeit, nur den nötigen Modellen und engen Limits, oder lassen Sie Ihr eigenes Backend kurzlebige Schlüssel ausgeben; siehe [API-Schlüssel](../console/api-keys.md).

## Was Kaeso nicht tut

- **Wiederholungen.** Kaeso sendet jede Anfrage einmal. Die SDKs wiederholen selbst, wo der Fehler es zulässt.
- **Gespeicherter Zustand.** Responses-Anfragen, die den Anbieter bitten, eine Antwort zu speichern oder fortzusetzen (`store: true`, `previous_response_id`, `conversation`, `background: true`), werden abgelehnt; jedes Mal die ganze Unterhaltung senden.
- **Stille Änderungen.** Ein Feld, das sich nicht zum Anbieter des Modells übertragen lässt, wird mit einem Fehler abgelehnt, der es nennt, und nie weggelassen. Anfragen an Chat Completions, Messages und Embeddings mit einem Feld, dessen Name mit `header:` beginnt oder einen `.` enthält, werden mit 400 abgelehnt, weil Kaeso diesen Namen nicht mitführen kann; Responses behält solche Felder im Body. [Formatübersetzung](../concepts/format-translation.md) listet, was zwischen den Formaten übertragen wird.
