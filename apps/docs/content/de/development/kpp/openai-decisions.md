---
title: OpenAI Decisions
description: Wie jedes Feld der Decisions-API von OpenAI auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld der Decisions-API von OpenAI nach: wohin es in KPP gehört, was aus einer Anfrage eines anderen Entscheidungsformats wird und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::openai_decisions::OpenAiDecisions` (`crates/kaeso-codecs/src/openai_decisions/`)
- **Endpunkte:** `POST /v1/decisions` und die [Modell-Endpunkte](#modelle) von OpenAI
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_decisions/`, aus der API-Referenz und den Typen von `openai-python` gebaut, keine aufgezeichnet

**Quellen.** Die Referenz [Create a decision](https://developers.openai.com/api/reference/resources/decisions/methods/create) von OpenAI, der [Leitfaden zu Decisions](https://developers.openai.com/api/docs/guides/decisions) und die [Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes), gelesen am 2026-11-12. `openai-python` 3.49.0: `types/decision_create_params.py` und `types/decision.py`.

Das Gateway leitet Entscheidungsanfragen noch nicht weiter; der Codec ist dafür bereit.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_decisions`; ein anderer Dialekt lehnt sie ab, außer sie stehen auf der [Allowlist](cross-format.md#allowlist).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage: `DecisionCreateParams`

Eine Entscheidungsanfrage wird zu `Request::Decide(DecideRequest)` dekodiert. Ihre Fragen behalten ihre Reihenfolge, und `answers[i]` der Antwort beantwortet `questions[i]`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `DecideRequest.model` typisiert | Erforderlich. |
| `input` String | `input = DecisionInput::Text` typisiert | |
| `input` Liste | `input = DecisionInput::Messages` typisiert | Eine `Message` je Element ([Eingabeelemente](#eingabeelemente)). |
| `questions` | `questions` typisiert | Eine Liste; alles andere ist `InvalidRequest`. |
| `questions[].type` | `Question.kind` typisiert | `predicate`, `choice` oder `score`. Ein anderer Typ ist `InvalidRequest`. |
| `questions[].name` | `Question.name` typisiert | Optional. Namen dürfen sich wiederholen; `validate()` erlaubt das. |
| `questions[].instructions` String | `Instructions::Text` typisiert | |
| `questions[].instructions` Liste | `Instructions::Blocks` typisiert | Teile `input_text`, `input_image` und `input_json` ([Teile](#inhaltsteile)). |
| `choices[].value` | `ChoiceOption.value` typisiert | Ein String ist `ChoiceValue::Text`, ein boolescher Wert `ChoiceValue::Bool`. Alles andere ist `InvalidRequest`. |
| `choices[].description` | `ChoiceOption.description` typisiert | `Description::Text`. |
| `levels[].label` | `Level.label` typisiert | Stufen stehen aufsteigend; Stufe `i` zählt `i`. |
| `levels[].description` | `Level.description` typisiert | `Description::Text`. |
| `prompt_cache_key` | `DecideRequest.cache.key` typisiert | |
| `prompt_cache_options` mit `mode` und `ttl` | `cache.mode`, `cache.ttl` typisiert | Die Form `cache_options` bewahrt ein leeres Objekt. Jede andere Gestalt bleibt ganz in den Extras, ein Hinweis. |
| `safety_identifier` | `DecideRequest.user` typisiert | |
| unbekannte Schlüssel | Extras | Auf jeder Ebene: Anfrage, Frage, Option, Stufe, Element und Teil. |
| Header `openai-beta` | Extras `header:openai-beta` | Nur an OpenAI Decisions weitergegeben. |

Ein `predicate` mit `choices` oder `levels`, ein `choice` ohne `choices` oder mit `levels` und ein `score` ohne `levels` oder mit `choices` sind `InvalidRequest`. Ein Body mit `state` und ohne `input` ist eine System-One-Anfrage. Er ist `InvalidRequest` mit einer Meldung, die `/v1/systemone` nennt (D15).

### Eingabeelemente

| Element | KPP | Anmerkungen |
|---|---|---|
| Nachricht mit `role: "user"` | `Message` mit `Role::User` | `type: "message"` ist Form `typed`; ein String als Inhalt ist ein Textblock, Form `content_string`. |
| Nachricht mit `role: "developer"` | `Message` mit `Role::System` | Die Form `role` bewahrt `developer`. Nur Textteile. |
| Nachricht mit `role: "system"` | `Message` mit `Role::System` | Nur Textteile. |
| Nachricht mit `role: "assistant"` | `Message` mit `Role::Assistant` | Teile `output_text` und `refusal`; `phase` bleibt in den Extras. |
| `function_call` | eine Assistentennachricht mit einem `Block::ToolCall` | `call_id`, `name`; `arguments` wird als JSON-String gehalten. `id`, `namespace` und `status` bleiben in den Extras. |
| `custom_tool_call` | eine Assistentennachricht mit einem `Block::ToolCall` | Form `custom`; `input` wird als JSON-String gehalten. |
| `function_call_output` | eine Benutzernachricht mit einem `Block::ToolResult` | `output` als Text oder als Text- und Bildteile; ein String ist Form `output_string`. |
| `custom_tool_call_output` | eine Benutzernachricht mit einem `Block::ToolResult` | Form `custom`. |
| ein anderer `type` | keins | `InvalidRequest`: Dateien, Audio und Elementreferenzen gehören nicht zur API. |

### Inhaltsteile

| Teil | KPP | Anmerkungen |
|---|---|---|
| `input_text`, `output_text` | `Block::Text` | Ein Typ, der nicht zu seiner Rolle passt, etwa `input_text` in einer Assistentennachricht, ist Form `text_type`. |
| `input_image` mit einer Data-URL | `Block::Image`, `Source::Base64` | Die Bytes werden dekodiert; ein anderer Kopf als `data:<type>;base64` ist Form `data_url_header`. Base64, das sich nicht dekodieren lässt, ist `InvalidRequest`. |
| `input_image` mit einer `http(s)`-URL | `Block::Image`, `Source::Url` | Kaeso ruft sie nie ab. |
| `input_image.detail` | Extras | Ein Hinweis, weggelassen, wenn die Anfrage das Format wechselt. |
| `input_json` | `Block::Json` | Byte für Byte erhalten. |
| `refusal` | `Block::Refusal` | In Assistentennachrichten. |
| `prompt_cache_breakpoint` | `Block.cache` | Ein `CacheHint`, der den Breakpoint wie geschrieben hält, Form `breakpoint`. |
| ein anderer `type` | `Block::Unknown` | An `openai_decisions` gebunden; ein anderes Format lehnt ihn ab. |

## Anfragen schreiben

Der Encoder schreibt, was KPP hält, und lehnt vor dem Senden ab, was das Format von OpenAI nicht annimmt. Eine Anfrage eines anderen Formats, etwa System One, wechselt nach diesen Regeln; [Formate wechseln](cross-format.md#entscheidungen) nennt die Gründe.

| KPP | Geschrieben als | Anmerkungen |
|---|---|---|
| `DecisionInput::Text` | der String `input` | |
| `DecisionInput::Json` | eine Benutzernachricht mit einem Teil `input_json` | Nur ein Objekt oder eine Liste von Objekten, innerhalb von 32 Ebenen, 10.000 Werten und 256 KiB kompaktem JSON. Sonst ein Teil `input_text` mit dem JSON, wie geschrieben, in `adapted` als `state` genannt (D13). |
| `Instructions::Json` | das JSON, wie geschrieben, als String der Anweisungen | In `adapted` als `instructions` genannt. |
| fehlende `Instructions` | `""` | `instructions` ist erforderlich; eine Anfrage im eigenen Format lässt es weg. |
| `Predicate.when_true`, `.when_false` | Zeilen, die den Anweisungen angefügt werden | `It is true when: …` und `It is false when: …` nach einer Leerzeile. Anweisungen aus Teilen bekommen einen Textteil. In `adapted` als `criteria` genannt (D11). |
| `Description::Json` | sein JSON, wie geschrieben, als Text | In `adapted` als `criteria` genannt. |
| `Level` ohne Label | seine Beschreibung als `label` | Stufen von System One haben nur Beschreibungen. Eine leere Stufe ist `""`. |
| `ChoiceValue::Bool` | `true`, `false` | |
| eine Asset-Referenz in einem String | der String, um sie herum geteilt, das Bild an ihrer Stelle als Teil `input_image` | In der Eingabe, in einem String einer Zustandsliste und in Anweisungen. Eine Referenz anderswo, etwa in einem Objekt, ist `Unsupported` (`image reference`); in einer Beschreibung ist sie `image in criteria`. |
| `Source::Base64` | eine Data-URL | |
| `Source::Url` | die URL | Nur `http` und `https`; andere sind `Unsupported` (`image URL`). |
| `Source::FileId` | keins | `Unsupported` (`image file id`): senden Sie das Bild als Data-URL. |
| Audio-, Video- und Dokumentblöcke | keins | `Unsupported`, mit dem Teil benannt. |
| `QuestionKind::Box` | keins | `Unsupported` (`box question`). |
| eine Auswahl mit weniger als 2 oder mehr als 255 Optionen | keins | `Unsupported` (`options`). |
| eine Bewertung mit weniger als 2 oder mehr als 10 Stufen | keins | `Unsupported` (`levels`). |
| mehr Fragen als `DecisionsProfile::max_questions` | keins | `Unsupported` (`questions`); 400 bei OpenAI (D5). |
| `ToolResultBlock.is_error` | keins | `Unsupported` (`tool error`): das Format hat keine Fehlermarkierung. |

`EgressCtx` behält den Namen und die Optionswerte jeder gesendeten Frage, sodass die Antwort nach Position zurückgelesen wird.

## Antwort: `Decision`

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `DecideResponse.model` typisiert | |
| `answers[]` | `answers` typisiert | Eine je Frage, in Reihenfolge. Eine andere Anzahl, ein Typ, der nicht zur Frage passt, oder ein Wert, der nicht gefragt war, ist ein Anbieterfehler. |
| `answers[].name` | keins | Aus der Anfrage geschrieben. Eine Antwort, die die Frage anders nennt, behält ihren Namen in der Form `name`. |
| `predicate.probability` | `PredicateAnswer.probability` typisiert | |
| `choice.choice` | `ChoiceAnswer.option` typisiert | Der Index der Option mit diesem Wert; `true` und `"true"` bleiben getrennt. |
| `choice.probabilities[]` | `ChoiceAnswer.probabilities` typisiert | Nach Option; `None`, wo die Antwort keinen Eintrag hat. Eine andere Reihenfolge ist Form `order`. |
| `score.score` | `ScoreAnswer.score` typisiert | |
| `score.probabilities[]` | `ScoreAnswer.probabilities` typisiert | Nach `value`, dem Index der Stufe. Die Labels, wie geschrieben, sind Form `labels` und werden aus der Anfrage neu gebaut, wenn die Antwort das Format wechselt. |
| `confidence` | `Answer.confidence` typisiert | Siehe [Konfidenz](#konfidenz). |
| `refusal` | `AnswerValue::Refusal` typisiert | Andere Antworten derselben Antwort zählen weiter. |
| `usage.input_tokens` | `usage.input_tokens` typisiert | OpenAI zählt Cache-Lese- und Schreibvorgänge darin mit, KPP nicht, daher werden beide abgezogen. |
| `usage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens` typisiert | |
| `usage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens` typisiert | |
| `usage.output_tokens` | `usage.output_tokens` typisiert | 0 bei OpenAI. |
| `usage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens` typisiert | |
| `usage` | `usage.raw` | `total_tokens` und andere Schlüssel kommen im selben Format zurück. |
| als ganze Zahlen geschriebene Zahlen | Form `integers` | Wieder als ganze Zahlen geschrieben, solange der Wert unverändert ist. |
| unbekannte Schlüssel | Extras | Auf jeder Ebene, Wahrscheinlichkeitseinträge eingeschlossen. |

Der Encoder schreibt die Antworten in der Reihenfolge der Fragen und nimmt jeden Namen, jeden Optionswert und jedes Stufenlabel aus der Anfrage der App in `EncodeCtx::request`. Die Antwort auf eine unbenannte Frage hat `name: null`. Eine Antwort aus einem anderen Format bekommt `input_tokens` mit den Cache-Zahlen wieder addiert und `total_tokens` als Eingabe plus Ausgabe.

### Konfidenz

Eine Antwort eines Anbieters von OpenAI Decisions behält ihre `confidence`, wie gesendet. OpenAI dokumentiert nicht, wie es sie berechnet, aber beide Beispiele seines Leitfadens passen zu den Formeln von TypeSafe. Deshalb bekommt eine Antwort aus einem anderen Format diese: `(p_max − 1/n) / (1 − 1/n)` für eine Auswahl, `max(0, 1 − Σ p_i·|i − m| / MAD_unif)` für eine Bewertung (D8). Ein Prädikat hat keine.

## Streaming

Entscheidungen werden nicht gestreamt. Der Stream-Encoder gibt für jedes Event `Unsupported` (`streaming`) zurück; der Stream-Decoder meldet ein `StreamEvent::Error` mit `Unsupported` (`streaming`) und danach nichts mehr.

## Fehler

Fehler nutzen die Hülle von OpenAI, `{"error": {"message", "type", "param", "code"}}`, gelesen und geschrieben wie bei [OpenAI Responses](openai-responses.md#fehler-errorresponse). OpenAI dokumentiert keine eigenen Fehlercodes für Entscheidungen, daher folgen die Arten dem Status, dem `type` und dem `code`. Ein OpenAI-Fehler erreicht eine OpenAI-App, wie gesendet. Jeder andere Fehler wird in der Hülle mit dem Status seiner Art geschrieben, und ein 429 oder 503 behält sein `retry-after`.

## Modelle

Der Codec liest und schreibt die Modellliste von OpenAI, wie unter [Modelle](openai-chat.md#modelle) auf der Seite zu OpenAI Chat Completions beschrieben.

## Anbieter, die dieses Format verwenden

Jeder Anbieter hat ein `DecisionsProfile` in `EgressCtx::decisions_profile`.

### OpenAI

`DecisionsProfile::OPENAI`, `POST /v1/decisions`: höchstens 400 Fragen, Bilder in der Eingabe und in den Anweisungen der Fragen. Nur `gpt-6-luna` beantwortet Entscheidungen.

### Andere Server

`DecisionsProfile::GENERIC` prüft keine Fragengrenze. Sein Pfad kommt aus `EgressCtx::with_base_path` oder `DecisionsProfile::with_path`. `DecisionsProfile::with_instruction_images(false)` lehnt Bilder in Anweisungen für einen Server ab, der sie nur in der Eingabe annimmt.
