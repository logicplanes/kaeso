---
title: Fehler
description: Was eine fehlgeschlagene Modellanfrage zurückgibt, in jedem API-Format.
order: 2
---

Eine fehlgeschlagene Modellanfrage erhält einen HTTP-Status und einen Fehler-Body im Format, das die App aufgerufen hat, sodass das SDK seinen gewohnten Fehler auslöst. Die Meldung sagt, was schiefging und was zu tun ist. Jede Antwort trägt `kaeso-request-id`, und das Anfrageprotokoll hat einen Eintrag unter derselben ID, mit der eigenen Meldung des Anbieters, wo es eine gab. Die Platform API hat [eigene Fehler](platform-api.md#fehler).

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

`Retry-After` steht in ganzen Sekunden, aufgerundet; hat die Wartezeit Sekundenbruchteile, nennt `retry-after-ms` sie in Millisekunden. Die Form hat kein Feld für die ID der Anfrage; sie steht im Header `kaeso-request-id`.

## Anthropic-Format

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit reached: this key's request rate of 60 requests per minute; retry in 12 seconds."
  },
  "request_id": "req_…"
}
```

`request_id` ist dieselbe ID wie in `kaeso-request-id`, auch bei einem Fehler, der vom Anbieter kommt.

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

## Warum Anfragen fehlschlagen

| Status | Ursache |
| --- | --- |
| 400 | Die Anfrage ist nicht lesbar; das Modell nimmt eine verwendete Einstellung nicht an; ein Feld lässt sich nicht zum Anbieter übertragen; der Kontext ist zu lang; mehrere Anbieter im Projekt bieten die Modell-ID an; für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet |
| 401 | Kein Schlüssel; oder der Schlüssel ist unbekannt, deaktiviert, archiviert oder abgelaufen; sein Mitglied oder Dienstkonto ist nicht mehr aktiv; sein Projekt oder seine Organisation ist nicht nutzbar |
| 403 | Der Schlüssel hat nur Plattformzugriff, ist ein Plattform-Schlüssel der Organisation oder darf keine Modelle aufrufen (`api.request`); oder der Schlüssel oder sein Mitglied darf dieses Modell nicht verwenden |
| 404 | Das Modell ist nicht im Katalog des Projekts, der Anbieter stellt es dem Anbieter-Schlüssel des Projekts nicht bereit, oder den Pfad gibt es nicht |
| 405 | Der Pfad nimmt diese Methode nicht an |
| 413 | Der Body ist größer als `KAESO_BODY_LIMITS_MODEL_API`, standardmäßig 32 MiB |
| 429 | Ein Ratenlimit oder Budget, oder zu viele unbekannte Schlüssel von derselben Adresse. `Retry-After` sagt, wann es wieder geht, außer bei einem aufgebrauchten Budget, dessen Meldung nennt, wann es zurückgesetzt wird |
| 500 | Ein Anbieter-Schlüssel lässt sich nicht entschlüsseln, oder der Anbieter des Modells ist in dieser Installation nicht verfügbar; in den OpenAI-Formaten zusätzlich alle 503-Ursachen unten |
| 502 | Der Anbieter ist nicht erreichbar, oder die Übertragung ist fehlgeschlagen |
| 503 | Im Anthropic-Format: Katalog, API-Schlüssel oder Anbieter-Schlüssel lassen sich nicht laden, ein hartes Budget ist nicht prüfbar, weil Valkey ausgefallen ist, oder das Gateway startet neu. In den OpenAI-Formaten: Der Anbieter ist überlastet |
| 504 | Der Anbieter hat nicht rechtzeitig geantwortet |
| 529 | Der Anbieter ist überlastet, im Anthropic-Format |

Ein begonnener Stream kann seinen Status nicht mehr ändern: Scheitert der Anbieter oder beendet er den Stream vor dem Ende der Antwort, endet der Stream stattdessen mit einem Fehler-Event im Format der App.

## Fehler des Anbieters

Sprechen App und Anbieter dasselbe Format, kommt ein Fehler des Anbieters so zurück, wie er ihn gesendet hat, mit seinem Status; ein Anthropic-Fehler erhält Kaesos Anfrage-ID in `request_id`. Andernfalls behält der Fehler seine Bedeutung im Format der App: Ein Anbieter-Schlüssel, den der Anbieter ablehnt, kommt als 401 oder 403 zurück, das Ratenlimit des Anbieters als 429, seine Ablehnung aus Abrechnungsgründen als 402 oder 429 und seine übrigen Serverfehler mit ihrem 5xx-Status. Das Anfrageprotokoll behält die eigene Meldung des Anbieters.

### Ein Modell, das der Anbieter nicht bereitstellt

Antwortet der Anbieter auf ein Katalogmodell mit 404, prüft Kaeso, ob der Anbieter dieses Modell dem Anbieter-Schlüssel des Projekts noch bereitstellt. Kaeso fragt dazu `GET /v1/models/{model}` beim Anbieter ab und liest, weil ein 404 dort auch ein fehlender Endpunkt sein kann, zusätzlich die vollständige Liste `GET /v1/models`.

| Die Liste | Was die App erhält | Der Katalog |
| --- | --- | --- |
| Antwortet ohne das Modell, weil es eingestellt ist oder der Schlüssel keinen Zugriff hat | 404 mit einer Meldung, die Anbieter und Modell nennt; `param` ist in den OpenAI-Formaten `model` | Das Modell wird als **Nicht verfügbar** markiert ([Verfügbarkeit](../console/models-and-aliases.md#verfügbarkeit)), außer der Anbieter-Schlüssel des Projekts hat sich inzwischen geändert |
| Enthält das Modell | Das 404 des Anbieters, wie gesendet: Es betraf etwas anderes, etwa eine Datei-ID | Unverändert |
| Antwortet nicht, wie bei manchen OpenAI-kompatiblen Diensten | Das 404 des Anbieters, wie gesendet | Unverändert |

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "Anthropic does not serve the model `claude-sonnet-4-5` to this project's provider key; check the key's access to it, or remove the model from the catalogue."
  },
  "request_id": "req_…"
}
```

Kaeso sendet die Anfrage nie erneut, weder an dieses noch an ein anderes Modell.
