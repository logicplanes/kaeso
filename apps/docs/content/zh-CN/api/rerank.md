---
title: 重排序 API
description: 使用 Cohere 的重排序格式，按文档回答查询的程度为其排序，可用于 Kaeso 调用其重排序模型的每个服务商。
order: 13
---

应用发送一个查询和一组文档，重排序模型返回每个文档的相关性分数，最相关的排在最前。检索流水线用它挑选交给聊天模型的段落。Kaeso 接受 Cohere 的重排序格式，即 Cohere SDK、LangChain、LlamaIndex 和大多数重排序客户端发送的格式，并调用每个服务商自己的重排序 API。

```bash tab="curl"
curl https://kaeso.example.com/v1/rerank \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "rerank-v5.0-fast", "query": "Which plan includes priority support?", "documents": ["Starter includes email support.", "Business includes priority support by phone and chat.", "All plans include the community forum."], "top_n": 2}'
```

```python tab="Cohere SDK"
import os
import cohere

client = cohere.ClientV2(api_key=os.environ["KAESO_API_KEY"], base_url="https://kaeso.example.com")

reply = client.rerank(
    model="rerank-v5.0-fast",
    query="Which plan includes priority support?",
    documents=[
        "Starter includes email support.",
        "Business includes priority support by phone and chat.",
        "All plans include the community forum.",
    ],
    top_n=2,
)
for result in reply.results:
    print(result.index, result.relevance_score)
```

```ts tab="Cohere SDK (TypeScript)"
import { CohereClientV2 } from "cohere-ai";

const client = new CohereClientV2({ token: process.env.KAESO_API_KEY, environment: "https://kaeso.example.com" });

const reply = await client.rerank({
  model: "rerank-v5.0-fast",
  query: "Which plan includes priority support?",
  documents: ["Starter includes email support.", "Business includes priority support by phone and chat.", "All plans include the community forum."],
  topN: 2,
});
console.log(reply.results);
```

回复按文档在请求中的位置列出各文档：

```json
{
  "results": [
    { "index": 1, "relevance_score": 0.93 },
    { "index": 0, "relevance_score": 0.19 }
  ],
  "id": "1a6f2c4e-0000-0000-0000-000000000000",
  "meta": { "api_version": { "version": "2" }, "billed_units": { "input_tokens": 61 } }
}
```

## 端点

| 端点 | 格式 | 密钥 | 请求体 |
| --- | --- | --- | --- |
| `POST /v1/rerank`、`POST /v2/rerank` | Cohere 的重排序格式 | `Authorization: Bearer` | JSON：`model`、`query`、`documents`、`top_n`、`max_tokens_per_doc`、`return_documents` |

