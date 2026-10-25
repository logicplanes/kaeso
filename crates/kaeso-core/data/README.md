# Model data

Data shipped with every gateway release, and where it comes from. [Model data](https://docs.kaeso.app/development/architecture/gateway#model-data) explains what each file holds and how the gateway uses it.

## `models-dev.json`

A snapshot of [models.dev](https://models.dev), the open (MIT-licensed) model database, filtered to the providers Kaeso calls.

| | |
|---|---|
| Source | `https://models.dev/api.json` |
| Fetched | 2026-10-25T22:43:45Z |
| Content | the providers Kaeso calls (`anthropic`, `deepseek`, `groq`, `mistral`, `openai`, `openrouter`, `togetherai`, `xai`), unchanged otherwise; 555 models |
| SHA-256 | `03e3426f1ee3a10b6b219c7906f289f9886831da3b54ac7f1d81e78ded4474a7` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

`models-dev-providers.txt` lists every provider id in the same `api.json`. To refresh both for a release:

```sh
cargo xtask models-dev
```

## `supplement.toml`

Every entry comes from one of these sources, checked on 2026-10-11:

| Entries | Values | Source |
|---|---|---|
| Claude 4.5 models | budget thinking only; no effort except on Claude Opus 4.5 | Anthropic, [Thinking](https://platform.claude.com/docs/en/build-with-claude/thinking) and [Effort](https://platform.claude.com/docs/en/build-with-claude/effort); live check: `claude-haiku-4-5` answers `thinking: {type: adaptive}` with 400 |
| Claude 4.6 and later | adaptive thinking and their effort levels | the same pages; live check: `claude-sonnet-4-6` accepts adaptive thinking |
| `claude-fable-5-1`, `claude-opus-5-5`, `claude-sonnet-5-5` | no forced tool choice | Anthropic, [migration guide](https://platform.claude.com/docs/en/about-claude/models/migration-guide) (forced tool use returns 400 `tool_choice: type "tool" and "any" are not supported for this model.`, token counting included; Claude Fable 5 and Opus 5 accept it); a live run on 2026-10-17 got that 400 from all three for `any` and `tool`, while `auto` worked |
| `text-embedding-3-small`, `text-embedding-3-large`, `text-embedding-ada-002` | the Embeddings endpoint only | OpenAI model pages (`https://developers.openai.com/api/docs/models/<id>`) |
| `gpt-5.4-mini`, `gpt-6-astra`, `gpt-5.6-terra` | Responses, the default for every OpenAI generation model, listed explicitly | live check: these models reject function tools together with `reasoning_effort` on Chat Completions; Responses accepts them |
| `mistral-embed` | the Embeddings endpoint only | Mistral, [API reference](https://docs.mistral.ai/api/) (`EmbeddingRequest`); live check on 2026-10-19: `POST /v1/embeddings` answers with 1024 floats, and `GET /v1/models` lists it with `completion_chat: false` |

The one-hour cache write price, which models.dev doesn't list, is Anthropic's: twice the input price ([pricing](https://platform.claude.com/docs/en/about-claude/pricing), read 2026-10-19).
