---
title: Cohere Rerank
description: Cohere 重排序格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 Cohere 重排序格式的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::cohere_rerank::CohereRerank`（`crates/kaeso-codecs/src/cohere_rerank/`）
- **端点：**作为应用的格式时为 `POST /v1/rerank` 和 `POST /v2/rerank`（[重排序 API](../../api/rerank.md)）；作为服务商的格式时为 Cohere 主机根路径下的 `POST /v2/rerank`
- **Fixture：**`crates/kaeso-codecs/fixtures/cohere_rerank/`，来自 Cohere 参考文档中的示例和 schema，其中两个拒绝密钥的回复为实际录制

**来源。** Cohere 的 [Rerank v2 参考](https://docs.cohere.com/reference/rerank)、[Rerank v1 参考](https://docs.cohere.com/v1/reference/rerank)和[重排序模型](https://docs.cohere.com/docs/rerank)页面，查阅于 2026-11-14。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `cohere_rerank`；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **形式标记**：编解码器私有的 `Extras::form` 标记；`verbatim` 保留已声明键的 `null` 写法。

## 请求

重排序请求解码为 `Request::Rerank(RerankRequest)`。作为应用的格式时，编解码器读取 Cohere 的 v2 请求体，以及大多数重排序客户端额外发送的 v1 字段；作为服务商的格式时，它写出 v2。

| 字段 | KPP | 说明 |
|---|---|---|
| `model` | `RerankRequest.model` 类型化 | 必填，与 v2 相同。 |
| `query` | `query` 类型化 | 必填。 |
| 字符串形式的 `documents` | `documents[i].text` 类型化 | |
| `{"text"}` 对象形式的 `documents`（v1） | `documents[i].text` 类型化，形式标记 `object` | 对象的其他键保留为该文档的 extras。写给 Cohere 时为字符串；带有其他键的文档在那里会被拒绝（`document fields`），因为 v2 只接受文本。 |
| `top_n` | `top_n` 类型化 | `0` 会被拒绝（`InvalidRequest`）。 |
| `max_tokens_per_doc` | `max_tokens_per_doc` 类型化 | `0` 会被拒绝。 |
| `return_documents`（v1） | `return_documents` 类型化 | 不写给 Cohere，因为其 v2 没有这个字段；回复从应用的请求中取得文档。 |
| `relevant_spans`、`priority`、`rank_fields` 和其他键 | extras | 原样发给 Cohere。 |
| 超过 10,000 个 `documents` | `InvalidRequest` | `limits::MAX_RERANK_DOCUMENTS`，在解码之前计数。 |

`decode_request` 拒绝其他任何类型的请求，其他每个编解码器也会拒绝重排序请求，并提示使用 `/v1/rerank`。`check()` 在请求和每个文档上拒绝 `cross::check_foreign` 所拒绝的内容，例如其他方言的 extras。

## 回复

| 字段 | KPP | 说明 |
|---|---|---|
| `results[].index` | `RerankResult.index` 类型化 | 文档在请求中的位置。 |
| `results[].relevance_score` | `relevance_score` 类型化 | 原样传递；Cohere 的分数介于 0 和 1 之间。 |
| 字符串或 `{"text"}` 形式的 `results[].document` | `document` 类型化 | v1 回复和其他服务商的回复带有它；对象形式时带形式标记 `object`。来自其他方言时写为 `{"text"}`；当应用用 `return_documents` 要求文档而服务商没有发送时，从应用的请求中补上。 |
| `results[].relevant_spans` 和其他键 | 该结果的 extras | |
| `id` | `RerankResponse.id` 类型化 | 回复中没有时省略。 |
| `meta.billed_units.input_tokens` | `usage.input_tokens` 类型化 | Rerank 5 按 token 计费。`meta` 保留在 `usage.raw` 中，并原样写回。 |
| `meta.billed_units.search_units` | `search_units` 类型化 | Rerank 3.5 和 4 按搜索单位计费。 |
| 缺少 `meta` | 形式标记 `no_meta` | 不会为没有 `meta` 的回复虚构一个。 |
| 未知键 | extras | |

来自其他方言的回复会得到 `meta: {"api_version": {"version": "2"}, "billed_units": {…}}`，其中包含其输入 token、搜索单位或两者。

## 流式传输

该端点不流式传输。流编码器对每个事件都返回 `Unsupported`（`streaming`）；流解码器报告一个 `StreamEvent::Error`，之后不再输出任何内容。

## 错误

Cohere 返回 `{"id", "message"}`。状态码决定错误类型：400 和 422 为 `InvalidRequest`，401 为 `Authentication`，402 为 `Billing`，403 为 `PermissionDenied`，404 为 `NotFound`，429 为带 `retry-after` 的 `RateLimited`，498（被屏蔽的 token）为 `ContentFiltered`，503 为 `Overloaded`，504 为 `Timeout`，其余为 `ProviderError`。Cohere 自己的错误体保留在 `KppError.raw` 中，并原样再次发送给 Cohere Rerank 应用。其他任何错误以 `{"id", "message"}` 的形式到达应用，`id` 为网关的请求 ID，状态码由错误类型决定。没有 `message` 的错误体按 [OpenAI Responses](openai-responses.md#错误errorresponse) 编解码器的方式读取。

## 模型

不提供 Cohere 自己的模型列表：`GET /v1/models` 以 OpenAI 的结构回答（[模型](openai-chat.md#模型)），并像列出其他模型一样列出重排序模型。

## 使用此格式的服务商

### Cohere

Cohere 的重排序 API 是其主机根路径下的 `POST /v2/rerank`，与 Kaeso 中 Cohere 基础 URL 所指向的兼容 API 并列（`RerankEgress::at_root`）。其重排序模型为 Rerank 5，即 `rerank-v5.0-pro` 和 `rerank-v5.0-fast`，按 token 计费。按搜索单位计费的 Rerank 4 和 3.5 不在 KPMR 的 Cohere 目录中。