两个路径接受相同的请求体：Cohere SDK 发往 `/v2/rerank`，大多数其他客户端发往 `/v1/rerank`。重排序回复从不流式传输，请求体与聊天请求一样受 [`body_limits.model_api`](../self-hosting/configuration.md#请求体与响应体) 限制。一个请求最多包含 10,000 个文档；服务商接受得更少：Cohere 建议最多 1,000 个，Voyage AI 最多接受 1,000 个。

| 字段 | 含义 |
| --- | --- |
| `model` | 重排序模型的任意[模型名称](model-api.md#模型名称)，或别名。 |
| `query` | 用来为文档打分的查询。 |
| `documents` | 要打分的文本，可以是字符串，也可以是 `{"text": "…"}` 对象。 |
| `top_n` | 回复列出多少个最相关的文档；省略时列出全部。 |
| `max_tokens_per_doc` | 服务商在何处截断每个文档。 |
| `return_documents` | 每个结果是否以 `{"text": "…"}` 带上其文档；省略时为 false。Cohere 的 v2 格式没有这个字段，因此当服务商省略文档时，Kaeso 会从请求中补上。 |

Cohere 自己的字段，例如 `relevant_spans`（Context Focus）和 `priority`，原样发给 Cohere，回复中的 `relevant_spans` 也会返回。没有对应位置的服务商会在发送任何内容之前拒绝该请求。

密钥、通过别名和路由解析模型、限额和预算、请求日志，以及 Cohere 格式 `{"id", "message"}` 的错误，都与其他模型请求相同。

## 服务商

| 服务商 | 发送的格式 | 路径 | 模型 |
| --- | --- | --- | --- |
| Cohere | Cohere Rerank v2 | `/v2/rerank`，位于 Cohere 主机的根路径，与其兼容 API 并列 | Rerank 5，即 `rerank-v5.0-pro` 和 `rerank-v5.0-fast` |
| Jina AI | Jina Rerank（[映射](../development/kpp/jina-rerank.md)） | `/v1/rerank` | `jina-reranker-v3.5`、`jina-reranker-v3`、`jina-reranker-m0`（仅文本）、`jina-reranker-v2-base-multilingual`、v1 重排序模型和 ColBERT 模型 |
| Voyage AI | Voyage Rerank（[映射](../development/kpp/voyage-rerank.md)） | `/v1/rerank` | `rerank-3`、`rerank-3-lite`，以及较早的 `rerank-2.5`、`rerank-2.5-lite`、`rerank-2` 和 `rerank-2-lite` |

Jina AI 总会收到 `return_documents`，因为除非明确关闭，它会返回文档；对 `jina-reranker-v3` 和 `jina-reranker-v3.5`，`max_tokens_per_doc` 以 `max_doc_length` 发送；其 `usage.total_tokens` 以 `billed_units.input_tokens` 返回。Voyage AI 收到的 `top_n` 为 `top_k`，每个文档为其文本；其回复中的文档以 `{"text": "…"}` 返回，其 `usage.total_tokens` 以 `billed_units.input_tokens` 返回。

[KPMR](../kpmr/index.md) 将其类型标为 `reranking` 的模型就是重排序模型。Kaeso 尚未调用其重排序 API 的服务商（例如 NVIDIA）的重排序模型，会在发送任何内容之前被拒绝：“Kaeso calls no rerank models on NVIDIA, so `nvidia/rerank-qa-mistral-4b` can't rerank documents.”

## 拒绝的内容

Kaeso 在发送之前拒绝以下请求，并以 Cohere 的格式返回 400：

- 生成模型或嵌入模型：“The model `command-a-03-2025` is a generation model; it cannot rerank documents.”；
- 空的 `documents` 列表、为 0 的 `top_n` 或 `max_tokens_per_doc`，或者既不是字符串也不是带 `text` 的对象的文档；
- 发给只接受文本的服务商（例如 Cohere、Jina AI 和 Voyage AI）且除 `text` 外还有其他字段的文档；
- 发给 Voyage AI 的 `max_tokens_per_doc`（Voyage AI 会自行在模型的上限处截断每个文档），以及发给 `jina-reranker-v3` 和 `jina-reranker-v3.5` 以外的 Jina AI 重排序模型的 `max_tokens_per_doc`；
- 发给 Cohere 以外服务商的 Cohere 自有字段，例如 `relevant_spans`。

重排序模型拒绝聊天、token 计数和嵌入：“The model `rerank-v5.0-fast` is a rerank model; send it to `/v1/rerank`.”

## 分数、费用、缓存和日志

- **分数**由服务商给出，原样传递。Cohere 的分数介于 0 和 1 之间；其他服务商没有记载范围，因此不要跨服务商比较分数。
- **费用。** 重排序请求按服务商计费的 token 数和模型的输入价格计费。Cohere 的 Rerank 5 以 `billed_units.input_tokens` 报告这些 token；Voyage AI 按查询的 token 数乘以文档数、再加上每个文档的 token 数计费，并以 `usage.total_tokens` 报告，Jina AI 也以同样方式报告其计费的 token。更早的 Cohere 模型按搜索单位计费，Kaeso 会记录它们，但无法为其定价。没有输入价格的模型费用为**未知**，[预算](../console/limits-and-budgets.md)会拒绝它。预算按服务商的计费方式，为每个文档预留一次查询。
- **响应缓存。** [响应缓存](../console/response-cache.md)像回答重复的聊天请求一样回答重复的重排序请求。
- **日志。** [请求日志](../console/request-logs.md)显示端点 `rerank`、请求和打分后的结果。

## 后续步骤

- [Cohere Rerank](../development/kpp/cohere-rerank.md)：了解每个字段如何映射到 KPP。
- [模型和别名](../console/models-and-aliases.md)：添加重排序模型并在它们之间路由。
- [各服务商的 API](../concepts/provider-apis.md#重排序)：查看 Kaeso 通过哪些服务商进行重排序。
