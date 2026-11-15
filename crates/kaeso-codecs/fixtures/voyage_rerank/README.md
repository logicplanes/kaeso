# Fixtures: `voyage_rerank`

Wire bodies for Voyage AI's rerank API, `POST /v1/rerank` on `https://ai.mongodb.com` (Atlas model API keys) or `https://api.voyageai.com` (keys from Voyage's own dashboard): a query and documents as strings, answered with `data` sorted by `relevance_score` and `usage.total_tokens`.

## Sources

- MongoDB's OpenAPI document of the Atlas Embedding and Reranking API, version 1.1, read on 2026-11-14 (https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json, `rerankDocuments`): `query`, `documents` (strings, at most 1,000), `model`, `top_k`, `return_documents`, `truncation`; the reply's `object`, `data[].index`, `.relevance_score`, `.document`, `model` and `usage.total_tokens`.
- Voyage's error codes, read on 2026-11-14: https://docs.voyageai.com/docs/error-codes.
- `requests/basic-live.json` is the body sent live on 2026-11-14, answered with a 401 for a made-up key. The errors marked live were answered on 2026-11-14 without a key or with a made-up one; Voyage validates the body before the key. No key was available, so every reply is built from the schema (`schema:`).

| file | what it covers | provenance |
|---|---|---|
| `requests/basic-live.json`, `responses/basic-live.json` | the smallest request; scores without documents | live request; reply schema |
| `requests/top-k-documents.json`, `responses/top-k-documents.json` | `top_k`, `return_documents` and `truncation`; documents in the reply as strings | schema |
| `errors/recorded-cohere-fields-400.json`, `.meta.json` | 400 for Cohere's `top_n` and `max_tokens_per_doc` | live, without a key |
| `errors/recorded-object-documents-400.json`, `.meta.json` | 400 for a document written as `{"text"}` | live, without a key |
| `errors/recorded-invalid-key-401.json`, `.meta.json` | 401 for a key Voyage doesn't know | live, with a made-up key |
| `errors/recorded-no-models-404.json`, `.meta.json` | 404 for `GET /v1/models`, which Voyage doesn't serve | live, without a key |
| `errors/rate-limit-429.json`, `.meta.json` | 429 with `retry-after` | schema: 429 `detail`; wording and header built |

## Doubts

- Scores' range is not documented, and the replies' scores are built.
- The 429 is worded by Kaeso: Voyage documents the status and the `detail` field, not the text or a `retry-after` header.
