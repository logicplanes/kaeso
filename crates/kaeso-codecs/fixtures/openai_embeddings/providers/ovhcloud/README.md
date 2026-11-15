# openai_embeddings / providers / ovhcloud (OVHcloud AI Endpoints)

OVHcloud AI Endpoints, provider id `ovhcloud`, base URL `https://oai.endpoints.kepler.ai.cloud.ovh.net`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: the OpenAPI document of `bge-m3` on the unified endpoint, https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/bge-m3/openapi.json (`EmbeddingParameters`), read on 2026-11-14.

AI Endpoints answers without a key, at 2 requests a minute per IP and model, so the reply and the refusal of `dimensions` are real ones, sent without a key on 2026-11-14 and saved unchanged; the error was answered live with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/recorded-qwen3-dims.json`, `responses/recorded-qwen3-dims.json` | two strings to `Qwen3-Embedding-8B` with `dimensions: 32` and `encoding_format: "float"`: 32 values each, usage with `completion_tokens: 0`, no `id` | live, without a key |
| `requests/recorded-bge-m3-dimensions.json`, `errors/recorded-bge-m3-dimensions.json` | `dimensions: 256` to `bge-m3`, whose size is fixed: 400 "feature 'dimensions' is not currently supported" | live, without a key |
| `requests/recorded-invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 403 with only a `message`, "Forbidden: authentication failed." | live, with a made-up key |

The schema declares `input`, `model`, `encoding_format` (`float` or `base64`), `dimensions` and `user`. `Qwen3-Embedding-8B` takes `dimensions` from 32 to 4096; `bge-m3` always answers 1024 values and refuses `dimensions` with 400, so Kaeso sends the field as written (`EmbeddingsProfile::OPENAI`).
