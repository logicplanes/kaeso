---
title: Organisationen und Projekte
description: Organisationen und Projekte erstellen und wechseln und ihre Nutzung ablesen.
order: 1
---

Eine **Organisation** enthält Mitglieder, Anbieter-Schlüssel und Projekte. Ein **Projekt** enthält einen Modellkatalog, API-Schlüssel, Limits und Anfrageprotokolle. Personen gehören einer Organisation und einigen ihrer Projekte an, jeweils mit einer Rolle.

## Organisationen

Eine neue Organisation entsteht über das Menü am Fuß der Seitenleiste mit **Organisation erstellen**. Das eigene Konto wird ihr Inhaber, und sie beginnt mit einem Standardprojekt. Ob Organisationen erstellt werden dürfen, entscheidet die Installation; siehe [Instanzverwaltung](instance-administration.md#einstellungen).

Dasselbe Menü listet die eigenen Organisationen und wechselt zwischen ihnen.

### Übersicht

Die **Übersicht** der Organisation zeigt Anfragen, Token, Kosten und Fehler für einen wählbaren Zeitraum, von heute bis seit Beginn, die Kosten nach Projekt und eine Aufschlüsselung nach Projekt, Modell oder Mitglied. Die Nutzung erfordert das Recht **Gesamtnutzung ansehen**, die Kosten **Gesamtkosten ansehen**.

### Einstellungen

Unter **Organisationseinstellungen**, im Menü am Fuß der Seitenleiste, hat die Organisation diese Seiten:

| Seite | Inhalt |
| --- | --- |
| Allgemein | Bild, Name und ID, Übertragen der Inhaberschaft, Löschen der Organisation |
| Mitglieder | Mitglieder, Einladungen und Rollen; siehe [Mitglieder und Rollen](members-and-roles.md) |
| Sicherheit | Single Sign-on, verlangtes Single Sign-on und verlangte Zwei-Faktor-Authentifizierung, die maximale Schlüssellaufzeit |
| Plattform-Schlüssel | Schlüssel für Skripte, die die Organisation verwalten; siehe [API-Schlüssel](api-keys.md#plattform-schlüssel) |
| Protokollierung | Inhaltsprotokollierung, Aufbewahrung, Speicher und Löschen von Protokollen; siehe [Anfrageprotokolle](request-logs.md#aufbewahrung) |
| Audit-Log | Jede Änderung in der Organisation, mit Urheber und Zeitpunkt; exportierbar als JSON Lines |

### Single Sign-on

Unter **Sicherheit** **Identitätsanbieter verbinden** wählen, damit sich Personen über den eigenen OpenID-Connect-Anbieter anmelden. Die Issuer-URL eingeben, Client-ID und Secret der dort registrierten App und die E-Mail-Domains, die sich darüber anmelden. Festlegen, ob Personen ohne Einladung beitreten können und mit welcher Rolle; das gilt nur für Personen, die neu bei Kaeso sind, denn ein bestehendes Konto meldet sich erst über Single Sign-on an, wenn es Mitglied der Organisation ist. Internationalisierte Domains lassen sich in beiden Formen eingeben, `bücher.example` oder `xn--bcher-kva.example`. Die Seite zeigt die Weiterleitungs-URL, die beim Anbieter zu registrieren ist.

Sobald die Verbindung steht, macht **Single Sign-on verlangen** es zum einzigen Weg in die Organisation. **Zwei-Faktor-Authentifizierung verlangen** gilt für die Anmeldung mit Passwort, nicht für Anmeldelinks oder Single Sign-on.

### Eine Organisation löschen

Wenn die Installation es erlaubt, löscht ein Inhaber die Organisation unter **Allgemein** › **Organisation löschen** und bestätigt mit ihrem Namen; das erfordert eine kürzliche Anmeldung, und die Mitglieder werden per E-Mail informiert. Ihre Schlüssel funktionieren sofort nicht mehr; sie bleibt für eine Wartezeit, standardmäßig 7 Tage, schreibgeschützt und wiederherstellbar und wird dann endgültig gelöscht. Bis dahin zeigt jede Seite einen Hinweis, und **Organisation wiederherstellen** holt sie zurück.

## Projekte

**Projekte** listet die Projekte der Organisation; archivierte stehen unter **Alle**. **Projekt erstellen** fragt nach einem Namen; das eigene Konto wird Admin des Projekts. Projekte zu erstellen erfordert das Recht **Projekte erstellen**.

Das **Standardprojekt** ist das Projekt, dem jedes Mitglied angehört und in dem neue Mitglieder beginnen. Um es zu ändern, im Menü eines anderen Projekts **Als Standard festlegen** wählen; das erfordert das Recht **Organisation bearbeiten**.

### Übersicht

Die **Übersicht** eines Projekts zeigt seine Anfragen, Token, Kosten und Fehler über einen Zeitraum, eine Aufschlüsselung nach Zeit, Modell, Schlüssel oder Mitglied, seine Limits mit dem Verbrauch und die letzten Fehler.

### Einstellungen

| Einstellung | Wirkung |
| --- | --- |
| Bild, Name | Erscheinen in der Seitenleiste und in den Anfrageprotokollen |
| Schlüssellaufzeit | **Vorgabe der Organisation verwenden** oder ein kürzeres Maximum für neue Schlüssel dieses Projekts |
| Inhaltsprotokollierung | Ob Anfragen und Antworten mit dem Protokolleintrag gespeichert werden; sie kann nicht eingeschaltet sein, wenn die Organisation sie abgeschaltet hat |
| Projekt archivieren | Archiviert das Projekt samt Schlüsseln und Dienstkonten, endgültig, sodass seine Schlüssel nicht mehr funktionieren; das Standardprojekt lässt sich nicht archivieren |

Projekte werden archiviert, nie gelöscht; so bleiben ihre Nutzung und Protokolle erhalten.

### Audit-Log

Das **Audit-Log** eines Projekts listet jede Änderung im Projekt mit Urheber und Zeitpunkt, filterbar nach Aktion, Akteur und Tagen. Es erfordert das Recht **Projekt-Audit-Log ansehen**.
