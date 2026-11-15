---
title: Jina Rerank
description: Jina AI 重排序格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 Jina AI 重排序格式的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::jina_rerank::JinaRerank`（`crates/kaeso-codecs/src/jina_rerank/`）
- **端点：**作为 Jina AI 的格式时为 `https://api.jina.ai` 上的 `POST /v1/rerank`（[支持的服务商](../../concepts/supported-providers.md)）。应用以 [Cohere 的格式](cohere-rerank.md)进行重排序；没有面向应用的端点接受此格式。
- **Fixture：**`crates/kaeso-codecs/fixtures/jina_rerank/`，包括来自 Jina OpenAPI 示例的请求、根据 schema 构建的回复，以及实际返回的密钥被拒绝错误

**来源。** Jina AI 的 [OpenAPI 文档](https://api.jina.ai/openapi.json)（`info.version` `2026.10.13.1624`：`RerankerV3Request`、`TextRerankerRequest`、`RerankingResponse`、`ErrorResponse`）和 [API 参考](https://docs.jina.ai/api.md)，查阅于 2026-11-14。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `jina_rerank`；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **形式标记**：编解码器私有的 `Extras::form` 标记。

## 请求

`model` 决定请求的 schema。Jina AI 先校验密钥再校验请求体，因此无法录制它是否拒绝不认识的字段。

| 字段 | KPP | 说明 |
|---|---|---|
| `model` | `RerankRequest.model` 类型化 | |
| `query` | `query` 类型化 | 字符串；`jina-reranker-m0` 的图片查询在 KPP 中没有对应位置，会被拒绝。 |
| 字符串形式的 `documents` | `documents[i].text` 类型化 | |
| `{"text"}` 对象形式的 `documents` | `documents[i].text` 类型化，形式标记 `object` | 写回给 Jina 时为对象；来自 Cohere 的以字符串发送。除 `text` 外还有其他字段的文档或 `{"image"}` 会被拒绝。 |
| `top_n` | `top_n` 类型化 | |
| `return_documents` | `return_documents` 类型化 | Jina 的默认值为 true，Cohere 的为 false：不带该字段的 Jina 请求体解码为 true，编码器总会写出它。 |
| `max_doc_length` | `max_tokens_per_doc` 类型化 | 仅限 `jina-reranker-v3` 和 `jina-reranker-v3.5`；对其他模型，`max_tokens_per_doc` 会被拒绝。 |
| `return_embeddings` 和其他键 | extras | 从 Jina 的请求体中读取；没有应用格式会写出它们。 |

`check()` 拒绝 `cross::check_foreign` 所拒绝的内容，例如 Cohere 的 `relevant_spans` 和 `priority`。

## 回复

| 字段 | KPP | 说明 |
|---|---|---|
| `model` | `model` 类型化 | |
| `object` | 不为 `"list"` 时为形式标记 `object_name`，缺少时为 `no_object` | |
| `results[].index` | `RerankResult.index` 类型化 | |
| `results[].relevance_score` | `relevance_score` 类型化 | 原样传递；Jina 没有记载范围。 |
| 字符串或 `{"text"}` 形式的 `results[].document` | `document` 类型化，对象时为形式标记 `object` | 以发送时的形式返回；Cohere 应用收到 `{"text"}`。`{"image"}` 会作为 Kaeso 无法读取的回复被拒绝。 |
| `results[].embedding` 和其他键 | 结果的 extras | |
| `usage.total_tokens` | `usage.input_tokens` 类型化 | `usage` 保留在 `usage.raw` 中；缺少时为 `no_usage`。 |
| 未知的键 | extras | |

## 流式传输

该端点不流式传输，与 [Cohere Rerank](cohere-rerank.md#流式传输) 相同。

## 错误

Jina AI 以 `{"detail", "code", "request_id"}` 回复，422 时带有 `errors`。当错误码比状态码说明更多时，由错误码决定类型：`AUTHZ_INSUFFICIENT_BALANCE`（403）为 `Billing`，`INPUT_TOKEN_LIMIT_EXCEEDED`（400）为 `ContextTooLong`，`INPUT_MODEL_NOT_FOUND`（400）为 `NotFound`；其他情况由状态码决定，与 [Voyage Rerank](voyage-rerank.md#错误) 相同。没有 `detail` 的响应体按 [OpenAI Responses](openai-responses.md#错误errorresponse) 编解码器的方式读取。Cohere 应用以 `message` 收到该文本。[OpenAI Embeddings](openai-embeddings.md#jina-ai) 编解码器以同样方式读取 Jina 的错误。

## 模型

Jina AI 的 `GET /v1/models` 公开可用，采用 OpenRouter 的格式，ID 为 `jina-ai/<id>`；`GET /v1/models/{id}` 可按短 ID 找到模型。编解码器以 OpenAI 的格式读写模型列表（[模型](openai-chat.md#模型)）。

## 使用此格式的服务商

### Jina AI

Jina AI 的重排序模型，位于服务商基础 URL 下的 `/v1/rerank`（`RerankEgress::of`）：`jina-reranker-v3.5`、`jina-reranker-v3`、`jina-reranker-m0`（文本）、`jina-reranker-v2-base-multilingual`、`jina-reranker-v1-base-en`、`-turbo-en` 和 `-tiny-en`，以及 ColBERT 模型 `jina-colbert-v2` 和 `jina-colbert-v1-en`。
