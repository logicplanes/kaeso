# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR builds the catalogue from [models.dev](https://models.dev), the open (MIT-licensed) model database, and applies its reviewed registry: corrections, added and removed models, and what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-09T14:17:43Z-2cd0a457e482` |
| Generated | 2026-11-09T14:17:43Z |
| Content | the providers Kaeso calls (`alibaba`, `anthropic`, `cerebras`, `cohere`, `deepseek`, `elevenlabs`, `gemini-agent-platform`, `google`, `groq`, `minimax`, `mistral`, `moonshotai`, `ollama-cloud`, `openai`, `openrouter`, `togetherai`, `vercel`, `xai`, `zai`); 1419 models |
| SHA-256 | `d00c9b94405ae98cb2fe42dc022b7d81d7d6e68adea8d4333337d7d9cdc61f40` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
