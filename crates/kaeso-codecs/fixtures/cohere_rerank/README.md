# Fixtures: `cohere_rerank`

Wire bodies for Cohere's Rerank API v2, `POST /v2/rerank`, which Kaeso also serves as `POST /v1/rerank`: a query and documents, answered with the documents' scores, best first.

## Sources

- Cohere's API reference, read on 2026-11-14: https://docs.cohere.com/reference/rerank (v2: `model`, `query`, `documents` as strings, `top_n`, `max_tokens_per_doc`, `relevant_spans`, `priority`; the reply's `results`, `id` and `meta` with `api_version`, `billed_units` and `tokens`; the error statuses and `message`) and https://docs.cohere.com/v1/reference/rerank (v1: documents as `{"text"}` objects, `return_documents`, `rank_fields`, results with `document` and `meta.billed_units.search_units`).
- The `context-focus` request and response are the v2 reference's example verbatim, and `responses/search-units.json` is the v1 reference's example reply verbatim (`doc:`). The two `401` errors were recorded live on 2026-11-14 against `https://api.cohere.ai/v2/rerank` without a key and with the made-up key `kaeso-fake-key-0000`; their `id` is zeroed. Everything else is built from the schema (`schema:`).

The layout and the provenance marks (`doc:`, `sdk:`, `schema:`) are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures), and the rules every constructed fixture follows in [Fixture conventions](https://docs.kaeso.app/development/kpp/fixture-conventions).

| file | what it covers | provenance |
|---|---|---|
| `requests/context-focus.json` | v2 request with `top_n` and `relevant_spans` (Rerank 5's Context Focus) | doc: v2 reference example |
| `responses/context-focus.json` | v2 reply with `relevant_spans` and `meta.billed_units.input_tokens` | doc: v2 reference example |
| `requests/search-units.json` | the v1 example's request as v2 writes it: its documents as strings | schema: v1 reference example, documents flattened |
| `responses/search-units.json` | reply billed in search units (Rerank 3.5 and 4) | doc: v1 reference example reply |
| `requests/max-tokens-per-doc.json` | `max_tokens_per_doc` and `priority` | schema: v2 `max_tokens_per_doc`, `priority` |
| `responses/max-tokens-per-doc.json` | reply with `meta.tokens` beside `billed_units` | schema: v2 `meta.tokens`; scores built |
| `requests/object-documents.json` | documents as `{"text"}` objects and `return_documents`, the v1 fields Kaeso's `/v1/rerank` reads | schema: v1 `documents`, `return_documents` |
| `responses/object-documents.json` | results with `document: {"text"}` | schema: v1 `RerankResponse.results[].document` |
| `errors/invalid-api-key-401.json`, `.meta.json` | a key Cohere doesn't know | live, 2026-11-14, with a made-up key |
| `errors/no-api-key-401.json`, `.meta.json` | a request without a key | live, 2026-11-14 |
| `errors/invalid-request-400.json`, `.meta.json` | a body Cohere can't take | schema: 400 `message`; wording built |
| `errors/rate-limit-429.json`, `.meta.json` | too many requests, with `retry-after` | schema: 429 `message`; wording and header built |
| `errors/unavailable-503.json`, `.meta.json` | Cohere unavailable | schema: 503 `message`; wording built |

## Round trip

Every request, response and error round-trips as an equal JSON value, except `requests/object-documents.json`: Cohere's v2 API takes documents as strings and has no `return_documents`, so the request Kaeso sends Cohere writes each document's text and leaves `return_documents` out; the reply then carries the documents from the app's request (`tests.rs`, `object_documents_reach_cohere_as_strings_and_come_back_from_the_request`).

## Doubts

- The 400, 429 and 503 bodies are worded by Kaeso: Cohere's reference names the status and the `message` field, not the text. Confirm them by recording with a real key.
- Whether Cohere's v2 API refuses a document written as an object, or `return_documents`, is not documented; Kaeso sends neither.
