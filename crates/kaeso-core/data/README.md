# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-19T05:40:21Z-89f750d585e7` |
| Generated | 2026-11-19T05:40:21Z |
| Content | the providers Kaeso calls (`alibaba`, `amazon-bedrock`, `anthropic`, `azure`, `baseten`, `cartesia`, `cerebras`, `cloudflare-ai-gateway`, `cohere`, `deepgram`, `deepinfra`, `deepseek`, `elevenlabs`, `fal`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `huggingface`, `ionos`, `jina`, `minimax`, `mistral`, `moonshotai`, `nebius`, `novita-ai`, `nvidia`, `ollama-cloud`, `openai`, `openrouter`, `ovhcloud`, `perplexity`, `qianfan`, `replicate`, `sambanova`, `scaleway`, `siliconflow`, `standardcompute`, `stepfun-ai`, `tencent-tokenhub`, `togetherai`, `typesafe`, `upstage`, `venice`, `vercel`, `volcengine`, `voyage`, `xai`, `zai`); 2931 models |
| SHA-256 | `13fadc72210fb3ddf1441d61f315c80529168308353aaa8112eb5e8a01de05c4` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
