---
title: OpenAI Embeddings
description: Wie jedes Feld des Formats OpenAI Embeddings auf KPP abgebildet wird und zurück.
order: 7
---

- **Codec:** `kaeso_codecs::openai_embeddings::OpenAiEmbeddings` (`crates/kaeso-codecs/src/openai_embeddings/`)
- **Endpunkte:** `POST /v1/embeddings` und die [Modell-Endpunkte](#modelle), die alle OpenAI-Codecs teilen
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_embeddings/`

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `e95c0fe615f45a19e8878af923a729948445a6bd` (`CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`), die [Referenz zum Erstellen](https://developers.openai.com/api/reference/resources/embeddings/methods/create) und der [Leitfaden zu Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes).

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_embeddings`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage: `CreateEmbeddingRequest`

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `model` | `EmbedRequest.model` typisiert | Erforderlich. |
| `input` String | `input = [EmbedInput::Text]` typisiert | Die kanonische Schreibweise für einen Text. |
| `input` Array von Strings | `EmbedInput::Text` je String typisiert | Array mit einem Element: Form `input: "array"`. |
| `input` Array von Token-IDs | `input = [EmbedInput::Tokens]` typisiert | Die kanonische Schreibweise für ein Token-Array. |
| `input` Array von Token-ID-Arrays | `EmbedInput::Tokens` je Array typisiert | Array mit einem Element: Form `input: "nested"`. |
| `input` leeres Array | `input = []` | Für die API ungültig (`minItems: 1`); `EmbedRequest::validate()`, das das Gateway nach dem Dekodieren ausführt, lehnt es mit `InvalidRequest` ab. |
| `input` jede andere Gestalt (Zahlen, die kein `u32` sind, gemischte Arrays) | `InvalidRequest` | |
| `encoding_format` `float`, `base64` oder fehlend | `encoding` `Float`, `Base64` oder `Float` typisiert | Ausdrückliches `float`: Form `encoding_format: "float"`. Andere Werte: `InvalidRequest`, weil KPP keinen Platz für sie hat. |
| `dimensions` | `dimensions` typisiert | |
| `user` | `user` typisiert | |
| `null`-Werte | Form (`verbatim`) | |
| unbekannte Schlüssel | Extras | Ein Schlüssel, der mit `header:` beginnt oder einen `.` enthält, lässt sich nicht mitführen: `InvalidRequest` (`` the field `…` has a name Kaeso cannot carry; remove it ``). |
| Header `openai-beta` | Extras `header:openai-beta` | Wird nur an Embeddings weitergeleitet. |

Die Spezifikation erlaubt 1 bis 2.048 Eingaben und höchstens 8.192 Token je Eingabe. Der Codec setzt diese Grenzen nicht durch, und der Anbieter antwortet mit seinem eigenen 400, wenn sie überschritten werden.

`decode_request` lehnt Anfragen zum Generieren und zum Zählen von Token ab (`generation request`). `check()` lehnt sie ebenfalls ab, außerdem Anfragen, die Text- und Token-Eingaben mischen (`mixed text and token inputs`), und alles, was `cross::check_foreign` ablehnt, etwa Extras anderer Dialekte ([Anfragen](cross-format.md#anfragen)).

## Antwort: `CreateEmbeddingResponse`

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `object` (`list`) | keins | Ein anderer Wert: Form `object`. |
| `model` | `EmbedResponse.model` typisiert | |
| `data[].index` | `Embedding.index` typisiert | |
| `data[].embedding` Array von Zahlen | `EmbeddingVector::Float` (`f64`, identisch zurückgeschrieben) typisiert | |
| `data[].embedding` String | `EmbeddingVector::Base64` (wie empfangen) typisiert | Hat der Anbieter in der anderen Kodierung geantwortet, werden Antworten in das `encoding_format` der App umgewandelt, das aus `EncodeCtx::request` kommt: Floats werden zu Base64 aus Little-Endian-`f32`, und Base64 wird zu Floats in ihrer kürzesten `f32`-Schreibweise. Base64, das keine ganze Zahl von `f32`-Werten ergibt, lässt sich nicht umwandeln: `ProviderError`. Ohne die Anfrage der App in `EncodeCtx` bleiben Vektoren, wie sie empfangen wurden. |
| `data[].object` (`embedding`) | keins | Ein anderer Wert: Form `object` am Embedding. Unbekannte Schlüssel: Extras des Embeddings. |
| `usage.prompt_tokens` | `usage.input_tokens` typisiert | Das Nutzungsobjekt bleibt in `usage.raw` erhalten. |
| `usage.total_tokens` | abgeleitet | Wird als `prompt_tokens` geschrieben, da Embeddings keine Ausgabe-Token haben; das rohe Nutzungsobjekt dieses Dialekts gewinnt. |
| `usage` `null` | Form (`verbatim`) | |
| `usage` fehlt | Form `no_usage` | Es wird kein Nutzungsobjekt erfunden, wenn der Anbieter keines gesendet hat. |
| unbekannte Schlüssel | Extras | Ein Schlüssel, der mit `header:` beginnt oder einen `.` enthält, lässt sich nicht mitführen: `ProviderError`. |

Antworten auf Generierungsanfragen lassen sich in diesem Format nicht schreiben, und die Antwort eines Anbieters wird nur für eine Embeddings-Anfrage gelesen (beides `Unsupported`, `generation response`).

## Streaming

Der Endpunkt streamt nicht. Der Stream-Encoder gibt für jedes Event `Unsupported` (`streaming`) zurück; der Stream-Decoder meldet ein `StreamEvent::Error` mit `Unsupported` (`streaming`) und danach nichts mehr.

## Fehler

Fehler haben die Fehlergestalt von OpenAI, `{"error": {message, type, param, code, misalignment}}`. `misalignment` bleibt, wenn vorhanden, mit dem ganzen Body in `KppError.raw` erhalten und wird unverändert an Apps von OpenAI Embeddings und Responses erneut gesendet. Dekodieren und Kodieren folgen denselben Tabellen wie [die Fehler von OpenAI Responses](openai-responses.md#fehler-errorresponse), und die Umsetzung teilt sich dieser Codec mit dem Responses-Codec. Hier dekodierte Fehler tragen den Ursprung `openai_embeddings`.

## Modelle

`GET /v1/models` und `GET /v1/models/{model}` haben in jeder OpenAI-API dieselbe Gestalt. Dieser Codec kodiert und dekodiert sie mit der gemeinsamen Umsetzung, die unter [Modelle](openai-chat.md#modelle) auf der Seite zu OpenAI Chat Completions beschrieben ist, mit den Fixtures in `fixtures/openai_chat/models/`. Dekodierte Extras tragen den Ursprung `openai_embeddings`.
