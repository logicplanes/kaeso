---
title: Anmeldung und Single Sign-on
description: Festlegen, wie sich Personen anmelden, eine Installation nur mit Single Sign-on betreiben und wieder hineinkommen, wenn nichts anderes geht.
order: 5
---

Personen melden sich an der Konsole mit einem Passwort an, mit einem Anmeldelink per E-Mail oder über den Identitätsanbieter ihrer Organisation (Single Sign-on, OpenID Connect). Drei [Einstellungen](configuration.md#anmeldung) schalten jede Methode für die ganze Installation ein oder aus: `KAESO_AUTH_PASSWORD`, `KAESO_AUTH_MAGIC_LINK` und `KAESO_AUTH_OIDC`. Ein Admin der Organisation verbindet den Identitätsanbieter unter **Sicherheit**; siehe [Single Sign-on](../console/organizations-and-projects.md#single-sign-on).

## Die Anmeldeseite

Die Anmeldeseite fragt meist zuerst nach der E-Mail-Adresse und bietet dann an, was für sie funktioniert: ein Passwort, einen Anmeldelink und Single Sign-on, wenn die Domain der Adresse zu einer Organisation mit Identitätsanbieter gehört.

In zwei Fällen überspringt sie die Adresse und zeigt sofort die Schaltfläche des Anbieters:

| Fall | Was die Seite zeigt |
| --- | --- |
| Die Installation meldet sich nur per Single Sign-on an (Passwörter und Anmeldelinks sind aus), und genau eine Organisation hat einen Identitätsanbieter | Die Schaltfläche dieses Anbieters |
| Jemand öffnet den Anmeldelink einer Organisation, `https://kaeso.example.com/sign-in?org=org_…` | Die Schaltfläche dieser Organisation und **Stattdessen mit E-Mail anmelden**, solange andere Methoden eingeschaltet sind |

Die Seite **Sicherheit** der Organisation zeigt ihren Anmeldelink. Die Schaltfläche lautet **Weiter mit** und dem Namen, den der Admin dem Anbieter gegeben hat, etwa **Weiter mit Acme ID**, ohne Namen **Weiter mit SSO**. Nutzen mehrere Organisationen Single Sign-on und gibt es keinen Anmeldelink, fragt die Seite nach der Adresse, deren Domain die Organisation bestimmt.

**Ohne Klick starten**, eine Option jeder Verbindung, schickt Personen zum Anbieter, sobald die Seite nur ihn anbietet, und direkt nachdem sie eine Adresse eingegeben haben, deren einziger Weg hinein dieser Anbieter ist. Die Seite wartet auf einen Klick, nachdem sich jemand abgemeldet hat, damit die eigene Sitzung des Anbieters ihn nicht sofort wieder anmeldet, und nachdem der Anbieter eine Anmeldung abgelehnt hat, damit der Grund lesbar bleibt.

## Links, die direkt zum Anbieter führen

Ein Link auf `GET /platform/v1/auth/oidc/start` schickt den Browser direkt zum Identitätsanbieter, für ein Lesezeichen oder eine Intranetseite:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` nennt die Organisation; es kann fehlen, solange genau eine Organisation Single Sign-on nutzt. Der Link beginnt eine Anmeldung nur in dem Browser, der ihn öffnet: Das Gateway bindet den Ablauf mit einem Cookie an diesen Browser und prüft State, Nonce und PKCE-Verifier, wenn der Anbieter den Browser zurückschickt. Angemeldet ist am Ende, wer sich beim Anbieter anmeldet; ein auf einer anderen Website platzierter Link kann also niemanden in ein fremdes Konto anmelden, und der Link braucht keinen `Origin`-Header. Probleme enden auf der Anmeldeseite mit dem Grund.

## Auf die richtige Seite zurückkommen

Wer ohne Sitzung eine Seite der Konsole öffnet, kommt auf die Anmeldeseite mit dieser Seite in `next` und nach der Anmeldung zu ihr zurück, auch über den Identitätsanbieter. `next` bleibt beim Anmeldeablauf auf dem Gateway, nie in der URL des Anbieters. Es zählt nur ein Pfad der Konsole dieser Installation: Ein Wert, der mit `//` oder einem Backslash beginnt, eine andere Website nennt, Zeilenumbrüche enthält oder in die Platform API oder die Modell-API zeigt, wird abgelehnt, und die Startseite öffnet sich. Schlägt die Anmeldung fehl, beim Anbieter oder danach, behält die Anmeldeseite `next` für den nächsten Versuch und die Organisation, wenn die Anmeldung sie genannt hat, über ihren Anmeldelink oder ihre Schaltfläche, sodass die Seite neben dem Grund wieder die Schaltfläche dieser Organisation zeigt.

## Wer sich per Single Sign-on anmeldet

- Nur Adressen der E-Mail-Domains der Verbindung melden sich über sie an. Jede Domain gehört in einer Installation zu einer Organisation.
- Der Anbieter darf die Adresse nicht als unbestätigt kennzeichnen.
- Wer neu bei Kaeso ist, tritt der Organisation mit ihrer Rolle für Personen ohne Einladung bei oder mit einer Einladung an die eigene Adresse; ohne beides gibt es keinen Zugang.
- Ein bestehendes Konto meldet sich per Single Sign-on nur dort an, wo es bereits Mitglied ist; um eine Einladung anzunehmen, meldet es sich zuerst anders an.
- Eine Adresse einer anderen Domain, etwa eines öffentlichen Mailanbieters, kann sich nicht über den Anbieter der Organisation anmelden, auch nicht mit einer Einladung. Auf einer Installation mit ausgeschalteten Passwörtern und Anmeldelinks hat eine solche Person keinen Weg hinein. Die Domain eines öffentlichen Mailanbieters nicht eintragen, um das zu umgehen: Jede Adresse dieser Domain würde sich dann über den eigenen Anbieter anmelden, und keine andere Organisation könnte die Domain nutzen.

## Eine Installation ohne Passwörter

Um nur mit Single Sign-on zu arbeiten, die anderen Methoden ausschalten:

```yaml
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

Die Einrichtungsseite fragt dann nach keinem Passwort, und der erste Admin bekommt nie eines. Nach der Einrichtung öffnet sie die Seite **Sicherheit** der Organisation, um den Identitätsanbieter in derselben Sitzung zu verbinden. Dem ersten Admin eine Adresse einer der Domains der Verbindung geben, damit er sich danach darüber anmelden kann. Single Sign-on zu verlangen bringt auf einer solchen Installation nichts; es ausgeschaltet lassen, bis die Verbindung funktioniert, denn eine Organisation, die Single Sign-on verlangt, lehnt jede andere Sitzung ab, auch die aus dem Anmeldelink eines Betreibers.

## Wieder hineinkommen

Führt kein Weg hinein, weil der Identitätsanbieter ausgefallen oder falsch eingerichtet ist oder die Sitzung der Einrichtung endete, bevor Single Sign-on funktionierte, dort, wo das Gateway läuft, einen einmaligen Anmeldelink für ein bestehendes Konto ausgeben:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

Der Link funktioniert einmal, innerhalb von 15 Minuten, auch wenn Anmeldelinks per E-Mail ausgeschaltet sind, und Passwörter müssen dafür nicht wieder eingeschaltet werden. Wer ihn hat, kann sich als diese Person anmelden; deshalb erscheint er nur im Terminal, wird nie protokolliert oder verschickt, und das Audit-Log verzeichnet ihn als `user.sign_in_link`.

## Der Einrichtungslink

Solange eine Installation keine Benutzer hat, meldet das Log des Gateways, dass sie noch nicht eingerichtet ist, nie den Einrichtungslink selbst: Sonst könnte jeder, der die Logs lesen kann, die Installation übernehmen. `kaeso setup-link` gibt den Link aus; siehe [Was beim ersten Start geschieht](installation.md#was-beim-ersten-start-geschieht).
