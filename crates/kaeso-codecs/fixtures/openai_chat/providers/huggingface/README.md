# `fixtures/openai_chat/providers/huggingface`

Hugging Face Inference Providers (`huggingface`), the router at `https://router.huggingface.co/v1`, which speaks OpenAI Chat Completions and forwards each request to one of many backends (Cerebras, DeepInfra, Fireworks, Groq, Novita, Together and others). Model ids are `org/model`, optionally with a backend or policy suffix: `org/model:cerebras`, `org/model:fastest`, `org/model:cheapest`, `org/model:preferred`.

Sources, read 2026-10-19:

- [Inference Providers](https://huggingface.co/docs/inference-providers/index): base URL, Bearer token, model suffixes, `GET /v1/models`, chat completions only on the OpenAI-compatible endpoint
- [Chat Completion API reference](https://huggingface.co/docs/inference-providers/tasks/chat-completion): request and response fields
- [Pricing and billing](https://huggingface.co/docs/inference-providers/pricing): credits, pay as you go
- huggingface.js at commit [`457119ebd1d68f76dc2f9d0f218ffbadb62e7325`](https://github.com/huggingface/huggingface.js/blob/457119ebd1d68f76dc2f9d0f218ffbadb62e7325/packages/inference/test/tapes.json) (2025-04-22): the SDK's recorded HTTP tapes, taken from replies that came through `router.huggingface.co` from the `hf-inference` backend (Text Generation Inference)

The test token had no credits, so every generation request got HTTP 402 before it reached a backend. The successful reply and stream are therefore from the SDK's tapes, not from live recordings, and there is no tool-call fixture: neither the tapes nor the docs contain a tool-call reply from the router.

| file | what it covers | provenance |
|---|---|---|
| `errors/recorded-plain-developer-string.json` | HTTP 402 when the account has no credits; `error` is a plain string, not an object | recorded 2026-10-19, `meta-llama/Llama-3.1-8B-Instruct:deepinfra` |
| `errors/recorded-stream-include-usage.json` | the same 402 for a streaming request: a JSON body, no stream | recorded 2026-10-19, `meta-llama/Llama-3.1-8B-Instruct:deepinfra` |
| `errors/recorded-invalid-token.json` | HTTP 401 for an invalid token; `error` is a plain string | recorded 2026-10-19 with a made-up token |
| `errors/recorded-unknown-model.json` | HTTP 400 `model_not_found` in the OpenAI error shape | recorded 2026-10-19 |
| `errors/recorded-unknown-backend.json` | HTTP 400 `model_not_supported` for an unknown `:provider` suffix | recorded 2026-10-19 |
| `errors/sdk-tgi-template-error.json` | HTTP 422 passed through from a backend: plain-string `error` plus `error_type` | tapes.json entry `a568a8ed71…`, `hf-inference` backend, verbatim body |
| `requests/unknown-model.json` | a model id the router does not know | written for the recording |
| `requests/unknown-backend.json` | a backend suffix the router does not know | written for the recording |
| `requests/invalid-token.json` | a plain request with the `:cheapest` policy suffix, sent with an invalid token | written for the recording |
| `responses/sdk-tgi-basic.json` | a reply from the `hf-inference` backend: empty `id`, `system_fingerprint`, `logprobs: null` | tapes.json entry `5c37128d38…`, verbatim body |
| `streams/sdk-tgi-stream.sse` | a stream from the `hf-inference` backend: `usage: null` on every chunk, empty `id`, `[DONE]` | tapes.json entry `4304afb3c7…`, verbatim body |
| `models/list-recorded.json` | live `GET /v1/models`, 2 of 133 models kept unchanged: `architecture` and a `providers` array per model (backend, status, context length, pricing, tool support, latency, throughput), some backends without `pricing` | recorded 2026-10-19 |

Round trip: every reply, stream and model listing round-trips unchanged; the router's extra model fields stay extras. An error whose `error` is a plain string decodes with that string as its message and keeps the body as its raw form; re-encoded for an OpenAI Chat Completions app it takes the OpenAI error shape with the same message and status. HTTP 402 is a billing error.
