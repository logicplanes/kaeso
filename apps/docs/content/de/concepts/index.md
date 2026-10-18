---
title: Konzepte
description: Die Begriffe dieser Seiten und wie Kaeso mit Anfragen, Formaten und Rechten umgeht.
order: 2
---

Der [Ablauf einer Anfrage](request-flow.md) verfolgt eine Modellanfrage durch das Gateway, die [Formatübersetzung](format-translation.md) erklärt, was KPP zwischen API-Formaten überträgt, und [Rechte](rights.md) listet, was jede Rolle darf. Alle Seiten verwenden diese Begriffe im selben Sinn.

## Begriffe

| Begriff | Bedeutung |
| --- | --- |
| Installation | Eine Kaeso-Installation: Gateway, Konsole und ihre Datenbanken unter einer Domain ([Selbst hosten](../self-hosting/index.md)) |
| Instanz-Admin | Ein Benutzer, der die Installation betreibt: ihre Benutzer, Organisationen und Einstellungen. Das gibt keine Rechte in Organisationen ([Instanzverwaltung](../console/instance-administration.md)) |
| Organisation | Enthält Mitglieder, Anbieter-Schlüssel, Plattform-Schlüssel und Projekte ([Organisationen und Projekte](../console/organizations-and-projects.md)) |
| Projekt | Enthält einen Modellkatalog, API-Schlüssel, Dienstkonten, Limits und Anfrageprotokolle. Jede Organisation hat ein Standardprojekt |
| Mitglied | Eine Person in einer Organisation, mit einer Organisationsrolle und einer Projektrolle in jedem ihrer Projekte |
| Dienstkonto | Ein Projektmitglied für Software, etwa ein Deployment oder einen Job; seine Schlüssel überdauern die Personen, die sie erstellt haben |
| Rolle, Recht | Ein Recht erlaubt eine Art von Aktion, etwa `keys.all.view`; eine Rolle ist eine benannte Menge von Rechten ([Rechte](rights.md)) |
| API-Schlüssel | `sk-kso-…`: ein Schlüssel eines Mitglieds oder Dienstkontos in einem Projekt. Mit Inferenzzugriff ruft er Modelle über die Modell-API auf, mit Plattformzugriff verwaltet er das Projekt über die Platform API ([API-Schlüssel](../console/api-keys.md)) |
| Plattform-Schlüssel | `sk-kso-org-…`: ein Schlüssel, der eine ganze Organisation über die Platform API verwaltet und nie Modelle aufruft |
| Anbieter-Schlüssel | Der OpenAI- oder Anthropic-Schlüssel der Organisation, mit dem Kaeso den Anbieter aufruft; Apps sehen ihn nie ([Anbieter-Schlüssel](../console/provider-keys.md)) |
| Hauptschlüssel | Der eigene Schlüssel der Installation, der die gespeicherten Geheimnisse verschlüsselt, etwa Anbieter-Schlüssel ([Backups und Hauptschlüssel](../self-hosting/backups.md)) |
| Katalog | Die Modelle, die ein Projekt aufrufen darf, mit ihren Preisen, Limits und Fähigkeiten aus models.dev ([Modelle und Aliasse](../console/models-and-aliases.md)) |
| Alias | Ein eigener Name eines Projekts für ein Modell seines Katalogs, etwa `fast` |
| Limit | Eine Anfrage- oder Token-Rate oder ein Token- oder Kostenbudget, für ein Projekt, ein Mitglied oder einen Schlüssel; ein hartes Budget lehnt Anfragen ab, ein weiches warnt nur ([Limits und Budgets](../console/limits-and-budgets.md)) |
| Anfrageprotokoll | Der Eintrag zu einer Modellanfrage: Status, Tokens, Kosten, Zeiten und, wo aufbewahrt, ihr Inhalt ([Anfrageprotokolle](../console/request-logs.md)) |
| API-Format | Die Form einer Anfrage und ihrer Antwort: OpenAI Chat Completions, Responses oder Embeddings, oder Anthropic Messages |
| KPP | Das Kaeso Provider Protocol: das eine Format, in das und aus dem das Gateway jedes API-Format übersetzt ([Formatübersetzung](format-translation.md)) |
| Modell-API, Platform API | Die beiden APIs einer Installation: `/v1`, die Apps für Modelle aufrufen, und `/platform/v1`, über die Konsole und Skripte sie verwalten ([API](../api/index.md)) |
