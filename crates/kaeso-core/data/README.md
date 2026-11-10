# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-10T17:35:37Z-68955f075cd2` |
| Generated | 2026-11-10T17:35:37Z |
| Content | the providers Kaeso calls (`alibaba`, `anthropic`, `baseten`, `cerebras`, `cohere`, `deepseek`, `elevenlabs`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `huggingface`, `minimax`, `mistral`, `moonshotai`, `nebius`, `nvidia`, `ollama-cloud`, `openai`, `openrouter`, `sambanova`, `scaleway`, `siliconflow`, `togetherai`, `vercel`, `xai`, `zai`); 1729 models |
| SHA-256 | `6b2500bb3467bccbb7cb70405d6bb63494393e0bbb2b17a78cd095d8923052af` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
