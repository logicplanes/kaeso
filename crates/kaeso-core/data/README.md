# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-14T15:52:53Z-3dcacffe8a1d` |
| Generated | 2026-11-14T15:52:53Z |
| Content | the providers Kaeso calls (`alibaba`, `anthropic`, `baseten`, `cerebras`, `cloudflare-ai-gateway`, `cohere`, `deepinfra`, `deepseek`, `elevenlabs`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `huggingface`, `minimax`, `mistral`, `moonshotai`, `nebius`, `nvidia`, `ollama-cloud`, `openai`, `openrouter`, `perplexity`, `sambanova`, `scaleway`, `siliconflow`, `standardcompute`, `togetherai`, `typesafe`, `vercel`, `xai`, `zai`); 2113 models |
| SHA-256 | `258a00daa1c9b4c78b7bc6b48107433e3cff2dff94e2ea588c2743b7e2da4a42` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
