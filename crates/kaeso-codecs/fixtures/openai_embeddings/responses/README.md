# `fixtures/openai_embeddings/responses`

| file | what it covers | provenance |
|---|---|---|
| `base64-batch.json` | base64 with several inputs, `text-embedding-3-large` | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `base64-encoding.json` | `encoding_format: base64` (little-endian f32, 8 dims) | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `large-model-float.json` | `text-embedding-3-large` with float output | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `string-array.json` | array of strings, `dimensions: 8`, `encoding_format` omitted (default float) | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `string-input.json` | single float embedding, usage | doc: https://developers.openai.com/api/reference/resources/embeddings/methods/create (example response, verbatim) |
| `token-arrays.json` | array of token-id arrays | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `token-ids.json` | single array of token ids | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `user-id.json` | `user` end-user id | schema: CreateEmbeddingResponse, Embedding (openapi.yaml@e95c0fe615f4); vectors are random unit vectors printed as shortest float32 decimals like the doc example |
| `unknown-objects.json` | MADE-UP `object` values on the list and on the item, and a base64 vector | schema: CreateEmbeddingResponse + invented values (unknown-* fixture) |
| `without-usage.json` | `string-input.json` without `usage`, as some OpenAI-compatible providers answer; the codec keeps the absence in form `no_usage` and the gateway estimates the input | schema: CreateEmbeddingResponse with the `usage` key removed (constructed) |
