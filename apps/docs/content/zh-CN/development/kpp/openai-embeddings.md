---
title: OpenAI Embeddings
description: OpenAI Embeddings 格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 7
---

在这里查阅 OpenAI Embeddings 格式的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_embeddings::OpenAiEmbeddings`（`crates/kaeso-codecs/src/openai_embeddings/`）
- **端点：**`POST /v1/embeddings`，以及所有 OpenAI 编解码器共用的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_embeddings/`

**来源。** OpenAI OpenAPI 规范的提交 `e95c0fe615f45a19e8878af923a729948445a6bd`（`CreateEmbeddingRequest`、`CreateEmbeddingResponse`、`Embedding`、`ErrorResponse`）、[创建接口参考](https://developers.openai.com/api/reference/resources/embeddings/methods/create)和[错误码指南](https://developers.openai.com/api/docs/guides/error-codes)。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `openai_embeddings`；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **形式标记**：编解码器私有的 `Extras::form` 标记；`verbatim` 保留已声明键的 `null` 写法。

## 请求：`CreateEmbeddingRequest`

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `EmbedRequest.model`（类型化） | 必填。 |
| `input` 字符串 | `input = [EmbedInput::Text]`（类型化） | 单个文本的规范写法。 |
| `input` 字符串数组 | 每个字符串一个 `EmbedInput::Text`（类型化） | 只有一个元素的数组：形式标记 `input: "array"`。 |
| `input` Token ID 数组 | `input = [EmbedInput::Tokens]`（类型化） | 单个 Token 数组的规范写法。 |
| `input` Token ID 数组的数组 | 每个数组一个 `EmbedInput::Tokens`（类型化） | 只有一个元素的数组：形式标记 `input: "nested"`。 |
| `input` 空数组 | `input = []` | 对 API 无效（`minItems: 1`）；网关在解码后运行的 `EmbedRequest::validate()` 会以 `InvalidRequest` 拒绝它。 |
| `input` 其他任何结构（不是 `u32` 的数字、混合数组） | `InvalidRequest` | |
| `encoding_format` 为 `float`、`base64` 或缺失 | `encoding` 为 `Float`、`Base64` 或 `Float`（类型化） | 显式的 `float`：形式标记 `encoding_format: "float"`。其他值：`InvalidRequest`，因为 KPP 中没有存放它们的位置。 |
| `dimensions` | `dimensions`（类型化） | |
| `user` | `user`（类型化） | |
| `null` 值 | 形式标记（`verbatim`） | |
| 未知键 | extras | 以 `header:` 开头或含有 `.` 的键无法携带：`InvalidRequest`（`` the field `…` has a name Kaeso cannot carry; remove it ``）。 |
| 请求头 `openai-beta` | extras `header:openai-beta` | 只转发给 Embeddings。 |

规范允许 1 到 2,048 个输入，每个输入最多 8,192 个 Token。编解码器不强制执行这些限制，超出时由服务商返回它自己的 400。

`decode_request` 会拒绝生成请求和 Token 计数请求（`generation request`）。`check()` 同样拒绝它们，还会拒绝混合文本输入与 Token 输入的请求（`mixed text and token inputs`），以及 `cross::check_foreign` 拒绝的一切内容，例如其他方言的 extras（[请求](cross-format.md#请求)）。

## 回复：`CreateEmbeddingResponse`

| 线路字段 | KPP | 说明 |
|---|---|---|
| `object`（`list`） | 无 | 其他值：形式标记 `object`。 |
| `model` | `EmbedResponse.model`（类型化） | |
| `data[].index` | `Embedding.index`（类型化） | |
| `data[].embedding` 数字数组 | `EmbeddingVector::Float`（类型化；`f64`，原样写回） | |
| `data[].embedding` 字符串 | `EmbeddingVector::Base64`（类型化；按收到的原样） | 服务商以另一种编码回复时，回复会转换为应用的 `encoding_format`（取自 `EncodeCtx::request`）：浮点数变为小端 `f32` 的 base64，base64 则变为以最短 `f32` 形式写出的浮点数。字节数不是 `f32` 整数倍的 base64 无法转换：`ProviderError`。`EncodeCtx` 中没有应用的请求时，向量保持收到时的样子。 |
| `data[].object`（`embedding`） | 无 | 其他值：该嵌入向量上的形式标记 `object`。未知键：该嵌入向量的 extras。 |
| `usage.prompt_tokens` | `usage.input_tokens`（类型化） | 用量对象保留在 `usage.raw` 中。 |
| `usage.total_tokens` | 推导得出 | 写为 `prompt_tokens`，因为嵌入没有输出 Token；本方言的原始用量对象优先。 |
| `usage` 为 `null` | 形式标记（`verbatim`） | |
| 没有 `usage` | 形式标记 `no_usage` | 服务商没有发送用量对象时，不会凭空生成一个。 |
| 未知键 | extras | 以 `header:` 开头或含有 `.` 的键无法携带：`ProviderError`。 |

生成请求的回复无法以这种格式写出，服务商的回复也只有在嵌入请求时才会被读取（两者都是 `Unsupported`，`generation response`）。

## 流式

该端点不支持流式。流编码器对每个事件都返回 `Unsupported`（`streaming`）；流解码器报告一个带 `Unsupported`（`streaming`）的 `StreamEvent::Error`，之后不再产生任何内容。

## 错误

错误使用 OpenAI 的错误结构 `{"error": {message, type, param, code, misalignment}}`。存在 `misalignment` 时，它会连同整个错误体保留在 `KppError.raw` 中，并原样再次发送给 OpenAI Embeddings 和 Responses 应用。解码和编码遵循与 [OpenAI Responses 的错误](openai-responses.md#错误errorresponse)相同的表格，其实现也与 Responses 编解码器共用。在这里解码的错误带有来源 `openai_embeddings`。

## 模型

`GET /v1/models` 和 `GET /v1/models/{model}` 在每个 OpenAI API 中都使用同一种结构。本编解码器使用 OpenAI Chat Completions 页面中[模型](openai-chat.md#模型)一节所述的共用实现对其编码和解码，fixture 位于 `fixtures/openai_chat/models/`。解码出的 extras 带有来源 `openai_embeddings`。

## 使用此格式的服务商

提供 Embeddings 的其他服务商（OpenAI 以外）会录制到 `fixtures/openai_embeddings/providers/<id>/`（[fixture](fixtures.md)）。

### Together AI

来源：Together AI 的 [OpenAI 兼容性说明](https://docs.together.ai/docs/inference/openai-compatibility)（`embeddings.create` 对应 `POST /v1/embeddings`）和 [Serverless 模型](https://docs.together.ai/docs/serverless-models)（2026-10-19 阅读），以及 `fixtures/openai_embeddings/providers/togetherai/`。Together AI 只在专用端点上提供嵌入模型；没有专用端点的密钥会收到 400 `model_not_available`。其回复没有 `usage`，本编解码器本来就能读取这种回复。

### OpenRouter

来源：OpenRouter 的 [Embeddings 参考](https://openrouter.ai/docs/api_reference/embeddings)（2026-10-19 阅读），以及 `fixtures/openai_embeddings/providers/openrouter/` 中使用 `nvidia/nemotron-3-embed-1b:free` 的实时录制。回复中的 `model` 是实际提供服务的模型，而不是请求的 ID；`usage.cost`、`usage.is_byok`、`usage.cost_details` 以及顶层的 `provider` 和 `id` 保留在 extras 中，原样往返。上游服务商的错误以 400 返回，其 `message` 以文本形式包含该服务商的错误体。
