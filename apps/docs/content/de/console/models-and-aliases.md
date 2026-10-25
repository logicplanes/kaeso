---
title: Modelle und Aliasse
description: Wählen Sie die Modelle, die ein Projekt aufrufen darf, passen Sie ihre Werte an und geben Sie ihnen eigene Namen.
order: 5
---

Der Katalog eines Projekts auf seiner Seite **Modelle** listet die Modelle, die seine Schlüssel aufrufen dürfen. Eine Anfrage nach einem Modell außerhalb des Katalogs lehnt Kaeso ab. Modelle, ihre Werte und Aliasse zu ändern erfordert das Recht **Modelle verwalten**.

## Modelle hinzufügen

Wählen Sie **Modelle hinzufügen**, wählen Sie dann in der durchsuchbaren Liste neben dem Suchfeld einen Anbieter und durchsuchen Sie seine Modelle. Die von Ihrer Organisation verbundenen Anbieter stehen zuerst. Der Dialog merkt sich den Anbieter, bis Sie die Konsole neu laden oder sich abmelden. Hat die Organisation keinen [Anbieter-Schlüssel](provider-keys.md) für ihn, weist der Dialog darauf hin, dass Anfragen an seine Modelle fehlschlagen, bis jemand einen hinzufügt.

Der Dialog listet die Modelle, die neuesten zuerst, jedes mit seinem Kontextfenster sowie seinen Eingabe- und Ausgabepreisen. Passen mehr als 50 Modelle, zeigt er die ersten 50 und sagt, wie viele passen, sodass Sie sie mit der Suche eingrenzen. Wählen Sie eines oder mehrere aus und fügen Sie sie hinzu. **Veraltete anzeigen** listet auch veraltete Modelle, und **Werte prüfen** zeigt die Werte eines Modells, bevor Sie es hinzufügen.

