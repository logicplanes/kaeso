---
title: Fehler
description: Was eine fehlgeschlagene Modellanfrage in jedem API-Format zurückgibt und was jeder Status bedeutet.
order: 6
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
from openai import APIStatusError, OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.responses.create(
        model="gpt-9",
        input="Write a haiku about gateways.",
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.responses.create({
    model: "gpt-9",
    input: "Write a haiku about gateways.",
  });
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
    "message": "The model `gpt-9` does not exist or this key may not use it.",
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
| 500, 502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

Der Body hat kein Feld für die ID der Anfrage. Lesen Sie sie deshalb aus dem Header `kaeso-request-id`. `Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet. Hat die Wartezeit Sekundenbruchteile, nennt `retry-after-ms` sie in Millisekunden.

## Anthropic-Format

Dieselbe Anfrage mit dem Anthropic SDK:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "gpt-9",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic, APIStatusError

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.messages.create(
        model="gpt-9",
        max_tokens=1024,
        messages=[
            {"role": "user", "content": "Write a haiku about gateways."}
        ],
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

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
    "message": "The model `gpt-9` does not exist or this key may not use it."
  },
  "request_id": "req_…"
}
```

| Status | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| Jeder andere | `api_error` |

`Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet.

## Gemini-Format

Dieselbe Anfrage mit dem Google Gen AI SDK:

```bash tab="curl"
curl -i https://kaeso.example.com/v1beta/models/gpt-9:generateContent \
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
from google.genai import errors

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com"},
)

try:
    client.models.generate_content(
        model="gpt-9",
        contents="Write a haiku about gateways.",
    )
