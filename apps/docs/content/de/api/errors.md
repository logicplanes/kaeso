---
title: Fehler
description: Was eine fehlgeschlagene Modellanfrage zurückgibt, in jedem API-Format.
order: 2
---

Eine fehlgeschlagene Modellanfrage erhält einen HTTP-Status und einen Fehler-Body im Format, das die App aufgerufen hat, sodass das SDK seinen gewohnten Fehler auslöst. Die Meldung sagt, was schiefging und was zu tun ist.

## OpenAI-Formate

Chat Completions, Responses und Embeddings teilen eine Fehlerform:

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

`Retry-After` steht in ganzen Sekunden, aufgerundet; hat die Wartezeit einen Sekundenbruchteil, nennt `retry-after-ms` sie in Millisekunden.

## Anthropic-Format

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit reached: this key's request rate of 60 requests per minute; retry in 12 seconds."
  }
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
| andere | `api_error` |

## Fehler des Anbieters

Sprechen App und Anbieter dasselbe Format, kommt der Fehler eines Anbieters so zurück, wie der Anbieter ihn gesendet hat, mit seinem Status. Andernfalls behält er seine Bedeutung im Format der App: Ein Anbieter-Schlüssel, den der Anbieter ablehnt, kommt als 401 oder 403 zurück, das eigene Ratenlimit des Anbieters als 429, seine Ablehnung aus Abrechnungsgründen als 402 oder 429 und seine übrigen Serverfehler mit ihrem 5xx-Status. Das Anfrageprotokoll behält die eigene Meldung des Anbieters.

## Warum Anfragen fehlschlagen

| Status | Ursache |
| --- | --- |
| 400 | Die Anfrage ist nicht lesbar, ein Feld lässt sich nicht zum Anbieter übertragen, der Kontext ist zu lang, eine Modell-ID wird von mehreren Anbietern im Projekt bedient, oder für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet |
| 401 | Kein Schlüssel, oder der Schlüssel ist unbekannt, deaktiviert, archiviert oder abgelaufen, sein Mitglied oder Dienstkonto ist nicht mehr aktiv, oder sein Projekt oder seine Organisation ist nicht nutzbar |
| 403 | Der Schlüssel hat nur Plattformzugriff, ist ein Plattform-Schlüssel einer Organisation oder darf keine Modelle aufrufen (`api.request`), oder der Schlüssel oder sein Mitglied darf dieses Modell nicht nutzen |
| 404 | Das Modell steht nicht im Katalog des Projekts, oder der Pfad wird nicht bedient |
| 405 | Der Pfad nimmt diese Methode nicht an |
| 413 | Der Body ist größer als `KAESO_BODY_LIMITS_MODEL_API`, standardmäßig 32 MiB |
| 429 | Ein Ratenlimit oder Budget, oder zu viele unbekannte Schlüssel von derselben Adresse; `Retry-After` sagt, wann ein neuer Versuch möglich ist, außer bei einem aufgebrauchten Budget, dessen Meldung sagt, wann es neu beginnt |
| 500 | Ein Anbieter-Schlüssel lässt sich nicht entschlüsseln, oder der Anbieter des Modells ist in dieser Installation nicht verfügbar; für Apps im OpenAI-Format auch die Ursachen von 503 weiter unten |
| 502 | Der Anbieter war nicht erreichbar, oder die Übertragung schlug fehl |
| 503 | Für Apps im Anthropic-Format: Katalog, API-Schlüssel oder Anbieter-Schlüssel lassen sich nicht laden, ein hartes Budget lässt sich nicht prüfen, weil Valkey ausgefallen ist, oder das Gateway startet neu; für Apps im OpenAI-Format: Der Anbieter ist überlastet |
| 504 | Der Anbieter hat nicht rechtzeitig geantwortet |
| 529 | Der Anbieter ist überlastet, für Apps im Anthropic-Format |

Ein bereits gestarteter Stream kann seinen Status nicht mehr ändern: Schlägt der Anbieter fehl oder beendet er den Stream, bevor die Antwort vollständig ist, endet der Stream stattdessen mit einem Fehler-Event im Format der App.

Jede Antwort trägt `kaeso-request-id`. Das Anfrageprotokoll hat dieselbe ID, mit der eigenen Meldung des Anbieters, sofern es eine gab.
