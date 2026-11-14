# openai_embeddings / providers / upstage (Upstage)

Upstage, provider id `upstage`, base URL `https://api.upstage.ai`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: https://console.upstage.ai/api/embeddings and the embeddings guide https://console.upstage.ai/docs/capabilities/embed, read on 2026-11-14.

No key was available. The reply is the API reference's single-input example, whose vector the reference cuts with `...`; the four values it shows are kept. Only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/solar-embedding-2-query-doc.json`, `responses/solar-embedding-2-query-doc.json` | one string to `solar-embedding-2-query`; usage with `prompt_tokens` and `total_tokens` | doc |
| `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key` with OpenAI's `error` object and `param: ""` | live, with a made-up key |

The request takes `model` and `input` (a string or up to 100 strings) and documents no `dimensions`, `encoding_format` or `user`; Kaeso refuses `dimensions`, since every vector has 1,024 dimensions. Vectors have 1,024 dimensions and are normalized. Queries and passages are separate models: a search embeds questions with `*-query` and documents with `*-passage`.
