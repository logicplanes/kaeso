---
title: HTTP
description: 以 JSON 通过 HTTP，把您自己的脚本和服务的请求和工具调用发送到来源。
order: 8
---

类型为 **HTTP** 的来源接受采用 Kaeso 自有 JSON 格式的请求和工具调用批次。它适用于绕过 Kaeso 调用服务商、且无法导出 OpenTelemetry 的脚本、任务或服务。

[添加一个](index.md#添加来源)类型为 **HTTP** 的来源，然后用来源密钥把一个批次发送到 `/ingest/v1/events`：

```bash
curl https://kaeso.example.com/ingest/v1/events \
  -H "Authorization: Bearer sk-kso-src-…" \
  -H "Content-Type: application/json" \
  -d '{"requests":[{"id":"req_0001","duration_ms":1200,"provider":"openai","model":"gpt-5.4-mini","status":200,"usage":{"input_tokens":1200,"output_tokens":300},"user_email":"ada@example.com"}],"tool_calls":[{"id":"call_0001","duration_ms":80,"server":"linear","tool":"list_issues","user_email":"ada@example.com"}]}'
```

Kaeso 会回答它保留了哪些内容：

```json
{ "requests": 1, "tool_calls": 1, "duplicates": 0, "refused": 0, "refusal": null }
```

## 发送批次

批次是一个包含 `requests` 和 `tool_calls` 的 JSON 对象，两者都可以省略。每批最多发送 5,000 条记录、8 MiB，也可以使用 `Content-Encoding: gzip` 压缩。Kaeso 无法保留的记录会被单独拒绝：`refused` 统计被拒绝的数量，`refusal` 给出最后一次拒绝的原因，例如 `no_model`。Kaeso 无法读取的请求体会得到 `400`，其中的任何内容都不会保留。

每条记录都需要一个在来源内唯一的 `id`。以相同的 `id` 再次发送记录时，Kaeso 只保留一次，因此您可以重试失败的批次。

## 请求

| 字段 | 含义 |
| --- | --- |
| `id` | 必填。您为请求指定的 ID |
| `model` | 必填。模型，例如 `gpt-5.4-mini` |
| `provider` | 服务商的 ID，例如 `openai`，用于按 KPMR 为请求定价 |
| `started_at` | 开始时间，采用 RFC 3339；默认为当前时间。最早为七天前 |
| `duration_ms`、`ttft_ms` | 耗时，以及到首 Token 的时间 |
| `status` | HTTP 状态；默认为 200，带有 `error` 时为 502 |
| `error` | 失败请求出了什么问题 |
| `usage` | 请求的 Token，见下文 |
| `cost_usd` | 以美元计的费用，在 Kaeso 没有该模型价格时使用 |
| `stream` | 回复是否以流式返回 |
| `client` | 发送请求的应用，例如 `nightly-evals/1.4` |
| `user_email` | 来源按邮箱匹配成员时，请求计入的成员 |
| `tags` | 最多十个标签，与网关的 `kaeso-tags` 请求头接受的标签相同 |
| `provider_request_id` | 服务商的请求 ID |
| `prompt`、`reply` | 最后一个提示词和回复，仅在来源保留内容时保留 |

`usage` 接受 `input_tokens`、`output_tokens`、`cache_read_tokens`、`cache_write_tokens`、`cache_write_1h_tokens` 和 `reasoning_tokens`。默认情况下，`input_tokens` 不含从缓存读取或写入缓存的部分，与 Anthropic 的计数方式相同。如果它包含这些部分（如 OpenAI 和 Gemini 的计数方式），请把 `input_includes_cache` 设为 `true`。

## 工具调用

| 字段 | 含义 |
| --- | --- |
| `id` | 必填。您为调用指定的 ID |
| `tool` | 必填。工具名称 |
| `server` | MCP 服务器的名称，例如 `linear` |
| `started_at`、`duration_ms` | 开始时间和耗时 |
| `ok` | 是否成功；默认为 `true` |
| `error` | 失败的调用出了什么问题 |
| `client`、`user_email` | 与请求相同 |
| `arguments`、`result` | 调用的 JSON，仅在来源保留内容时保留 |

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [OpenTelemetry](opentelemetry.md)：如果您的应用能导出 GenAI 遥测数据，可以改为发送它。
