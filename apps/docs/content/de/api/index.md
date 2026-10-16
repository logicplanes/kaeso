---
title: API
description: Die beiden APIs einer Kaeso-Installation und wie sie aufgerufen werden.
order: 4
---

Eine Kaeso-Installation stellt auf ihrer Domain zwei APIs bereit:

| API | Pfad | Für | Authentifizierung |
| --- | --- | --- | --- |
| Modell-API | `/v1` | Apps, die Modelle aufrufen, in den Formaten von OpenAI und Anthropic | Ein API-Schlüssel eines Projekts mit Inferenzzugriff |
| Platform API | `/platform/v1` | Die Konsole und Skripte, die Organisationen, Projekte, Schlüssel, Modelle, Limits und Protokolle verwalten | Eine Sitzung, ein API-Schlüssel eines Projekts mit Plattformzugriff oder ein Plattform-Schlüssel der Organisation |

| Seite | Inhalt |
| --- | --- |
| [Modell-API](model-api.md) | Endpunkte, Basis-URLs, Schlüssel, Modellnamen, Streaming, Antwort-Header |
| [Fehler](errors.md) | Was eine fehlgeschlagene Modellanfrage zurückgibt, in jedem Format |
| [Platform API](platform-api.md) | Konventionen, Authentifizierung, Fehler, ein Durchgang mit curl, Bereiche, Rollen und Rechte, der Playground, das OpenAPI-Dokument |
