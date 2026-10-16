# `fixtures/openai_embeddings/requests`

| file | what it covers | provenance |
|---|---|---|
| `base64-batch.json` | base64 with several inputs, `text-embedding-3-large` | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `base64-encoding.json` | `encoding_format: base64` (little-endian f32, 8 dims) | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `large-model-float.json` | `text-embedding-3-large` with float output | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `string-array.json` | array of strings, `dimensions: 8`, `encoding_format` omitted (default float) | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `string-input.json` | string input, `encoding_format: float`, `dimensions: 3` | doc: https://developers.openai.com/api/reference/resources/embeddings/methods/create (example request, same as openapi.yaml createEmbedding) |
| `token-arrays.json` | array of token-id arrays | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `token-ids.json` | single array of token ids | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
| `unknown-top-level-field.json` | MADE-UP top-level field `x_future_field` (Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `user-id.json` | `user` end-user id | schema: CreateEmbeddingRequest (openapi.yaml@e95c0fe615f4) |
