---
title: Formatwechsel
description: Die Regeln, denen die Codecs folgen, wenn eine Anfrage oder Antwort das API-Format wechselt, und ihre Gründe.
order: 1
---

Eine App kann ein API-Format sprechen, während der Anbieter ein anderes spricht. Für diesen Fall gelten die folgenden Regeln der Codecs, jeweils mit ihrem Grund. Eine Anfrage, die in ihrem eigenen Format bleibt, wird exakt reproduziert, und keine dieser Regeln greift.

## Anfragen

`cross::check_foreign` durchläuft eine Anfrage und lehnt mit `Unsupported` ab, was das Ziel nicht verwenden kann:

- Extras-Felder aus einem anderen Dialekt, außer das Feld steht auf der [Allowlist](extras-allowlist.md) oder darf aufgrund seines Werts das Format wechseln (`cross::is_droppable_value`); Extras, die Felder, aber keinen Ursprung haben, werden stattdessen mit `InvalidRequest` abgelehnt, weil niemand sie zurückschreiben könnte;
- unbekannte Blöcke, Tools und Tool-Auswahlen aus einem anderen Dialekt; ein unbekannter Block ohne `type`, aber mit `role` enthält eine ganze Nachricht, etwa eine `function`-Nachricht von OpenAI Chat, und wird `` `<role>` message `` genannt, mit dem Hinweis „Remove the message, or send the request to a model served through its original format.“;
- Datei-IDs eines Anbieters, die der Zielanbieter nicht auflösen kann ([Dateien](#dateien));
- Zitate aus einem anderen Dialekt: Kein Ziel dokumentiert, was Zitate in früheren Runden bewirken, und die Formate lassen sich nicht aufeinander abbilden.

Das veraltete Function Calling von OpenAI Chat übersteht den Round Trip innerhalb von Chat und wechselt in keinen anderen Dialekt. Legacy-Aufrufe haben keine Aufruf-IDs, daher sind sie keine KPP-Tool-Aufrufe: `functions` und `function_call` auf oberster Ebene sind Extras der Anfrage (`` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``), das `function_call` einer Assistentennachricht ist ein Extra der Nachricht und wird als `messages[i].function_call` abgelehnt, und eine `function`-Nachricht wird zu einer Benutzernachricht mit einem unbekannten Block und als `` `function` message `` abgelehnt.

Nicht abgelehnt wird, was die Encoder stattdessen weglassen: Reasoning-Blöcke, die das Ziel nicht zurückspielen kann ([Reasoning](#reasoning)), und Cache-Hinweise, wo das Ziel selbst cacht ([Cache-Hinweise](#cache-hinweise)).

`check()` lehnt auch ab, was ein Ziel überhaupt nicht ausdrücken kann. Anthropic braucht `max_tokens`, daher ist eine Generierungsanfrage ohne `sampling.max_output_tokens` dort `InvalidRequest` und nennt `max_tokens`, statt einen erfundenen Wert zu bekommen.

## Antworten

Antworten fließen von einem Anbieter zu einer App, und eine fertige, bereits bezahlte Antwort abzulehnen wäre schlimmer, als weniger davon zu zeigen. Daten eines anderen Dialekts werden daher in dieser Rangfolge behandelt (`cross::reply_policy`, `cross::reasoning_reply_policy`):

1. **abgebildet**, wo der Dialekt der App sie darstellen kann: Reasoning eines anderen Dialekts wird zu einem Reasoning-Item von OpenAI Responses oder einem Anthropic-`thinking`-Block, mit dem Originalblock verpackt in einem [Reasoning-Blob](#reasoning-blobs), den die App unverändert zurücksendet;
2. **weggelassen**, wo er das nicht kann, und in Streams durch [Keepalive](#keepalives)-Frames ersetzt, damit die Verbindung nie verstummt: Reasoning für Apps von OpenAI Chat (das Gateway [bewahrt es](#vom-gateway-aufbewahrtes-reasoning) zwischen den Runden auf), unbekannte Blöcke und Stream-Extras aus einem anderen Dialekt, fremde Zitate und Extras.

Das Gateway protokolliert die KPP-Antwort vollständig, daher geht in Kaesos Aufzeichnungen nichts verloren. Unbekannte Blöcke, Zitate und Extras erreichen einen anderen Dialekt nur, wenn der Anbieter etwas sendet, worum die Anfrage nicht gebeten hat, weil die Anfrageseite fremde Server-Tools, Zitate und ähnliche Funktionen ablehnt.

Stoppgründe, die das Format der App nicht kennt, werden auf den nächstliegenden abgebildet, den es kennt; jede [Mapping-Seite](index.md) listet sie auf.

## Extras

Extras aus Dialekt A werden nur in Dialekt A zurückgeschrieben. Geht eine Anfrage mit Extras an einen anderen Dialekt, gibt `check()` des Ziels `Unsupported` zurück und nennt das Feld, außer das Feld steht auf der [Extras-Allowlist](extras-allowlist.md): Dann wird es stillschweigend weggelassen. Die Allowlist enthält nur Felder, von denen bekannt ist, dass sie keinen Einfluss darauf haben, was das Modell schreibt, etwa Beta-Header und Hinweise zur Auflösung, jeweils mit dem echten Client, der sie sendet.

## Nutzung

`Usage::input_tokens` schließt Cache-Lesevorgänge und Cache-Schreibvorgänge aus, die eigene Felder sind. Das entspricht den Preisspalten von models.dev, sodass sich die Kosten als Summe aus jeder Anzahl mal ihrem Preis ergeben. Das rohe Nutzungsobjekt des Anbieters bleibt immer erhalten.

## Systemnachrichten

Eine Systemnachricht nach dem Beginn einer Unterhaltung bleibt als `Role::System`-Nachricht erhalten.

- An Anthropic werden die Systemnachrichten einer aus Anthropic Messages dekodierten Anfrage unverändert gesendet, und die API beurteilt ihre Position und welche Modelle sie unterstützen. Claude Code sendet solche Nachrichten bei jeder Anfrage.
- Systemnachrichten aus anderen Dialekten sind für Anthropic `Unsupported`: Anthropic erlaubt sie nur für einige Modelle und nur nach einem Benutzerzug, während System- und Developer-Nachrichten von OpenAI an beliebiger Stelle stehen dürfen.
- An OpenAI werden sie zu späteren `system`- oder `developer`-Nachrichten.

## Reasoning

Anthropic signiert seine Thinking-Blöcke, und OpenAI verschlüsselt seine Reasoning-Items. Opakes Reasoning (Signaturen, verschlüsselter Inhalt, Item-IDs) wird immer nur an den Dialekt und den Anbieter zurückgespielt, die es ausgegeben haben.

- **In Anfragen** werden lesbarer Reasoning-Text und Zusammenfassungen, die an einen anderen Dialekt gehen, als Hinweis weggelassen; der [Reasoning-Blob](#reasoning-blobs) daneben stellt den Originalblock wieder her, wenn die Unterhaltung zu ihrem Anbieter zurückkehrt.
- **Ohne Signatur** geht ein Anthropic-`thinking`-Block, der aus Anthropic Messages gelesen wurde, so an Anthropic zurück, wie er ankam, in Anfragen und Antworten: Eine fehlende `signature` bleibt fehlend, und `signature: ""` bleibt `""`. Unsigniertes Anthropic-Reasoning, das nicht aus Anthropic Messages gelesen wurde, etwa ein von Code gebauter Block, hat nichts zum Zurückspielen und wird weggelassen.
- **In Antworten** wird Reasoning eines anderen Dialekts in der Reasoning-Form der App gezeigt, wo diese eine hat, mit dem Originalblock in einem Reasoning-Blob, sodass es in der nächsten Runde wieder seinen Ursprung erreicht. Apps von OpenAI Chat haben keine solche Form, daher bewahrt das Gateway ihr Reasoning zwischen den Runden auf.
- **Reasoning-Einstellungen** werden zwischen einer Aufwandsstufe und einem Token-Budget umgerechnet:

| Von | Nach | Wird zu |
| --- | --- | --- |
| Aufwand `none` | Anthropic | Kein Thinking, der Standard bei Anthropic |
| Aufwand `low` bis `max` | Anthropic-Modell mit adaptivem Thinking | Adaptives Thinking mit diesem Aufwand |
| Aufwand `low` bis `max` | Anthropic-Modell ohne adaptives Thinking | `enabled`-Thinking mit einem Budget aus der Tabelle unten |
| Nur ein Budget | Anthropic | `enabled`-Thinking mit diesem Budget |
| Ein Anthropic-Budget | OpenAI | Ein Aufwand aus der Tabelle unten |
| Anthropic `thinking: {type: "disabled"}` | OpenAI | Aufwand `none` |

Beide OpenAI-Dialekte dokumentieren `none` als niedrigsten Wert ihres gemeinsamen Enums `ReasoningEffort`, daher hat abgeschaltetes Thinking von Anthropic immer eine Entsprechung. Die Aufwandsstufe `minimal` hat keine Zeile und bleibt für Anthropic `Unsupported`.

### Aufwand und Budget

Die Umrechnung steckt in `kaeso_codecs::cross::reasoning_map`.

| Anthropic `budget_tokens` | OpenAI-Aufwand |
| --- | --- |
| unter 4.096 | `low` |
| 4.096 bis 16.383 | `medium` |
| 16.384 und mehr | `high` |

Ein ausdrücklicher Aufwand in derselben Anfrage hat Vorrang vor dem Budget.

| OpenAI-Aufwand | Anthropic `budget_tokens` |
| --- | --- |
| `low` | 2.048 |
| `medium` | 8.192 |
| `high`, `xhigh`, `max` | 24.576 |

Diese Tabelle gilt nur, wenn das Zielmodell kein adaptives Thinking hat (Claude 4.5 und älter), was das Gateway mit `EgressCtx::adaptive_thinking = Some(false)` aus dem Feld `adaptive_thinking` des Katalogs angibt; Claude 4.7 und neuer lehnen `enabled`-Thinking ab und bekommen stattdessen adaptives Thinking mit dem Aufwand. Das Budget wird dann an die Regeln von Anthropic angepasst: mindestens 1.024 und kleiner als `max_tokens`, also `max_tokens − 1`, wenn das kleiner ist. Ist `max_tokens` 1.024 oder kleiner, gibt es kein gültiges Budget, daher wird `max_tokens` um 1.024 erhöht und das Budget ist 1.024: Die App behält ihr eigenes `max_tokens` für die Antwort. `reasoning_map::budget_within` übernimmt Anpassen und Erhöhen, `reasoning_map::fit_budget` nur das Anpassen.

Die Quellen sind das Enum `ReasoningEffort` von OpenAI in der OpenAPI-Spezifikation (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max`; nicht jedes Modell unterstützt jeden Wert) und die Seite [Extended Thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) von Anthropic: `budget_tokens` ist mindestens 1.024 und kleiner als `max_tokens`, und adaptives Thinking gibt es nicht bei Claude Sonnet 4.5, Opus 4.5, Haiku 4.5 und früheren Claude-4-Modellen.

### Lesbares Reasoning und Zusammenfassungen

`reasoning.summary` und `reasoning.include_encrypted` sind über Dialekte hinweg Hinweise: Sie steuern nur, welches Reasoning zurückkommt, und werden daher weggelassen, wo das Ziel keine Entsprechung hat. Codex sendet beide bei jeder Anfrage.

OpenAI liefert lesbares Reasoning nur, wenn die Anfrage `reasoning.summary` setzt, und Claude Opus 4.7 und neuer liefern Thinking-Blöcke mit leerem Text, außer die Anfrage setzt `thinking.display: "summarized"`. Daher gilt:

- Eine Anfrage aus einem anderen Dialekt mit Reasoning-Zusammenfassung (`auto`, `concise`, `detailed`) und eingeschaltetem Thinking geht mit `display: "summarized"` an Anthropic, außer bei der Token-Zählung.
- Eine Anfrage im Format Anthropic Messages mit eingeschaltetem Thinking (`enabled`, `adaptive` oder ein Aufwand außer `none`) und einem `display` außer `omitted` geht mit `summary: "auto"` an OpenAI Responses, außer bei der Token-Zählung.

Das Gateway teilt den Encodern außerdem mit, ob das Zielmodell Reasoning beherrscht (`EgressCtx::reasoning`, aus dem Feld `reasoning` des Katalogs). Bei `Some(true)` schaltet eine Zusammenfassung ohne Aufwand, Modus oder Budget (OpenAI `reasoning: {summary: "auto"}`) das Thinking von Anthropic in der Standardtiefe des Modells ein: adaptiv ohne Aufwand oder mit dem Budget für `medium`, dem Standardaufwand von OpenAI. Ebenfalls bei `Some(true)` zählt Anthropic-Thinking `adaptive` ohne Aufwand für OpenAI Responses als Reasoning: Es fordert `include: ["reasoning.encrypted_content"]` an und, wie oben, eine Zusammenfassung. Anfragen ohne Reasoning-Einstellungen, der Aufwand `none` und Modelle, die kein Reasoning beherrschen oder unbekannt sind, bekommen nie Reasoning eingeschaltet. Apps von OpenAI Chat, deren Format kein Reasoning zeigt, fordern keine Zusammenfassungen an.

### Thinking und Sampling

Schaltet Kaeso das Thinking für eine Anfrage aus einem anderen Dialekt ein, wird das, was Anthropic neben Thinking verbietet, angepasst statt abgelehnt, weil Sampling-Einstellungen und erzwungene Tools in OpenAI-Apps alltäglich sind:

- `temperature`, `top_p` und `top_k` werden weggelassen. Anthropic lehnt `temperature` und `top_k` mit Thinking ab und begrenzt `top_p` auf 0,95 bis 1, und Claude Opus 4.7 und neuer lehnen jeden Wert ab, der nicht dem Standard entspricht.
- `enabled`-Thinking wird nicht mit einem erzwungenen Tool (`tool_choice` `required` oder ein benanntes Tool) gesendet, einer Kombination, die Anthropic ablehnt. Wo Anthropic erzwungene Tools akzeptiert, bei adaptivem Thinking, beginnt die Antwort mit dem Tool-Aufruf und enthält ohnehin kein Thinking, daher geht nichts verloren. Ein erzwungenes Tool mit adaptivem Thinking wird unverändert gesendet.

Die eigenen Einstellungen einer Anthropic-App werden so gesendet, wie sie ankamen.

### Tool-Runden ohne Thinking

Anthropic lehnt eine Anfrage mit `enabled`-Thinking ab, deren letzter Assistentenzug Tool-Aufrufe ohne vorangehenden, zurückspielbaren Thinking-Block gemacht hat, einen signierten oder einen redigierten („a final assistant message must start with a thinking block“). Eine solche Anfrage geht für diese Runde ohne `thinking` an Anthropic. Adaptives Thinking bleibt erhalten, weil adaptive Modelle den führenden Thinking-Block nicht brauchen.

Da Reasoning mit der Unterhaltung mitreist, ist das selten. Es kommt trotzdem vor, wenn die Runde wirklich von einem anderen Anbieter stammt (ein Modellwechsel mitten in einer Tool-Schleife), wenn eine App das Reasoning-Item, den Thinking-Block oder den Blob weggelassen hat oder wenn das Gateway das Reasoning einer Chat-App nicht mehr hat, weil es abgelaufen ist oder ein anderer API-Schlüssel die Runde gesendet hat.

## Reasoning über Runden hinweg

Ohne Hilfe würde jede Runde einer Tool-Schleife, in der eine App das Modell eines anderen Anbieters nutzt, das Reasoning dieses Modells verlieren: Budget-Thinking würde in jeder Folgerunde abgeschaltet, und adaptives Thinking würde verlieren, was es zuvor überlegt hat. `kaeso_codecs::continuity` lässt Reasoning mit der Unterhaltung mitreisen.

### Reasoning-Blobs

Für Apps von OpenAI Responses und Anthropic Messages verpackt KPP den ganzen Reasoning-Block eines anderen Dialekts (lesbarer Text, Zusammenfassungen, Signatur, verschlüsselter Inhalt, redigierte Daten, Item-ID) in einen Reasoning-Blob: `continuity::REASONING_BLOB_PREFIX` (`kpp:reasoning:v1:`), gefolgt vom KPP-JSON des Blocks ohne Form-Markierungen, in base64url ohne Padding.

- Apps von OpenAI Responses bekommen ein Reasoning-Item mit einer ID `rs_kaeso_…`, das Thinking als einen einzigen Zusammenfassungstext und den Blob in `encrypted_content`; Streams tragen es als `response.reasoning_summary_*`-Events.
- Apps von Anthropic Messages bekommen einen `thinking`-Block mit den Zusammenfassungen und dem Reasoning-Text als Thinking, Absätze durch eine Leerzeile getrennt, und dem Blob als `signature`, gestreamt als ein einziges `signature_delta`, wenn der Block stoppt.

Beide Anfrage-Decoder erkennen einen Blob, wo immer er steht (`continuity::unwrap_reasoning`), und stellen den Originalblock mit seinem Ursprung wieder her, sodass die Encoder ihn wie jedes andere Reasoning an seinen eigenen Dialekt und Anbieter zurückspielen.

Der Blob wird gesendet, ob die App `include: ["reasoning.encrypted_content"]` angefordert hat oder nicht, und unabhängig von ihrer Einstellung für `store`: Kaeso speichert nichts, und die Agent-Schleifen der SDKs (`to_input_list()` von openai-agents, Codex, Claude Code, der Tool Runner von Anthropic) senden Ausgabe-Items und Content-Blöcke so zurück, wie sie ankamen. Der Blob enthält den lesbaren Text ein zweites Mal, weil er den Block exakt wiederherstellen muss (Anthropic prüft den Thinking-Text gegen seine Signatur), während Apps die lesbare Kopie bearbeiten oder weglassen können; sendet eine App beides zurück, hat der Blob Vorrang.

Das Präfix enthält `:`, das weder im base64-Alphabet der Anthropic-Signaturen noch im base64url-Alphabet des verschlüsselten Inhalts von OpenAI vorkommt, daher wird ein Blob nie mit den eigenen Daten der Anbieter verwechselt. Ein Wert mit dem Präfix, der sich nicht dekodieren lässt, bleibt als opake Daten des Anbieters erhalten, und der Anbieter lehnt ihn dann ab, sodass nichts stillschweigend wegfällt. Kaeso verschlüsselt oder signiert den Blob nicht: Er enthält, was die App schon gesehen hat, und Daten, die nur der ausgebende Anbieter prüfen kann.

Anfragen aus anderen Dialekten fordern bei OpenAI Responses `include: ["reasoning.encrypted_content"]` an, wenn sie einen Aufwand außer `none` oder ein Thinking-Budget setzen oder das Thinking für ein Modell einschalten, von dem das Gateway weiß, dass es Reasoning beherrscht ([Lesbares Reasoning und Zusammenfassungen](#lesbares-reasoning-und-zusammenfassungen)); sonst kann Kaeso nicht erkennen, ob das Modell Reasoning beherrscht, und OpenAI lehnt verschlüsseltes Reasoning für Modelle ab, die keines beherrschen. Da diese Anfragen `store: false` tragen ([Speicherung und IDs bei Responses](#speicherung-und-ids-bei-responses)), wird Reasoning ohne verschlüsselten Inhalt nicht an OpenAI gesendet, das seine Item-ID nicht auflösen könnte.

### Vom Gateway aufbewahrtes Reasoning

Chat Completions hat kein Feld für Reasoning, und KPP erfindet keines. Die Antwort lässt das Reasoning weg, und das Gateway bewahrt es auf, mit den Tool-Aufrufen als Schlüssel, denen es vorausging; Tool-Call-IDs sind eindeutig, werden vom Anbieter vergeben und von jedem Chat-Client zurückgesendet, daher eignen sie sich als Schlüssel für das Reasoning, ohne das Format der App zu ändern.

1. Nach einer Antwort liefern `continuity::reasoning_for_tool_calls` (bei einer vollständigen Antwort) oder ein `continuity::ReasoningCollector` (bei einem Stream) ein `CarriedReasoning` pro Tool-Aufruf, dem zurückspielbares Reasoning vorausging: signiertes oder redigiertes Thinking von Anthropic, verschlüsseltes Reasoning von OpenAI. Reasoning nach dem letzten Tool-Aufruf wird nicht aufbewahrt.
2. Das Gateway speichert die Einträge in Valkey, verschlüsselt mit dem Hauptschlüssel und nur mit dem API-Schlüssel lesbar, der die Anfrage gestellt hat, für `KAESO_CONTINUITY_REASONING_TTL` (standardmäßig eine Stunde).
3. Bei der nächsten Anfrage der App, nach dem Dekodieren und vor `check` und dem Kodieren, listet `continuity::tool_calls_missing_reasoning` die nachzuschlagenden Tool-Call-IDs auf, und `continuity::restore_reasoning` setzt das Gefundene wieder in die Assistentenzüge ein: Reasoning für den ersten Tool-Aufruf eines Zugs an den Anfang des Zugs, für einen späteren Aufruf direkt hinter den vorherigen Aufruf. Züge, die bereits Reasoning enthalten, bleiben unverändert.

Die Encoder spielen wiederhergestellte Blöcke nur an ihren eigenen Anbieter zurück.

## Keepalives

Ein Anbieter kann minutenlang streamen, ohne etwas zu erzeugen, das das Format der App zeigen kann: Claudes Thinking für eine App von OpenAI Chat, `ping`-Events von Anthropic, unbekannte Blöcke eines anderen Dialekts oder ein Reasoning-Modell von OpenAI, das während des Reasonings nichts sendet. Proxys mit Leerlauf-Timeouts (nginx und AWS ALB nach 60 Sekunden, Cloudflare nach 100) würden die Verbindung trennen, und die Zeit bis zum ersten Byte entspräche der gesamten Denkzeit. Daher verstummt ein laufender Stream nie wegen der Übersetzung: Jeder Stream-Encoder hat einen Keepalive-Frame, der in seinem Dialekt gültig ist und von den offiziellen SDKs ignoriert wird.

- **OpenAI Chat Completions und Responses:** der SSE-Kommentar `: keepalive`, gefolgt von einer Leerzeile. Zeilen, die mit einem Doppelpunkt beginnen, sind in der SSE-Spezifikation Kommentare, und die Parser der OpenAI-SDKs überspringen sie.
- **Anthropic Messages:** `event: ping` mit `data: {"type": "ping"}`, genau wie Anthropic es sendet; die Anthropic-SDKs überspringen Ping-Events.

Frames gelangen auf zwei Wegen auf die Leitung:

- **Im Stream.** Ein Encoder hängt den Frame an das an, was `StreamEncoder::encode` zurückgibt, wenn ein Event aus einem anderen Dialekt keine Bytes erzeugt hat. Ein Taktgeber begrenzt das ohne Uhr (`kaeso_codecs::keepalive`): Ein Extra-Event eines anderen Dialekts wie `ping`, das der Anbieter bereits taktet, bringt immer einen Frame ein; verborgener Text (Thinking, das die App nicht sehen kann, gepufferte Tool-Argumente) bringt einen Frame pro `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` Bytes ein, also 1.024; alles, was der Encoder schreibt, setzt die Zählung zurück. Events aus dem eigenen Dialekt der App bringen nie Frames ein, sodass eine Wiedergabe im selben Dialekt exakt bleibt.
- **Vom Gateway gesteuert.** `StreamEncoder::keepalive` liefert den Frame auf Abruf, für Stille beim Anbieter, die der Encoder nicht sehen kann. Das Gateway schreibt ihn, wenn es `KAESO_STREAMS_KEEPALIVE_INTERVAL` lang nichts an die App geschrieben hat, standardmäßig 15 Sekunden.

Der Encoder für OpenAI Chat sendet seinen Rollen-Chunk außerdem, sobald die Antwort beginnt, statt mit dem ersten Text, sodass die App vom Stream hört, während das Modell denkt.

## Strukturierte Ausgabe

Strukturierte Ausgabe an Anthropic nutzt die nativen Structured Outputs von Anthropic, nie eine Nachbildung über ein erzwungenes Tool. Das Ausgabeformat `json_object` hat bei Anthropic keine Entsprechung und ist `Unsupported`. Das Ausgabeformat von Anthropic ist nur ein Schema: Der `name` eines Schemas wird weggelassen, während eine `description` oder `strict: false` `Unsupported` sind. Schemas, die von Anthropic an OpenAI Chat gehen, bekommen den Namen `response`, weil Chat einen verlangt.

## Sampling

Sampling-Einstellungen, die ein Ziel nicht einhalten kann, sind `Unsupported` und werden nie weggelassen oder umskaliert:

| Ziel | Abgelehnt |
| --- | --- |
| Anthropic | `seed`, Frequency- und Presence-Penalties und eine `temperature` außerhalb von 0 bis 1, dem Bereich bei Anthropic |
| OpenAI Chat Completions | `top_k` und mehr als vier Stoppsequenzen |
| OpenAI Responses | `top_k`, `seed`, die Penalties und Stoppsequenzen, die es bei Responses nicht gibt |

Zahlen behalten ihre Schreibweise innerhalb eines Dialekts: `temperature: 1` kommt als `1` zurück und `1.0` als `1.0`, solange der Wert unverändert ist. Über Dialekte hinweg gilt die eigene Schreibweise des Ziels, daher erreicht `temperature: 1` aus Chat Anthropic als `1.0`.

KPP hat kein `n`: Eine Chat-Anfrage mit `n` über 1 wird schon beim Dekodieren abgelehnt, unabhängig vom Ziel. `logprobs`, `logit_bias` und `top_logprobs` von Chat sind Extras, die jeder andere Dialekt ablehnt ([Extras](#extras)).

## Zustandsbehaftete Responses-Funktionen

`previous_response_id`, `conversation`, `store: true` und `background: true` brauchen Zustand bei OpenAI, und der Codec lehnt sie mit `Unsupported` ab; das Gateway, nicht der Codec, müsste sie vor KPP auflösen.

## Speicherung und IDs bei Responses

- Anfragen aus anderen Dialekten, die für OpenAI Responses kodiert werden, tragen `"store": false`. Kaeso ist zustandslos, und ohne dieses Feld würde OpenAI jede übersetzte Unterhaltung speichern. Aus Responses dekodierte Anfragen behalten die Schreibweise der App, daher bedeutet ein fehlendes `store` weiterhin, dass OpenAI die Antwort speichert.
- Ausgabe-Items von Responses brauchen IDs, die Antworten aus anderen Dialekten nicht haben. Kaeso gibt ihnen IDs, die mit `msg_kaeso_`, `fc_kaeso_` und `rs_kaeso_` beginnen; sendet eine App sie zurück, entfernt der Anfrage-Decoder sie, sodass OpenAI nie eine ID erhält, die es nicht selbst vergeben hat.

## Cache-Hinweise

Cache-Hinweise sind Hinweise. OpenAI cacht selbst, daher werden Hinweise aus anderen Dialekten für OpenAI weggelassen. OpenAIs eigenes `prompt_cache_breakpoint` wird zu einem KPP-`CacheHint` dekodiert und nur in den Dialekt zurückgeschrieben, aus dem es kam.

## Tool-Fehler

`ToolResult.is_error` ist ein Hinweis für Ziele ohne Fehler-Flag, also OpenAI Chat und Responses: Es wird weggelassen, und der Inhalt des Ergebnisses trägt den Fehlertext. Würde es abgelehnt, ließe sich kein fehlgeschlagener Tool-Aufruf eines Clients im Anthropic-Format wie Claude Code mehr an OpenAI routen.

## Dateien

Eine Datei-ID eines Anbieters (`file-…`) ist im Dateispeicher dieses Anbieters nutzbar, und die beiden OpenAI-Dialekte für Generierung teilen sich einen, daher nimmt `cross::check_foreign` den `Origin` des Ziels entgegen, also seinen Dialekt und Anbieter. Datei-IDs, die zu einem anderen Anbieter wechseln, sind `Unsupported`.

## Die Anthropic-Version

Der Anthropic-Codec legt eine API-Version fest, `2023-06-01`, exportiert als `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`. Es ist die einzige Version, die die [Seite zur Versionierung](https://platform.claude.com/docs/en/api/versioning) von Anthropic auflistet, die Version, die die offiziellen SDKs senden, und die, von der jede Anthropic-Fixture ausgeht.

- **Egress.** Jede Anfrage, die der Anthropic-Codec kodiert, trägt `anthropic-version: 2023-06-01`, außer die eigene Anthropic-Anfrage der App hat ein `anthropic-version` weitergegeben, das eine neuere Version nennt: ein wohlgeformtes `YYYY-MM-DD`, das nach der festgelegten Version einsortiert wird. Ein fehlerhafter oder älterer Wert wird ignoriert. Eine Anfrage aus einem anderen Dialekt trägt den Header nie und bekommt daher immer die festgelegte Version.
- **Ingress.** Ein weitergegebenes `anthropic-version`, das der festgelegten Version entspricht, wird nicht in den Extras der Anfrage gespeichert: Der Codec sendet es ohnehin, und es aufzubewahren würde jede Anfrage eines Anthropic-SDKs an ihren Dialekt binden. Jeder andere Wert wird als Extra `header:anthropic-version` aufbewahrt und beim Egress wie oben verglichen.
- **Warum die neuere Version gewinnt.** Anthropic ergänzt eine Version nur, mit neuen optionalen Eingaben und neuen Ausgabewerten, und eine App, die eine neuere Version anfordert, verlässt sich auf deren Verhalten. Die Decoder tolerieren unbekannte Felder, Blöcke und Events, daher führt eine neuere Version zu Extras und unbekannten Items, statt zu scheitern.

Um die festgelegte Version zu ändern, die Konstante ändern, die Fixtures mit `cargo xtask record` neu aufzeichnen ([Aufzeichnen](fixtures.md#live-antworten-aufzeichnen)) und diesen Abschnitt aktualisieren.
