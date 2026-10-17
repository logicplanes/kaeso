---
title: Ablauf einer Anfrage
description: Die Schritte, die eine Modellanfrage durch das Gateway nimmt.
order: 1
---

Jede Modellanfrage nimmt denselben Weg durch das Gateway. Nichts wird unübersetzt durchgereicht, und nichts wird wiederholt. Jede Ablehnung auf dem Weg ist ein Fehler im eigenen Format der App.

```text
app ─▶ key ─▶ decode ─▶ model ─▶ limits ─▶ provider key ─▶ encode and send ─▶ provider
app ◀─ encode reply ◀─ decode reply ◀──────────────────────────────────────── provider
                             └─▶ usage, cost and request log
```

## Schlüssel

Das Gateway liest den API-Schlüssel aus `x-api-key`, sonst aus `Authorization: Bearer`. Der Schlüssel bestimmt das Projekt und handelt dort mit den Rechten seines Mitglieds oder Dienstkontos. Er braucht Inferenzzugriff, und sein Inhaber braucht das Recht `api.request`; Plattform-Schlüssel von Organisationen können keine Modelle aufrufen. Schlüssel, die in den letzten 30 Sekunden nachgeschlagen wurden, werden aus dem Speicher beantwortet.

## Decodieren

Der Body darf bis zu `KAESO_BODY_LIMITS_MODEL_API` (32 MiB) groß sein; ein größerer wird mit 413 abgelehnt. KPP decodiert die Anfrage aus dem Format der App, etwa OpenAI Chat Completions oder Anthropic Messages, in Kaesos kanonisches Format. Eine Anfrage, die sich nicht lesen lässt oder die etwas verlangt, das Kaeso nie an einen Anbieter sendet, etwa mehr als eine Auswahl oder gespeicherten Responses-Zustand, wird hier abgelehnt.

## Modell

Der Modellname wird im Katalog des Projekts aufgelöst: zuerst ein Alias, dann `provider/model`, dann die eigene ID des Modells. Ein Name, der dort nicht steht, wird mit 404 abgelehnt, eine Modell-ID, die mehrere Anbieter im Projekt bedienen, mit 400. Der Modellzugriff des Mitglieds und der des Schlüssels werden geprüft (403).

Der Katalogeintrag liefert Limits, Preise und Fähigkeiten des Modells, das Format, in dem der Anbieter aufgerufen wird, ob es Reasoning nutzt, ob es adaptives Denken hat, und sein Zeitlimit. Die Anfrage wird daran geprüft und mit 400 abgelehnt, wenn sie etwas braucht, das dem Modell fehlt:

- Bilder, PDFs, Tools, ein JSON-Schema oder Reasoning;
- eine Aufwandsstufe, die das Modell nicht listet, oder adaptives Denken für ein Claude-Modell ohne adaptives Denken;
- eine erzwungene Tool-Wahl (`required`, `any` oder ein benanntes Tool) für ein Modell, das keine annimmt, etwa Claude Fable 5.1, Opus 5.5 und Sonnet 5.5; die Meldung nennt das Feld und den Wert der App selbst;
- eine Temperatur ungleich 1 bei einem Modell, das keine annimmt, es sei denn, Kaeso schaltet das Denken von Claude ein und lässt die Temperatur weg;
- Embeddings von einem Generierungsmodell oder Text von einem Embedding-Modell.

Eine Anfrage in einem OpenAI-Format für ein Claude-Modell ohne Höchstzahl an Ausgabe-Token erhält das Ausgabelimit des Modells. Anthropic Messages verlangt `max_tokens`, daher wird eine Messages-Anfrage ohne das Feld mit 400 abgelehnt, so wie Anthropic sie ablehnt.

## Limits

Jedes zutreffende Limit wird auf einmal geprüft, in einem Schritt in Valkey: die Raten und Budgets des Schlüssels, seines Mitglieds und des Projekts, für alle Modelle oder für dieses. Eine Anfrage über einer Rate oder einem harten Budget wird mit 429 und `Retry-After` abgelehnt. Weiche Budgets lehnen keine Anfragen ab. Ist Valkey nicht erreichbar, wird eine Anfrage unter einem harten Budget mit 503 abgelehnt, und andere Anfragen gehen ungeprüft durch. Lassen sich die Limits des Projekts nicht aus PostgreSQL laden, wird jede Anfrage mit 503 abgelehnt, weil Kaeso nicht erkennen kann, welche Limits gelten.

## Anbieter-Schlüssel

Der Anbieter-Schlüssel des Projekts für den Anbieter des Modells wird gewählt: der dem Projekt zugewiesene Schlüssel, sonst der Standard der Organisation. Fehlt einer, wird die Anfrage mit 400 abgelehnt. Der Schlüssel wird mit dem Hauptschlüssel entschlüsselt und nur im Speicher der Instanz gehalten, höchstens 30 Sekunden lang.

## Codieren und senden

