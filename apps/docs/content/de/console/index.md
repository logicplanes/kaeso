---
title: Die Konsole verwenden
description: Verwalten Sie Organisationen, Projekte, Schlüssel und Modelle in der Konsole, der Weboberfläche Ihrer Installation.
order: 3
---

Die Konsole läuft unter der Domain Ihrer Installation, etwa `https://kaeso.example.com`. Melden Sie sich mit Ihrer E-Mail-Adresse an; was die Anmeldeseite anbietet, erklärt [Anmeldung und Single Sign-on](../self-hosting/sign-in.md).

## Zurechtfinden

Die Seitenleiste zeigt die Seiten der Organisation, oder die eines Projekts, wenn Sie darin arbeiten. Das Menü oben wechselt die Organisation und öffnet die **Organisationseinstellungen**; in einem Projekt wechselt es das Projekt und führt zurück zur Organisation. Das Menü an ihrem Fuß öffnet Ihr **Konto** und, für Instanz-Admins, die **Instanz**. Was die Seitenleiste zeigt, hängt von Ihren [Rechten](members-and-roles.md) ab.

Drücken Sie ⌘K oder Strg+K, um Seiten, Schlüssel, Modelle und Mitglieder zu suchen, etwas zu erstellen oder Organisation oder Projekt zu wechseln. Fügen Sie die ID einer Anfrage ein, `req_…` aus ihrem Header `kaeso-request-id`, um sie im Anfrageprotokoll des aktuellen Projekts zu öffnen. Hat kein Feld den Fokus, wirken auch einzelne Tasten, etwa `c` zum Erstellen auf der aktuellen Seite oder `g` `k` für die API-Schlüssel. Jede Seite eines Projekts hat ihre eigene `g`-Taste, etwa `g` `y` für den Playground. Drücken Sie `?`, um alle Tastenkürzel zu sehen, die dort wirken, wo Sie gerade sind, mit den Tasten der Anfrageprotokolle, Tool-Aufrufe und des Routen-Editors.

Eine Liste, etwa der API-Schlüssel oder Mitglieder, ist ein einziger Tab-Stopp. Wechseln Sie die Zeile mit den Pfeiltasten oder `j` und `k`, und öffnen Sie sie mit Enter.

## Nächste Schritte

- [Organisationen und Projekte](organizations-and-projects.md): Erstellen Sie Organisationen und Projekte, verbinden Sie Single Sign-on und verwalten Sie ihre Einstellungen.
- [Mitglieder und Rollen](members-and-roles.md): Laden Sie Personen ein, geben Sie ihnen Rollen und Rechte, und erstellen Sie Dienstkonten.
- [Anbieter-Schlüssel](provider-keys.md): Verbinden Sie Ihre Schlüssel für Modellanbieter und wählen Sie, welche Projekte welchen nutzen.
- [Eigene Anbieter](custom-providers.md): Fügen Sie einen eigenen Server, etwa ein selbst betriebenes vLLM oder einen internen Proxy, als Anbieter hinzu.
- [MCP-Server der Organisation](mcp-servers.md): Fügen Sie die MCP-Server der Organisation hinzu, melden Sie sich an und prüfen Sie die Tools, die sie ändern.
- [Guardrails](guardrails.md): Lassen Sie eigene Dienste Anfragen prüfen, bevor ein Modell sie erhält, und Antworten, bevor Apps sie erhalten.
- [Modelle und Aliasse](models-and-aliases.md): Wählen Sie die Modelle, die ein Projekt aufrufen darf, und geben Sie ihnen eigene Namen.
- [API-Schlüssel](api-keys.md): Erstellen Sie Schlüssel für Apps und Skripte, begrenzen Sie sie und schalten Sie sie ab.
- [Tools](tools.md): Wählen Sie die MCP-Tools, die das Portal eines Projekts anbietet, wer sie aufruft, und verbinden Sie Clients.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie Anfragen, Token und Ausgaben eines Projekts, eines Mitglieds oder eines Schlüssels.
- [Anfrageprotokolle](request-logs.md): Finden Sie jede Anfrage, exportieren Sie Protokolle und wählen Sie, was bleibt.
- [Antwort-Cache](response-cache.md): Beantworten Sie wiederholte, gleiche Anfragen aus einer gespeicherten Antwort.
- [Webhooks](webhooks.md): Senden Sie die Benachrichtigungen der Organisation signiert an Bereitschaftstools, Chat und Skripte.
- [Nutzung teilen](share-usage.md): Teilen Sie schreibgeschützte Links zu Nutzung, Mitgliedsstatistiken und Budgets mit Personen ohne Konto.
- [Playground](playground.md): Vergleichen Sie bis zu vier Modelle nebeneinander.
- [Konto und Sprache](account.md): Schützen Sie Ihr Konto und wählen Sie Design und Sprache.
- [Instanzverwaltung](instance-administration.md): Verwalten Sie Benutzer, Organisationen, Einstellungen und Updates der Installation.
