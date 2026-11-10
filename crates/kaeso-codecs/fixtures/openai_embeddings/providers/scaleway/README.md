# openai_embeddings / providers / scaleway (Scaleway)

Scaleway Generative APIs, provider id `scaleway`, base URL `https://api.scaleway.ai`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: https://www.scaleway.com/en/developers/api/generative-apis/ (`CreateEmbeddingResponse`) and the guide https://www.scaleway.com/en/docs/generative-apis/how-to/query-embedding-models/, read on 2026-11-09.

No key was available. The reply is built from the reference, which adds `id`, `created` and `usage.completion_tokens: 0` to OpenAI's reply; only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/bge-multilingual-gemma2.json`, `responses/bge-multilingual-gemma2.json` | one string to `bge-multilingual-gemma2`; `id`, `created` and `completion_tokens` stay in `Extras` | doc |
| `errors/recorded-invalid-api-key.json` | 403 `forbidden` with a numeric `code` | live, with a made-up key |

`dimensions` takes only a model's largest size, the reference says, so Scaleway answers other values itself.
