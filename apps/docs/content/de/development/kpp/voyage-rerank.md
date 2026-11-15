---
title: Voyage Rerank
description: Wie jedes Feld des Rerank-Formats von Voyage AI auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld des Rerank-Formats von Voyage AI nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::voyage_rerank::VoyageRerank` (`crates/kaeso-codecs/src/voyage_rerank/`)
- **Endpunkt:** `POST /v1/rerank` als Format von Voyage AI, unter `https://ai.mongodb.com` oder einem anderen Host von Voyage AI ([Unterstützte Anbieter](../../concepts/supported-providers.md)). Apps ordnen im [Format von Cohere](cohere-rerank.md) neu; kein Endpunkt für Apps nimmt dieses an.
- **Fixtures:** `crates/kaeso-codecs/fixtures/voyage_rerank/`, die live gesendete Anfrage, aus dem Schema gebaute Antworten und die Fehler, live ohne gültigen Schlüssel beantwortet

**Quellen.** Das [OpenAPI-Dokument](https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json) von MongoDB zur Atlas Embedding and Reranking API (Version 1.1, `rerankDocuments`), die [Fehlercodes](https://docs.voyageai.com/docs/error-codes) von Voyage AI und die [Abrechnungsseite](https://www.mongodb.com/docs/voyageai/management/billing/) von MongoDB, gelesen am 14.11.2026.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `voyage_rerank`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`.

## Anfrage

Voyage AI lehnt jedes Feld, das es nicht kennt, mit 400 ab und prüft den Body vor dem Schlüssel (aufgezeichnet am 14.11.2026). Der Encoder schreibt nur die Felder unten.

| Feld | KPP | Anmerkungen |
|---|---|---|
| `query` | `RerankRequest.query` typisiert | |
| `documents` als Strings | `documents[i].text` typisiert | Nur Strings: Ein Dokument, das eine App als `{"text"}` geschrieben hat, geht als sein Text, und eines mit Feldern neben `text` wird abgelehnt (`document fields`). |
| `model` | `model` typisiert | |
| `top_k` | `top_n` typisiert | Das `top_n` von Cohere; `top_n` selbst lehnt Voyage AI ab. |
| `return_documents` | `return_documents` typisiert | Nur gesendet, wenn die App es gesetzt hat; der Standard von Voyage AI ist false, wie bei Cohere. |
| `truncation` und andere Schlüssel | Extras | Aus einem Body von Voyage AI gelesen; kein Format einer App schreibt sie. |
| `max_tokens_per_doc` | abgelehnt | Voyage AI schneidet jedes Dokument an der Grenze des Modells ab und lehnt das Feld ab. |

`check()` lehnt ab, was `cross::check_foreign` ablehnt, etwa `relevant_spans` und `priority` von Cohere.

## Antwort

| Feld | KPP | Anmerkungen |
|---|---|---|
| `object` | Form `object`, wenn nicht `"list"` | |
| `data[].index` | `RerankResult.index` typisiert | Der Schlüssel von Voyage AI ist `data`, nicht `results` wie bei Cohere. |
| `data[].relevance_score` | `relevance_score` typisiert | Unverändert weitergegeben; Voyage AI dokumentiert keinen Bereich. |
| `data[].document` | `document` typisiert | Ein String; eine Cohere-App bekommt ihn als `{"text"}`. |
| `model` | `model` typisiert | |
| `usage.total_tokens` | `usage.input_tokens` typisiert | Voyage AI rechnet die Tokens der Suchanfrage einmal pro Dokument plus die Tokens jedes Dokuments ab und meldet diese Summe. `usage` bleibt in `usage.raw`. |
| `usage` fehlt | Form `no_usage` | |
| unbekannte Schlüssel | Extras | |

## Streaming

Der Endpunkt streamt nicht, wie bei [Cohere Rerank](cohere-rerank.md#streaming).

## Fehler

Voyage AI antwortet mit `{"detail": "<Text>"}` ohne Code. Der Status gibt die Art: 400 und 422 `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied`, 404 `NotFound`, 413 `RequestTooLarge`, 429 `RateLimited` mit `retry-after`, 503 `Overloaded`, 408 und 504 `Timeout`, der Rest `ProviderError`. Ein Body ohne `detail` wird gelesen, wie der Codec für [OpenAI Responses](openai-responses.md#fehler-errorresponse) ihn liest. Eine Cohere-App bekommt den Text als `message`.

## Modelle

Voyage AI bietet keine Modellliste: `GET /v1/models` und `GET /v1/models/{model}` antworten mit 404 (aufgezeichnet am 14.11.2026), daher nennt **Verfügbarkeit prüfen** seine Modelle unklar. Der Codec liest und schreibt Listen im Format von OpenAI ([Modelle](openai-chat.md#modelle)).

## Anbieter mit diesem Format

### Voyage AI

Die Rerank-Modelle von Voyage AI, `rerank-3`, `rerank-3-lite`, `rerank-2.5`, `rerank-2.5-lite`, `rerank-2` und `rerank-2-lite`, unter `/v1/rerank` unter der Basis-URL des Anbieters (`RerankEgress::of`).
