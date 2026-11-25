# anthropic_messages / providers / ollama-cloud (Ollama Cloud)

Ollama Cloud's Anthropic-compatible Messages API, `POST https://ollama.com/v1/messages`, with `Authorization: Bearer`, which Kaeso sends; `x-api-key` alone is not accepted, and `anthropic-version` must be `2023-06-01` or newer. A request that arrives in this dialect is sent here as it came.

No key was available. The requests are the [Anthropic compatibility](https://docs.ollama.com/api/anthropic-compatibility.md) page's examples, read on 2026-11-25, with the cloud model `gemma4:31b` in place of the local `qwen3-coder`; the page shows no reply, so replies and the stream are constructed with only the fields it lists as supported, and ids are zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/hello.json`, `responses/hello.json` | the page's cloud curl example; usage with only `input_tokens` and `output_tokens` | doc (request, supported response fields); the reply is constructed |
| `requests/tools.json`, `responses/tools.json` | the page's tool example, without `tool_choice`, which Ollama doesn't support; a `tool_use` reply with `stop_reason: "tool_use"` | doc (request); the reply is constructed |
| `requests/count-stream.json`, `streams/count-stream.sse` | the page's streaming example; a `ping` event, listed as supported | doc (request, event list); the stream is constructed |

Not supported, per the page: `/v1/messages/count_tokens`, `tool_choice`, `metadata`, `cache_control`, batches, citations, PDF `document` blocks, images by URL, and `error` events in a stream (errors come as HTTP statuses). `budget_tokens` is accepted but not enforced, and token counts are approximations.
