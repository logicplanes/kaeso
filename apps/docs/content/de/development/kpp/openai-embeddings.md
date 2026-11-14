---
title: OpenAI Embeddings
description: Wie jedes Feld des Formats OpenAI Embeddings auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld des Formats OpenAI Embeddings nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::openai_embeddings::OpenAiEmbeddings` (`crates/kaeso-codecs/src/openai_embeddings/`)
- **Endpunkte:** `POST /v1/embeddings` und die [Modell-Endpunkte](#modelle), die alle OpenAI-Codecs teilen
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_embeddings/`

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `e95c0fe615f45a19e8878af923a729948445a6bd` (`CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`), die [Referenz zum Erstellen](https://developers.openai.com/api/reference/resources/embeddings/methods/create) und der [Leitfaden zu Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes).

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_embeddings`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage: `CreateEmbeddingRequest`

| Wire-Feld | KPP | Anmerkungen |
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

| Wire-Feld | KPP | Anmerkungen |
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

## Anbieter mit diesem Format

Andere Anbieter als OpenAI, die Embeddings bereitstellen, zeichnen in `fixtures/openai_embeddings/providers/<id>/` auf ([Fixtures](fixtures.md)).

### Together AI

Quellen: die [OpenAI-Kompatibilität](https://docs.together.ai/docs/inference/openai-compatibility) (`embeddings.create` ist `POST /v1/embeddings`) und die [Serverless-Modelle](https://docs.together.ai/docs/serverless-models) von Together AI, gelesen am 2026-10-19, und `fixtures/openai_embeddings/providers/togetherai/`. Together AI stellt Embedding-Modelle nur auf dedizierten Endpunkten bereit; ein Schlüssel ohne einen solchen erhält einen 400 `model_not_available`. Seine Antwort hat kein `usage`, was dieser Codec bereits liest.

### OpenRouter

Quelle: die [Embeddings-Referenz](https://openrouter.ai/docs/api_reference/embeddings) von OpenRouter, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_embeddings/providers/openrouter/` mit `nvidia/nemotron-3-embed-1b:free`. Das `model` der Antwort nennt das ausgelieferte Modell, nicht die angefragte ID; `usage.cost`, `usage.is_byok`, `usage.cost_details` sowie `provider` und `id` auf oberster Ebene bleiben in den Extras und kommen unverändert zurück. Ein Fehler des dahinterliegenden Anbieters kommt als 400, dessen `message` den Fehler-Body dieses Anbieters als Text enthält.

### Mistral

Quellen: Mistrals [API-Referenz](https://docs.mistral.ai/api/) (`EmbeddingRequest`, `EmbeddingResponse`), gelesen am 2026-10-19, und die Live-Aufnahmen in `fixtures/openai_embeddings/providers/mistral/` mit `mistral-embed`. Mistral nimmt `encoding_format: base64` an, antwortet aber mit Gleitkommazahlen; für eine App, die base64 verlangt hat, kodiert der Codec sie als base64. Mistral lehnt OpenAIs `dimensions` und Token-IDs als Eingabe mit 422 ab und `output_dimension` bei `mistral-embed` mit 400. Diese Fehler-Bodys haben kein `error`-Objekt, daher erhalten Apps einen OpenAI-Fehler mit Mistrals Meldung, wenn der Body sie als String enthält, und sonst einen, der den Body zitiert.

### Vercel

Quellen: Vercels Seite zu [Embeddings](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/embeddings), gelesen am 2026-10-29, und die Live-Aufzeichnungen in `fixtures/openai_embeddings/providers/vercel/` mit `google/gemini-embedding-001` und `dimensions: 8`. Ein Vercel-Modell ist ein Embeddings-Modell, wenn KPMR ihm die Art `embedding` gibt, wie es auch Vercels Modellliste tut. `providerMetadata` der Antwort (Routing und Kosten) bleibt in den Extras und kommt unverändert zurück. Ein Konto im kostenlosen Tarif darf manche Embedding-Modelle nicht aufrufen, etwa `openai/text-embedding-3-small`, die mit 403 antworten.

### Cohere

Quellen: Coheres Seite [Compatibility API](https://docs.cohere.com/docs/compatibility-api), gelesen am 2026-10-30, und die Live-Aufnahmen in `fixtures/openai_embeddings/providers/cohere/` mit `embed-english-light-v3.0` und `embed-v4.0`. Cohere nimmt `encoding_format` `float` und `base64` an und antwortet im verlangten Format. Seine Seite nennt `dimensions` als nicht unterstützt, doch `embed-v4.0` beachtet es (`responses/recorded-embed-dimensions`, 256 Gleitkommazahlen); die v3-Modelle lehnen es mit 400 ab. Token-IDs als Eingabe lehnt Cohere mit 422 ab. Coheres Fehler-Bodys sind `{"id", "message"}`, daher erhalten Apps einen OpenAI-Fehler mit Coheres Meldung. KPMR führt kein Embed-Modell von Cohere, daher werden sie als eigene Modelle hinzugefügt, die als Embeddings senden.

### QwenCloud

Quellen: die Seite [Embedding](https://www.alibabacloud.com/help/en/model-studio/embedding) von Alibaba Cloud Model Studio, heute QwenCloud, gelesen am 2026-10-30, und die Live-Aufnahmen in `fixtures/openai_embeddings/providers/alibaba/` mit `text-embedding-v4` und `text-embedding-v3` in der internationalen Region (Singapur). QwenCloud nimmt `encoding_format` `float` und `base64` an und antwortet im verlangten Format, dazu `dimensions` aus den Größen, die jedes Modell nennt (`responses/recorded-embed-dimensions`, 256 Gleitkommazahlen); eine andere Größe erhält 400 mit diesen Größen. Ein Batch nimmt höchstens 10 Eingaben; 11 erhalten 400 (`errors/recorded-embed-batch-eleven`). Token-IDs als Eingabe erhalten 400, ein Generierungsmodell 404. Antworten tragen ein `id` auf oberster Ebene, das in den Extras bleibt. KPMR führt kein Embeddings-Modell von QwenCloud, daher werden sie als eigene Modelle hinzugefügt, die als Embeddings senden.

### DeepInfra

Quellen: das [OpenAPI document](https://api.deepinfra.com/openapi.json) von DeepInfra (`OpenAIEmbeddingsIn`) und seine [Anleitung zu Embeddings](https://docs.deepinfra.com/apis/embeddings.md), gelesen am 2026-11-14, und `fixtures/openai_embeddings/providers/deepinfra/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Die Embedding-Modelle von DeepInfra wie `BAAI/bge-m3` und `Qwen/Qwen3-Embedding-8B` nehmen `encoding_format` und `dimensions` ab 32 an und füllen einen Vektor über die Größe des Modells hinaus mit Nullen. DeepInfras eigene Felder `input_type` und `task` bleiben in den Extras, und die Listen von Inhaltsteilen seiner multimodalen Modelle gehen von einer OpenAI-Embeddings-App so, wie sie geschrieben sind.

### Fireworks AI

Quellen: Fireworks' Referenz [create embeddings](https://docs.fireworks.ai/api-reference/creates-an-embedding-vector-representing-the-input-text) und sein [Embeddings-Leitfaden](https://docs.fireworks.ai/guides/querying-embeddings-models), gelesen am 2026-11-09, und `fixtures/openai_embeddings/providers/fireworks-ai/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Die serverlosen Modelle sind `qwen3-embedding-8b` (`fireworks/qwen3-embedding-8b`) und ältere Modelle unter ihren Hugging-Face-IDs wie `nomic-ai/nomic-embed-text-v1.5`; nur veränderbare nehmen `dimensions` an. Fireworks' eigene Felder `prompt_template`, `normalize` und `return_logits` bleiben Extras der Anfrage. Sein Schema hat kein `encoding_format`, daher kodiert der Codec die Floats als Base64 für eine App, die danach fragt.

### Nebius Token Factory

Quellen: die Referenz [create embeddings](https://docs.tokenfactory.nebius.com/api-reference/inference/create-embeddings) von Nebius, gelesen am 2026-11-09, und `fixtures/openai_embeddings/providers/nebius/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Das Embedding-Modell von Nebius ist `Qwen/Qwen3-Embedding-8B`. Es nimmt `encoding_format` und `dimensions` an, und jede Antwort trägt `service_tier`, das in den Extras bleibt.

### Scaleway

Quellen: die [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/) von Scaleway (`CreateEmbeddingResponse`), gelesen am 2026-11-09, und `fixtures/openai_embeddings/providers/scaleway/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Die serverlosen Modelle sind `bge-multilingual-gemma2` und `qwen3-embedding-8b`. Antworten ergänzen `id`, `created` und `usage.completion_tokens: 0`, die in den Extras bleiben. `dimensions` nimmt nur die größte Größe eines Modells an, daher beantwortet Scaleway andere Werte selbst.

### NVIDIA

Quellen: die Embedding-Referenzen von NVIDIA wie [`nvidia/nv-embedqa-e5-v5`](https://docs.api.nvidia.com/nim/reference/nvidia-nv-embedqa-e5-v5-infer), gelesen am 2026-11-10, und `fixtures/openai_embeddings/providers/nvidia/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Zu seinen Modellen gehören `nvidia/nemotron-3.5-embed-1b` und `nvidia/nv-embedqa-e5-v5`. Anfragen nehmen `input_type`, `query` oder `passage`, das die Modelle für die Suche verlangen, und `truncate`, `NONE`, `START` oder `END`; beide bleiben in den Extras und gehen unverändert. Die Fehler von NVIDIA sind Problemdokumente, die Apps in der Form von OpenAI erhalten, mit dem Inhalt von NVIDIA in der Meldung.

### SiliconFlow

Quellen: die Referenz [create embeddings](https://docs.siliconflow.com/en/api-reference/embeddings/create-embeddings) von SiliconFlow, gelesen am 2026-11-10, und `fixtures/openai_embeddings/providers/siliconflow/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Zu seinen Modellen gehören `Qwen/Qwen3-Embedding-8B`, `-4B` und `-0.6B` und `BAAI/bge-m3`. Nur die Qwen3-Modelle nehmen `dimensions` an, und eine Anfrage nimmt höchstens 32 Zeichenketten. Antworten tragen `usage.completion_tokens: 0`, das in den Extras bleibt. Die Fehler von SiliconFlow haben kein `error`-Objekt; Apps erhalten sie in der Form von OpenAI mit der Meldung von SiliconFlow.

### Cloudflare AI Gateway

Quellen: die Seite [OpenAI compatibility](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/) und die [Modellseiten](https://developers.cloudflare.com/workers-ai/models/) von Workers AI, gelesen am 2026-11-14, die die Anfrage, aber keine Antwort zeigen, und `fixtures/openai_embeddings/providers/cloudflare-ai-gateway/`, live aufgezeichnet. Kaeso sendet die Anfrage an `/client/v4/accounts/{account_id}/ai/v1/embeddings`, das Konto des Schlüssels an der Stelle von `/v1`, mit dem Gateway-Header des Schlüssels, wenn er eines nennt. Die Antwort hat `object`, `data` und `model` und kein `usage`, daher schätzt Kaeso die Tokens der Anfrage. Workers AI ignoriert `dimensions` und antwortet in der vollen Größe des Modells, daher lehnt das Embeddings-Profil des Anbieters es ab (`without_dimensions()`); `encoding_format: "base64"` beantwortet es mit Gleitkommazahlen, die der Codec der App in der Kodierung gibt, die sie verlangt hat. `@cf/google/embeddinggemma-2` beantwortet diesen Pfad auch mit `input` mit 400, daher listet KPMR es nicht.
