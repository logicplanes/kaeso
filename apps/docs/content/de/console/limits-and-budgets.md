---
title: Limits und Budgets
description: Begrenzen Sie Anfragen, Token und Ausgaben eines Projekts, eines Mitglieds oder eines Schlüssels.
order: 6
---

Limits begrenzen die Anfragen, Token und Ausgaben eines Projekts, eines Mitglieds oder Dienstkontos oder eines Schlüssels. Eine Anfrage muss jedes Limit bestehen, das für sie gilt: das ihres Schlüssels, ihres Inhabers und ihres Projekts.

## Limit hinzufügen

1. Wählen Sie auf der Seite **Limits** eines Projekts **Limit hinzufügen**.
2. **Für wen gilt es?** Wählen Sie das Projekt, ein Mitglied oder Dienstkonto, oder einen Schlüssel.
3. **Welches Limit?** Wählen Sie Art, Betrag, Zeitraum und Modell. Wählen Sie bei einem Budget außerdem, ob es hart ist und wann es warnt.

Die Seite listet jedes Limit mit seinem Verbrauch. **Alle**, **Projekt**, **Mitglieder** und **Schlüssel** zeigen je eine Ebene. Die Limits eines Mitglieds, Dienstkontos oder Schlüssels stehen auch in dessen Sheet.

Limits zu sehen erfordert das Recht **Limits ansehen**, sie zu ändern **Limits verwalten**.

## Arten von Limits

| Art | Zählt | Zeitraum |
| --- | --- | --- |
| Ausgabenbudget | US-Dollar, zu den Preisen des Modells | Ein Tag, ein Monat oder insgesamt |
| Token-Budget | Token | Ein Tag, ein Monat oder insgesamt |
| Anfragerate | Anfragen | Eine gleitende Minute |
| Token-Rate | Token | Eine gleitende Minute |

Tage und Monate sind Kalendertage und -monate in UTC, und „insgesamt“ beginnt nie neu. Ein Limit gilt für alle Modelle oder für eines.

Ein Limit kann nicht höher sein als dasselbe Limit eine Ebene darüber. Das Ausgabenbudget eines Schlüssels bleibt in dem seines Inhabers, das eines Mitglieds in dem des Projekts.

## Harte und weiche Budgets

Eine Rate lehnt Anfragen darüber immer ab und warnt nie. Ein Budget ist hart, solange **Danach Anfragen ablehnen** eingeschaltet ist, wie bei einem neuen Limit. Ein hartes Budget lehnt Anfragen ab, bis sein Zeitraum neu beginnt. Ein weiches Budget lässt Anfragen durch und warnt nur.

Ein Budget warnt bei bis zu 10 Prozentwerten seines Betrags, standardmäßig bei 50, 80 und 100 Prozent. Bei jedem schickt Kaeso eine E-Mail an die Inhaber und Admins der Organisation und an die Admins des Projekts. Beim Budget eines Schlüssels geht sie auch an dessen Inhaber. Jede Person kann diese E-Mails unter **Budget-Warnungen** in ihrem Konto abschalten.

## Limit ändern

Beim Bearbeiten ändern Sie den Betrag eines Limits und, bei einem Budget, seine Warnungen und ob es hart ist. Um Art, Zeitraum oder Modell zu ändern, entfernen Sie das Limit und fügen Sie ein neues hinzu.

Hat jemand anderes das Limit geändert, nachdem die Seite geladen wurde, meldet das Speichern dies und lädt das aktuelle Limit. Nehmen Sie Ihre Änderung dann erneut vor.

## Was eine App sieht

Kaeso beantwortet eine abgelehnte Anfrage mit Status 429, im API-Format der App, mit einer Meldung, die das Limit nennt:

- Über einer Rate sagt die Meldung, wann ein neuer Versuch möglich ist, und der Header `Retry-After` nennt die Sekunden.
- Bei einem aufgebrauchten Budget sagt die Meldung, wann das Budget neu beginnt.

Solange noch laufende Anfragen ein Budget fast aufgebraucht haben, lehnt Kaeso die nächste Anfrage mit `Retry-After: 1` ab, bis sie fertig sind. Ein hartes Budget ohne abgeschlossene Anfrage in seinem Zeitraum oder den 30 Tagen davor lässt nur eine Anfrage zugleich zu, bis die erste fertig ist. Bis dahin weiß Kaeso nicht, was eine Anfrage kostet.

> [!NOTE]
> Kaeso zählt Limits in Valkey. Solange Valkey nicht erreichbar ist, lassen Raten und weiche Budgets Anfragen durch, und harte Budgets lehnen sie mit Status 503 ab.

## Nächste Schritte

- [API-Schlüssel](api-keys.md): Geben Sie jeder App einen eigenen Schlüssel, damit ihre Limits ihre eigenen sind.
- [Anfrageprotokolle](request-logs.md): Finden Sie die Anfragen, die ein Limit abgelehnt hat.
- [Fehler](../api/errors.md): Sehen Sie, wie eine abgelehnte Anfrage in jedem API-Format aussieht.
