---
title: OpenAI Embeddings
description: How every field of the OpenAI Embeddings format maps to KPP and back.
order: 7
---

OpenAI Embeddings, `POST /v1/embeddings`. Codec: `kaeso_codecs::openai_embeddings::OpenAiEmbeddings`.

Sources: the OpenAI OpenAPI spec at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (`CreateEmbeddingRequest`, `CreateEmbeddingResponse`, `Embedding`, `ErrorResponse`), the [create reference](https://developers.openai.com/api/reference/resources/embeddings/methods/create) and the [error codes guide](https://developers.openai.com/api/docs/guides/error-codes).

The tables use these terms:

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
| unknown keys | extras | |
| header `openai-beta` | extras `header:openai-beta` | Forwarded to Embeddings only. |

The spec allows 1 to 2,048 inputs and at most 8,192 tokens per input. The codec does not enforce these limits, and the provider answers with its own 400 when they are exceeded.

`check()` refuses generation and token-count requests (`generation request`), requests that mix text and token inputs (`mixed text and token inputs`), and everything `cross::check_foreign` refuses, such as extras of other dialects ([requests](cross-format.md#requests)).

## Response: `CreateEmbeddingResponse`

| Wire field | KPP | Notes |
|---|---|---|
| `object` (`list`) | none | Another value: form `object`. |
| `model` | `EmbedResponse.model` typed | |
| `data[].index` | `Embedding.index` typed | |
| `data[].embedding` array of numbers | `EmbeddingVector::Float` (`f64`, printed back identically) typed | |
| `data[].embedding` string | `EmbeddingVector::Base64` (as received) typed | When the provider answered in the other encoding, replies are converted to the app's `encoding_format`, taken from `EncodeCtx::request`: floats become base64 of little-endian `f32`, and base64 becomes floats printed in their shortest `f32` form. Base64 that is not a whole number of `f32` values can't be converted: `ProviderError`. |
| `data[].object` (`embedding`) | none | Another value: form `object` on the embedding. Unknown keys: extras of the embedding. |
| `usage.prompt_tokens` | `usage.input_tokens` typed | The usage object is kept in `usage.raw`. |
| `usage.total_tokens` | derived | Written as `prompt_tokens`, since embeddings have no output tokens; the raw usage object of this dialect wins. |
| `usage` `null` | form (`verbatim`) | |
| `usage` absent | form `no_usage` | No usage object is invented when the provider sent none. |
| unknown keys | extras | |

Generation replies cannot be written in this format (`Unsupported`, `generation response`).

## Streaming

The endpoint does not stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` with `Unsupported` (`streaming`) and then nothing.

## Errors

Errors use the OpenAI error shape `{"error": {message, type, param, code, misalignment}}`. `misalignment`, when present, is kept in `KppError.raw` with the whole body and sent again unchanged to OpenAI Embeddings and Responses apps. Decoding and encoding follow the same tables as [the errors of OpenAI Responses](openai-responses.md#errors-errorresponse), and the implementation is shared with the Responses codec. Errors decoded here carry the origin `openai_embeddings`.

## Models

`GET /v1/models` and `GET /v1/models/{model}` use the same shape in every OpenAI API. This codec encodes and decodes it with the shared implementation described under [models](openai-chat.md#models) on the OpenAI Chat Completions page, with the fixtures in `fixtures/openai_chat/models/`. Decoded extras carry the origin `openai_embeddings`.
