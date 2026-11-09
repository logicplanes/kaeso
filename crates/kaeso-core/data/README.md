# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-09T17:36:44Z-d475e9d55817` |
| Generated | 2026-11-09T17:36:44Z |
| Content | the providers Kaeso calls (`alibaba`, `anthropic`, `cerebras`, `cohere`, `deepseek`, `elevenlabs`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `minimax`, `mistral`, `moonshotai`, `nebius`, `ollama-cloud`, `openai`, `openrouter`, `sambanova`, `scaleway`, `togetherai`, `vercel`, `xai`, `zai`); 1509 models |
| SHA-256 | `14fb6e5646627a7674b94c08ab3c4c3af5b089f986306865683c89a6a54dc73d` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
