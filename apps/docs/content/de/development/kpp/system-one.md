---
title: System One
description: Wie jedes Feld des Entscheidungsformats System One von TypeSafe auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld des Formats System One von TypeSafe nach: wohin es in KPP gehört, wie jeder Anbieter es annimmt und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::system_one::SystemOne` (`crates/kaeso-codecs/src/system_one/`)
- **Endpunkte:** `POST /v1/systemone`, als JSON oder als Upload mit `multipart/form-data`, und die [Modell-Endpunkte](#modelle) von OpenAI
- **Fixtures:** `crates/kaeso-codecs/fixtures/system_one/`, aus dem OpenAPI-Dokument und der Dokumentation von TypeSafe gebaut; aufgezeichnet sind die zwei Antworten von TypeSafe auf abgelehnte Schlüssel in `providers/typesafe/` und die Antworten und Ablehnungen von Clef bei Cloudflare in `providers/cloudflare-ai-gateway/`

**Quellen.** Das OpenAPI-Dokument von TypeSafe `https://api.typesafe.ai/openapi.json` in Version 0.2.0 (`SystemOneRequest`, `Question`, `NoulCriteria`, `Asset`, `SystemOneResponse`, `Answer`, `HTTPBadRequestError`, `HTTPValidationError`) und seine Seiten zu [System One](https://docs.typesafe.ai/api/system-one), [Bildern](https://docs.typesafe.ai/images), [Fehlern](https://docs.typesafe.ai/api/errors) und [Konfidenz](https://docs.typesafe.ai/confidence), gelesen am 2026-11-12.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `system_one`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage: `SystemOneRequest`

