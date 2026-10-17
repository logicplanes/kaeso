---
title: API
description: Die beiden APIs einer Kaeso-Installation und wie sie aufgerufen werden.
order: 4
---

Eine Kaeso-Installation stellt auf ihrer Domain zwei APIs bereit:

| API | Pfad | Aufgerufen von | Authentifizierung |
| --- | --- | --- | --- |
| [Modell-API](model-api.md) | `/v1` | Apps, über die offiziellen SDKs von OpenAI und Anthropic | Ein Projekt-API-Schlüssel mit Inferenzzugriff |
| [Platform API](platform-api.md) | `/platform/v1` | Der Konsole, Skripten und CI, die Organisationen, Projekte, Schlüssel, Modelle, Limits und Protokolle verwalten | Eine Konsolensitzung, ein Projekt-API-Schlüssel mit Plattformzugriff oder ein Plattform-Schlüssel der Organisation |

[Fehler](errors.md) beschreibt, was eine fehlgeschlagene Modellanfrage in jedem Format zurückgibt. Die Fehler der Platform API und ihr OpenAPI-Dokument stehen auf [ihrer Seite](platform-api.md).
