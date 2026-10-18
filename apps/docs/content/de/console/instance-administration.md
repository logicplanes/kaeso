---
title: Instanzverwaltung
description: Verwalten Sie Benutzer, Organisationen, Einstellungen und Updates der ganzen Installation.
order: 10
---

Instanz-Admins betreiben die Installation. Der erste Benutzer, angelegt über den Einrichtungslink, ist einer und kann weitere zu Instanz-Admins machen. Instanz-Admin zu sein gibt keine Rechte in Organisationen: Sie sehen deren Metadaten, nie ihre Schlüssel, Protokolle oder Inhalte.

Die Seiten finden Sie unter **Instanz** im Menü am Fuß der Seitenleiste.

## Benutzer verwalten

**Benutzer** listet jedes Konto der Installation: aktiv, deaktiviert oder gelöscht. **Benutzer einladen** lädt jemanden ohne Organisation in die Installation ein. Einer Organisation tritt die Person bei, wenn eine Organisation sie einlädt.

Wählen Sie einen Benutzer, um sein Sheet zu öffnen. Es hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Instanz-Admin | Macht den Benutzer zum Instanz-Admin oder nimmt ihm diese Rolle |
| Deaktivieren, Reaktivieren | Sperrt die Anmeldung, beendet seine Sitzungen und deaktiviert seine Schlüssel, oder erlaubt sie wieder |
| Sitzungen beenden | Meldet den Benutzer überall ab |
| Zwei-Faktor zurücksetzen | Entfernt Authenticator-App und Wiederherstellungscodes, für jemanden, der beides verloren hat |
| Benutzer löschen | Entfernt Namen und E-Mail-Adresse und archiviert die Schlüssel. Der Verlauf zeigt einen gelöschten Benutzer |

Kaeso informiert den Benutzer per E-Mail, wenn Sie seine Sitzungen beenden oder seine Zwei-Faktor-Authentifizierung zurücksetzen.

## Organisationen verwalten

**Organisationen** listet jede Organisation mit ihren Mitgliedern, Projekten und ihrem Zustand. **Organisation erstellen** legt eine mit dem Inhaber an, den Sie wählen, oder mit Ihnen als Inhaber, wenn Sie niemanden wählen.

Die Seite einer Organisation zeigt ihre Metadaten, Inhaber, Mitglieder und Projekte. Sie zeigt außerdem ihre Anfragen, Token, Kosten und gespeicherten Protokolle der letzten 30 Tage. Dort können Sie die Organisation umbenennen, löschen und wiederherstellen.

Um in eine Organisation zu gelangen, wählen Sie **Inhaber hinzufügen** und einen beliebigen aktiven Benutzer, oder **Mich selbst hinzufügen**. Das Audit-Log der Organisation verzeichnet das. Für eine zur Löschung vorgemerkte Organisation löscht **Jetzt löschen** sie sofort statt nach ihrer Schonfrist.

## Installationseinstellungen ändern

Unter **Einstellungen**:

| Einstellung | Auswahl |
| --- | --- |
| Kontoerstellung | **Nur auf Einladung**, **Erlaubte E-Mail-Domains** oder **Alle**. Eingeladene können ihr Konto immer anlegen |
| Organisationserstellung | **Nur Instanz-Admins**, **Benutzer erlaubter Domains** oder **Alle angemeldeten Benutzer** |
| Erlaubte Domains | Die Domains für die beiden Einstellungen darüber. `*.example.com` umfasst Subdomains |
| Löschen von Organisationen | Ob sich Organisationen überhaupt löschen lassen, und die Schonfrist, 1 bis 90 Tage |
| Aufbewahrung der Anfrageprotokolle | Die Standards für neue Organisationen und die längste Aufbewahrung, die eine Organisation wählen darf |

Kontoerstellung, Organisationserstellung oder die erlaubten Domains zu ändern erfordert eine kürzliche Anmeldung.

Anmeldemethoden und E-Mail legt die [Konfiguration](../self-hosting/configuration.md) des Gateways fest, und diese Seite zeigt sie nur an. **Test-E-Mail senden** prüft, ob das Gateway E-Mails senden kann.

## Modelldaten synchronisieren

**Modellquellen** zeigt die models.dev-Daten der Installation: die verwendeten Daten, die letzte Synchronisierung und die automatische Synchronisierung, standardmäßig alle 24 Stunden. **Synchronisieren** holt die Daten sofort. Ist models.dev nicht erreichbar, bleiben die aktuellen Daten in Gebrauch.

## System prüfen

**System** zeigt Version und Zustand des Gateways und ob PostgreSQL, ClickHouse und Valkey laufen. Es zeigt außerdem den Puffer der Anfrageprotokolle und den Speicher. Die Seite prüft alle 10 Sekunden, und **Jetzt prüfen** prüft sofort.

## Kaeso aktualisieren

**Updates** zeigt die Version der Installation und das neueste Kaeso-Release mit den Release-Notizen aller Releases seither. Eine Markierung **Neu** neben **Updates** in der Seitenleiste der Einstellungen zeigt, dass ein neueres Release erschienen ist.

- **Jetzt prüfen** sucht sofort nach einem Release. Sonst sucht das Gateway zweimal täglich.
- Die Einstellungen der Seite schalten diese Prüfung ab, bieten Vorabversionen an und schalten Updates mit einem Klick über den [Updater](../self-hosting/upgrades.md#updates-mit-einem-klick) ein.
- Mit dem Updater aktualisiert **Auf … aktualisieren** die Installation, nachdem Sie bestätigt haben, und verfolgt jeden Schritt, bis Kaeso wieder da ist.
- Ohne Updater, oder für ein Release, das Schritte von Hand verlangt, zeigt die Seite die Befehle und die Upgrade-Anleitung.

Der Verlauf listet jedes Update mit der Person, die es gestartet hat, und seinem Ausgang.

## Audit-Log lesen

Das **Audit-Log** der Instanz verzeichnet Änderungen an Benutzern, Organisationen und den Einstellungen der Installation, jeweils mit Urheber und Zeitpunkt.

## Nächste Schritte

- [Konfiguration](../self-hosting/configuration.md): Legen Sie Anmeldemethoden, E-Mail und andere Einstellungen des Gateways fest.
- [Upgrades](../self-hosting/upgrades.md): Bringen Sie die Installation auf ein neues Release, mit oder ohne Updater.
- [Anmeldung und Single Sign-on](../self-hosting/sign-in.md): Kommen Sie wieder hinein, wenn sich niemand mehr anmelden kann.
