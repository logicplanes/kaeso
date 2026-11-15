# Fixtures: Voyage AI (`voyage`) on `openai_embeddings`

Voyage AI's text embeddings, `POST https://ai.mongodb.com/v1/embeddings` with an Atlas model API key, or `https://api.voyageai.com/v1/embeddings` with a key from Voyage's own dashboard; both answer the same way (recorded 2026-11-14). `Authorization: Bearer`. The body is close to OpenAI's: `output_dimension` in place of `dimensions`, `encoding_format` with `base64` only, Voyage's own `input_type`, `truncation` and `output_dtype`, and no `user`. Voyage refuses every field it doesn't know with a 400, and validates the body before the key.

No key was available. The `published-documents` request and reply are the example of Anthropic's embeddings guide, with the vectors cut to the two values it shows; `query-base64` is built from MongoDB's OpenAPI document; the errors were answered live without a valid key.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Atlas Embedding and Reranking API, OpenAPI document version 1.1 (`/embeddings`) | https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json |
| API overview (hosts, keys, scoped endpoints) | https://www.mongodb.com/docs/voyageai/api-reference/overview/ |
| Error codes (`{"detail"}`, statuses) | https://docs.voyageai.com/docs/error-codes |
| A published request and reply of `voyage-4` | https://platform.claude.com/docs/en/build-with-claude/embeddings |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/published-documents.json`, `responses/published-documents.json` | two string inputs with `input_type: "document"`; the reply's `usage` holds only `total_tokens` | published, vectors cut to the two values shown |
| `requests/query-base64.json`, `responses/query-base64.json` | one string input with `input_type`, `output_dimension`, `output_dtype`, `truncation` and `encoding_format: "base64"`; a base64 vector of three little-endian `float32` values | schema |
| `errors/recorded-openai-fields-400.json` | 400 for OpenAI's `user`, `dimensions` and `encoding_format: "float"`, which Voyage refuses | live, without a key |
| `errors/recorded-bad-input-400.json` | 400 for a token id as input, `encoding_format: "float"` and an unknown field | live, without a key |
| `errors/recorded-invalid-key-401.json` | 401 for a key Voyage doesn't know | live, with a made-up key |
| `errors/recorded-no-key-401.json` | 401 for a request without a key | live |

## Where Voyage AI differs from OpenAI Embeddings

| Difference | How KPP carries it | Source |
|---|---|---|
| `output_dimension`, only 256, 512, 1024 or 2048, in place of `dimensions` | `EmbedRequest.dimensions` is sent as `output_dimension`; another size is refused before sending (`EmbeddingsProfile::VOYAGE`) | OpenAPI document |
| no `user` | refused before sending | live 400 |
| `encoding_format` takes `base64` only | `float`, Voyage's default, is sent as nothing | live 400 |
| no token-id inputs | refused before sending | live 400 |
| `input_type`, `truncation`, `output_dtype` | request extras, sent as written | OpenAPI document |
| `usage` holds only `total_tokens` | read as the input tokens; an OpenAI app gets `prompt_tokens` as well | published reply |
| errors are `{"detail": "…"}` | an OpenAI app gets them in OpenAI's shape with Voyage's text | live |
| no `GET /v1/models` (404) | the catalogue lists Voyage's models from KPMR | live |

The byte order of Voyage's base64 vectors is not documented; the codec reads them as little-endian `float32`, as NumPy writes them on the machines Voyage's SDK runs on. Confirm it by recording with a key.