KPP prüft die Anfrage am Format des Anbieters und codiert sie; was dieses Format nicht ausdrücken kann, wird hier abgelehnt, bevor etwas den Anbieter erreicht ([Formatübersetzung](format-translation.md)). Für eine Chat-Completions-App, die über ein anderes Format bedient wird, wird zuerst das Reasoning aus früheren Zügen wieder eingefügt ([Reasoning zwischen Zügen](format-translation.md#reasoning-zwischen-zügen)).

| Modell | Aufgerufene API |
| --- | --- |
| OpenAI | Responses, oder Chat Completions, wenn der Katalog das vorgibt; Embeddings für Embeddings |
| Anthropic | Messages |

Token-Zählungen gehen an `/v1/messages/count_tokens` von Anthropic oder `/v1/responses/input_tokens` von OpenAI. `KAESO_PROVIDERS_OPENAI_BASE_URL` und `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` legen fest, wo die Anbieter liegen.

| Zeitlimit | Einstellung | Standard | Begrenzt |
| --- | --- | --- | --- |
| Verbindung | `KAESO_UPSTREAM_CONNECT_TIMEOUT` | 10 s | Den Verbindungsaufbau zum Anbieter, TCP und TLS |
| Anfrage | `KAESO_UPSTREAM_REQUEST_TIMEOUT` | 600 s | Eine ganze Anfrage, die nicht streamt |
| Erstes Byte | `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | 60 s | Einen Stream, bis der Anbieter antwortet |
| Leerlauf | `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 300 s | Einen Stream, die längste Stille zwischen zwei Chunks |

Ein eigenes Zeitlimit des Modells im Katalog ersetzt die Zeitlimits für Anfrage und erstes Byte. Eine Zeitüberschreitung wird mit 504 beantwortet, ein nicht erreichbarer Anbieter mit 502 ([Aufrufe an Anbieter](../self-hosting/configuration.md#aufrufe-an-anbieter)).

## Antwort

Ein Fehler des Anbieters kommt so an, wie der Anbieter ihn gesendet hat, wenn die App das Format des Anbieters spricht, und behält sonst seine Bedeutung im Format der App ([Fehler des Anbieters](../api/errors.md#fehler-des-anbieters)). Eine Antwort wird in das kanonische Format decodiert und im Format der App codiert, bei einem Stream Event für Event. Eine Antwort wird nie wegen ihres Inhalts abgelehnt: Was das Format der App nicht ausdrücken kann, fehlt in dem, was die App erhält, und bleibt im Anfrageprotokoll erhalten, wenn die Inhaltsprotokollierung eingeschaltet ist.

Antworten tragen `kaeso-model` und `kaeso-provider`. Die Header eines Streams werden gesendet, sobald der Anbieter antwortet. Solange ein Stream offen und still ist, schreibt das Gateway Keepalives, die das SDK der App ignoriert ([lange Streams](../self-hosting/domains-and-tls.md#lange-streams)).

## Wenn ein Stream fehlschlägt

Sind die Header eines Streams gesendet, kann sich sein Status nicht mehr ändern. Ein Stream, der danach fehlschlägt, endet mit einem Fehler-Event im Format der App, und das Anfrageprotokoll verzeichnet ihn als Fehler:

| Ursache | Protokollierter Status | Fehlerart |
| --- | --- | --- |
| Der Anbieter sendet ein Fehler-Event | Der Status des Fehlers, sonst 502 | Die Art des Fehlers |
| Die Verbindung zum Anbieter bricht ab, oder der Stream endet, bevor die Antwort vollständig ist | 502 | `stream_interrupted` |
| Der Anbieter schweigt länger als `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 504 | `timeout` |
| KPP kann ein Event nicht für die App codieren | Der Status des Fehlers, sonst 500 | Die Art des Fehlers |

Er zählt als Fehler in der Nutzung und in `kaeso_stream_failures_total`, während `kaeso_requests_total` ihn als 200 zählt ([Monitoring](../self-hosting/monitoring.md)). Jeder gestreamte Eintrag im Anfrageprotokoll nennt in `stream_progress`, wie weit er kam: die Events des Anbieters, die an die App geschriebenen Bytes und Keepalives und die Zeit bis zu den ersten Antwort-Bytes.

## Wenn die App verschwindet

Schließt eine App die Verbindung, stoppt das Gateway die Anfrage beim Anbieter sofort, sodass der Anbieter aufhört zu generieren. Die Anfrage wird als 499 `client_disconnected` protokolliert.

Einem Stream wird berechnet, was der Anbieter gemeldet hat, plus eine Schätzung des Rests:

- die Eingabe aus der Nutzung, die der Anbieter zu Beginn des Streams meldet, sonst aus seinem Endpunkt zum Zählen von Token, sonst aus der Größe der Anfrage;
- die Ausgabe aus dem empfangenen Inhalt.

Dasselbe gilt für einen Stream, der abbricht. Solche Protokolleinträge enthalten `usage.estimated: true`, und Kosten und Limits verwenden dieselben Zahlen. Schätzungen der Ausgabe fallen bei Reasoning, das der Anbieter nicht streamt, zu niedrig aus.

## Wenn das Gateway herunterfährt

Bei einem Shutdown-Signal nimmt das Gateway keine neuen Verbindungen mehr an und lässt offene Anfragen und Streams für `KAESO_SHUTDOWN_GRACE` (30 Sekunden) weiterlaufen. Eine dann noch offene erhält einen Fehler im Format der App, „Kaeso is restarting“: 503, bei Apps im OpenAI-Format 500, was die SDKs wiederholen; ein bereits gestarteter Stream endet stattdessen mit einem Fehler-Event. Sie wird mit der Fehlerart `shutdown` berechnet und protokolliert, bei einem Stream mit geschätzter Nutzung. Das Gateway wartet bis zu 10 Sekunden darauf, bevor es Nutzung und Protokolle ein letztes Mal schreibt.

## Nutzung und Protokolle

Die Token der Anfrage werden zu den Preisen des Modells zum Zeitpunkt der Anfrage berechnet, dem Schlüssel, dem Mitglied und dem Projekt, und auf ihre Budgets angerechnet. Ein Protokolleintrag verzeichnet die Metadaten, dazu Anfrage und Antwort, wenn die Inhaltsprotokollierung für Organisation und Projekt eingeschaltet ist. Die Protokollierung hält nie eine Anfrage auf: Einträge warten im Speicher und werden stapelweise in ClickHouse geschrieben. Ist der Puffer voll, werden die ältesten Einträge verworfen.
