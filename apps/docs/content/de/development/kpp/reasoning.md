---
title: Reasoning über Formate hinweg
nav: Reasoning
description: Wie die Codecs Reasoning, seine Einstellungen und seine Signaturen zwischen den API-Formaten weitergeben.
order: 3
---

Anthropic signiert seine Thinking-Blöcke, und OpenAI verschlüsselt seine Reasoning-Items. Opakes Reasoning (Signaturen, verschlüsselter Inhalt, Item-IDs) wird immer nur an den Dialekt und den Anbieter zurückgespielt, die es ausgegeben haben.

## Welches Reasoning zurückgespielt wird

- **In Anfragen** werden lesbarer Reasoning-Text und Zusammenfassungen, die an einen anderen Dialekt gehen, als Hinweis weggelassen. Der [Reasoning-Blob](#reasoning-blobs) daneben stellt den Originalblock wieder her, wenn die Unterhaltung zu ihrem Anbieter zurückkehrt.
- **Ohne Signatur** geht ein Anthropic-`thinking`-Block, der aus Anthropic Messages gelesen wurde, so an Anthropic zurück, wie er ankam, in Anfragen und Antworten. Eine fehlende `signature` bleibt fehlend, und `signature: ""` bleibt `""`. Unsigniertes Anthropic-Reasoning, das nicht aus Anthropic Messages gelesen wurde, etwa ein von Code gebauter Block, hat nichts zum Zurückspielen und wird weggelassen.
- **In Antworten** wird Reasoning eines anderen Dialekts in der Reasoning-Form der App gezeigt, wo diese eine hat. Dort reist der Originalblock in einem Reasoning-Blob mit, sodass es in der nächsten Runde wieder seinen Ursprung erreicht. Apps von OpenAI Chat haben keine solche Form, daher bewahrt das Gateway ihr Reasoning zwischen den Runden auf.

## Aufwand und Budget

Reasoning-Einstellungen werden zwischen einer Aufwandsstufe und einem Token-Budget umgerechnet:

| Von | Nach | Wird zu |
| --- | --- | --- |
| Aufwand `none` | Anthropic | Kein `thinking`, der Standard bei Anthropic; Modelle, die ohne es denken, tun das weiterhin |
| Aufwand `low` bis `max` | Anthropic-Modell mit adaptivem Thinking | Adaptives Thinking mit diesem Aufwand |
| Aufwand `low` bis `max` | Anthropic-Modell ohne adaptives Thinking | `enabled`-Thinking mit einem Budget aus der Tabelle unten |
| Nur ein Budget | Anthropic | `enabled`-Thinking mit diesem Budget |
| Ein Anthropic-Budget | OpenAI | Ein Aufwand aus der Tabelle unten |
| Anthropic `thinking: {type: "disabled"}` | OpenAI | Aufwand `none` |

Beide OpenAI-Dialekte dokumentieren `none` als niedrigsten Wert ihres gemeinsamen Enums `ReasoningEffort`, daher hat abgeschaltetes Thinking von Anthropic immer eine Entsprechung. Die Aufwandsstufe `minimal` hat keine Zeile und bleibt für Anthropic `Unsupported`.

Die Umrechnung steckt in `kaeso_codecs::cross::reasoning_map`.

| Anthropic `budget_tokens` | OpenAI-Aufwand |
| --- | --- |
| unter 4.096 | `low` |
| 4.096 bis 16.383 | `medium` |
| 16.384 und mehr | `high` |

Ein ausdrücklicher Aufwand in derselben Anfrage hat Vorrang vor dem Budget. Wird das Budget zu einem Aufwand, den das Modell nicht unter seinen Aufwandsstufen führt, lehnt das Gateway die Anfrage ab, bevor es sie sendet.

Die Ablehnung nennt `thinking.budget_tokens` und das nächste Budget, das zu einer Stufe des Modells führt. Führt keines dazu, bittet sie stattdessen darum, das Thinking auszuschalten. So kennt etwa `mistral-small-latest` nur `none` und `high` und braucht daher ein `budget_tokens` von 16.384 oder mehr.

| OpenAI-Aufwand | Anthropic `budget_tokens` |
| --- | --- |
| `low` | 2.048 |
| `medium` | 8.192 |
| `high`, `xhigh`, `max` | 24.576 |

Diese Tabelle gilt nur, wenn das Zielmodell kein adaptives Thinking hat (Claude 4.5 und älter). Das Gateway gibt das mit `EgressCtx::adaptive_thinking = Some(false)` aus dem Feld `adaptive_thinking` des Katalogs an; Claude 4.7 und neuer lehnen `enabled`-Thinking ab und bekommen stattdessen adaptives Thinking mit dem Aufwand.

Das Budget wird dann an die Regeln von Anthropic angepasst: mindestens 1.024 und kleiner als `max_tokens`, also `max_tokens − 1`, wenn das kleiner ist. Ist `max_tokens` 1.024 oder kleiner, gibt es kein gültiges Budget, daher wird `max_tokens` um 1.024 erhöht und das Budget ist 1.024: Die App behält ihr eigenes `max_tokens` für die Antwort. `reasoning_map::budget_within` übernimmt Anpassen und Erhöhen, `reasoning_map::fit_budget` nur das Anpassen.

Die erste Quelle ist das Enum `ReasoningEffort` von OpenAI in der OpenAPI-Spezifikation: `none`, `minimal`, `low`, `medium`, `high`, `xhigh` und `max`, wobei nicht jedes Modell jeden Wert unterstützt. Die zweite ist die Seite [Extended Thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) von Anthropic. Dort steht, dass `budget_tokens` mindestens 1.024 und kleiner als `max_tokens` ist. Außerdem steht dort, dass es adaptives Thinking bei Claude Sonnet 4.5, Opus 4.5, Haiku 4.5 und früheren Claude-4-Modellen nicht gibt.

## Lesbares Reasoning und Zusammenfassungen

`reasoning.summary` und `reasoning.include_encrypted` sind über Dialekte hinweg Hinweise: Sie steuern nur, welches Reasoning zurückkommt, und werden daher weggelassen, wo das Ziel keine Entsprechung hat. Codex sendet beide bei jeder Anfrage.

OpenAI liefert lesbares Reasoning nur, wenn die Anfrage `reasoning.summary` setzt, und Claude Opus 4.7 und neuer liefern Thinking-Blöcke mit leerem Text, außer die Anfrage setzt `thinking.display: "summarized"`. Daher gilt, außer bei der Token-Zählung:

- eine Anfrage aus einem anderen Dialekt mit Reasoning-Zusammenfassung (`auto`, `concise`, `detailed`) und eingeschaltetem Thinking geht mit `display: "summarized"` an Anthropic;
- eine Anfrage im Format Anthropic Messages mit eingeschaltetem Thinking (`enabled`, `adaptive` oder ein Aufwand außer `none`) und einem `display` außer `omitted` geht mit `summary: "auto"` an OpenAI Responses.

Das Gateway teilt den Encodern außerdem mit, ob das Zielmodell Reasoning beherrscht (`EgressCtx::reasoning`, aus dem Feld `reasoning` des Katalogs). Bei `Some(true)`:

- schaltet eine Zusammenfassung ohne Aufwand, Modus oder Budget (OpenAI `reasoning: {summary: "auto"}`) das Thinking von Anthropic in der Standardtiefe des Modells ein. Das ist adaptives Thinking ohne Aufwand oder das Budget für `medium`, den Standardaufwand von OpenAI;
- zählt Anthropic-Thinking `adaptive` ohne Aufwand für OpenAI Responses als Reasoning: Es fordert `include: ["reasoning.encrypted_content"]` an und, wie oben, eine Zusammenfassung.

Anfragen ohne Reasoning-Einstellungen, der Aufwand `none` und Modelle, die kein Reasoning beherrschen oder unbekannt sind, bekommen nie Reasoning eingeschaltet. Apps von OpenAI Chat, deren Format kein Reasoning zeigt, fordern keine Zusammenfassungen an.

Anbieter, die Chat Completions bereitstellen, liefern lesbares Reasoning in einem eigenen Feld, etwa DeepSeeks `reasoning_content` oder Groqs `reasoning`, ohne dass eine Zusammenfassung angefordert wird. Ihr Chat-Profil nennt das Feld (`reasoning_fields`, [Anbieterprofile](openai-chat-providers.md#anbieterprofile)), sodass das Reasoning zu einem KPP-Reasoning-Block wird: Apps von Anthropic Messages erhalten es als `thinking`-Block und Apps von Responses als Reasoning-Item mit Zusammenfassung, jeweils mit dem ursprünglichen Block in einem [Reasoning-Blob](#reasoning-blobs).

## Thinking und Sampling

Sampling-Einstellungen und erzwungene Tools sind in OpenAI-Apps alltäglich. Schaltet Kaeso das Thinking für eine Anfrage aus einem anderen Dialekt ein, passt es daher an, was Anthropic neben Thinking verbietet, statt die Anfrage abzulehnen:

- `temperature`, `top_p` und `top_k` werden weggelassen. Anthropic lehnt `temperature` und `top_k` mit Thinking ab und begrenzt `top_p` auf 0,95 bis 1, und Claude Opus 4.7 und neuer lehnen jeden Wert ab, der nicht dem Standard entspricht.
- `enabled`-Thinking wird nicht mit einem erzwungenen Tool (`tool_choice` `required` oder ein benanntes Tool) gesendet, einer Kombination, die Anthropic ablehnt. Wo Anthropic erzwungene Tools akzeptiert, bei adaptivem Thinking, beginnt die Antwort mit dem Tool-Aufruf und enthält ohnehin kein Thinking, daher geht nichts verloren. Ein erzwungenes Tool mit adaptivem Thinking wird unverändert gesendet.

Die eigenen Einstellungen einer Anthropic-App werden so gesendet, wie sie ankamen.

## Tool-Runden ohne Thinking

Anthropic lehnt eine Anfrage mit `enabled`-Thinking ab, deren letzte Assistentenrunde Tool-Aufrufe ohne vorangehenden, zurückspielbaren Thinking-Block gemacht hat. Der Block muss signiert oder redigiert sein, und die Ablehnung lautet „a final assistant message must start with a thinking block“. Eine solche Anfrage geht für diese Runde ohne `thinking` an Anthropic. Adaptives Thinking bleibt erhalten, weil adaptive Modelle den führenden Thinking-Block nicht brauchen.

Da Reasoning mit der Unterhaltung mitreist, ist das selten. Es kommt trotzdem in drei Fällen vor:

- Die Runde stammt wirklich von einem anderen Anbieter, nach einem Modellwechsel mitten in einer Tool-Schleife.
- Die App hat das Reasoning-Item, den Thinking-Block oder den Blob weggelassen.
- Das Gateway hat das Reasoning einer Chat-App nicht mehr, weil es abgelaufen ist oder ein anderer API-Schlüssel die Runde gesendet hat.

## Reasoning über Runden hinweg

Ohne Hilfe würde jede Runde einer Tool-Schleife, in der eine App das Modell eines anderen Anbieters nutzt, das Reasoning dieses Modells verlieren. Budget-Thinking würde in jeder Folgerunde abgeschaltet, und adaptives Thinking würde verlieren, was es zuvor überlegt hat. `kaeso_codecs::continuity` lässt Reasoning mit der Unterhaltung mitreisen, für Responses- und Anthropic-Apps in einem Blob, für Chat-Apps im Gateway.

### Reasoning-Blobs

Für Apps von OpenAI Responses und Anthropic Messages verpackt KPP den ganzen Reasoning-Block eines anderen Dialekts in einen Reasoning-Blob. Das umfasst lesbaren Text, Zusammenfassungen, Signatur, verschlüsselten Inhalt, redigierte Daten und die Item-ID. Der Blob ist `continuity::REASONING_BLOB_PREFIX` (`kpp:reasoning:v1:`), gefolgt vom KPP-JSON des Blocks in base64url ohne Padding, ohne Form-Markierungen.

| App | Bekommt | Blob steckt in |
| --- | --- | --- |
| OpenAI Responses | Ein Reasoning-Item mit einer ID `rs_kaeso_…` und dem Thinking als einem einzigen Zusammenfassungstext, gestreamt als `response.reasoning_summary_*`-Events | `encrypted_content` |
| Anthropic Messages | Einen `thinking`-Block mit den Zusammenfassungen und dem Reasoning-Text als Thinking, Absätze durch eine Leerzeile getrennt | `signature`, gestreamt als ein einziges `signature_delta`, wenn der Block stoppt |

Beide Anfrage-Decoder erkennen einen Blob, wo immer er steht (`continuity::unwrap_reasoning`), und stellen den Originalblock mit seinem Ursprung wieder her. Die Encoder spielen ihn dann wie jedes andere Reasoning an seinen eigenen Dialekt und Anbieter zurück.

- **Immer gesendet.** Der Blob wird gesendet, ob die App `include: ["reasoning.encrypted_content"]` angefordert hat oder nicht, und unabhängig von ihrer Einstellung für `store`. Kaeso speichert nichts, und die Agent-Schleifen der SDKs senden Ausgabe-Items und Content-Blöcke so zurück, wie sie ankamen. Zu diesen Schleifen gehören `to_input_list()` von openai-agents, Codex, Claude Code und der Tool Runner von Anthropic.
- **Der Blob hat Vorrang.** Er enthält den lesbaren Text ein zweites Mal, weil er den Block exakt wiederherstellen muss (Anthropic prüft den Thinking-Text gegen seine Signatur). Apps können die lesbare Kopie bearbeiten oder weglassen. Sendet eine App beides zurück, hat der Blob Vorrang.
- **Nie mit Daten des Anbieters verwechselt.** Das Präfix enthält `:`, das weder im base64-Alphabet der Anthropic-Signaturen noch im base64url-Alphabet des verschlüsselten Inhalts von OpenAI vorkommt. Ein Wert mit dem Präfix, der sich nicht dekodieren lässt, bleibt als opake Daten des Anbieters erhalten, und der Anbieter lehnt ihn dann ab, sodass nichts stillschweigend wegfällt.
- **Von Kaeso weder verschlüsselt noch signiert.** Er enthält, was die App schon gesehen hat, und Daten, die nur der ausgebende Anbieter prüfen kann.

Anfragen aus anderen Dialekten fordern bei OpenAI Responses `include: ["reasoning.encrypted_content"]` an, wenn sie einen Aufwand außer `none` oder ein Thinking-Budget setzen. Sie fordern es auch an, wenn sie das Thinking für ein Modell einschalten, von dem das Gateway weiß, dass es Reasoning beherrscht ([Lesbares Reasoning und Zusammenfassungen](#lesbares-reasoning-und-zusammenfassungen)). Sonst kann Kaeso nicht erkennen, ob das Modell Reasoning beherrscht, und OpenAI lehnt verschlüsseltes Reasoning für Modelle ab, die keines beherrschen. Da diese Anfragen `store: false` tragen ([Speicherung und IDs bei Responses](cross-format.md#speicherung-und-ids-bei-responses)), wird Reasoning ohne verschlüsselten Inhalt nicht an OpenAI gesendet, das seine Item-ID nicht auflösen könnte.

### Vom Gateway aufbewahrtes Reasoning

Chat Completions hat kein Feld für Reasoning, und KPP erfindet keines. Die Antwort lässt das Reasoning weg, und das Gateway bewahrt es auf, mit den Tool-Aufrufen als Schlüssel, denen es vorausging. Tool-Aufruf-IDs sind eindeutig, werden vom Anbieter vergeben und von jedem Chat-Client zurückgesendet, daher eignen sie sich als Schlüssel für das Reasoning, ohne das Format der App zu ändern.

1. Nach einer Antwort liefern `continuity::reasoning_for_tool_calls` (bei einer vollständigen Antwort) oder ein `continuity::ReasoningCollector` (bei einem Stream) ein `CarriedReasoning` pro Tool-Aufruf, dem zurückspielbares Reasoning vorausging: signiertes oder redigiertes Thinking von Anthropic, verschlüsseltes Reasoning von OpenAI. Reasoning nach dem letzten Tool-Aufruf wird nicht aufbewahrt.
2. Das Gateway speichert die Einträge in Valkey, verschlüsselt mit dem Hauptschlüssel und nur mit dem API-Schlüssel lesbar, der die Anfrage gestellt hat, für `KAESO_CONTINUITY_REASONING_TTL` (standardmäßig eine Stunde).
3. Bei der nächsten Anfrage der App, nach dem Dekodieren und vor `check` und dem Kodieren, listet `continuity::tool_calls_missing_reasoning` die nachzuschlagenden Tool-Aufruf-IDs auf. `continuity::restore_reasoning` setzt das Gefundene dann wieder in die Assistentenrunden ein. Reasoning für den ersten Tool-Aufruf einer Runde kommt an den Anfang der Runde, Reasoning für einen späteren Aufruf direkt hinter den vorherigen Aufruf. Reasoning wird nur in eine Runde der App zurückgesetzt, die genau die Tool-Aufrufe der Antwort enthält, aus der es stammt (`CarriedReasoning::turn_calls`), denn Anthropic bindet Denken an die Runde, die es erzeugt hat; eine App, die zwei Antworten zu einer Runde zusammengefasst hat, erhält sie ohne. Runden, die bereits Reasoning enthalten, bleiben unverändert.

Die Encoder spielen wiederhergestellte Blöcke nur an ihren eigenen Anbieter zurück. Für Anthropic bittet eine Anfrage, die sie zurückspielt, Anthropic, einen Block zu verwerfen, dessen Unterhaltung sich geändert hat, statt die Anfrage abzulehnen ([Anthropic Messages](anthropic-messages.md)).
