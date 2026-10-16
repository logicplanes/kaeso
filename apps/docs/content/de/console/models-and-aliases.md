---
title: Modelle und Aliasse
description: Die Modelle wählen, die ein Projekt aufrufen darf, ihre Werte anpassen und ihnen Aliasse geben.
order: 4
---

Der **Katalog** eines Projekts listet die Modelle, die seine Schlüssel aufrufen dürfen. Eine Anfrage für ein Modell außerhalb des Katalogs wird abgelehnt. Der Katalog liegt auf der Seite **Modelle** des Projekts.

## Modelle hinzufügen

**Modelle hinzufügen** wählen, OpenAI oder Anthropic wählen, suchen und ein oder mehrere Modelle auswählen; **Veraltete anzeigen** listet auch veraltete Modelle, und **Werte prüfen** zeigt die Werte eines einzelnen Modells vor dem Hinzufügen. Kaeso übernimmt Preise, Limits und Fähigkeiten jedes Modells von [models.dev](https://models.dev), einer offenen Datenbank für Modelle. **Alle aktuellen Anthropic-Modelle hinzufügen**, oder OpenAI-Modelle, fügt jedes Modell des Anbieters hinzu, das nicht veraltet ist und Text oder Embeddings erzeugt, und überspringt die, die schon im Katalog stehen; Modelle, die später erscheinen, kommen nicht von selbst hinzu.

**Eigenes Modell hinzufügen** fügt ein Modell über seine ID hinzu. Wenn models.dev es nicht listet, seine Ein- und Ausgaben, Limits, Preise und das verwendete API-Format eingeben.

Die Installation aktualisiert die Daten von models.dev standardmäßig alle 24 Stunden. Instanz-Admins sehen ihren Stand und synchronisieren sie sofort unter **Instanz** › **Modellquellen**. Ein Modell, das models.dev nicht mehr listet, bleibt mit seinen letzten bekannten Werten im Katalog, markiert als **Nicht mehr gelistet**.

## Die Werte eines Modells

Das Sheet eines Modells zeigt die verwendeten Werte und woher jeder stammt: von models.dev, aus Kaesos eigenen Ergänzungen oder aus einer Anpassung in diesem Projekt. **Werte anpassen** ändert sie nur für dieses Projekt; **Auf models.dev zurücksetzen** setzt einen Wert auf die Quelle zurück. Hat jemand anderes das Modell geändert, nachdem sein Sheet geladen wurde, meldet das Speichern oder Entfernen dies und lädt die aktuellen Werte, sodass sich die Änderung erneut vornehmen lässt.

| Wert | Wirkung |
| --- | --- |
| Anzeigename | Der Name in der Konsole |
| Eingaben, Ausgaben | Text, Bilder, Audio, Video, Embeddings |
| Kontext, Ausgabelimit | Die Token-Limits des Modells |
| Eingabe-, Ausgabe-, Cache-Lese- und Cache-Schreibpreis | Pro Million Token, in US-Dollar; Nutzung und Budgets werden zu den Preisen berechnet, die zum Zeitpunkt der Anfrage gelten |
| Sendet als | Das API-Format, in dem Kaeso das Modell aufruft: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses oder OpenAI Embeddings |
| Tool-Aufrufe, Reasoning, strukturierte Ausgabe | Was das Modell unterstützt |
| Unterstützt Temperatur | Ob das Modell `temperature` annimmt |
| Adaptives Denken, Aufwandsstufen | Wie das Reasoning des Modells eingeschaltet wird und welche Aufwandsstufen es annimmt |
| Zeitlimit | Sekunden für eine Anfrage an dieses Modell; ohne Wert gilt das Anfrage-Zeitlimit der Installation |
| Veröffentlicht | Das Veröffentlichungsdatum des Modells |

**Aus dem Katalog entfernen** nimmt ein Modell heraus; Anfragen dafür werden ab dann abgelehnt, und Aliasse und Modelllisten, die es nennen, verlieren es.

## Aliasse

Ein **Alias** ist ein eigener Name des Projekts, der auf ein Modell seines Katalogs zeigt, etwa `fast` für `claude-haiku-4-5`. Apps rufen den Alias auf, und das Modell dahinter lässt sich wechseln, ohne die Apps zu ändern.

Auf dem Tab **Aliasse** fragt **Alias erstellen** nach einem Namen und einem Ziel. Ein Name hat 1 bis 200 Zeichen, ohne Leerzeichen und ohne `/`. Ein Alias mit demselben Namen wie ein Modell hat Vorrang; die Konsole markiert, dass er das Modell verdeckt.

## Wie eine Anfrage ein Modell nennt

Eine Anfrage kann ein Modell auf drei Arten nennen, und Kaeso sucht in dieser Reihenfolge:

1. Ein Alias des Projekts: `fast`.
2. Anbieter und Modell: `anthropic/claude-haiku-4-5`.
3. Die eigene ID des Modells: `claude-haiku-4-5`.

Haben zwei Anbieter ein Modell mit derselben ID, die Form `provider/model` verwenden.

Modelle, Anpassungen und Aliasse zu ändern erfordert das Recht **Modelle verwalten**.