Jedes Modell bringt seine Preise, Limits und Fähigkeiten von [models.dev](https://models.dev) mit, einer offenen Datenbank von Modellen. Zwei Abkürzungen decken den Rest ab:

- **Alle aktuellen OpenAI-Modelle hinzufügen**, ebenso für jeden anderen Anbieter, fügt jedes nicht veraltete Text- und Embeddings-Modell des Anbieters hinzu. Modelle, die schon im Katalog sind, überspringt es, und später erscheinende Modelle fügt es nicht hinzu. Der Dialog sagt, wie viele Modelle er hinzufügt, und warnt vor 50 oder mehr.
- **Eigenes Modell hinzufügen** fügt ein Modell per ID hinzu. Kennt models.dev es nicht, geben Sie Eingaben und Ausgaben, Limits, Preise und das API-Format an, mit dem Kaeso es aufruft. Eine ID darf nicht `.` oder `..` sein und keines davon zwischen Schrägstrichen enthalten, weil eine URL diesen Teil verwirft.

Für einen [eigenen Anbieter](custom-providers.md#seine-modelle-hinzufügen) Ihrer Organisation listet der Dialog stattdessen die Modell-IDs, die die Modellliste des Anbieters nennt, und Sie geben jeden Wert selbst ein.

Die Installation aktualisiert die Daten von models.dev standardmäßig alle 24 Stunden, und Instanz-Admins sehen ihren Stand unter [Modellquellen](instance-administration.md#modelldaten-synchronisieren). Ein Modell, das models.dev nicht mehr führt, bleibt mit seinen letzten bekannten Werten im Katalog und ist als **Nicht mehr gelistet** markiert. Ein Modell, das models.dev als veraltet kennzeichnet, ist als **Veraltet** markiert.

## Ein Modell finden

Der Tab **Katalog** gruppiert die Modelle nach Anbieter. Grenzen Sie ihn auf einen Anbieter ein, auf eine Fähigkeit wie **Tool-Aufrufe** oder **Bildeingabe**, oder mit **Nur angepasste** auf Modelle, deren Werte in diesem Projekt geändert sind. Auf einem Smartphone liegen diese Filter unter **Filter**. Das Sortiermenü ordnet die Modelle jedes Anbieters nach **Modell-ID**, **Eingabepreis** oder **Neueste zuerst**.

## Verfügbarkeit

Nicht jedes Modell im Katalog stellt der Anbieter dem Anbieter-Schlüssel des Projekts auch bereit. Der Anbieter hat es vielleicht eingestellt, oder dem Schlüssel fehlt der Zugriff. Die Liste markiert solche Modelle als **Nicht verfügbar**, und das Sheet jedes Modells zeigt seine **Verfügbarkeit**:

| Verfügbarkeit | Bedeutung |
| --- | --- |
| Verfügbar | Der Anbieter hat es bei der letzten Prüfung oder Anfrage bereitgestellt |
| Nicht verfügbar | Der Anbieter hat erklärt, dass er es diesem Schlüssel nicht bereitstellt |
| Nicht geprüft | Noch liegt keine Antwort vor, oder ein Anbieter-Schlüssel der Organisation hat sich seither geändert |

Wählen Sie **Verfügbarkeit prüfen**, um den Anbieter jedes Modells mit dem Anbieter-Schlüssel des Projekts zu fragen. Kaeso markiert ein Modell nur dann als nicht verfügbar, wenn der Anbieter mit „nicht gefunden“ antwortet und seine Modellliste bestätigt, dass das Modell fehlt. Kann Kaeso ein Modell nicht prüfen, behält das Modell seine Verfügbarkeit, und die Meldung nennt den Grund. Zum Beispiel hat das Projekt keinen Schlüssel für diesen Anbieter, oder der Anbieter hat den Schlüssel abgelehnt, auch wenn nur seine Modellliste ihn ablehnt.

Auch Anfragen ändern die Verfügbarkeit. Antwortet der Anbieter auf eine Anfrage mit „nicht gefunden“ und bestätigt seine Modellliste das, wird das Modell nicht verfügbar, und die App erhält einen [Fehler, der das sagt](../api/errors.md). Gelingt eine Anfrage nach einem nicht verfügbaren Modell, wird es wieder verfügbar. Jede Antwort zählt nur für den Anbieter-Schlüssel, der sie erhalten hat.

Ein nicht verfügbares Modell bleibt im Katalog, und Kaeso beantwortet Anfragen danach mit diesem Fehler, ohne den Anbieter aufzurufen. Anfragen gehen nie an ein anderes Modell. Die Modellliste der Modell-API (`GET /v1/models`) lässt es aus.

Um das zu beheben, geben Sie dem Anbieter-Schlüssel Zugriff auf das Modell, oder entfernen Sie das Modell aus dem Katalog. Alle fünf Minuten lässt Kaeso eine Anfrage zum Anbieter durch, um erneut zu fragen. Hat der Schlüssel Zugriff erhalten, wird das Modell von selbst wieder verfügbar, oder sofort, wenn Sie **Verfügbarkeit prüfen** wählen. Das Audit-Log verzeichnet jede Prüfung.

## Werte eines Modells ändern

Das Sheet eines Modells zeigt unter **Verwendete Werte** jeden Wert mit seiner Herkunft: von models.dev, aus Kaesos eigenen Ergänzungen oder aus einem eigenen Wert dieses Projekts. **Werte anpassen** ändert sie nur für dieses Projekt, und **Auf models.dev zurücksetzen** stellt einen Wert der Quelle wieder her. Hat jemand anderes das Modell geändert, nachdem sein Sheet geladen wurde, meldet das Speichern dies und lädt die aktuellen Werte, sodass Sie Ihre Änderung erneut vornehmen können.

| Wert | Bedeutung |
| --- | --- |
| Anzeigename | Der Name, den die Konsole zeigt |
| Eingaben, Ausgaben | Eingaben: Text, Bilder, PDF, Audio und Video. Ausgaben: Text, Bilder, Audio und Embeddings |
| Kontext, Ausgabelimit | Die Token-Limits des Modells |
| Eingabe-, Ausgabe-, Cache-Lese- und Cache-Schreibpreis | US-Dollar je Million Token. Nutzung und Budgets rechnen mit den Preisen, die zum Zeitpunkt der Anfrage gelten. Der Cache-Schreibpreis ist der für fünf Minuten, wie models.dev ihn führt. Schreibzugriffe in Anthropics Ein-Stunden-Cache kosten das Doppelte des Eingabepreises, [Anthropics Regel](https://platform.claude.com/docs/en/about-claude/pricing), oder den Cache-Schreibpreis, wenn das Modell keinen Eingabepreis hat |
| Sendet als | Das API-Format, mit dem Kaeso das Modell aufruft: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses oder OpenAI Embeddings |
| Tool-Aufrufe, Reasoning, strukturierte Ausgabe | Was das Modell unterstützt |
| Temperatur | Ob das Modell `temperature` annimmt |
| Erzwungene Tool-Wahl | Ob das Modell `tool_choice` mit `required`, `any` oder einem benannten Tool annimmt. Für ein Modell ohne diese Fähigkeit lehnt Kaeso solche Anfragen ab, bevor es den Anbieter aufruft |
| Adaptives Denken, Aufwandsstufen | Wie das Reasoning des Modells eingeschaltet wird und welche Stufen es annimmt |
| Thinking-Bindung | Ob ein Anthropic-Modell prüft, dass zurückgespieltes Denken aus derselben Unterhaltung stammt. Bei einem solchen Modell bittet Kaeso Anthropic, Denken zu verwerfen, das Apps in anderen Formaten verändert zurückschicken, statt die Anfrage scheitern zu lassen |
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

Haben zwei Anbieter ein Modell mit derselben ID, verwenden Sie die Form `provider/model`. Kaeso trennt sie am ersten `/`, eine Modell-ID mit `/` bleibt also erhalten: `togetherai/Qwen/Qwen3.5-9B` meint `Qwen/Qwen3.5-9B` von Together AI.

Eine Modell-ID kann `/` enthalten, etwa OpenRouters `anthropic/claude-sonnet-4.5` oder Groqs `openai/gpt-oss-20b`. Kaeso sucht die Form Anbieter/Modell vor einer Modell-ID, daher geht ein Name, der mit der ID eines anderen Anbieters beginnt, an diesen Anbieter. Hat ein Projekt OpenRouters `openai/gpt-4o` und OpenAIs `gpt-4o`, ruft `openai/gpt-4o` OpenAI auf. Stellen Sie den Anbieter voran, um sicherzugehen: `openrouter/openai/gpt-4o` ruft immer das Modell von OpenRouter auf, `groq/openai/gpt-oss-20b` immer das von Groq.

## Nächste Schritte

- [Modelle aufrufen](../get-started/calling-models.md): Rufen Sie die Modelle Ihres Katalogs aus einer App auf.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie Anfragen, Token und Ausgaben eines Projekts oder eines Schlüssels.
- [Playground](playground.md): Vergleichen Sie Modelle nebeneinander, bevor Sie eines wählen.
