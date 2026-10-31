# Fixtures: Alibaba Cloud (`alibaba`) on `openai_embeddings`

Alibaba Cloud Model Studio's OpenAI-compatible embeddings in the international (Singapore) region, `POST https://dashscope-intl.aliyuncs.com/compatible-mode/v1/embeddings` with `Authorization: Bearer`. Source: https://www.alibabacloud.com/help/en/model-studio/embedding, read 2026-10-30, which gives `text-embedding-v4` 1,024 dimensions by default, at most 10 rows a batch and 8,192 tokens a row, at 0.07 USD per million input tokens.

Every file under `responses/` and `errors/` was recorded live with `cargo xtask record` on 2026-10-30; `RECORDED.md` in each folder names the request and model.

| file | what it covers |
|---|---|
| `requests/embed-string.json`, `responses/recorded-embed-string.json` | one string with `encoding_format: float`; 1,024 floats from `text-embedding-v4`, and a top-level `id` |
| `requests/embed-base64.json`, `responses/recorded-embed-base64.json` | `encoding_format: base64` on `text-embedding-v3`, answered as base64 |
| `requests/embed-dimensions.json`, `responses/recorded-embed-dimensions.json` | two strings with `dimensions: 256` on `text-embedding-v4` |
| `requests/embed-batch-ten.json`, `responses/recorded-embed-batch-ten.json` | ten strings, the most a batch takes, with `dimensions: 64` on `text-embedding-v3` |
| `requests/embed-batch-eleven.json`, `errors/recorded-embed-batch-eleven.json` | 400: `batch size is invalid, it should not be larger than 10` |
| `requests/embed-dimensions-invalid.json`, `errors/recorded-embed-dimensions-invalid.json` | 400 for `dimensions: 100`, naming the sizes `text-embedding-v3` takes |
| `requests/embed-token-ids.json`, `errors/recorded-embed-token-ids.json` | 400 for token ids as input |
| `requests/embed-generation-model.json`, `errors/recorded-embed-generation-model.json` | 404 `model_not_supported` for a generation model |
| `requests/embed-unknown-model.json`, `errors/recorded-embed-unknown-model.json` | 404 `model_not_found` |

The error bodies are OpenAI's `error` object beside a top-level `request_id`, so they take the usual error round trip. KPMR lists no Model Studio embeddings model; people add them as custom models that send as Embeddings.