Eine Entscheidungsanfrage wird zu `Request::Decide(DecideRequest)` dekodiert. Ihre Fragen behalten ihre Reihenfolge, und `answers[i]` der Antwort beantwortet `questions[i]`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `DecideRequest.model` typisiert | Erforderlich. |
| `state` String | `input = DecisionInput::Text` typisiert | |
| `state` Objekt oder Array | `input = DecisionInput::Json` typisiert | Byte für Byte bewahrt, samt Reihenfolge der Schlüssel. |
| `state` mit `image_url`-Teilen | `input = DecisionInput::Json` und `assets` typisiert | Jeder Teil wird in jeder Tiefe zu einem Asset unter einer neuen Referenz, `kaeso-image-<n>-ref`, die dort steht, wo der Teil stand; die eigenen Felder des Teils bleiben Extras des Assets, Form `state_part`. So nehmen Perplexity, OpenRouter und Baseten Bilder an ([Bilder](#bilder)). |
| `questions` | `questions` typisiert | Ein Objekt benannter Fragen; alles andere ist `InvalidRequest`. Jeder Schlüssel ist `Question.name`. |
| `questions.*.type` | `Question.kind` typisiert | `noul` ist `Predicate`, `choice`, `score` und `box` behalten ihre Namen. Ein anderer Typ ist `InvalidRequest`. |
| `questions.*.instructions` String | `Instructions::Text` typisiert | |
| `questions.*.instructions` Objekt oder Array | `Instructions::Json` typisiert | |
| `questions.*.instructions` `null` | nichts, Form (`verbatim`) | |
| noul-`criteria` Objekt | `Predicate.when_true`, `.when_false` typisiert | `true` und `false` als `Description::Text` oder `Description::Json`; `null` ist Form (`verbatim`). Das Objekt selbst ist Form `criteria_object`, damit `{}` zurückkommt. |
| noul-`criteria` `null` | Form (`verbatim`) | |
| choice-`criteria` Objekt | `Choice.options` typisiert | Jeder Schlüssel ist `ChoiceValue::Text`, in Reihenfolge; seine Beschreibung wie bei noul. Eine Beschreibung `null` ist Form `null` an der Option. |
| score-`criteria` Array | `Score.levels` typisiert | Jedes Element ist die `description` der Stufe; Stufe `i` zählt `i`. |
| box-`criteria` | Extras | Eine Box-Frage hat keine Kriterien; trotzdem gesendete bleiben an der Frage. |
| `assets` | `assets` typisiert | Nach Referenz, in Reihenfolge. |
| `assets.*.type` | nichts | Muss `image` sein, sonst `InvalidRequest`; fehlend ist Form `no_type`. |
| `assets.*.format` | der Medientyp der Quelle | `png` ist `image/png`, `jpg` und `jpeg` sind `image/jpeg`, wie geschrieben in Form `format` bewahrt. Ein anderer Wert ist `InvalidRequest`. |
| `assets.*.data_base64` | `Asset.source = Source::Base64` typisiert | Braucht `format`. Text, der kein Base64 ist, ist `InvalidRequest`. |
| `assets.*.part` | `Asset.source = Source::Base64` typisiert | Nur in einem Upload: die Bytes des genannten Teils, ein Ausschnitt des Bodys. Form `part`, `part_type` und `filename`. Ein `data_base64` daneben bleibt Extras, weil TypeSafe es ignoriert. |
| unbekannte Schlüssel | Extras | Auf jeder Ebene: Anfrage, Frage, Kriterien, Asset. |
| Header `typesafe-danger-zone-images` | Extras `header:typesafe-danger-zone-images` | Wird nur an System One weitergeleitet. |
| Header `typesafe-danger-zone-long-context` | Extras `header:typesafe-danger-zone-long-context` | Wird nur an System One weitergeleitet. Kaeso sendet ihn TypeSafe auch selbst, wenn eine Anfrage ihn braucht ([TypeSafe](#typesafe)). |

Der erste Teil eines Uploads ist `request`, die JSON-Anfrage. Jeder andere Teil ist ein Bild, das genau ein Asset in `part` nennt, mit dem Typ `image/png`, `image/jpeg`, `image/jpg`, `application/octet-stream` oder keinem. Ein fehlender, ungenutzter oder doppelter Teil oder ein anderer Typ ist `InvalidRequest`. Uploads gehen nur an TypeSafe wieder als Uploads; andere Anbieter bekommen die Bilder als Base64.

`DecideRequest::validate()` verlangt ein Modell, mindestens eine Frage, mindestens eine Option und Stufe und für jedes Asset eine Referenz in einem String des Zustands, der Anweisungen oder der Beschreibungen. Objektschlüssel zählen nie, weil TypeSafe sie nicht ersetzt. `decode_request` lehnt Anfragen jeder anderen Art ab.

## Bilder

Jedes Bild ist ein Asset, auf das Strings verweisen. Wohin es geht, entscheidet das `SystemOneProfile` des Anbieters in `EgressCtx::system_one_profile`:

| `MediaPlacement` | Geschrieben als | Abgelehnt |
|---|---|---|
| `Assets` | `assets` mit `data_base64`, oder wieder die Teile des Uploads | Bild-URLs |
| `State` | Ein `image_url`-Teil mit Data-URL anstelle jeder Referenz, die ein ganzer String im Zustand ist | Eine Referenz in einem längeren String, in Anweisungen oder in Beschreibungen |
| `StateOrAssets` | Teile, wo die App Teile gesendet hat, `assets` für die übrigen | Bild-URLs in `assets` |
| `Conversation` | Eine Benutzernachricht unter `state.messages`: jeder String ein `text`-Teil, jedes Bild ein `image_url`-Teil | Bilder in einem anderen Zustand als Text, einer Liste von Strings oder `{"messages": …}` |
| `Root` | Eine Liste `images` von Data-URLs auf oberster Ebene, der Reihe nach aus einem Zustands-Array genommen; ein Bild, das auf andere Elemente folgte, rückt vor den Zustand, in `adapted` als `image_order` genannt | Eine Referenz an jeder anderen Stelle als als ganzes Element eines Zustands-Arrays |
| `None` | Nichts | Jedes Bild |

Vor dem Senden liest der Codec Format und Größe jedes Bildes aus seinem Header, ohne es zu dekodieren (`kaeso_codecs::image_size`), und lehnt ab, was das Profil ablehnt: seine Formate, seine Bytes je Bild und je Anfrage, seine Pixel je Seite oder je Bild. Bilder werden nie umgewandelt oder skaliert. Die Ablehnungen nennen `image URL`, `image format`, `image size` oder `animated image`, mit dem Feld in `param`.

## Fragen und Optionen

| KPP | Geschrieben als | Anmerkungen |
|---|---|---|
| `Question.name` | der Schlüssel der Frage | Bleibt, wenn jede Frage einen Namen hat, der nur einmal vorkommt und für den Anbieter gültig ist; sonst geht jede Frage als `q0`…`qN-1`, in `adapted` als `names` genannt. Die Antwort wird nach Position zurückgelesen. |
| `Instructions::Blocks` | ein String oder ein Array aus Strings, JSON-Werten und Bildreferenzen | Bilder werden zu Assets. |
| `ChoiceValue::Bool` | `"true"`, `"false"` | In `adapted` als `booleans` genannt. `true` und `"true"` in einer Frage sind `Unsupported` (`repeated choice value`). |
| `Level` mit `label` und Text-`description` | `"label: description"` | In `adapted` als `levels` genannt. Ein Label allein wird als Stufe geschrieben. |
| `QuestionKind::Box` | `box` | `Unsupported` (`box question`), wo das Profil keine beantwortet. |
| `Choice.options`, `Score.levels` | `criteria` | Weniger als `min_options` oder `min_levels` des Profils oder mehr als `max_options` oder `max_levels` ist `Unsupported` (`options`, `levels`). |
| `Question.instructions` | `instructions` | Eine Frage ohne sie oder mit leerem Text ist `Unsupported` (`question without instructions`), wo `instructions_required` des Profils gesetzt ist. |
| `DecideRequest.cache`, `Block.cache` | nichts | Hinweise, für die System One keinen Platz hat, in `left_out` als `cache` genannt. |
| `DecideRequest.user` | nichts | Ein Hinweis, in `left_out` als `user` genannt. |
| `DecisionInput::Messages` | der Zustand | Nur eine Benutzernachricht; ihre Text-, JSON- und Bildblöcke werden zu einem String oder einem Array. Mehr Nachrichten sind `Unsupported` (`conversation input`). |

## Antwort: `SystemOneResponse`

Der Decoder liest jede Antwort unter dem Schlüssel, mit dem ihre Frage gesendet wurde, aus `EgressCtx`. Eine Antwort auf eine nicht gestellte Frage, eine fehlende Antwort, eine nicht gefragte Option oder Stufe oder eine Antwort eines anderen Typs ist ein Anbieterfehler.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `DecideResponse.model` typisiert | Das Modell, das geantwortet hat, etwa `jev-1.14.0` für `jev-latest`. Ein Kurzname, den `ModelField::ShortName` des Profils geschrieben hat, bekommt sein Präfix zurück, sodass `clef` von Cloudflare als `@cf/cloudflare/clef` gelesen wird. |
| `answers.*` noul | `AnswerValue::Predicate(PredicateAnswer { probability })` typisiert | `noul` ist die Wahrscheinlichkeit für Ja. |
| `answers.*` choice | `AnswerValue::Choice(ChoiceAnswer { option, probabilities })` typisiert | `option` ist der Index der gewählten Option; `probabilities` hat einen Eintrag je Option, `None`, wo die Antwort keinen hat. |
| `answers.*` score | `AnswerValue::Score(ScoreAnswer { score, probabilities })` typisiert | `probabilities` nach Stufe, aus den Schlüsseln `"0"`, `"1"`, …. |
| `answers.*.legend` | Form `legend` | Aus den Stufen der Anfrage neu gebaut, wenn die Antwort Formate wechselt. |
| `answers.*` box | `AnswerValue::Box(BoxAnswer)` typisiert | Kanten als Anteile; `confidence: null` ist Form `null_confidence`. |
| `answers.*.confidence` | `Answer.confidence` typisiert | Siehe [Konfidenz](#konfidenz). |
| `answers.*` refusal | `AnswerValue::Refusal` typisiert | TypeSafe sendet keine; geschrieben für eine Antwort, die ein anderes Format verweigert hat. |
| als ganze Zahlen geschriebene Zahlen | Form `integers` | Wieder als ganze Zahlen geschrieben, solange der Wert unverändert ist. |
| `usage.input_tokens` | `usage.input_tokens` typisiert | Bilder eingeschlossen. |
| `usage.output_tokens` | `usage.output_tokens` typisiert | TypeSafe zählt sie und berechnet sie nicht. |
| `usage` | `usage.raw` | Andere Schlüssel, etwa `cost` von OpenRouter, kommen im selben Format zurück. |
| unbekannte Schlüssel | Extras | Auf jeder Ebene. |

Antworten werden in der Reihenfolge der Fragen unter den eigenen Schlüsseln der App geschrieben, die der Encoder aus der Anfrage der App in `EncodeCtx::request` nimmt.

### Konfidenz

Eine Antwort eines System-One-Anbieters behält ihre `confidence`, wie gesendet. Eine Antwort aus einem anderen Format bekommt die [dokumentierten Formeln](https://docs.typesafe.ai/confidence) von TypeSafe: `(p_max − 1/n) / (1 − 1/n)` für eine Auswahl und `max(0, 1 − Σ p_i·|i − m| / MAD_unif)` für eine Bewertung, 1 für eine einzige Option oder Stufe. Eine Ja-Nein-Antwort hat keine, und eine Box behält die des Anbieters oder `null`.

Perplexity berechnet die Konfidenz einer Bewertung gegen die Abstände zur wahrscheinlichsten Stufe statt zur Mitte, daher weichen seine Werte von denen von TypeSafe ab. Seine Antworten behalten sie, wie gesendet.

## Streaming

Entscheidungen werden nicht gestreamt. Der Stream-Encoder gibt für jedes Event `Unsupported` (`streaming`) zurück; der Stream-Decoder meldet ein `StreamEvent::Error` mit `Unsupported` (`streaming`) und danach nichts mehr.

## Fehler

| Status | Body | KPP |
|---|---|---|
| 400 | `{"detail": "…"}` oder `{"detail": {"message": …}}` | `InvalidRequest`; `ContextTooLong`, wenn der Body `max_tokens_exceeded` nennt |
| 401, oder 403 mit `detail.error_type` `authentication_error` | `{"detail": …}` | `Authentication`; mit diesem 403 antwortet TypeSafe auf eine Anfrage ohne Schlüssel |
| 413 | `{"detail": …}` | `RequestTooLarge` |
| 415 | `{"detail": …}` | `InvalidRequest` |
| 422 | `{"detail": [{"loc", "msg", "type", "input"?, "ctx"?}]}` | `InvalidRequest`, die Stelle des ersten Problems ohne `body` in `param` |
| 429 | `{"detail": …}`, `retry-after`, `retry-after-ms` | `RateLimited` mit seiner Wartezeit |
| 529 | `{"detail": …}` | `Overloaded` |

Ein Body ohne `detail` wird wie ein OpenAI-Fehler gelesen; das deckt die `error`-Hülle von OpenRouter und Perplexity und die von Cloudflare ab. Ein System-One-Fehler erreicht eine System-One-App, wie gesendet. Jeder andere Fehler wird als `{"detail": message}` mit dem Status seiner Art geschrieben: 422 mit `detail[{loc, msg, type: "value_error"}]` für ein `InvalidRequest` mit `param`, 400 für andere ungültige oder nicht unterstützte Anfragen, 402 für Abrechnung, 529 für einen überlasteten Anbieter.

## Modelle

`GET /v1/models` von TypeSafe antwortet mit `{"models": [{name, description, release_date}]}`, und kein Header unterscheidet das SDK von TypeSafe von dem von OpenAI. Dieser Codec liest und schreibt die Modellliste von OpenAI, wie unter [Modelle](openai-chat.md#modelle) auf der Seite zu OpenAI Chat Completions beschrieben. Von einem Anbieter liest er auch die eigene Modellliste von TypeSafe: Jeder `name` ist eine Modell-ID, `description` und `release_date` bleiben in den Extras. TypeSafe hat kein `GET /v1/models/{model}`.

## Anbieter, die dieses Format verwenden

Jeder Anbieter hat ein `SystemOneProfile` und legt ab, was nur er sendet, in `fixtures/system_one/providers/<id>/` ([Fixtures](fixtures.md)).

### TypeSafe

`SystemOneProfile::TYPESAFE`, `POST /v1/systemone`. Bilder gehen in `assets` oder als Teile eines Uploads, PNG und JPEG mit 280 bis 1568 Pixeln je Seite und höchstens 16 MiB, nur Standbilder, mit `TypeSafe-Danger-Zone-Images: true`, sobald eine Anfrage ein Bild oder eine Box-Frage enthält. Box-Fragen brauchen Jev 1.14. Bis zu 255 Optionen und 10 Stufen.

Ohne `TypeSafe-Danger-Zone-Long-Context: true` bekommt eine Anfrage 64k Token für den State mit allen Fragen und 32k für den State mit einer Frage; mit dem Header 262k für beides ([Models](https://docs.typesafe.ai/models#long-context)). `long_context` im Profil hält die üblichen Grenzen. Schätzt der Encoder den State mit seinen Fragen über einer davon, zu 4 Byte Text je Token und ohne Bilder, für die TypeSafe noch keine Zählung veröffentlicht, sendet er den Header und nennt `long_context` in `adapted`. Einen Header, den die App selbst gesendet hat, leitet er unverändert weiter, und nichts wird angepasst. Langer Kontext kostet je Token gleich viel.

### OpenRouter

`SystemOneProfile::OPENROUTER`, `POST /v1/systemone` unter `https://openrouter.ai/api`. Quellen: der [Jev-Leitfaden](https://openrouter.ai/docs/guides/community/jev), die [Referenz der Decisions-API](https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request) und der Leitfaden zu multimodalen Entscheidungen, gelesen am 2026-11-12. OpenRouter nimmt Bilder als `image_url`-Teile im Zustand und als `assets` an, aber nicht als Uploads. Seine Antwort fügt `id`, `provider` und `usage.cost` hinzu, die in Extras bleiben.

### Perplexity

`SystemOneProfile::PERPLEXITY`, `POST /v1/decisions` mit einem System-One-Body. Quelle: der [Schnellstart der Decisions-API](https://docs.perplexity.ai/docs/decisions/quickstart), gelesen am 2026-11-12. Bilder sind `image_url`-Teile mit Data-URLs in PNG, JPEG oder WebP an beliebiger Stelle im Zustand, höchstens 8. Perplexity beantwortet keine Box-Fragen und lehnt ein Feld auf oberster Ebene ab, das es nicht dokumentiert; eigene Felder einer App sind daher vor dem Senden `Unsupported`. `mode` und `max_staleness` werden nicht gesendet.

### Baseten

`SystemOneProfile::BASETEN`, `POST /v1/systemone`. Quelle: die [System-One-Referenz](https://docs.baseten.co/reference/inference-api/system-one), gelesen am 2026-11-12. Bilder gehen in eine Benutzernachricht unter `state.messages`, PNG, JPEG oder WebP, höchstens 16; `service_tier` bleibt in Extras. Keine Box-Fragen.

### Fireworks AI

`SystemOneProfile::FIREWORKS`, `POST /v1/systemone` unter `https://api.fireworks.ai/inference`, nur Text und keine Box-Fragen. Das Modell ist in einer privaten Vorschau, daher folgt das Profil der Anbieterseite von LiteLLM statt der von Fireworks selbst.

### Cloudflare Workers AI

`SystemOneProfile::CLOUDFLARE_WORKERS_AI`, für die Clef-Modelle von Cloudflare. Quellen, gelesen am 2026-11-13: die Modellseiten von [Clef](https://developers.cloudflare.com/workers-ai/models/clef/), [Clef-flash](https://developers.cloudflare.com/workers-ai/models/clef-flash/) und [Clef-omni](https://developers.cloudflare.com/workers-ai/models/clef-omni/) mit ihren `schema-input.json` und `schema-output.json`, die [Fehler](https://developers.cloudflare.com/workers-ai/platform/errors/) von Workers AI und die [REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/#call-a-workers-ai-model) von AI Gateway, die den Workers-AI-Pfad mit dem Modell darin beibehält und ihn über das in `cf-aig-gateway-id` genannte Gateway leitet.

- **Pfad.** `/run/@cf/cloudflare/{model}` neben der `/v1`-Wurzel des Schlüssels: `POST /client/v4/accounts/{account_id}/ai/run/@cf/cloudflare/clef`. Der Body nennt das Modell mit seinem Kurznamen, `clef`, `clef-flash` oder `clef-omni`; ein anderer Name ist ein 422.
- **Grenzen.** 1 bis 64 Fragen mit Anweisungen, Schlüssel aus Buchstaben, Ziffern, `_`, `.` und `-` mit bis zu 100 Zeichen, 2 bis 255 Optionen und 2 bis 10 Stufen; keine Box-Fragen. Cloudflare lehnt Felder ab, die es nicht dokumentiert, daher sind eigene Felder einer App vor dem Senden `Unsupported`.
- **Bilder.** Eine Liste `images` auf oberster Ebene aus Data-URLs in PNG, JPEG oder WebP, höchstens 4, je höchstens 4 MiB und 16 Megapixel, zusammen 8 MiB. Clef-omni nimmt auch `audio` und `videos` an, für die KPP keinen Platz hat.
- **Antworten.** Die Antwort ist das `result` der Hülle von Cloudflare, `{"result", "success", "errors", "messages"}`. Eine Ja-Nein-Antwort hat keine `confidence`. Fehler kommen in der Hülle mit `success: false`, etwa 401 mit Code 10000 für ein abgelehntes Token, 422 mit Code 3030, der die verletzte Schema-Regel nennt, und 400 mit Code 2001 für ein Gateway, das das Konto nicht hat.

Live-Prüfungen am 2026-11-13: Die dokumentierte Anfrage antwortete auf allen drei Modellen mit 200, mit und ohne `cf-aig-gateway-id`; die Grenzen oben kamen als die Ablehnungen zurück, die in `providers/cloudflare-ai-gateway/errors/` aufgezeichnet sind; und ein Zustand mit 10.350 Tokens wurde ganz gelesen. Der Zustand wird also nur gekürzt, um in den Kontext des Modells zu passen, wie die Modellseiten sagen, nicht bei etwa 2.000 Tokens.

### Andere Server

`SystemOneProfile::GENERIC` nimmt jeden Fragetyp an, sendet Bild-URLs, wo seine Platzierung Teile schreibt, und prüft keine Grenzen für Bilder. Sein Pfad kommt aus `EgressCtx::with_base_path` und die Platzierung der Bilder aus `SystemOneProfile::with_media`, für vLLM, llama.cpp und andere Server des Formats.
