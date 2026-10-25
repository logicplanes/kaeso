---
title: Konto und Sprache
description: Schützen Sie Ihr Konto, melden Sie andere Browser ab und wählen Sie Design und Sprache.
order: 10
---

Ihr Konto finden Sie unter **Konto** im Menü am Fuß der Seitenleiste. Es hat diese Seiten:

| Seite | Inhalt |
| --- | --- |
| Profil | Bild, Name, E-Mail-Adresse und Konto-ID, die Organisation, die das Konto verwaltet, sowie das Löschen des Kontos |
| Sicherheit | Passwort, Zwei-Faktor-Authentifizierung, Wiederherstellungscodes und Verknüpfungen mit Single Sign-on |
| Sitzungen | Die Browser, in denen Sie angemeldet sind |
| Agenten | Die Agenten, die Sie über den [MCP-Server](../api/mcp.md) verbunden haben, mit ihren Rechten und ihrer Aktivität |
| API-Schlüssel | Ihre eigenen Schlüssel in allen Projekten ([API-Schlüssel](api-keys.md)) |
| Mitgliedschaften | Ihre Organisationen und deren Projekte mit Ihren Rollen sowie das Verlassen einer Organisation |
| Benachrichtigungen | Welche E-Mails Sie erhalten |
| Darstellung | Design und Sprache |

## E-Mail-Adresse ändern

Wählen Sie unter **Profil** die Option **E-Mail ändern** und geben Sie die neue Adresse ein, bei eingeschalteter Zwei-Faktor-Authentifizierung auch einen Code aus Ihrer Authenticator-App oder einen Wiederherstellungscode. Kaeso schickt einen Bestätigungslink an die neue Adresse und einen Link zum Abbrechen der Änderung an die alte. Die Änderung gilt, sobald Sie den Bestätigungslink öffnen. Melden Sie den Browser ab, der die Änderung angefordert hat, melden Sie sich überall ab oder setzen Sie Ihr Passwort zurück, ist die Änderung ebenfalls abgebrochen.

Die Bestätigung meldet alle anderen Sitzungen Ihres Kontos ab. Der Browser, der den Link öffnet, bleibt angemeldet; öffnen Sie ihn dort, wo Sie nicht angemeldet sind, bleibt der Browser angemeldet, der die Änderung angefordert hat.

