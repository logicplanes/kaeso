# openai_embeddings / providers / nvidia (NVIDIA NIM)

NVIDIA NIM, provider id `nvidia`, base URL `https://integrate.api.nvidia.com`, `POST /v1/embeddings` with `Authorization: Bearer`. API reference: https://docs.api.nvidia.com/nim/reference/retrieval-apis, read on 2026-10-19.

Nothing here was recorded live: Kaeso has no NVIDIA NIM test key. Every file is the `x-nvai-meta.examples[0]` string (`requestJson` or `responseJson`) of the OpenAPI document that the model's page embeds, taken without changes. The vectors are NVIDIA's examples; `usage` is `0` in the nv-embedqa-e5-v5 example as published.

| file | what it covers | provenance |
|---|---|---|
| `requests/doc-nv-embedqa-e5-v5-query.json` | asymmetric model: `input_type: query`, `truncate: NONE`, `encoding_format: float` (Extras besides `encoding_format`) | doc: https://docs.api.nvidia.com/nim/reference/nvidia-nv-embedqa-e5-v5-infer |
| `responses/doc-nv-embedqa-e5-v5-query.json` | 1024 floats, `usage` without `completion_tokens` | doc: https://docs.api.nvidia.com/nim/reference/nvidia-nv-embedqa-e5-v5-infer |
| `requests/doc-bge-m3.json` | symmetric model: `truncate` without `input_type` | doc: https://docs.api.nvidia.com/nim/reference/baai-bge-m3-invoke |
| `responses/doc-bge-m3.json` | float vector with non-zero `usage` | doc: https://docs.api.nvidia.com/nim/reference/baai-bge-m3-invoke |
| `requests/doc-nemotron-embed-vl-image-query.json` | multimodal model: `image` and `image_name` next to `input` and `input_type` (Extras) | doc: https://docs.api.nvidia.com/nim/reference/nvidia-llama-nemotron-embed-vl-1b-v2-infer (example 1) |
