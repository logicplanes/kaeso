---
title: Instanzverwaltung
description: Benutzer, Organisationen und Einstellungen der ganzen Installation verwalten.
order: 10
---

**Instanz-Admins** betreiben die Installation. Der erste Benutzer, angelegt über den Einrichtungslink, ist einer und kann weitere zu Instanz-Admins machen. Instanz-Admin zu sein gibt keine Rechte in Organisationen: Es zeigt ihre Metadaten, nie ihre Schlüssel, Protokolle oder Inhalte.

Die Seiten liegen unter **Instanz** im Menü am Fuß der Seitenleiste.

## Benutzer

**Benutzer** listet jedes Konto der Installation, aktiv, deaktiviert oder gelöscht. **Benutzer einladen** lädt jemanden ohne Organisation in die Installation ein; einer Organisation tritt die Person bei, sobald diese sie einlädt. Das Sheet eines Benutzers hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Instanz-Admin | Macht den Benutzer zum Instanz-Admin oder nimmt ihm diese Rolle |
| Deaktivieren, Reaktivieren | Sperrt die Anmeldung, beendet seine Sitzungen und deaktiviert seine Schlüssel, oder erlaubt alles wieder |
| Sitzungen beenden | Meldet den Benutzer überall ab |
| Zwei-Faktor zurücksetzen | Entfernt Authenticator-App und Wiederherstellungscodes, für jemanden, der beides verloren hat |
| Benutzer löschen | Entfernt Name und E-Mail-Adresse und archiviert die Schlüssel; der Verlauf zeigt einen gelöschten Benutzer |

Der betroffene Benutzer erhält eine E-Mail, wenn seine Sitzungen beendet oder seine Zwei-Faktor-Authentifizierung zurückgesetzt wird.

## Organisationen

**Organisationen** listet jede Organisation mit ihren Mitgliedern, Projekten und ihrem Zustand. **Organisation erstellen** legt eine mit dem gewählten Inhaber an, oder mit der eigenen Person als Inhaber, wenn keiner gewählt ist.

Die Seite einer Organisation zeigt ihre Metadaten, Inhaber, Mitglieder und Projekte sowie Anfragen, Token, Kosten und gespeicherte Protokolle der letzten 30 Tage. **Inhaber hinzufügen** macht jeden aktiven Benutzer zum Inhaber, auch die eigene Person (**Mich selbst hinzufügen**); so gelangt ein Instanz-Admin in eine Organisation, und ihr Audit-Log verzeichnet es. Auf der Seite lässt sich die Organisation außerdem umbenennen, löschen und wiederherstellen, und **Jetzt löschen** löscht eine zur Löschung vorgemerkte Organisation sofort statt nach ihrer Schonfrist.

## Einstellungen

| Einstellung | Optionen |
| --- | --- |
| Kontoerstellung | **Nur auf Einladung**, **Erlaubte E-Mail-Domains** oder **Alle**; Eingeladene können ihr Konto immer erstellen |
| Organisationserstellung | **Nur Instanz-Admins**, **Benutzer erlaubter Domains** oder **Alle angemeldeten Benutzer** |
| Erlaubte Domains | Die Domains für die beiden Einstellungen darüber; `*.example.com` deckt Subdomains ab |
| Löschen von Organisationen | Ob sich Organisationen überhaupt löschen lassen, und die Schonfrist, 1 bis 90 Tage |
| Aufbewahrung der Anfrageprotokolle | Die Standardwerte für neue Organisationen und die längste Aufbewahrung, die eine Organisation wählen darf |

Kontoerstellung, Organisationserstellung oder die erlaubten Domains zu ändern erfordert eine kürzliche Anmeldung. Anmeldemethoden und E-Mail werden in der [Konfiguration](../self-hosting/configuration.md) des Gateways festgelegt und hier nur angezeigt; **Test-E-Mail senden** prüft, ob das Gateway E-Mails versenden kann.

## Modellquellen

**Modellquellen** zeigt die models.dev-Daten der Installation: die verwendeten Daten, die letzte Synchronisierung und die automatische, standardmäßig alle 24 Stunden. **Synchronisieren** holt sie sofort; ist models.dev nicht erreichbar, bleiben die aktuellen Daten in Gebrauch.

## System

**System** zeigt Version und Zustand des Gateways, ob PostgreSQL, ClickHouse und Valkey laufen, den Puffer der Anfrageprotokolle und den Speicher. Die Seite prüft alle 10 Sekunden, **Jetzt prüfen** sofort.

## Audit-Log

Das **Audit-Log** der Instanz verzeichnet Änderungen an Benutzern, Organisationen und den Einstellungen der Installation, mit Urheber und Zeitpunkt.
