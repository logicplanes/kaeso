---
title: Modelle und Aliasse
description: Wählen Sie die Modelle, die ein Projekt aufrufen darf, passen Sie ihre Werte an und geben Sie ihnen eigene Namen.
order: 4
---

Der Katalog eines Projekts auf seiner Seite **Modelle** listet die Modelle, die seine Schlüssel aufrufen dürfen. Eine Anfrage nach einem Modell außerhalb des Katalogs lehnt Kaeso ab. Modelle, ihre Werte und Aliasse zu ändern erfordert das Recht **Modelle verwalten**.

## Modelle hinzufügen

Wählen Sie **Modelle hinzufügen** und durchsuchen Sie die Modelle eines Anbieters. Wählen Sie eines oder mehrere aus und fügen Sie sie hinzu. **Veraltete anzeigen** listet auch veraltete Modelle, und **Werte prüfen** zeigt die Werte eines Modells, bevor Sie es hinzufügen.

Jedes Modell bringt seine Preise, Limits und Fähigkeiten von [models.dev](https://models.dev) mit, einer offenen Datenbank von Modellen. Zwei Abkürzungen decken den Rest ab:

- **Alle aktuellen OpenAI-Modelle hinzufügen**, ebenso für Anthropic, fügt jedes nicht veraltete Text- und Embeddings-Modell des Anbieters hinzu. Modelle, die schon im Katalog sind, überspringt es, und später erscheinende Modelle fügt es nicht hinzu.
- **Eigenes Modell hinzufügen** fügt ein Modell per ID hinzu. Kennt models.dev es nicht, geben Sie Eingaben und Ausgaben, Limits, Preise und das API-Format an, mit dem Kaeso es aufruft.

Die Installation aktualisiert die Daten von models.dev standardmäßig alle 24 Stunden, und Instanz-Admins sehen ihren Stand unter [Modellquellen](instance-administration.md#modelldaten-synchronisieren). Ein Modell, das models.dev nicht mehr führt, bleibt mit seinen letzten bekannten Werten im Katalog und ist als **Nicht mehr gelistet** markiert. Ein Modell, das models.dev als veraltet kennzeichnet, ist als **Veraltet** markiert.

## Verfügbarkeit

Nicht jedes Modell im Katalog stellt der Anbieter dem Anbieter-Schlüssel des Projekts auch bereit. Der Anbieter hat es vielleicht eingestellt, oder dem Schlüssel fehlt der Zugriff. Die Liste markiert solche Modelle als **Nicht verfügbar**, und das Sheet jedes Modells zeigt seine **Verfügbarkeit**:

| Verfügbarkeit | Bedeutung |
| --- | --- |
| Verfügbar | Der Anbieter hat es bei der letzten Prüfung oder Anfrage bereitgestellt |
| Nicht verfügbar | Der Anbieter hat erklärt, dass er es diesem Schlüssel nicht bereitstellt |
| Nicht geprüft | Noch liegt keine Antwort vor, oder ein Anbieter-Schlüssel der Organisation hat sich seither geändert |

Wählen Sie **Verfügbarkeit prüfen**, um den Anbieter jedes Modells mit dem Anbieter-Schlüssel des Projekts zu fragen. Kaeso markiert ein Modell nur dann als nicht verfügbar, wenn der Anbieter mit „nicht gefunden“ antwortet und seine Modellliste bestätigt, dass das Modell fehlt. Ein Modell, das sich nicht prüfen lässt, behält seine Verfügbarkeit, und die Meldung nennt den Grund, etwa dass das Projekt keinen Schlüssel für diesen Anbieter hat.

Auch Anfragen ändern die Verfügbarkeit. Antwortet der Anbieter auf eine Anfrage mit „nicht gefunden“ und bestätigt seine Modellliste das, wird das Modell nicht verfügbar, und die App erhält einen [Fehler, der das sagt](../api/errors.md). Gelingt eine Anfrage nach einem nicht verfügbaren Modell, wird es wieder verfügbar. Jede Antwort zählt nur für den Anbieter-Schlüssel, der sie erhalten hat.

Ein nicht verfügbares Modell bleibt im Katalog, und Kaeso beantwortet Anfragen danach mit diesem Fehler, ohne den Anbieter aufzurufen. Alle fünf Minuten lässt es eine Anfrage zum Anbieter durch, um erneut zu fragen; so wird ein Modell, auf das der Schlüssel Zugriff erhalten hat, von selbst wieder verfügbar. **Verfügbarkeit prüfen** aktualisiert es sofort. Anfragen gehen nie an ein anderes Modell. Die Modellliste der Modell-API (`GET /v1/models`) lässt es aus. Um das zu beheben, geben Sie dem Anbieter-Schlüssel Zugriff auf das Modell, oder entfernen Sie das Modell aus dem Katalog. Das Audit-Log verzeichnet jede Prüfung.

## Werte eines Modells ändern

Das Sheet eines Modells zeigt unter **Verwendete Werte** jeden Wert mit seiner Herkunft: von models.dev, aus Kaesos eigenen Ergänzungen oder aus einem eigenen Wert dieses Projekts. **Werte anpassen** ändert sie nur für dieses Projekt, und **Auf models.dev zurücksetzen** stellt einen Wert der Quelle wieder her. Hat jemand anderes das Modell geändert, nachdem sein Sheet geladen wurde, meldet das Speichern dies und lädt die aktuellen Werte, sodass Sie Ihre Änderung erneut vornehmen können.

| Wert | Bedeutung |
| --- | --- |
| Anzeigename | Der Name, den die Konsole zeigt |
| Eingaben, Ausgaben | Text, Bilder, Audio, Video, Embeddings |
| Kontext, Ausgabelimit | Die Token-Limits des Modells |
| Eingabe-, Ausgabe-, Cache-Lese- und Cache-Schreibpreis | US-Dollar je Million Token. Nutzung und Budgets rechnen mit den Preisen, die zum Zeitpunkt der Anfrage gelten. Der Cache-Schreibpreis ist der für fünf Minuten, wie models.dev ihn führt; Schreibzugriffe in Anthropics Ein-Stunden-Cache kosten das Doppelte des Eingabepreises, [Anthropics Regel](https://platform.claude.com/docs/en/about-claude/pricing), oder den Cache-Schreibpreis, wenn das Modell keinen Eingabepreis hat |
| Sendet als | Das API-Format, mit dem Kaeso das Modell aufruft: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses oder OpenAI Embeddings |
| Tool-Aufrufe, Reasoning, strukturierte Ausgabe | Was das Modell unterstützt |
| Unterstützt Temperatur | Ob das Modell `temperature` annimmt |
| Unterstützt erzwungene Tool-Wahl | Ob das Modell `tool_choice` mit `required`, `any` oder einem benannten Tool annimmt. Für ein Modell ohne diese Fähigkeit lehnt Kaeso solche Anfragen ab, bevor es den Anbieter aufruft |
| Adaptives Denken, Aufwandsstufen | Wie das Reasoning des Modells eingeschaltet wird und welche Stufen es annimmt |
| Zeitlimit | Wie viele Sekunden eine Anfrage an dieses Modell dauern darf. Ohne Angabe gilt das Anfrage-Zeitlimit der Installation |
| Veröffentlicht | Das Veröffentlichungsdatum des Modells |

## Modell entfernen

Wählen Sie **Aus dem Katalog entfernen** im Sheet eines Modells. Kaeso lehnt Anfragen nach dem Modell ab sofort ab, und Aliasse und Modelllisten, die es nennen, verlieren es.

## Alias erstellen

Ein Alias ist ein eigener Name des Projekts für ein Modell seines Katalogs, etwa `fast` für `claude-haiku-4-5`. Apps senden den Alias als Modell, und Sie können das Modell dahinter wechseln, ohne die Apps anzufassen.

Wählen Sie auf dem Tab **Aliasse** **Alias erstellen** und geben Sie einen Namen und ein **Ziel** an. Ein Name hat 1 bis 200 Zeichen, ohne Leerzeichen und ohne `/`. Ein Alias hat Vorrang vor einem gleichnamigen Modell, und die Liste kennzeichnet ihn mit **Verdeckt ein Modell**.

## Wie eine Anfrage ein Modell nennt

Kaeso sucht das Modell, das eine Anfrage nennt, in dieser Reihenfolge:

1. Ein Alias des Projekts: `fast`.
2. Ein Anbieter und ein Modell: `anthropic/claude-haiku-4-5`.
3. Eine Modell-ID: `claude-haiku-4-5`.

Haben zwei Anbieter ein Modell mit derselben ID, verwenden Sie die Form `provider/model`.

## Nächste Schritte

- [Modelle aufrufen](../get-started/calling-models.md): Rufen Sie die Modelle Ihres Katalogs aus einer App auf.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie Anfragen, Token und Ausgaben eines Projekts oder eines Schlüssels.
- [Playground](playground.md): Vergleichen Sie Modelle nebeneinander, bevor Sie eines wählen.
