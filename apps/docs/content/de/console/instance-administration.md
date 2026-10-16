---
title: Instanzverwaltung
description: Benutzer, Organisationen und Einstellungen der ganzen Installation verwalten.
order: 10
---

**Instanz-Admins** betreiben die Installation. Der erste Benutzer, angelegt über den Einrichtungslink, ist einer; er kann weitere zu Instanz-Admins machen. Instanz-Admin zu sein gibt keine Rechte in Organisationen: Es zeigt ihre Metadaten, nie ihre Schlüssel, Protokolle oder Inhalte.

Die Seiten liegen unter **Einstellungen** › **Instanz**: im Menü am Fuß der Seitenleiste **Instanz** wählen.

## Benutzer

Jedes Konto der Installation, ob aktiv, deaktiviert oder gelöscht. **Benutzer einladen** lädt jemanden ohne Organisation in die Installation ein. Das Sheet eines Benutzers:

| Aktion | Wirkung |
| --- | --- |
| Instanz-Admin | Macht den Benutzer zum Instanz-Admin oder beendet das |
| Deaktivieren, Reaktivieren | Sperrt die Anmeldung, beendet die Sitzungen und deaktiviert die Schlüssel, oder erlaubt sie wieder |
| Sitzungen beenden | Meldet den Benutzer überall ab |
| Zwei-Faktor zurücksetzen | Entfernt Authenticator und Wiederherstellungscodes, für jemanden, der beides verloren hat |
| Benutzer löschen | Entfernt Name und E-Mail-Adresse und archiviert die Schlüssel; der Verlauf zeigt die Person als gelöschten Benutzer |

## Organisationen

Jede Organisation mit ihren Mitgliedern, Projekten und ihrem Status. **Organisation erstellen** legt eine mit einem gewählten Inhaber an, oder mit dem eigenen Konto als Inhaber, wenn keiner gewählt ist.

Die Seite einer Organisation zeigt ihre Metadaten, ihre Inhaber und die letzten 30 Tage an Anfragen, Token, Kosten und Protokollen. **Inhaber hinzufügen** macht einen beliebigen aktiven Benutzer zum Inhaber, auch das eigene Konto; so gelangt ein Instanz-Admin in eine Organisation, und es wird im Audit-Log der Organisation festgehalten. Von hier aus lassen sich Organisationen auch umbenennen, löschen und wiederherstellen oder sofort statt nach ihrer Wartezeit löschen.

## Einstellungen

| Einstellung | Optionen |
| --- | --- |
| Kontoerstellung | **Nur auf Einladung**, **Erlaubte E-Mail-Domains** oder **Alle** |
| Organisationserstellung | **Nur Instanz-Admins**, **Benutzer erlaubter Domains** oder **Alle angemeldeten Benutzer** |
| Erlaubte Domains | Domains für die beiden Einstellungen oben; `*.example.com` umfasst Subdomains |
| Löschen von Organisationen | Ob Organisationen gelöscht werden können, und die Wartezeit, 1 bis 90 Tage |
| Aufbewahrung der Anfrageprotokolle | Standardwerte für neue Organisationen und das Längste, was eine Organisation wählen darf |

Anmeldemethoden und E-Mail werden in der [Konfiguration](../self-hosting/configuration.md) des Gateways festgelegt und hier nur angezeigt. **Test-E-Mail senden** prüft die E-Mail-Einstellungen.

## Modellquellen

Der Stand der Modelldaten der Installation von models.dev: die verwendeten Daten, die letzte Synchronisierung und die automatische Synchronisierung, standardmäßig alle 24 Stunden. **Synchronisieren** aktualisiert sie; ist models.dev nicht abrufbar, bleiben die aktuellen Daten in Gebrauch.

## System

Version und Status des Gateways, ob PostgreSQL, ClickHouse und Valkey laufen, der Puffer der Anfrageprotokolle und der Speicher. Die Seite prüft alle 10 Sekunden.

## Audit-Log

Änderungen an Benutzern, Organisationen und den Einstellungen der Installation, jeweils mit Urheber und Zeitpunkt.
