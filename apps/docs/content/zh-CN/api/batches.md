---
title: 批处理 API
description: 使用 OpenAI 的 Batch 和 Files API，以更低价格运行大量请求，同时遵守你的密钥和限额。
order: 5
---

批处理会在服务商处于 24 小时内运行发给同一模型的大量请求，价格低于实时请求。Kaeso 提供 OpenAI 的 Batch 和 Files API，因此 OpenAI SDK 的批处理流程无需改动即可使用：上传请求文件、创建批处理、查询其状态，然后下载结果（见 [Batch API](https://developers.openai.com/api/docs/guides/batch)）。

## 支持哪些模型

批处理在其批处理 API 接受 OpenAI 批处理格式的服务商处运行。目前是 OpenAI，价格为实时价格的一半。按照 OpenAI 的要求，一个批处理只能指定一个模型和一个端点：`/v1/chat/completions`、`/v1/responses` 或 `/v1/embeddings`。Kaeso 暂不在格式之间转换批处理，因此每一行都必须使用模型所属服务商接受的格式。

## 运行批处理

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

文件的每一行是一个请求：`{"custom_id": "request-1", "method": "POST", "url": "/v1/chat/completions", "body": {"model": "gpt-5", "messages": [...]}}`。

## Kaeso 检查什么

上传文件时，Kaeso 会像所指端点读取实时请求那样读取每一行，在任何内容到达服务商之前：

- 每一行指定相同的端点和相同的模型，各有自己的 `custom_id`，`method` 为 `POST`，且不使用流式。
- 模型的解析方式与实时请求相同，可以是名称、`provider/model` 或 ID，且密钥必须有权使用它。别名会被拒绝：批处理只能指定一个模型。
- 模型所属服务商支持批处理，且项目在该服务商的密钥是 API 密钥。

任何一行违反规则，整个文件都会被拒绝，并注明行号。Kaeso 转发文件时，会把每一行的模型改写为服务商使用的名称，因此**服务商模型 ID** 不同的模型会以该 ID 运行。文件大小必须在模型 API 的请求体上限之内（`body_limits.model_api`）。

| 方法和路径 | 作用 |
| --- | --- |
| `POST /v1/files` | 上传批处理输入文件（`purpose` 为 `batch`） |
| `GET /v1/files/{file_id}`、`DELETE /v1/files/{file_id}` | 读取或删除文件 |
| `GET /v1/files/{file_id}/content` | 下载文件：输入文件，或批处理的输出或错误 |
| `POST /v1/batches`、`GET /v1/batches` | 创建批处理，或列出项目的批处理，最新的在前（`after`、`limit`） |
| `GET /v1/batches/{batch_id}`、`POST /v1/batches/{batch_id}/cancel` | 读取批处理（同时向服务商查询其状态），或取消它 |

文件和批处理有自己的 Kaeso ID，只响应创建它们的项目的密钥；对其他密钥而言它们不存在。它们保存在服务商处、创建它们的服务商密钥之下：一旦项目改用该服务商的其他密钥，Kaeso 就无法再访问它们。

## 费用、限额和日志

- 上传文件和创建批处理各计为一次请求，计入密钥的速率。各行不计入每分钟速率。
- 批处理结束时，Kaeso 读取其输出一次，并把每一行记录为发给批处理模型的请求，按批处理价格计算服务商报告的用量。服务商处理失败的行会以其状态记录，不含用量。每一行都会写入一条请求日志。
- Kaeso 每五分钟向服务商查询一次未结束的批处理，应用查询某个批处理时也会查询。
