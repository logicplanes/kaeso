---
title: Instanzverwaltung
description: Verwalten Sie Benutzer, Organisationen, Einstellungen und Updates der ganzen Installation.
order: 12
---

Instanz-Admins betreiben die Installation. Der erste Benutzer, angelegt über den Einrichtungslink, ist einer und kann weitere zu Instanz-Admins machen. Instanz-Admin zu sein gibt keine Rechte in Organisationen: Sie sehen deren Metadaten, nie ihre Schlüssel, Protokolle oder Inhalte.

Die Seiten finden Sie unter **Instanz** im Menü am Fuß der Seitenleiste.

## Benutzer verwalten

**Benutzer** listet jedes Konto der Installation: aktiv, deaktiviert oder gelöscht. **Benutzer einladen** lädt jemanden ohne Organisation in die Installation ein. Einer Organisation tritt die Person bei, wenn eine Organisation sie einlädt. **Letzte Anmeldung** zeigt, wann sich jeder Benutzer zuletzt angemeldet hat, mit welcher Methode auch immer; das Einschalten der Zwei-Faktor-Authentifizierung zählt nicht als Anmeldung. Bei jemandem, der sich nicht mehr angemeldet hat, seit die Installation das festhält, steht **Nie**.

**Zwei-Faktor** zeigt, wer eine Authenticator-App nutzt. Instanz-Admins tragen das Abzeichen **Instanz-Admin**, und **Verwaltet von** nennt die Organisation, die ein Konto verwaltet ([verwaltete Konten](../self-hosting/sign-in.md#verwaltete-konten)). Wählen Sie einen Benutzer, um sein Sheet zu öffnen. Es listet die Organisationen, denen er angehört, mit seiner Rolle in jeder, und hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Instanz-Admin | Macht den Benutzer zum Instanz-Admin oder nimmt ihm diese Rolle |
| Deaktivieren, Reaktivieren | Sperrt die Anmeldung, beendet seine Sitzungen und deaktiviert seine Schlüssel, oder erlaubt sie wieder |
| Sitzungen beenden | Meldet den Benutzer überall ab und widerruft seine Agenten |
| Zwei-Faktor zurücksetzen | Entfernt Authenticator-App und Wiederherstellungscodes, für jemanden, der beides verloren hat; beendet auch seine Sitzungen und Agenten |
| Benutzer löschen | Entfernt Namen und E-Mail-Adresse und archiviert die Schlüssel. Der Verlauf zeigt einen gelöschten Benutzer |

Kaeso informiert den Benutzer per E-Mail, wenn Sie seine Sitzungen beenden oder seine Zwei-Faktor-Authentifizierung zurücksetzen.

## Organisationen verwalten

**Organisationen** listet jede Organisation mit ihren Mitgliedern, Projekten und ihrem Zustand. **Organisation erstellen** legt eine mit dem Inhaber an, den Sie wählen, oder mit Ihnen als Inhaber, wenn Sie niemanden wählen. **Anmeldung** sagt, ob eine Organisation Single Sign-on, Zwei-Faktor-Authentifizierung oder beides verlangt, und **Single Sign-on** filtert die Liste nach Organisationen mit oder ohne SSO-Verbindung.

Die Seite einer Organisation zeigt ihre Metadaten, Inhaber, Mitglieder und Projekte. Sie zeigt außerdem ihre Anfragen, Token, Kosten und gespeicherten Protokolle der letzten 30 Tage. Dort können Sie die Organisation umbenennen, löschen und wiederherstellen.

Um in eine Organisation zu gelangen, wählen Sie **Inhaber hinzufügen** und einen beliebigen aktiven Benutzer, oder **Mich selbst hinzufügen**. Das Audit-Log der Organisation verzeichnet das. Für eine zur Löschung vorgemerkte Organisation löscht **Jetzt löschen** sie sofort statt nach ihrer Schonfrist.

## Installationseinstellungen ändern

Unter **Allgemein**:

| Einstellung | Auswahl |
| --- | --- |
| Kontoerstellung | **Nur auf Einladung**, **Erlaubte E-Mail-Domains** oder **Alle**. Eingeladene können ihr Konto immer anlegen |
| Organisationserstellung | **Nur Instanz-Admins**, **Benutzer erlaubter Domains** oder **Alle angemeldeten Benutzer** |
| Erlaubte Domains | Die Domains für die beiden Einstellungen darüber. `*.example.com` umfasst Subdomains |
| Single Sign-on für Instanz-Admins | **Keine** oder eine Organisation mit Single Sign-on, deren Anbieter ihre verwalteten Instanz-Admins zur Verwaltung der Instanz anmelden darf ([Instanz-Admins](../self-hosting/sign-in.md#instanz-admins)) |
| Zeitzone | Die Zeitzone der Installation. Die Konsole zeigt jede Zeit darin, für alle, Anfrageprotokoll und Audit-Log filtern nach ihren Tagen, und automatische Updates richten sich nach ihr. **Diese Zone verwenden** wechselt zu der Ihres Browsers |
| Wechselkurse | Ob die Installation die Referenzkurse der EZB an jedem Arbeitstag holt, der Tag der neuesten und warum der letzte Abruf fehlschlug. Aus behält die vorhandenen Kurse ([Währungen](../concepts/currencies.md)) |
| Geteilte Links | **Geteilte Links erlauben** lässt Organisationen [Nutzung teilen](share-usage.md); aus, funktioniert kein Link mehr. **Erstellt mit Kaeso** zeigt ein kleines Zeichen am Fuß geteilter Seiten |
| Löschen von Organisationen | Ob sich Organisationen überhaupt löschen lassen, und die Schonfrist, 1 bis 90 Tage |

Unter **Protokollierung** legt **Aufbewahrung der Anfrageprotokolle** die Standards für neue Organisationen fest und die längste Aufbewahrung, die eine Organisation wählen darf. Wird die längste gesenkt, sinkt jede Organisation darüber auf sie, und ihre älteren Einträge werden sofort gelöscht, wie wenn eine Organisation ihre Aufbewahrung verkürzt.

Kontoerstellung, Organisationserstellung oder die erlaubten Domains zu ändern erfordert eine kürzliche Anmeldung. Single Sign-on für Instanz-Admins zu ändern erfordert eine mit Passwort oder Anmeldelink; ohne sie ändert ein Operator es auf dem Server mit `kaeso instance admin-sso` ([Kaeso-Befehle ausführen](../self-hosting/installation.md#kaeso-befehle-ausführen)). Die Einrichtungsseite setzt die Zeitzone anfangs auf die des ersten Benutzers, und Nutzungsdiagramme zählen weiter nach UTC-Tagen.

Passwörter, Anmeldelinks und E-Mail legt die [Konfiguration](../self-hosting/configuration.md) des Gateways fest. **Allgemein** zeigt, ob Passwörter und Anmeldelinks eingeschaltet sind, und **System** die E-Mail, beide nur zur Ansicht. Single Sign-on richtet jede Organisation auf ihrer Seite **Sicherheit** ein. **Test-E-Mail senden** auf **System** prüft, ob das Gateway E-Mails senden kann.

## Verknüpfte Abrechnung einrichten

Unter **Verknüpfte Abrechnung** richtet ein Instanz-Admin die OAuth-App der Installation bei jedem Anbieter mit [verknüpfter Abrechnung](linked-billing.md) ein, damit Organisationen diesen Anbieter über ihr eigenes Konto dort bezahlen können. Die Seite zeigt die zu registrierende Weiterleitungs-URI, den Aussteller und das Subject, gebildet aus der öffentlichen URL der Installation, nimmt die Client-ID der App und die übrigen Werte entgegen, und **Prüfen** testet die App, bevor jemand verknüpft. Solange keine App eingerichtet ist, sehen Organisationen die Option mit dem Hinweis, einen Instanz-Admin zu fragen. Wird die App entfernt oder ersetzt, hält das jeden verknüpften Schlüssel an, der dann **Instanz muss eingerichtet werden** zeigt; lehnt der Anbieter die App ab, bekommen Instanz-Admins eine E-Mail.

## Agenten zulassen

Agenten wie Claude Code oder Codex handeln über den [MCP-Server](../api/mcp.md) für ihren Benutzer. Unter **Allgemein** enthält **Agenten** die Agenten-Richtlinie der Instanz:

| Einstellung | Was sie bewirkt |
| --- | --- |
| Agenten zulassen | Standardmäßig an. Ausgeschaltet kann sich kein Agent anmelden, und jeder Agent erhält bei seiner nächsten Anfrage 403 |
| Höchstens auf der Instanz | **Keine**, **Lesen**, **Konfigurieren** oder **Admin**, für die Instanzrechte, die ein Instanz-Admin seinen Agenten geben kann. **Konfigurieren** ist der Standard |

Nur die Konsolensitzung eines Instanz-Admins ändert die Richtlinie; kein Agent kann das. Ein Agent macht nie jemanden zum Instanz-Admin, ändert nie Single Sign-on für Instanz-Admins und löscht nie einen Benutzer oder eine Organisation sofort. [Instanzrechte](../concepts/rights.md#instanzrechte) listet, was jedes Recht erlaubt.

## System prüfen

**System** zeigt Version und Zustand des Gateways und ob PostgreSQL, ClickHouse und Valkey laufen. ClickHouse erscheint als aus, wenn die Installation keine Anfrageprotokolle speichert. Es zeigt außerdem den Puffer der Anfrageprotokolle, den Speicher und wie das Gateway E-Mails sendet. Die Seite aktualisiert sich alle 10 Sekunden.

**Modelldaten** zeigt die Daten von [KPMR](../concepts/kpmr.md), die in Gebrauch sind: ihre Quelle, ihre Revision, wann sie abgerufen wurden, wie viele Modelle sie enthalten, die letzte Prüfung und gegebenenfalls ihren Fehler. Das Gateway hält die Daten selbst aktuell; nur die [Konfiguration](../self-hosting/configuration.md#kpmr) ändert, wie. Ist KPMR nicht erreichbar, bleiben die aktuellen Daten in Gebrauch.

## Kaeso aktualisieren

**Updates** zeigt die Version der Installation und das neueste Kaeso-Release mit den Release-Notizen aller Releases seither. Eine Markierung **Neu** neben **Updates** in der Seitenleiste der Einstellungen zeigt, dass ein neueres Release erschienen ist. Das Gateway sucht alle 5 Minuten nach Releases, und die Seite sagt, wenn eine Suche fehlgeschlagen ist.

- **Releases** bietet nur stabile Releases an oder auch Vorabversionen. Nur die [Konfiguration](../self-hosting/configuration.md) des Gateways schaltet die Suche ab, für Installationen ohne Internetzugang.
- Mit dem [Updater](../self-hosting/upgrades.md#der-updater) wählt **Wie Updates installiert werden** zwischen **Von Hand** und **Automatisch**. Automatisch installiert Kaeso neue Releases in einem Update-Fenster: täglich oder wöchentlich, zu einer Stunde in der Zeitzone der Installation. Eine neue Hauptversion oder ein Release mit Schritten von Hand wartet auf Sie.
- **Auf … aktualisieren** aktualisiert die Installation, nachdem Sie bestätigt haben, in beiden Modi, und verfolgt jeden Schritt, bis Kaeso wieder da ist.
- Ohne Updater, oder für ein Release, das Schritte von Hand verlangt, zeigt die Seite die Befehle und die Upgrade-Anleitung: die des Installers oder, bei einer Installation aus dem [Helm-Chart](../self-hosting/platforms/kubernetes.md), `helm upgrade`.

Der Verlauf listet jedes Update mit der Person, die es gestartet hat, oder **Automatisch**, und seinem Ausgang.

Nach einem Update sieht jeder, der die Konsole öffnet, einmal **Neu in Kaeso**. Der Dialog zeigt die neue Version auf ihrem eigenen Artwork, das sich nie ändert, mit ihren Verbesserungen, und **Nächstes** führt durch jedes verpasste Release. **Weiter zur Konsole**, Escape oder ein Klick daneben schließt den Dialog. **Neuigkeiten ansehen** unter **Updates** zeigt die Notizen der laufenden Version erneut.

## Audit-Log lesen

Das **Audit-Log** der Instanz verzeichnet Änderungen an Benutzern, Organisationen und den Einstellungen der Installation, jeweils mit Urheber und Zeitpunkt. Es kennzeichnet Änderungen von Agenten wie das Audit-Log einer Organisation, und der Filter **Agenten** zeigt nur diese ([Audit-Log lesen](organizations-and-projects.md#audit-log-lesen)). **JSON Lines exportieren** lädt die Ereignisse herunter, die die Filter zeigen, eines pro Zeile, das neueste zuerst, und das Log verzeichnet den Export. `instance/audit-log/export` der Platform API tut dasselbe mit dem Recht `instance.audit_log.export`.

Das Blatt eines Benutzers unter **Benutzer** öffnet es mit **Änderungen dieser Person** bei seinen Änderungen und mit **Änderungen an dieser Person** bei den Änderungen an ihm.

## Nächste Schritte

- [Konfiguration](../self-hosting/configuration.md): Legen Sie Anmeldemethoden, E-Mail und andere Einstellungen des Gateways fest.
- [Upgrades](../self-hosting/upgrades.md): Bringen Sie die Installation auf ein neues Release, mit oder ohne Updater.
- [Anmeldung und Single Sign-on](../self-hosting/sign-in.md): Kommen Sie wieder hinein, wenn sich niemand mehr anmelden kann.
