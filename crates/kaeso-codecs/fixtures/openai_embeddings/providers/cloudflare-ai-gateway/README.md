# openai_embeddings / providers / cloudflare-ai-gateway (Cloudflare AI Gateway)

Workers AI's embedding models through Cloudflare's REST API, provider id `cloudflare-ai-gateway`, base URL `https://api.cloudflare.com`, `POST /client/v4/accounts/{account_id}/ai/v1/embeddings` with `Authorization: Bearer` and a Cloudflare API token with Workers AI read access. Source: Workers AI's [OpenAI compatibility](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/) page and its [model pages](https://developers.cloudflare.com/workers-ai/models/), read on 2026-11-14; neither documents the reply of this path, so every file here was recorded live on 2026-11-14 with the account id replaced by `0123456789abcdef0123456789abcdef` and the error ids by zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/recorded-bge-m3.json`, `responses/recorded-bge-m3.json` | two strings to `@cf/baai/bge-m3`; the reply has `object`, `data` and `model` and no `usage` (the neurons come only in the `cf-ai-neurons` header); vectors cut to four values | live, without a gateway; the same request with `cf-aig-gateway-id` answered the same shape with the gateway's `cf-aig-*` headers |
| `requests/recorded-unknown-model.json`, `errors/recorded-unknown-model.json` | 400 code 5007 "No such model" in Cloudflare's `errors` envelope | live |
| `requests/recorded-embeddinggemma-input.json`, `errors/recorded-embeddinggemma-input.json` | 400 code 5006 for `@cf/google/embeddinggemma-2`, whose own schema takes `input` but which this path refuses for lack of it, so KPMR doesn't list it | live |
| `errors/recorded-invalid-api-key.json` | 401 code 10000 "Authentication error" for a made-up token | live |

What the path does with OpenAI's fields, recorded live: `dimensions` is ignored (256 asked, 1024 answered by `@cf/baai/bge-m3` and `@cf/qwen/qwen3-embedding-0.6b`), so the provider's embeddings profile refuses it; `encoding_format: "base64"` is answered with floats, which the codec gives the app in the encoding it asked for; `user` is accepted.
