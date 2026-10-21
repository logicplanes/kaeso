---
title: Anmeldung und Single Sign-on
description: Legen Sie Anmeldemethoden fest, betreiben Sie Kaeso nur mit Single Sign-on und kommen Sie wieder hinein, wenn nichts geht.
order: 7
---

Personen melden sich an der Konsole mit einem Passwort an, mit einem Anmeldelink per E-Mail oder über den Identitätsanbieter ihrer Organisation per OpenID Connect. Drei [Einstellungen zur Anmeldung](configuration.md#anmeldung) schalten jede Methode für die ganze Installation ein oder aus. Ein Admin der Organisation verbindet den Identitätsanbieter auf der Seite **Sicherheit** der Organisation ([Organisationen und Projekte](../console/organizations-and-projects.md)).

## Die Anmeldeseite

Die Anmeldeseite fragt zuerst nach der E-Mail-Adresse. Dann bietet sie an, was für diese Adresse funktioniert: ein Passwort, einen Anmeldelink und Single Sign-on, wenn die Domain der Adresse zu einer Organisation mit Identitätsanbieter gehört.

In zwei Fällen zeigt die Seite sofort den Button eines Anbieters:

- Passwörter und Anmeldelinks sind abgeschaltet, und genau eine Organisation hat einen Identitätsanbieter.
- Jemand öffnet den Anmeldelink der Organisation, etwa `https://kaeso.example.com/sign-in?org=org_…`, den ihre Seite **Sicherheit** zeigt. Solange andere Methoden eingeschaltet sind, bietet die Seite außerdem **Stattdessen mit E-Mail anmelden** an.

Auf dem Button steht **Weiter mit** und der Name, den der Admin dem Anbieter gegeben hat, ohne Namen **Weiter mit SSO**. Ist **Ohne Klick starten** eingeschaltet, schickt die Seite Personen sofort zum Anbieter, wenn er ihr einziger Weg hinein ist. Nach einer Abmeldung wartet sie trotzdem auf einen Klick, ebenso wenn der Anbieter eine Anmeldung abgelehnt hat, damit der Grund lesbar bleibt, und wenn der Browser schon angemeldet ist.

Nach der Anmeldung kehren Personen zu der Konsolenseite zurück, die sie zuerst geöffnet hatten, auch über den Identitätsanbieter.

Verlangt eine Aktion eine kürzliche Anmeldung, bittet die Konsole den Anbieter, die Person erneut anzumelden (`prompt=login`, `max_age=0`), wie OpenID Connect es vorsieht. Sagt die Antwort eines Anbieters, dass die letzte Anmeldung älter als die Anfrage ist, oder sagt sie nicht, wann sie war (`auth_time`), wird sie mit `stale_sign_in` abgelehnt; eine Bestätigung bedeutet also immer eine frische Anmeldung.

Ein Anbieter kann eine Person auch ohne Nachfrage mit einer eigenen, älteren Sitzung anmelden. Eine Sitzung aus Single Sign-on zählt deshalb ab dem `auth_time` des Anbieters, nicht ab der Rückkehr zu Kaeso, und gilt nur als kürzliche Anmeldung, solange dieser Zeitpunkt höchstens 15 Minuten zurückliegt. Kaeso misst ihn am Ausstellungszeitpunkt `iat` des Tokens, auf der Uhr des Anbieters; die Uhren beider Server müssen also nicht übereinstimmen. Ohne `auth_time` gilt die Sitzung von Anfang an als älter als 15 Minuten.

## Links, die direkt zum Anbieter führen

Ein Link auf `/platform/v1/auth/oidc/start` schickt einen Browser ohne Anmeldung direkt zum Identitätsanbieter, etwa für ein Lesezeichen oder eine Intranetseite:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` nennt die Organisation; Sie können es weglassen, solange nur eine Organisation Single Sign-on nutzt. `next` ist die Konsolenseite, die sich danach öffnet. Der Link öffnet die Anmeldeseite der Organisation, die Organisation und Anbieter nennt und sofort zum Anbieter weitergeht.

Ist der Browser schon bei Kaeso angemeldet, fragt die Seite zuerst: Sie nennt das angemeldete Konto und wartet auf einen Klick. Single Sign-on ersetzt nie die Sitzung eines anderen Kontos. Kommt vom Anbieter ein anderes Konto zurück, bleibt der Browser angemeldet, und die Seite meldet `signed_in_as_other`; zum Wechseln melden Sie sich zuerst ab. Ein auf einer anderen Website platzierter Link kann einen angemeldeten Browser also weder abmelden noch im Konto einer anderen Person anmelden.

## Wer sich über Single Sign-on anmeldet

- Nur Adressen unter den bestätigten E-Mail-Domains der Verbindung ([E-Mail-Domains bestätigen](../console/organizations-and-projects.md#e-mail-domains-bestätigen)). Jede Domain bestätigt auf einer Installation eine Organisation.
- Nur Adressen, die der Anbieter mit `email_verified` als bestätigt kennzeichnet. Für einen Anbieter, der den Claim nie sendet, kann eine Verbindung **Bestätigte Adressen verlangen** abschalten; eine als unbestätigt gekennzeichnete Adresse bleibt abgewiesen.
- Wer neu bei Kaeso ist, tritt der Organisation mit einer Einladung an seine Adresse bei oder mit der Rolle, die die Verbindung Personen ohne Einladung gibt. Ohne beides erhält er keinen Zugang. Kaeso legt das Konto auch an, wo die [Einstellung zur Kontoerstellung](../console/instance-administration.md#installationseinstellungen-ändern) keine Registrierungen erlaubt, denn der Anbieter der Organisation bürgt für die Person.
- Kaeso verknüpft jedes Konto mit seiner Identität beim Anbieter, dem Aussteller und dem Subjekt des Anbieters. Spätere Anmeldungen folgen dieser Verknüpfung, nicht der E-Mail-Adresse.
- Ein bestehendes Konto meldet sich über Single Sign-on erst an, wenn seine Inhaberin oder sein Inhaber es nach einer Anmeldung auf anderem Weg unter **Konto** › **Sicherheit** verknüpft ([Single Sign-on verknüpfen](../console/account.md#single-sign-on-verknüpfen)), und nur dort, wo es Mitglied ist. Um eine Einladung anzunehmen, meldet es sich zuerst auf anderem Weg an.
- Eine Adresse unter einer anderen Domain, etwa der eines öffentlichen Mail-Anbieters, kann sich nicht über den Anbieter der Organisation anmelden, auch nicht mit Einladung.

Eine Anmeldung über den Anbieter einer Organisation öffnet nur diese Organisation und liest das eigene Konto der Person. Andere Organisationen, die Verwaltung der Instanz und Änderungen an E-Mail, Passwort und Zwei-Faktor-Authentifizierung des Kontos brauchen eine weitere Anmeldung ([nach einer Anmeldung über Single Sign-on](../console/account.md#nach-einer-anmeldung-über-single-sign-on)).

Eine Verbindung kann keine Domain eines öffentlichen Mail-Anbieters wie `gmail.com` enthalten: Jede Adresse dort würde sich über den Anbieter einer einzigen Organisation anmelden.

## Nur mit Single Sign-on betreiben

Damit sich Personen nur über Identitätsanbieter anmelden, schalten Sie vor dem ersten Start die anderen Methoden ab:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

1. Geben Sie dem ersten Admin eine Adresse unter einer Domain, die der Identitätsanbieter abdecken wird, damit er sich später darüber anmelden kann.
2. Öffnen Sie den Einrichtungslink. Die Einrichtungsseite fragt nach keinem Passwort und öffnet danach die Seite **Sicherheit** der Organisation.
3. Verbinden Sie den Identitätsanbieter in derselben Sitzung.
4. Wählen Sie unter **Konto** › **Sicherheit** die Option **Verknüpfen** neben der Organisation und melden Sie sich beim Anbieter an. Das verknüpft Ihr Konto und prüft, dass die Verbindung funktioniert.

Eine Anmeldung über Single Sign-on verwaltet die Instanz nicht. Für die Verwaltung der Instanz melden Sie sich mit einem [einmaligen Anmeldelink](#wieder-hineinkommen) vom Server an.

> [!IMPORTANT]
> Lassen Sie **Single Sign-on verlangen** ausgeschaltet, bis die Verbindung funktioniert. Auf einer Installation ohne Passwörter bringt es nichts, und es lehnt jede andere Sitzung ab, auch eine über den Anmeldelink eines Betreibers.

## Wieder hineinkommen

Wenn kein Weg hinein funktioniert, geben Sie auf dem Server einen einmaligen Anmeldelink für ein bestehendes Konto aus. Das hilft, wenn der Identitätsanbieter ausgefallen oder falsch eingerichtet ist oder die Einrichtungssitzung endete, bevor Single Sign-on funktionierte:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

Der Link funktioniert einmal, innerhalb von 15 Minuten, auch wenn Anmeldelinks per E-Mail abgeschaltet sind; Passwörter müssen Sie also nie wieder einschalten. Wer den Link hat, kann sich als diese Person anmelden. Kaeso gibt ihn nur in Ihrem Terminal aus, protokolliert oder sendet ihn nie, und das Audit-Log hält ihn als `user.sign_in_link` fest. Ein deaktiviertes Konto erhält erst einen Link, wenn Sie es wieder aktivieren. Bei eingeschalteter Zwei-Faktor-Authentifizierung fragt auch der Link nach einem Code; hat die Person ihren Authenticator verloren, entfernt `kaeso user reset-2fa` sie zuerst.

## Nächste Schritte

- [Instanzverwaltung](../console/instance-administration.md): Verwalten Sie Benutzer, Organisationen und die Einstellungen der Instanz.
- [Fehlerbehebung](troubleshooting.md): Beheben Sie häufige Probleme einer Installation.
