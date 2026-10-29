# Fixtures: Vercel (`vercel`) on `openai_embeddings`

Vercel AI Gateway's Embeddings API, `POST https://ai-gateway.vercel.sh/v1/embeddings`, with `Authorization: Bearer`. Vercel models that models.dev gives `type: "embedding"` use it.

## Sources (read 2026-10-29)

| Source | Address |
|---|---|
| Embeddings | https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/embeddings |

Every live recording was made on 2026-10-29 with a free-tier Vercel account, with `nvidia/nemotron-3.5-lightning-free` (free) or models that cost under $0.0001 a request, and is listed with its model in the `RECORDED.md` of its folder. The free tier refuses paid models such as Claude with 403, and allows 5 requests a minute to the free model. Vercel's `provider_metadata` (or `providerMetadata`) carries `gateway.routing.clientSessionId`, a fingerprint of the client; it is replaced with zeros and nothing else is changed. Generation ids (`gen_…`) and the upstream provider's request ids are kept as recorded.

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/embedding.json` | Two inputs with `dimensions: 8` | written for recording |
| `requests/unknown-model.json` | An unknown model | written for recording |
| `responses/recorded-embedding.json` | Two vectors of 8 floats, `usage` with `prompt_tokens` and `total_tokens`, and top-level `providerMetadata` | live, `google/gemini-embedding-001` |
| `errors/recorded-unknown-model.json` | 404 `model_not_found` | live |

`providerMetadata` stays in Extras and round-trips unchanged. Some embedding models answer a free-tier account with 403, such as `openai/text-embedding-3-small`.
