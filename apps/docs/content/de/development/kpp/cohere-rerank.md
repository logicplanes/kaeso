---
title: Cohere Rerank
description: Wie jedes Feld des Rerank-Formats von Cohere auf KPP abgebildet wird und zurück.
order: 8
---

Schlagen Sie hier jedes Feld des Rerank-Formats von Cohere nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::cohere_rerank::CohereRerank` (`crates/kaeso-codecs/src/cohere_rerank/`)
- **Endpunkte:** `POST /v1/rerank` und `POST /v2/rerank` als Format einer App ([Rerank-API](../../api/rerank.md)); `POST /v2/rerank` an der Wurzel von Coheres Host als Format eines Anbieters
- **Fixtures:** `crates/kaeso-codecs/fixtures/cohere_rerank/`, aus den Beispielen der Referenz von Cohere und dem Schema, die zwei Antworten auf abgelehnte Schlüssel aufgezeichnet

**Quellen.** Die [Referenz zu Rerank v2](https://docs.cohere.com/reference/rerank) und die [Referenz zu Rerank v1](https://docs.cohere.com/v1/reference/rerank) von Cohere sowie seine Seite zu den [Rerank-Modellen](https://docs.cohere.com/docs/rerank), gelesen am 14.11.2026.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `cohere_rerank`; ein anderer Dialekt lehnt sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine Codec-eigene Markierung in `Extras::form`; `verbatim` bewahrt die Schreibweise `null` deklarierter Schlüssel.

## Anfrage

Eine Rerank-Anfrage wird zu `Request::Rerank(RerankRequest)` dekodiert. Als Format einer App liest der Codec den v2-Body von Cohere und zusätzlich die v1-Felder, die die meisten Rerank-Clients senden; als Format eines Anbieters schreibt er v2.

| Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `RerankRequest.model` typisiert | Pflicht, wie in v2. |
| `query` | `query` typisiert | Pflicht. |
| `documents` als Strings | `documents[i].text` typisiert | |
| `documents` als `{"text"}`-Objekte (v1) | `documents[i].text` typisiert, Form `object` | Andere Schlüssel des Objekts bleiben Extras des Dokuments. An Cohere als Strings geschrieben; ein Dokument mit anderen Schlüsseln wird dort abgelehnt (`document fields`), da v2 nur Text annimmt. |
| `top_n` | `top_n` typisiert | `0` wird abgelehnt (`InvalidRequest`). |
| `max_tokens_per_doc` | `max_tokens_per_doc` typisiert | `0` wird abgelehnt. |
| `return_documents` (v1) | `return_documents` typisiert | Nicht an Cohere geschrieben, dessen v2 kein solches Feld hat; die Antwort nimmt die Dokumente aus der Anfrage der App. |
| `relevant_spans`, `priority`, `rank_fields` und andere Schlüssel | Extras | Unverändert an Cohere gesendet. |
| mehr als 10.000 `documents` | `InvalidRequest` | `limits::MAX_RERANK_DOCUMENTS`, vor dem Dekodieren gezählt. |

`decode_request` lehnt jede andere Art von Anfrage ab, und jeder andere Codec lehnt eine Rerank-Anfrage mit einem Hinweis auf `/v1/rerank` ab. `check()` lehnt ab, was `cross::check_foreign` ablehnt, etwa Extras eines anderen Dialekts, bei der Anfrage und bei jedem Dokument.

## Antwort

| Feld | KPP | Anmerkungen |
|---|---|---|
| `results[].index` | `RerankResult.index` typisiert | Die Position des Dokuments in der Anfrage. |
| `results[].relevance_score` | `relevance_score` typisiert | Unverändert weitergegeben; die von Cohere liegen zwischen 0 und 1. |
| `results[].document` als String oder `{"text"}` | `document` typisiert | v1-Antworten und die anderer Anbieter enthalten es; Form `object` bei einem Objekt. Aus einem anderen Dialekt als `{"text"}` geschrieben und aus der Anfrage der App ergänzt, wenn sie mit `return_documents` danach fragte und der Anbieter keines sandte. |
| `results[].relevant_spans` und andere Schlüssel | Extras des Ergebnisses | |
| `id` | `RerankResponse.id` typisiert | Weggelassen, wenn die Antwort keine hat. |
| `meta.billed_units.input_tokens` | `usage.input_tokens` typisiert | Rerank 5 rechnet Tokens ab. `meta` bleibt in `usage.raw` erhalten und wird unverändert zurückgeschrieben. |
| `meta.billed_units.search_units` | `search_units` typisiert | Rerank 3.5 und 4 rechnen Sucheinheiten ab. |
| `meta` fehlt | Form `no_meta` | Für eine Antwort ohne `meta` wird keines erfunden. |
| unbekannte Schlüssel | Extras | |

Eine Antwort aus einem anderen Dialekt erhält `meta: {"api_version": {"version": "2"}, "billed_units": {…}}` mit ihren Eingabe-Tokens, ihren Sucheinheiten oder beidem.

## Streaming

Der Endpunkt streamt nicht. Der Stream-Encoder gibt für jedes Event `Unsupported` (`streaming`) zurück; der Stream-Decoder meldet ein `StreamEvent::Error` und danach nichts.

## Fehler

Cohere antwortet mit `{"id", "message"}`. Der Status bestimmt die Art: 400 und 422 `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied`, 404 `NotFound`, 429 `RateLimited` mit `retry-after`, 498 (ein gesperrtes Token) `ContentFiltered`, 503 `Overloaded`, 504 `Timeout` und der Rest `ProviderError`. Coheres eigener Body bleibt in `KppError.raw` erhalten und wird unverändert erneut an eine App von Cohere Rerank gesendet. Jeder andere Fehler erreicht die App als `{"id", "message"}` mit der Anfrage-ID des Gateways als `id` und dem Status der Art. Einen Body ohne `message` liest der Codec so wie der Codec von [OpenAI Responses](openai-responses.md#fehler-errorresponse).

## Modelle

Coheres eigene Modellliste wird nicht bedient: `GET /v1/models` antwortet in der Gestalt von OpenAI ([Modelle](openai-chat.md#modelle)) und listet Rerank-Modelle wie jedes Modell.

## Anbieter mit diesem Format

### Cohere

Die Rerank-API von Cohere ist `POST /v2/rerank` an der Wurzel seines Hosts, neben der Kompatibilitäts-API, auf die die Basis-URL von Cohere in Kaeso zeigt (`RerankEgress::at_root`). Seine Rerank-Modelle sind Rerank 5, `rerank-v5.0-pro` und `rerank-v5.0-fast`, abgerechnet pro Token. Rerank 4 und 3.5, abgerechnet pro Sucheinheit, stehen nicht im Cohere-Katalog von KPMR.
