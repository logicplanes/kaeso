---
title: Die Konsole nutzen
description: Orientierung in der Konsole, der Weboberfläche einer Kaeso-Installation.
order: 3
---

Die Konsole läuft unter der Domain der Installation, etwa `https://kaeso.example.com`. Die Anmeldung erfolgt per E-Mail-Adresse: je nachdem, was Installation und Organisation erlauben, mit Passwort, mit einem Anmeldelink per E-Mail oder per Single Sign-on.

## Zwei Modi

**Die App** ist der Arbeitsbereich in einer Organisation und ihren Projekten. Die Seitenleiste zeigt **Übersicht**, **Projekte** und **Anbieter** der Organisation, danach eine Liste ihrer Projekte, und in einem Projekt dessen **Übersicht**, **Playground**, **API-Schlüssel**, **Anfrageprotokolle**, **Modelle**, **Limits**, **Mitglieder**, **Dienstkonten**, **Audit-Log** und **Einstellungen**.

**Einstellungen** enthält das eigene Konto, die Einstellungen der Organisation und, für Instanz-Admins, die Einstellungen der Installation. Geöffnet wird es über das Menü am Fuß der Seitenleiste mit **Organisationseinstellungen**, **Konto** oder **Instanz**; dasselbe Menü wechselt die Organisation und enthält **Abmelden**.

## Was angezeigt wird

Was angezeigt wird, hängt von den eigenen Rechten ab. Eine Seite, für die das Recht fehlt, erscheint nicht in der Seitenleiste. Eine Aktion, die nicht erlaubt ist, ist deaktiviert, und ihr Tooltip nennt das nötige Recht und wer es vergeben kann. [Mitglieder und Rollen](members-and-roles.md) erklärt die Rechte.

Einige sensible Aktionen, etwa einen Anbieter-Schlüssel hinzufügen oder einem Schlüssel Plattformzugriff geben, verlangen eine erneute Anmeldung, wenn die letzte Anmeldung mehr als 15 Minuten zurückliegt.

## Suche und Tastenkürzel

⌘K oder Strg+K öffnet die Suche: zu jeder Seite springen, Schlüssel, Modelle, Mitglieder und Dienstkonten des aktuellen Projekts finden, Neues erstellen, Organisation oder Projekt wechseln und das Design ändern. `c` erstellt auf der aktuellen Seite, `g` gefolgt von einem Buchstaben öffnet eine Seite, etwa `g k` die API-Schlüssel, und `?` listet alle Tastenkürzel.

## Seiten

| Seite | Inhalt |
| --- | --- |
| [Organisationen und Projekte](organizations-and-projects.md) | Erstellen und Wechseln, das Standardprojekt, Übersichten und Einstellungen |
| [Mitglieder und Rollen](members-and-roles.md) | Personen einladen, Rollen und Rechte, Dienstkonten |
| [Anbieter-Schlüssel](provider-keys.md) | OpenAI und Anthropic verbinden |
| [Modelle und Aliasse](models-and-aliases.md) | Der Katalog des Projekts, angepasste Werte und Aliasse |
| [API-Schlüssel](api-keys.md) | Schlüssel für Apps und Skripte |
| [Limits und Budgets](limits-and-budgets.md) | Raten und Budgets für Projekte, Mitglieder und Schlüssel |
| [Anfrageprotokolle](request-logs.md) | Jede Anfrage, ihr Inhalt, Exporte und Aufbewahrung |
| [Playground](playground.md) | Modelle nebeneinander ausprobieren |
| [Konto und Sprache](account.md) | Profil, Sicherheit, Sitzungen, Design und Sprache |
| [Instanzverwaltung](instance-administration.md) | Benutzer, Organisationen und Einstellungen der Installation |
