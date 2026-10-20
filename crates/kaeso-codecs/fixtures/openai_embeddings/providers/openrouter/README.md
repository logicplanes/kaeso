# Fixtures: OpenRouter (`openrouter`) on `openai_embeddings`

OpenRouter's Embeddings API, `POST https://openrouter.ai/api/v1/embeddings`, with `Authorization: Bearer`. Kaeso's base URL is `https://openrouter.ai/api`.

Source: https://openrouter.ai/docs/api_reference/embeddings, read 2026-10-19. Every recording used the free model `nvidia/nemotron-3-embed-1b:free` and is listed in the `RECORDED.md` of its folder.

| File | What it covers | Provenance |
|---|---|---|
| `requests/free-embedding.json` | One string, `encoding_format: "float"` | written for recording |
| `requests/free-embedding-base64.json` | Two strings, `encoding_format: "base64"` | written for recording |
| `requests/free-embedding-dimensions.json` | `dimensions`, which this model refuses | written for recording |
| `responses/recorded-free-embedding.json` | A 2048-dimension vector; `model` names the served model (`private/openrouter/…`), not the requested id; `usage.cost`, `usage.is_byok`, `usage.cost_details`; top-level `provider` and `id` | live |
| `responses/recorded-free-embedding-base64.json` | Two base64 vectors with the same extra fields | live |
| `errors/recorded-free-embedding-dimensions.json` | 400 whose `message` wraps the upstream provider's own error body as text | live |

The extra reply fields stay in `openai_embeddings` Extras and round-trip unchanged.
