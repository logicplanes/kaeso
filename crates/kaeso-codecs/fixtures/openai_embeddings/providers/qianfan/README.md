# Fixtures: Baidu Qianfan (`qianfan`) on `openai_embeddings`

`POST https://qianfan.baidubce.com/v2/embeddings` (https://cloud.baidu.com/doc/qianfan-api/s/qmwbl45cf, read 2026-11-14): `model`, `input`, `user` and `encoding_format` with `float` only; no `dimensions`. No key was available.

| File | What it covers | Provenance |
|---|---|---|
| `requests/embedding-v1-doc.json`, `responses/embedding-v1-doc.json` | one input; the reply adds `id` and `created` | reference examples; the reply's elided vector values (`...`) left out |
| `errors/recorded-invalid-iam-token.json` | 401 for an unknown key | live, with a made-up key; id zeroed |
