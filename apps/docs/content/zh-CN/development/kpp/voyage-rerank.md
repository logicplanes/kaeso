---
title: Voyage Rerank
description: Voyage AI 重排序格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 Voyage AI 重排序格式的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::voyage_rerank::VoyageRerank`（`crates/kaeso-codecs/src/voyage_rerank/`）
- **端点：**作为 Voyage AI 的格式时为 `https://ai.mongodb.com` 或 Voyage AI 其他主机上的 `POST /v1/rerank`（[支持的服务商](../../concepts/supported-providers.md)）。应用以 [Cohere 的格式](cohere-rerank.md)进行重排序；没有面向应用的端点接受此格式。
- **Fixture：**`crates/kaeso-codecs/fixtures/voyage_rerank/`，包括实际发送的请求、根据 schema 构建的回复，以及在没有有效密钥时实际返回的错误

**来源。** MongoDB 关于 Atlas Embedding and Reranking API 的 [OpenAPI 文档](https://www.mongodb.com/docs/api/doc/atlas-embedding-and-reranking-api.json)（版本 1.1，`rerankDocuments`）、Voyage AI 的[错误码](https://docs.voyageai.com/docs/error-codes)和 MongoDB 的[计费页面](https://www.mongodb.com/docs/voyageai/management/billing/)，查阅于 2026-11-14。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `voyage_rerank`；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **形式标记**：编解码器私有的 `Extras::form` 标记。

## 请求

Voyage AI 会以 400 拒绝它不认识的任何字段，并且先校验请求体再校验密钥（2026-11-14 录制）。编码器只写出下表中的字段。

| 字段 | KPP | 说明 |
|---|---|---|
| `query` | `RerankRequest.query` 类型化 | |
| 字符串形式的 `documents` | `documents[i].text` 类型化 | 只接受字符串：应用写成 `{"text"}` 的文档以其文本发送，除 `text` 外还有其他字段的文档会被拒绝（`document fields`）。 |
| `model` | `model` 类型化 | |
| `top_k` | `top_n` 类型化 | 即 Cohere 的 `top_n`；Voyage AI 自己会拒绝 `top_n`。 |
| `return_documents` | `return_documents` 类型化 | 仅在应用设置时发送；Voyage AI 的默认值为 false，与 Cohere 相同。 |
| `truncation` 和其他键 | extras | 从 Voyage AI 的请求体中读取；没有应用格式会写出它们。 |
| `max_tokens_per_doc` | 拒绝 | Voyage AI 会在模型的上限处截断每个文档，并拒绝该字段。 |

`check()` 拒绝 `cross::check_foreign` 所拒绝的内容，例如 Cohere 的 `relevant_spans` 和 `priority`。

## 回复

| 字段 | KPP | 说明 |
|---|---|---|
| `object` | 不为 `"list"` 时为形式标记 `object` | |
| `data[].index` | `RerankResult.index` 类型化 | Voyage AI 的键是 `data`，而不是 Cohere 的 `results`。 |
| `data[].relevance_score` | `relevance_score` 类型化 | 原样传递；Voyage AI 没有记载范围。 |
| `data[].document` | `document` 类型化 | 字符串；Cohere 应用以 `{"text"}` 收到它。 |
| `model` | `model` 类型化 | |
| `usage.total_tokens` | `usage.input_tokens` 类型化 | Voyage AI 按查询的 token 数乘以文档数、再加上每个文档的 token 数计费，并报告这个总和。`usage` 保留在 `usage.raw` 中。 |
| 缺少 `usage` | 形式标记 `no_usage` | |
| 未知的键 | extras | |

## 流式传输

该端点不流式传输，与 [Cohere Rerank](cohere-rerank.md#流式传输) 相同。

## 错误

Voyage AI 以 `{"detail": "<文本>"}` 回复，不带错误码。状态码决定类型：400 和 422 为 `InvalidRequest`，401 为 `Authentication`，402 为 `Billing`，403 为 `PermissionDenied`，404 为 `NotFound`，413 为 `RequestTooLarge`，429 为带 `retry-after` 的 `RateLimited`，503 为 `Overloaded`，408 和 504 为 `Timeout`，其余为 `ProviderError`。没有 `detail` 的响应体按 [OpenAI Responses](openai-responses.md#错误errorresponse) 编解码器的方式读取。Cohere 应用以 `message` 收到该文本。

## 模型

Voyage AI 不提供模型列表：`GET /v1/models` 和 `GET /v1/models/{model}` 返回 404（2026-11-14 录制），因此**检查可用性**会将它的模型判为无法确定。编解码器以 OpenAI 的格式读写模型列表（[模型](openai-chat.md#模型)）。

## 使用此格式的服务商

### Voyage AI

Voyage AI 的重排序模型 `rerank-3`、`rerank-3-lite`、`rerank-2.5`、`rerank-2.5-lite`、`rerank-2` 和 `rerank-2-lite`，位于服务商基础 URL 下的 `/v1/rerank`（`RerankEgress::of`）。
