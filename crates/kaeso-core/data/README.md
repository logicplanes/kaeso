# Model data

Data shipped with every gateway release.

## `models-dev.json`

A snapshot of [models.dev](https://models.dev), the open (MIT-licensed) model database, so a fresh install works without network access.

| | |
|---|---|
| Source | `https://models.dev/api.json` |
| Fetched | 2026-10-19T00:54:58Z |
| Content | the providers Kaeso calls (`anthropic`, `openai`, `xai`), unchanged otherwise; 83 models |
| SHA-256 | `0a1d7406e2d21d2db3295e75eb888e3409169d88c0f39d0795eb420d97baf04f` |
| License | MIT, © models.dev contributors (see [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md)) |

The file keeps models.dev's shape (`{provider: {id, name, models: {id: model}}}`), pretty-printed with sorted keys. Only providers in `crates/kaeso-core/src/provider.rs` are kept; the full file lists over 200 providers Kaeso cannot call. When the gateway syncs (every 24 hours by default, `KAESO_MODELS_DEV_SYNC`), it applies the same filter and stores the result in PostgreSQL; the newer of the stored and the built-in snapshot wins at startup.

To refresh the snapshot for a release:

```sh
cargo xtask models-dev
```

It downloads `api.json` (`MODELS_DEV_URL` overrides the address), filters it, checks that the gateway can read the result, rewrites this file's table and `EMBEDDED_FETCHED_AT` in `crates/kaeso-core/src/models_dev.rs`, and prints the new hash. Tests check that the snapshot, this table and the constant agree.

## `supplement.toml`

Fields models.dev does not have. A catalogue entry's effective value is its override, else models.dev, else this file, else empty.

| Field | Meaning |
|---|---|
| `adaptive_thinking` | Anthropic models: whether `thinking: {type: "adaptive"}` is accepted. `false` makes kpp turn an effort from another format into a thinking budget. |
| `forced_tool_choice` | `false` for models that refuse a forced tool choice (Anthropic `tool_choice` `any` or `tool`); the gateway then refuses `required`, `any` or a named tool before calling the provider. Absent means the model takes one. |
| `efforts` | Supported effort levels, used when models.dev lists none. |
| `egress_dialect` | The format Kaeso sends: for OpenAI models `openai_responses` (the default), `openai_chat` or `openai_embeddings`. |

Every entry comes from one of these sources, checked on 2026-10-11:

| Entries | Values | Source |
|---|---|---|
| Claude 4.5 models | budget thinking only; no effort except on Claude Opus 4.5 | Anthropic, [Thinking](https://platform.claude.com/docs/en/build-with-claude/thinking) and [Effort](https://platform.claude.com/docs/en/build-with-claude/effort); live check: `claude-haiku-4-5` answers `thinking: {type: adaptive}` with 400 |
| Claude 4.6 and later | adaptive thinking and their effort levels | the same pages; live check: `claude-sonnet-4-6` accepts adaptive thinking |
| `claude-fable-5-1`, `claude-opus-5-5`, `claude-sonnet-5-5` | no forced tool choice | Anthropic, [migration guide](https://platform.claude.com/docs/en/about-claude/models/migration-guide) (forced tool use returns 400 `tool_choice: type "tool" and "any" are not supported for this model.`, token counting included; Claude Fable 5 and Opus 5 accept it); a live run on 2026-10-17 got that 400 from all three for `any` and `tool`, while `auto` worked |
| `text-embedding-3-small`, `text-embedding-3-large`, `text-embedding-ada-002` | the Embeddings endpoint only | OpenAI model pages (`https://developers.openai.com/api/docs/models/<id>`) |
| `gpt-5.4-mini`, `gpt-6-astra`, `gpt-5.6-terra` | Responses, the default for every OpenAI generation model, listed explicitly | live check: these models reject function tools together with `reasoning_effort` on Chat Completions; Responses accepts them |
