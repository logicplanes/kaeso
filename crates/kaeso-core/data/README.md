# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR builds the catalogue from [models.dev](https://models.dev), the open (MIT-licensed) model database, and applies its reviewed registry: corrections, added and removed models, and what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-05T15:32:03Z-3da8fa6af2e5` |
| Generated | 2026-11-05T15:32:03Z |
| Content | the providers Kaeso calls (`alibaba`, `anthropic`, `cerebras`, `cohere`, `deepseek`, `groq`, `mistral`, `moonshotai`, `ollama-cloud`, `openai`, `openrouter`, `togetherai`, `vercel`, `xai`); 1060 models |
| SHA-256 | `48fd1b3e24cdaedfbd23616e1560fc0bac7f695ff23232126b40ae1ade8aedf3` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
