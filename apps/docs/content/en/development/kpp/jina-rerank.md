---
title: Jina Rerank
description: How every field of Jina AI's rerank format maps to KPP and back.
order: 8
---

Look up any field of Jina AI's rerank format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::jina_rerank::JinaRerank` (`crates/kaeso-codecs/src/jina_rerank/`)
- **Endpoint:** `POST /v1/rerank` as Jina AI's format, at `https://api.jina.ai` ([supported providers](../../concepts/supported-providers.md)). Apps rerank in [Cohere's format](cohere-rerank.md); no app endpoint takes this one.
- **Fixtures:** `crates/kaeso-codecs/fixtures/jina_rerank/`, the requests from Jina's OpenAPI examples, replies built from the schema, and the refused-key errors answered live

**Sources.** Jina AI's [OpenAPI document](https://api.jina.ai/openapi.json) (`info.version` `2026.10.13.1624`: `RerankerV3Request`, `TextRerankerRequest`, `RerankingResponse`, `ErrorResponse`) and [API reference](https://docs.jina.ai/api.md), read on 2026-11-14.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `jina_rerank`; another dialect refuses them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker.

## Request

The `model` picks the request schema. Jina AI checks the key before the body, so whether it refuses fields it doesn't know couldn't be recorded.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `RerankRequest.model` typed | |
| `query` | `query` typed | A string; `jina-reranker-m0`'s image queries have no place in KPP and are refused. |
| `documents` strings | `documents[i].text` typed | |
| `documents` `{"text"}` objects | `documents[i].text` typed, form `object` | Written back as objects to Jina; from Cohere they go as strings. A document with fields beside `text`, or an `{"image"}`, is refused. |
| `top_n` | `top_n` typed | |
| `return_documents` | `return_documents` typed | Jina's default is true, Cohere's false: a Jina body without it decodes as true, and the encoder always writes it. |
| `max_doc_length` | `max_tokens_per_doc` typed | `jina-reranker-v3` and `jina-reranker-v3.5` only; for the other models `max_tokens_per_doc` is refused. |
| `return_embeddings` and other keys | extras | Read from a Jina body; no app format writes them. |

`check()` refuses what `cross::check_foreign` refuses, such as Cohere's `relevant_spans` and `priority`.

## Response

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `model` typed | |
| `object` | form `object_name` when not `"list"`, `no_object` when absent | |
| `results[].index` | `RerankResult.index` typed | |
| `results[].relevance_score` | `relevance_score` typed | Passed on unchanged; Jina documents no range. |
| `results[].document` string or `{"text"}` | `document` typed, form `object` for an object | Echoed in the form it was sent; a Cohere app gets `{"text"}`. An `{"image"}` is refused as a reply Kaeso can't read. |
| `results[].embedding` and other keys | extras of the result | |
| `usage.total_tokens` | `usage.input_tokens` typed | `usage` is kept in `usage.raw`; `no_usage` when absent. |
| unknown keys | extras | |

## Streaming

The endpoint does not stream, as for [Cohere Rerank](cohere-rerank.md#streaming).

## Errors

Jina AI answers `{"detail", "code", "request_id"}`, with `errors` on a 422. The code sets the kind where it says more than the status: `AUTHZ_INSUFFICIENT_BALANCE` (403) `Billing`, `INPUT_TOKEN_LIMIT_EXCEEDED` (400) `ContextTooLong`, `INPUT_MODEL_NOT_FOUND` (400) `NotFound`; otherwise the status does, as for [Voyage Rerank](voyage-rerank.md#errors). A body without `detail` is read as the [OpenAI Responses](openai-responses.md#errors-errorresponse) codec reads it. A Cohere app gets the text as `message`. The [OpenAI Embeddings](openai-embeddings.md#jina-ai) codec reads Jina's errors the same way.

## Models

Jina AI's `GET /v1/models` is public and in OpenRouter's shape, with ids as `jina-ai/<id>`; `GET /v1/models/{id}` finds a model by its short id. The codec reads and writes listings in OpenAI's shape ([models](openai-chat.md#models)).

## Providers that use this format

### Jina AI

Jina AI's rerankers at `/v1/rerank` under the provider's base URL (`RerankEgress::of`): `jina-reranker-v3.5`, `jina-reranker-v3`, `jina-reranker-m0` (with text), `jina-reranker-v2-base-multilingual`, `jina-reranker-v1-base-en`, `-turbo-en` and `-tiny-en`, and the ColBERT models `jina-colbert-v2` and `jina-colbert-v1-en`.
