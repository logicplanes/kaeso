# openai_embeddings / providers / togetherai

Together AI (`togetherai`), recorded live with `cargo xtask record openai_embeddings --provider togetherai` on 2026-10-19.

| | |
|---|---|
| API reference | https://docs.together.ai/docs/inference/openai-compatibility (`embeddings.create` maps to `POST /v1/embeddings`), read 2026-10-19 |
| SDK types | `togethercomputer/together-py@c55bb3003581d8be51f8eb102debf314cb3bcfe2`, `src/together/types/embedding.py` and `embedding_create_params.py` |
| Base URL | `https://api.together.ai` |
| Model | `BAAI/bge-base-en-v1.5` |

Together served no embedding model serverless on 2026-10-19 ([serverless models](https://docs.together.ai/docs/serverless-models): "There are currently no embedding models offered via serverless"), so the only live reply is the error a key without a dedicated endpoint gets. The SDK's reply type has `data[].embedding`, `data[].index`, `data[].object`, `model` and `object`, and no `usage`; the codec reads a reply without `usage` already.

## errors

| file | request | status | what it covers |
|---|---|---|---|
| `recorded-string-input.json` | `openai_embeddings/requests/string-input.json` | 400 | top-level `id`, `code: model_not_available` on `/v1/embeddings` |

`errors/RECORDED.md` lists the model and time of the recording.
