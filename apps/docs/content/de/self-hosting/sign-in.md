---
title: Anmeldung und Single Sign-on
description: Festlegen, wie sich Personen anmelden, eine Installation nur mit Single Sign-on betreiben und wieder hineinkommen, wenn nichts anderes geht.
order: 7
---

Personen melden sich an der Konsole mit einem Passwort an, mit einem Anmeldelink per E-Mail oder über den Identitätsanbieter ihrer Organisation (Single Sign-on mit OpenID Connect). Drei [Einstellungen](configuration.md#anmeldung) schalten jede Methode für die ganze Installation ein oder aus: `KAESO_AUTH_PASSWORD`, `KAESO_AUTH_MAGIC_LINK` und `KAESO_AUTH_OIDC`. Ein Admin der Organisation verbindet den Identitätsanbieter unter **Sicherheit**; siehe [Single Sign-on](../console/organizations-and-projects.md#single-sign-on).

## Die Anmeldeseite

Die Anmeldeseite fragt meist zuerst nach der E-Mail-Adresse und bietet dann an, was für sie funktioniert: ein Passwort, einen Anmeldelink und Single Sign-on, wenn die Domain der Adresse zu einer Organisation mit Identitätsanbieter gehört.

In zwei Fällen überspringt sie die Adresse und zeigt sofort den Button des Anbieters:

| Fall | Was die Seite zeigt |
| --- | --- |
| Passwörter und Anmeldelinks sind aus, und genau eine Organisation hat einen Identitätsanbieter | Den Button dieses Anbieters |
| Jemand öffnet den Anmeldelink einer Organisation, `https://kaeso.example.com/sign-in?org=org_…` | Den Button dieser Organisation und **Stattdessen mit E-Mail anmelden**, solange andere Methoden eingeschaltet sind |

Die Seite **Sicherheit** der Organisation zeigt ihren Anmeldelink. Der Button lautet **Weiter mit** und dem Namen, den der Admin dem Anbieter gegeben hat, etwa **Weiter mit Acme ID**, ohne Namen **Weiter mit SSO**. Nutzen mehrere Organisationen Single Sign-on und gibt es keinen Anmeldelink, fragt die Seite nach der Adresse und wählt die Organisation über deren Domain.

**Ohne Klick starten**, eine Option jeder Verbindung, schickt Personen zum Anbieter, sobald die Seite nur ihn anbietet, und direkt nachdem sie eine Adresse eingegeben haben, deren einziger Weg hinein dieser Anbieter ist. Auf einen Klick wartet die Seite trotzdem nach einer Abmeldung, damit die eigene Sitzung des Anbieters niemanden sofort wieder anmeldet, und nachdem der Anbieter eine Anmeldung abgelehnt hat, damit der Grund lesbar bleibt.

## Links, die direkt zum Anbieter führen

Ein Link auf `GET /platform/v1/auth/oidc/start` schickt den Browser direkt zum Identitätsanbieter, für ein Lesezeichen oder eine Intranetseite:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` nennt die Organisation und kann fehlen, solange genau eine Organisation Single Sign-on nutzt. Der Link beginnt eine Anmeldung nur in dem Browser, der ihn öffnet: Das Gateway bindet den Ablauf mit einem Cookie an diesen Browser und prüft State, Nonce und PKCE-Verifier, wenn der Anbieter den Browser zurückschickt. Angemeldet ist am Ende, wer sich beim Anbieter anmeldet; ein auf einer anderen Website platzierter Link kann also niemanden in ein fremdes Konto anmelden, und der Link braucht keinen `Origin`-Header. Jedes Problem endet auf der Anmeldeseite mit dem Grund.

## Auf die richtige Seite zurückkommen

Wer ohne Sitzung eine Seite der Konsole öffnet, landet auf der Anmeldeseite mit dieser Seite in `next` und kehrt nach der Anmeldung zu ihr zurück, auch über den Identitätsanbieter. Das Gateway hält `next` beim Anmeldeablauf, nie in der URL des Anbieters. Es zählt nur ein Pfad der Konsole dieser Installation: Ein Wert, der mit `//` oder einem Backslash beginnt, eine andere Website nennt, Zeilenumbrüche enthält oder in die Platform API oder die Modell-API zeigt, wird ignoriert, und die Startseite öffnet sich. Schlägt eine Anmeldung fehl, beim Anbieter oder danach, behält die Anmeldeseite `next` für den nächsten Versuch. Sie behält auch die Organisation, wenn die Anmeldung über deren Anmeldelink oder Button begann, und zeigt neben dem Grund wieder den Button dieser Organisation.

## Wer sich per Single Sign-on anmeldet

- Nur Adressen der E-Mail-Domains der Verbindung. Jede Domain gehört in einer Installation zu einer Organisation.
- Nur Adressen, die der Anbieter nicht als unbestätigt kennzeichnet.
- Wer neu bei Kaeso ist, tritt der Organisation mit ihrer Rolle für Personen ohne Einladung bei oder mit einer Einladung an die eigene Adresse; ohne beides gibt es keinen Zugang.
- Ein bestehendes Konto meldet sich per Single Sign-on nur dort an, wo es bereits Mitglied ist. Um eine Einladung anzunehmen, meldet es sich zuerst anders an.
- Eine Adresse einer anderen Domain, etwa eines öffentlichen Mailanbieters, kann sich nicht über den Anbieter der Organisation anmelden, auch nicht mit einer Einladung. Auf einer Installation mit ausgeschalteten Passwörtern und Anmeldelinks hat eine solche Person keinen Weg hinein. Die Domain des öffentlichen Mailanbieters nicht in die Verbindung aufnehmen, um das zu umgehen: Jede Adresse dieser Domain würde sich dann über den eigenen Anbieter anmelden, und keine andere Organisation könnte die Domain nutzen.

## Eine Installation ohne Passwörter

Um nur mit Single Sign-on zu arbeiten, die anderen Methoden ausschalten:

```yaml
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

Die Einrichtungsseite fragt dann nach keinem Passwort, und der erste Admin erhält nie eines. Nach der Einrichtung öffnet sich die Seite **Sicherheit** der Organisation, damit der Identitätsanbieter in derselben Sitzung verbunden werden kann. Dem ersten Admin eine Adresse einer der Domains der Verbindung geben, damit er sich danach darüber anmelden kann.

**Single Sign-on verlangen** ausgeschaltet lassen, bis die Verbindung funktioniert: Auf einer solchen Installation bringt die Option nichts, und eine Organisation, die Single Sign-on verlangt, lehnt jede andere Sitzung ab, auch eine aus dem Anmeldelink eines Betreibers.

## Wieder hineinkommen

Führt kein Weg hinein, weil der Identitätsanbieter ausgefallen oder falsch eingerichtet ist oder die Sitzung der Einrichtung endete, bevor Single Sign-on funktionierte, auf dem Server einen einmaligen Anmeldelink für ein bestehendes Konto ausgeben:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

Der Link funktioniert einmal, innerhalb von 15 Minuten, auch wenn Anmeldelinks per E-Mail ausgeschaltet sind; Passwörter müssen also nie wieder eingeschaltet werden. Wer ihn hat, kann sich als diese Person anmelden: Er erscheint nur im Terminal, wird nie protokolliert oder verschickt, und das Audit-Log verzeichnet ihn als `user.sign_in_link`. Ein deaktiviertes Konto erhält keinen Link; es zuerst wieder aktivieren.
