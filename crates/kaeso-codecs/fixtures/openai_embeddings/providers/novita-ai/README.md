# openai_embeddings / providers / novita-ai (Novita AI)

Novita AI, provider id `novita-ai`, base URL `https://api.novita.ai/openai`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: Novita AI's create embeddings reference https://docs.novita.ai/api-reference/model-apis-llm-create-embeddings.md, read on 2026-11-14.

No key was available. The request and the reply are built from the reference, which lists the fields of both but shows no example; only the error was answered live, without a key.

| file | what it covers | provenance |
|---|---|---|
| `requests/bge-m3.json`, `responses/bge-m3.json` | two strings to `baai/bge-m3` with `encoding_format: "float"`; `usage` with `prompt_tokens` and `total_tokens` | built from the schema |
| `errors/recorded-missing-api-key.json` | 403 `INVALID_API_KEY` in Novita AI's own `{"code", "reason", "message", "metadata"}` | live |

The reference takes `input` (a string or an array), `model` and `encoding_format` (`float` or `base64`) and names no `dimensions` or `user`; Kaeso refuses `dimensions` (`without_dimensions()`). Its embeddings models are `baai/bge-m3` and `qwen/qwen3-embedding-8b`; Novita AI's default model listing leaves them out (`GET /v1/models/{model}` answers for them).
