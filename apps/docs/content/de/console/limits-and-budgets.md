---
title: Limits und Budgets
description: Anfrage- und Token-Raten sowie Ausgaben für Projekte, Mitglieder und Schlüssel begrenzen.
order: 6
---

Limits halten die Nutzung in festgelegten Grenzen. Sie gelten auf drei Ebenen, und eine Anfrage muss alle bestehen: die ihres Schlüssels, ihres Mitglieds und ihres Projekts.

## Arten

| Art | Zählt | Zeitraum |
| --- | --- | --- |
| Ausgabenbudget | US-Dollar, zu den Preisen des Modells | Ein Tag, ein Monat oder insgesamt |
| Token-Budget | Token | Ein Tag, ein Monat oder insgesamt |
| Anfragerate | Anfragen | Pro Minute, gleitend |
| Token-Rate | Token | Pro Minute, gleitend |

Tage und Monate sind Kalendertage und -monate in UTC. Ein Gesamtwert wird nie zurückgesetzt.

Ein Limit gilt für alle Modelle oder für ein Modell. Ein Limit kann nicht höher sein als dasselbe Limit auf der Ebene darüber: Das Ausgabenbudget eines Schlüssels bleibt im Rahmen des Budgets seines Mitglieds und das eines Mitglieds im Rahmen des Projektbudgets. Beträge sind positiv; Anfragen und Token sind ganze Zahlen.

## Hart und weich

Raten lehnen Anfragen darüber immer ab. Ein Budget ist **hart**, wenn **Danach Anfragen ablehnen** eingeschaltet ist, wie bei einem neuen Limit: Anfragen werden abgelehnt, bis der Zeitraum neu beginnt. Ein **weiches** Budget lässt Anfragen durch und warnt nur.

Ein Budget kann bei bis zu 10 Prozentwerten seines Betrags warnen, standardmäßig bei 50, 80 und 100 Prozent; Raten senden keine Warnungen. Die Inhaber und Admins der Organisation, die Admins des Projekts und, beim Budget eines Schlüssels, dessen Inhaber erhalten bei jedem davon eine E-Mail, sofern sie Budget-Warnungen in ihrem Konto nicht abgeschaltet haben.

## Limit hinzufügen

Auf der Seite **Limits** eines Projekts **Limit hinzufügen** wählen:

1. **Für wen gilt es?** Das Projekt, ein Mitglied oder Dienstkonto oder ein Schlüssel.
2. **Welches Limit?** Art, Betrag, Zeitraum, Modell, ob es hart ist, und die Warnungen.

Die Seite listet jedes Limit mit dem bisherigen Verbrauch, und **Alle**, **Projekt**, **Mitglieder** und **Schlüssel** zeigen jeweils eine Ebene. Limits eines Mitglieds, eines Dienstkontos oder eines Schlüssels erscheinen auch in deren Sheets. Beim Bearbeiten eines Limits ändern sich Betrag, Warnungen und ob es hart ist; um Art, Zeitraum oder Modell zu ändern, das Limit entfernen und ein neues hinzufügen. Limits zu sehen erfordert das Recht **Limits ansehen**, sie zu ändern **Limits verwalten**.

## Was eine App sieht

Eine abgelehnte Anfrage erhält Status 429 im eigenen Format der App, mit einer Meldung, die das Limit nennt:

- Über einer Rate: Die Meldung sagt, wann ein neuer Versuch möglich ist, und `Retry-After` nennt die Sekunden.
- Ein aufgebrauchtes Budget: Die Meldung sagt, wann es neu beginnt.

Wenn laufende Anfragen ein Budget fast aufgebraucht haben, wird die nächste Anfrage mit `Retry-After: 1` abgelehnt, bis sie fertig sind.

> [!NOTE]
> Limits werden in Valkey gezählt. Ist Valkey nicht erreichbar, lassen Raten und weiche Budgets Anfragen durch, und harte Budgets lehnen sie mit 503 ab, bis Valkey wieder da ist.
