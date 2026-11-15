---
title: Rerank API
description: Order documents by how well they answer a query, with Cohere's rerank format, on every provider whose rerank models Kaeso calls.
order: 12
---

Apps send a query and a list of documents, and a rerank model answers with each document's relevance score, best first. Retrieval pipelines use it to pick the passages they hand to a chat model. Kaeso takes Cohere's rerank format, which the Cohere SDK, LangChain, LlamaIndex and most rerank clients send, and calls each provider's own rerank API.

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

The reply lists the documents by their position in the request:

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

## Endpoint

| Endpoint | Format | Key | Body |
| --- | --- | --- | --- |
| `POST /v1/rerank`, `POST /v2/rerank` | Cohere's rerank format | `Authorization: Bearer` | JSON: `model`, `query`, `documents`, `top_n`, `max_tokens_per_doc`, `return_documents` |

Both paths take the same body: `/v2/rerank` is where the Cohere SDK sends it, `/v1/rerank` where most other clients do. Rerank replies are never streamed, and the body is bounded by [`body_limits.model_api`](../self-hosting/configuration.md#request-and-response-bodies), like chat requests. A request holds at most 10,000 documents; providers take fewer, and Cohere recommends at most 1,000.

| Field | Meaning |
| --- | --- |
| `model` | Any [model name](model-api.md#model-names) of a rerank model, or an alias. |
| `query` | What the documents are scored against. |
| `documents` | The texts to score, as strings or as `{"text": "…"}` objects. |
| `top_n` | How many of the best documents the reply lists; all when left out. |
| `max_tokens_per_doc` | Where the provider cuts off each document. |
| `return_documents` | Whether each result carries its document, as `{"text": "…"}`; false when left out. Cohere's v2 format has no such field, so Kaeso fills the documents in from the request when the provider leaves them out. |

Cohere's own fields, such as `relevant_spans` (Context Focus) and `priority`, go to Cohere as written, and the reply's `relevant_spans` come back. A provider that has no place for one refuses the request before anything is sent.

Keys, model resolution with aliases and routes, limits and budgets, request logs and errors in Cohere's shape, `{"id", "message"}`, work as for every other model request.

## Providers

| Provider | Format sent | Path | Models |
| --- | --- | --- | --- |
| Cohere | Cohere Rerank v2 | `/v2/rerank`, at the root of Cohere's host beside its compatibility API | Rerank 5, `rerank-v5.0-pro` and `rerank-v5.0-fast` |

A model is a rerank model when [KPMR](../concepts/kpmr.md) gives it the kind `reranking`. Rerank models of a provider whose rerank API Kaeso doesn't call yet, such as NVIDIA's, are refused before anything is sent: "Kaeso calls no rerank models on NVIDIA, so `nvidia/rerank-qa-mistral-4b` can't rerank documents.".

## What is refused

Kaeso refuses before sending, with a 400 in Cohere's shape:

- a generation or embeddings model: "The model `command-a-03-2025` is a generation model; it cannot rerank documents.";
- an empty `documents` list, a `top_n` or `max_tokens_per_doc` of 0, or a document that is neither a string nor an object with `text`;
- a document with fields beside `text` for a provider that takes text only, such as Cohere.

A rerank model refuses chat, token counts and embeddings: "The model `rerank-v5.0-fast` is a rerank model; send it to `/v1/rerank`.".

## Scores, costs, cache and logs

- **Scores** are the provider's, passed on unchanged. Cohere's lie between 0 and 1; other providers don't document a range, so don't compare scores across providers.
- **Costs.** A rerank request costs the tokens the provider bills at the model's input price. Cohere's Rerank 5 reports them as `billed_units.input_tokens`. Older Cohere models bill search units, which Kaeso logs but can't price. A model without an input price costs **Unknown**, and [budgets](../console/limits-and-budgets.md) refuse it. Budgets reserve the query once per document, as providers bill it.
- **Response cache.** The [response cache](../console/response-cache.md) answers a repeated rerank request like a repeated chat request.
- **Logs.** The [request log](../console/request-logs.md) shows the endpoint `rerank`, the request and the scored results.

## Next steps

- [Cohere Rerank](../development/kpp/cohere-rerank.md): See how each field maps to KPP.
- [Models and aliases](../console/models-and-aliases.md): Add rerank models and route between them.
- [APIs by provider](../concepts/provider-apis.md#rerank): See which providers Kaeso reranks with.
