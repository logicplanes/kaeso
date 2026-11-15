# openai_embeddings / providers / tencent-tokenhub (Tencent TokenHub)

Tencent TokenHub, provider id `tencent-tokenhub`, base URL `https://tokenhub.tencentmaas.com`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: TokenHub's embeddings page https://cloud.tencent.com/document/product/1823/133515, read on 2026-11-14.

No key was available. The request and the reply are the page's examples; only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/kinfra-text-doc.json`, `responses/kinfra-text-doc.json` | one string to `kinfra-text-embedding-0.6b` with `encoding_format: "float"`; usage with `prompt_tokens_details` of text, image and video tokens | doc |
| `errors/recorded-invalid-api-key.json` | 401 `401002` with OpenAI's `error` object and TokenHub's `message_zh`, `code` and `source` | live, with a made-up key |

TokenHub's text models take at most 2000 characters per text and 128 texts per request as advised; `encoding_format` takes only `float`, and `dimensions` is fixed per model (1024 and 2560). Its multimodal models answer at `/v1/embeddings/multimodal`, which isn't OpenAI's format.
