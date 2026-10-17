---
title: Konto und Sprache
description: Profil, Passwort, Zwei-Faktor-Authentifizierung, Sitzungen, Design und Sprache des eigenen Kontos.
order: 9
---

Das eigene Konto liegt unter **Konto** im Menü am Fuß der Seitenleiste:

| Seite | Inhalt |
| --- | --- |
| Profil | Bild, Name, E-Mail-Adresse und Konto-ID; das Konto löschen |
| Sicherheit | Passwort, Zwei-Faktor-Authentifizierung und Wiederherstellungscodes |
| Sitzungen | Die Browser, in denen man angemeldet ist |
| API-Schlüssel | Die eigenen Schlüssel in allen Projekten ([API-Schlüssel](api-keys.md)) |
| Mitgliedschaften | Die eigenen Organisationen und ihre Projekte mit den eigenen Rollen; eine Organisation verlassen |
| Benachrichtigungen | Welche E-Mails man erhält |
| Darstellung | Design und Sprache |

## E-Mail-Adresse

**E-Mail ändern** sendet einen Bestätigungslink an die neue Adresse und informiert die alte. Die Änderung gilt, sobald der Link geöffnet wird. Sie erfordert eine kürzliche Anmeldung.

## Passwort

**Passwort ändern** meldet die anderen Sitzungen ab. Wer sich nur mit Links oder Single Sign-on anmeldet und kein Passwort hat, legt mit **Passwort festlegen** eines an.

## Zwei-Faktor-Authentifizierung

Die Zwei-Faktor-Authentifizierung verlangt bei jeder Anmeldung mit Passwort einen Code aus einer Authenticator-App; Anmeldelinks und Single Sign-on brauchen keinen.

1. Unter **Sicherheit** **Einrichten** wählen.
2. Den QR-Code mit der App scannen oder den Einrichtungsschlüssel eingeben.
3. Den sechsstelligen Code aus der App eingeben und **Einschalten** wählen.
4. Die 10 Wiederherstellungscodes sichern, die die Konsole nur dieses eine Mal zeigt: kopieren oder **.txt herunterladen**. Jeder ersetzt einmal einen Code bei der Anmeldung.

**Neu erzeugen** ersetzt die Wiederherstellungscodes, und **Ausschalten** entfernt die Zwei-Faktor-Authentifizierung; beides erfordert eine kürzliche Anmeldung. Wer Authenticator und Wiederherstellungscodes verloren hat, lässt die Zwei-Faktor-Authentifizierung von einem Instanz-Admin zurücksetzen.

## Sitzungen

**Sitzungen** listet jeden Browser, in dem man angemeldet ist, und wie die Anmeldung erfolgte. **Beenden** meldet einen davon ab, **Überall abmelden** alle außer dem aktuellen. Eine Sitzung endet nach 7 Tagen ohne Nutzung und spätestens nach 30 Tagen.

## Eine Organisation verlassen und das Konto löschen

**Organisation verlassen** unter **Mitgliedschaften** nimmt einen aus der Organisation und ihren Projekten und archiviert die eigenen Schlüssel dort. **Konto löschen** unter **Profil** nimmt einen aus jeder Organisation und archiviert alle eigenen Schlüssel; im Verlauf erscheint man als gelöschter Benutzer. Bestätigt wird mit der eigenen E-Mail-Adresse, und es erfordert eine kürzliche Anmeldung.

Beides geht nicht, solange man der letzte Inhaber einer Organisation ist: zuerst jemand anderen zum Inhaber machen.

## Benachrichtigungen

**Budget-Warnungen** und **Ablaufende Schlüssel** schalten diese E-Mails ein oder aus. Sicherheits-E-Mails, etwa zu einem geänderten Passwort, werden immer gesendet.

## Design und Sprache

Unter **Darstellung**:

- **Design**: **System**, **Hell** oder **Dunkel**. Es wird in diesem Browser gespeichert.
- **Sprache**: **Automatisch**, English, Deutsch oder 简体中文. Automatisch folgt der Sprache des Browsers, oder Englisch, wenn die Konsole diese Sprache nicht hat. Die Wahl wird im Konto gespeichert, gilt also auch in anderen Browsern, und E-Mails von Kaeso verwenden sie ebenfalls.

Produktnamen und Modell-IDs bleiben in jeder Sprache unverändert.