except errors.APIError as error:
    print(error.code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import { ApiError, GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

try {
  await ai.models.generateContent({
    model: "gpt-9",
    contents: "Write a haiku about gateways.",
  });
} catch (error) {
  if (!(error instanceof ApiError)) throw error;
  console.log(error.status, error.message);
}
```

Der Fehler-Body folgt Googles Form. Er hat kein Feld für die ID der Anfrage. Lesen Sie sie deshalb aus dem Header `kaeso-request-id`:

```json
{
  "error": {
    "code": 404,
    "message": "The model `gpt-9` does not exist or this key may not use it.",
    "status": "NOT_FOUND"
  }
}
```

| Status | `error.status` |
| --- | --- |
| 400 | `INVALID_ARGUMENT` |
| 401 | `UNAUTHENTICATED` |
| 403 | `PERMISSION_DENIED` |
| 404 | `NOT_FOUND` |
| 405, 413 | `INVALID_ARGUMENT` |
| 429 | `RESOURCE_EXHAUSTED` |
| 500, 502 | `INTERNAL` |
| 503 | `UNAVAILABLE`, oder `INTERNAL`, wenn Kaeso seine eigenen Daten nicht laden kann oder neu startet |
| 504 | `DEADLINE_EXCEEDED` |

`Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet. Bei 429 oder 503 mit bekannter Wartezeit steht sie auch in `details`, als `RetryInfo` mit `retryDelay`, etwa `"23s"`.

## ElevenLabs-Format

Derselbe Fehler mit dem offiziellen ElevenLabs SDK, das den Schlüssel in `xi-api-key` sendet:

```bash tab="curl"
curl -i "https://kaeso.example.com/v1/text-to-speech/21m00Tcm4TlvDq8ikWAM" \
  -H "xi-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"text": "Write a haiku about gateways.", "model_id": "gpt-9"}'
```

```python tab="Python"
import os
from elevenlabs import ElevenLabs
from elevenlabs.core.api_error import ApiError

client = ElevenLabs(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    audio = client.text_to_speech.convert(
        voice_id="21m00Tcm4TlvDq8ikWAM",
        text="Write a haiku about gateways.",
        model_id="gpt-9",
    )
    b"".join(audio)
except ApiError as error:
    print(error.status_code, error.body)
```

```ts tab="TypeScript"
import { ElevenLabsClient, ElevenLabsError } from "@elevenlabs/elevenlabs-js";

const client = new ElevenLabsClient({
  baseUrl: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.textToSpeech.convert("21m00Tcm4TlvDq8ikWAM", {
    text: "Write a haiku about gateways.",
    modelId: "gpt-9",
  });
} catch (error) {
  if (!(error instanceof ElevenLabsError)) throw error;
  console.log(error.statusCode, error.body);
}
```

Der Fehler-Body folgt der dokumentierten Form von ElevenLabs, mit demselben Wert in `code` und `status`. Lesen Sie die ID der Anfrage aus dem Header `kaeso-request-id`:

```json
{
  "detail": {
    "type": "not_found",
    "code": "not_found",
    "message": "The model `gpt-9` does not exist or this key may not use it.",
    "status": "not_found",
    "param": "model"
  }
}
```

| Status | `detail.type` | `detail.code` |
| --- | --- | --- |
| 400 | `invalid_request` | `invalid_parameters`, oder `unsupported_parameter` für ein Feld, das der Anbieter des Modells nicht nimmt |
| 401 | `authentication_error` | `invalid_api_key` |
| 402 | `payment_required` | `insufficient_credits` |
| 403 | `authorization_error` | `permission_denied` |
| 404 | `not_found` | `not_found` |
| 413 | `invalid_request` | `request_too_large` |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 503 | `service_unavailable` | `system_busy` |
| 500, 502, 504 | `internal_error` | `internal_error` |

Einen Fehler, den ElevenLabs selbst sendet, erhält die App so, wie ElevenLabs ihn gesendet hat, mit seinem Status. `Retry-After` nennt die Wartezeit in ganzen Sekunden, aufgerundet. Ein Pfad, den Kaeso nicht bedient, etwa `/v1/dubbing`, erhält 404 in dieser Form ([ElevenLabs API](elevenlabs.md#was-kaeso-ablehnt)).

## Ursachen

Die Meldung nennt die Ursache und was zu tun ist. Diese Ursachen stehen hinter jedem Status:

| Status | Ursache |
| --- | --- |
| 400 | Kaeso kann die Anfrage nicht lesen, oder das Modell nimmt eine Einstellung oder ein Feld der Anfrage nicht an. |
| 400 | Die Anfrage enthält mehr Nachrichten, Inhaltsblöcke in einer Nachricht, Werkzeuge oder Embedding-Eingaben, als Kaeso annimmt ([die Anfrage](../concepts/request-flow.md#die-anfrage)). |
| 400 | Die Anfrage zählt Token, und der Anbieter des Modells hat keinen Endpunkt zum Zählen von Token. |
| 400 | Der Kontext ist zu lang für das Modell. |
| 400 | Mehrere Anbieter im Projekt bieten die Modell-ID an. Senden Sie stattdessen `provider/model`. |
| 400 | Das Projekt hat keinen Anbieter-Schlüssel für den Anbieter des Modells. |
| 401 | Der Schlüssel fehlt, ist unbekannt, deaktiviert, archiviert oder abgelaufen. |
| 401 | Das Mitglied oder Dienstkonto des Schlüssels ist nicht mehr aktiv, oder sein Projekt oder seine Organisation ist nicht nutzbar. |
| 403 | Der Schlüssel hat nur Plattformzugriff, oder es ist ein Plattform-Schlüssel. Beide rufen keine Modelle auf. |
| 403 | Dem Mitglied oder Dienstkonto des Schlüssels fehlt das Recht `api.request`. |
| 403 | Ein Ausgabenbudget gilt, und die Anfrage nutzt eine Funktion, die der Anbieter über Tokens hinaus abrechnet, etwa ein Werkzeug des Anbieters oder `service_tier: priority`, oder ein Modell ohne Preis ([Limits und Budgets](../console/limits-and-budgets.md#funktionen-die-ein-ausgabenbudget-nicht-bepreisen-kann)). |
| 404 | Das Modell steht nicht im Katalog des Projekts, der Schlüssel oder sein Mitglied darf es nicht verwenden, oder der Anbieter stellt es dem Anbieter-Schlüssel des Projekts nicht bereit. Ein Modell, das der Schlüssel nicht verwenden darf, antwortet wie eines, das es nicht gibt, damit ein Schlüssel nicht erfährt, welche anderen Modelle das Projekt hat. |
| 404 | Den Pfad gibt es nicht. |
| 405 | Der Pfad nimmt diese Methode nicht an. |
| 413 | Der Body ist größer als `KAESO_BODY_LIMITS_MODEL_API`, standardmäßig 32 MiB. |
| 429 | Ein Ratenlimit oder Budget ist erreicht, oder die Anfrage könnte mehr kosten, als davon übrig ist, und die Meldung nennt es. Bei einem aufgebrauchten Budget nennt sie, wann es zurückgesetzt wird. |
| 429 | Von Ihrer Adresse kamen zu viele Anfragen mit unbekannten Schlüsseln. |
| 500 | Kaeso kann den Anbieter-Schlüssel nicht entschlüsseln, oder der Anbieter des Modells ist in dieser Installation nicht verfügbar. |
| 500 | Der Anbieter des Modells ist ein eigener Anbieter, und die Installation hat eigene Anbieter ausgeschaltet. |
| 502 | Kaeso erreicht den Anbieter nicht, oder die Übertragung ist fehlgeschlagen. |
| 502 | Ein eigener Anbieter liegt an einer Adresse, die die Installation nicht erlaubt, oder hat in einer Form geantwortet, die sein Format nicht lesen kann. |
| 502 | Der Anbieter hat den Anbieter-Schlüssel des Projekts abgelehnt, die Anfrage damit nicht erlaubt, oder ihn aus Abrechnungs- oder Kontingentgründen abgelehnt. |
| 503 | Kaeso kann den Katalog, die Schlüssel, die Anbieter-Schlüssel oder die Limits nicht laden, oder Kaeso startet gerade neu. Die OpenAI-Formate antworten darauf mit 500. |
| 503 | Der Anbieter ist überlastet, in den OpenAI-Formaten und im Gemini-Format. |
| 504 | Der Anbieter hat nicht rechtzeitig geantwortet. |
| 529 | Der Anbieter ist überlastet, im Anthropic-Format. |

Hat ein Stream begonnen, kann sich sein Status nicht mehr ändern. Scheitert der Anbieter oder beendet er den Stream zu früh, endet der Stream mit einem Fehler-Event im Format Ihrer App.

## Fehler des Anbieters

Sprechen Ihre App und der Anbieter dasselbe Format, gibt Kaeso den Fehler des Anbieters mit seinem Status weiter, außer wenn der Anbieter den Anbieter-Schlüssel des Projekts abgelehnt hat. Ein Anthropic-Fehler erhält zusätzlich Kaesos ID in `request_id`. In jedem Fehler ersetzt Kaeso alles, was wie ein Schlüssel oder eine Konto-, Benutzer-, Projekt- oder Organisations-ID aussieht, durch `[redacted]`. Das gilt auch für gestreamte und für ein anderes Format neu geschriebene Fehler.

Andernfalls schreibt Kaeso den Fehler im Format Ihrer App und behält seine Bedeutung:

| Der Anbieter | Ihre App erhält |
| --- | --- |
| Lehnt den Anbieter-Schlüssel ab | 502, in jedem Format |
| Erlaubt die Anfrage mit dem Schlüssel nicht, etwa für ein Modell, das sein Tarif ausschließt | 502, in jedem Format |
| Lehnt die Anfrage wegen seines Ratenlimits ab | 429 |
| Lehnt aus Abrechnungs- oder Kontingentgründen ab | 502, in jedem Format |
| Ist überlastet | 529 im Anthropic-Format, 503 in den OpenAI-Formaten und im Gemini-Format |
| Scheitert mit einem anderen Serverfehler | Seinen eigenen 5xx-Status. Eine Gemini-App erhält 500, bei einer Zeitüberschreitung 504 |

Ein abgelehnter Anbieter-Schlüssel ist ein Problem der Installation, nicht Ihrer App. Das 401, 402 oder 403 des Anbieters ließe ein SDK einen falschen Kaeso-Schlüssel melden. Die Meldung des Anbieters kann außerdem einen Teil des Anbieter-Schlüssels oder die Organisations-ID beim Anbieter enthalten.

Stattdessen erhält Ihre App einen Anbieterfehler mit der Meldung „The provider refused this project's provider key. Ask an organization admin to check it under Providers.“, oder derselben mit „for billing or quota reasons“. Beim 403 des Anbieters, das mit einem funktionierenden Schlüssel kommt, der diese Anfrage nicht stellen darf, lautet die Meldung „The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers.“ Ein Organisations-Admin prüft den Schlüssel unter **Anbieter**.

Das Anfrageprotokoll behält die eigene Meldung des Anbieters, wobei alles, was wie ein Schlüssel, eine Projekt- oder Organisations-ID aussieht, durch `[redacted]` ersetzt ist. Kaeso sendet jede Anfrage einmal und wiederholt sie nie, weder an dieses noch an ein anderes Modell. Die SDKs wiederholen selbst, wo der Fehler es zulässt.

### Eigene Anbieter

Ein [eigener Anbieter](../console/custom-providers.md) erhält Meldungen, die ihn bei seiner ID nennen, damit Sie Probleme Ihres eigenen Servers von denen eines Anbieters unterscheiden:

| Was geschah | Status | Meldung |
| --- | --- | --- |
| Kaeso kann sich nicht mit ihm verbinden | 502 | „Kaeso cannot reach the custom provider `…`; check its base URL.“ |
| Seine Adresse ist nicht erlaubt | 502 | „The custom provider `…` is at an address this install does not allow; ask its operator.“ |
| Er hat den Schlüssel des Projekts abgelehnt | 502 | „The custom provider `…` refused this project's key; ask an organization admin to check it under Providers.“ |
| Er will einen Schlüssel, nimmt in Kaeso aber keinen | 502 | „The custom provider `…` asks for a key; an organization admin sets how it signs in under Providers.“ |
| Seine Antwort ist nicht in seinem Format | 502 | „The custom provider `…` answered in a form `…` cannot read; check its API format.“ |

Antwortet der Anbieter mit einem Fehler in seinem Format, erhält Ihre App diesen Fehler wie bei jedem Anbieter. Das Anfrageprotokoll behält, was der Anbieter gesagt hat.

### Modelle, die der Anbieter nicht bereitstellt

Antwortet der Anbieter auf ein Modell aus dem Katalog mit 404, fragt Kaeso den Anbieter, ob er dieses Modell dem Anbieter-Schlüssel des Projekts noch bereitstellt. Ist das nicht so, erhält Ihre App 404 mit einer Meldung, die Anbieter und Modell nennt. Ein 410, das Anbieter für ein eingestelltes Modell senden, zählt wie ein 404. In einem anderen Format als dem des Anbieters erhält Ihre App es als 404.

Der Katalog zeigt das Modell dann als **Nicht verfügbar** ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)), außer der Anbieter-Schlüssel des Projekts hat sich während der Anfrage geändert. Spätere Anfragen nach dem Modell beantwortet Kaeso dann mit demselben 404, ohne den Anbieter aufzurufen, bis auf eine Anfrage alle fünf Minuten. Die erste Antwort und jede spätere tragen dieselbe Meldung, ebenso das Anfrageprotokoll.

Führt der Anbieter das Modell noch, erhält Ihre App das 404 des Anbieters so, wie er es gesendet hat: Es betraf etwas anderes, etwa eine Datei-ID. Kann der Anbieter keine Auskunft geben, weil er keine Modellliste hat oder die Liste scheitert, erhält Ihre App ebenfalls das 404 des Anbieters, wie er es gesendet hat. In beiden Fällen bleibt der Katalog unverändert.
