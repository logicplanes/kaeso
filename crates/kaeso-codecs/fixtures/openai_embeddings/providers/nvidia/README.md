# openai_embeddings / providers / nvidia (NVIDIA)

NVIDIA's hosted NIM API, provider id `nvidia`, base URL `https://integrate.api.nvidia.com`, `POST /v1/embeddings` with `Authorization: Bearer`. Reference: https://docs.api.nvidia.com/nim/reference/nvidia-nv-embedqa-e5-v5-infer and https://docs.api.nvidia.com/nim/reference/nvidia-llama-nemotron-embed-1b-v2-infer, read on 2026-11-10.

Not recorded from a successful call. The reply is built from the reference's response schema (`object`, `data[].object`, `data[].embedding`, `data[].index`, `model`, `usage.prompt_tokens`, `usage.total_tokens`); only the error was answered live, with a made-up key.

| file | what it covers | provenance |
|---|---|---|
| `requests/query.json`, `responses/query.json` | a query with NVIDIA's `input_type` and `truncate`, which stay request `Extras` and go as written | doc |
| `errors/recorded-invalid-api-key.json` | 403 problem document `Authorization failed`; the Embeddings codec reads no `detail`, so apps get the body in the message of an OpenAI error | live, with a made-up key |

The retrieval models such as `nvidia/nv-embedqa-e5-v5` require `input_type`, `query` or `passage`, which only an Embeddings app can send; NVIDIA answers a request without it itself. `user` is accepted and ignored, the reference says.
