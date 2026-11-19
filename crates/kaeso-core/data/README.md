# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `kpmr.json`

A snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, in format 1, for the providers Kaeso calls; its `provider_ids` list every provider KPMR knows. KPMR reviews the catalogue in its registry, including what only Kaeso needs, such as adaptive thinking, forced tool choice, effort levels, the one-hour cache write price and the dialect Kaeso sends. Every registry entry cites its sources on the model's page at [kpmr.kaeso.app](https://kpmr.kaeso.app).

| | |
|---|---|
| Source | `https://kpmr.kaeso.app/v1/catalogue` |
| Revision | `2026-11-18T23:16:54Z-119131156842` |
| Generated | 2026-11-18T23:16:54Z |
| Content | the providers Kaeso calls (`alibaba`, `amazon-bedrock`, `anthropic`, `azure`, `baseten`, `cerebras`, `cloudflare-ai-gateway`, `cohere`, `deepinfra`, `deepseek`, `elevenlabs`, `fireworks-ai`, `gemini-agent-platform`, `google`, `groq`, `huggingface`, `ionos`, `jina`, `minimax`, `mistral`, `moonshotai`, `nebius`, `novita-ai`, `nvidia`, `ollama-cloud`, `openai`, `openrouter`, `ovhcloud`, `perplexity`, `sambanova`, `scaleway`, `siliconflow`, `standardcompute`, `stepfun-ai`, `tencent-tokenhub`, `togetherai`, `typesafe`, `upstage`, `venice`, `vercel`, `volcengine`, `voyage`, `xai`, `zai`); 2873 models |
| SHA-256 | `17e2fad23e6e49a1cd0967688c3978b244d942358f5470697feca369cb818576` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

To refresh it for a release, download the catalogue and check its signature against the built-in key:

```sh
cargo xtask kpmr
```
