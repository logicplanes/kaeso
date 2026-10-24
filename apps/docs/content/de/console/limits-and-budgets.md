---
title: Limits und Budgets
description: Begrenzen Sie Anfragen, Token und Ausgaben eines Projekts, eines Mitglieds oder eines Schlüssels.
order: 7
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

## Funktionen, die ein Ausgabenbudget nicht bepreisen kann

Kaeso bepreist eine Anfrage nach ihren Tokens. Manche Funktionen rechnen zusätzlich pro Aufruf oder zu einem höheren Satz ab, und das kann Kaeso nicht zählen. Solange ein Ausgabenbudget für eine Anfrage gilt, hart oder weich, lehnt Kaeso sie deshalb mit Status 403 ab, bevor sie den Anbieter erreicht, wenn sie Folgendes verwendet:

- ein Werkzeug, das der Anbieter ausführt, etwa `web_search` oder `code_execution` von Anthropic oder `web_search`, `file_search`, `image_generation`, `code_interpreter` oder `mcp` von OpenAI, auch wenn Kaeso `web_search` von OpenAI an Claude sendet;
- `mcp_servers`, `web_search_options`, `plugins` von OpenRouter oder `search_parameters` von xAI;
- einen `service_tier` außer `auto`, `default`, `flex`, `on_demand` oder `standard_only`, etwa `priority`;
- die `anthropic-beta` `context-1m-…`, die lange Prompts zu einem höheren Satz abrechnet;
- ein Modell ohne Eingabe- oder Ausgabepreis oder mit einem Preis von null, außer ein Admin hat diesen Preis am Modell von Hand gesetzt. Embeddings brauchen nur einen Eingabepreis.

Werkzeuge, die die App selbst ausführt, sind erlaubt: Funktionen und eigene Werkzeuge, `bash`, `text_editor`, `computer` und `memory` von Anthropic sowie `local_shell`, `apply_patch`, `computer_use_preview` und die `namespace`-Tools von OpenAI, die Funktionen gruppieren. Token-Budgets und Raten lehnen diese Funktionen nicht ab, und ohne Ausgabenbudget sind alle Funktionen erlaubt.

Um eine solche Funktion zu nutzen, rufen Sie sie mit einem Schlüssel auf, für den kein Ausgabenbudget gilt: weder das des Schlüssels noch das seines Inhabers noch das des Projekts. Die Fehlermeldung nennt die Funktion und die Ebene des Budgets, etwa „This key has a spend budget that can't price `web_search_20250305`; ask an admin.“

## Limit ändern

Beim Bearbeiten ändern Sie den Betrag eines Limits und, bei einem Budget, seine Warnungen und ob es hart ist. Um Art, Zeitraum oder Modell zu ändern, entfernen Sie das Limit und fügen Sie ein neues hinzu.

Hat jemand anderes das Limit geändert, nachdem die Seite geladen wurde, meldet das Speichern dies und lädt das aktuelle Limit. Nehmen Sie Ihre Änderung dann erneut vor.

## Was eine App sieht

Kaeso beantwortet eine abgelehnte Anfrage mit Status 429, im API-Format der App, mit einer Meldung, die das Limit nennt:

- Über einer Rate sagt die Meldung, wann ein neuer Versuch möglich ist, und der Header `Retry-After` nennt die Sekunden.
- Bei einem aufgebrauchten Budget sagt die Meldung, wann das Budget neu beginnt.

Solange noch laufende Anfragen ein hartes Budget oder eine Token-Rate aufbrauchen könnten, lehnt Kaeso die nächste Anfrage mit `Retry-After: 1` ab, bis sie fertig sind. Jede laufende Anfrage hält, was sie höchstens kosten kann, zu den Preisen des Modells. Das sind ihre Eingabe, aus ihrer Größe geschätzt, und ihr Ausgabelimit (`max_tokens` oder `max_output_tokens`, sonst das des Modells). Ein Stream hält es, solange er läuft.

Das Limit von SpaceX lässt das Reasoning aus, daher hält eine Anfrage an ein Reasoning-Modell von SpaceX das Ausgabelimit des Modells. Ein ChatGPT-Konto kann kein Ausgabelimit senden, daher hält auch eine Anfrage darüber das Ausgabelimit des Modells. Unter einem Budget hält eine Anfrage mindestens, was eine durchschnittliche Anfrage im Zeitraum oder in den 30 Tagen davor gekostet hat.

Eine Anfrage, die mehr kosten könnte, als von einem harten Budget oder einer Token-Rate übrig ist, lehnt Kaeso ab, auch wenn nichts anderes läuft. Die Meldung bittet um weniger Ausgabe-Token. Hat eine Token-Rate Platz für die Anfrage, sobald ältere Anfragen aus ihrer Minute fallen, nennt die Meldung stattdessen, wann ein neuer Versuch möglich ist. Um unter einem kleinen Budget mehr Anfragen gleichzeitig auszuführen, senden Sie ein niedrigeres Ausgabelimit.

Eine Anfrage ohne Ausgabelimit an ein Modell ohne Ausgabelimit und ohne Kontextfenster kann Kaeso nicht begrenzen. Sie kommt durch, wenn nichts anderes läuft, und kann ein Limit daher allein überschreiten. Unter einem harten Budget ohne abgeschlossene Anfrage in seinem Zeitraum oder den 30 Tagen davor läuft sie allein.

> [!NOTE]
> Kaeso zählt Limits in Valkey. Solange Valkey nicht erreichbar ist, lassen Raten und weiche Budgets Anfragen durch, und harte Budgets lehnen sie mit Status 503 ab.

## Nächste Schritte

- [API-Schlüssel](api-keys.md): Geben Sie jeder App einen eigenen Schlüssel, damit ihre Limits ihre eigenen sind.
- [Anfrageprotokolle](request-logs.md): Finden Sie die Anfragen, die ein Limit abgelehnt hat.
- [Fehler](../api/errors.md): Sehen Sie, wie eine abgelehnte Anfrage in jedem API-Format aussieht.
