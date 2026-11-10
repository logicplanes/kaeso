# openai_embeddings / providers / siliconflow (SiliconFlow)

SiliconFlow, provider id `siliconflow`, base URL `https://api.siliconflow.com`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: https://docs.siliconflow.com/en/api-reference/embeddings/create-embeddings (`EmbeddingsRequest`, `EmbeddingsResponse`), read on 2026-11-10.

Not recorded from a successful call. The request is the reference's example with `dimensions`, which only the Qwen3 embeddings models take; the reply is built from the response schema, whose `usage` always has `completion_tokens`. Only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/qwen3-embedding.json`, `responses/qwen3-embedding.json` | one string to `Qwen/Qwen3-Embedding-8B` with `dimensions: 1024`; `completion_tokens: 0` stays in `Extras` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `{"code": 30014, "data": null, "message": "Token is invalid."}`, without OpenAI's `error` object | live, with a made-up key |

The models are `Qwen/Qwen3-Embedding-8B`, `-4B` and `-0.6B`, `BAAI/bge-m3`, `BAAI/bge-large-en-v1.5`, `BAAI/bge-large-zh-v1.5` and `netease-youdao/bce-embedding-base_v1`. A request takes at most 32 strings.
