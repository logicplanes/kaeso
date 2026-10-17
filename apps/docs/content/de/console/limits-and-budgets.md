---
title: Limits und Budgets
description: Anfrage- und Token-Raten sowie Ausgaben für Projekte, Mitglieder und Schlüssel begrenzen.
order: 6
---

Limits halten die Nutzung in selbst gesetzten Grenzen, auf drei Ebenen: dem Projekt, einem Mitglied oder Dienstkonto und einem Schlüssel. Eine Anfrage muss jedes Limit bestehen, das für sie gilt, das ihres Schlüssels, ihres Mitglieds und ihres Projekts.

## Arten

| Art | Zählt | Zeitraum |
| --- | --- | --- |
| Ausgabenbudget | US-Dollar, zu den Preisen des Modells | Ein Tag, ein Monat oder insgesamt |
| Token-Budget | Token | Ein Tag, ein Monat oder insgesamt |
| Anfragerate | Anfragen | Eine gleitende Minute |
| Token-Rate | Token | Eine gleitende Minute |

Tage und Monate sind Kalendertage und -monate in UTC; „insgesamt“ wird nie zurückgesetzt. Ein Limit gilt für alle Modelle oder für eines.

Ein Limit kann nicht höher sein als dasselbe Limit der Ebene darüber: Das Ausgabenbudget eines Schlüssels bleibt in dem seines Mitglieds, das eines Mitglieds in dem des Projekts. Anfragen und Token sind ganze Zahlen bis 10^12; ein Ausgabenbudget beträgt höchstens 1.000.000.000 US-Dollar, mit bis zu neun Nachkommastellen.

## Harte und weiche Budgets

Raten lehnen Anfragen darüber immer ab. Ein Budget ist **hart**, solange **Danach Anfragen ablehnen** eingeschaltet ist, wie bei einem neuen Limit: Anfragen werden abgelehnt, bis der Zeitraum neu beginnt. Ein **weiches** Budget lässt Anfragen durch und warnt nur.

Ein Budget warnt bei bis zu 10 Prozentwerten seines Betrags, standardmäßig bei 50, 80 und 100 Prozent. Bei jedem geht eine E-Mail an die Inhaber und Admins der Organisation, die Admins des Projekts und, beim Budget eines Schlüssels, an dessen Inhaber, sofern sie **Budget-Warnungen** in ihrem Konto nicht abgeschaltet haben. Raten warnen nicht.

## Limit hinzufügen

Auf der Seite **Limits** eines Projekts **Limit hinzufügen** wählen:

1. **Für wen gilt es?** Das Projekt, ein Mitglied oder Dienstkonto, oder ein Schlüssel.
2. **Welches Limit?** Art, Betrag, Zeitraum und Modell und, bei einem Budget, ob es hart ist und wann es warnt.

Die Seite listet jedes Limit mit seinem Verbrauch; **Alle**, **Projekt**, **Mitglieder** und **Schlüssel** zeigen je eine Ebene. Die Limits eines Mitglieds, Dienstkontos oder Schlüssels stehen auch in dessen Sheet.

Beim Bearbeiten eines Limits ändern sich sein Betrag und, bei einem Budget, seine Warnungen und ob es hart ist; um Art, Zeitraum oder Modell zu ändern, das Limit entfernen und ein neues hinzufügen. Hat jemand anderes das Limit geändert, nachdem die Seite geladen wurde, meldet das Speichern dies und lädt das aktuelle Limit, damit sich die Änderung erneut vornehmen lässt. Limits zu sehen erfordert das Recht **Limits ansehen**, sie zu ändern **Limits verwalten**.

## Was eine App sieht

Eine abgelehnte Anfrage erhält den Status 429 im API-Format der App, mit einer Meldung, die das Limit nennt:

- über einer Rate sagt die Meldung, wann ein neuer Versuch möglich ist, und `Retry-After` nennt die Sekunden;
- bei einem aufgebrauchten Budget sagt die Meldung, wann es neu beginnt.

Haben noch laufende Anfragen ein Budget fast aufgebraucht, wird die nächste Anfrage mit `Retry-After: 1` abgelehnt, bis sie fertig sind. Ein hartes Budget ohne abgeschlossene Anfrage in seinem Zeitraum oder den 30 Tagen davor lässt nur eine Anfrage zugleich zu, bis die erste fertig ist, weil noch nicht bekannt ist, was eine Anfrage kostet.

> [!NOTE]
> Limits werden in Valkey gezählt. Solange Valkey nicht erreichbar ist, lassen Raten und weiche Budgets Anfragen durch, und harte Budgets lehnen sie mit Status 503 ab.
