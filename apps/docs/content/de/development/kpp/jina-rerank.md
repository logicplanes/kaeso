---
title: Jina Rerank
description: Wie jedes Feld des Rerank-Formats von Jina AI auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld des Rerank-Formats von Jina AI nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::jina_rerank::JinaRerank` (`crates/kaeso-codecs/src/jina_rerank/`)
- **Endpunkt:** `POST /v1/rerank` als Format von Jina AI, unter `https://api.jina.ai` ([Unterstützte Anbieter](../../concepts/supported-providers.md)). Apps ordnen im [Format von Cohere](cohere-rerank.md) neu; kein Endpunkt für Apps nimmt dieses an.
- **Fixtures:** `crates/kaeso-codecs/fixtures/jina_rerank/`, die Anfragen aus den Beispielen des OpenAPI-Dokuments von Jina, aus dem Schema gebaute Antworten und die live beantworteten Fehler zu abgelehnten Schlüsseln

**Quellen.** Das [OpenAPI-Dokument](https://api.jina.ai/openapi.json) von Jina AI (`info.version` `2026.10.13.1624`: `RerankerV3Request`, `TextRerankerRequest`, `RerankingResponse`, `ErrorResponse`) und seine [API-Referenz](https://docs.jina.ai/api.md), gelesen am 14.11.2026.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `jina_rerank`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`.

## Anfrage

Das `model` wählt das Schema der Anfrage. Jina AI prüft den Schlüssel vor dem Body, daher ließ sich nicht aufzeichnen, ob es Felder ablehnt, die es nicht kennt.

| Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `RerankRequest.model` typisiert | |
| `query` | `query` typisiert | Ein String; die Bild-Suchanfragen von `jina-reranker-m0` haben in KPP keinen Platz und werden abgelehnt. |
| `documents` als Strings | `documents[i].text` typisiert | |
| `documents` als `{"text"}`-Objekte | `documents[i].text` typisiert, Form `object` | An Jina als Objekte zurückgeschrieben; von Cohere gehen sie als Strings. Ein Dokument mit Feldern neben `text` oder ein `{"image"}` wird abgelehnt. |
| `top_n` | `top_n` typisiert | |
| `return_documents` | `return_documents` typisiert | Der Standard von Jina ist true, der von Cohere false: Ein Body von Jina ohne das Feld wird als true dekodiert, und der Encoder schreibt es immer. |
| `max_doc_length` | `max_tokens_per_doc` typisiert | Nur `jina-reranker-v3` und `jina-reranker-v3.5`; für die anderen Modelle wird `max_tokens_per_doc` abgelehnt. |
| `return_embeddings` und andere Schlüssel | Extras | Aus einem Body von Jina gelesen; kein Format einer App schreibt sie. |

`check()` lehnt ab, was `cross::check_foreign` ablehnt, etwa `relevant_spans` und `priority` von Cohere.

## Antwort

| Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `model` typisiert | |
| `object` | Form `object_name`, wenn nicht `"list"`, `no_object`, wenn es fehlt | |
| `results[].index` | `RerankResult.index` typisiert | |
| `results[].relevance_score` | `relevance_score` typisiert | Unverändert weitergegeben; Jina dokumentiert keinen Bereich. |
| `results[].document` als String oder `{"text"}` | `document` typisiert, Form `object` bei einem Objekt | In der Form zurückgegeben, in der es gesendet wurde; eine Cohere-App bekommt `{"text"}`. Ein `{"image"}` wird als Antwort abgelehnt, die Kaeso nicht lesen kann. |
| `results[].embedding` und andere Schlüssel | Extras des Ergebnisses | |
| `usage.total_tokens` | `usage.input_tokens` typisiert | `usage` bleibt in `usage.raw`; `no_usage`, wenn es fehlt. |
| unbekannte Schlüssel | Extras | |

## Streaming

Der Endpunkt streamt nicht, wie bei [Cohere Rerank](cohere-rerank.md#streaming).

## Fehler

Jina AI antwortet mit `{"detail", "code", "request_id"}`, bei einem 422 mit `errors`. Der Code bestimmt die Art, wo er mehr sagt als der Status: `AUTHZ_INSUFFICIENT_BALANCE` (403) `Billing`, `INPUT_TOKEN_LIMIT_EXCEEDED` (400) `ContextTooLong`, `INPUT_MODEL_NOT_FOUND` (400) `NotFound`; sonst bestimmt sie der Status, wie bei [Voyage Rerank](voyage-rerank.md#fehler). Ein Body ohne `detail` wird gelesen, wie der Codec für [OpenAI Responses](openai-responses.md#fehler-errorresponse) ihn liest. Eine Cohere-App bekommt den Text als `message`. Der Codec für [OpenAI Embeddings](openai-embeddings.md#jina-ai) liest die Fehler von Jina genauso.

## Modelle

`GET /v1/models` von Jina AI ist öffentlich und hat das Format von OpenRouter, mit IDs als `jina-ai/<id>`; `GET /v1/models/{id}` findet ein Modell an seiner kurzen ID. Der Codec liest und schreibt Listen im Format von OpenAI ([Modelle](openai-chat.md#modelle)).

## Anbieter mit diesem Format

### Jina AI

Die Rerank-Modelle von Jina AI unter `/v1/rerank` unter der Basis-URL des Anbieters (`RerankEgress::of`): `jina-reranker-v3.5`, `jina-reranker-v3`, `jina-reranker-m0` (mit Text), `jina-reranker-v2-base-multilingual`, `jina-reranker-v1-base-en`, `-turbo-en` und `-tiny-en` sowie die ColBERT-Modelle `jina-colbert-v2` und `jina-colbert-v1-en`.
