---
title: Organisationen und Projekte
description: Erstellen Sie Organisationen und Projekte, verbinden Sie Single Sign-on und verwalten Sie ihre Einstellungen.
order: 1
---

Eine Organisation umfasst Mitglieder, Anbieter-Schlüssel und Projekte. Ein Projekt umfasst einen Modellkatalog, API-Schlüssel, Dienstkonten, Limits und Anfrageprotokolle. Personen gehören einer Organisation mit einer Organisationsrolle an und jedem ihrer Projekte mit einer Projektrolle.

## Organisation erstellen

Öffnen Sie das Menü am Fuß der Seitenleiste und wählen Sie **Organisation erstellen**. Sie werden Inhaber der neuen Organisation, und sie beginnt ohne Projekte. Wer Organisationen erstellen darf, legt die Installation fest ([Instanzeinstellungen](instance-administration.md#installationseinstellungen-ändern)).

Dasselbe Menü listet Ihre Organisationen und wechselt zwischen ihnen. Wer Kaeso oder eine andere Organisation öffnet, sieht die **Übersicht** der Organisation; ein Projekt öffnen Sie dort oder in der Seitenleiste.

## Nutzung ablesen

Die **Übersicht** der Organisation ist ihre Startseite. Sie beginnt mit dem Release, das Kaeso ausführt, und zeigt dann Anfragen, Token, Cache-Treffer und fehlgeschlagene Anfragen der letzten 7, 14 oder 30 Tage, mit dem Recht auf Kosten auch die Ausgaben des Monats nach Projekt. Darunter stehen die Projekte mit ihren heutigen Anfragen, die Mitglieder, Anbieterschlüssel, MCP-Server, die letzten Änderungen und die neuesten Modelle im Katalog. Jede Summe öffnet **Analysen** bei dieser Kennzahl.

**Analysen** zeigt Anfragen, Token, Kosten, Fehler und Tool-Aufrufe für einen Zeitraum Ihrer Wahl, von heute bis zur gesamten Laufzeit, und vergleicht jede Summe mit dem Zeitraum davor, außer für heute, das noch läuft, und die gesamte Laufzeit. Die Summen stehen oben, und die gewählte wird gegen den Zeitraum davor gezeichnet, der als gestrichelte Linie erscheint: nach Stunden für ein bis zwei Tage, wenn Anfrageprotokolle an sind, nach UTC-Tagen bis zu einem Quartal, nach Wochen bis zu zwei Jahren und darüber nach Monaten. Unter dem Diagramm stehen die fünf Projekte und Modelle mit dem größten Anteil an dieser Summe, jeweils mit Anteil und Änderung; **Zuverlässigkeit** mit den fehlgeschlagenen Anfragen, ihren Arten und den langsamsten Stunden; **Effizienz** mit den Cache-Treffern und den Token pro Anfrage; und **Ausgaben** mit der Prognose für den Monat und den Kosten nach Projekt. Die Fehlerarten und die langsamsten Stunden stammen aus den Anfrageprotokollen, für Zeiträume bis 62 Tage. Einen Tag, eine Woche oder einen Monat, der noch läuft, zeichnet sie gestrichelt oder heller, und sein Tooltip sagt „bisher“. **Projekte**, **Modelle** und **Mitglieder** schlüsseln den Zeitraum zeilenweise auf, mit Anteil, Wachstum und Rückgang, Fehlerquote, Token, Cache-Treffern und Kosten. Für die Nutzung brauchen Sie das Recht **Gesamtnutzung ansehen**, für die Kosten **Gesamtkosten ansehen**.

Eine Aufschlüsselung mit Medienanfragen hat eine Spalte **Medien**: die erzeugten Bilder, die gesprochenen Zeichen, die Länge des erzeugten oder transkribierten Audios und die erzeugten Videos mit ihrer Länge. Eine Gruppe, deren Anfragen keinen bekannten Preis hatten, zeigt ihre Kosten als **Unbekannt**; eine, bei der nur einige keinen hatten, zeigt ihre Kosten mit der Zahl der unbekannten Anfragen.

Die **Übersicht** eines Projekts zeigt seine Aktivität der letzten Tage, seine neuesten Anfragen, sobald sie eintreffen, seine Limits mit ihrem Verbrauch, seine Modelle nach Nutzung, seine Schlüssel nach letzter Verwendung, seine Mitglieder und die letzten Änderungen. Seine **Analysen** zeigen dasselbe wie die der Organisation für ein Projekt, mit **Modelle**, **Schlüssel** und **Mitglieder** dem Ausgabenbudget des Projekts neben der Prognose und einer Aufschlüsselung nach Zeit, Modell, Schlüssel, Mitglied oder Route am Fuß der Seite. Eine Zeile einer Liste öffnet die Anfrageprotokolle des Zeitraums mit diesem Modell, diesem Schlüssel oder dieser Fehlerart.

Eine Route ist das Format der App, das Format, in dem Kaeso den Anbieter aufrief, und der Anbieter. Ihre Aufschlüsselung hat eine Spalte **Cache-Treffer**: den Anteil der Eingabe, der aus dem Prompt-Cache gelesen wurde. Dasselbe Modell kann über eine Route den Cache treffen und über eine andere nie, daher zeigt **Route**, wo das Caching versagt. Anfragen aus der Zeit, bevor Kaeso Routen erfasste, erscheinen als **Route nicht erfasst**.

## Organisationseinstellungen ändern

Öffnen Sie **Organisationseinstellungen** im Menü am Fuß der Seitenleiste. Dort finden Sie diese Seiten:

| Seite | Inhalt |
| --- | --- |
| Allgemein | Bild, Name und Organisations-ID, **Inhaberschaft übertragen** und das Löschen der Organisation |
| Mitglieder | Mitglieder, Einladungen und Rollen ([Mitglieder und Rollen](members-and-roles.md)) |
| Sicherheit | Single Sign-on, die Anforderungen an die Anmeldung, die maximale Schlüssellaufzeit und die Agenten der Mitglieder |
| Plattform-Schlüssel | Schlüssel für Skripte, die die Organisation verwalten ([Plattform-Schlüssel](api-keys.md#plattform-schlüssel)) |
| Protokollierung | Inhaltsprotokollierung, Aufbewahrung, Speicher und das Löschen von Protokollen ([Anfrageprotokolle](request-logs.md#aufbewahrung)) |
| Audit-Log | Jede Änderung in der Organisation ([Audit-Log](#audit-log-lesen)) |

Ein Bild ist eine PNG-, JPEG- oder WebP-Datei, die die Konsole quadratisch zuschneidet.

## Single Sign-on

Mit Single Sign-on melden sich Mitglieder über Ihren OpenID-Connect-Identitätsanbieter an. Legen Sie zuerst beim Anbieter eine App für Kaeso an. Wählen Sie dann unter **Sicherheit** **Identitätsanbieter verbinden** und beantworten Sie fünf Schritte:

1. **Wo ist Ihr Identitätsanbieter?** Der Schritt zeigt die **Weiterleitungs-URL**, die Sie in der App als Callback der Anmeldung eintragen. Geben Sie die Issuer-URL der App ein, etwa `https://login.example.com`.
2. **Welche App meldet Personen an?** Geben Sie ihre Client-ID und ihr Client-Secret ein.
3. **Welche E-Mail-Domains melden sich darüber an?** Die Anmeldeseite leitet Personen mit einer Adresse unter diesen Domains zu Ihrem Anbieter. Internationalisierte Domains funktionieren in beiden Schreibweisen, `bücher.example` oder `xn--bcher-kva.example`.
4. **Beitritt ohne Einladung möglich?** Wählen Sie **Nur mit Einladung** oder die Rolle, mit der Personen ohne Kaeso-Konto beitreten. Diese Rolle kann nicht Inhaber sein.
5. **Wie bietet die Anmeldeseite ihn an?** Tragen Sie unter **Name auf der Schaltfläche** einen Namen ein, etwa `Acme ID` für „Weiter mit Acme ID“. Bleibt das Feld leer, steht auf dem Button „Weiter mit SSO“. Mit **Ohne Klick starten** geht die Anmeldeseite zu Ihrem Anbieter weiter, ohne seinen Button zu zeigen. Das geschieht, wenn jemand eine Adresse an Ihren bestätigten Domains eingibt und der Anbieter sein einziger Weg hinein ist. **Bestätigte Adressen verlangen** weist Tokens ab, die die Adresse nicht als bestätigt kennzeichnen; schalten Sie es nur für einen Anbieter ab, der `email_verified` nie sendet.

Nach dem Verbinden zeigt die Seite die **Weiterleitungs-URL**: Tragen Sie sie beim Anbieter als Callback der Anmeldung ein. Um Mitglieder aus dem Intranet oder über ein Lesezeichen zu Single Sign-on zu schicken, verlinken Sie die Anmeldeseite Ihrer Installation, etwa `https://kaeso.example.com/sign-in`. Sie fragt zuerst nach der Adresse ([die Anmeldeseite](../self-hosting/sign-in.md#die-anmeldeseite)).

### E-Mail-Domains bestätigen

Single Sign-on meldet nur Adressen bestätigter Domains an und lässt nur sie beitreten, damit keine Organisation eine Domain beansprucht, die ihr nicht gehört. Für jede Domain zeigt **Sicherheit**, ob sie **Bestätigt** oder **Nicht bestätigt** ist, und den TXT-Eintrag, den eine nicht bestätigte braucht. Legen Sie ihn bei Ihrem DNS-Anbieter an und wählen Sie dann **Bestätigen** neben der Domain:

```text
_kaeso-verify.acme.example  TXT  "kaeso-verify=Qm9ZtX3c8L1aV7pK2rN5wE0yH4uJ6sD9fG1bC3xT8eA"
```

DNS-Änderungen brauchen etwas, bis sie sichtbar sind; wählen Sie **Bestätigen** später erneut. Domains öffentlicher Mail-Anbieter wie `gmail.com` lassen sich nicht hinzufügen, ebenso wenig eine Domain, die eine andere Organisation bestätigt hat. Eine Domain, für die das öffentliche DNS nicht antwortet, bestätigt ein Operator auf dem Server mit `kaeso sso verify-domain acme.example`.

**Single Sign-on bearbeiten** ändert die Verbindung. Ein leeres Client-Secret behält das gespeicherte, solange Issuer und Client-ID gleich bleiben. Ein neuer Issuer oder eine neue Client-ID beendet die Sitzungen, die der bisherige Anbieter angemeldet hat. **Trennen** entfernt sie und beendet die Sitzungen, die sie angemeldet hat, und Mitglieder melden sich wieder mit Passwort oder Anmeldelink an.

Single Sign-on meldet ein bestehendes Kaeso-Konto erst an, wenn seine Inhaberin oder sein Inhaber es unter **Konto** mit Ihrem Anbieter verknüpft ([Single Sign-on verknüpfen](account.md#single-sign-on-verknüpfen)). Das Konto muss außerdem der Organisation angehören: Laden Sie die Person ein, und sie nimmt die Einladung nach einer Anmeldung auf anderem Weg an. Single Sign-on zu ändern verlangt eine Konsolensitzung mit kürzlicher Anmeldung; Plattform-Schlüssel können es nicht. [Anmeldung und Single Sign-on](../self-hosting/sign-in.md) enthält alle Regeln und beschreibt, wie eine Installation allein mit Single Sign-on läuft.

Mitglieder bestätigen eine kürzliche Anmeldung mit Passwort oder Anmeldelink. Single Sign-on bestätigt nur für Konten ohne anderen Weg, etwa die von der Organisation verwalteten, solange **Single Sign-on verlangen** eingeschaltet ist. Das gelingt nur, wenn Ihr Anbieter auf Anfrage frisch anmeldet und in seinen ID-Tokens `auth_time` sendet.

Kommt eine Bestätigung ohne frische Anmeldung zurück, zeigt **Sicherheit** die Zeile **Kürzliche Anmeldung**, und der Anbieter wird zum Bestätigen nicht mehr angeboten. Ist **Single Sign-on verlangen** eingeschaltet, können die verwalteten Konten dann gar nicht bestätigen ([kürzliche Anmeldungen](../self-hosting/sign-in.md#kürzliche-anmeldungen)).

Sobald die Verbindung steht, verwaltet die Organisation die Konten an ihren bestätigten Domains: Eine Anmeldung über ihren Anbieter öffnet alles, was diese Konten öffnen können, außer der Verwaltung der Instanz ([verwaltete Konten](../self-hosting/sign-in.md#verwaltete-konten)).

## Single Sign-on oder Zwei-Faktor-Authentifizierung verlangen

Unter **Sicherheit** legen Sie fest, wie Mitglieder in die Organisation gelangen:

| Einstellung | Wirkung |
| --- | --- |
| Single Sign-on verlangen | Konten an ihren bestätigten Domains melden sich nur per SSO an; Gäste behalten ihre eigene Anmeldung. Die Konten, die sie verwaltet, können sich nirgends auf der Installation mit Passwort oder Anmeldelink anmelden, und ihre anderen Sitzungen enden. Setzt einen verbundenen Anbieter voraus |
| Zwei-Faktor-Authentifizierung verlangen | Mitglieder brauchen eine Anmeldung mit einem Code aus ihrer Authenticator-App, per Passwort oder Anmeldelink. Eine Anmeldung über Single Sign-on reicht ebenfalls, über den eigenen Anbieter der Organisation oder den der Organisation, die das Konto des Mitglieds verwaltet |
| Maximale Schlüssellaufzeit | Die längste Laufzeit eines neuen Schlüssels in Tagen, oder **Kein Limit**. Bestehende Schlüssel behalten ihren Ablauf, und ein Projekt kann ein kürzeres Limit setzen |

Wenn Sie eine Anforderung einschalten, die Sie selbst nicht erfüllen, ist die Organisation auch für Sie gesperrt. Das gilt, bis Sie sich über den Anbieter anmelden oder die Zwei-Faktor-Authentifizierung einrichten. Die Konsole fragt vor dem Speichern nach. Wenn Sie Single Sign-on verlangen, bleibt Ihre eigene Sitzung bestehen; ein [Anmeldelink vom Server](../self-hosting/sign-in.md#wieder-hineinkommen) ist also nie Ihr einziger Weg zurück. Wenn Sie den Identitätsanbieter trennen, schaltet das auch **Single Sign-on verlangen** aus.

## Agenten von Mitgliedern zulassen

Mitglieder können Agenten wie Claude Code oder Codex verbinden, die über den [MCP-Server](../api/mcp.md) für sie handeln. Unter **Sicherheit** enthält der Abschnitt **Agenten** die Agenten-Richtlinie der Organisation. Sie entscheidet, ob diese Agenten in der Organisation handeln dürfen und wie weit:

| Einstellung | Was sie bewirkt |
| --- | --- |
| Agenten von Mitgliedern zulassen | Standardmäßig an. Ausgeschaltet erhält jeder Agent in dieser Organisation bei seiner nächsten Anfrage 403 und arbeitet anderswo weiter |
| Höchstens erlaubt | **Observer**, **Builder**, **Operator** oder **Vollzugriff**, jeweils einschließlich der Vorlagen davor. **Vollzugriff** ist der Standard. Ein Agent erhält hier nie mehr, was auch immer sein Mitglied ihm gegeben hat |

Die Richtlinie zu ändern, erfordert **Rollen zuweisen** und eine Konsolensitzung; kein Agent kann sie ändern. [Agenten](../concepts/rights.md#vorlagen) listet, was jede Vorlage enthält.

**Agenten mit Zugang** darunter listet die Agenten, die in der Organisation handeln dürfen, mit ihrem Mitglied, ihrer Modellfamilie und wann sie erstellt und zuletzt genutzt wurden. Sie zu sehen, erfordert **Mitglieder ansehen**. **Zugang schließen** schließt die Organisation für einen Agenten und erfordert **Rollen zuweisen**. Der Agent arbeitet in den anderen Organisationen seines Mitglieds weiter, und sein Mitglied kann ihm diese wieder öffnen.

## Organisation löschen

Wählen Sie unter **Allgemein** **Organisation löschen** und bestätigen Sie mit dem Namen der Organisation. Sie brauchen dafür das Recht **Organisation löschen**, das Inhaber haben, und die Installation muss das Löschen erlauben.

1. Alle API-Schlüssel der Organisation hören sofort auf zu funktionieren, und Kaeso informiert die Mitglieder per E-Mail.
2. Während einer Schonfrist von 7 Tagen (sofern die Installation keine andere festlegt) ist die Organisation schreibgeschützt, und jede Seite zeigt einen Hinweis. Ein Inhaber kann noch **Organisation wiederherstellen** wählen.
3. Nach der Schonfrist löscht Kaeso ihre Projekte, Schlüssel, Anbieter-Schlüssel, Einstellungen, Nutzung und Protokolle endgültig. Ist die Schonfrist abgelaufen, lässt sich die Organisation nicht mehr wiederherstellen, auch bevor Kaeso sie ganz gelöscht hat.

## Audit-Log lesen

Das **Audit-Log** der Organisation unter **Organisationseinstellungen** verzeichnet jede Änderung in der Organisation. Das **Audit-Log** eines Projekts in dessen Seitenleiste verzeichnet jede Änderung im Projekt.

Jeder Eintrag zeigt, wann es geschah, wer die Änderung vorgenommen hat, die Aktion und ihr Ziel. Filtern Sie nach Akteur, Aktion, Ziel-ID und Tagen. Wählen Sie einen Eintrag, um IP-Adresse, User-Agent und Anfrage-ID, geänderte Rechte und die Werte vorher und nachher zu sehen.

Die Änderung eines Agenten zeigt das Bild des Mitglieds, für das er handelt, mit einem kleinen Bot-Abzeichen unten rechts. Fahren Sie darüber, um Mitglied, Agent und Modellfamilie zu sehen. der Filter **Agenten** zeigt nur die Änderungen von Agenten, und die Platform API nennt den Agenten in `actor.agent` ([Aktivität und Audit](../api/mcp.md#aktivität-und-audit)).

| Aktion | Recht |
| --- | --- |
| Das Audit-Log der Organisation lesen | **Audit-Log ansehen** |
| Es mit den aktuellen Filtern als JSON Lines exportieren | **Audit-Log exportieren** |
| Das Audit-Log eines Projekts lesen | **Projekt-Audit-Log ansehen** |

## Projekt erstellen

Wählen Sie unter **Projekte** **Projekt erstellen** und geben Sie einen Namen ein. Sie werden Admin des Projekts. Dafür brauchen Sie das Recht **Projekte erstellen**.

**Archiviert** listet archivierte Projekte, **Alle** jedes Projekt. Die Suche findet ein Projekt nach seinem Namen, und wer die Nutzung der Organisation sehen darf, sieht bei jedem Projekt seine Anfragen der letzten 30 Tage.

Alle Projekte einer Organisation sind gleich. Wer der Organisation beitritt, kommt dadurch in kein Projekt: Eine Einladung nennt die Projekte, denen jemand beitritt, und alle anderen fügen Sie auf der Seite **Mitglieder** eines Projekts hinzu.

## Projekteinstellungen ändern

Die Seite **Einstellungen** eines Projekts enthält diese Einstellungen:

| Einstellung | Wirkung |
| --- | --- |
| Bild, Projektname | Erscheinen in der Seitenleiste und in den Anfrageprotokollen |
| Projekt-ID | Die ID, die die Platform API verwendet |
| Maximale Schlüssellaufzeit | **Vorgabe der Organisation verwenden** oder ein kürzeres Maximum für neue Schlüssel dieses Projekts |
| Inhaltsprotokollierung | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden. Lässt sich nicht einschalten, solange die Organisation sie ausgeschaltet hat |
| Projekt archivieren | Archiviert das Projekt mit seinen Schlüsseln und Dienstkonten endgültig. Seine Schlüssel hören sofort auf zu funktionieren |

Kaeso archiviert Projekte und löscht sie nie, damit ihre Nutzung und Protokolle erhalten bleiben. Jedes Projekt lässt sich archivieren.

## Nächste Schritte

- [Mitglieder und Rollen](members-and-roles.md): Laden Sie Personen ein und geben Sie ihnen Rollen in der Organisation und ihren Projekten.
- [Anbieter-Schlüssel](provider-keys.md): Verbinden Sie die Schlüssel der Modellanbieter, mit denen Ihre Projekte Modelle aufrufen.
- [Anfrageprotokolle](request-logs.md): Legen Sie fest, was die Organisation aufbewahrt und wie lange.
