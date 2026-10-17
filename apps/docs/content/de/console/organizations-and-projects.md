---
title: Organisationen und Projekte
description: Organisationen und Projekte erstellen und wechseln, ihre Nutzung ablesen und ihre Einstellungen verwalten.
order: 1
---

Eine **Organisation** umfasst Mitglieder, Anbieter-Schlüssel und Projekte. Ein **Projekt** umfasst einen Modellkatalog, API-Schlüssel, Dienstkonten, Limits und Anfrageprotokolle. Personen gehören einer Organisation mit einer Organisationsrolle an und einigen ihrer Projekte mit je einer Projektrolle ([Mitglieder und Rollen](members-and-roles.md)).

## Organisationen

**Organisation erstellen** im Menü am Fuß der Seitenleiste macht die eigene Person zum Inhaber der neuen Organisation und legt ein Standardprojekt an. Wer Organisationen erstellen darf, entscheidet die Installation ([Instanzeinstellungen](instance-administration.md#einstellungen)). Dasselbe Menü listet die eigenen Organisationen und wechselt zwischen ihnen.

### Übersicht

Die **Übersicht** der Organisation zeigt Anfragen, Token, Kosten und Fehler für einen Zeitraum von heute bis zur gesamten Laufzeit, die Kosten nach Projekt und eine Aufschlüsselung der Nutzung nach Projekt, Modell oder Mitglied. Die Nutzung erfordert das Recht **Gesamtnutzung ansehen**, die Kosten **Gesamtkosten ansehen**.

### Einstellungen

Die **Organisationseinstellungen** im Menü am Fuß der Seitenleiste haben diese Seiten:

| Seite | Inhalt |
| --- | --- |
| Allgemein | Bild, Name und Organisations-ID; **Inhaberschaft übertragen**; die Organisation löschen |
| Mitglieder | Mitglieder, Einladungen und Rollen ([Mitglieder und Rollen](members-and-roles.md)) |
| Sicherheit | Single Sign-on, verlangtes Single Sign-on und verlangte Zwei-Faktor-Authentifizierung, die maximale Schlüssellaufzeit |
| Plattform-Schlüssel | Schlüssel für Skripte, die die Organisation verwalten ([Plattform-Schlüssel](api-keys.md#plattform-schlüssel)) |
| Protokollierung | Inhaltsprotokollierung, Aufbewahrung, Speicher und das Löschen von Protokollen ([Anfrageprotokolle](request-logs.md#aufbewahrung)) |
| Audit-Log | Jede Änderung in der Organisation ([Audit-Log](#audit-log)) |

Bilder sind PNG-, JPEG- oder WebP-Dateien und werden quadratisch zugeschnitten.

### Single Sign-on

Unter **Sicherheit** meldet **Identitätsanbieter verbinden** Mitglieder über den eigenen OpenID-Connect-Anbieter an. Zuerst beim Anbieter eine App für Kaeso anlegen, dann fünf Schritte ausfüllen:

1. **Wo ist Ihr Identitätsanbieter?** Die Issuer-URL der App, etwa `https://login.example.com`.
2. **Welche App meldet Personen an?** Ihre Client-ID und ihr Client-Secret.
3. **Welche E-Mail-Domains melden sich darüber an?** Personen mit einer Adresse unter diesen Domains werden zum Anbieter geleitet. Internationalisierte Domains funktionieren in beiden Schreibweisen, `bücher.example` oder `xn--bcher-kva.example`.
4. **Beitritt ohne Einladung möglich?** **Nur mit Einladung** oder die Rolle, mit der neue Personen ohne Kaeso-Konto beitreten. Inhaber ist nicht möglich.
5. **Wie bietet die Anmeldeseite ihn an?** Der **Name auf der Schaltfläche**, etwa `Acme ID` für **Weiter mit Acme ID**; leer lautet sie **Weiter mit SSO**. **Ohne Klick starten** leitet Personen direkt zum Anbieter weiter, wenn die Anmeldeseite nur ihn anbietet, außer direkt nach dem Abmelden oder nachdem der Anbieter die Anmeldung abgelehnt hat.

Nach dem Verbinden zeigt die Seite:

- die **Weiterleitungs-URL**, die beim Anbieter als Callback der Anmeldung einzutragen ist;
- den **Anmeldelink**, der die Anmeldeseite mit der Schaltfläche dieses Anbieters öffnet, ohne dass eine Adresse einzugeben ist; er lässt sich mit den Mitgliedern teilen, etwa im Intranet.

**Single Sign-on bearbeiten** ändert die Verbindung; ein leeres Secret behält das gespeicherte. **Trennen** entfernt sie, und Mitglieder melden sich wieder mit Passwort oder Anmeldelink an.

Ein bestehendes Kaeso-Konto meldet sich über Single Sign-on nur in Organisationen an, denen es schon angehört: Es wird eingeladen und nimmt die Einladung nach einer Anmeldung auf anderem Weg an. [Anmeldung und Single Sign-on](../self-hosting/sign-in.md) enthält alle Regeln und beschreibt, wie eine Installation allein mit Single Sign-on läuft.

### Anforderungen

Ebenfalls unter **Sicherheit**:

| Einstellung | Wirkung |
| --- | --- |
| Single Sign-on verlangen | Mitglieder gelangen nur über den Identitätsanbieter in die Organisation; andere Sitzungen gelten weiter für ihre übrigen Organisationen. Setzt einen verbundenen Anbieter voraus |
| Zwei-Faktor-Authentifizierung verlangen | Wer sich mit Passwort anmeldet, braucht Zwei-Faktor-Authentifizierung. Anmeldelinks und Single Sign-on sind nicht betroffen |
| Maximale Schlüssellaufzeit | Die längste Laufzeit eines neuen Schlüssels in Tagen, oder **Kein Limit**. Bestehende Schlüssel behalten ihren Ablauf, und Projekte können ein kürzeres Limit setzen |

Wer selbst nicht über den Anbieter angemeldet ist oder noch keine Zwei-Faktor-Authentifizierung nutzt, sperrt sich mit der jeweiligen Anforderung selbst aus der Organisation aus, bis das nachgeholt ist; die Konsole weist vor dem Speichern darauf hin.

### Eine Organisation löschen

Erlaubt die Installation das Löschen, wählt jemand mit dem Recht **Organisation löschen**, standardmäßig ein Inhaber, unter **Allgemein** **Organisation löschen** und bestätigt mit dem Namen der Organisation.

- Alle API-Schlüssel der Organisation hören sofort auf zu funktionieren, und die Mitglieder werden per E-Mail informiert.
- Während der Schonfrist, 7 Tage, sofern die Installation nichts anderes festlegt, ist die Organisation schreibgeschützt, jede Seite zeigt einen Hinweis, und ein Inhaber kann sie mit **Organisation wiederherstellen** zurückholen.
- Danach werden Projekte, Schlüssel, Anbieter-Schlüssel, Einstellungen, Nutzung und Protokolle endgültig gelöscht.

### Audit-Log

Das **Audit-Log** der Organisation in ihren Einstellungen verzeichnet jede Änderung in der Organisation, das **Audit-Log** eines Projekts in dessen Seitenleiste jede Änderung im Projekt. Jeder Eintrag zeigt Zeitpunkt, Akteur, Aktion und Ziel; gefiltert wird nach Akteur, Aktion, Ziel-ID und Tagen. Ein Eintrag zeigt außerdem IP-Adresse, User-Agent und Anfrage-ID, geänderte Rechte und die Werte vorher und nachher.

| Aktion | Recht |
| --- | --- |
| Das Audit-Log der Organisation lesen | **Audit-Log ansehen** |
| Es mit den aktuellen Filtern als JSON Lines **exportieren** | **Audit-Log exportieren** |
| Das Audit-Log eines Projekts lesen | **Projekt-Audit-Log ansehen** |

## Projekte

**Projekte** listet die Projekte der Organisation, archivierte unter **Alle**. **Projekt erstellen** fragt nach einem Namen und macht die eigene Person zum Admin des Projekts; dafür ist das Recht **Projekte erstellen** nötig.

Das **Standardprojekt** ist das Projekt, dem jedes Mitglied angehört und in dem neue Mitglieder beginnen. **Als Standard festlegen** im Menü eines anderen Projekts verlegt es dorthin; das bisherige bleibt ein Projekt mit seinen Mitgliedern. Dafür ist das Recht **Organisation bearbeiten** nötig.

### Übersicht

Die **Übersicht** eines Projekts zeigt Anfragen, Token, Kosten und Fehler für einen Zeitraum, eine Aufschlüsselung nach Zeit, Modell, Schlüssel oder Mitglied, die Limits des Projekts mit ihrem Verbrauch und die letzten Fehler.

### Einstellungen

| Einstellung | Wirkung |
| --- | --- |
| Bild, Projektname | Erscheinen in der Seitenleiste und in den Anfrageprotokollen |
| Projekt-ID | Die ID, die die Platform API verwendet |
| Maximale Schlüssellaufzeit | **Vorgabe der Organisation verwenden** oder ein kürzeres Maximum für neue Schlüssel dieses Projekts |
| Inhaltsprotokollierung | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden; lässt sich nicht einschalten, solange die Organisation sie ausgeschaltet hat |
| Projekt archivieren | Archiviert das Projekt mit seinen Schlüsseln und Dienstkonten dauerhaft; seine Schlüssel hören sofort auf zu funktionieren |

Projekte werden archiviert, nie gelöscht, damit ihre Nutzung und Protokolle erhalten bleiben. Das Standardprojekt lässt sich nicht archivieren; vorher ein anderes Projekt zum Standard machen.
