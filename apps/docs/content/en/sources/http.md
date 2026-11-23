---
title: HTTP
description: Send the requests and tool calls of your own scripts and services to a source, as JSON over HTTP.
order: 8
---

A source of the kind **HTTP** takes batches of requests and tool calls in Kaeso's own JSON. Use it for a script, a job or a service that calls providers without Kaeso and can't export OpenTelemetry.

[Add a source](index.md#add-a-source) of the kind **HTTP**, then post a batch to `/ingest/v1/events` with the source's key:

```bash
curl https://kaeso.example.com/ingest/v1/events \
  -H "Authorization: Bearer sk-kso-src-…" \
  -H "Content-Type: application/json" \
  -d '{"requests":[{"id":"req_0001","duration_ms":1200,"provider":"openai","model":"gpt-5.4-mini","status":200,"usage":{"input_tokens":1200,"output_tokens":300},"user_email":"ada@example.com"}],"tool_calls":[{"id":"call_0001","duration_ms":80,"server":"linear","tool":"list_issues","user_email":"ada@example.com"}]}'
```

Kaeso answers with what it kept:

```json
{ "requests": 1, "tool_calls": 1, "duplicates": 0, "refused": 0, "refusal": null }
```

## Send a batch

A batch is one JSON object with `requests` and `tool_calls`, either of which may be left out. Send at most 5,000 records and 8 MiB a batch, gzipped with `Content-Encoding: gzip` if you like. A record Kaeso can't keep is refused alone: `refused` counts them and `refusal` names the reason of the last one, such as `no_model`. A body Kaeso can't read is answered with `400`, and nothing of it is kept.

Every record needs an `id` that is unique within the source. Send a record again with the same `id` and Kaeso keeps it once, so you can retry a batch that failed.

## Requests

| Field | Meaning |
| --- | --- |
| `id` | Required. Your id of the request |
| `model` | Required. The model, such as `gpt-5.4-mini` |
| `provider` | The provider's id, such as `openai`, which prices the request from KPMR |
| `started_at` | When it started, in RFC 3339; now by default. At most seven days ago |
| `duration_ms`, `ttft_ms` | How long it took, and until the first token |
| `status` | Its HTTP status; 200, or 502 with an `error` |
| `error` | What went wrong, for a failed request |
| `usage` | Its tokens, below |
| `cost_usd` | Its cost in US dollars, used when Kaeso has no price for the model |
| `stream` | Whether the reply was streamed |
| `client` | The app that sent it, such as `nightly-evals/1.4` |
| `user_email` | The member it counts for, when the source matches members by email |
| `tags` | Up to ten tags; Kaeso drops the rest |
| `provider_request_id` | The provider's id of the request |
| `prompt`, `reply` | The last prompt and the reply, kept only when the source keeps content |

`usage` takes `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `cache_write_1h_tokens` and `reasoning_tokens`. By default `input_tokens` leaves out what was read from or written to the cache, as Anthropic counts it. Set `input_includes_cache` to `true` when it holds them, as OpenAI and Gemini count it.

## Tool calls

| Field | Meaning |
| --- | --- |
| `id` | Required. Your id of the call |
| `tool` | Required. The tool's name |
| `server` | The MCP server's name, such as `linear` |
| `started_at`, `duration_ms` | When it started and how long it took |
| `ok` | Whether it succeeded; `true` by default |
| `error` | What went wrong, for a failed call |
| `client`, `user_email` | As for requests |
| `arguments`, `result` | The call's JSON, kept only when the source keeps content |

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [OpenTelemetry](opentelemetry.md): Send GenAI telemetry instead, if your app exports it.
