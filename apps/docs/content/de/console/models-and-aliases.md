---
title: Modelle und Aliasse
description: Die Modelle wählen, die ein Projekt aufrufen darf, ihre Werte anpassen und ihnen Aliasse geben.
order: 4
---

Der **Katalog** eines Projekts auf seiner Seite **Modelle** listet die Modelle, die seine Schlüssel aufrufen dürfen. Eine Anfrage nach einem Modell außerhalb des Katalogs lehnt Kaeso ab. Modelle, eigene Werte und Aliasse zu ändern erfordert das Recht **Modelle verwalten**.

## Modelle hinzufügen

**Modelle hinzufügen** durchsucht die Modelle eines Anbieters. Ein oder mehrere auswählen und hinzufügen; **Veraltete anzeigen** listet auch veraltete Modelle, und **Werte prüfen** zeigt die Werte eines Modells vor dem Hinzufügen. Jedes Modell bringt seine Preise, Limits und Fähigkeiten von [models.dev](https://models.dev) mit, einer offenen Datenbank von Modellen.

- **Alle aktuellen OpenAI-Modelle hinzufügen**, ebenso für Anthropic, fügt jedes nicht veraltete Text- und Embedding-Modell des Anbieters hinzu und überspringt die, die schon im Katalog sind. Später erscheinende Modelle kommen nicht von selbst hinzu.
- **Eigenes Modell hinzufügen** fügt ein Modell per ID hinzu. Kennt models.dev es nicht, sind Eingaben und Ausgaben, Limits, Eingabe- und Ausgabepreis und das API-Format anzugeben, mit dem Kaeso es aufruft.

Die Installation aktualisiert die Daten von models.dev standardmäßig alle 24 Stunden; Instanz-Admins sehen ihren Stand unter [**Modellquellen**](instance-administration.md#modellquellen). Ein Modell, das models.dev nicht mehr führt, bleibt mit seinen letzten bekannten Werten im Katalog und ist als **Nicht mehr gelistet** markiert. Eines, das models.dev als veraltet kennzeichnet, ist als **Veraltet** markiert.

## Verfügbarkeit

Nicht jedes Modell im Katalog stellt der Anbieter dem Anbieter-Schlüssel des Projekts auch bereit: Der Anbieter hat es vielleicht eingestellt, oder dem Schlüssel fehlt der Zugriff. Die Liste markiert solche Modelle als **Nicht verfügbar**, und das Sheet jedes Modells zeigt seine **Verfügbarkeit**:

| Verfügbarkeit | Bedeutung |
| --- | --- |
| Verfügbar | Der Anbieter hat es bei der letzten Prüfung oder Anfrage bereitgestellt |
| Nicht verfügbar | Der Anbieter hat erklärt, dass er es diesem Schlüssel nicht bereitstellt |
| Nicht geprüft | Noch liegt keine Antwort vor, oder ein Anbieter-Schlüssel der Organisation hat sich seither geändert |

**Verfügbarkeit prüfen** fragt den Anbieter jedes Modells mit dem Anbieter-Schlüssel des Projekts und meldet das Ergebnis. Als nicht verfügbar gilt ein Modell nur, wenn der Anbieter mit „nicht gefunden“ antwortet und seine Modellliste bestätigt, dass das Modell fehlt. Ein Modell, das sich nicht prüfen lässt, behält seine Verfügbarkeit, und die Meldung nennt den Grund: Das Projekt hat keinen Schlüssel für den Anbieter, der Anbieter hat den Schlüssel abgelehnt oder nicht geantwortet, seine Modellliste antwortet nicht, oder der Schlüssel hat sich während der Prüfung geändert.

Auch Anfragen liefern Kaeso diese Information. Antwortet der Anbieter auf eine Anfrage mit „nicht gefunden“ und bestätigt seine Modellliste das, wird das Modell nicht verfügbar, und die App erhält einen [Fehler, der das sagt](../api/errors.md); gelingt eine Anfrage nach einem nicht verfügbaren Modell, wird es wieder verfügbar. Eine Antwort zählt nur für den Schlüssel, der sie erhalten hat.

Ein nicht verfügbares Modell bleibt im Katalog, und Anfragen danach gehen weiter an seinen Anbieter; Kaeso leitet sie nie an ein anderes Modell um. Die Modellliste der Modell-API (`GET /v1/models`) lässt es aus. Dem Anbieter-Schlüssel Zugriff auf das Modell geben oder das Modell aus dem Katalog entfernen. Prüfungen stehen im Audit-Log als `catalogue_model.check`.

## Die Werte eines Modells

Das Sheet eines Modells zeigt die **Verwendeten Werte** und woher jeder stammt: von models.dev, aus Kaesos eigenen Ergänzungen oder aus einem eigenen Wert dieses Projekts. **Werte anpassen** ändert sie nur für dieses Projekt, und **Auf models.dev zurücksetzen** stellt einen Wert der Quelle wieder her. Hat jemand anderes das Modell geändert, nachdem sein Sheet geladen wurde, meldet das Speichern dies und lädt die aktuellen Werte, damit sich die Änderung erneut vornehmen lässt.

| Wert | Bedeutung |
| --- | --- |
| Anzeigename | Der Name in der Konsole |
| Eingaben, Ausgaben | Text, Bilder, Audio, Video, Embeddings |
| Kontext, Ausgabelimit | Die Token-Limits des Modells |
| Eingabe-, Ausgabe-, Cache-Lese- und Cache-Schreibpreis | US-Dollar je Million Token; Nutzung und Budgets rechnen mit den Preisen, die zum Zeitpunkt der Anfrage gelten |
| Sendet als | Das API-Format, mit dem Kaeso das Modell aufruft: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses oder OpenAI Embeddings |
| Tool-Aufrufe, Reasoning, strukturierte Ausgabe | Was das Modell unterstützt |
| Unterstützt Temperatur | Ob das Modell `temperature` annimmt |
| Unterstützt erzwungene Tool-Wahl | Ob das Modell `tool_choice` `required`, `any` oder ein benanntes Tool annimmt; für ein Modell ohne diese Fähigkeit lehnt Kaeso solche Anfragen ab, bevor es den Anbieter aufruft |
| Adaptives Denken, Aufwandsstufen | Wie das Reasoning des Modells eingeschaltet wird und welche Stufen es annimmt |
| Zeitlimit | Sekunden, die eine Anfrage an dieses Modell dauern darf; ohne Angabe gilt das Anfrage-Zeitlimit der Installation |
| Veröffentlicht | Das Veröffentlichungsdatum des Modells |

**Aus dem Katalog entfernen** nimmt ein Modell heraus. Anfragen danach werden ab dann abgelehnt, und Aliasse und Modelllisten, die es nennen, verlieren es.

## Aliasse

Ein **Alias** ist ein eigener Name des Projekts für ein Modell seines Katalogs, etwa `fast` für `claude-haiku-4-5`. Apps senden den Alias als Modell, und das Modell dahinter lässt sich wechseln, ohne die Apps anzufassen.

Auf dem Tab **Aliasse** fragt **Alias erstellen** nach einem Namen und einem **Ziel**. Ein Name hat 1 bis 200 Zeichen, ohne Leerzeichen und ohne `/`. Ein Alias hat Vorrang vor einem gleichnamigen Modell; die Liste kennzeichnet ihn mit **Verdeckt ein Modell**.

## Wie eine Anfrage ein Modell nennt

Kaeso sucht das Modell, das eine Anfrage nennt, in dieser Reihenfolge:

1. Ein Alias des Projekts: `fast`.
2. Anbieter und Modell: `anthropic/claude-haiku-4-5`.
3. Eine Modell-ID: `claude-haiku-4-5`.

Haben zwei Anbieter ein Modell mit derselben ID, die Form `provider/model` verwenden.
