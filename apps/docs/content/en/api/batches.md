---
title: Batches API
description: Run many requests at a lower price with OpenAI's Batch and Files APIs, within your keys and limits.
order: 5
---

A batch runs many requests to one model at the provider within 24 hours, at a lower price than live requests. Kaeso serves OpenAI's Batch and Files APIs, so the OpenAI SDK's batch flow works unchanged: upload a file of requests, create a batch, ask how it stands, and download the results ([Batch API](https://developers.openai.com/api/docs/guides/batch)).

## Which models

A batch runs at a provider whose batch API takes OpenAI's batch format. Today that is OpenAI, at half the live price. A batch names one model and one endpoint, as OpenAI requires: `/v1/chat/completions`, `/v1/responses` or `/v1/embeddings`. Kaeso doesn't translate batches between formats yet, so every line must be in a format the model's provider takes.

## Run a batch

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-...")

batch_file = client.files.create(file=open("requests.jsonl", "rb"), purpose="batch")
batch = client.batches.create(
    input_file_id=batch_file.id,
    endpoint="/v1/chat/completions",
    completion_window="24h",
)
batch = client.batches.retrieve(batch.id)
if batch.status == "completed":
    results = client.files.content(batch.output_file_id).text
```

Each line of the file is one request: `{"custom_id": "request-1", "method": "POST", "url": "/v1/chat/completions", "body": {"model": "gpt-5", "messages": [...]}}`.

## What Kaeso checks

When you upload the file, Kaeso reads every line as the endpoint it names reads a live request, before anything reaches the provider:

- Every line names the same endpoint and the same model, with a `custom_id` of its own and `method` `POST`, and no stream.
- The model is resolved like a live request's, by its name, `provider/model` or id, and the key must be allowed it. An alias is refused: a batch names one model.
- The model's provider runs batches, and the project's key for it is an API key.

A line that breaks a rule refuses the whole file, with its line number. Kaeso sends the file on with each line's model as the provider names it, so a model whose **Provider model ID** differs runs under that id. The file must fit the model API's body limit (`body_limits.model_api`).

| Method and path | What it does |
| --- | --- |
| `POST /v1/files` | Uploads a batch input file (`purpose` `batch`) |
| `GET /v1/files/{file_id}`, `DELETE /v1/files/{file_id}` | Reads or deletes a file |
| `GET /v1/files/{file_id}/content` | Downloads a file: an input file, or a batch's output or errors |
| `POST /v1/batches`, `GET /v1/batches` | Creates a batch, or lists the project's batches, newest first (`after`, `limit`) |
| `GET /v1/batches/{batch_id}`, `POST /v1/batches/{batch_id}/cancel` | Reads a batch, asking the provider how it stands, or cancels it |

Files and batches have Kaeso ids, and answer only to keys of the project that made them; for any other key they don't exist. They stay at the provider under the provider key that made them: once the project uses another key for the provider, Kaeso can't reach them.

## Costs, limits and logs

- Uploading a file and creating a batch each count as one request against the key's rates. The lines don't count against per-minute rates; creating a batch checks and holds its worst-case tokens and batch-discounted spend against hard token and spend budgets until the batch ends, and refuses features a spend budget can't price.
- When a batch ends, Kaeso reads its output once and records each line as a request of the batch's model, with the provider's usage at the batch price. Lines the provider failed are recorded with their status and no usage. A request log entry is written for each line.
- Kaeso asks the provider about open batches every five minutes, and whenever an app asks for a batch.
