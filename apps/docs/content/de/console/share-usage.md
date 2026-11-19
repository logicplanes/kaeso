---
title: Nutzung teilen
description: Teilen Sie schreibgeschützte Links zu Nutzung, Mitgliedsstatistiken oder Budgets mit Personen ohne Konto.
order: 10
---

Ein geteilter Link zeigt die Zahlen einer Seite Personen ohne Kaeso-Konto, etwa der Buchhaltung oder einer Führungskraft. Er bleibt aktuell oder behält die Zahlen des Tages, an dem er entstand, und funktioniert nicht mehr, wenn er abläuft oder Sie ihn widerrufen.

## Was Sie teilen können

Die Schaltfläche **Teilen** steht dort, wo die jeweilige Ansicht steht:

| Geteilt | Wo | Was der Link zeigt |
| --- | --- | --- |
| Analysen | **Analysen** der Organisation oder eines Projekts | Die Summen des Zeitraums, das Diagramm der gewählten Kennzahl gegenüber den Tagen davor und die Nutzung nach Projekt, Modell, Schlüssel, Mitglied, Route und Client |
| Mitgliedsstatistik | Das Blatt eines Mitglieds unter **Organisationseinstellungen** → **Mitglieder** | Anfragen, Token, Kosten, Cache-Treffer und Verlauf eines Mitglieds, nach Modell und Projekt |
| Budgetstand | **Limits** eines Projekts | Die Limits und Budgets des Projekts mit Verbrauch und Prognose |
| Projektübersicht | **Übersicht** eines Projekts | Die letzten 30 Tage des Projekts: Anfragen, Kosten, Fehlerquote und Top-Modelle |

Anfrageprotokolle werden nie geteilt: Sie können Prompts und Antworten enthalten. Auch Schlüssel, Adressen und IDs von Mitgliedern nicht.

## Festlegen, was ein Link zeigt

Der Dialog von **Teilen** legt fest:

- **Titel**, den die Seite zeigt. Er nennt Projekt, Organisation oder Mitglied, außer Sie verbergen diesen Namen.
- **Zahlen.** **Live** zeigt bei jedem Öffnen die letzten Tage, so viele wie der Zeitraum der Seite. **Momentaufnahme** behält die Zahlen des Zeitraums, wie sie jetzt sind.
- **Kosten zeigen**, standardmäßig aus. Nur angeboten, wenn Sie die Kosten sehen.
- **Verbergen** von Namen der Mitglieder, Schlüssel und Projekte. Verborgene Namen erscheinen als *Mitglied 1*, *Schlüssel 2* und so weiter, nach der Zahl der Anfragen.
- **Wer** ihn öffnen kann: **Angemeldete Mitglieder dieser Organisation** oder **Alle mit dem Link**, wo die Organisation es erlaubt.
- **Läuft ab** nach 1, 7, 30 oder 90 Tagen oder nie, innerhalb der längsten Gültigkeit der Organisation.

Der Link wird einmal gezeigt. Kaeso behält nur seinen Hash; kopieren Sie ihn also, bevor Sie den Dialog schließen.

Ein Link zeigt nie mehr, als die Person sieht, die ihn erstellt hat. Bei jedem Öffnen prüft Kaeso, ob sie noch Mitglied ist und das Recht hat, zu teilen und zu sehen, was der Link zeigt. Ein Link, dessen Ersteller die Organisation verlassen oder diese Rechte verloren hat, funktioniert nicht mehr; fehlt nur das Recht, Kosten zu sehen, zeigt er sie nicht mehr.

## Die geteilte Seite

Die Seite öffnet sich unter `https://kaeso.example.com/share/…` in der Sprache und im hellen oder dunklen Modus des Betrachters, auf dem Telefon oder dem Computer. Sie führt nicht in die Konsole, und Suchmaschinen werden gebeten, sie nicht zu indexieren. Chats und Beiträge zeigen ein Vorschaubild des Diagramms. Am Fuß steht ein kleines Zeichen **Erstellt mit Kaeso**, das Instanz-Admins ausschalten können.

## Links verwalten

**Organisationseinstellungen** → **Geteilte Links** listet die Links, die noch funktionieren, mit Zielgruppe, Ersteller, Aufrufen, letztem Öffnen und Ablauf. **Widerrufen** beendet einen Link sofort. Ohne `usage.share.manage` sehen und widerrufen Sie nur Ihre eigenen Links.

Unter **Organisationseinstellungen** → **Sicherheit** legt **Geteilte Links** fest: **Geteilte Links erlauben** (**Aus**, **Nur Mitglieder** oder **Alle mit dem Link**) und die **Längste Gültigkeit**. Eine Verschärfung gilt auch für frühere Links: Mit **Aus** funktionieren sie nicht mehr, mit **Nur Mitglieder** brauchen sie die Anmeldung eines Mitglieds. Instanz-Admins können geteilte Links für die ganze Installation ausschalten ([Instanzverwaltung](instance-administration.md#installationseinstellungen-ändern)).

Erstellen, Öffnen und Widerrufen eines Links stehen im Audit-Log. Wer `usage.share.manage` hat, bekommt eine E-Mail, wenn jemand einen Link teilt, und die Webhooks der Organisation für **Sicherheitseinstellungen geändert** bekommen ein Ereignis `share.created` ([Webhooks](webhooks.md)).

## Rechte

| Recht | Erlaubt |
| --- | --- |
| `usage.share` | Teilen, was Sie sehen, und eigene Links widerrufen. Inhaber, Admins, Abrechnung, Entwickler und Mitglieder haben es |
| `usage.share.manage` | Jeden Link sehen und widerrufen und die Richtlinie festlegen. Inhaber und Admins haben es |

Die Analysen einer Organisation zu teilen braucht `usage.org.view`, die Analysen oder Übersicht eines Projekts `usage.view` darin und seine Budgets `limits.view`. Die Statistik anderer zu teilen braucht `usage.org.view`; Ihre eigene braucht nichts weiter. Kosten zu zeigen braucht `costs.org.view` oder `costs.view`.

In der Platform API erstellt `POST /platform/v1/orgs/{org}/shared-links` einen Link, `GET` listet sie und `DELETE …/shared-links/{link}` widerruft einen ([Platform API](../api/platform-api.md)). Agenten, die mit dem MCP-Server von Kaeso verbunden sind, bekommen dasselbe als Tools ([MCP-Server](../api/mcp.md)).

## Nächste Schritte

- [Organisationen und Projekte](organizations-and-projects.md): Lesen Sie, was Analysen zeigen, bevor Sie sie teilen.
- [Mitglieder und Rollen](members-and-roles.md): Geben oder nehmen Sie die Rechte zum Teilen.
- [Webhooks](webhooks.md): Senden Sie neue geteilte Links an einen Chat oder ein Bereitschaftstool.
