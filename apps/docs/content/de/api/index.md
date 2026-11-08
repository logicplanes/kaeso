---
title: API
description: Modelle aus Ihren Apps aufrufen und Ihre Installation über HTTP verwalten.
order: 4
---

Jede Installation stellt auf ihrer Domain zwei APIs bereit. Apps rufen Modelle über die Modell-API unter `/v1` und `/v1beta` mit einem API-Schlüssel auf, und Skripte verwalten die Installation über die Platform API unter `/platform/v1`. Agenten erreichen die Platform API über den MCP-Server unter `/mcp`, und Clients rufen die Tools eines Projekts über sein MCP-Portal unter `/mcp/<Projekt-ID>` auf.

## Nächste Schritte

- [Modell-API](model-api.md): Modelle mit den SDKs von OpenAI, Anthropic und Google Gen AI aufrufen und Endpunkte, Header und Modellnamen nachschlagen.
- [Bilder-API](images.md): Bilder mit der Images API von OpenAI bei jedem Anbieter erzeugen und bearbeiten, der Bilder macht.
- [Audio-API](audio.md): Sprache erzeugen und Audio transkribieren oder übersetzen, mit der Audio-API von OpenAI bei jedem Anbieter, der sie anbietet.
- [Fehler](errors.md): Nachlesen, was eine fehlgeschlagene Modellanfrage in jedem Format zurückgibt und warum sie fehlschlug.
- [Platform API](platform-api.md): Authentifizierung, Konventionen, Fehler und Routen zum Verwalten einer Installation nachschlagen.
- [Mit der Platform API automatisieren](automation.md): Ein Projekt, ein Modell und den Schlüssel einer App per Skript einrichten.
- [MCP-Server](mcp.md): Coding-Agenten verbinden, festlegen, was sie dürfen, und ihre Tools und Anmeldung nachschlagen.
- [MCP-Portal-API](mcp-portal.md): Rufen Sie die Tools eines Projekts über MCP auf, und schlagen Sie Anmeldung, Versionen und Fehler des Portals nach.
