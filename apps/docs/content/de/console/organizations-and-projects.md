---
title: Organisationen und Projekte
description: Erstellen Sie Organisationen und Projekte, verbinden Sie Single Sign-on und verwalten Sie ihre Einstellungen.
order: 1
---

Eine Organisation umfasst Mitglieder, Anbieter-Schlüssel und Projekte. Ein Projekt umfasst einen Modellkatalog, API-Schlüssel, Dienstkonten, Limits und Anfrageprotokolle. Personen gehören einer Organisation mit einer Organisationsrolle an und jedem ihrer Projekte mit einer Projektrolle.

## Organisation erstellen

Öffnen Sie das Menü am Fuß der Seitenleiste und wählen Sie **Organisation erstellen**. Sie werden Inhaber der neuen Organisation, und sie beginnt mit einem Standardprojekt. Wer Organisationen erstellen darf, legt die Installation fest ([Instanzeinstellungen](instance-administration.md#installationseinstellungen-ändern)).

Dasselbe Menü listet Ihre Organisationen und wechselt zwischen ihnen.

## Nutzung ablesen

Die **Übersicht** der Organisation zeigt Anfragen, Token, Kosten und Fehler für einen Zeitraum Ihrer Wahl, von heute bis zur gesamten Laufzeit. Sie schlüsselt außerdem die Kosten nach Projekt auf und die Nutzung nach Projekt, Modell oder Mitglied. Für die Nutzung brauchen Sie das Recht **Gesamtnutzung ansehen**, für die Kosten **Gesamtkosten ansehen**.

Die **Übersicht** eines Projekts zeigt dieselben Zahlen für ein Projekt, aufgeschlüsselt nach Zeit, Modell, Schlüssel oder Mitglied. Sie listet außerdem die Limits des Projekts mit ihrem Verbrauch und die letzten Fehler.

## Organisationseinstellungen ändern

Öffnen Sie **Organisationseinstellungen** im Menü am Fuß der Seitenleiste. Dort finden Sie diese Seiten:

| Seite | Inhalt |
| --- | --- |
| Allgemein | Bild, Name und Organisations-ID, **Inhaberschaft übertragen** und das Löschen der Organisation |
| Mitglieder | Mitglieder, Einladungen und Rollen ([Mitglieder und Rollen](members-and-roles.md)) |
| Sicherheit | Single Sign-on, die Anforderungen an die Anmeldung und die maximale Schlüssellaufzeit |
| Plattform-Schlüssel | Schlüssel für Skripte, die die Organisation verwalten ([Plattform-Schlüssel](api-keys.md#plattform-schlüssel)) |
| Protokollierung | Inhaltsprotokollierung, Aufbewahrung, Speicher und das Löschen von Protokollen ([Anfrageprotokolle](request-logs.md#aufbewahrung)) |
| Audit-Log | Jede Änderung in der Organisation ([Audit-Log](#audit-log-lesen)) |

Ein Bild ist eine PNG-, JPEG- oder WebP-Datei, die die Konsole quadratisch zuschneidet.

## Single Sign-on

Mit Single Sign-on melden sich Mitglieder über Ihren OpenID-Connect-Identitätsanbieter an. Legen Sie zuerst beim Anbieter eine App für Kaeso an. Wählen Sie dann unter **Sicherheit** **Identitätsanbieter verbinden** und beantworten Sie fünf Schritte:

1. **Wo ist Ihr Identitätsanbieter?** Geben Sie die Issuer-URL der App ein, etwa `https://login.example.com`.
2. **Welche App meldet Personen an?** Geben Sie ihre Client-ID und ihr Client-Secret ein.
3. **Welche E-Mail-Domains melden sich darüber an?** Die Anmeldeseite leitet Personen mit einer Adresse unter diesen Domains zu Ihrem Anbieter. Internationalisierte Domains funktionieren in beiden Schreibweisen, `bücher.example` oder `xn--bcher-kva.example`.
4. **Beitritt ohne Einladung möglich?** Wählen Sie **Nur mit Einladung** oder die Rolle, mit der Personen ohne Kaeso-Konto beitreten. Diese Rolle kann nicht Inhaber sein.
5. **Wie bietet die Anmeldeseite ihn an?** Tragen Sie unter **Name auf der Schaltfläche** einen Namen ein, etwa `Acme ID` für „Weiter mit Acme ID“. Bleibt das Feld leer, steht auf dem Button „Weiter mit SSO“. **Ohne Klick starten** leitet Personen direkt zu Ihrem Anbieter, wenn die Anmeldeseite nur ihn anbietet, außer direkt nach dem Abmelden oder wenn der Anbieter sie abgewiesen hat. **Bestätigte Adressen verlangen** weist Tokens ab, die die Adresse nicht als bestätigt kennzeichnen; schalten Sie es nur für einen Anbieter ab, der `email_verified` nie sendet.

Nach dem Verbinden zeigt die Seite zwei Adressen:

- Die **Weiterleitungs-URL**: Tragen Sie sie beim Anbieter als Callback der Anmeldung ein.
- Den **Anmeldelink**: Er öffnet die Anmeldeseite mit dem Button Ihres Anbieters, sodass Mitglieder keine Adresse eingeben. Teilen Sie ihn, etwa im Intranet.

### E-Mail-Domains bestätigen

Single Sign-on meldet nur Adressen bestätigter Domains an und lässt nur sie beitreten, damit keine Organisation eine Domain beansprucht, die ihr nicht gehört. Für jede Domain zeigt **Sicherheit** einen TXT-Eintrag. Legen Sie ihn bei Ihrem DNS-Anbieter an und wählen Sie dann **Domains bestätigen**:

```text
_kaeso-verify.acme.example  TXT  "kaeso-verify=Qm9ZtX3c8L1aV7pK2rN5wE0yH4uJ6sD9fG1bC3xT8eA"
```

DNS-Änderungen brauchen etwas, bis sie sichtbar sind; wählen Sie **Domains bestätigen** später erneut. Domains öffentlicher Mail-Anbieter wie `gmail.com` lassen sich nicht hinzufügen, ebenso wenig eine Domain, die eine andere Organisation bestätigt hat. Eine Domain, für die das öffentliche DNS nicht antwortet, bestätigt ein Operator auf dem Server mit `kaeso sso verify-domain acme.example`.

**Single Sign-on bearbeiten** ändert die Verbindung. Ein leeres Client-Secret behält das gespeicherte, solange Issuer und Client-ID gleich bleiben. **Trennen** entfernt sie, und Mitglieder melden sich wieder mit Passwort oder Anmeldelink an.

Single Sign-on meldet ein bestehendes Kaeso-Konto erst an, wenn seine Inhaberin oder sein Inhaber es unter **Konto** mit Ihrem Anbieter verknüpft ([Single Sign-on verknüpfen](account.md#single-sign-on-verknüpfen)), und nur, wenn das Konto der Organisation angehört. Laden Sie die Person ein, und sie nimmt die Einladung nach einer Anmeldung auf anderem Weg an. Single Sign-on zu ändern verlangt eine Konsolensitzung mit kürzlicher Anmeldung; Plattform-Schlüssel können es nicht. [Anmeldung und Single Sign-on](../self-hosting/sign-in.md) enthält alle Regeln und beschreibt, wie eine Installation allein mit Single Sign-on läuft.

## Single Sign-on oder Zwei-Faktor-Authentifizierung verlangen

Unter **Sicherheit** legen Sie fest, wie Mitglieder in die Organisation gelangen:

| Einstellung | Wirkung |
| --- | --- |
| Single Sign-on verlangen | Mitglieder gelangen nur über den Identitätsanbieter in die Organisation. Ihre anderen Sitzungen gelten weiter für ihre übrigen Organisationen. Setzt einen verbundenen Anbieter voraus |
| Zwei-Faktor-Authentifizierung verlangen | Mitglieder brauchen eine Anmeldung mit Passwort und einem Code aus ihrer Authenticator-App. Ein Anmeldelink reicht nicht; eine Anmeldung über den eigenen Identitätsanbieter der Organisation schon |
| Maximale Schlüssellaufzeit | Die längste Laufzeit eines neuen Schlüssels in Tagen, oder **Kein Limit**. Bestehende Schlüssel behalten ihren Ablauf, und ein Projekt kann ein kürzeres Limit setzen |

Wenn Sie eine Anforderung einschalten, die Sie selbst nicht erfüllen, ist die Organisation auch für Sie gesperrt. Das gilt, bis Sie sich über den Anbieter anmelden oder die Zwei-Faktor-Authentifizierung einrichten. Die Konsole warnt Sie vor dem Speichern. Wenn Sie den Identitätsanbieter trennen, schaltet das auch **Single Sign-on verlangen** aus.

## Organisation löschen

Wählen Sie unter **Allgemein** **Organisation löschen** und bestätigen Sie mit dem Namen der Organisation. Sie brauchen dafür das Recht **Organisation löschen**, das Inhaber haben, und die Installation muss das Löschen erlauben.

1. Alle API-Schlüssel der Organisation hören sofort auf zu funktionieren, und Kaeso informiert die Mitglieder per E-Mail.
2. Während einer Schonfrist von 7 Tagen (sofern die Installation keine andere festlegt) ist die Organisation schreibgeschützt, und jede Seite zeigt einen Hinweis. Ein Inhaber kann noch **Organisation wiederherstellen** wählen.
3. Nach der Schonfrist löscht Kaeso ihre Projekte, Schlüssel, Anbieter-Schlüssel, Einstellungen, Nutzung und Protokolle endgültig.

## Audit-Log lesen

Das **Audit-Log** der Organisation unter **Organisationseinstellungen** verzeichnet jede Änderung in der Organisation. Das **Audit-Log** eines Projekts in dessen Seitenleiste verzeichnet jede Änderung im Projekt.

Jeder Eintrag zeigt, wann es geschah, wer die Änderung vorgenommen hat, die Aktion und ihr Ziel. Filtern Sie nach Akteur, Aktion, Ziel-ID und Tagen. Wählen Sie einen Eintrag, um IP-Adresse, User-Agent und Anfrage-ID, geänderte Rechte und die Werte vorher und nachher zu sehen.

| Aktion | Recht |
| --- | --- |
| Das Audit-Log der Organisation lesen | **Audit-Log ansehen** |
| Es mit den aktuellen Filtern als JSON Lines exportieren | **Audit-Log exportieren** |
| Das Audit-Log eines Projekts lesen | **Projekt-Audit-Log ansehen** |

## Projekt erstellen

Wählen Sie unter **Projekte** **Projekt erstellen** und geben Sie einen Namen ein. Sie werden Admin des Projekts. Dafür brauchen Sie das Recht **Projekte erstellen**. Archivierte Projekte stehen unter **Alle**.

Das Standardprojekt ist das Projekt, dem jedes Mitglied angehört und in dem neue Mitglieder beginnen. Um den Standard zu verlegen, wählen Sie **Als Standard festlegen** im Menü eines anderen Projekts. Das bisherige Standardprojekt bleibt ein Projekt mit seinen Mitgliedern. Dafür brauchen Sie das Recht **Organisation bearbeiten**.

## Projekteinstellungen ändern

Die Seite **Einstellungen** eines Projekts enthält diese Einstellungen:

| Einstellung | Wirkung |
| --- | --- |
| Bild, Projektname | Erscheinen in der Seitenleiste und in den Anfrageprotokollen |
| Projekt-ID | Die ID, die die Platform API verwendet |
| Maximale Schlüssellaufzeit | **Vorgabe der Organisation verwenden** oder ein kürzeres Maximum für neue Schlüssel dieses Projekts |
| Inhaltsprotokollierung | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden. Lässt sich nicht einschalten, solange die Organisation sie ausgeschaltet hat |
| Projekt archivieren | Archiviert das Projekt mit seinen Schlüsseln und Dienstkonten endgültig. Seine Schlüssel hören sofort auf zu funktionieren |

Kaeso archiviert Projekte und löscht sie nie, damit ihre Nutzung und Protokolle erhalten bleiben. Das Standardprojekt lässt sich nicht archivieren: Machen Sie zuerst ein anderes Projekt zum Standard.

## Nächste Schritte

- [Mitglieder und Rollen](members-and-roles.md): Laden Sie Personen ein und geben Sie ihnen Rollen in der Organisation und ihren Projekten.
- [Anbieter-Schlüssel](provider-keys.md): Verbinden Sie die Schlüssel der Modellanbieter, mit denen Ihre Projekte Modelle aufrufen.
- [Anfrageprotokolle](request-logs.md): Legen Sie fest, was die Organisation aufbewahrt und wie lange.
