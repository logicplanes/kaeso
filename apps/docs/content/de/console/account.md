---
title: Konto und Sprache
description: Profil, Passwort, Zwei-Faktor-Authentifizierung, Sitzungen, Design und Sprache des eigenen Kontos.
order: 9
---

Das eigene Konto liegt unter **Einstellungen** › **Konto**: im Menü am Fuß der Seitenleiste **Konto** wählen.

| Seite | Inhalt |
| --- | --- |
| Profil | Bild, Name, E-Mail-Adresse, Konto-ID und das Löschen des Kontos |
| Sicherheit | Passwort, Zwei-Faktor-Authentifizierung und Wiederherstellungscodes |
| Sitzungen | Wo das Konto angemeldet ist |
| API-Schlüssel | Die eigenen Schlüssel in allen Projekten |
| Mitgliedschaften | Die eigenen Organisationen und Projekte mit den Rollen darin; eine Organisation verlassen |
| Benachrichtigungen | Welche E-Mails ankommen |
| Darstellung | Design und Sprache |

## E-Mail-Adresse

**E-Mail ändern** sendet einen Bestätigungslink an die neue Adresse und informiert die alte. Die Änderung gilt, sobald der Link geöffnet wird. Sie erfordert eine kürzliche Anmeldung.

## Passwort und Zwei-Faktor-Authentifizierung

**Passwort ändern** meldet die anderen Sitzungen ab. Wer kein Passwort hat, weil die Anmeldung per Link oder Single Sign-on erfolgt, legt mit **Passwort festlegen** eines an.

Die Zwei-Faktor-Authentifizierung nutzt eine Authenticator-App und gilt für die Anmeldung mit Passwort; Anmeldelinks brauchen keinen Code. **Einrichten** zeigt einen QR-Code und einen Einrichtungsschlüssel; den sechsstelligen Code aus der App eingeben und **Einschalten** wählen. Dazu gibt es 10 Wiederherstellungscodes, jeder einmal anstelle eines Codes nutzbar und nur in diesem Moment angezeigt; sie kopieren oder herunterladen und sicher aufbewahren. **Neu erzeugen** ersetzt sie, und **Ausschalten** entfernt die Zwei-Faktor-Authentifizierung.

Wenn Authenticator und Wiederherstellungscodes beide verloren sind, kann ein Instanz-Admin die Zwei-Faktor-Authentifizierung zurücksetzen.

## Sitzungen

**Sitzungen** listet jeden Browser, in dem das Konto angemeldet ist, und wie die Anmeldung erfolgte. **Beenden** meldet eine Sitzung ab; **Überall abmelden** meldet alle außer der aktuellen ab. Sitzungen enden von selbst nach 7 Tagen ohne Nutzung und spätestens nach 30 Tagen.

## Verlassen und löschen

**Organisation verlassen** unter **Mitgliedschaften** entfernt das Konto aus der Organisation und ihren Projekten und archiviert die eigenen Schlüssel dort. **Konto löschen** unter **Profil** entfernt das Konto aus allen Organisationen und archiviert alle eigenen Schlüssel; zur Bestätigung die eigene E-Mail-Adresse eingeben, und es erfordert eine kürzliche Anmeldung. Beides geht nicht, solange das Konto der letzte Inhaber einer Organisation ist: vorher jemand anderen zum Inhaber machen.

## Benachrichtigungen

**Budget-Warnungen** und **Ablaufende Schlüssel** schalten diese E-Mails ein oder aus. Sicherheits-E-Mails, etwa zu einem geänderten Passwort, werden immer gesendet.

## Design und Sprache

Unter **Darstellung**:

- **Design**: **System**, **Hell** oder **Dunkel**. Die Wahl gilt in diesem Browser.
- **Sprache**: **Automatisch**, English, Deutsch oder 简体中文. Automatisch folgt der Sprache des Browsers und fällt auf Englisch zurück. Die Wahl wird im Konto gespeichert, gilt also auch in anderen Browsern, und E-Mails von Kaeso nutzen sie ebenfalls.

Produktnamen und Modell-IDs bleiben in jeder Sprache, wie sie sind.
