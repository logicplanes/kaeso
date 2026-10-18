---
title: Konzepte
description: Die Begriffe dieser Seiten kennenlernen und erfahren, wie Kaeso mit Anfragen, Formaten und Rechten umgeht.
order: 2
---

Diese Seiten erklären, wie Kaeso arbeitet, damit Sie wissen, was Sie erwartet, bevor Sie sich darauf verlassen. Jede Seite dieser Dokumentation verwendet die Begriffe unten im selben Sinn.

## Begriffe

| Begriff | Bedeutung |
| --- | --- |
| Installation | Eine Kaeso-Bereitstellung: Gateway, Konsole und ihre Datenbanken unter einer Domain ([Selbst hosten](../self-hosting/index.md)) |
| Instanz-Admin | Ein Benutzer, der die Installation betreibt: ihre Benutzer, Organisationen und Einstellungen. Das gibt keine Rechte in Organisationen ([Instanzverwaltung](../console/instance-administration.md)) |
| Organisation | Enthält Mitglieder, Anbieter-Schlüssel, Plattform-Schlüssel und Projekte ([Organisationen und Projekte](../console/organizations-and-projects.md)) |
| Projekt | Enthält einen Modellkatalog, API-Schlüssel, Dienstkonten, Limits und Anfrageprotokolle. Jede Organisation hat ein Standardprojekt |
| Mitglied | Eine Person in einer Organisation. Ein Mitglied hat eine Organisationsrolle und in jedem seiner Projekte eine Projektrolle |
| Dienstkonto | Ein Projektmitglied für Software, etwa ein Deployment oder einen Job. Seine Schlüssel funktionieren weiter, wenn die Personen gehen, die sie erstellt haben |
| Recht, Rolle | Ein Recht erlaubt eine Art von Aktion, etwa `keys.all.view`. Eine Rolle ist eine benannte Menge von Rechten ([Rechte](rights.md)) |
| API-Schlüssel | `sk-kso-…`, der Schlüssel eines Mitglieds oder Dienstkontos in einem Projekt. Mit Inferenzzugriff ruft er Modelle auf, mit Plattformzugriff verwaltet er das Projekt ([API-Schlüssel](../console/api-keys.md)) |
| Plattform-Schlüssel | `sk-kso-org-…`, ein Schlüssel, der eine ganze Organisation über die Platform API verwaltet. Er ruft nie Modelle auf |
| Anbieter-Schlüssel | Der OpenAI- oder Anthropic-Schlüssel der Organisation, mit dem Kaeso den Anbieter aufruft. Apps sehen ihn nie ([Anbieter-Schlüssel](../console/provider-keys.md)) |
| Hauptschlüssel | Der eigene Schlüssel der Installation, der die gespeicherten Secrets verschlüsselt, etwa Anbieter-Schlüssel ([Backups und Hauptschlüssel](../self-hosting/backups.md)) |
| Katalog | Die Modelle, die ein Projekt aufrufen darf, mit ihren Preisen, Limits und Fähigkeiten. Sie kommen aus models.dev, und ein Projekt kann eigene Modelle hinzufügen oder Felder überschreiben ([Modelle und Aliasse](../console/models-and-aliases.md)) |
| Alias | Ein eigener Name eines Projekts für ein Modell seines Katalogs, etwa `fast` |
| Limit | Eine Anfrage- oder Token-Rate oder ein Token- oder Ausgabenbudget, für ein Projekt, ein Mitglied oder einen Schlüssel. Ein hartes Budget lehnt Anfragen ab, ein weiches warnt nur ([Limits und Budgets](../console/limits-and-budgets.md)) |
| Anfrageprotokoll | Der Eintrag zu einer Modellanfrage: Status, Token, Kosten, Zeiten und, wo aufbewahrt, ihr Inhalt ([Anfrageprotokolle](../console/request-logs.md)) |
| API-Format | Die Form einer Anfrage und ihrer Antwort: OpenAI Chat Completions, Responses oder Embeddings oder Anthropic Messages |
| KPP | Das Kaeso Provider Protocol, das eine Format, in das und aus dem das Gateway jedes API-Format übersetzt ([Formatübersetzung](format-translation.md)) |
| Modell-API, Platform API | Die beiden APIs einer Installation. Apps rufen Modelle über `/v1` auf, Konsole und Skripte verwalten die Installation über `/platform/v1` ([API](../api/index.md)) |

## Nächste Schritte

- [Ablauf einer Anfrage](request-flow.md): Verfolgen Sie eine Modellanfrage durch das Gateway, und erfahren Sie, welchen Fehler sie erhält und was sie kostet.
- [Formatübersetzung](format-translation.md): Sehen Sie, was erhalten bleibt, wenn eine App das Modell eines anderen Anbieters aufruft.
- [Rechte](rights.md): Schlagen Sie jedes Recht nach, welche Rollen es haben und nach welchen Regeln Sie es vergeben.
