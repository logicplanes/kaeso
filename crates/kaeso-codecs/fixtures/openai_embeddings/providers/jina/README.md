# Fixtures: Jina AI (`jina`) on `openai_embeddings`

Jina AI's embeddings, `POST https://api.jina.ai/v1/embeddings` with a `jina_` key (`Authorization: Bearer`). The body is OpenAI's with Jina's own fields: `embedding_type` in place of `encoding_format`, `task`, `normalized`, `truncate`, `late_chunking` and, per model, more; no `user`, no token ids. The `model` picks the request schema. Jina checks the key before the body, so without a key only the refused-key errors can be recorded.

No key was available. The requests are the examples of Jina's OpenAPI document; the replies are built from its `EmbeddingResponse` schema; the errors were answered live without a valid key, their `request_id` zeroed; the listing is Jina's public `GET /v1/models`, cut to three models.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| OpenAPI document, `info.version` `2026.10.13.1624` (`EmbeddingsV3Request`, `EmbeddingsV5Request`, `EmbeddingResponse`, `EmbeddingUsage`, `ErrorResponse`, `ModelListResponse`) | https://api.jina.ai/openapi.json |
| API reference (authentication, errors, models) | https://docs.jina.ai/api.md |
| Model listing, public | https://api.jina.ai/v1/models |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/v3-doc.json` | `jina-embeddings-v3` with `task`, `late_chunking`, `normalized`, `dimensions` and `embedding_type` | the `EmbeddingsV3Request` example, verbatim |
| `requests/v5-text-doc.json` | `jina-embeddings-v5-text-small` | the first `EmbeddingsV5Request` example, verbatim |
| `requests/query-base64.json` | `embedding_type: "base64"`, as Kaeso writes an OpenAI app's `encoding_format: "base64"` | schema |
| `responses/v5-text-float.json` | a float vector, `usage` with `total_tokens` and `prompt_tokens` | schema; vector cut to three values |
| `responses/query-base64.json` | a base64 vector of three little-endian `float32` values | schema |
| `errors/recorded-no-key-401.json` | `AUTH_MISSING_API_KEY` | live |
| `errors/recorded-invalid-key-401.json` | `AUTH_INVALID_API_KEY` | live, with a made-up key |
| `errors/recorded-basic-auth-401.json` | `AUTH_INVALID_FORMAT`, a key not sent as Bearer | live |
| `models/list-recorded.json` | `GET /v1/models`, OpenRouter's shape, ids as `jina-ai/<id>` | live, cut to three models |

## Where Jina AI differs from OpenAI Embeddings

| Difference | How KPP carries it | Source |
|---|---|---|
| `embedding_type` in place of `encoding_format` | `EmbedRequest.encoding` is sent as `embedding_type` (`EmbeddingsProfile::JINA`); an app that sets both is refused | OpenAPI document |
| no `user` and no token ids in any schema | refused before sending; whether Jina refuses unknown fields itself can't be seen without a key | OpenAPI document |
| `task`, `normalized`, `truncate`, `late_chunking`, `return_multivector`, `input_type` | request extras, sent as written | OpenAPI document |
| inputs as `{"text"}`, `{"image"}`, `{"video"}`, `{"audio"}`, `{"pdf"}` objects | refused by the OpenAI Embeddings codec, which reads strings and token ids only | OpenAPI document |
| `usage.prompt_tokens` counts text only; `image_tokens` and others beside it | `prompt_tokens` is read; the rest stays in raw usage | `EmbeddingUsage` |
| errors are `{"detail", "code", "request_id"}` | an OpenAI app gets them in OpenAI's shape with Jina's text; `code` sets the kind where it says more than the status | live |
| listing ids carry `jina-ai/`, requests don't | `ModelListing::Partial`; `GET /v1/models/{id}` finds a model by its short id | live |

The byte layout of Jina's base64 vectors is not documented; they are passed on as Jina sends them.
