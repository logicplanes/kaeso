# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-18T01:12:57Z-d735d46d5351` |
| Generated | 2026-11-18T01:12:57Z |
| Content | the providers Kaeso calls (`alibaba`, `amazon-bedrock`, `anthropic`, `azure`, `baseten`, `cerebras`, `cloudflare-ai-gateway`, `cohere`, `deepinfra`, `deepseek`, `elevenlabs`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `huggingface`, `ionos`, `jina`, `minimax`, `mistral`, `moonshotai`, `nebius`, `novita-ai`, `nvidia`, `ollama-cloud`, `openai`, `openrouter`, `ovhcloud`, `perplexity`, `sambanova`, `scaleway`, `siliconflow`, `standardcompute`, `stepfun-ai`, `tencent-tokenhub`, `togetherai`, `typesafe`, `upstage`, `venice`, `vercel`, `volcengine`, `voyage`, `xai`, `zai`); 2857 models |
| SHA-256 | `cb3daa4dfc86a96e38e448f6cd633d6463323952005bcbfcb362b20414d22676` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
