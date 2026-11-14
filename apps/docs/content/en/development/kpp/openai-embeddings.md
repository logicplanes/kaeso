---
title: OpenAI Embeddings
description: How every field of the OpenAI Embeddings format maps to KPP and back.
order: 8
---

Look up any field of the OpenAI Embeddings format here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::openai_embeddings::OpenAiEmbeddings` (`crates/kaeso-codecs/src/openai_embeddings/`)
- **Endpoints:** `POST /v1/embeddings`, and the [models](#models) endpoints shared by every OpenAI codec
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_embeddings/`

**Sources.** The OpenAI OpenAPI spec at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (`CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`), the [create reference](https://developers.openai.com/api/reference/resources/embeddings/methods/create) and the [error codes guide](https://developers.openai.com/api/docs/guides/error-codes).

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `openai_embeddings`; another dialect refuses them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker; `verbatim` keeps `null` spellings of declared keys.

## Request: `CreateEmbeddingRequest`

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `EmbedRequest.model` typed | Required. |
| `input` string | `input = [EmbedInput::Text]` typed | The canonical spelling for one text. |
| `input` array of strings | `EmbedInput::Text` per string typed | One-element array: form `input: "array"`. |
| `input` array of token ids | `input = [EmbedInput::Tokens]` typed | The canonical spelling for one token array. |
| `input` array of token-id arrays | `EmbedInput::Tokens` per array typed | One-element array: form `input: "nested"`. |
| `input` empty array | `input = []` | Invalid for the API (`minItems: 1`); `EmbedRequest::validate()`, which the gateway runs after decoding, refuses it with `InvalidRequest`. |
| `input` any other shape (numbers that are not `u32`, mixed arrays) | `InvalidRequest` | |
| `encoding_format` `float`, `base64` or absent | `encoding` `Float`, `Base64` or `Float` typed | Explicit `float`: form `encoding_format: "float"`. Other values: `InvalidRequest`, because KPP has no place for them. |
| `dimensions` | `dimensions` typed | |
| `user` | `user` typed | |
| `null` values | form (`verbatim`) | |
| unknown keys | extras | A key that starts with `header:` or contains `.` can't be carried: `InvalidRequest` (`` the field `…` has a name Kaeso cannot carry; remove it ``). |
| header `openai-beta` | extras `header:openai-beta` | Forwarded to Embeddings only. |

The spec allows 1 to 2,048 inputs and at most 8,192 tokens per input. The codec does not enforce these limits, and the provider answers with its own 400 when they are exceeded.

`decode_request` refuses generation and token-count requests (`generation request`). `check()` refuses them too, and also requests that mix text and token inputs (`mixed text and token inputs`) and everything `cross::check_foreign` refuses, such as extras of other dialects ([requests](cross-format.md#requests)).

## Response: `CreateEmbeddingResponse`

| Wire field | KPP | Notes |
|---|---|---|
| `object` (`list`) | none | Another value: form `object`. |
| `model` | `EmbedResponse.model` typed | |
| `data[].index` | `Embedding.index` typed | |
| `data[].embedding` array of numbers | `EmbeddingVector::Float` (`f64`, printed back identically) typed | |
| `data[].embedding` string | `EmbeddingVector::Base64` (as received) typed | When the provider answered in the other encoding, replies are converted to the app's `encoding_format`, taken from `EncodeCtx::request`: floats become base64 of little-endian `f32`, and base64 becomes floats printed in their shortest `f32` form. Base64 that is not a whole number of `f32` values can't be converted: `ProviderError`. Without the app's request in `EncodeCtx`, vectors stay as received. |
| `data[].object` (`embedding`) | none | Another value: form `object` on the embedding. Unknown keys: extras of the embedding. |
| `usage.prompt_tokens` | `usage.input_tokens` typed | The usage object is kept in `usage.raw`. |
| `usage.total_tokens` | derived | Written as `prompt_tokens`, since embeddings have no output tokens; the raw usage object of this dialect wins. |
| `usage` `null` | form (`verbatim`) | |
| `usage` absent | form `no_usage` | No usage object is invented when the provider sent none. |
| unknown keys | extras | A key that starts with `header:` or contains `.` can't be carried: `ProviderError`. |

Generation replies cannot be written in this format, and a provider's reply is read only for an embeddings request (both `Unsupported`, `generation response`).

## Streaming

The endpoint does not stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` with `Unsupported` (`streaming`) and then nothing.

## Errors

Errors use the OpenAI error shape `{"error": {message, type, param, code, misalignment}}`. `misalignment`, when present, is kept in `KppError.raw` with the whole body and sent again unchanged to OpenAI Embeddings and Responses apps. Decoding and encoding follow the same tables as [the errors of OpenAI Responses](openai-responses.md#errors-errorresponse), and the implementation is shared with the Responses codec. Errors decoded here carry the origin `openai_embeddings`.

## Models

`GET /v1/models` and `GET /v1/models/{model}` use the same shape in every OpenAI API. This codec encodes and decodes it with the shared implementation described under [models](openai-chat.md#models) on the OpenAI Chat Completions page, with the fixtures in `fixtures/openai_chat/models/`. Decoded extras carry the origin `openai_embeddings`.

## Providers that use this format

Providers other than OpenAI that serve Embeddings record into `fixtures/openai_embeddings/providers/<id>/` ([fixtures](fixtures.md)).

### Together AI

Sources: Together AI's [OpenAI compatibility](https://docs.together.ai/docs/inference/openai-compatibility) (`embeddings.create` is `POST /v1/embeddings`) and [serverless models](https://docs.together.ai/docs/serverless-models), read 2026-10-19, and `fixtures/openai_embeddings/providers/togetherai/`. Together AI serves embedding models only on dedicated endpoints; a key without one gets a 400 `model_not_available`. Its reply has no `usage`, which this codec reads already.

### OpenRouter

Source: OpenRouter's [embeddings reference](https://openrouter.ai/docs/api_reference/embeddings), read 2026-10-19, and the live recordings in `fixtures/openai_embeddings/providers/openrouter/`, with `nvidia/nemotron-3-embed-1b:free`. The reply's `model` names the served model, not the requested id; `usage.cost`, `usage.is_byok`, `usage.cost_details` and the top-level `provider` and `id` stay in extras and round-trip unchanged. An error from the upstream provider comes as a 400 whose `message` holds that provider's error body as text.

### Mistral

Sources: Mistral's [API reference](https://docs.mistral.ai/api/) (`EmbeddingRequest`, `EmbeddingResponse`), read 2026-10-19, and the live recordings in `fixtures/openai_embeddings/providers/mistral/`, with `mistral-embed`. Mistral takes `encoding_format: base64` but answers with floats; for an app that asked for base64 the codec encodes them as base64. Mistral refuses OpenAI's `dimensions` and token-id input with a 422, and `output_dimension` on `mistral-embed` with a 400. Those error bodies have no `error` object, so apps get an OpenAI error with Mistral's message when the body has one as a string, and one that quotes the body otherwise.

### Vercel

Sources: Vercel's [embeddings](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/embeddings) page, read 2026-10-29, and the live recordings in `fixtures/openai_embeddings/providers/vercel/`, with `google/gemini-embedding-001` and `dimensions: 8`. A Vercel model is an embeddings model when KPMR gives it the kind `embedding`, as Vercel's listing does. The reply's `providerMetadata` (routing and cost) stays in extras and round-trips unchanged. A free-tier account may not call some embedding models, such as `openai/text-embedding-3-small`, which answer 403.

### Cohere

Sources: Cohere's [Compatibility API](https://docs.cohere.com/docs/compatibility-api) page, read 2026-10-30, and the live recordings in `fixtures/openai_embeddings/providers/cohere/`, with `embed-english-light-v3.0` and `embed-v4.0`. Cohere takes `encoding_format` `float` and `base64` and answers in the format asked. Its page lists `dimensions` as unsupported, but `embed-v4.0` honours it (`responses/recorded-embed-dimensions`, 256 floats); the v3 models refuse it with a 400. Token-id input is refused with a 422. Cohere's error bodies are `{"id", "message"}`, so apps get an OpenAI error with Cohere's message. KPMR lists no Cohere Embed model, so they are added as custom models that send as Embeddings.

### QwenCloud

Sources: Alibaba Cloud Model Studio's (now QwenCloud's) [embedding](https://www.alibabacloud.com/help/en/model-studio/embedding) page, read 2026-10-30, and the live recordings in `fixtures/openai_embeddings/providers/alibaba/`, with `text-embedding-v4` and `text-embedding-v3` in the international (Singapore) region. QwenCloud takes `encoding_format` `float` and `base64` and answers in the format asked, and `dimensions` from the sizes each model lists (`responses/recorded-embed-dimensions`, 256 floats); another size gets 400 naming them. A batch takes at most 10 inputs; 11 get 400 (`errors/recorded-embed-batch-eleven`). Token-id input gets 400, and a generation model 404. Replies carry a top-level `id`, which stays in extras. KPMR lists no QwenCloud embeddings model, so they are added as custom models that send as Embeddings.

### DeepInfra

Sources: DeepInfra's [OpenAPI document](https://api.deepinfra.com/openapi.json) (`OpenAIEmbeddingsIn`) and its [embeddings guide](https://docs.deepinfra.com/apis/embeddings.md), read 2026-11-14, and `fixtures/openai_embeddings/providers/deepinfra/`, built from them, with the error answered live without a valid key. DeepInfra's embeddings models, such as `BAAI/bge-m3` and `Qwen/Qwen3-Embedding-8B`, take `encoding_format` and `dimensions` from 32 up, padding a vector beyond the model's size with zeros. DeepInfra's own `input_type` and `task` stay in extras, and the content-part lists of its multimodal models go as written from an OpenAI Embeddings app.

### Fireworks AI

Sources: Fireworks' [create embeddings](https://docs.fireworks.ai/api-reference/creates-an-embedding-vector-representing-the-input-text) reference and [embeddings guide](https://docs.fireworks.ai/guides/querying-embeddings-models), read 2026-11-09, and `fixtures/openai_embeddings/providers/fireworks-ai/`, built from them, with the error answered live without a valid key. The serverless models are `qwen3-embedding-8b` (`fireworks/qwen3-embedding-8b`) and older models under their Hugging Face ids, such as `nomic-ai/nomic-embed-text-v1.5`; only resizable ones take `dimensions`. Fireworks' own `prompt_template`, `normalize` and `return_logits` stay request extras. Its schema has no `encoding_format`, so the codec encodes floats as base64 for an app that asks for it.

### Nebius Token Factory

Sources: Nebius' [create embeddings](https://docs.tokenfactory.nebius.com/api-reference/inference/create-embeddings) reference, read 2026-11-09, and `fixtures/openai_embeddings/providers/nebius/`, built from it, with the error answered live without a valid key. Nebius' embeddings model is `Qwen/Qwen3-Embedding-8B`. It takes `encoding_format` and `dimensions`, and every reply carries `service_tier`, which stays in extras.

### Scaleway

Sources: Scaleway's [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/) (`CreateEmbeddingResponse`), read 2026-11-09, and `fixtures/openai_embeddings/providers/scaleway/`, built from it, with the error answered live without a valid key. The serverless models are `bge-multilingual-gemma2` and `qwen3-embedding-8b`. Replies add `id`, `created` and `usage.completion_tokens: 0`, which stay in extras. `dimensions` takes only a model's largest size, so Scaleway answers other values itself.

### NVIDIA

Sources: NVIDIA's embeddings references, such as [`nvidia/nv-embedqa-e5-v5`](https://docs.api.nvidia.com/nim/reference/nvidia-nv-embedqa-e5-v5-infer), read 2026-11-10, and `fixtures/openai_embeddings/providers/nvidia/`, built from them, with the error answered live without a valid key. Its models include `nvidia/nemotron-3.5-embed-1b` and `nvidia/nv-embedqa-e5-v5`. Requests take `input_type`, `query` or `passage`, which the retrieval models require, and `truncate`, `NONE`, `START` or `END`; both stay in extras and go as written. NVIDIA's errors are problem documents, which apps get in OpenAI's shape with NVIDIA's body in the message.

### SiliconFlow

Sources: SiliconFlow's [create embeddings](https://docs.siliconflow.com/en/api-reference/embeddings/create-embeddings) reference, read 2026-11-10, and `fixtures/openai_embeddings/providers/siliconflow/`, built from it, with the error answered live without a valid key. Its models include `Qwen/Qwen3-Embedding-8B`, `-4B` and `-0.6B` and `BAAI/bge-m3`. Only the Qwen3 models take `dimensions`, and a request takes at most 32 strings. Replies carry `usage.completion_tokens: 0`, which stays in extras. SiliconFlow's errors have no `error` object; apps get them in OpenAI's shape with SiliconFlow's message.

### Cloudflare AI Gateway

Sources: Workers AI's [OpenAI compatibility](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/) page and [model pages](https://developers.cloudflare.com/workers-ai/models/), read 2026-11-14, which show the request but no reply, and `fixtures/openai_embeddings/providers/cloudflare-ai-gateway/`, recorded live. Kaeso sends the request to `/client/v4/accounts/{account_id}/ai/v1/embeddings`, the key's account in place of `/v1`, with the key's gateway header when it names one. The reply has `object`, `data` and `model` and no `usage`, so Kaeso estimates the request's tokens. Workers AI ignores `dimensions` and answers the model's full size, so the provider's embeddings profile refuses it (`without_dimensions()`); it answers `encoding_format: "base64"` with floats, which the codec gives the app in the encoding it asked for. `@cf/google/embeddinggemma-2` answers this path 400 even with `input`, so KPMR doesn't list it.
