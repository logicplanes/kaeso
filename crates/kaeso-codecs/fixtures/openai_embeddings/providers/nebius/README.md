# openai_embeddings / providers / nebius (Nebius Token Factory)

Nebius Token Factory, provider id `nebius`, base URL `https://api.tokenfactory.nebius.com`, `POST /v1/embeddings` with `Authorization: Bearer`. API reference: https://docs.tokenfactory.nebius.com/api-reference/inference/create-embeddings (`EmbeddingRequest`, `EmbeddingResponse`) and the example https://docs.tokenfactory.nebius.com/api-reference/examples/create-embeddings, read on 2026-11-09.

No key was available. The reply is built from the reference's `EmbeddingResponse`, which always carries `service_tier`; only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/qwen3-embedding.json`, `responses/qwen3-embedding.json` | two strings to `Qwen/Qwen3-Embedding-8B` with `dimensions`; `service_tier` on the reply stays in `Extras` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `unauthenticated` with OpenAI's `error` object and a top-level `detail` | live, with a made-up key |
