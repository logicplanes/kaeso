---
title: Voyage Rerank
description: How every field of Voyage AI's rerank format maps to KPP and back.
order: 8
---

Look up any field of Voyage AI's rerank format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::voyage_rerank::VoyageRerank` (`crates/kaeso-codecs/src/voyage_rerank/`)
- **Endpoint:** `POST /v1/rerank` as Voyage AI's format, at `https://ai.mongodb.com` or another Voyage AI host ([supported providers](../../concepts/supported-providers.md)). Apps rerank in [Cohere's format](cohere-rerank.md); no app endpoint takes this one.
- **Fixtures:** `crates/kaeso-codecs/fixtures/voyage_rerank/`, the request sent live, replies built from the schema, and the errors answered live without a valid key

**Sources.** MongoDB's [OpenAPI document](https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json) of the Atlas Embedding and Reranking API (version 1.1, `rerankDocuments`), Voyage AI's [error codes](https://docs.voyageai.com/docs/error-codes) and MongoDB's [billing page](https://www.mongodb.com/docs/voyageai/management/billing/), read on 2026-11-14.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `voyage_rerank`; another dialect refuses them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker.

## Request

Voyage AI refuses every field it doesn't know with a 400, and validates the body before the key (recorded 2026-11-14). The encoder writes only the fields below.

| Wire field | KPP | Notes |
|---|---|---|
| `query` | `RerankRequest.query` typed | |
| `documents` strings | `documents[i].text` typed | Strings only: a document an app wrote as `{"text"}` goes as its text, and one with fields beside `text` is refused (`document fields`). |
| `model` | `model` typed | |
| `top_k` | `top_n` typed | Cohere's `top_n`; Voyage AI refuses `top_n` itself. |
| `return_documents` | `return_documents` typed | Sent only when the app set it; Voyage AI's default is false, as Cohere's. |
| `truncation` and other keys | extras | Read from a Voyage AI body; no app format writes them. |
| `max_tokens_per_doc` | refused | Voyage AI cuts each document at the model's limit and refuses the field. |

`check()` refuses what `cross::check_foreign` refuses, such as Cohere's `relevant_spans` and `priority`.

## Response

| Wire field | KPP | Notes |
|---|---|---|
| `object` | form `object` when not `"list"` | |
| `data[].index` | `RerankResult.index` typed | Voyage AI's key is `data`, not Cohere's `results`. |
| `data[].relevance_score` | `relevance_score` typed | Passed on unchanged; Voyage AI documents no range. |
| `data[].document` | `document` typed | A string; a Cohere app gets it as `{"text"}`. |
| `model` | `model` typed | |
| `usage.total_tokens` | `usage.input_tokens` typed | Voyage AI bills the query's tokens once per document plus every document's tokens and reports that sum. `usage` is kept in `usage.raw`. |
| `usage` absent | form `no_usage` | |
| unknown keys | extras | |

## Streaming

The endpoint does not stream, as for [Cohere Rerank](cohere-rerank.md#streaming).

## Errors

Voyage AI answers `{"detail": "<text>"}` with no code. The status gives the kind: 400 and 422 `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied`, 404 `NotFound`, 413 `RequestTooLarge`, 429 `RateLimited` with `retry-after`, 503 `Overloaded`, 408 and 504 `Timeout`, and the rest `ProviderError`. A body without `detail` is read as the [OpenAI Responses](openai-responses.md#errors-errorresponse) codec reads it. A Cohere app gets the text as `message`.

## Models

Voyage AI serves no model listing: `GET /v1/models` and `GET /v1/models/{model}` answer 404 (recorded 2026-11-14), so **Check availability** calls its models inconclusive. The codec reads and writes listings in OpenAI's shape ([models](openai-chat.md#models)).

## Providers that use this format

### Voyage AI

Voyage AI's rerankers, `rerank-3`, `rerank-3-lite`, `rerank-2.5`, `rerank-2.5-lite`, `rerank-2` and `rerank-2-lite`, at `/v1/rerank` under the provider's base URL (`RerankEgress::of`).
