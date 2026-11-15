---
title: Cohere Rerank
description: How every field of Cohere's rerank format maps to KPP and back.
order: 8
---

Look up any field of Cohere's rerank format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::cohere_rerank::CohereRerank` (`crates/kaeso-codecs/src/cohere_rerank/`)
- **Endpoints:** `POST /v1/rerank` and `POST /v2/rerank` as an app's format ([Rerank API](../../api/rerank.md)); `POST /v2/rerank` at the root of Cohere's host as a provider's
- **Fixtures:** `crates/kaeso-codecs/fixtures/cohere_rerank/`, from Cohere's reference examples and the schema, with the two refused-key replies recorded

**Sources.** Cohere's [Rerank v2 reference](https://docs.cohere.com/reference/rerank) and [Rerank v1 reference](https://docs.cohere.com/v1/reference/rerank), and its [rerank models](https://docs.cohere.com/docs/rerank) page, read on 2026-11-14.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `cohere_rerank`; another dialect refuses them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker; `verbatim` keeps `null` spellings of declared keys.

## Request

A rerank request decodes to `Request::Rerank(RerankRequest)`. As an app's format the codec reads Cohere's v2 body and the v1 fields most rerank clients send besides; as a provider's format it writes v2.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `RerankRequest.model` typed | Required, as in v2. |
| `query` | `query` typed | Required. |
| `documents` strings | `documents[i].text` typed | |
| `documents` `{"text"}` objects (v1) | `documents[i].text` typed, form `object` | Other keys of the object stay extras of the document. Written to Cohere as strings; a document with other keys is refused there (`document fields`), since v2 takes text only. |
| `top_n` | `top_n` typed | `0` is refused (`InvalidRequest`). |
| `max_tokens_per_doc` | `max_tokens_per_doc` typed | `0` is refused. |
| `return_documents` (v1) | `return_documents` typed | Not written to Cohere, whose v2 has no such field; the reply takes the documents from the app's request. |
| `relevant_spans`, `priority`, `rank_fields` and other keys | extras | Sent to Cohere as written. |
| more than 10,000 `documents` | `InvalidRequest` | `limits::MAX_RERANK_DOCUMENTS`, counted before decoding. |

`decode_request` refuses every other kind of request, and every other codec refuses a rerank request with a hint that names `/v1/rerank`. `check()` refuses what `cross::check_foreign` refuses, such as extras of another dialect, at the request and at each document.

## Response

| Wire field | KPP | Notes |
|---|---|---|
| `results[].index` | `RerankResult.index` typed | The document's position in the request. |
| `results[].relevance_score` | `relevance_score` typed | Passed on unchanged; Cohere's lie between 0 and 1. |
| `results[].document` string or `{"text"}` | `document` typed | v1 replies, and other providers', carry it; form `object` for an object. Written as `{"text"}` from another dialect, and filled in from the app's request when it asked with `return_documents` and the provider sent none. |
| `results[].relevant_spans` and other keys | extras of the result | |
| `id` | `RerankResponse.id` typed | Left out when the reply has none. |
| `meta.billed_units.input_tokens` | `usage.input_tokens` typed | Rerank 5 bills tokens. `meta` is kept in `usage.raw` and written back unchanged. |
| `meta.billed_units.search_units` | `search_units` typed | Rerank 3.5 and 4 bill search units. |
| `meta` absent | form `no_meta` | No `meta` is invented for a reply without one. |
| unknown keys | extras | |

A reply from another dialect gets `meta: {"api_version": {"version": "2"}, "billed_units": {…}}` with its input tokens, its search units, or both.

## Streaming

The endpoint does not stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` and then nothing.

## Errors

Cohere answers `{"id", "message"}`. The status gives the kind: 400 and 422 `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied`, 404 `NotFound`, 429 `RateLimited` with `retry-after`, 498 (a blocked token) `ContentFiltered`, 503 `Overloaded`, 504 `Timeout`, and the rest `ProviderError`. Cohere's own body is kept in `KppError.raw` and sent again unchanged to a Cohere Rerank app. Any other error reaches the app as `{"id", "message"}` with the gateway's request id as `id` and the kind's status. A body without `message` is read as the [OpenAI Responses](openai-responses.md#errors-errorresponse) codec reads it.

## Models

Cohere's own model listing isn't served: `GET /v1/models` answers in OpenAI's shape ([models](openai-chat.md#models)), and lists rerank models like any model.

## Providers that use this format

### Cohere

Cohere's Rerank API is `POST /v2/rerank` at the root of its host, beside the compatibility API Kaeso's Cohere base URL points to (`RerankEgress::at_root`). Its rerank models are Rerank 5, `rerank-v5.0-pro` and `rerank-v5.0-fast`, billed per token. Rerank 4 and 3.5, billed per search unit, are not in KPMR's Cohere catalogue.
