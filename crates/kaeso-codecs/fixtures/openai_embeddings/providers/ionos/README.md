# openai_embeddings / providers / ionos (IONOS AI Model Hub)

IONOS AI Model Hub, provider id `ionos`, base URL `https://openai.inference.de-txl.ionos.com`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/embeddings/create-embeddings and the BGE M3 model page https://docs.ionos.com/cloud/ai/ai-model-hub/models/embedding-models/bge-m3, read on 2026-11-14.

No token was available. The reply is the model page's example; only the error was answered live, with a made-up token.

| file | what it covers | provenance |
|---|---|---|
| `requests/bge-m3-doc.json`, `responses/bge-m3-doc.json` | one string to `BAAI/bge-m3`; usage with `prompt_tokens` and `total_tokens` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `paas-token-1` in IONOS's own `messages` shape | live, with a made-up token |

IONOS's request takes `encoding_format`, a model-specific `dimensions` (it answers an unsupported value with 400 itself) and `user`, plus `messages` for one multimodal input on `Qwen/Qwen3-VL-Embedding-8B`, which OpenAI's format doesn't have.
