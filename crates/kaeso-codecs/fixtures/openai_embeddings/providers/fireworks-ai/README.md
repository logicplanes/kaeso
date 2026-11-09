# openai_embeddings / providers / fireworks-ai (Fireworks AI)

Fireworks AI, provider id `fireworks-ai`, base URL `https://api.fireworks.ai/inference`, `POST /v1/embeddings` with `Authorization: Bearer`. API reference: https://docs.fireworks.ai/api-reference/creates-an-embedding-vector-representing-the-input-text and the guide https://docs.fireworks.ai/guides/querying-embeddings-models, read on 2026-11-09.

No key was available. The reply is built from the reference's `CreateEmbeddingResponse` (`data[].index`, `data[].object`, `data[].embedding`, `model`, `object`, `usage`); only the error was answered live, with a made-up key. Its request id is replaced with zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/qwen3-embedding.json`, `responses/qwen3-embedding.json` | two strings to `fireworks/qwen3-embedding-8b`, the serverless model the guide recommends, with `dimensions` (it is resizable) | doc |
| `requests/nomic-prompt-template.json` | a legacy serverless model by its Hugging Face id, with Fireworks' own `normalize`; such fields stay request `Extras` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `UNAUTHORIZED` with a top-level `request_id` | live, with a made-up key |

Fireworks takes no `encoding_format` in its schema; the codec answers an app that asks for base64 by encoding the floats itself, as for Mistral.
