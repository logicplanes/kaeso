# openai_embeddings / providers / deepinfra (DeepInfra)

DeepInfra, provider id `deepinfra`, base URL `https://api.deepinfra.com`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: DeepInfra's OpenAPI document https://api.deepinfra.com/openapi.json (`OpenAIEmbeddingsIn`) and the embeddings guide https://docs.deepinfra.com/apis/embeddings.md, read on 2026-11-14.

No key was available. The reply is built from the guide, whose usage has `prompt_tokens` and `total_tokens`; only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/bge-m3.json`, `responses/bge-m3.json` | two strings to `BAAI/bge-m3` with `encoding_format: "float"` | doc |
| `requests/nemotron-query-doc.json` | the guide's query to `nvidia/Nemotron-3.5-Embed-1B-BF16` with DeepInfra's `input_type`, which stays in `Extras` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key` with OpenAI's `error` object | live, with a made-up key |

DeepInfra's schema adds `input_type`, `task`, `service_tier` and `fail_fast` to OpenAI's fields, and takes `dimensions` from 32 up, padding a vector beyond the model's size with zeros. Its multimodal models take content-part lists as items of `input`, which an OpenAI Embeddings app sends as written.
