---
title: Gemini Decisions
description: Wie jedes Feld der Decisions-API von Gemini auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld der Decisions-API von Gemini nach: wohin es in KPP gehört, was aus einer Anfrage eines anderen Entscheidungsformats wird und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::gemini_decisions::GeminiDecisions` (`crates/kaeso-codecs/src/gemini_decisions/`)
- **Endpunkte:** `POST /v1beta/decisions` und die [Modell-Endpunkte](#modelle) von Gemini
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini_decisions/`, aus dem Leitfaden, der API-Referenz und den Typen von `google-genai` gebaut, keine aufgezeichnet

**Quellen.** Googles [Leitfaden zur Decisions-API](https://aistudio.google.com/docs/decisions-api) und die [Referenz der Decisions-API](https://ai.google.dev/api/decisions), gelesen am 2026-11-12. `google-genai` 3.3.0 für Python: `types/decisions` und die Inhaltsblöcke der Interactions-API, die sie verwenden.

Das Gateway leitet Entscheidungsanfragen noch nicht weiter; der Codec ist dafür bereit.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `gemini_decisions`; ein anderer Dialekt lehnt sie ab, außer sie stehen auf der [Allowlist](cross-format.md#allowlist).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage: `CreateDecisionRequest`

Eine Entscheidungsanfrage wird zu `Request::Decide(DecideRequest)` dekodiert. Ihre Fragen behalten die Reihenfolge des Objekts `questions`, und `answers[i]` der Antwort beantwortet `questions[i]`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `DecideRequest.model` typisiert | Erforderlich. |
| `input` String | `input = DecisionInput::Text` typisiert | |
| `input` Inhaltsblock | `input = DecisionInput::Messages` typisiert | Eine Benutzer-`Message` mit einem Block ([Inhaltsblöcke](#inhaltsblöcke)); Form `object`. |
| `input` Liste von Blöcken | `input = DecisionInput::Messages` typisiert | Eine Benutzer-`Message` mit den Blöcken in Reihenfolge. |
| `questions` | `questions` typisiert | Ein Objekt nach Fragenschlüssel. Eine Liste ist `InvalidRequest` und nennt `/v1/decisions`. |
| Schlüssel in `questions` | `Question.name` typisiert | |
| `questions.*.type` | `Question.kind` typisiert | `noul` ist `QuestionKind::Predicate`, `choice` und `score` behalten ihre Namen. Ein anderer Typ ist `InvalidRequest`. |
| `instructions` String | `Instructions::Text` typisiert | |
| `instructions` Textblock oder Liste von Textblöcken | `Instructions::Blocks` typisiert | Nur Textblöcke; ein einzelner Block ist Form `object`. Ein Medienblock ist `InvalidRequest`. |
| `criteria` von `noul` | `Predicate` typisiert | `true_description` und `false_description` sind `when_true` und `when_false`; die Form `criteria` bewahrt das Objekt, wenn beide fehlen. |
| `criteria` von `choice` | `Choice.options` typisiert | Jeder Schlüssel ist ein `ChoiceValue::Text`, sein String eine `Description::Text`, auch `""`. Ein Wert, der kein String ist, ist `InvalidRequest`. |
| `criteria` von `score` | `Score.levels` typisiert | Jeder String ist die `Description::Text` einer Stufe; Stufen stehen aufsteigend. |
| unbekannte Schlüssel | Extras | Auf jeder Ebene: Anfrage, Frage, `criteria` von `noul` und Block. |

Ein Body mit `state` und ohne `input` ist eine System-One-Anfrage, `InvalidRequest` mit `/v1/systemone` in der Meldung.

### Inhaltsblöcke

| Block | KPP | Anmerkungen |
|---|---|---|
| `text` | `Block::Text` | `annotations` bleibt in den Extras. |
| `image`, `audio`, `video`, `document` mit `data` | `Block::Image`, `Audio`, `Video`, `Document` mit `Source::Base64` | Mit `data` ist `mime_type` erforderlich. Base64, das sich nicht dekodieren lässt, ist `InvalidRequest`. |
| dieselben mit `uri` | `Source::FileId` für eine URI der Files API, sonst `Source::Url` | `mime_type` ist Form `mime_type`. Kaeso ruft nie eine URL ab. |
| `data` und `uri` zusammen oder keins von beiden | keins | `InvalidRequest`. |
| `resolution` | Extras | Bei Bildern und Videos. Ein Hinweis auf der [Allowlist](cross-format.md#allowlist). |
| ein anderer `type` | `Block::Unknown` | Gebunden an `gemini_decisions`; ein anderes Format lehnt ihn ab. |

## Anfragen schreiben

Der Encoder schreibt, was KPP hält, und lehnt vor dem Senden ab, was das Format von Gemini nicht annimmt. Eine Anfrage eines anderen Formats wechselt nach diesen Regeln; [Entscheidungen über Formate hinweg](decisions-across-formats.md) nennt die Gründe.

| KPP | Geschrieben als | Anmerkungen |
|---|---|---|
| `Question.name` | der Fragenschlüssel | Bleibt, wenn jede Frage einen Namen hat, der nur einmal vorkommt und zu `[A-Za-z0-9_-]{1,64}` passt; sonst `q0`…`qN-1` für jede Frage, in `adapted` als `names` genannt (D19). |
| `DecisionInput::Json` | der String `input`, sein JSON, wie geschrieben | Googles Leitfaden verlangt JSON als Text (D13); keine Anpassung wird genannt. |
| `Block::Json` | ein Textblock mit seinem JSON, wie geschrieben | |
| eine Asset-Referenz in einem String | der String, um sie herum geteilt, das Bild an ihrer Stelle als Block `image` | Im String der Eingabe und in Strings einer Zustandsliste; der Text eines Listenelements und des nächsten wird mit einem Zeilenumbruch verbunden. Anderswo ist sie `Unsupported` (`image reference`), in Anweisungen `image in question instructions`, in Beschreibungen `image in criteria`. |
| `Instructions::Json`, `Description::Json` | ihr JSON, wie geschrieben, als Text | In `adapted` als `instructions` oder `criteria` genannt. |
| fehlende `Instructions` | `""` | `instructions` ist erforderlich; eine Anfrage im eigenen Format lässt es weg. |
| `ChoiceValue::Bool` | `"true"`, `"false"` | In `adapted` als `booleans` genannt (D18); eine Frage mit `true` und `"true"` ist `Unsupported`. |
| ein Optionsschlüssel, den Google ablehnt | keins | `Unsupported` (`option key`): mehr als 128 Zeichen oder 512 Bytes, ein Leerzeichen an einem Ende, ein Steuerzeichen oder ein Leerzeichen außerhalb von ASCII. |
| fehlende `ChoiceOption.description` | `""` | |
| `Level` mit Label und Beschreibung | `"label: description"` | In `adapted` als `levels` genannt. Eine Stufe ohne Text ist `Unsupported` (`score level`). |
| `Source::Base64`, Data-URL | `data` und `mime_type` | |
| `Source::Url` | `uri` und `mime_type` | Nur `https`. Der MIME-Typ kommt aus der Dateiendung, wie bei [Gemini](gemini.md); ohne sie ist das Bild `Unsupported`. URLs mit `http`, `gs://` und von YouTube sind `Unsupported`. |
| `Source::FileId` von Gemini | `uri` | Datei-IDs anderer Anbieter sind `Unsupported`. |
| `Source::Text` | keins | `Unsupported` (`text document`): senden Sie den Text als Textblock. |
| `detail` von OpenAI mit `low` oder `high` | `resolution` | `auto` und `original` werden weggelassen. |
| Bilder in Anweisungen, Verweigerungen, Tool-Elemente | keins | `Unsupported`, mit dem Teil benannt. |
| `QuestionKind::Box` | keins | `Unsupported` (`box question`). |
| weniger als 2 oder mehr als 255 Optionen oder Stufen | keins | `Unsupported` (`options`, `levels`). |
| mehr als 1.000 Fragen | keins | `Unsupported` (`questions`) (D5). |
| ein Gespräch | keins | `Unsupported` (`conversation input`) (D12). |
| `cache`, `user`, Cache-Breakpoints | keins | Weggelassen, in `left_out` als `cache` und `user` genannt. |

`EgressCtx` behält den Schlüssel und die Optionsschlüssel jeder gesendeten Frage sowie das Modell, sodass die Antwort nach Position zurückgelesen wird.

## Antwort: `CreateDecisionResponse`

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| keins | `DecideResponse.model` typisiert | Die Antwort von Gemini nennt kein Modell; der Decoder nimmt das der Anfrage. |
| `answers` | `answers` typisiert | Eine je Frage, nach Schlüssel. Ein Schlüssel, der nicht gefragt war, eine fehlende Antwort oder ein Typ, der nicht zur Frage passt, ist ein Anbieterfehler. |
| `noul.noul` | `PredicateAnswer.probability` typisiert | Google begrenzt ihn auf 0,01 bis 0,99; KPP behält ihn, wie gesendet. |
| `choice.choice` | `ChoiceAnswer.option` typisiert | Der Index der Option mit diesem Schlüssel. |
| `choice.probabilities` | `ChoiceAnswer.probabilities` typisiert | Nach Option; `None`, wo die Antwort keinen Eintrag hat. |
| `score.score` | `ScoreAnswer.score` typisiert | |
| `score.probabilities` | `ScoreAnswer.probabilities` typisiert | Nach Index der Stufe. |
| `score.legend` | Form `legend` | Aus den Stufen der Anfrage neu gebaut, wenn die Antwort das Format wechselt. |
| `confidence` | `Answer.confidence` typisiert | Siehe [Konfidenz](#konfidenz). |
| `usage.total_input_tokens` | `usage.input_tokens` typisiert | Ohne `total_cached_tokens`, das `usage.cache_read_tokens` ist. |
| `usage.input_tokens_by_modality` | `usage.input_image_tokens`, `input_audio_tokens` typisiert | Bild-, Video- und Dokumenttokens zählen als Bildtokens, wie Googles Leitfaden sie zählt. |
| `usage.total_output_tokens` | `usage.output_tokens` typisiert | 0 bei Google. |
| `usage` | `usage.raw` | Der ganze Verbrauch kommt im selben Format zurück. |
| `prompt_feedback.block_reason` | keins | Die Eingabe wurde blockiert: `ContentFiltered`, siehe [Fehler](#fehler). Ohne Grund bleibt `prompt_feedback` in den Extras. |
| als ganze Zahlen geschriebene Zahlen | Form `integers` | Wieder als ganze Zahlen geschrieben, solange der Wert unverändert ist. |
| unbekannte Schlüssel | Extras | Auf jeder Ebene. |

Der Encoder schreibt die Antworten unter den Schlüsseln der App in der Reihenfolge der Fragen und nimmt jeden Optionsschlüssel und jede Stufe aus der Anfrage der App in `EncodeCtx::request`. Er schreibt kein `model`. Eine Antwort aus einem anderen Format bekommt `total_input_tokens` mit dem Cache mitgezählt, `input_tokens_by_modality` mit Text als Rest, `total_output_tokens`, wie gemeldet, und `total_tokens`.

### Konfidenz

Eine Antwort von Gemini behält ihre `confidence`, wie gesendet. Eine Antwort aus einem anderen Format bekommt Googles Regel: die höchste Wahrscheinlichkeit für eine Auswahl und eine Bewertung, `max(p, 1 − p)` für eine Ja-Nein-Frage (D8). `confidence` wird immer geschrieben, da Googles SDK es verlangt.

### Verweigerungen

Das Format von Gemini hat keine Verweigerung als Antwort. Eine Antwort mit Verweigerungen von OpenAI erreicht eine Gemini-App als Fehler, `ContentFiltered` mit einer Meldung, die die verweigerten Fragen nennt (D9).

## Streaming

Entscheidungen werden nicht gestreamt. Der Stream-Encoder gibt für jedes Event `Unsupported` (`streaming`) zurück; der Stream-Decoder meldet ein `StreamEvent::Error` mit `Unsupported` (`streaming`) und danach nichts mehr.

## Fehler

Fehler nutzen die Hülle von Google, gelesen und geschrieben wie bei [Gemini](gemini.md#fehler). Eine blockierte Eingabe ist ein 400, dessen `code` statt des HTTP-Status ein Sicherheitscode ist (`safety`, `prohibited_content`, `blocklist`, `image_safety`, `content_blocked`). KPP liest ihn als `ContentFiltered`. Eine Antwort mit `prompt_feedback.block_reason` beantwortet nichts, daher ist auch sie `ContentFiltered` (D10).

Ein Gemini-Fehler erreicht eine Gemini-App, wie gesendet. Jedes andere `ContentFiltered` wird als `{"error": {"code", "message"}}` mit Status 400 geschrieben. Sein Code ist der Code des Blockiergrunds, sonst `safety`. Jeder andere Fehler bekommt die Hülle von Google mit dem Status seiner Art.

## Modelle

Der Codec liest und schreibt die Modellliste von Gemini, wie unter [Modelle](gemini.md#modelle) auf der Seite zu Gemini beschrieben.

## Anbieter, die dieses Format verwenden

### Google AI Studio

`POST /v1beta/decisions`, nur `gemini-decision-1`: 1 bis 1.000 Fragen, 2 bis 255 Optionen und Stufen, Eingaben aus Text, Bildern, Audio, Video und PDF. `EgressCtx::with_base_path` sendet an einen anderen Pfad. Gemini Enterprise Agent Platform bedient noch keine Entscheidungen (D21).
