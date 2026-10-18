---
title: Fehler
description: Was eine fehlgeschlagene Modellanfrage in jedem API-Format zurückgibt und was jeder Status bedeutet.
order: 2
---

Eine fehlgeschlagene Modellanfrage gibt einen HTTP-Status und einen Fehler im Format zurück, das Ihre App aufgerufen hat. So löst das SDK seinen gewohnten Fehler aus. Jede Antwort trägt den Header `kaeso-request-id`, und die [Anfrageprotokolle](../console/request-logs.md) führen einen Eintrag unter dieser ID. Die Fehler der Platform API stehen auf [ihrer eigenen Seite](platform-api.md#fehler).

Diese Anfrage nennt ein Modell, das nicht im Katalog des Projekts steht, deshalb antwortet Kaeso mit 404:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-9", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
import openai

client = openai.OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

try:
    client.responses.create(model="gpt-9", input="Write a haiku about gateways.")
except openai.APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

try {
  await client.responses.create({ model: "gpt-9", input: "Write a haiku about gateways." });
} catch (error) {
  if (!(error instanceof OpenAI.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

## OpenAI-Formate

Chat Completions, Responses und Embeddings teilen einen Fehler-Body:

```json
{
  "error": {
    "message": "The model `gpt-9` is not in this project's model catalogue. A project admin can add it under the project's models.",
    "type": "invalid_request_error",
    "param": "model",
    "code": null
  }
}
```

| Status | `type` | `code` |
| --- | --- | --- |
| 400 | `invalid_request_error` | `context_length_exceeded` oder `content_policy_violation`, wo zutreffend |
| 401 | `invalid_request_error` | |
| 403 | `request_forbidden` | |
| 404 | `invalid_request_error` | |
| 405 | `invalid_request_error` | |
| 413 | `invalid_request_error` | |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 429 | `insufficient_quota` | `insufficient_quota`, wenn der Anbieter aus Abrechnungsgründen ablehnt |
| 500, 502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

Der Body hat kein Feld für die ID der Anfrage. Lesen Sie sie deshalb aus dem Header `kaeso-request-id`. `Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet. Hat die Wartezeit Sekundenbruchteile, nennt `retry-after-ms` sie in Millisekunden.

## Anthropic-Format

Dieselbe Anfrage mit dem Anthropic-SDK:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-9", "max_tokens": 1024, "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

```python tab="Python"
import os
import anthropic

client = anthropic.Anthropic(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])

try:
    client.messages.create(
        model="gpt-9",
        max_tokens=1024,
        messages=[{"role": "user", "content": "Write a haiku about gateways."}],
    )
except anthropic.APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({ baseURL: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

try {
  await client.messages.create({
    model: "gpt-9",
    max_tokens: 1024,
    messages: [{ role: "user", content: "Write a haiku about gateways." }],
  });
} catch (error) {
  if (!(error instanceof Anthropic.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

Der Fehler-Body trägt die ID der Anfrage in `request_id`, auch wenn der Fehler vom Anbieter kommt:

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "The model `gpt-9` is not in this project's model catalogue. A project admin can add it under the project's models."
  },
  "request_id": "req_…"
}
```

| Status | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 402 | `billing_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| Jeder andere | `api_error` |

`Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet.

## Ursachen

Die Meldung nennt die Ursache und was zu tun ist. Diese Ursachen stehen hinter jedem Status:

| Status | Ursache |
| --- | --- |
| 400 | Kaeso kann die Anfrage nicht lesen, oder das Modell nimmt eine Einstellung oder ein Feld der Anfrage nicht an. |
| 400 | Der Kontext ist zu lang für das Modell. |
| 400 | Mehrere Anbieter im Projekt bieten die Modell-ID an. Senden Sie stattdessen `provider/model`. |
| 400 | Das Projekt hat keinen Anbieter-Schlüssel für den Anbieter des Modells. |
| 401 | Der Schlüssel fehlt, ist unbekannt, deaktiviert, archiviert oder abgelaufen. |
| 401 | Das Mitglied oder Dienstkonto des Schlüssels ist nicht mehr aktiv, oder sein Projekt oder seine Organisation ist nicht nutzbar. |
| 403 | Der Schlüssel hat nur Plattformzugriff, oder es ist ein Plattform-Schlüssel. Beide rufen keine Modelle auf. |
| 403 | Dem Mitglied oder Dienstkonto des Schlüssels fehlt das Recht `api.request`, oder es darf dieses Modell nicht verwenden. |
| 404 | Das Modell steht nicht im Katalog des Projekts, oder der Anbieter stellt es dem Anbieter-Schlüssel des Projekts nicht bereit. |
| 404 | Den Pfad gibt es nicht. |
| 405 | Der Pfad nimmt diese Methode nicht an. |
| 413 | Der Body ist größer als `KAESO_BODY_LIMITS_MODEL_API`, standardmäßig 32 MiB. |
| 429 | Ein Ratenlimit oder Budget ist erreicht, und die Meldung nennt es. Bei einem aufgebrauchten Budget nennt sie, wann es zurückgesetzt wird. |
| 429 | Von Ihrer Adresse kamen zu viele Anfragen mit unbekannten Schlüsseln. |
| 500 | Kaeso kann den Anbieter-Schlüssel nicht entschlüsseln, oder der Anbieter des Modells ist in dieser Installation nicht verfügbar. |
| 502 | Kaeso erreicht den Anbieter nicht, oder die Übertragung ist fehlgeschlagen. |
| 503 | Kaeso kann den Katalog, die Schlüssel, die Anbieter-Schlüssel oder die Limits nicht laden, oder Kaeso startet gerade neu. Die OpenAI-Formate antworten darauf mit 500. |
| 503 | Der Anbieter ist überlastet, in den OpenAI-Formaten. |
| 504 | Der Anbieter hat nicht rechtzeitig geantwortet. |
| 529 | Der Anbieter ist überlastet, im Anthropic-Format. |

Hat ein Stream begonnen, kann sich sein Status nicht mehr ändern. Scheitert der Anbieter oder beendet er den Stream zu früh, endet der Stream mit einem Fehler-Event im Format Ihrer App.

## Fehler des Anbieters

Sprechen Ihre App und der Anbieter dasselbe Format, gibt Kaeso den Fehler des Anbieters mit seinem Status weiter. Ein Anthropic-Fehler erhält zusätzlich Kaesos ID in `request_id`.

Andernfalls schreibt Kaeso den Fehler im Format Ihrer App und behält seine Bedeutung:

| Der Anbieter | Ihre App erhält |
| --- | --- |
| Lehnt den Anbieter-Schlüssel ab | 401 oder 403 |
| Lehnt die Anfrage wegen seines Ratenlimits ab | 429 |
| Lehnt aus Abrechnungsgründen ab | 402 im Anthropic-Format, 429 `insufficient_quota` in den OpenAI-Formaten |
| Ist überlastet | 529 im Anthropic-Format, 503 in den OpenAI-Formaten |
| Scheitert mit einem anderen Serverfehler | Seinen eigenen 5xx-Status |

Das Anfrageprotokoll behält die eigene Meldung des Anbieters. Kaeso sendet jede Anfrage einmal und wiederholt sie nie, weder an dieses noch an ein anderes Modell. Die SDKs wiederholen selbst, wo der Fehler es zulässt.

### Modelle, die der Anbieter nicht bereitstellt

Antwortet der Anbieter auf ein Modell aus dem Katalog mit 404, fragt Kaeso den Anbieter, ob er dieses Modell dem Anbieter-Schlüssel des Projekts noch bereitstellt. Ist das nicht so, erhält Ihre App 404 mit einer Meldung, die Anbieter und Modell nennt. Der Katalog zeigt das Modell dann als **Nicht verfügbar** ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)), außer der Anbieter-Schlüssel des Projekts hat sich während der Anfrage geändert.

Führt der Anbieter das Modell noch, erhält Ihre App das 404 des Anbieters so, wie er es gesendet hat: Es betraf etwas anderes, etwa eine Datei-ID. Kann der Anbieter keine Auskunft geben, weil er keine Modellliste hat oder die Liste scheitert, erhält Ihre App ebenfalls das 404 des Anbieters, wie er es gesendet hat. In beiden Fällen bleibt der Katalog unverändert.