Verwaltet eine Organisation Ihr Konto, gehört Ihre Adresse ihrem Identitätsanbieter, und **Profil** zeigt kein **E-Mail ändern**. Ändert der Anbieter Ihre Adresse innerhalb der bestätigten Domains der Organisation, ändert Ihre nächste Anmeldung über ihn sie auch in Kaeso ([verwaltete Konten](../self-hosting/sign-in.md#verwaltete-konten)).

## Passwort ändern

Unter **Sicherheit** setzt **Passwort ändern** ein neues Passwort und meldet Ihre anderen Sitzungen ab. Melden Sie sich nur mit Links oder Single Sign-on an, haben Sie kein Passwort, und **Passwort festlegen** fügt eines hinzu.

## Zwei-Faktor-Authentifizierung einschalten

Die Zwei-Faktor-Authentifizierung fragt bei jeder Anmeldung mit Passwort und mit jedem Anmeldelink nach einem Code aus einer Authenticator-App. Single Sign-on fragt nicht danach; dort prüft Sie Ihr Identitätsanbieter. Eine Organisation, die Zwei-Faktor-Authentifizierung verlangt, akzeptiert eine Anmeldung mit Code. Sie akzeptiert auch eine Anmeldung über ihren eigenen Identitätsanbieter oder über den Anbieter der Organisation, die Ihr Konto verwaltet.

Ein Anmeldelink zeigt zuerst das Konto, bei dem er anmeldet. Wählen Sie **Anmelden** nur, wenn Sie den Link selbst angefordert haben. Ist dieser Browser bei einem anderen Konto angemeldet, ersetzt die Anmeldung diese Sitzung hier.

1. Wählen Sie unter **Sicherheit** die Option **Einrichten**.
2. Scannen Sie den QR-Code mit Ihrer Authenticator-App, oder geben Sie den Einrichtungsschlüssel ein.
3. Geben Sie den 6-stelligen Code aus der App ein und wählen Sie **Einschalten**.
4. Bewahren Sie die 10 Wiederherstellungscodes auf, die die Konsole einmal zeigt: Kopieren Sie sie oder wählen Sie **.txt herunterladen**. Jeder Code meldet Sie einmal an, statt eines Codes aus der App.

Das Einrichten verlangt eine Anmeldung innerhalb der letzten 15 Minuten. **Neu erzeugen** ersetzt die Wiederherstellungscodes, und **Ausschalten** entfernt die Zwei-Faktor-Authentifizierung; beide verlangen zuerst einen aktuellen Code oder einen Wiederherstellungscode. Haben Sie Authenticator und Wiederherstellungscodes verloren, kann ein Instanz-Admin die Zwei-Faktor-Authentifizierung für Sie zurücksetzen.

Eine Anmeldung nimmt bis zu drei Codes an; nach drei falschen beginnen Sie erneut mit Passwort oder Anmeldelink. Codes aus der App funktionieren nach 10 falschen Codes in Folge nicht mehr, ob bei der Anmeldung oder beim Bestätigen einer Änderung an Ihrem Konto. Kaeso schickt Ihnen dann eine E-Mail, nach je 10 weiteren erneut. Ein Wiederherstellungscode meldet Sie weiter an und entsperrt die App-Codes, ebenso ein Zurücksetzen des Passworts per E-Mail.

## Single Sign-on verknüpfen

Der Identitätsanbieter einer Organisation meldet Sie erst an, wenn Ihr Konto mit ihm verknüpft ist. Ein Konto, das die Single Sign-on der Organisation angelegt hat, ist von Anfang an verknüpft. **Single Sign-on** unter **Sicherheit** listet Ihre Organisationen mit Identitätsanbieter.

1. Melden Sie sich mit Ihrem Passwort oder einem Anmeldelink an.
2. Wählen Sie **Verknüpfen** neben der Organisation.
3. Melden Sie sich beim Identitätsanbieter an. Sie kommen zurück zu **Sicherheit** und sind angemeldet wie zuvor.

**Trennen** hindert den Anbieter daran, Sie anzumelden, und beendet die Sitzungen, in denen er Sie angemeldet hat; wie andere Sicherheitsänderungen braucht es eine kürzliche Anmeldung. Das Audit-Log der Organisation verzeichnet beides, als `user.sso_link` und `user.sso_unlink`.

## Andere Browser abmelden

**Sitzungen** listet jeden Browser, in dem Sie angemeldet sind, und wie Sie sich angemeldet haben. **Beenden** meldet einen davon ab, und **Überall abmelden** meldet alle außer diesem ab.

Eine Sitzung endet nach 7 Tagen ohne Nutzung, spätestens nach 30 Tagen.

## Agenten verwalten

**Agenten** listet jeden Agenten, den Sie autorisiert haben: sein Bild, seinen Namen und seine Harness, eine Zusammenfassung seiner Rechte, die Organisationen, die er erreicht, und wann er erstellt, zuletzt genutzt wurde und abläuft. Wählen Sie einen Agenten, um seine Seite zu öffnen. Dort können Sie:

- ihn umbenennen oder seine Harness ändern;
- seine Rechte im selben Editor wie auf der Zustimmungsseite ändern und dann **Rechte speichern** wählen;
- ihm mit **Neue Laufzeit ab heute** einen neuen Ablauf geben;
- ihn **Widerrufen**, was ihn sofort abmeldet;
- seine **Aktivität** lesen: jeden Tool-Aufruf der letzten 90 Tage, mit Tool, Ziel, Ergebnis, Zeit und IP-Adresse.

Ein Agent hört auf zu funktionieren, wenn er abläuft, nach 90 Tagen ohne Nutzung oder wenn Sie ihn widerrufen. Braucht er ein Recht, das ihm fehlt, gibt er Ihnen einen Link zu seiner Seite hier, mit diesem Recht zur Freigabe bereit ([fehlende Rechte](../api/mcp.md#fehlende-rechte)).

## Sensible Aktion bestätigen

Manche Aktionen verlangen eine Anmeldung innerhalb der letzten 15 Minuten. Ist Ihre älter, bittet die Konsole Sie, sich erneut anzumelden, bevor sie die Aktion ausführt. Eine Anmeldung über Single Sign-on zählt ab dem Zeitpunkt, zu dem Ihr Identitätsanbieter Sie zuletzt geprüft hat, und der kann lange vor der Rückkehr zu Kaeso liegen.

- In Ihrem Konto: die E-Mail-Adresse ändern, ein Passwort festlegen, wenn Sie keines haben, und das Konto löschen. Ebenso die Zwei-Faktor-Authentifizierung einrichten oder ausschalten, Wiederherstellungscodes neu erzeugen sowie **Verknüpfen** und **Trennen** unter **Single Sign-on**.
- In einer Organisation: die Aktionen, deren Recht laut [Rechte](../concepts/rights.md) eine kürzliche Anmeldung erfordert, etwa Anbieter-Schlüssel hinzufügen, Plattform-Schlüssel erstellen oder Protokolle löschen.
- Für Ihre Agenten: einem davon ein mit ⚠ gekennzeichnetes, sensibles Recht geben ([Agenten](../concepts/rights.md#sensible-rechte)).
- Als Instanz-Admin: jemanden zum Instanz-Admin machen oder das zurücknehmen, einen Benutzer löschen, die Zwei-Faktor-Authentifizierung eines Benutzers zurücksetzen und einen Inhaber einer Organisation einsetzen. Ebenso eine Organisation zur Löschung vormerken, **Jetzt löschen**, **Auf … aktualisieren**, **Automatisch** für Updates wählen und die Kontoerstellung, die Organisationserstellung oder die erlaubten Domains ändern. **Single Sign-on für Instanz-Admins** verlangt eine Anmeldung mit Passwort oder Anmeldelink.

Die Konsole fragt nach Ihrem Passwort oder sendet einen Anmeldelink oder einen Link, um ein Passwort festzulegen. Nur wenn Ihr Konto nichts davon hat, etwa weil die Organisation, die es verwaltet, Single Sign-on verlangt, bestätigt sie über Ihren Identitätsanbieter.

Über Single Sign-on öffnet die Konsole Ihren Identitätsanbieter in einem kleinen Fenster und bittet Sie, sich dort erneut anzumelden. Die Seite und Ihre Eingaben bleiben, wie sie sind, und die Aktion läuft weiter, sobald Sie sich angemeldet haben. Nur Ihre eigene verknüpfte Identität bestätigt, aus einer frischen Anmeldung, und eine Ablehnung nennt den Grund.

Blockiert der Browser das Fenster, geht die Konsole in diesem Tab zum Anbieter und bringt Sie zur Seite zurück. Eine Bestätigung über den Anbieter einer Organisation macht die Sitzung zu einer aus dem Single Sign-on dieser Organisation.

Einen Anbieter, der einmal ohne frische Anmeldung geantwortet hat, bietet sie nicht mehr an ([kürzliche Anmeldungen](../self-hosting/sign-in.md#kürzliche-anmeldungen)).

## Nach einer Anmeldung über Single Sign-on

Gehört Ihre Adresse zu einer bestätigten E-Mail-Domain einer Organisation mit Single Sign-on, verwaltet diese Organisation Ihr Konto, und **Profil** zeigt **Verwaltet von** mit ihrem Namen. Eine Anmeldung über ihren Identitätsanbieter ist eine vollständige Anmeldung: Sie öffnet jede Organisation, der Sie angehören, jede mit ihren eigenen Anforderungen. Außerdem können Sie damit Ihr Konto ändern, wofür weiterhin eine kürzliche Anmeldung nötig ist. Die Verwaltung der Instanz braucht eine weitere Anmeldung, es sei denn, ein Instanz-Admin hat die Organisation dafür gewählt.

Eine Anmeldung über den Anbieter einer Organisation, die Ihr Konto nicht verwaltet, öffnet nur diese Organisation und lässt Sie Ihr Konto lesen. Das geschieht, wenn Ihre Adresse zu einer anderen Domain gehört. **API-Schlüssel** zeigt dann nur Ihre Schlüssel in dieser Organisation. Alles andere braucht eine weitere Anmeldung, und die Konsole fragt nach Ihrem Passwort oder schickt einen Anmeldelink:

- E-Mail-Adresse, Passwort oder Zwei-Faktor-Authentifizierung ändern;
- eine Organisation erstellen, eine andere verlassen oder das Konto löschen;
- eine andere Organisation öffnen oder die Instanz verwalten.

Verlangt die Organisation, die Ihr Konto verwaltet, Single Sign-on, melden Sie sich nur über ihren Anbieter an, und er bestätigt auch kürzliche Anmeldungen ([verwaltete Konten](../self-hosting/sign-in.md#verwaltete-konten)).

Ohne Passwort wählen Sie **Link per E-Mail senden** unter **Konto** › **Sicherheit** oder **Passwort vergessen?** auf der Anmeldeseite und legen eines über die E-Mail fest. Denselben Link bietet die Konsole an, wenn eine Aktion eine weitere Anmeldung verlangt und nichts anderes funktioniert.

## E-Mails wählen

Unter **Benachrichtigungen** schalten **Budget-Warnungen** und **Ablaufende Schlüssel** diese E-Mails ein oder aus. Sicherheits-E-Mails, etwa zu einem geänderten Passwort, schickt Kaeso immer.

## Design und Sprache wählen

Unter **Darstellung**:

- **Design** ist **System**, **Hell** oder **Dunkel**. Dieser Browser speichert es.
- **Sprache** ist automatisch, English, Deutsch oder 简体中文. Automatisch folgt der Sprache Ihres Browsers, oder Englisch, wenn die Konsole sie nicht hat. Kaeso speichert die Wahl in Ihrem Konto, sodass sie Ihnen in andere Browser folgt, und auch E-Mails von Kaeso verwenden sie.

Produktnamen und Modell-IDs bleiben in jeder Sprache, wie sie sind.

## Organisation verlassen oder Konto löschen

**Organisation verlassen** unter **Mitgliedschaften** nimmt Sie aus der Organisation und ihren Projekten und archiviert Ihre Schlüssel dort. Ihr einziger Inhaber kann sie erst verlassen, wenn jemand anderes ebenfalls Inhaber ist.

**Konto löschen** unter **Profil** nimmt Sie aus jeder Organisation und archiviert alle Ihre Schlüssel. Der Verlauf zeigt Sie als gelöschten Benutzer. Bestätigen Sie mit Ihrer E-Mail-Adresse.

Beides geht nicht, solange Sie der letzte Inhaber einer Organisation sind. Machen Sie zuerst jemand anderen zum Inhaber.

## Nächste Schritte

- [Organisationen und Projekte](organizations-and-projects.md): Verlangen Sie Zwei-Faktor-Authentifizierung oder Single Sign-on für eine Organisation.
- [API-Schlüssel](api-keys.md): Erstellen Sie Schlüssel für Ihre Apps und Skripte.
- [MCP-Server](../api/mcp.md): Verbinden Sie einen Coding-Agenten und legen Sie fest, was er darf.
- [Anmeldung und Single Sign-on](../self-hosting/sign-in.md): Sehen Sie, wie die Anmeldeseite funktioniert und wie Sie wieder hineinkommen.
