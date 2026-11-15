# openai_embeddings / providers / venice (Venice)

Venice, provider id `venice`, base URL `https://api.venice.ai/api`, `POST /v1/embeddings` with `Authorization: Bearer`. References: Venice's OpenAPI document https://docs.venice.ai/swagger.yaml (`CreateEmbeddingRequestSchema`, `additionalProperties: false`) and embeddings guide https://docs.venice.ai/guides/features/embeddings, read on 2026-11-14.

No key was available. The request and the reply were recorded against Venice by the cassette tests of [Rig](https://github.com/0xPlaygrounds/rig) and are published in its repository ([`embeddings/embeddings_smoke.yaml`](https://github.com/0xPlaygrounds/rig/blob/46b86a65b2b23a963f36deed384e0869da30931d/crates/rig-cassette/fixtures/cassettes/venice/embeddings/embeddings_smoke.yaml), commit `46b86a65b2b23a963f36deed384e0869da30931d`), the vectors cut to their first three values; the key error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/qwen3-0-6b-published.json`, `responses/qwen3-0-6b-published.json` | three strings to `text-embedding-qwen3-0-6b`; OpenAI's reply shape without a `cost` | published (Rig), vectors cut |
| `errors/recorded-invalid-api-key.json` | 401 `{"error": "Authentication failed"}` | live, with a made-up key |
| `errors/unknown-model-published.json` | 404 `{"error": "Specified model not found: …"}` | published (Rig, `embedding_matrix/error_preserves_provider_body.yaml`) |

`dimensions` works only on the models with custom dimensions, such as `text-embedding-qwen3-8b`, and Venice refuses unknown fields and token arrays as input with 400 itself.
